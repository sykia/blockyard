use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

pub const MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
#[derive(Clone, Deserialize)]
pub struct Manifest {
    pub versions: Vec<ManifestVersion>,
}
#[derive(Clone, Deserialize)]
pub struct ManifestVersion {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub sha1: Option<String>,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
}
#[derive(Clone, Deserialize)]
pub struct Download {
    pub url: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}
#[derive(Clone, Deserialize)]
pub struct AssetIndex {
    pub id: String,
    pub url: String,
    pub sha1: Option<String>,
}
#[derive(Clone, Deserialize)]
pub struct JavaVersion {
    #[serde(rename = "majorVersion")]
    pub major_version: u32,
}
#[derive(Clone, Deserialize)]
pub struct Version {
    pub id: String,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    pub downloads: Option<HashMap<String, Download>>,
    #[serde(rename = "assetIndex")]
    pub asset_index: Option<AssetIndex>,
    pub libraries: Vec<Library>,
    pub arguments: Option<Arguments>,
    #[serde(rename = "minecraftArguments")]
    pub minecraft_arguments: Option<String>,
    #[serde(rename = "javaVersion")]
    pub java_version: Option<JavaVersion>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    #[serde(rename = "inheritsFrom")]
    pub inherits_from: Option<String>,
    pub logging: Option<Logging>,
}
#[derive(Clone, Deserialize)]
pub struct Logging {
    pub client: Option<ClientLogging>,
}
#[derive(Clone, Deserialize)]
pub struct ClientLogging {
    pub argument: String,
    pub file: LoggingFile,
}
#[derive(Clone, Deserialize)]
pub struct LoggingFile {
    pub id: String,
    pub url: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}
#[derive(Clone, Deserialize, Default)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Value>,
    #[serde(default)]
    pub jvm: Vec<Value>,
    #[serde(default, rename = "default-user-jvm")]
    pub default_user_jvm: Vec<Value>,
}
#[derive(Clone, Deserialize)]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibraryDownloads>,
    pub rules: Option<Vec<Rule>>,
    pub natives: Option<HashMap<String, String>>,
    pub url: Option<String>,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}
#[derive(Clone, Deserialize)]
pub struct LibraryDownloads {
    pub artifact: Option<DownloadPath>,
    pub classifiers: Option<HashMap<String, DownloadPath>>,
}
#[derive(Clone, Deserialize)]
pub struct DownloadPath {
    pub path: String,
    pub url: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}
#[derive(Clone, Deserialize)]
pub struct Rule {
    pub action: String,
    pub os: Option<OsRule>,
    pub features: Option<HashMap<String, bool>>,
}
#[derive(Clone, Deserialize)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
    pub version: Option<String>,
    #[serde(rename = "versionRange")]
    pub version_range: Option<VersionRange>,
}
#[derive(Clone, Deserialize)]
pub struct VersionRange {
    pub min: Option<String>,
    pub max: Option<String>,
}
fn version_numbers(value: &str) -> Vec<u32> {
    value
        .split('.')
        .map_while(|segment| {
            segment
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .ok()
        })
        .collect()
}
fn version_range_matches(value: &str, range: &VersionRange) -> bool {
    let actual = version_numbers(value);
    if actual.is_empty() {
        return false;
    }
    let compare = |other: &str| {
        let mut left = actual.clone();
        let mut right = version_numbers(other);
        let len = left.len().max(right.len());
        left.resize(len, 0);
        right.resize(len, 0);
        left.cmp(&right)
    };
    range
        .min
        .as_ref()
        .is_none_or(|min| compare(min) != std::cmp::Ordering::Less)
        && range
            .max
            .as_ref()
            .is_none_or(|max| compare(max) == std::cmp::Ordering::Less)
}
#[derive(Clone, Deserialize)]
pub struct AssetObjects {
    pub objects: HashMap<String, AssetObject>,
}
#[derive(Clone, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

pub fn os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}
fn os_version() -> &'static str {
    static VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    VERSION.get_or_init(|| {
        #[cfg(unix)]
        {
            std::process::Command::new("/usr/bin/uname")
                .arg("-r")
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
                .unwrap_or_default()
        }
        #[cfg(not(unix))]
        {
            String::new()
        }
    })
}
pub fn allowed(rules: Option<&Vec<Rule>>, features: &HashMap<String, bool>) -> bool {
    let Some(rules) = rules else { return true };
    let mut result = false;
    for rule in rules {
        let os_ok = rule.os.as_ref().is_none_or(|os| {
            os.name.as_ref().is_none_or(|n| n == os_name())
                && os.arch.as_ref().is_none_or(|a| {
                    regex::Regex::new(a)
                        .ok()
                        .is_some_and(|r| r.is_match(std::env::consts::ARCH))
                })
                && os.version.as_ref().is_none_or(|v| {
                    regex::Regex::new(v)
                        .ok()
                        .is_some_and(|r| !os_version().is_empty() && r.is_match(os_version()))
                })
                && os
                    .version_range
                    .as_ref()
                    .is_none_or(|range| version_range_matches(os_version(), range))
        });
        let features_ok = rule.features.as_ref().is_none_or(|f| {
            f.iter()
                .all(|(k, v)| features.get(k).copied().unwrap_or(false) == *v)
        });
        if os_ok && features_ok {
            result = rule.action == "allow";
        }
    }
    result
}
pub fn argument_values(
    items: &[Value],
    features: &HashMap<String, bool>,
) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for item in items {
        if let Some(s) = item.as_str() {
            out.push(s.to_owned());
            continue;
        }
        if let Some(obj) = item.as_object() {
            let rules: Option<Vec<Rule>> = obj
                .get("rules")
                .map(|v| serde_json::from_value(v.clone()).map_err(|e| e.to_string()))
                .transpose()?;
            if allowed(rules.as_ref(), features) {
                match obj.get("value") {
                    Some(Value::String(s)) => out.push(s.clone()),
                    Some(Value::Array(a)) => {
                        for v in a {
                            out.push(v.as_str().ok_or("Invalid argument value")?.to_owned());
                        }
                    }
                    _ => return Err("Invalid argument entry".into()),
                }
            }
        }
    }
    Ok(out)
}
pub fn maven_path(name: &str) -> Result<String, String> {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() < 3
        || parts.len() > 4
        || parts
            .iter()
            .any(|p| p.is_empty() || p.contains("..") || p.contains(['/', '\\']) || p == &".")
    {
        return Err(format!("Invalid Maven coordinate: {name}"));
    }
    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let version = parts[2];
    let classifier = if parts.len() == 4 {
        format!("-{}", parts[3])
    } else {
        String::new()
    };
    Ok(format!(
        "{group}/{artifact}/{version}/{artifact}-{version}{classifier}.jar"
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rules_order() {
        let rules = vec![
            Rule {
                action: "allow".into(),
                os: None,
                features: None,
            },
            Rule {
                action: "disallow".into(),
                os: Some(OsRule {
                    name: Some(os_name().into()),
                    arch: None,
                    version: None,
                    version_range: None,
                }),
                features: None,
            },
        ];
        assert!(!allowed(Some(&rules), &HashMap::new()));
    }
    #[test]
    fn maven_rejects_traversal() {
        assert!(maven_path("a:b:../x").is_err());
    }
    #[test]
    fn version_range_bounds() {
        let range = VersionRange {
            min: Some("10.0.17134".into()),
            max: Some("11.0".into()),
        };
        assert!(!version_range_matches("10.0.16000", &range));
        assert!(version_range_matches("10.0.17134", &range));
        assert!(!version_range_matches("11.0", &range));
    }
}
