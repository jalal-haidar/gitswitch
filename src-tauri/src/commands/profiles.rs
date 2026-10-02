use tauri::AppHandle;
use uuid::Uuid;

use crate::commands::rules::sync_rules;
use crate::commands::ssh;
use crate::config::store;
use crate::errors::BackendError;
use crate::git;
use crate::models::GitProfile;

#[tauri::command]
pub fn get_profiles(app: AppHandle) -> Result<Vec<GitProfile>, BackendError> {
    Ok(store::load_config(&app)?.profiles)
}

#[tauri::command]
pub fn add_profile(app: AppHandle, mut profile: GitProfile) -> Result<GitProfile, BackendError> {
    let mut config = store::load_config(&app)?;

    if profile.id.is_empty() {
        profile.id = Uuid::new_v4().to_string();
    }

    // The first profile, or one marked as default, becomes the only default.
    if profile.is_default || config.profiles.is_empty() {
        profile.is_default = true;
        for existing in &mut config.profiles {
            existing.is_default = false;
        }
    }

    config.profiles.push(profile.clone());
    store::save_config(&app, &config)?;
    Ok(profile)
}

#[tauri::command]
pub fn update_profile(app: AppHandle, profile: GitProfile) -> Result<GitProfile, BackendError> {
    let mut config = store::load_config(&app)?;

    let index = config
        .profiles
        .iter()
        .position(|p| p.id == profile.id)
        .ok_or_else(|| BackendError::not_found("Profile not found"))?;

    if profile.is_default {
        for other in &mut config.profiles {
            other.is_default = false;
        }
    }
    config.profiles[index] = profile.clone();

    // Never leave the list without a default.
    if config.profiles.iter().all(|p| !p.is_default) {
        config.profiles[0].is_default = true;
    }

    store::save_config(&app, &config)?;
    sync_rules(&app)?;
    ssh::refresh_alias_for(&profile)?;
    Ok(profile)
}

#[tauri::command]
pub fn delete_profile(app: AppHandle, id: String) -> Result<(), BackendError> {
    let mut config = store::load_config(&app)?;

    let initial_len = config.profiles.len();
    config.profiles.retain(|p| p.id != id);
    if config.profiles.len() == initial_len {
        return Err(BackendError::not_found("Profile not found"));
    }

    config.directory_rules.retain(|r| r.profile_id != id);

    if config.profiles.iter().all(|p| !p.is_default) && !config.profiles.is_empty() {
        config.profiles[0].is_default = true;
    }

    store::save_config(&app, &config)?;
    sync_rules(&app)?;
    ssh::remove_alias_for(&id)?;
    Ok(())
}

#[tauri::command]
pub fn switch_profile_globally(app: AppHandle, id: String) -> Result<(), BackendError> {
    let config = store::load_config(&app)?;
    let profile = config
        .profiles
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| BackendError::not_found("Profile not found"))?;

    git::apply_identity_globally(&profile.name, &profile.email, profile.gpg_key_id.as_deref())
}

#[tauri::command]
pub fn apply_identity(
    _app: AppHandle,
    name: String,
    email: String,
    gpg_key: Option<String>,
) -> Result<(), BackendError> {
    git::apply_identity_globally(&name, &email, gpg_key.as_deref())
}
