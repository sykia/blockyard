mod auth;
mod catalog;
mod download;
mod engine;
#[cfg(target_os = "linux")]
mod hyprland;
mod java;
mod metadata;
mod model;
mod neoforge;
mod offline;
mod store;
mod updates;
use model::{Account, AccountKind, Database, Instance, Loader, ModFile, Settings, VersionChoice};
use std::{path::PathBuf, sync::Arc};
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;
struct State {
    db: Arc<Mutex<Database>>,
    client: reqwest::Client,
    root: PathBuf,
    running: Arc<Mutex<Vec<String>>>,
}
fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(format!("Blockyard/{}", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn snapshot(state: tauri::State<'_, State>) -> Result<Database, String> {
    let mut db = state.db.lock().await;
    let snapshot = db.clone();
    db.warnings.clear();
    Ok(snapshot)
}
#[tauri::command]
async fn versions(state: tauri::State<'_, State>) -> Result<Vec<VersionChoice>, String> {
    let show = state.db.lock().await.settings.show_snapshots;
    Ok(engine::manifest(&state.client, &state.root)
        .await?
        .versions
        .into_iter()
        .filter(|v| v.kind == "release" || show && v.kind == "snapshot")
        .map(|v| VersionChoice {
            id: v.id,
            kind: v.kind,
            release_time: v.release_time,
        })
        .collect())
}
#[tauri::command]
async fn fabric_versions(
    state: tauri::State<'_, State>,
    minecraft: String,
) -> Result<Vec<String>, String> {
    let url = format!("https://meta.fabricmc.net/v2/versions/loader/{minecraft}");
    let value: Vec<serde_json::Value> = state
        .client
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(value
        .into_iter()
        .filter_map(|v| {
            v.pointer("/loader/version")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
        })
        .collect())
}
#[tauri::command]
async fn neoforge_versions(
    state: tauri::State<'_, State>,
    minecraft: String,
) -> Result<Vec<String>, String> {
    neoforge::versions(&state.client, &minecraft).await
}
#[tauri::command]
async fn create_instance(
    state: tauri::State<'_, State>,
    name: String,
    version: String,
    loader: Loader,
) -> Result<Instance, String> {
    if name.trim().is_empty() || name.len() > 80 {
        return Err("Instance name must be 1–80 characters".into());
    }
    let id = uuid::Uuid::new_v4().to_string();
    let mut db = state.db.lock().await;
    let instance = Instance {
        id: id.clone(),
        name: name.trim().into(),
        version,
        loader,
        java_path: None,
        ram_mb: db.settings.ram_mb,
        jvm_args: String::new(),
        width: db.settings.width,
        height: db.settings.height,
        icon: "cube".into(),
        last_played: None,
        playtime_seconds: 0,
    };
    std::fs::create_dir_all(state.root.join("instances").join(&id).join("game"))
        .map_err(|e| e.to_string())?;
    db.instances.push(instance.clone());
    store::save(&state.root, &db)?;
    Ok(instance)
}
#[tauri::command]
async fn save_instance(state: tauri::State<'_, State>, instance: Instance) -> Result<(), String> {
    if instance.name.trim().is_empty()
        || instance.ram_mb < 512
        || instance.width < 320
        || instance.height < 240
    {
        return Err("Invalid instance settings".into());
    }
    let mut db = state.db.lock().await;
    let old = db
        .instances
        .iter_mut()
        .find(|i| i.id == instance.id)
        .ok_or("Instance not found")?;
    *old = instance;
    store::save(&state.root, &db)
}
#[tauri::command]
async fn delete_instance(state: tauri::State<'_, State>, id: String) -> Result<(), String> {
    if state.running.lock().await.contains(&id) {
        return Err("Cannot delete a running instance".into());
    }
    let mut db = state.db.lock().await;
    let index = db
        .instances
        .iter()
        .position(|i| i.id == id)
        .ok_or("Instance not found")?;
    std::fs::remove_dir_all(state.root.join("instances").join(&id)).map_err(|e| e.to_string())?;
    db.instances.remove(index);
    store::save(&state.root, &db)
}
#[tauri::command]
async fn save_settings(state: tauri::State<'_, State>, settings: Settings) -> Result<(), String> {
    if settings.download_concurrency == 0
        || settings.download_concurrency > 32
        || settings.ram_mb < 512
        || !matches!(settings.theme.as_str(), "dark" | "light" | "system")
    {
        return Err("Invalid settings".into());
    }
    let mut db = state.db.lock().await;
    db.settings = settings;
    store::save(&state.root, &db)
}
#[tauri::command]
async fn start_login(state: tauri::State<'_, State>) -> Result<auth::DeviceCode, String> {
    let id = state.db.lock().await.settings.microsoft_client_id.clone();
    auth::start(&state.client, &id).await
}
#[tauri::command]
async fn finish_login(
    state: tauri::State<'_, State>,
    code: auth::DeviceCode,
) -> Result<Account, String> {
    let id = state.db.lock().await.settings.microsoft_client_id.clone();
    let account = auth::finish(&state.client, &id, &code).await?;
    let mut db = state.db.lock().await;
    db.accounts.retain(|a| a.id != account.id);
    db.accounts.push(account.clone());
    db.active_account = Some(account.id.clone());
    store::save(&state.root, &db)?;
    Ok(account)
}
#[tauri::command]
async fn add_offline_account(
    state: tauri::State<'_, State>,
    name: String,
) -> Result<Account, String> {
    let account = offline::account(&name)?;
    let mut db = state.db.lock().await;
    if let Some(existing) = db.accounts.iter().find(|a| a.id == account.id) {
        if existing.kind != AccountKind::Offline {
            return Err("This player ID is already used by a Microsoft account".into());
        }
    } else {
        db.accounts.push(account.clone());
    }
    db.active_account = Some(account.id.clone());
    store::save(&state.root, &db)?;
    Ok(account)
}
#[tauri::command]
async fn select_account(state: tauri::State<'_, State>, id: String) -> Result<(), String> {
    let mut db = state.db.lock().await;
    if !db.accounts.iter().any(|a| a.id == id) {
        return Err("Account not found".into());
    }
    db.active_account = Some(id);
    store::save(&state.root, &db)
}
#[tauri::command]
async fn logout(state: tauri::State<'_, State>, id: String) -> Result<(), String> {
    let mut db = state.db.lock().await;
    let account = db
        .accounts
        .iter()
        .find(|a| a.id == id)
        .ok_or("Account not found")?;
    if account.kind == AccountKind::Microsoft {
        auth::logout(&id)?;
    }
    db.accounts.retain(|a| a.id != id);
    if db.active_account.as_deref() == Some(&id) {
        db.active_account = db.accounts.first().map(|a| a.id.clone())
    }
    store::save(&state.root, &db)
}
#[tauri::command]
async fn play(
    app: tauri::AppHandle,
    state: tauri::State<'_, State>,
    id: String,
) -> Result<(), String> {
    let db = state.db.lock().await.clone();
    let instance = db
        .instances
        .iter()
        .find(|i| i.id == id)
        .cloned()
        .ok_or("Instance not found")?;
    let account_id = db
        .active_account
        .as_ref()
        .ok_or("Add an account before playing")?;
    let mut account = db
        .accounts
        .iter()
        .find(|a| &a.id == account_id)
        .cloned()
        .ok_or("Active account not found")?;
    {
        let mut running = state.running.lock().await;
        if running.contains(&id) {
            return Err("Instance is already running".into());
        }
        running.push(id.clone());
    }
    let root = state.root.clone();
    let client = state.client.clone();
    let running = state.running.clone();
    let database = state.db.clone();
    tauri::async_runtime::spawn(async move {
        let result = async {
            let token = if account.kind == AccountKind::Offline {
                "0".to_owned()
            } else {
                let token =
                    auth::access(&client, &db.settings.microsoft_client_id, &mut account).await?;
                {
                    let mut current = database.lock().await;
                    if let Some(item) = current.accounts.iter_mut().find(|a| a.id == account.id) {
                        *item = account.clone();
                    }
                    store::save(&root, &current)?;
                }
                token
            };
            engine::run(
                app.clone(),
                client,
                root.clone(),
                instance,
                account,
                token,
                db.settings.download_concurrency,
                db.settings.java_path,
                db.settings.minimize_on_launch,
            )
            .await
        }
        .await;
        match result {
            Ok(seconds) => {
                let mut current = database.lock().await;
                if let Some(item) = current.instances.iter_mut().find(|i| i.id == id) {
                    item.playtime_seconds += seconds;
                    item.last_played = Some(chrono::Utc::now().to_rfc3339());
                }
                let _ = store::save(&root, &current);
            }
            Err(error) => {
                let _ = app.emit(
                    "game-status",
                    model::GameStatus {
                        instance_id: id.clone(),
                        phase: "error".into(),
                        progress: 0.0,
                        message: error,
                        exit_code: None,
                    },
                );
            }
        }
        running.lock().await.retain(|v| v != &id);
    });
    Ok(())
}
#[tauri::command]
async fn mods(state: tauri::State<'_, State>, id: String) -> Result<Vec<ModFile>, String> {
    let db = state.db.lock().await;
    if !db.instances.iter().any(|i| i.id == id) {
        return Err("Instance not found".into());
    }
    drop(db);
    let dir = state.root.join("instances").join(id).join("game/mods");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let e = entry.map_err(|e| e.to_string())?;
        let name = e.file_name().to_string_lossy().to_string();
        if name.ends_with(".jar") || name.ends_with(".jar.disabled") {
            files.push(ModFile {
                name,
                enabled: e.path().extension().and_then(|s| s.to_str()) == Some("jar"),
                size: e.metadata().map_err(|e| e.to_string())?.len(),
            });
        }
    }
    Ok(files)
}
#[tauri::command]
async fn add_mod(state: tauri::State<'_, State>, id: String, source: String) -> Result<(), String> {
    let db = state.db.lock().await;
    if !db.instances.iter().any(|i| i.id == id) {
        return Err("Instance not found".into());
    }
    drop(db);
    let src = PathBuf::from(source);
    if src.extension().and_then(|s| s.to_str()) != Some("jar") {
        return Err("Select a .jar file".into());
    }
    let name = src
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("Invalid file name")?;
    if !store::safe_filename(name) {
        return Err("Unsafe file name".into());
    }
    let dir = state.root.join("instances").join(id).join("game/mods");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::copy(&src, dir.join(name)).map_err(|e| e.to_string())?;
    Ok(())
}
#[tauri::command]
async fn toggle_mod(
    state: tauri::State<'_, State>,
    id: String,
    name: String,
) -> Result<(), String> {
    if !store::safe_filename(&name) {
        return Err("Unsafe file name".into());
    }
    let db = state.db.lock().await;
    if !db.instances.iter().any(|i| i.id == id) {
        return Err("Instance not found".into());
    }
    drop(db);
    let dir = state.root.join("instances").join(id).join("game/mods");
    let target = if name.ends_with(".jar.disabled") {
        name.trim_end_matches(".disabled").to_string()
    } else if name.ends_with(".jar") {
        format!("{name}.disabled")
    } else {
        return Err("Invalid mod file".into());
    };
    std::fs::rename(dir.join(name), dir.join(target)).map_err(|e| e.to_string())
}
#[tauri::command]
async fn remove_mod(
    state: tauri::State<'_, State>,
    id: String,
    name: String,
) -> Result<(), String> {
    if !store::safe_filename(&name) || !(name.ends_with(".jar") || name.ends_with(".jar.disabled"))
    {
        return Err("Invalid mod file".into());
    }
    let db = state.db.lock().await;
    if !db.instances.iter().any(|i| i.id == id) {
        return Err("Instance not found".into());
    }
    drop(db);
    std::fs::remove_file(
        state
            .root
            .join("instances")
            .join(id)
            .join("game/mods")
            .join(name),
    )
    .map_err(|e| e.to_string())
}
#[tauri::command]
async fn instance_files(state: tauri::State<'_, State>, id: String) -> Result<Vec<String>, String> {
    let db = state.db.lock().await;
    if !db.instances.iter().any(|i| i.id == id) {
        return Err("Instance not found".into());
    }
    drop(db);
    let base = state.root.join("instances").join(id);
    let mut files = Vec::new();
    for relative in ["launcher-logs/session.log", "game/logs/latest.log"] {
        let path = base.join(relative);
        if path.exists() {
            files.push(path.to_string_lossy().to_string())
        }
    }
    let crashes = base.join("game/crash-reports");
    if let Ok(entries) = std::fs::read_dir(crashes) {
        let mut reports = entries
            .flatten()
            .filter(|e| e.path().is_file())
            .map(|e| e.path().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        reports.sort();
        reports.reverse();
        files.extend(reports)
    }
    Ok(files)
}
#[tauri::command]
async fn read_log(
    state: tauri::State<'_, State>,
    id: String,
    path: String,
) -> Result<String, String> {
    let db = state.db.lock().await;
    if !db.instances.iter().any(|i| i.id == id) {
        return Err("Instance not found".into());
    }
    drop(db);
    let base = state
        .root
        .join("instances")
        .join(id)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let file = PathBuf::from(path)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let allowed = [
        base.join("launcher-logs"),
        base.join("game/logs"),
        base.join("game/crash-reports"),
    ];
    if !allowed
        .iter()
        .any(|dir| dir.exists() && dir.canonicalize().ok().is_some_and(|d| file.starts_with(d)))
    {
        return Err("File outside log directories".into());
    }
    let data = std::fs::read(file).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&data[data.len().saturating_sub(500_000)..]).to_string();
    let secret =
        regex::Regex::new(r"(?i)(--accessToken\s+|Bearer\s+)[^\s]+").map_err(|e| e.to_string())?;
    Ok(secret.replace_all(&text, "$1[redacted]").to_string())
}
#[tauri::command]
async fn instance_directory(state: tauri::State<'_, State>, id: String) -> Result<String, String> {
    let db = state.db.lock().await;
    if !db.instances.iter().any(|i| i.id == id) {
        return Err("Instance not found".into());
    }
    Ok(state
        .root
        .join("instances")
        .join(id)
        .join("game")
        .to_string_lossy()
        .to_string())
}
#[tauri::command]
async fn catalog_key_status() -> bool {
    catalog::key_is_set()
}
#[tauri::command]
async fn catalog_set_key(key: String) -> Result<(), String> {
    catalog::set_key(&key)
}
#[tauri::command]
async fn catalog_search(
    state: tauri::State<'_, State>,
    provider: catalog::Provider,
    kind: catalog::Kind,
    query: String,
    instance_id: Option<String>,
) -> Result<Vec<catalog::Project>, String> {
    let db = state.db.lock().await;
    let instance = instance_id
        .as_deref()
        .and_then(|id| db.instances.iter().find(|i| i.id == id))
        .cloned();
    drop(db);
    catalog::search(&state.client, provider, kind, &query, instance.as_ref()).await
}
#[tauri::command]
async fn catalog_project_detail(
    state: tauri::State<'_, State>,
    provider: catalog::Provider,
    project: String,
) -> Result<catalog::ProjectDetail, String> {
    catalog::project_detail(&state.client, provider, &project).await
}
#[tauri::command]
async fn catalog_releases(
    state: tauri::State<'_, State>,
    provider: catalog::Provider,
    project: String,
    instance_id: Option<String>,
) -> Result<Vec<catalog::Release>, String> {
    let db = state.db.lock().await;
    let instance = instance_id
        .as_deref()
        .and_then(|id| db.instances.iter().find(|i| i.id == id))
        .cloned();
    drop(db);
    catalog::releases(&state.client, provider, &project, instance.as_ref()).await
}
#[tauri::command]
async fn catalog_install_mod(
    state: tauri::State<'_, State>,
    provider: catalog::Provider,
    project: String,
    release: String,
    instance_id: String,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let instance = db
        .instances
        .iter()
        .find(|i| i.id == instance_id)
        .cloned()
        .ok_or("Instance not found")?;
    drop(db);
    catalog::install_mod(
        &state.client,
        &state.root,
        &instance,
        provider,
        &project,
        &release,
    )
    .await
}
#[tauri::command]
async fn catalog_install_pack(
    state: tauri::State<'_, State>,
    provider: catalog::Provider,
    project: String,
    release: String,
) -> Result<Instance, String> {
    let settings = state.db.lock().await.settings.clone();
    let instance = catalog::install_pack(
        &state.client,
        &state.root,
        &settings,
        provider,
        &project,
        &release,
    )
    .await?;
    let mut db = state.db.lock().await;
    db.instances.push(instance.clone());
    store::save(&state.root, &db)?;
    Ok(instance)
}
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let root = store::root(app.handle()).map_err(std::io::Error::other)?;
            let db = store::load(&root).map_err(std::io::Error::other)?;
            app.manage(State {
                db: Arc::new(Mutex::new(db)),
                client: client().map_err(std::io::Error::other)?,
                root,
                running: Arc::new(Mutex::new(Vec::new())),
            });
            app.manage(updates::Pending::new());
            updates::register(app.handle()).map_err(std::io::Error::other)?;
            #[cfg(target_os = "linux")]
            hyprland::float_main_window(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            catalog_key_status,
            catalog_set_key,
            catalog_search,
            catalog_releases,
            catalog_project_detail,
            catalog_install_mod,
            catalog_install_pack,
            versions,
            fabric_versions,
            neoforge_versions,
            create_instance,
            save_instance,
            delete_instance,
            save_settings,
            start_login,
            finish_login,
            add_offline_account,
            select_account,
            logout,
            play,
            mods,
            add_mod,
            toggle_mod,
            remove_mod,
            instance_files,
            read_log,
            instance_directory,
            updates::updater_ready,
            updates::check_update,
            updates::install_update,
            updates::restart_launcher
        ]);
    if let Err(e) = builder.run(tauri::generate_context!()) {
        eprintln!("Launcher failed: {e}");
    }
}
