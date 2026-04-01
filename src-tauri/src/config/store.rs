use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

use keyring::Entry;

use crate::models::AppConfig;
use std::collections::HashMap;
use once_cell::sync::Lazy;
use std::sync::Mutex;
use crate::models::GitConfigSnapshot;

// In-memory transient snapshots keyed by repo path. Not persisted to disk.
static TRANSIENT_SNAPSHOTS: Lazy<Mutex<HashMap<String, GitConfigSnapshot>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Normalize a snapshot key to a canonical form so the same repo directory
/// always resolves to the same key regardless of how the path is represented.
fn normalize_snapshot_key(key: &str) -> String {
    // Try to canonicalize; fall back to lowercased + forward-slash form.
    if let Ok(canonical) = std::fs::canonicalize(key) {
        let s = canonical.to_string_lossy().to_string();
        // Strip Windows extended-length prefix for consistency
        #[cfg(windows)]
        {
            if let Some(stripped) = s.strip_prefix("\\\\?\\") {
                return stripped.to_lowercase();
            }
        }
        return s.to_lowercase();
    }
    // Fallback: normalize slashes and case
    key.replace('\\', "/").to_lowercase()
}

pub fn set_transient_snapshot(key: &str, snap: GitConfigSnapshot) {
    let normalized = normalize_snapshot_key(key);
    match TRANSIENT_SNAPSHOTS.lock() {
        Ok(mut m) => {
            m.insert(normalized, snap);
        }
        Err(poisoned) => {
            eprintln!("[store] snapshot mutex poisoned, recovering for write");
            poisoned.into_inner().insert(normalized, snap);
        }
    }
}

pub fn take_transient_snapshot(key: &str) -> Option<GitConfigSnapshot> {
    let normalized = normalize_snapshot_key(key);
    match TRANSIENT_SNAPSHOTS.lock() {
        Ok(mut m) => m.remove(&normalized),
        Err(poisoned) => {
            eprintln!("[store] snapshot mutex poisoned, recovering for take");
            poisoned.into_inner().remove(&normalized)
        }
    }
}

pub fn has_transient_snapshot(key: &str) -> bool {
    let normalized = normalize_snapshot_key(key);
    match TRANSIENT_SNAPSHOTS.lock() {
        Ok(m) => m.contains_key(&normalized),
        Err(poisoned) => {
            eprintln!("[store] snapshot mutex poisoned, recovering for check");
            poisoned.into_inner().contains_key(&normalized)
        }
    }
}

/// Try a keyring operation; if it fails, emit a "keyring-warning" event to the
/// frontend so the user is informed their credential is NOT securely stored.
fn try_keyring<F: FnOnce() -> keyring::Result<()>>(app: &AppHandle, label: &str, op: F) {
    if let Err(e) = op() {
        let _ = app.emit(
            "keyring-warning",
            format!("Keyring operation failed for '{label}': {e}. The value will be stored in plain text."),
        );
    }
}

const CONFIG_FILE_NAME: &str = "profiles.json";

fn get_config_path(app_handle: &AppHandle) -> Result<PathBuf> {
    let app_dir = app_handle
        .path()
        .app_config_dir()
        .context("Failed to get app config dir")?;
    
    if !app_dir.exists() {
        fs::create_dir_all(&app_dir).context("Failed to create app config directory")?;
    }
    
    Ok(app_dir.join(CONFIG_FILE_NAME))
}

pub fn load_config(app_handle: &AppHandle) -> Result<AppConfig> {
    let config_path = get_config_path(app_handle)?;
    
    if !config_path.exists() {
        let default_config = AppConfig::default();
        save_config(app_handle, &default_config)?;
        return Ok(default_config);
    }
    
    let contents = fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read config file at {:?}", config_path))?;
        
    let mut config: AppConfig = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse config file at {:?}", config_path))?;

    // Attempt to retrieve any sensitive fields from the OS keyring
    if config.settings.store_sensitive_in_keyring {
        for profile in &mut config.profiles {
        // ssh_key_path
        let key = format!("{}:ssh_key_path", profile.id);
        if let Ok(k) = Entry::new("gitswitch", &key).get_password() {
            if !k.is_empty() {
                profile.ssh_key_path = Some(k);
            }
        }

        // gpg_key_id
        let key = format!("{}:gpg_key_id", profile.id);
        if let Ok(k) = Entry::new("gitswitch", &key).get_password() {
            if !k.is_empty() {
                profile.gpg_key_id = Some(k);
            }
        }
        }
    }

    Ok(config)
}

pub fn save_config(app_handle: &AppHandle, config: &AppConfig) -> Result<()> {
    let config_path = get_config_path(app_handle)?;
    // Before serializing, move sensitive fields into OS keyring and clear them
    let mut config_for_save = config.clone();
    if config_for_save.settings.store_sensitive_in_keyring {
        for profile in &mut config_for_save.profiles {
            if let Some(ref ssh) = profile.ssh_key_path {
                let key = format!("{}:ssh_key_path", profile.id);
                let entry = Entry::new("gitswitch", &key);
                let ssh_val = ssh.clone();
                try_keyring(app_handle, &key, move || entry.set_password(&ssh_val));
                profile.ssh_key_path = None;
            }

            if let Some(ref gpg) = profile.gpg_key_id {
                let key = format!("{}:gpg_key_id", profile.id);
                let entry = Entry::new("gitswitch", &key);
                let gpg_val = gpg.clone();
                try_keyring(app_handle, &key, move || entry.set_password(&gpg_val));
                profile.gpg_key_id = None;
            }
        }
    } else {
        // If not storing in keyring, silently attempt to clean up any existing
        // keyring entries (best-effort; failures are harmless here).
        for profile in &mut config_for_save.profiles {
            let key = format!("{}:ssh_key_path", profile.id);
            let _ = Entry::new("gitswitch", &key).delete_password();
            let key = format!("{}:gpg_key_id", profile.id);
            let _ = Entry::new("gitswitch", &key).delete_password();
            // keep fields as-is in JSON
        }
    }

    let contents = serde_json::to_string_pretty(&config_for_save)
        .context("Failed to serialize AppConfig")?;
        
    // Atomic write: write to a temp file in the same directory, flush, then rename.
    // This avoids truncation/corruption if the process is interrupted during write.
    let tmp_path = config_path.with_extension("json.tmp");

    // Create and write the temp file
    let mut tmp_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&tmp_path)
        .with_context(|| format!("Failed to create temp config file at {:?}", tmp_path))?;

    tmp_file
        .write_all(contents.as_bytes())
        .context("Failed to write to temp config file")?;

    // Ensure contents are flushed to disk
    tmp_file
        .sync_all()
        .context("Failed to sync temp config file to disk")?;

    // Attempt atomic rename. On some platforms (Windows) rename may fail if the
    // destination exists — attempt a remove+rename fallback there.
    match fs::rename(&tmp_path, &config_path) {
        Ok(_) => Ok(()),
        Err(_e) => {
            #[cfg(windows)]
            {
                if config_path.exists() {
                    fs::remove_file(&config_path).with_context(|| {
                        format!("Failed to remove existing config file at {:?}", config_path)
                    })?;
                }

                fs::rename(&tmp_path, &config_path).with_context(|| {
                    format!("Failed to rename temp config file to {:?}", config_path)
                })?;

                Ok(())
            }

            #[cfg(not(windows))]
            {
                Err(anyhow::anyhow!(
                    "Failed to rename temp config file to {:?}: {}",
                    config_path,
                    _e
                ))
            }
        }
    }
}
