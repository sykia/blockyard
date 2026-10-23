use crate::model::Database;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}
/// All mutable Minecraft data belongs to this instance, including identical versions.
pub fn prepare_game_directory(root: &Path, id: &str) -> Result<PathBuf, String> {
    if !safe_filename(id) {
        return Err("Invalid instance ID".into());
    }
    let game = root.join("instances").join(id).join("game");
    for folder in [
        "mods",
        "config",
        "saves",
        "resourcepacks",
        "shaderpacks",
        "screenshots",
        "logs",
        "crash-reports",
    ] {
        fs::create_dir_all(game.join(folder)).map_err(|e| e.to_string())?;
    }
    Ok(game)
}
pub fn load(root: &Path) -> Result<Database, String> {
    let path = root.join("state.json");
    #[cfg(windows)]
    let path = if !path.exists() && root.join("state.json.bak").exists() {
        root.join("state.json.bak")
    } else {
        path
    };
    if !path.exists() {
        return Ok(Database::default());
    }
    let data = fs::read(&path).map_err(|e| e.to_string())?;
    let value: serde_json::Value = match serde_json::from_slice(&data) {
        Ok(value) => value,
        Err(e) => {
            let backup = root.join(format!(
                "state.corrupt-{}.json",
                chrono::Utc::now().timestamp()
            ));
            fs::copy(&path, &backup).map_err(|e| e.to_string())?;
            let mut db = Database::default();
            db.warnings.push(format!(
                "State file was corrupt ({e}). A copy was saved at {}.",
                backup.display()
            ));
            return Ok(db);
        }
    };
    let mut db = Database::default();
    if let Some(instances) = value.get("instances").and_then(|v| v.as_array()) {
        for item in instances {
            match serde_json::from_value(item.clone()) {
                Ok(instance) => db.instances.push(instance),
                Err(e) => db.warnings.push(format!("Skipped a damaged instance: {e}")),
            }
        }
    }
    if let Some(accounts) = value.get("accounts").and_then(|v| v.as_array()) {
        for item in accounts {
            match serde_json::from_value(item.clone()) {
                Ok(account) => db.accounts.push(account),
                Err(e) => db
                    .warnings
                    .push(format!("Skipped a damaged account record: {e}")),
            }
        }
    }
    if let Some(settings) = value.get("settings") {
        match serde_json::from_value(settings.clone()) {
            Ok(s) => db.settings = s,
            Err(e) => db.warnings.push(format!("Settings were reset: {e}")),
        }
    }
    db.active_account = value
        .get("activeAccount")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    Ok(db)
}
pub fn save(root: &Path, db: &Database) -> Result<(), String> {
    let path = root.join("state.json");
    let tmp = root.join("state.json.tmp");
    fs::write(
        &tmp,
        serde_json::to_vec_pretty(db).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    #[cfg(windows)]
    if path.exists() {
        fs::copy(&path, root.join("state.json.bak")).map_err(|e| e.to_string())?;
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    fs::rename(tmp, path).map_err(|e| e.to_string())
}
pub fn safe_filename(name: &str) -> bool {
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let stem = name.split('.').next().unwrap_or(name);
    !name.is_empty()
        && !name.contains(['/', '\\', '\0'])
        && name != "."
        && name != ".."
        && !name.starts_with('.')
        && !reserved.iter().any(|r| r.eq_ignore_ascii_case(stem))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Instance, Loader};
    #[test]
    fn game_data_is_private_and_preserved() {
        let root =
            std::env::temp_dir().join(format!("blockyard-isolation-{}", uuid::Uuid::new_v4()));
        let first = prepare_game_directory(&root, "first").unwrap();
        let second = prepare_game_directory(&root, "second").unwrap();
        for file in [
            "options.txt",
            "mods/example.jar",
            "config/example.toml",
            "saves/world/level.dat",
        ] {
            let path = first.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, b"first instance").unwrap();
            assert!(!second.join(file).exists());
        }
        prepare_game_directory(&root, "first").unwrap();
        assert_eq!(
            fs::read(first.join("options.txt")).unwrap(),
            b"first instance"
        );
        assert!(second.join("mods").is_dir());
        assert!(second.join("config").is_dir());
        assert!(prepare_game_directory(&root, "../outside").is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn paths() {
        assert!(!safe_filename("../x"));
        assert!(!safe_filename("x/y"));
        assert!(safe_filename("mod.jar"));
    }
    #[test]
    fn damaged_instance_does_not_hide_valid_one() {
        let dir = std::env::temp_dir().join(format!("blockyard-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let good = Instance {
            id: "good".into(),
            name: "Playable".into(),
            version: "1.21.1".into(),
            loader: Loader::Vanilla,
            java_path: None,
            ram_mb: 2048,
            jvm_args: String::new(),
            width: 800,
            height: 600,
            icon: "cube".into(),
            last_played: None,
            playtime_seconds: 0,
        };
        let state = serde_json::json!({"instances":[{"id":"broken"},good],"settings":crate::model::Settings::default()});
        fs::write(dir.join("state.json"), serde_json::to_vec(&state).unwrap()).unwrap();
        let loaded = load(&dir).unwrap();
        assert_eq!(loaded.instances.len(), 1);
        assert_eq!(loaded.instances[0].name, "Playable");
        assert_eq!(loaded.warnings.len(), 1);
        fs::remove_dir_all(dir).unwrap();
    }
}
