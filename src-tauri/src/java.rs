use regex::Regex;
use std::{
    collections::HashMap,
    path::PathBuf,
    process::Command,
    sync::{Mutex, OnceLock},
};

static LAST_JAVA: OnceLock<Mutex<HashMap<u32, String>>> = OnceLock::new();

fn remembered(required: u32) -> Option<String> {
    LAST_JAVA
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .get(&required)
        .cloned()
}

fn remember(required: u32, path: &str) {
    LAST_JAVA
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .insert(required, path.to_owned());
}

pub fn major(path: &str) -> Result<u32, String> {
    let output = Command::new(path)
        .arg("-version")
        .output()
        .map_err(|e| format!("Cannot run Java at {path}: {e}"))?;
    let text = String::from_utf8_lossy(&output.stderr);
    let re = Regex::new(r#"version "(?:1\.)?(\d+)"#).map_err(|e| e.to_string())?;
    re.captures(&text)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse().ok())
        .ok_or_else(|| format!("Cannot determine Java version: {text}"))
}
pub fn find(required: u32, override_path: Option<&str>) -> Result<String, String> {
    if let Some(path) = override_path {
        let actual = major(path)?;
        if actual == required {
            return Ok(path.into());
        }
        return Err(format!(
            "Java {required} required, selected Java is {actual}"
        ));
    }
    let mut candidates = Vec::<PathBuf>::new();
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        candidates.push(PathBuf::from(home).join("bin").join(if cfg!(windows) {
            "java.exe"
        } else {
            "java"
        }));
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(entries) = std::fs::read_dir("/usr/lib/jvm") {
            for e in entries.flatten() {
                candidates.push(e.path().join("bin/java"));
            }
        }
    }
    candidates.push(PathBuf::from("java"));
    for candidate in candidates {
        let path = candidate.to_string_lossy().to_string();
        if major(&path).ok() == Some(required) {
            remember(required, &path);
            return Ok(path);
        }
    }
    Err(format!(
        "Java {required} not found. Install a matching JDK/JRE or select it in instance settings."
    ))
}

pub async fn resolve(
    client: &reqwest::Client,
    root: &std::path::Path,
    required: u32,
    override_path: Option<&str>,
) -> Result<String, String> {
    if override_path.is_none() {
        if let Some(path) = remembered(required) {
            if major(&path).ok() == Some(required) {
                return Ok(path);
            }
        }
        let executable = if cfg!(windows) { "java.exe" } else { "java" };
        let managed = root.join("runtimes").join(required.to_string());
        let managed = if cfg!(target_os = "macos") {
            managed.join("Contents/Home/bin/java")
        } else {
            managed.join("bin").join(executable)
        };
        if managed.exists() && major(&managed.to_string_lossy()).ok() == Some(required) {
            let path = managed.to_string_lossy().to_string();
            remember(required, &path);
            return Ok(path);
        }
    }
    match find(required, override_path) {
        Ok(path) => return Ok(path),
        Err(error) if override_path.is_some() => return Err(error),
        Err(_) => {}
    }
    let executable = if cfg!(windows) { "java.exe" } else { "java" };
    let home = root.join("runtimes").join(required.to_string());
    let os = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    };
    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "x86_64") {
        "x64"
    } else {
        return Err("Managed Java unavailable on this architecture".into());
    };
    let url=format!("https://api.adoptium.net/v3/assets/latest/{required}/hotspot?architecture={arch}&image_type=jre&os={os}&vendor=eclipse");
    let releases: Vec<serde_json::Value> = client
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let package = releases
        .first()
        .and_then(|r| r.pointer("/binary/package"))
        .ok_or(format!(
            "No Temurin Java {required} runtime for this platform"
        ))?;
    let link = package
        .get("link")
        .and_then(|v| v.as_str())
        .ok_or("Java download URL missing")?;
    let checksum = package
        .get("checksum")
        .and_then(|v| v.as_str())
        .ok_or("Java checksum missing")?;
    if !link.starts_with("https://")
        || checksum.len() != 64
        || !checksum.chars().all(|c| c.is_ascii_hexdigit())
    {
        return Err("Invalid Java release metadata".into());
    }
    let response = client
        .get(link)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    use sha2::{Digest, Sha256};
    if !hex::encode(Sha256::digest(&bytes)).eq_ignore_ascii_case(checksum) {
        return Err("Managed Java SHA-256 mismatch".into());
    }
    let staging = root
        .join("runtimes")
        .join(format!("{}.tmp-{}", required, uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let result = (|| -> Result<(), String> {
        if cfg!(windows) {
            let cursor = std::io::Cursor::new(bytes);
            let mut archive = zip::ZipArchive::new(cursor).map_err(|e| e.to_string())?;
            for index in 0..archive.len() {
                let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;
                let name = entry.name().to_owned();
                let is_dir = entry.is_dir();
                unpack_entry(&name, is_dir, &staging, &mut entry)?;
            }
        } else {
            let cursor = std::io::Cursor::new(bytes);
            let decoder = flate2::read::GzDecoder::new(cursor);
            let mut archive = tar::Archive::new(decoder);
            for item in archive.entries().map_err(|e| e.to_string())? {
                let mut entry = item.map_err(|e| e.to_string())?;
                let kind = entry.header().entry_type();
                if !kind.is_file() && !kind.is_dir() {
                    continue;
                }
                let path = entry
                    .path()
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .to_string();
                unpack_entry(&path, kind.is_dir(), &staging, &mut entry)?;
            }
        }
        Ok(())
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    let java = if cfg!(target_os = "macos") {
        staging.join("Contents/Home/bin/java")
    } else {
        staging.join("bin").join(executable)
    };
    if major(&java.to_string_lossy()).ok() != Some(required) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err("Downloaded Java did not pass version check".into());
    }
    if home.exists() {
        std::fs::remove_dir_all(&home).map_err(|e| e.to_string())?
    }
    std::fs::rename(&staging, &home).map_err(|e| e.to_string())?;
    let final_java = if cfg!(target_os = "macos") {
        home.join("Contents/Home/bin/java")
    } else {
        home.join("bin").join(executable)
    };
    let path = final_java.to_string_lossy().to_string();
    remember(required, &path);
    Ok(path)
}
fn unpack_entry<R: std::io::Read>(
    name: &str,
    is_dir: bool,
    staging: &std::path::Path,
    reader: &mut R,
) -> Result<(), String> {
    let path = crate::download::relative_path(name.trim_end_matches('/'))?;
    let mut parts = path.components();
    parts.next();
    let relative = parts.as_path();
    if relative.as_os_str().is_empty() {
        return Ok(());
    }
    let dest = staging.join(relative);
    if is_dir {
        std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    } else {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut file = std::fs::File::create(&dest).map_err(|e| e.to_string())?;
        let copied = std::io::copy(&mut std::io::Read::take(reader, 500_000_001), &mut file)
            .map_err(|e| e.to_string())?;
        if copied > 500_000_000 {
            return Err("Managed Java archive entry is too large".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if dest.file_name().and_then(|n| n.to_str()) == Some("java") {
                std::fs::set_permissions(dest, std::fs::Permissions::from_mode(0o755))
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}
