use serde::Serialize;

use crate::path_security;
use crate::ssh_keys;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedKey {
    pub private_key_path: String,
    pub public_key: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshKeyInfo {
    pub path: String,
    pub name: String,
}

fn ssh_dir() -> Result<std::path::PathBuf, String> {
    Ok(path_security::canonical_user_home()?.join(".ssh"))
}

/// Generates a passphrase-less ed25519 key in `~/.ssh` named after the profile label.
/// Add a passphrase afterwards with `ssh-keygen -p` for sensitive keys.
#[tauri::command]
pub async fn generate_ssh_key(label: String, email: String) -> Result<GeneratedKey, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = ssh_dir()?;
        let file_name = format!("gitswitch_{}", ssh_keys::slugify(&label));
        let key = ssh_keys::generate_key(&dir, &file_name, email.trim())?;
        let public_key = ssh_keys::read_public_key(&key)?;
        Ok(GeneratedKey {
            private_key_path: key.to_string_lossy().into_owned(),
            public_key,
        })
    })
    .await
    .map_err(|e| format!("Background task failed: {e}"))?
}

#[tauri::command]
pub fn list_ssh_keys() -> Result<Vec<SshKeyInfo>, String> {
    Ok(ssh_keys::list_keys(&ssh_dir()?)
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
pub fn read_public_key(key_path: String) -> Result<String, String> {
    let key = path_security::canonical_ssh_key(key_path.trim())?;
    ssh_keys::read_public_key(&key)
}
