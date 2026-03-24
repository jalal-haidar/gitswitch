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
    /// Only populated by detect/scan commands — never persisted to profiles.json
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_service: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryRule {
    #[serde(default)]
    pub id: String,
    pub path: String,
    pub profile_id: String,
    /// Epoch-ms timestamp of the last auto-switch event for this rule
    #[serde(default)]
    pub last_triggered_at: Option<u64>,
}

/// Returned by `scan_repos` — describes a discovered git repository.
/// Never written back to profiles.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannedRepo {
    pub path: String,
    pub name: String,
    pub user_name: Option<String>,
    pub user_email: Option<String>,
    pub remote_url: Option<String>,
    /// One of: "github", "gitlab", "bitbucket", "other", or None if no remote
    pub remote_service: Option<String>,
    /// ID of the GitSwitch profile whose name+email matches this repo's identity
    pub matched_profile_id: Option<String>,
    /// Repo-local `core.sshCommand` if configured (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitConfigSnapshot {
    pub user_name: Option<String>,
    pub user_email: Option<String>,
    pub user_signingkey: Option<String>,
    pub commit_gpgsign: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub core_ssh_command: Option<String>,
}

// Add ssh_command to scanned repo so UI can show repo-local core.sshCommand
impl ScannedRepo {
    // keep existing struct; helper constructor can be added elsewhere if needed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct AppSettings {
    pub auto_switch: bool,
    pub show_notifications: bool,
    pub start_with_system: bool,
    pub theme: String,
    pub store_sensitive_in_keyring: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            auto_switch: true,
            show_notifications: true,
            start_with_system: false,
            theme: "system".to_string(),
            store_sensitive_in_keyring: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(default)]
    pub profiles: Vec<GitProfile>,
    #[serde(default)]
    pub active_profile_id: Option<String>,
    #[serde(default)]
    pub directory_rules: Vec<DirectoryRule>,
    #[serde(default)]
    pub settings: AppSettings,
}
