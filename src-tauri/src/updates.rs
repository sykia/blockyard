use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use tauri::{utils::config::BundleType, AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

const FEED: &str = "https://github.com/sykia/blockyard/releases/latest/download/latest.json";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InstallKind {
    #[cfg(target_os = "linux")]
    Arch,
    #[cfg(target_os = "linux")]
    Deb,
    #[cfg(target_os = "linux")]
    Rpm,
    #[cfg(target_os = "linux")]
    AppImage,
    #[cfg(windows)]
    Windows,
}
struct Checked {
    update: Update,
    kind: InstallKind,
}
pub struct Pending(Mutex<Option<Checked>>);
impl Pending {
    pub fn new() -> Self {
        Self(Mutex::new(None))
    }
}

#[cfg(target_os = "linux")]
fn package_owner(executable: &Path) -> bool {
    executable == Path::new("/usr/bin/blockyard")
        && Command::new("/usr/bin/pacman")
            .args(["-Qqo", "/usr/bin/blockyard"])
            .output()
            .ok()
            .is_some_and(|out| {
                out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "blockyard"
            })
}
fn install_kind() -> Option<InstallKind> {
    #[cfg(target_os = "linux")]
    {
        match tauri::utils::platform::bundle_type() {
            Some(BundleType::AppImage) => Some(InstallKind::AppImage),
            Some(BundleType::Deb) => Some(InstallKind::Deb),
            Some(BundleType::Rpm) => Some(InstallKind::Rpm),
            _ => std::env::current_exe()
                .ok()
                .filter(|p| package_owner(p))
                .map(|_| InstallKind::Arch),
        }
    }
    #[cfg(windows)]
    {
        (tauri::utils::platform::bundle_type() == Some(BundleType::Nsis))
            .then_some(InstallKind::Windows)
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        None
    }
}
pub fn register(app: &AppHandle) -> Result<(), String> {
    app.plugin(tauri_plugin_updater::Builder::new().build())
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn updater_ready() -> bool {
    install_kind().is_some()
}
#[tauri::command]
pub async fn check_update(
    app: AppHandle,
    pending: tauri::State<'_, Pending>,
) -> Result<Option<String>, String> {
    let kind = install_kind().ok_or("Updates are available in installed builds only")?;
    let mut builder = app
        .updater_builder()
        .endpoints(vec![url::Url::parse(FEED).map_err(|e| e.to_string())?])
        .map_err(|e| e.to_string())?;
    #[cfg(target_os = "linux")]
    if kind == InstallKind::Arch {
        builder = builder.target("linux-x86_64-arch");
    }
    let found = builder
        .build()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    let version = found.as_ref().map(|u| u.version.clone());
    *pending.0.lock().map_err(|e| e.to_string())? = found.map(|update| Checked { update, kind });
    Ok(version)
}
fn emit(app: &AppHandle, phase: &str, received: u64, total: Option<u64>, message: &str) {
    let _ = app.emit(
        "update-progress",
        serde_json::json!({"phase":phase,"received":received,"total":total,"message":message}),
    );
}
#[cfg(target_os = "linux")]
fn exists(command: &str) -> bool {
    Path::new("/usr/bin").join(command).is_file()
}
#[cfg(target_os = "linux")]
fn terminal() -> Option<(&'static str, &'static [&'static str])> {
    [
        ("kitty", &["--hold", "-e"][..]),
        ("konsole", &["--hold", "-e"]),
        ("xterm", &["-hold", "-e"]),
        ("alacritty", &["--hold", "-e"]),
        ("gnome-terminal", &["--wait", "--"]),
    ]
    .into_iter()
    .find(|(name, _)| exists(name))
}
#[cfg(target_os = "linux")]
fn install_command(
    kind: InstallKind,
) -> Result<(&'static str, &'static [&'static str], &'static str), String> {
    match kind {
        InstallKind::Arch => Ok(("pacman", &["-U", "--"], ".pkg.tar.zst")),
        InstallKind::Deb => Ok(("apt-get", &["install", "-y", "--"], ".deb")),
        InstallKind::Rpm if exists("dnf") => Ok(("dnf", &["install", "-y", "--"], ".rpm")),
        InstallKind::Rpm => Ok(("rpm", &["-Uvh", "--"], ".rpm")),
        _ => Err("This installation does not use a system package manager".into()),
    }
}
#[cfg(target_os = "linux")]
fn save_verified_package(
    app: &AppHandle,
    version: &str,
    extension: &str,
    bytes: &[u8],
) -> Result<PathBuf, String> {
    use std::io::Write;
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("updates")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join(format!("blockyard-{version}{extension}"));
    let mut handle = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&file)
        .map_err(|e| e.to_string())?;
    handle.write_all(bytes).map_err(|e| e.to_string())?;
    handle.sync_all().map_err(|e| e.to_string())?;
    Ok(file)
}
#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    pending: tauri::State<'_, Pending>,
) -> Result<(), String> {
    let checked = pending
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .ok_or("Check for updates first")?;
    let downloaded = Arc::new(AtomicU64::new(0));
    let counter = downloaded.clone();
    let emitter = app.clone();
    let bytes = checked
        .update
        .download(
            move |chunk, total| {
                let received = counter.fetch_add(chunk as u64, Ordering::Relaxed) + chunk as u64;
                emit(
                    &emitter,
                    "download",
                    received,
                    total,
                    "Downloading signed update…",
                );
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;
    emit(
        &app,
        "verified",
        bytes.len() as u64,
        Some(bytes.len() as u64),
        "Update signature verified.",
    );
    match checked.kind {
        #[cfg(windows)]
        InstallKind::Windows => {
            emit(&app, "install", 0, None, "Opening Windows installer…");
            checked.update.install(bytes).map_err(|e| e.to_string())?;
            Ok(())
        }
        #[cfg(target_os = "linux")]
        InstallKind::AppImage => {
            emit(&app, "install", 0, None, "Installing update…");
            checked.update.install(bytes).map_err(|e| e.to_string())?;
            emit(
                &app,
                "installed",
                0,
                None,
                "Update installed. Restart Blockyard to use it.",
            );
            Ok(())
        }
        #[cfg(target_os = "linux")]
        kind => {
            let (terminal, terminal_args) = terminal().ok_or("No supported terminal found. Install kitty, Konsole, xterm, Alacritty or GNOME Terminal.")?;
            if !exists("sudo") {
                return Err("sudo is required to update a system package".into());
            }
            let (manager, manager_args, extension) = install_command(kind)?;
            if !exists(manager) {
                return Err(format!("{manager} is required to update this package"));
            }
            let file = save_verified_package(&app, &checked.update.version, extension, &bytes)?;
            let mut command = Command::new(Path::new("/usr/bin").join(terminal));
            command
                .args(terminal_args)
                .arg("/usr/bin/sudo")
                .arg(Path::new("/usr/bin").join(manager))
                .args(manager_args)
                .arg(&file);
            match command.spawn() {
                Ok(mut child) => {
                    let folder = file.parent().map(Path::to_path_buf);
                    std::thread::spawn(move || {
                        let _ = child.wait();
                        if let Some(folder) = folder {
                            let _ = std::fs::remove_dir_all(folder);
                        }
                    });
                    emit(&app, "terminal", 0, None, "Installer opened in a terminal. Enter your sudo password there, then restart Blockyard.");
                    Ok(())
                }
                Err(error) => {
                    let _ =
                        std::fs::remove_dir_all(file.parent().unwrap_or(Path::new("/nonexistent")));
                    Err(format!("Could not open terminal: {error}"))
                }
            }
        }
    }
}
#[tauri::command]
pub fn restart_launcher(app: AppHandle) -> Result<(), String> {
    std::thread::spawn(move || app.restart());
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "linux")]
    #[test]
    fn package_owner_rejects_other_paths() {
        assert!(!package_owner(Path::new("/tmp/blockyard")));
    }
}
