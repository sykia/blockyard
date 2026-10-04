use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub version: String,
    pub loader: Loader,
    pub java_path: Option<String>,
    pub ram_mb: u32,
    pub jvm_args: String,
    pub width: u32,
    pub height: u32,
    pub icon: String,
    pub last_played: Option<String>,
    pub playtime_seconds: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "version", rename_all = "lowercase")]
pub enum Loader {
    Vanilla,
    Fabric(String),
    Neoforge(String),
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub ram_mb: u32,
    pub java_path: Option<String>,
    pub download_concurrency: usize,
    pub show_snapshots: bool,
    pub theme: String,
    pub minimize_on_launch: bool,
    pub width: u32,
    pub height: u32,
    pub microsoft_client_id: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            ram_mb: 4096,
            java_path: None,
            download_concurrency: 8,
            show_snapshots: false,
            theme: "dark".into(),
            minimize_on_launch: false,
            width: 1280,
            height: 720,
            microsoft_client_id: String::new(),
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: AccountKind,
    pub xuid: String,
    pub expires_at: i64,
}
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    #[default]
    Microsoft,
    Offline,
}
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Database {
    pub instances: Vec<Instance>,
    pub accounts: Vec<Account>,
    pub active_account: Option<String>,
    pub settings: Settings,
    #[serde(default)]
    pub warnings: Vec<String>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionChoice {
    pub id: String,
    pub kind: String,
    pub release_time: String,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModFile {
    pub name: String,
    pub enabled: bool,
    pub size: u64,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameStatus {
    pub instance_id: String,
    pub phase: String,
    pub progress: f64,
    pub message: String,
    pub exit_code: Option<i32>,
}
