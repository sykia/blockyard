use crate::{
    download::{self, Job},
    metadata::Version,
};
use std::path::Path;
const BASE: &str = "https://maven.neoforged.net/releases/net/neoforged/neoforge";
pub async fn versions(client: &reqwest::Client, minecraft: &str) -> Result<Vec<String>, String> {
    let Some(prefix) = version_prefix(minecraft) else {
        return Ok(Vec::new());
    };
    let xml = client
        .get(format!("{BASE}/maven-metadata.xml"))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    let re = regex::Regex::new(r"<version>([^<]+)</version>").map_err(|e| e.to_string())?;
    let mut list = re
        .captures_iter(&xml)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_owned()))
        .filter(|v| v.starts_with(&prefix))
        .collect::<Vec<_>>();
    list.reverse();
    Ok(list)
}
fn version_prefix(minecraft: &str) -> Option<String> {
    let parts: Vec<&str> = minecraft.split('.').collect();
    if parts.len() < 2 || parts.iter().any(|p| p.parse::<u32>().is_err()) {
        return None;
    }
    let version = if parts[0] == "1" {
        format!("{}.{}", parts[1], parts.get(2).unwrap_or(&"0"))
    } else if parts.len() == 2 {
        format!("{}.{}.0", parts[0], parts[1])
    } else {
        minecraft.to_owned()
    };
    Some(format!("{version}."))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn current_version_prefixes() {
        assert_eq!(version_prefix("1.21.1").as_deref(), Some("21.1."));
        assert_eq!(version_prefix("26.3").as_deref(), Some("26.3.0."));
        assert_eq!(version_prefix("26.1.2").as_deref(), Some("26.1.2."));
    }
    #[tokio::test]
    async fn installed_profile_needs_no_network_lookup() {
        let root = std::env::temp_dir().join(format!("blockyard-neo-{}", uuid::Uuid::new_v4()));
        let directory = root.join("versions/neoforge-21.1.200");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("neoforge-21.1.200.json"),
            br#"{"id":"neoforge-21.1.200","mainClass":"example.Main","libraries":[],"inheritsFrom":"1.21.1"}"#,
        )
        .unwrap();
        let profile = install(
            &reqwest::Client::new(),
            &root,
            "1.21.1",
            "21.1.200",
            "java",
            Path::new("missing.json"),
            Path::new("missing.jar"),
        )
        .await
        .unwrap();
        assert_eq!(profile.id, "neoforge-21.1.200");
        std::fs::remove_dir_all(root).unwrap();
    }
}
pub async fn install(
    client: &reqwest::Client,
    root: &Path,
    minecraft: &str,
    loader: &str,
    java: &str,
    base_json: &Path,
    base_jar: &Path,
) -> Result<Version, String> {
    let version_id = format!("neoforge-{loader}");
    let profile = root
        .join("versions")
        .join(&version_id)
        .join(format!("{version_id}.json"));
    if profile.exists() {
        let bytes = tokio::fs::read(&profile)
            .await
            .map_err(|e| format!("Cannot read NeoForge profile: {e}"))?;
        if let Ok(installed) = serde_json::from_slice::<Version>(&bytes) {
            if installed.id == version_id && installed.inherits_from.as_deref() == Some(minecraft) {
                return Ok(installed);
            }
        }
    }
    if !versions(client, minecraft)
        .await?
        .contains(&loader.to_owned())
    {
        return Err("NeoForge version does not match this Minecraft version".into());
    }
    {
        let profiles = root.join("launcher_profiles.json");
        if !profiles.exists() {
            std::fs::write(&profiles, br#"{"profiles":{},"settings":{},"version":3}"#)
                .map_err(|e| e.to_string())?;
        }
        let vanilla = root.join("versions").join(minecraft);
        std::fs::create_dir_all(&vanilla).map_err(|e| e.to_string())?;
        std::fs::copy(base_json, vanilla.join(format!("{minecraft}.json")))
            .map_err(|e| e.to_string())?;
        std::fs::copy(base_jar, vanilla.join(format!("{minecraft}.jar")))
            .map_err(|e| e.to_string())?;
        let stem = format!("neoforge-{loader}-installer.jar");
        let url = format!("{BASE}/{loader}/{stem}");
        let checksum = client
            .get(format!("{url}.sha1"))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .text()
            .await
            .map_err(|e| e.to_string())?;
        let checksum = checksum
            .split_whitespace()
            .next()
            .ok_or("NeoForge installer checksum missing")?
            .to_owned();
        if checksum.len() != 40 || !checksum.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err("Invalid NeoForge installer checksum".into());
        }
        let jar = root.join("cache/installers").join(stem);
        download::fetch(
            client,
            &Job {
                url,
                path: jar.clone(),
                sha1: Some(checksum),
                size: None,
            },
        )
        .await?;
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(900),
            tokio::process::Command::new(java)
                .arg("-jar")
                .arg(&jar)
                .arg("--install-client")
                .arg(root)
                .current_dir(root)
                .output(),
        )
        .await
        .map_err(|_| "NeoForge installer timed out")?
        .map_err(|e| e.to_string())?;
        if !output.status.success() {
            let text = format!(
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return Err(format!(
                "NeoForge installer failed ({}): {}",
                output.status,
                text.chars()
                    .rev()
                    .take(2000)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect::<String>()
            ));
        }
    }
    let bytes = tokio::fs::read(profile)
        .await
        .map_err(|e| format!("NeoForge profile not installed: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("Invalid NeoForge profile: {e}"))
}
