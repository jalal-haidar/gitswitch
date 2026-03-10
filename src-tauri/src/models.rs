use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitProfile {
    pub id: String,
    pub label: String,
    pub name: String,
    pub email: String,
    pub color: String,
    pub ssh_key_path: Option<String>,
    pub gpg_key_id: Option<String>,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryRule {
    pub path: String,
    pub profile_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub auto_switch: bool,
    pub show_notifications: bool,
    pub start_with_system: bool,
    pub theme: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            auto_switch: true,
            show_notifications: true,
            start_with_system: false,
            theme: "system".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub profiles: Vec<GitProfile>,
    pub directory_rules: Vec<DirectoryRule>,
    pub settings: AppSettings,
}
