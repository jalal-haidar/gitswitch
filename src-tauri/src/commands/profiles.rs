use std::process::Command;
use tauri::AppHandle;
use uuid::Uuid;

use crate::config::store;
use crate::models::{AppConfig, GitProfile};

#[tauri::command]
pub fn get_profiles(app: AppHandle) -> Result<Vec<GitProfile>, String> {
    let config = store::load_config(&app).map_err(|e| e.to_string())?;
    Ok(config.profiles)
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
    
    let initial_len = config.profiles.len();
    config.profiles.retain(|p| p.id != id);
    
    if config.profiles.len() == initial_len {
        return Err("Profile not found".to_string());
    }
    
    // If we deleted the default profile, make the first remaining one default (if any)
    if config.profiles.iter().all(|p| !p.is_default) && !config.profiles.is_empty() {
        config.profiles[0].is_default = true;
    }
    
    store::save_config(&app, &config).map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
pub fn switch_profile_globally(app: AppHandle, id: String) -> Result<(), String> {
    let config = store::load_config(&app).map_err(|e| e.to_string())?;
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
    let output = Command::new("git")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to execute git command: {}", e))?;
        
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Git command failed: {}", stderr));
    }
    
    Ok(())
}
