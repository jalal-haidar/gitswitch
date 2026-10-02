use std::path::Path;
use std::{env, fs};

use tauri::AppHandle;
use uuid::Uuid;

use crate::errors::BackendError;
use crate::git;
use crate::models::GitProfile;

const PRIVATE_KEY_NAMES: [&str; 4] = ["id_ed25519", "id_rsa", "id_ecdsa", "id_dsa"];

#[tauri::command]
pub fn detect_identities(
    _app: AppHandle,
    directory: Option<String>,
) -> Result<Vec<GitProfile>, BackendError> {
    // Config lookups run in `directory` (so repo-local values win), else the current dir.
    let dir = directory.or_else(|| env::var("PWD").ok()).unwrap_or_default();
    let cwd = if dir.is_empty() { None } else { Some(Path::new(&dir)) };

    let name = git::config_get("user.name", cwd)?.unwrap_or_default();
    let email = git::config_get("user.email", cwd)?.unwrap_or_default();
    let signingkey = git::config_get("user.signingkey", cwd)?.unwrap_or_default();
    let ssh_key_path = find_default_ssh_key();

    if name.is_empty() && email.is_empty() && signingkey.is_empty() && ssh_key_path.is_none() {
        return Ok(vec![]);
    }

    let label = match (name.is_empty(), email.is_empty()) {
        (false, false) => format!("{} <{}>", name, email),
        (false, true) => name.clone(),
        _ => email.clone(),
    };

    Ok(vec![GitProfile {
        id: Uuid::new_v4().to_string(),
        label,
        name,
        email,
        color: "#6A5ACD".to_string(),
        ssh_key_path,
        gpg_key_id: if signingkey.is_empty() { None } else { Some(signingkey) },
        is_default: false,
        ..Default::default()
    }])
}

fn find_default_ssh_key() -> Option<String> {
    let home = crate::paths::home_dir()?;
    let entries = fs::read_dir(home.join(".ssh")).ok()?;
    entries.flatten().map(|e| e.path()).find_map(|p| {
        let fname = p.file_name()?.to_str()?;
        PRIVATE_KEY_NAMES
            .contains(&fname)
            .then(|| p.to_string_lossy().into_owned())
    })
}
