use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default)]
pub(crate) struct KeyringBaseline {
    pub entries: HashMap<String, String>,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RepoApplySource {
    Manual,
    Auto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoApplyEvent {
    pub profile_id: String,
    pub profile_label: String,
    pub repository_path: String,
    pub source: RepoApplySource,
    pub occurred_at_epoch_ms: u64,
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
    /// ID of the single GitSwitch profile matching all five local Git fields.
    pub applied_profile_id: Option<String>,
    /// Repo-local `core.sshCommand` if configured (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_command: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitConfigSnapshot {
    pub user_name: Option<String>,
    pub user_email: Option<String>,
    pub user_signingkey: Option<String>,
    pub commit_gpgsign: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub core_ssh_command: Option<String>,
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
    /// Read only for migration from the pre-v0.2.8 ambiguous state. Never save it again.
    #[serde(default, rename = "activeProfileId", skip_serializing)]
    pub(crate) legacy_active_profile_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_repo_activity: Option<RepoApplyEvent>,
    #[serde(default)]
    pub directory_rules: Vec<DirectoryRule>,
    #[serde(default)]
    pub settings: AppSettings,
    /// Accounts that are expected to exist in the OS keyring. `None` denotes
    /// the legacy pre-manifest format; `Some(empty)` is an authoritative empty
    /// manifest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) keyring_entries: Option<HashSet<String>>,
    /// Hydrated credential values captured when the config was loaded. This is
    /// transaction state only and is never persisted.
    #[serde(skip)]
    pub(crate) keyring_baseline: KeyringBaseline,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_active_profile_is_read_but_never_written() {
        let config: AppConfig = serde_json::from_str(
            r#"{"activeProfileId":"legacy-profile","profiles":[],"directoryRules":[],"settings":{}}"#,
        )
        .unwrap();
        assert_eq!(
            config.legacy_active_profile_id.as_deref(),
            Some("legacy-profile")
        );

        let saved = serde_json::to_value(config).unwrap();
        assert!(saved.get("activeProfileId").is_none());
    }

    #[test]
    fn repo_activity_uses_explicit_camel_case_contract() {
        let event = RepoApplyEvent {
            profile_id: "work".to_string(),
            profile_label: "Work".to_string(),
            repository_path: "C:/code/repo".to_string(),
            source: RepoApplySource::Auto,
            occurred_at_epoch_ms: 42,
        };

        assert_eq!(
            serde_json::to_value(event).unwrap(),
            serde_json::json!({
                "profileId": "work",
                "profileLabel": "Work",
                "repositoryPath": "C:/code/repo",
                "source": "auto",
                "occurredAtEpochMs": 42
            })
        );
    }
}
