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
    !name.is_empty()
        && !name.contains(['/', '\\', '\0'])
        && name != "."
        && name != ".."
        && !name.starts_with('.')
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Instance, Loader};
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
