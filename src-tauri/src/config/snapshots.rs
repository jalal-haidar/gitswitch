use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::config::store::atomic_replace;
use crate::models::GitConfigSnapshot;
use crate::path_security;

const SNAPSHOT_FILE_NAME: &str = "git-snapshots.json";
const SNAPSHOT_VERSION: u32 = 1;

static SNAPSHOT_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct SnapshotDocument {
    pub version: u32,
    pub global: Option<GitConfigSnapshot>,
    pub repositories: HashMap<String, GitConfigSnapshot>,
}

impl Default for SnapshotDocument {
    fn default() -> Self {
        Self {
            version: SNAPSHOT_VERSION,
            global: None,
            repositories: HashMap::new(),
        }
    }
}

fn snapshot_lock() -> MutexGuard<'static, ()> {
    SNAPSHOT_LOCK.lock().unwrap_or_else(|poisoned| {
        eprintln!("[snapshots] mutex poisoned, recovering");
        poisoned.into_inner()
    })
}

fn snapshot_path(app: &AppHandle) -> Result<PathBuf> {
    let directory = app
        .path()
        .app_config_dir()
        .context("Failed to get app config directory")?;
    fs::create_dir_all(&directory)
        .with_context(|| format!("Failed to create app config directory at {directory:?}"))?;
    Ok(directory.join(SNAPSHOT_FILE_NAME))
}

pub(crate) fn normalize_repo_key(path: &Path) -> Result<String, String> {
    let canonical = path_security::canonicalize_existing(path, "repository")?;
    let mut normalized = canonical.to_string_lossy().replace('\\', "/");
    #[cfg(windows)]
    {
        normalized = normalized.to_lowercase();
    }
    Ok(normalized)
}

pub(crate) fn load_at(path: &Path) -> Result<SnapshotDocument> {
    let backup = path.with_extension("json.bak");
    if !path.exists() && backup.exists() {
        fs::rename(&backup, path)
            .with_context(|| format!("Failed to recover snapshot backup at {backup:?}"))?;
    }
    if !path.exists() {
        return Ok(SnapshotDocument::default());
    }
    let contents =
        fs::read(path).with_context(|| format!("Failed to read Git snapshots at {path:?}"))?;
    let document: SnapshotDocument = serde_json::from_slice(&contents)
        .with_context(|| format!("Failed to parse Git snapshots at {path:?}"))?;
    if document.version != SNAPSHOT_VERSION {
        anyhow::bail!(
            "Unsupported Git snapshot version {} at {:?}",
            document.version,
            path
        );
    }
    Ok(document)
}

pub(crate) fn persist_at(path: &Path, document: &SnapshotDocument) -> Result<()> {
    let contents =
        serde_json::to_vec_pretty(document).context("Failed to serialize durable Git snapshots")?;
    atomic_replace(path, &contents)
}

pub(crate) fn global(app: &AppHandle) -> Result<Option<GitConfigSnapshot>, String> {
    let _guard = snapshot_lock();
    let path = snapshot_path(app).map_err(|error| error.to_string())?;
    Ok(load_at(&path).map_err(|error| error.to_string())?.global)
}

pub(crate) fn swap_global(
    app: &AppHandle,
    replacement: Option<GitConfigSnapshot>,
) -> Result<Option<GitConfigSnapshot>, String> {
    let _guard = snapshot_lock();
    let path = snapshot_path(app).map_err(|error| error.to_string())?;
    let mut document = load_at(&path).map_err(|error| error.to_string())?;
    let previous = std::mem::replace(&mut document.global, replacement);
    persist_at(&path, &document).map_err(|error| error.to_string())?;
    Ok(previous)
}

pub(crate) fn repository(
    app: &AppHandle,
    repository: &Path,
) -> Result<Option<GitConfigSnapshot>, String> {
    let key = normalize_repo_key(repository)?;
    let _guard = snapshot_lock();
    let path = snapshot_path(app).map_err(|error| error.to_string())?;
    Ok(load_at(&path)
        .map_err(|error| error.to_string())?
        .repositories
        .get(&key)
        .cloned())
}

pub(crate) fn swap_repository(
    app: &AppHandle,
    repository: &Path,
    replacement: Option<GitConfigSnapshot>,
) -> Result<Option<GitConfigSnapshot>, String> {
    let key = normalize_repo_key(repository)?;
    let _guard = snapshot_lock();
    let path = snapshot_path(app).map_err(|error| error.to_string())?;
    let mut document = load_at(&path).map_err(|error| error.to_string())?;
    let previous = match replacement {
        Some(snapshot) => document.repositories.insert(key, snapshot),
        None => document.repositories.remove(&key),
    };
    persist_at(&path, &document).map_err(|error| error.to_string())?;
    Ok(previous)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_loads_empty_versioned_document() {
        let path = std::env::temp_dir().join(format!(
            "gitswitch-missing-snapshots-{}.json",
            std::process::id()
        ));
        let document = load_at(&path).unwrap();
        assert_eq!(document.version, SNAPSHOT_VERSION);
        assert!(document.global.is_none());
        assert!(document.repositories.is_empty());
    }
}
