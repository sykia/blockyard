use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::{Update, UpdaterExt};

pub struct Pending(pub Mutex<Option<Update>>);
const KEY: Option<&str> = option_env!("BLOCKYARD_UPDATE_PUBKEY");
const ENDPOINT: Option<&str> = option_env!("BLOCKYARD_UPDATE_ENDPOINT");
pub fn configured() -> bool {
    KEY.is_some_and(|s| !s.is_empty()) && ENDPOINT.is_some_and(|s| s.starts_with("https://"))
}
pub fn register(app: &AppHandle) -> Result<(), String> {
    if let Some(key) = KEY.filter(|_| configured()) {
        app.plugin(tauri_plugin_updater::Builder::new().pubkey(key).build())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command]
pub fn updater_ready() -> bool {
    configured()
}
#[tauri::command]
pub async fn check_update(
    app: AppHandle,
    pending: tauri::State<'_, Pending>,
) -> Result<Option<String>, String> {
    let (Some(key), Some(endpoint)) = (KEY, ENDPOINT) else {
        return Err("Updater is not configured for this build".into());
    };
    if !configured() {
        return Err("Updater endpoint must use HTTPS".into());
    }
    let url = url::Url::parse(endpoint).map_err(|e| e.to_string())?;
    let found = app
        .updater_builder()
        .pubkey(key)
        .endpoints(vec![url])
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    let version = found.as_ref().map(|u| u.version.clone());
    *pending.0.lock().map_err(|e| e.to_string())? = found;
    Ok(version)
}
#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    pending: tauri::State<'_, Pending>,
) -> Result<(), String> {
    let update = pending
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .ok_or("No checked update is pending")?;
    let emitter = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                let _ = emitter.emit(
                    "update-progress",
                    serde_json::json!({"chunk":chunk,"total":total}),
                );
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
