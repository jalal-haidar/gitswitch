use std::path::Path;

use serde::Serialize;
use tauri::AppHandle;

use crate::config::store;
use crate::errors::BackendError;
use crate::git;
use crate::identity;
use crate::models::DirectoryRule;
use crate::paths;

/// Rewrites identity files and the gitconfig block from the saved config.
pub fn sync_rules(app: &AppHandle) -> Result<(), BackendError> {
    let config = store::load_config(app)?;
    let gitconfig = paths::global_gitconfig()
        .ok_or_else(|| BackendError::io_error("Could not locate the home directory"))?;
    identity::apply_rules(&config, &store::config_dir(app)?.join("identities"), &gitconfig)
}

#[tauri::command]
pub fn get_directory_rules(app: AppHandle) -> Result<Vec<DirectoryRule>, BackendError> {
    Ok(store::load_config(&app)?.directory_rules)
}

#[tauri::command]
pub fn add_directory_rule(
    app: AppHandle,
    path: String,
    profile_id: String,
) -> Result<DirectoryRule, BackendError> {
    let mut config = store::load_config(&app)?;

    if !config.profiles.iter().any(|p| p.id == profile_id) {
        return Err(BackendError::not_found("Profile not found"));
    }

    let expanded = identity::normalize_gitdir(&path, paths::home_dir().as_deref());
    let real = std::fs::canonicalize(&expanded).map_err(|_| {
        BackendError::invalid_input(format!("Directory does not exist: {}", path.trim()))
    })?;
    if !real.is_dir() {
        return Err(BackendError::invalid_input(format!(
            "Not a directory: {}",
            path.trim()
        )));
    }
    let normalized = identity::normalize_gitdir(&real.to_string_lossy(), None);

    let duplicate = config
        .directory_rules
        .iter()
        .any(|r| r.path.eq_ignore_ascii_case(&normalized));
    if duplicate {
        return Err(BackendError::invalid_input(
            "A rule for this directory already exists",
        ));
    }

    let rule = DirectoryRule {
        path: normalized,
        profile_id,
    };
    config.directory_rules.push(rule.clone());
    store::save_config(&app, &config)?;
    sync_rules(&app)?;
    Ok(rule)
}

#[tauri::command]
pub fn remove_directory_rule(app: AppHandle, path: String) -> Result<(), BackendError> {
    let mut config = store::load_config(&app)?;
    let before = config.directory_rules.len();
    config.directory_rules.retain(|r| r.path != path);
    if config.directory_rules.len() == before {
        return Err(BackendError::not_found("Rule not found"));
    }
    store::save_config(&app, &config)?;
    sync_rules(&app)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveIdentity {
    pub name: Option<String>,
    pub email: Option<String>,
}

/// The identity git would use for commits made in `path`.
#[tauri::command]
pub fn preview_directory(_app: AppHandle, path: String) -> Result<EffectiveIdentity, BackendError> {
    let dir = Path::new(path.trim());
    if !dir.is_dir() {
        return Err(BackendError::invalid_input(format!(
            "Directory does not exist: {}",
            path.trim()
        )));
    }
    // `git config` outside a repository would ignore includeIf "gitdir:"; ask git
    // for the repo's identity only when the directory is inside one.
    if git::run_git(&["rev-parse", "--git-dir"], Some(dir)).is_err() {
        return Ok(EffectiveIdentity { name: None, email: None });
    }
    Ok(EffectiveIdentity {
        name: git::config_get("user.name", Some(dir))?,
        email: git::config_get("user.email", Some(dir))?,
    })
}
