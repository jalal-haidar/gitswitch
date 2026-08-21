use std::path::Path;
use std::process::Command;
use std::io::Write;

use crate::git::{self, no_window, GitScope, ProcessGitExecutor};
use serde::{Serialize, Deserialize};
use tauri::AppHandle;
use uuid::Uuid;

use crate::config::store;
use crate::models::{GitProfile, GitConfigSnapshot};

// Server-side validation/sanitization helpers
fn sanitize_string(s: &str, max_len: usize) -> String {
    let mut out = s.chars().filter(|c| !c.is_control()).collect::<String>();
    out.truncate(max_len);
    out.trim().to_string()
}

fn is_plausible_email(email: &str) -> bool {
    if email.len() < 5 || email.len() > 254 {
        return false;
    }

    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    // Reject multiple @ signs
    if domain.contains('@') {
        return false;
    }

    // Local part: at least 1 char, no leading/trailing dots, no consecutive dots
    if local.is_empty() || local.starts_with('.') || local.ends_with('.') || local.contains("..") {
        return false;
    }

    // Domain: must have at least one dot, valid structure
    if domain.is_empty()
        || !domain.contains('.')
        || domain.starts_with('.')
        || domain.ends_with('.')
        || domain.contains("..")
    {
        return false;
    }

    // Only allow safe characters
    let local_ok = local
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | '+'));
    let domain_ok = domain
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '.' | '-'));

    local_ok && domain_ok
}

/// Returns the current user's home directory, first expanding a leading `~`.
fn resolve_path(raw: &str) -> std::path::PathBuf {
    if raw.starts_with('~') {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_default();
        let stripped = raw.trim_start_matches('~').trim_start_matches(['/', '\\']);
        std::path::Path::new(&home).join(stripped)
    } else {
        std::path::PathBuf::from(raw)
    }
}

/// Returns the home directory path, or `None` if it cannot be determined.
fn user_home_dir() -> Option<std::path::PathBuf> {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
        .map(std::path::PathBuf::from)
}

fn validate_and_sanitize_profile(p: &mut GitProfile) -> Result<(), String> {
    // Limits chosen conservatively
    p.label = sanitize_string(&p.label, 100);
    p.name = sanitize_string(&p.name, 200);
    p.email = sanitize_string(&p.email, 254);
    p.color = sanitize_string(&p.color, 32);

    if let Some(ref ssh) = p.ssh_key_path.clone() {
        let raw = sanitize_string(ssh, 1024);
        if raw.is_empty() {
            p.ssh_key_path = None;
        } else {
            let resolved = resolve_path(&raw);
            // Security: SSH key must live inside the user's home directory
            match user_home_dir() {
                Some(home) => {
                    if !resolved.starts_with(&home) {
                        return Err(format!(
                            "SSH key path must be inside your home directory ({})",
                            home.display()
                        ));
                    }
                }
                None => {
                    return Err(
                        "Cannot determine home directory — SSH key path validation failed"
                            .to_string(),
                    );
                }
            }
            if !resolved.exists() {
                return Err(format!("SSH key file not found: {}", resolved.display()));
            }
            p.ssh_key_path = Some(resolved.to_string_lossy().into_owned());
        }
    }

    if let Some(ref mut gpg) = p.gpg_key_id.clone() {
        let s = sanitize_string(gpg, 128);
        if s.is_empty() {
            p.gpg_key_id = None;
        } else {
            p.gpg_key_id = Some(s);
        }
    }

    // Basic required fields
    if p.label.is_empty() {
        return Err("Profile label must not be empty".to_string());
    }
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

    // Assign a new ID if it's empty
    if profile.id.is_empty() {
        profile.id = Uuid::new_v4().to_string();
    }

    let saved = store::update_config(&app, |config| {
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
        Ok(profile.clone())
    })?;
    crate::tray::refresh_tray(&app);
    Ok(saved)
}

#[tauri::command]
pub fn update_profile(app: AppHandle, profile: GitProfile) -> Result<GitProfile, String> {
    // Validate and sanitize update payload
    let mut profile = profile;
    validate_and_sanitize_profile(&mut profile)?;

    let saved = store::update_config(&app, |config| {
        let mut found = false;
        for existing_profile in &mut config.profiles {
            if existing_profile.id == profile.id {
                existing_profile.label = profile.label.clone();
                existing_profile.name = profile.name.clone();
                existing_profile.email = profile.email.clone();
                existing_profile.color = profile.color.clone();
                existing_profile.ssh_key_path = profile.ssh_key_path.clone();
                existing_profile.gpg_key_id = profile.gpg_key_id.clone();
                if profile.is_default && !existing_profile.is_default {
                    existing_profile.is_default = true;
                }
                found = true;
            } else if profile.is_default {
                existing_profile.is_default = false;
            }
        }

        if !found {
            return Err(format!("Profile not found: {}", profile.id));
        }
        Ok(profile.clone())
    })?;
    crate::tray::refresh_tray(&app);
    Ok(saved)
}

#[tauri::command]
pub fn delete_profile(app: AppHandle, id: String) -> Result<(), String> {
    store::update_config(&app, |config| {
        if config.directory_rules.iter().any(|rule| rule.profile_id == id) {
            return Err("Cannot delete profile while it is referenced by directory rules".to_string());
        }

        let initial_len = config.profiles.len();
        config.profiles.retain(|p| p.id != id);
        if config.profiles.len() == initial_len {
            return Err(format!("Profile not found: {id}"));
        }

        if config.profiles.iter().all(|p| !p.is_default) && !config.profiles.is_empty() {
            config.profiles[0].is_default = true;
        }
        if config.active_profile_id.as_deref() == Some(id.as_str()) {
            config.active_profile_id = config.profiles.first().map(|p| p.id.clone());
        }
        Ok(())
    })?;
    crate::tray::refresh_tray(&app);
    Ok(())
}

#[tauri::command]
pub fn switch_profile_globally(app: AppHandle, id: String) -> Result<(), String> {
    let config = store::load_config(&app).map_err(|e| e.to_string())?;
    let profile = config.profiles.iter().find(|p| p.id == id)
        .ok_or_else(|| format!("Profile not found: {id}"))?;
        
    git::apply_profile(&ProcessGitExecutor::default(), &GitScope::Global, profile)?;

    store::update_config(&app, |config| {
        if !config.profiles.iter().any(|profile| profile.id == id) {
            return Err(format!("Profile not found: {id}"));
        }
        config.active_profile_id = Some(id.clone());
        Ok(())
    })?;
    crate::tray::refresh_tray(&app);
    Ok(())
}

pub fn switch_profile_for_repo(app: AppHandle, id: String, repo_path: &Path) -> Result<(), String> {
    let config = store::load_config(&app).map_err(|e| e.to_string())?;
    let profile = config
        .profiles
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("Profile not found: {id}"))?;

    // Capture a transient snapshot of repo-local git config before mutating it —
    // but only if there isn't already one (preserve the pre-switch baseline so
    // repeated rapid auto-switches don't wipe out the original values).
    let scope = GitScope::Local(repo_path.to_path_buf());
    let executor = ProcessGitExecutor::default();
    if !store::has_transient_snapshot(&repo_path.to_string_lossy()) {
        let snapshot = git::read_snapshot(&executor, &scope)?;
        store::set_transient_snapshot(&repo_path.to_string_lossy(), snapshot);
    }
    git::apply_profile(&executor, &scope, profile)?;

    store::update_config(&app, |config| {
        if !config.profiles.iter().any(|profile| profile.id == id) {
            return Err(format!("Profile not found: {id}"));
        }
        config.active_profile_id = Some(id.clone());
        Ok(())
    })?;
    crate::tray::refresh_tray(&app);
    Ok(())
}

#[tauri::command]
pub fn set_active_profile(app: AppHandle, id: String) -> Result<(), String> {
    store::update_config(&app, |config| {
        if !config.profiles.iter().any(|profile| profile.id == id) {
            return Err(format!("Profile not found: {id}"));
        }
        config.active_profile_id = Some(id.clone());
        Ok(())
    })?;
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

// NOTE: GitConfigSnapshot is defined in `models.rs` and imported above.

const EXPORT_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct ProfilesExport {
    version: u32,
    profiles: Vec<GitProfile>,
}

#[tauri::command]
pub fn export_profiles(app: AppHandle, path: String) -> Result<(), String> {
    // Validate export path is within the user's home directory
    let export_path = std::path::Path::new(&path);
    if let Some(home) = user_home_dir() {
        let parent = export_path.parent().unwrap_or(export_path);
        if !parent.starts_with(&home) {
            return Err("Export path must be inside your home directory".to_string());
        }
    }

    let config = store::load_config(&app).map_err(|e| e.to_string())?;
    let export = ProfilesExport {
        version: EXPORT_VERSION,
        profiles: config.profiles,
    };
    let json = serde_json::to_string_pretty(&export)
        .map_err(|e| format!("Serialization error: {e}"))?;
    let mut file = std::fs::File::create(&path)
        .map_err(|e| format!("Could not create file: {e}"))?;
    file.write_all(json.as_bytes())
        .map_err(|e| format!("Write error: {e}"))?;
    file.sync_all()
        .map_err(|e| format!("Sync error: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn import_profiles(app: AppHandle, path: String) -> Result<ImportResult, String> {
    let json = std::fs::read_to_string(&path)
        .map_err(|e| format!("Could not read file: {e}"))?;
    let export: ProfilesExport = serde_json::from_str(&json)
        .map_err(|_| "Invalid or unrecognised export file.".to_string())?;

    if export.version == 0 || export.version > EXPORT_VERSION {
        return Err(format!(
            "Unrecognised export version {}. Expected version {}.",
            export.version, EXPORT_VERSION
        ));
    }

    let result = store::update_config(&app, move |config| {
        let mut added = 0u32;
        let mut skipped = 0u32;

        for mut profile in export.profiles {
            profile.id = Uuid::new_v4().to_string();
            profile.is_default = false;
            if validate_and_sanitize_profile(&mut profile).is_err() {
                skipped += 1;
                continue;
            }

            let exists = config.profiles.iter().any(|p| {
                p.name.trim().to_lowercase() == profile.name.trim().to_lowercase()
                    && p.email.trim().to_lowercase() == profile.email.trim().to_lowercase()
            });
            if exists {
                skipped += 1;
                continue;
            }

            config.profiles.push(profile);
            added += 1;
        }

        Ok(ImportResult { added, skipped })
    })?;
    if result.added > 0 {
        crate::tray::refresh_tray(&app);
    }
    Ok(result)
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
    git::read_snapshot(&ProcessGitExecutor::default(), &GitScope::Global)
}

#[tauri::command]
pub fn restore_global_git_config(_app: AppHandle, snapshot: GitConfigSnapshot) -> Result<(), String> {
    restore_global_git_config_inner(snapshot)
}

pub fn restore_global_git_config_inner(snapshot: GitConfigSnapshot) -> Result<(), String> {
    git::restore_snapshot(
        &ProcessGitExecutor::default(),
        &GitScope::Global,
        &snapshot,
    )
}

/// Walk up from `path` until we find a directory that contains `.git`.
pub(crate) fn find_git_root(path: &Path) -> Option<std::path::PathBuf> {
    let mut current = path.to_path_buf();
    loop {
        if current.join(".git").exists() {
            return Some(current);
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return None,
        }
    }
}

/// Read a single key from a repo's *local* git config. Returns None if unset or on error.
/// Public so `auto_switch` can use it for the per-repo identity check.
pub(crate) fn read_local_git_config(repo_path: &Path, key: &str) -> Option<String> {
    git::read_value(
        &ProcessGitExecutor::default(),
        &GitScope::Local(repo_path.to_path_buf()),
        key,
    )
    .ok()
    .flatten()
}

/// Return value for `get_repo_local_config` — what is actually written
/// in this repository's `.git/config` right now.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoLocalConfig {
    pub user_name: Option<String>,
    pub user_email: Option<String>,
    pub user_signingkey: Option<String>,
    pub commit_gpgsign: Option<String>,
    pub core_ssh_command: Option<String>,
}

/// Tauri command: read the local git config of a repo and return the current values.
/// Used by the frontend to prove a profile switch actually landed in `.git/config`.
#[tauri::command]
pub fn get_repo_local_config(_app: AppHandle, repo_path: String) -> Result<RepoLocalConfig, String> {
    let path = Path::new(&repo_path);
    let git_root = find_git_root(path)
        .ok_or_else(|| format!("Not a git repository: {}", repo_path))?;

    let snapshot = git::read_snapshot(
        &ProcessGitExecutor::default(),
        &GitScope::Local(git_root),
    )?;
    Ok(RepoLocalConfig {
        user_name: snapshot.user_name,
        user_email: snapshot.user_email,
        user_signingkey: snapshot.user_signingkey,
        commit_gpgsign: snapshot.commit_gpgsign,
        core_ssh_command: snapshot.core_ssh_command,
    })
}

/// Tauri command: apply a profile to a specific repo directory.
/// Accepts any path inside the repo — walks up to find the .git root.
#[tauri::command]
pub fn apply_profile_to_repo(app: AppHandle, id: String, repo_path: String) -> Result<(), String> {
    let path = Path::new(&repo_path);
    if !path.exists() {
        return Err(format!("Path does not exist: {}", repo_path));
    }
    let git_root = find_git_root(path)
        .ok_or_else(|| format!("Not a git repository (or any parent directory): {}", repo_path))?;
    switch_profile_for_repo(app, id, &git_root)
}

#[tauri::command]
pub fn restore_repo_snapshot(app: AppHandle, repo_path: String) -> Result<(), String> {
    let path = Path::new(&repo_path);
    if !path.exists() {
        return Err(format!("Path does not exist: {}", repo_path));
    }
    let git_root = find_git_root(path)
        .ok_or_else(|| format!("Not a git repository (or any parent directory): {}", repo_path))?;

    // Take the transient snapshot (removes it from the store)
    let snap_opt = crate::config::store::take_transient_snapshot(&git_root.to_string_lossy());
    let snapshot = snap_opt.ok_or_else(|| "No transient snapshot found for this repository".to_string())?;

    git::restore_snapshot(
        &ProcessGitExecutor::default(),
        &GitScope::Local(git_root),
        &snapshot,
    )?;

    crate::tray::refresh_tray(&app);
    Ok(())
}

#[tauri::command]
pub fn has_repo_snapshot(_app: AppHandle, repo_path: String) -> Result<bool, String> {
    let path = Path::new(&repo_path);
    if !path.exists() {
        return Err(format!("Path does not exist: {}", repo_path));
    }
    let git_root = find_git_root(path)
        .ok_or_else(|| format!("Not a git repository (or any parent directory): {}", repo_path))?;

    Ok(crate::config::store::has_transient_snapshot(&git_root.to_string_lossy()))
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

    // Resolve and validate the key path is within the user's home directory
    let resolved_key = resolve_path(key_path.trim());
    match user_home_dir() {
        Some(home) => {
            if !resolved_key.starts_with(&home) {
                return Err("SSH key must be inside your home directory".to_string());
            }
        }
        None => {
            return Err(
                "Cannot determine home directory — SSH key path validation failed".to_string(),
            );
        }
    }

    if !resolved_key.exists() {
        return Err(format!("SSH key file not found: {}", key_path));
    }

    let key_path_str = resolved_key.to_string_lossy().to_string();

    // Validate and sanitize the host parameter
    let ssh_host = match host.as_deref() {
        Some(h) if !h.is_empty() => {
            let trimmed = h.trim();
            // Validate host format: must be a valid SSH destination (user@host or host)
            // Only allow alphanumeric, dots, hyphens, underscores, colons, and @
            if !trimmed
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | ':' | '@'))
            {
                return Err("Invalid host format — only alphanumeric characters, dots, hyphens, and @ are allowed".to_string());
            }
            if trimmed.len() > 253 {
                return Err("Host name is too long".to_string());
            }
            trimmed.to_string()
        }
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

    let mut ssh_cmd = Command::new("ssh");
    ssh_cmd.args(["-T", "-i", &key_path_str,
               "-o", "IdentitiesOnly=yes",
               "-o", "StrictHostKeyChecking=no",
               "-o", "BatchMode=yes",
               "-o", "ConnectTimeout=10",
               &ssh_host]);
    no_window(&mut ssh_cmd);
    let output = ssh_cmd.output()
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

    // GitHub: "Hi username! You've successfully authenticated, but GitHub does not provide shell access."
    // Older GitHub / some clients: "Hi username! You have successfully authenticated"
    // Also match the "does not provide shell access" variant which is the normal interactive response.
    let is_github_success = combined.contains("Hi ")
        && (combined.contains("successfully authenticated")
            || combined.contains("does not provide shell access"));
    if is_github_success {
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
    git::execute_checked(
        &ProcessGitExecutor::default(),
        &git::args(args),
        cwd,
    )
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

    // ── sanitize_string ──────────────────────────────────────────
    #[test]
    fn sanitize_removes_control_chars() {
        assert_eq!(sanitize_string("hello\x00world", 100), "helloworld");
        assert_eq!(sanitize_string("ab\x07cd\x1B", 100), "abcd");
    }

    #[test]
    fn sanitize_truncates_to_max_len() {
        assert_eq!(sanitize_string("abcdefgh", 5), "abcde");
    }

    #[test]
    fn sanitize_trims_whitespace() {
        assert_eq!(sanitize_string("  hello  ", 100), "hello");
    }

    #[test]
    fn sanitize_empty_input() {
        assert_eq!(sanitize_string("", 100), "");
    }

    // ── is_plausible_email ───────────────────────────────────────
    #[test]
    fn email_valid_simple() {
        assert!(is_plausible_email("user@example.com"));
        assert!(is_plausible_email("first.last@domain.co.uk"));
        assert!(is_plausible_email("user+tag@example.com"));
    }

    #[test]
    fn email_rejects_missing_at() {
        assert!(!is_plausible_email("userexample.com"));
    }

    #[test]
    fn email_rejects_double_at() {
        assert!(!is_plausible_email("user@@example.com"));
    }

    #[test]
    fn email_rejects_at_in_domain() {
        assert!(!is_plausible_email("user@ex@mple.com"));
    }

    #[test]
    fn email_rejects_leading_dot_local() {
        assert!(!is_plausible_email(".user@example.com"));
    }

    #[test]
    fn email_rejects_trailing_dot_local() {
        assert!(!is_plausible_email("user.@example.com"));
    }

    #[test]
    fn email_rejects_consecutive_dots_local() {
        assert!(!is_plausible_email("user..name@example.com"));
    }

    #[test]
    fn email_rejects_no_dot_in_domain() {
        assert!(!is_plausible_email("user@localhost"));
    }

    #[test]
    fn email_rejects_dot_start_domain() {
        assert!(!is_plausible_email("user@.example.com"));
    }

    #[test]
    fn email_rejects_consecutive_dots_domain() {
        assert!(!is_plausible_email("user@example..com"));
    }

    #[test]
    fn email_rejects_too_short() {
        assert!(!is_plausible_email("a@b"));
        assert!(!is_plausible_email(""));
    }

    #[test]
    fn email_rejects_too_long() {
        let long_local = "a".repeat(250);
        let email = format!("{}@example.com", long_local);
        assert!(email.len() > 254);
        assert!(!is_plausible_email(&email));
    }

    #[test]
    fn email_rejects_special_chars() {
        assert!(!is_plausible_email("user name@example.com"));
        assert!(!is_plausible_email("user<>@example.com"));
    }

    // ── resolve_path ─────────────────────────────────────────────
    #[test]
    fn resolve_path_absolute_unchanged() {
        let p = resolve_path("/some/path/to/key");
        assert_eq!(p, std::path::PathBuf::from("/some/path/to/key"));
    }

    #[test]
    fn resolve_path_tilde_expands() {
        let p = resolve_path("~/.ssh/id_ed25519");
        // Should not still start with ~
        assert!(!p.to_string_lossy().starts_with('~'));
        assert!(p.to_string_lossy().contains(".ssh"));
    }

    // ── validate_and_sanitize_profile ────────────────────────────
    #[test]
    fn validate_rejects_empty_label() {
        let mut profile = GitProfile {
            id: "test".to_string(),
            label: "".to_string(),
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            color: "#FF0000".to_string(),
            ssh_key_path: None,
            gpg_key_id: None,
            is_default: false,
            remote_url: None,
            remote_service: None,
        };
        let res = validate_and_sanitize_profile(&mut profile);
        assert!(res.is_err());
    }

    #[test]
    fn validate_rejects_bad_email() {
        let mut profile = GitProfile {
            id: "test".to_string(),
            label: "Test".to_string(),
            name: "Test".to_string(),
            email: "not-an-email".to_string(),
            color: "#FF0000".to_string(),
            ssh_key_path: None,
            gpg_key_id: None,
            is_default: false,
            remote_url: None,
            remote_service: None,
        };
        let res = validate_and_sanitize_profile(&mut profile);
        assert!(res.is_err());
    }

    #[test]
    fn validate_accepts_valid_profile() {
        let mut profile = GitProfile {
            id: "test".to_string(),
            label: "Work".to_string(),
            name: "John Doe".to_string(),
            email: "john@work.com".to_string(),
            color: "#6A5ACD".to_string(),
            ssh_key_path: None,
            gpg_key_id: None,
            is_default: false,
            remote_url: None,
            remote_service: None,
        };
        let res = validate_and_sanitize_profile(&mut profile);
        assert!(res.is_ok());
    }

    #[test]
    fn validate_sanitizes_long_label() {
        let long_label = "X".repeat(200);
        let mut profile = GitProfile {
            id: "test".to_string(),
            label: long_label,
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            color: "#FF0000".to_string(),
            ssh_key_path: None,
            gpg_key_id: None,
            is_default: false,
            remote_url: None,
            remote_service: None,
        };
        let res = validate_and_sanitize_profile(&mut profile);
        assert!(res.is_ok());
        assert_eq!(profile.label.len(), 100);
    }

    #[test]
    fn validate_clears_empty_ssh_key() {
        let mut profile = GitProfile {
            id: "test".to_string(),
            label: "Work".to_string(),
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            color: "#FF0000".to_string(),
            ssh_key_path: Some("   ".to_string()),
            gpg_key_id: None,
            is_default: false,
            remote_url: None,
            remote_service: None,
        };
        let res = validate_and_sanitize_profile(&mut profile);
        assert!(res.is_ok());
        assert!(profile.ssh_key_path.is_none());
    }

    // ── extract_github_username ──────────────────────────────────

    #[test]
    fn extract_username_standard() {
        let output = "Hi octocat! You've successfully authenticated, but GitHub does not provide shell access.";
        assert_eq!(extract_github_username(output).as_deref(), Some("octocat"));
    }

    #[test]
    fn extract_username_with_hyphens() {
        let output = "Hi my-user-name! You've successfully authenticated...";
        assert_eq!(extract_github_username(output).as_deref(), Some("my-user-name"));
    }

    #[test]
    fn extract_username_none_on_garbage() {
        assert!(extract_github_username("Permission denied (publickey).").is_none());
    }

    #[test]
    fn extract_username_none_on_empty() {
        assert!(extract_github_username("").is_none());
    }

    // ── find_git_root ────────────────────────────────────────────

    #[test]
    fn find_git_root_discovers_repo() {
        let tmp = std::env::temp_dir().join("gitswitch_test_find_root");
        let repo = tmp.join("project");
        let sub = repo.join("src").join("deep");
        let _ = std::fs::create_dir_all(sub.join(".keep")); // deep nested dir
        let _ = std::fs::create_dir_all(repo.join(".git"));

        let found = find_git_root(&sub);
        assert!(found.is_some(), "should find git root");
        let found = found.unwrap();
        assert!(found.ends_with("project"), "root should be project dir, got: {}", found.display());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn find_git_root_returns_none_at_filesystem_root() {
        // A path with no .git anywhere should return None eventually
        let result = find_git_root(std::path::Path::new("Z:\\nonexistent\\deep\\path"));
        assert!(result.is_none());
    }
}
