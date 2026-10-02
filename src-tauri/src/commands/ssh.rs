use std::path::PathBuf;

use serde::Serialize;
use tauri::AppHandle;

use crate::commands::rules::sync_rules;
use crate::config::store;
use crate::errors::BackendError;
use crate::models::GitProfile;
use crate::ssh;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedKey {
    pub private_key_path: String,
    pub public_key: String,
    pub profile: GitProfile,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshKeyInfo {
    pub path: String,
    pub name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshTestResult {
    pub success: bool,
    pub message: String,
}

fn config_path() -> Result<PathBuf, BackendError> {
    Ok(ssh::ssh_dir()?.join("config"))
}

fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, BackendError> + Send + 'static,
) -> impl std::future::Future<Output = Result<T, BackendError>> {
    async move {
        tauri::async_runtime::spawn_blocking(f)
            .await
            .map_err(|e| BackendError::io_error(format!("Background task failed: {}", e)))?
    }
}

fn generate_impl(
    app: &AppHandle,
    profile_id: &str,
    passphrase: &str,
) -> Result<GeneratedKey, BackendError> {
    let mut config = store::load_config(app)?;
    let profile = config
        .profiles
        .iter_mut()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| BackendError::not_found("Profile not found"))?;

    let file_name = format!("gitswitch_{}", ssh::slugify(&profile.label));
    let key = ssh::generate_key(&ssh::ssh_dir()?, &file_name, &profile.email, passphrase)?;
    let key_path = key.to_string_lossy().into_owned();

    profile.ssh_key_path = Some(key_path.clone());
    let updated = profile.clone();
    store::save_config(app, &config)?;
    sync_rules(app)?;
    ssh::refresh_host_block(&config_path()?, &updated)?;

    Ok(GeneratedKey {
        public_key: ssh::read_public_key(&key_path)?,
        private_key_path: key_path,
        profile: updated,
    })
}

/// Generates an ed25519 key for the profile and attaches it. The default passphrase
/// is empty; a passphrase passed here is visible in the process list while
/// `ssh-keygen` runs, so prefer `ssh-keygen -p` afterwards for sensitive keys.
#[tauri::command]
pub async fn generate_ssh_key(
    app: AppHandle,
    profile_id: String,
    passphrase: Option<String>,
) -> Result<GeneratedKey, BackendError> {
    blocking(move || generate_impl(&app, &profile_id, passphrase.as_deref().unwrap_or(""))).await
}

#[tauri::command]
pub fn list_ssh_keys(_app: AppHandle) -> Result<Vec<SshKeyInfo>, BackendError> {
    Ok(ssh::list_keys(&ssh::ssh_dir()?)
        .into_iter()
        .map(|p| SshKeyInfo {
            name: p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path: p.to_string_lossy().into_owned(),
        })
        .collect())
}

#[tauri::command]
pub fn read_public_key(_app: AppHandle, key_path: String) -> Result<String, BackendError> {
    ssh::read_public_key(&key_path)
}

/// Writes (or refreshes) the profile's `Host` block in `~/.ssh/config`.
#[tauri::command]
pub fn apply_ssh_alias(app: AppHandle, profile_id: String) -> Result<(), BackendError> {
    let config = store::load_config(&app)?;
    let profile = config
        .profiles
        .iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| BackendError::not_found("Profile not found"))?;
    ssh::write_host_block(&config_path()?, profile)
}

/// Tries `ssh -T git@<host>` without prompting. A rejected key is a normal result
/// (`success: false`), not an error.
#[tauri::command]
pub async fn test_ssh_connection(
    _app: AppHandle,
    host: String,
) -> Result<SshTestResult, BackendError> {
    ssh::validate_host(&host)?;
    blocking(move || {
        let target = format!("git@{}", host);
        let out = ssh::run_tool(
            "ssh",
            &[
                "-T",
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=10",
                "-o",
                "StrictHostKeyChecking=accept-new",
                &target,
            ],
        )?;
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
        .trim()
        .to_string();
        let lower = text.to_lowercase();
        // GitHub exits 1 even when authentication works.
        let success = out.status.success()
            || lower.contains("successfully authenticated")
            || lower.contains("welcome to gitlab");
        Ok(SshTestResult {
            success,
            message: if text.is_empty() { "No output from ssh".into() } else { text },
        })
    })
    .await
}

/// Called when a profile is edited or deleted so `~/.ssh/config` doesn't go stale.
pub fn refresh_alias_for(profile: &GitProfile) -> Result<(), BackendError> {
    ssh::refresh_host_block(&config_path()?, profile)
}

pub fn remove_alias_for(profile_id: &str) -> Result<(), BackendError> {
    ssh::remove_host_block(&config_path()?, profile_id)
}
