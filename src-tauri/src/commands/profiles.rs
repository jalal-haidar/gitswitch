use std::path::Path;
use std::process::Command;
use std::io::Write;
use serde::{Serialize, Deserialize};
use tauri::AppHandle;
use uuid::Uuid;

use crate::config::store;
use crate::models::GitProfile;
use crate::errors::BackendError;

// Server-side validation/sanitization helpers
fn sanitize_string(s: &str, max_len: usize) -> String {
    let mut out = s.chars().filter(|c| !c.is_control()).collect::<String>();
    out.truncate(max_len);
    out.trim().to_string()
}

fn is_plausible_email(email: &str) -> bool {
    // Basic check: has '@' and a '.' after it, and reasonable length
    if email.len() < 3 || email.len() > 254 {
        return false;
    }
    let bytes = email.as_bytes();
    if let Some(at_pos) = bytes.iter().position(|&b| b == b'@') {
        // require a dot after '@'
        return bytes.iter().skip(at_pos + 1).any(|&b| b == b'.');
    }
    false
}

fn validate_and_sanitize_profile(p: &mut GitProfile) -> Result<(), String> {
    // Limits chosen conservatively
    p.label = sanitize_string(&p.label, 100);
    p.name = sanitize_string(&p.name, 200);
    p.email = sanitize_string(&p.email, 254);
    p.color = sanitize_string(&p.color, 32);

    if let Some(ref ssh) = p.ssh_key_path.clone() {
        let s = sanitize_string(ssh, 1024);
        if s.is_empty() {
            p.ssh_key_path = None;
        } else {
            // Verify the key file actually exists
            if !std::path::Path::new(&s).exists() {
                return Err(format!("SSH key file not found: {}", s));
            }
            p.ssh_key_path = Some(s);
        }
    }

    if let Some(ref mut gpg) = p.gpg_key_id.clone() {
        let s = sanitize_string(&gpg, 128);
        if s.is_empty() {
            p.gpg_key_id = None;
        } else {
            p.gpg_key_id = Some(s);
        }
    }

    // Basic required fields
    if p.name.is_empty() {
        return Err("Profile name must not be empty".to_string());
    }

    if p.email.is_empty() || !is_plausible_email(&p.email) {
        return Err("Profile email is missing or invalid".to_string());
    }

    Ok(())
}

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
    // sanitize and validate incoming profile fields
    validate_and_sanitize_profile(&mut profile)?;

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
    crate::tray::refresh_tray(&app);
    Ok(profile)
}

#[tauri::command]
pub fn update_profile(app: AppHandle, profile: GitProfile) -> Result<GitProfile, String> {
    // Validate and sanitize update payload
    let mut profile = profile;
    validate_and_sanitize_profile(&mut profile)?;

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
    crate::tray::refresh_tray(&app);
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
    crate::tray::refresh_tray(&app);
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

    // Apply SSH key if configured
    match profile.ssh_key_path.as_deref() {
        Some(ssh_path) if !ssh_path.is_empty() => {
            let normalized = ssh_path.replace('\\', "/");
            let ssh_cmd = format!("ssh -i \"{}\" -o IdentitiesOnly=yes", normalized);
            execute_git_command(vec!["config", "--global", "core.sshCommand", &ssh_cmd])?;
        }
        _ => {
            execute_git_command(vec!["config", "--global", "--unset", "core.sshCommand"]).ok();
        }
    }

    config.active_profile_id = Some(id);
    store::save_config(&app, &config).map_err(|e| e.to_string())?;
    crate::tray::refresh_tray(&app);
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

    // Apply SSH key per-repo if configured
    match profile.ssh_key_path.as_deref() {
        Some(ssh_path) if !ssh_path.is_empty() => {
            let normalized = ssh_path.replace('\\', "/");
            let ssh_cmd = format!("ssh -i \"{}\" -o IdentitiesOnly=yes", normalized);
            execute_git_command_in_dir(vec!["config", "--local", "core.sshCommand", &ssh_cmd], Some(repo_path))?;
        }
        _ => {
            execute_git_command_in_dir(vec!["config", "--local", "--unset", "core.sshCommand"], Some(repo_path)).ok();
        }
    }

    config.active_profile_id = Some(id);
    store::save_config(&app, &config).map_err(|e| e.to_string())?;
    crate::tray::refresh_tray(&app);
    Ok(())
}

#[tauri::command]
pub fn set_active_profile(app: AppHandle, id: String) -> Result<(), String> {
    let mut config = store::load_config(&app).map_err(|e| e.to_string())?;
    // ensure profile exists
    let exists = config.profiles.iter().any(|p| p.id == id);
    if !exists {
        return Err("Profile not found".to_string());
    }
    config.active_profile_id = Some(id);
    store::save_config(&app, &config).map_err(|e| e.to_string())?;
    crate::tray::refresh_tray(&app);
    Ok(())
}

#[tauri::command]
pub fn apply_identity(_app: AppHandle, name: String, email: String, gpg_key: Option<String>) -> Result<(), String> {
    // Sanitize inputs
    let name = sanitize_string(&name, 200);
    let email = sanitize_string(&email, 254);

    if name.is_empty() {
        return Err("Identity name must not be empty".to_string());
    }
    if email.is_empty() || !is_plausible_email(&email) {
        return Err("Identity email is missing or invalid".to_string());
    }

    // Apply the given identity directly to global git config
    execute_git_command(vec!["config", "--global", "user.name", &name])?;
    execute_git_command(vec!["config", "--global", "user.email", &email])?;

    if let Some(ref gpg) = gpg_key {
        let gpg = sanitize_string(gpg, 128);
        if !gpg.is_empty() {
            execute_git_command(vec!["config", "--global", "user.signingkey", &gpg])?;
            execute_git_command(vec!["config", "--global", "commit.gpgsign", "true"]).ok();
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

#[derive(Serialize, Deserialize)]
pub struct GitConfigSnapshot {
    pub user_name: Option<String>,
    pub user_email: Option<String>,
    pub user_signingkey: Option<String>,
    pub commit_gpgsign: Option<String>,
}

fn capture_git_config_value(args: Vec<&str>) -> Result<Option<String>, String> {
    let mut command = Command::new("git");
    command.args(&args);

    let output = command.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            BackendError::git_not_found().to_string()
        } else {
            BackendError::io_error(format!("Failed to execute git command: {}", e)).to_string()
        }
    })?;

    if !output.status.success() {
        // If value isn't set, git returns non-zero; treat as None
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(Some(stdout.trim().to_string()))
}

#[derive(Serialize, Deserialize)]
struct ProfilesExport {
    version: u32,
    profiles: Vec<GitProfile>,
}

#[tauri::command]
pub fn export_profiles(app: AppHandle, path: String) -> Result<(), String> {
    let config = store::load_config(&app).map_err(|e| e.to_string())?;
    let export = ProfilesExport {
        version: 1,
        profiles: config.profiles,
    };
    let json = serde_json::to_string_pretty(&export)
        .map_err(|e| format!("Serialization error: {e}"))?;
    let mut file = std::fs::File::create(&path)
        .map_err(|e| format!("Could not create file: {e}"))?;
    file.write_all(json.as_bytes())
        .map_err(|e| format!("Write error: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn import_profiles(app: AppHandle, path: String) -> Result<ImportResult, String> {
    let json = std::fs::read_to_string(&path)
        .map_err(|e| format!("Could not read file: {e}"))?;
    let export: ProfilesExport = serde_json::from_str(&json)
        .map_err(|_| "Invalid or unrecognised export file.".to_string())?;

    let mut config = store::load_config(&app).map_err(|e| e.to_string())?;
    let mut added = 0u32;
    let mut skipped = 0u32;

    for mut profile in export.profiles {
        // Check duplicate by name + email (case-insensitive)
        let exists = config.profiles.iter().any(|p| {
            p.name.trim().to_lowercase() == profile.name.trim().to_lowercase()
                && p.email.trim().to_lowercase() == profile.email.trim().to_lowercase()
        });
        if exists {
            skipped += 1;
            continue;
        }
        // Always assign a fresh id to avoid collisions
        profile.id = Uuid::new_v4().to_string();
        profile.is_default = false;
        config.profiles.push(profile);
        added += 1;
    }

    store::save_config(&app, &config).map_err(|e| e.to_string())?;
    if added > 0 {
        crate::tray::refresh_tray(&app);
    }
    Ok(ImportResult { added, skipped })
}

#[derive(Serialize)]
pub struct ImportResult {
    added: u32,
    skipped: u32,
}

#[tauri::command]
pub fn snapshot_global_git_config(_app: AppHandle) -> Result<GitConfigSnapshot, String> {
    snapshot_global_git_config_inner()
}

pub fn snapshot_global_git_config_inner() -> Result<GitConfigSnapshot, String> {
    let name = capture_git_config_value(vec!["config", "--global", "--get", "user.name"])?;
    let email = capture_git_config_value(vec!["config", "--global", "--get", "user.email"])?;
    let signing = capture_git_config_value(vec!["config", "--global", "--get", "user.signingkey"])?;
    let gpgsign = capture_git_config_value(vec!["config", "--global", "--get", "commit.gpgsign"])?;

    Ok(GitConfigSnapshot {
        user_name: name,
        user_email: email,
        user_signingkey: signing,
        commit_gpgsign: gpgsign,
    })
}

#[tauri::command]
pub fn restore_global_git_config(_app: AppHandle, snapshot: GitConfigSnapshot) -> Result<(), String> {
    restore_global_git_config_inner(snapshot)
}

pub fn restore_global_git_config_inner(snapshot: GitConfigSnapshot) -> Result<(), String> {
    // name
    if let Some(name) = snapshot.user_name {
        execute_git_command(vec!["config", "--global", "user.name", &name])?;
    } else {
        execute_git_command(vec!["config", "--global", "--unset", "user.name"]).ok();
    }

    // email
    if let Some(email) = snapshot.user_email {
        execute_git_command(vec!["config", "--global", "user.email", &email])?;
    } else {
        execute_git_command(vec!["config", "--global", "--unset", "user.email"]).ok();
    }

    // signing key
    if let Some(gpg) = snapshot.user_signingkey {
        if !gpg.is_empty() {
            execute_git_command(vec!["config", "--global", "user.signingkey", &gpg])?;
            execute_git_command(vec!["config", "--global", "commit.gpgsign", "true"]).ok();
        }
    } else {
        execute_git_command(vec!["config", "--global", "--unset", "user.signingkey"]).ok();
        execute_git_command(vec!["config", "--global", "commit.gpgsign", "false"]).ok();
    }

    if let Some(gpgsign) = snapshot.commit_gpgsign {
        // attempt to set to the snapshot value (true/false)
        execute_git_command(vec!["config", "--global", "commit.gpgsign", &gpgsign])?;
    }

    Ok(())
}

#[derive(Serialize)]
pub struct SshTestResult {
    pub success: bool,
    pub username: Option<String>,
    pub message: String,
}

fn extract_github_username(output: &str) -> Option<String> {
    output.split("Hi ").nth(1)
        .and_then(|s| s.split('!').next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[tauri::command]
pub fn test_ssh_connection(key_path: String, host: Option<String>) -> Result<SshTestResult, String> {
    if key_path.trim().is_empty() {
        return Err("SSH key path is required".to_string());
    }

    if !std::path::Path::new(&key_path).exists() {
        return Err(format!("SSH key file not found: {}", key_path));
    }

    let ssh_host = match host.as_deref() {
        Some(h) if !h.is_empty() => h.to_string(),
        _ => "git@github.com".to_string(),
    };

    let service = if ssh_host.contains("github.com") {
        "GitHub"
    } else if ssh_host.contains("gitlab.com") {
        "GitLab"
    } else if ssh_host.contains("bitbucket.org") {
        "Bitbucket"
    } else {
        "Git host"
    };

    let output = Command::new("ssh")
        .args(["-T", "-i", &key_path,
               "-o", "IdentitiesOnly=yes",
               "-o", "StrictHostKeyChecking=no",
               "-o", "BatchMode=yes",
               "-o", "ConnectTimeout=10",
               &ssh_host])
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "ssh executable not found — install OpenSSH or Git for Windows".to_string()
            } else {
                format!("Failed to run ssh: {}", e)
            }
        })?;

    // GitHub/GitLab respond on stderr; combine both streams
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let combined = format!("{}{}", stderr, stdout);

    // GitHub: "Hi username! You have successfully authenticated"
    // GitLab: "Welcome to GitLab, @username!"
    if combined.contains("Hi ") && combined.contains("! You have successfully") {
        let username = extract_github_username(&combined);
        return Ok(SshTestResult {
            success: true,
            username: username.clone(),
            message: format!(
                "Connected to {} as {}",
                service,
                username.as_deref().unwrap_or("unknown")
            ),
        });
    }

    if combined.contains("Welcome to GitLab") {
        let username = combined.split('@').nth(1)
            .and_then(|s| s.split('!').next())
            .map(|s| s.trim().to_string());
        return Ok(SshTestResult {
            success: true,
            username: username.clone(),
            message: format!(
                "Connected to {} as {}",
                service,
                username.as_deref().unwrap_or("unknown")
            ),
        });
    }

    let combined_lower = combined.to_lowercase();
    if combined_lower.contains("permission denied") || combined_lower.contains("publickey") {
        return Ok(SshTestResult {
            success: false,
            username: None,
            message: format!(
                "Authentication failed — make sure this SSH key is added to your {} account",
                service
            ),
        });
    }

    if combined_lower.contains("connection refused") || combined_lower.contains("no route to host") || combined_lower.contains("timed out") {
        return Ok(SshTestResult {
            success: false,
            username: None,
            message: format!("Could not reach {} — check your network connection", service),
        });
    }

    Ok(SshTestResult {
        success: false,
        username: None,
        message: if combined.trim().is_empty() {
            format!("No response from {}", service)
        } else {
            combined.trim().to_string()
        },
    })
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

    #[test]
    fn snapshot_and_restore_roundtrip() {
        // Snapshot current global git config and immediately restore it.
        // This should succeed and leave the user's global config unchanged.
        let snap = snapshot_global_git_config_inner().expect("snapshot failed");
        let res = restore_global_git_config_inner(snap);
        assert!(res.is_ok(), "restore failed: {:?}", res);
    }
}
