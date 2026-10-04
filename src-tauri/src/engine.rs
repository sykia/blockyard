use crate::{
    download::{self, Job},
    metadata::{self, AssetObjects, Library, Manifest, Version},
    model::{Account, AccountKind, GameStatus, Instance, Loader},
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
    sync::OnceLock,
    time::{Duration, Instant},
};
use tauri::Emitter;
use tokio::{
    fs,
    io::{AsyncBufReadExt, BufReader},
};

pub async fn manifest(client: &reqwest::Client, root: &Path) -> Result<Manifest, String> {
    static CACHE: OnceLock<tokio::sync::Mutex<HashMap<PathBuf, (Instant, Duration, Manifest)>>> =
        OnceLock::new();
    let remembered = CACHE.get_or_init(|| tokio::sync::Mutex::new(HashMap::new()));
    {
        let cache = remembered.lock().await;
        if let Some((checked, ttl, value)) = cache.get(root) {
            if checked.elapsed() < *ttl {
                return Ok(value.clone());
            }
        }
    }
    let cache = root.join("cache/version_manifest_v2.json");
    let fresh = async {
        let bytes = client
            .get(metadata::MANIFEST)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .bytes()
            .await
            .map_err(|e| e.to_string())?;
        let parsed: Manifest = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if let Some(parent) = cache.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| e.to_string())?;
        }
        let temp = cache.with_extension("tmp");
        fs::write(&temp, &bytes).await.map_err(|e| e.to_string())?;
        fs::rename(temp, &cache).await.map_err(|e| e.to_string())?;
        Ok::<_, String>(parsed)
    }
    .await;
    let (result, ttl) = match fresh {
        Ok(m) => (m, Duration::from_secs(15 * 60)),
        Err(error) => {
            let bytes = fs::read(cache).await.map_err(|_| error)?;
            (
                serde_json::from_slice(&bytes).map_err(|e| e.to_string())?,
                Duration::from_secs(30),
            )
        }
    };
    remembered
        .lock()
        .await
        .insert(root.to_path_buf(), (Instant::now(), ttl, result.clone()));
    Ok(result)
}
async fn json_cached<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    job: Job,
) -> Result<T, String> {
    download::fetch(client, &job).await?;
    let bytes = fs::read(&job.path).await.map_err(|e| e.to_string())?;
    match serde_json::from_slice(&bytes) {
        Ok(value) => Ok(value),
        Err(_) => {
            fs::remove_file(&job.path)
                .await
                .map_err(|e| e.to_string())?;
            download::fetch(client, &job).await?;
            let bytes = fs::read(&job.path).await.map_err(|e| e.to_string())?;
            serde_json::from_slice(&bytes).map_err(|e| format!("Invalid metadata: {e}"))
        }
    }
}
async fn version(client: &reqwest::Client, root: &Path, id: &str) -> Result<Version, String> {
    let item = manifest(client, root)
        .await?
        .versions
        .into_iter()
        .find(|v| v.id == id)
        .ok_or_else(|| format!("Unknown Minecraft version: {id}"))?;
    json_cached(
        client,
        Job {
            url: item.url,
            path: root.join("cache/versions").join(format!("{id}.json")),
            sha1: item.sha1,
            size: None,
        },
    )
    .await
}
async fn fabric(
    client: &reqwest::Client,
    root: &Path,
    mc: &str,
    loader: &str,
) -> Result<Version, String> {
    if !loader
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || ".-_+".contains(c))
    {
        return Err("Invalid Fabric version".into());
    }
    let url = format!("https://meta.fabricmc.net/v2/versions/loader/{mc}/{loader}/profile/json");
    json_cached(
        client,
        Job {
            url,
            path: root
                .join("cache/loaders")
                .join(format!("fabric-{mc}-{loader}.json")),
            sha1: None,
            size: None,
        },
    )
    .await
}
fn lib_job(root: &Path, lib: &Library) -> Result<Option<Job>, String> {
    if let Some(artifact) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
        return Ok(Some(Job {
            url: artifact.url.clone(),
            path: root
                .join("libraries")
                .join(download::relative_path(&artifact.path)?),
            sha1: artifact.sha1.clone(),
            size: artifact.size,
        }));
    }
    if lib.downloads.is_some() {
        return Ok(None);
    }
    let path = metadata::maven_path(&lib.name)?;
    let base = lib
        .url
        .as_deref()
        .unwrap_or("https://libraries.minecraft.net/");
    if !base.starts_with("https://") {
        return Err("Insecure library repository".into());
    }
    Ok(Some(Job {
        url: format!("{}/{path}", base.trim_end_matches('/')),
        path: root.join("libraries").join(&path),
        sha1: lib.sha1.clone(),
        size: lib.size,
    }))
}
fn native_job(root: &Path, lib: &Library) -> Result<Option<Job>, String> {
    let Some(pattern) = lib
        .natives
        .as_ref()
        .and_then(|n| n.get(metadata::os_name()))
    else {
        return Ok(None);
    };
    let classifier = pattern.replace(
        "${arch}",
        if cfg!(target_pointer_width = "64") {
            "64"
        } else {
            "32"
        },
    );
    let Some(artifact) = lib
        .downloads
        .as_ref()
        .and_then(|d| d.classifiers.as_ref())
        .and_then(|c| c.get(&classifier))
    else {
        return Err(format!(
            "Native classifier {classifier} missing for {}",
            lib.name
        ));
    };
    Ok(Some(Job {
        url: artifact.url.clone(),
        path: root
            .join("libraries")
            .join(download::relative_path(&artifact.path)?),
        sha1: artifact.sha1.clone(),
        size: artifact.size,
    }))
}
fn extract_natives(archives: &[PathBuf], destination: &Path) -> Result<(), String> {
    std::fs::create_dir_all(destination).map_err(|e| e.to_string())?;
    for archive in archives {
        let file = std::fs::File::open(archive).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
        for i in 0..zip.len() {
            let mut item = zip.by_index(i).map_err(|e| e.to_string())?;
            let name = item.name().replace('\\', "/");
            if name.starts_with("META-INF/") || item.is_dir() {
                continue;
            }
            let relative = download::relative_path(&name)?;
            let ext = relative.extension().and_then(|v| v.to_str()).unwrap_or("");
            if !["dll", "so", "dylib", "jnilib"].contains(&ext) {
                continue;
            }
            if item.size() > 100_000_000 {
                return Err("Native archive entry is too large".into());
            }
            let out = destination.join(relative.file_name().ok_or("Invalid native filename")?);
            let mut writer = std::fs::File::create(out).map_err(|e| e.to_string())?;
            std::io::copy(&mut item, &mut writer).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
fn replace_args(items: Vec<String>, values: &HashMap<&str, String>) -> Vec<String> {
    items
        .into_iter()
        .map(|mut s| {
            for (k, v) in values {
                s = s.replace(&format!("${{{k}}}"), v)
            }
            s
        })
        .collect()
}
fn split_user_args(input: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if matches!(chars.peek(), Some('\\' | '"' | '\'')) {
                current.push(chars.next().unwrap_or('\\'));
            } else {
                current.push(ch)
            }
            continue;
        }
        if let Some(q) = quote {
            if ch == q {
                quote = None
            } else {
                current.push(ch)
            };
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch)
        } else if ch.is_whitespace() {
            if !current.is_empty() {
                args.push(std::mem::take(&mut current))
            }
        } else {
            current.push(ch)
        }
    }
    if quote.is_some() {
        return Err("Unclosed quote in custom JVM arguments".into());
    }
    if !current.is_empty() {
        args.push(current)
    }
    Ok(args)
}
fn emit(
    app: &tauri::AppHandle,
    id: &str,
    phase: &str,
    progress: f64,
    message: String,
    exit_code: Option<i32>,
) {
    let _ = app.emit(
        "game-status",
        GameStatus {
            instance_id: id.into(),
            phase: phase.into(),
            progress,
            message,
            exit_code,
        },
    );
}
fn crash_summary(game_dir: &Path, since: std::time::SystemTime) -> String {
    let reports = game_dir.join("crash-reports");
    let latest = std::fs::read_dir(reports).ok().and_then(|entries| {
        entries
            .flatten()
            .filter(|e| {
                e.path().is_file()
                    && e.metadata()
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .is_some_and(|t| t >= since)
            })
            .max_by_key(|e| e.file_name())
    });
    let source = latest
        .map(|e| e.path())
        .unwrap_or_else(|| game_dir.join("logs/latest.log"));
    let Ok(text) = std::fs::read_to_string(&source) else {
        return "Open logs for details.".into();
    };
    text.lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("Description:")
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        })
        .or_else(|| {
            text.lines()
                .rev()
                .find(|line| line.contains("Exception") || line.contains("ERROR"))
                .map(|s| s.trim().to_owned())
        })
        .unwrap_or_else(|| "Open logs for details.".into())
}
pub async fn run(
    app: tauri::AppHandle,
    client: reqwest::Client,
    root: PathBuf,
    instance: Instance,
    account: Account,
    token: String,
    concurrency: usize,
    default_java: Option<String>,
    minimize_on_launch: bool,
) -> Result<u64, String> {
    let id = &instance.id;
    let game_dir = root.join("instances").join(id).join("game");
    fs::create_dir_all(&game_dir)
        .await
        .map_err(|e| e.to_string())?;
    emit(
        &app,
        id,
        "install",
        0.0,
        "Reading version metadata".into(),
        None,
    );
    let base = version(&client, &root, &instance.version).await?;
    let required = base
        .java_version
        .as_ref()
        .map(|j| j.major_version)
        .unwrap_or(8);
    emit(
        &app,
        id,
        "java",
        0.0,
        format!("Finding Java {required}"),
        None,
    );
    let java = crate::java::resolve(
        &client,
        &root,
        required,
        instance.java_path.as_deref().or(default_java.as_deref()),
    )
    .await?;
    let client_jar = base
        .downloads
        .as_ref()
        .and_then(|d| d.get("client"))
        .ok_or("Client download missing")?;
    let client_path = root
        .join("cache/versions")
        .join(&base.id)
        .join("client.jar");
    let client_job = Job {
        url: client_jar.url.clone(),
        path: client_path.clone(),
        sha1: client_jar.sha1.clone(),
        size: client_jar.size,
    };
    let overlay = match &instance.loader {
        Loader::Vanilla => None,
        Loader::Fabric(v) => Some(fabric(&client, &root, &instance.version, v).await?),
        Loader::Neoforge(v) => {
            emit(&app, id, "loader", 0.0, "Installing NeoForge".into(), None);
            download::fetch(&client, &client_job).await?;
            Some(
                crate::neoforge::install(
                    &client,
                    &root,
                    &instance.version,
                    v,
                    &java,
                    &root
                        .join("cache/versions")
                        .join(format!("{}.json", instance.version)),
                    &client_path,
                )
                .await?,
            )
        }
    };
    if overlay
        .as_ref()
        .and_then(|v| v.inherits_from.as_deref())
        .is_some_and(|v| v != instance.version)
    {
        return Err("Loader profile targets a different Minecraft version".into());
    }
    let effective = overlay.as_ref().unwrap_or(&base);
    let mut libraries = overlay
        .as_ref()
        .map(|v| v.libraries.clone())
        .unwrap_or_default();
    libraries.extend(base.libraries.clone());
    let mut jobs = Vec::new();
    let mut classpath = Vec::<PathBuf>::new();
    let mut natives = Vec::new();
    for lib in &libraries {
        if !metadata::allowed(lib.rules.as_ref(), &HashMap::new()) {
            continue;
        }
        if let Some(job) = lib_job(&root, lib)? {
            classpath.push(job.path.clone());
            jobs.push(job);
        }
        if let Some(job) = native_job(&root, lib)? {
            natives.push(job.path.clone());
            jobs.push(job);
        }
    }
    jobs.push(client_job);
    classpath.push(client_path);
    let logging = base.logging.as_ref().and_then(|l| l.client.as_ref());
    let logging_path = if let Some(config) = logging {
        let path = root
            .join("cache/logging")
            .join(download::relative_path(&config.file.id)?);
        jobs.push(Job {
            url: config.file.url.clone(),
            path: path.clone(),
            sha1: config.file.sha1.clone(),
            size: config.file.size,
        });
        Some(path)
    } else {
        None
    };
    let index = base.asset_index.as_ref().ok_or("Asset index missing")?;
    let asset_index: AssetObjects = json_cached(
        &client,
        Job {
            url: index.url.clone(),
            path: root
                .join("assets/indexes")
                .join(format!("{}.json", index.id)),
            sha1: index.sha1.clone(),
            size: None,
        },
    )
    .await?;
    let asset_jobs = asset_index
        .objects
        .values()
        .map(|obj| {
            if obj.hash.len() < 2 || !obj.hash.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err("Invalid asset hash".to_owned());
            }
            let prefix = &obj.hash[..2];
            Ok(Job {
                url: format!(
                    "https://resources.download.minecraft.net/{prefix}/{}",
                    obj.hash
                ),
                path: root.join("assets/objects").join(prefix).join(&obj.hash),
                sha1: Some(obj.hash.clone()),
                size: Some(obj.size),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    jobs.extend(asset_jobs);
    download::fetch_all(&client, jobs, concurrency, &app, id, "install").await?;
    if index.id == "legacy" {
        for (name, obj) in &asset_index.objects {
            let relative = download::relative_path(name)?;
            let source = root
                .join("assets/objects")
                .join(&obj.hash[..2])
                .join(&obj.hash);
            let destination = root.join("assets/virtual/legacy").join(relative);
            if !destination.exists() {
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent)
                        .await
                        .map_err(|e| e.to_string())?;
                }
                fs::copy(source, destination)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    let native_dir = game_dir.join(".natives");
    extract_natives(&natives, &native_dir)?;
    let sep = if cfg!(windows) { ";" } else { ":" };
    let classpath = classpath
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(sep);
    let mut values = HashMap::new();
    values.insert("auth_player_name", account.name.clone());
    values.insert("version_name", effective.id.clone());
    values.insert("game_directory", game_dir.to_string_lossy().to_string());
    values.insert(
        "assets_root",
        root.join("assets").to_string_lossy().to_string(),
    );
    values.insert(
        "game_assets",
        root.join("assets/virtual/legacy")
            .to_string_lossy()
            .to_string(),
    );
    values.insert("assets_index_name", index.id.clone());
    values.insert("auth_uuid", account.id.clone());
    values.insert("auth_access_token", token.clone());
    values.insert(
        "user_type",
        if account.kind == AccountKind::Offline {
            "legacy"
        } else {
            "msa"
        }
        .into(),
    );
    values.insert(
        "version_type",
        base.kind.clone().unwrap_or("release".into()),
    );
    values.insert("user_properties", "{}".into());
    values.insert(
        "natives_directory",
        native_dir.to_string_lossy().to_string(),
    );
    values.insert("launcher_name", "Blockyard".into());
    values.insert("launcher_version", env!("CARGO_PKG_VERSION").into());
    values.insert("classpath", classpath);
    values.insert("classpath_separator", sep.into());
    values.insert(
        "library_directory",
        root.join("libraries").to_string_lossy().to_string(),
    );
    values.insert("resolution_width", instance.width.to_string());
    values.insert("resolution_height", instance.height.to_string());
    values.insert("auth_xuid", account.xuid.clone());
    values.insert("clientid", "".into());
    let features = HashMap::from([("has_custom_resolution".to_owned(), true)]);
    let mut jvm = Vec::new();
    if let Some(args) = base.arguments.as_ref() {
        jvm.extend(
            metadata::argument_values(&args.default_user_jvm, &features)?
                .into_iter()
                .filter(|v| !v.starts_with("-Xmx") && !v.starts_with("-Xms")),
        );
        jvm.extend(metadata::argument_values(&args.jvm, &features)?);
    }
    if let Some(extra) = overlay.as_ref().and_then(|v| v.arguments.as_ref()) {
        jvm.extend(metadata::argument_values(&extra.jvm, &features)?);
    }
    if !jvm.iter().any(|s| s.starts_with("-Djava.library.path=")) {
        jvm.push("-Djava.library.path=${natives_directory}".into());
    }
    if !jvm
        .iter()
        .any(|s| ["-cp", "-classpath", "--class-path"].contains(&s.as_str()))
    {
        jvm.extend(["-cp".into(), "${classpath}".into()]);
    }
    if let (Some(config), Some(path)) = (logging, logging_path) {
        jvm.push(config.argument.replace("${path}", &path.to_string_lossy()));
    }
    let mut game = if let Some(args) = base.arguments.as_ref() {
        metadata::argument_values(&args.game, &features)?
    } else {
        Vec::new()
    };
    if let Some(extra) = overlay.as_ref().and_then(|v| v.arguments.as_ref()) {
        game.extend(metadata::argument_values(&extra.game, &features)?);
    }
    if game.is_empty() {
        game = effective
            .minecraft_arguments
            .as_deref()
            .or(base.minecraft_arguments.as_deref())
            .ok_or("No game arguments")?
            .split_whitespace()
            .map(str::to_owned)
            .collect();
    }
    jvm = replace_args(jvm, &values);
    game = replace_args(game, &values);
    if jvm.iter().chain(&game).any(|s| s.contains("${")) {
        return Err("Unresolved launch argument in version metadata".into());
    }
    let mut command = tokio::process::Command::new(java);
    command
        .arg(format!("-Xmx{}M", instance.ram_mb))
        .args(split_user_args(&instance.jvm_args)?)
        .args(jvm)
        .arg(&effective.main_class)
        .args(game)
        .current_dir(&game_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    emit(&app, id, "launch", 1.0, "Starting Minecraft".into(), None);
    let launched_at = std::time::SystemTime::now() - std::time::Duration::from_secs(2);
    let mut child = command
        .spawn()
        .map_err(|e| format!("Could not start Minecraft: {e}"))?;
    if minimize_on_launch {
        use tauri::Manager;
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.minimize();
        }
    }
    let log_dir = root.join("instances").join(id).join("launcher-logs");
    fs::create_dir_all(&log_dir)
        .await
        .map_err(|e| e.to_string())?;
    let log_path = log_dir.join("session.log");
    let log = std::sync::Arc::new(tokio::sync::Mutex::new(
        fs::File::create(log_path)
            .await
            .map_err(|e| e.to_string())?,
    ));
    let mut readers = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        readers.push((
            "stdout",
            Box::new(BufReader::new(stdout)) as Box<dyn tokio::io::AsyncBufRead + Unpin + Send>,
        ));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push((
            "stderr",
            Box::new(BufReader::new(stderr)) as Box<dyn tokio::io::AsyncBufRead + Unpin + Send>,
        ));
    }
    let mut tasks = Vec::new();
    for (label, mut reader) in readers {
        let log = log.clone();
        let app = app.clone();
        let id = id.clone();
        let token = token.clone();
        let label = label.to_string();
        tasks.push(tokio::spawn(async move {
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let safe = line.replace(&token, "[redacted]");
                        use tokio::io::AsyncWriteExt;
                        let _ = log
                            .lock()
                            .await
                            .write_all(format!("[{label}] {safe}").as_bytes())
                            .await;
                        let _ =
                            app.emit("game-log", serde_json::json!({"instanceId":id,"line":safe}));
                    }
                }
            }
        }));
    }
    emit(
        &app,
        id,
        "running",
        1.0,
        "Minecraft is running".into(),
        None,
    );
    let started = std::time::Instant::now();
    let status = child.wait().await.map_err(|e| e.to_string())?;
    for task in tasks {
        let _ = task.await;
    }
    emit(
        &app,
        id,
        if status.success() {
            "stopped"
        } else {
            "crashed"
        },
        1.0,
        if status.success() {
            "Game closed".into()
        } else {
            format!(
                "Minecraft exited with code {}. {}",
                status.code().unwrap_or(-1),
                crash_summary(&game_dir, launched_at)
                    .chars()
                    .take(300)
                    .collect::<String>()
                    .replace(&token, "[redacted]")
            )
        },
        status.code(),
    );
    Ok(started.elapsed().as_secs())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn user_args() {
        assert_eq!(
            split_user_args("-Da='hello world' -Xms1G").unwrap(),
            vec!["-Da=hello world", "-Xms1G"]
        );
        assert!(split_user_args("'open").is_err());
    }
}
