use std::path::Path;
use std::process::Command;
use tauri::AppHandle;
use uuid::Uuid;

use crate::config::store;
use crate::models::GitProfile;
use crate::errors::BackendError;

#[tauri::command]
pub fn get_profiles(app: AppHandle) -> Result<Vec<GitProfile>, String> {
    let config = store::load_config(&app).map_err(|e| e.to_string())?;
    Ok(config.profiles)
}

#[tauri::command]
pub fn get_active_profile_id(app: AppHandle) -> Result<Option<String>, String> {
    let config = store::load_config(&app).map_err(|e| e.to_string())?;
    Ok(config.active_profile_id)
}

#[tauri::command]
pub fn add_profile(app: AppHandle, mut profile: GitProfile) -> Result<GitProfile, String> {
    let mut config = store::load_config(&app).map_err(|e| e.to_string())?;
    
    // Assign a new ID if it's empty
    if profile.id.is_empty() {
        profile.id = Uuid::new_v4().to_string();
    }
    
    // if this is the first profile, or marked as default, make all others non-default
    if profile.is_default || config.profiles.is_empty() {
        profile.is_default = true;
        for existing_profile in &mut config.profiles {
            existing_profile.is_default = false;
        }
    }

    if config.active_profile_id.is_none() {
        config.active_profile_id = Some(profile.id.clone());
    }
    
    config.profiles.push(profile.clone());
    store::save_config(&app, &config).map_err(|e| e.to_string())?;
    
    Ok(profile)
}

#[tauri::command]
pub fn update_profile(app: AppHandle, profile: GitProfile) -> Result<GitProfile, String> {
    let mut config = store::load_config(&app).map_err(|e| e.to_string())?;
    
    let mut found = false;
    for existing_profile in &mut config.profiles {
        if existing_profile.id == profile.id {
            // Update fields
            existing_profile.label = profile.label.clone();
            existing_profile.name = profile.name.clone();
            existing_profile.email = profile.email.clone();
            existing_profile.color = profile.color.clone();
            existing_profile.ssh_key_path = profile.ssh_key_path.clone();
            existing_profile.gpg_key_id = profile.gpg_key_id.clone();
            
            // Handle default change
            if profile.is_default && !existing_profile.is_default {
                existing_profile.is_default = true;
                found = true;
            } else {
                found = true;
            }
        } else if profile.is_default {
            existing_profile.is_default = false;
        }
    }
    
    if !found {
        return Err("Profile not found".to_string());
    }
    
    store::save_config(&app, &config).map_err(|e| e.to_string())?;
    
    Ok(profile)
}

#[tauri::command]
pub fn delete_profile(app: AppHandle, id: String) -> Result<(), String> {
    let mut config = store::load_config(&app).map_err(|e| e.to_string())?;

    if config.directory_rules.iter().any(|rule| rule.profile_id == id) {
        return Err("Cannot delete profile while it is referenced by directory rules".to_string());
    }
    
    let initial_len = config.profiles.len();
    config.profiles.retain(|p| p.id != id);
    
    if config.profiles.len() == initial_len {
        return Err("Profile not found".to_string());
    }
    
    // If we deleted the default profile, make the first remaining one default (if any)
    if config.profiles.iter().all(|p| !p.is_default) && !config.profiles.is_empty() {
        config.profiles[0].is_default = true;
    }

    if config.active_profile_id.as_deref() == Some(id.as_str()) {
        config.active_profile_id = config.profiles.first().map(|p| p.id.clone());
    }
    
    store::save_config(&app, &config).map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
pub fn switch_profile_globally(app: AppHandle, id: String) -> Result<(), String> {
    let mut config = store::load_config(&app).map_err(|e| e.to_string())?;
    let profile = config.profiles.iter().find(|p| p.id == id)
        .ok_or_else(|| "Profile not found".to_string())?;
        
    // Execute git config --global commands
    execute_git_command(vec!["config", "--global", "user.name", &profile.name])?;
    execute_git_command(vec!["config", "--global", "user.email", &profile.email])?;
    
    if let Some(ref gpg_key) = profile.gpg_key_id {
        if !gpg_key.is_empty() {
            execute_git_command(vec!["config", "--global", "user.signingkey", gpg_key])?;
            execute_git_command(vec!["config", "--global", "commit.gpgsign", "true"])?;
        } else {
            execute_git_command(vec!["config", "--global", "--unset", "user.signingkey"]).ok();
            execute_git_command(vec!["config", "--global", "commit.gpgsign", "false"]).ok();
        }
    } else {
        execute_git_command(vec!["config", "--global", "--unset", "user.signingkey"]).ok();
        execute_git_command(vec!["config", "--global", "commit.gpgsign", "false"]).ok();
    }

    config.active_profile_id = Some(id);
    store::save_config(&app, &config).map_err(|e| e.to_string())?;
    
    Ok(())
}

pub fn switch_profile_for_repo(app: AppHandle, id: String, repo_path: &Path) -> Result<(), String> {
    let mut config = store::load_config(&app).map_err(|e| e.to_string())?;
    let profile = config
        .profiles
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| "Profile not found".to_string())?;

    execute_git_command_in_dir(vec!["config", "--local", "user.name", &profile.name], Some(repo_path))?;
    execute_git_command_in_dir(vec!["config", "--local", "user.email", &profile.email], Some(repo_path))?;

    if let Some(ref gpg_key) = profile.gpg_key_id {
        if !gpg_key.is_empty() {
            execute_git_command_in_dir(vec!["config", "--local", "user.signingkey", gpg_key], Some(repo_path))?;
            execute_git_command_in_dir(vec!["config", "--local", "commit.gpgsign", "true"], Some(repo_path))?;
        } else {
            execute_git_command_in_dir(vec!["config", "--local", "--unset", "user.signingkey"], Some(repo_path)).ok();
            execute_git_command_in_dir(vec!["config", "--local", "commit.gpgsign", "false"], Some(repo_path)).ok();
        }
    } else {
        execute_git_command_in_dir(vec!["config", "--local", "--unset", "user.signingkey"], Some(repo_path)).ok();
        execute_git_command_in_dir(vec!["config", "--local", "commit.gpgsign", "false"], Some(repo_path)).ok();
    }

    config.active_profile_id = Some(id);
    store::save_config(&app, &config).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn apply_identity(_app: AppHandle, name: String, email: String, gpg_key: Option<String>) -> Result<(), String> {
    // Apply the given identity directly to global git config
    execute_git_command(vec!["config", "--global", "user.name", &name])?;
    execute_git_command(vec!["config", "--global", "user.email", &email])?;

    if let Some(ref gpg) = gpg_key {
        if !gpg.is_empty() {
            execute_git_command(vec!["config", "--global", "user.signingkey", gpg])?;
            execute_git_command(vec!["config", "--global", "commit.gpgsign", "true"])?;
        } else {
            execute_git_command(vec!["config", "--global", "--unset", "user.signingkey"]).ok();
            execute_git_command(vec!["config", "--global", "commit.gpgsign", "false"]).ok();
        }
    } else {
        execute_git_command(vec!["config", "--global", "--unset", "user.signingkey"]).ok();
        execute_git_command(vec!["config", "--global", "commit.gpgsign", "false"]).ok();
    }

    Ok(())
}

fn execute_git_command(args: Vec<&str>) -> Result<(), String> {
    execute_git_command_in_dir(args, None)
}

fn execute_git_command_in_dir(args: Vec<&str>, cwd: Option<&Path>) -> Result<(), String> {
    // Try to spawn `git` and handle common errors with structured hints
    let mut command = Command::new("git");
    command.args(&args);
    if let Some(path) = cwd {
        command.current_dir(path);
    }

    let output = command.output().map_err(|e| {
        // If git isn't found on PATH, return a helpful BackendError serialized to string
        if e.kind() == std::io::ErrorKind::NotFound {
            BackendError::git_not_found().to_string()
        } else {
            BackendError::io_error(format!("Failed to execute git command: {}", e)).to_string()
        }
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        // Detect permission denied
        let stderr_l = stderr.to_lowercase();
        if stderr_l.contains("permission denied") || stderr_l.contains("cannot open") {
            return Err(BackendError::permission_denied(stderr).to_string());
        }

        return Err(BackendError::git_failed(stderr).to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execute_git_command_returns_git_failed_on_bad_args() {
        // calling git with an invalid rev-parse flag should produce a failing exit status
        let res = execute_git_command(vec!["rev-parse", "--not-a-real-arg"]);
            if let Err(err) = res {
                // the error string should include serialized BackendError with kind GitFailed
                assert!(err.contains("GitFailed") || err.to_lowercase().contains("git command failed"), "unexpected error payload: {}", err);
            }
    }
}
