use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::git::{self, no_window, GitScope, ProcessGitExecutor};
use crate::path_security;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::config::{snapshots, store};
use crate::errors::BackendError;
use crate::models::{GitProfile, RepoApplyEvent, RepoApplySource};

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

fn normalize_profile_ssh_path(profile: &mut GitProfile) -> Result<(), String> {
    let Some(raw) = profile.ssh_key_path.clone() else {
        return Ok(());
    };
    let raw = sanitize_string(&raw, 1024);
    if raw.is_empty() {
        profile.ssh_key_path = None;
        return Ok(());
    }
    let canonical = path_security::canonical_ssh_key(&raw)?;
    profile.ssh_key_path = Some(canonical.to_string_lossy().into_owned());
    Ok(())
}

fn validate_and_sanitize_profile(p: &mut GitProfile) -> Result<(), String> {
    // Limits chosen conservatively
    p.label = sanitize_string(&p.label, 100);
    p.name = sanitize_string(&p.name, 200);
    p.email = sanitize_string(&p.email, 254);
    p.color = sanitize_string(&p.color, 32);

    normalize_profile_ssh_path(p)?;

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

pub(crate) fn verified_global_profile(app: &AppHandle) -> Option<GitProfile> {
    let config = store::load_config(app).ok()?;
    let snapshot = git::read_snapshot(&ProcessGitExecutor::default(), &GitScope::Global).ok()?;
    let profiles = config
        .profiles
        .into_iter()
        .map(|mut profile| {
            let _ = normalize_profile_ssh_path(&mut profile);
            profile
        })
        .collect::<Vec<_>>();
    git::unique_matching_profile(&profiles, &snapshot).cloned()
}

#[tauri::command]
pub fn get_global_active_profile_id(app: AppHandle) -> Result<Option<String>, String> {
    Ok(verified_global_profile(&app).map(|profile| profile.id))
}

pub(crate) fn migrate_legacy_active_state(app: &AppHandle) -> Result<(), String> {
    let config = store::load_config(app).map_err(|error| error.to_string())?;
    if config.legacy_active_profile_id.is_none() {
        return Ok(());
    }
    store::update_config(app, |config| {
        config.legacy_active_profile_id = None;
        Ok(())
    })
}

pub(crate) fn normalize_stored_profile_paths(app: &AppHandle) -> Result<(), String> {
    let config = store::load_config(app).map_err(|error| error.to_string())?;
    let replacements = config
        .profiles
        .iter()
        .filter_map(|profile| {
            let original = profile.ssh_key_path.clone()?;
            let mut normalized = profile.clone();
            normalize_profile_ssh_path(&mut normalized).ok()?;
            (normalized.ssh_key_path.as_ref() != Some(&original))
                .then_some((profile.id.clone(), normalized.ssh_key_path))
        })
        .collect::<Vec<_>>();
    if replacements.is_empty() {
        return Ok(());
    }
    store::update_config(app, |config| {
        for (profile_id, normalized_path) in &replacements {
            if let Some(profile) = config
                .profiles
                .iter_mut()
                .find(|profile| profile.id == *profile_id)
            {
                profile.ssh_key_path = normalized_path.clone();
            }
        }
        Ok(())
    })
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
        if config
            .directory_rules
            .iter()
            .any(|rule| rule.profile_id == id)
        {
            return Err(
                "Cannot delete profile while it is referenced by directory rules".to_string(),
            );
        }

        let initial_len = config.profiles.len();
        config.profiles.retain(|p| p.id != id);
        if config.profiles.len() == initial_len {
            return Err(format!("Profile not found: {id}"));
        }

        if config.profiles.iter().all(|p| !p.is_default) && !config.profiles.is_empty() {
            config.profiles[0].is_default = true;
        }
        Ok(())
    })?;
    crate::tray::refresh_tray(&app);
    Ok(())
}

#[tauri::command]
pub fn switch_profile_globally(app: AppHandle, id: String) -> Result<(), String> {
    let _transaction = git::transaction_guard();
    let config = store::load_config(&app).map_err(|e| e.to_string())?;
    let mut profile = config
        .profiles
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| format!("Profile not found: {id}"))?;
    normalize_profile_ssh_path(&mut profile)?;
    let executor = ProcessGitExecutor::default();
    let scope = GitScope::Global;
    let desired = git::snapshot_for_profile(&profile)?;
    git::preflight(&executor, &scope)?;
    let baseline = git::read_snapshot(&executor, &scope)?;
    let previous_snapshot = snapshots::swap_global(&app, Some(baseline.clone()))?;

    if let Err(operation_error) =
        git::apply_snapshot_transaction(&executor, &scope, "apply", &desired, &baseline)
    {
        return Err(compensate_snapshot_failure(
            operation_error,
            snapshots::swap_global(&app, previous_snapshot).err(),
        ));
    }

    crate::tray::refresh_tray(&app);
    let _ = app.emit("profiles-changed", ());
    Ok(())
}

pub fn switch_profile_for_repo(
    app: AppHandle,
    id: String,
    repo_path: &Path,
    source: RepoApplySource,
    rule_id: Option<&str>,
) -> Result<RepoApplyEvent, String> {
    let canonical_repo = find_git_root(repo_path).ok_or_else(|| {
        format!(
            "Not a git repository (or any parent directory): {}",
            repo_path.display()
        )
    })?;
    let _transaction = git::transaction_guard();
    let config = store::load_config(&app).map_err(|e| e.to_string())?;
    let mut profile = config
        .profiles
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| format!("Profile not found: {id}"))?;
    normalize_profile_ssh_path(&mut profile)?;
    let scope = GitScope::Local(canonical_repo.clone());
    let executor = ProcessGitExecutor::default();
    let desired = git::snapshot_for_profile(&profile)?;
    git::preflight(&executor, &scope)?;
    let baseline = git::read_snapshot(&executor, &scope)?;
    let previous_snapshot =
        snapshots::swap_repository(&app, &canonical_repo, Some(baseline.clone()))?;

    if let Err(operation_error) =
        git::apply_snapshot_transaction(&executor, &scope, "apply", &desired, &baseline)
    {
        return Err(compensate_snapshot_failure(
            operation_error,
            snapshots::swap_repository(&app, &canonical_repo, previous_snapshot).err(),
        ));
    }

    let occurred_at_epoch_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    let event = RepoApplyEvent {
        profile_id: profile.id.clone(),
        profile_label: profile.label.clone(),
        repository_path: canonical_repo.to_string_lossy().into_owned(),
        source,
        occurred_at_epoch_ms,
    };

    if let Err(operation_error) = store::update_config(&app, |config| {
        if !config.profiles.iter().any(|profile| profile.id == id) {
            return Err(format!("Profile not found: {id}"));
        }
        config.last_repo_activity = Some(event.clone());
        if let Some(rule_id) = rule_id {
            if let Some(rule) = config
                .directory_rules
                .iter_mut()
                .find(|rule| rule.id == rule_id)
            {
                rule.last_triggered_at = Some(occurred_at_epoch_ms);
            }
        }
        Ok(())
    }) {
        let rollback = collect_rollback_failures([
            git::rollback_to_snapshot(&executor, &scope, &baseline),
            snapshots::swap_repository(&app, &canonical_repo, previous_snapshot)
                .err()
                .map(|error| format!("snapshot rollback failed: {error}")),
        ]);
        return Err(BackendError::git_transaction("apply", operation_error, rollback).to_string());
    }
    Ok(event)
}

fn collect_rollback_failures<const N: usize>(failures: [Option<String>; N]) -> Option<String> {
    let failures: Vec<_> = failures.into_iter().flatten().collect();
    (!failures.is_empty()).then(|| failures.join("; "))
}

fn compensate_snapshot_failure(
    operation_error: String,
    snapshot_rollback: Option<String>,
) -> String {
    match snapshot_rollback {
        Some(snapshot_error) => BackendError::git_transaction(
            "apply",
            operation_error,
            Some(format!("snapshot rollback failed: {snapshot_error}")),
        )
        .to_string(),
        None => operation_error,
    }
}

#[tauri::command]
pub fn apply_identity(
    app: AppHandle,
    name: String,
    email: String,
    gpg_key: Option<String>,
) -> Result<(), String> {
    // Sanitize inputs
    let name = sanitize_string(&name, 200);
    let email = sanitize_string(&email, 254);

    if name.is_empty() {
        return Err("Identity name must not be empty".to_string());
    }
    if email.is_empty() || !is_plausible_email(&email) {
        return Err("Identity email is missing or invalid".to_string());
    }

    let _transaction = git::transaction_guard();
    let executor = ProcessGitExecutor::default();
    let scope = GitScope::Global;
    git::preflight(&executor, &scope)?;
    let baseline = git::read_snapshot(&executor, &scope)?;
    let mut desired = baseline.clone();
    desired.user_name = Some(name);
    desired.user_email = Some(email);
    desired.user_signingkey = gpg_key
        .as_deref()
        .map(|value| sanitize_string(value, 128))
        .filter(|value| !value.is_empty());
    desired.commit_gpgsign = Some(
        if desired.user_signingkey.is_some() {
            "true"
        } else {
            "false"
        }
        .to_string(),
    );

    let previous_snapshot = snapshots::swap_global(&app, Some(baseline.clone()))?;
    if let Err(operation_error) =
        git::apply_snapshot_transaction(&executor, &scope, "apply", &desired, &baseline)
    {
        return Err(compensate_snapshot_failure(
            operation_error,
            snapshots::swap_global(&app, previous_snapshot).err(),
        ));
    }
    crate::tray::refresh_tray(&app);
    let _ = app.emit("profiles-changed", ());
    Ok(())
}

const EXPORT_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct ProfilesExport {
    version: u32,
    profiles: Vec<GitProfile>,
}

#[tauri::command]
pub fn export_profiles(app: AppHandle, path: String) -> Result<(), String> {
    let export_path = path_security::canonical_export_target(&path)?;

    let config = store::load_config(&app).map_err(|e| e.to_string())?;
    let export = ProfilesExport {
        version: EXPORT_VERSION,
        profiles: config.profiles,
    };
    let json =
        serde_json::to_string_pretty(&export).map_err(|e| format!("Serialization error: {e}"))?;
    let mut file = std::fs::File::create(&export_path)
        .map_err(|e| format!("Could not create export file: {e}"))?;
    file.write_all(json.as_bytes())
        .map_err(|e| format!("Write error: {e}"))?;
    file.sync_all().map_err(|e| format!("Sync error: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn import_profiles(app: AppHandle, path: String) -> Result<ImportResult, String> {
    let json = std::fs::read_to_string(&path).map_err(|e| format!("Could not read file: {e}"))?;
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
pub fn has_global_snapshot(app: AppHandle) -> Result<bool, String> {
    Ok(snapshots::global(&app)?.is_some())
}

#[tauri::command]
pub fn get_last_repo_activity(app: AppHandle) -> Result<Option<RepoApplyEvent>, String> {
    Ok(store::load_config(&app)
        .map_err(|error| error.to_string())?
        .last_repo_activity)
}

#[tauri::command]
pub fn restore_global_snapshot(app: AppHandle) -> Result<(), String> {
    let _transaction = git::transaction_guard();
    let saved = snapshots::global(&app)?
        .ok_or_else(|| "No durable global Git snapshot is available".to_string())?;
    let executor = ProcessGitExecutor::default();
    let scope = GitScope::Global;
    git::preflight(&executor, &scope)?;
    let current = git::read_snapshot(&executor, &scope)?;
    git::apply_snapshot_transaction(&executor, &scope, "restore", &saved, &current)?;

    if let Err(operation_error) = snapshots::swap_global(&app, None) {
        let rollback = git::rollback_to_snapshot(&executor, &scope, &current);
        return Err(
            BackendError::git_transaction("restore", operation_error, rollback).to_string(),
        );
    }

    crate::tray::refresh_tray(&app);
    let _ = app.emit("profiles-changed", ());
    Ok(())
}

#[tauri::command]
pub fn discard_global_snapshot(app: AppHandle) -> Result<(), String> {
    let _transaction = git::transaction_guard();
    snapshots::swap_global(&app, None)?;
    Ok(())
}

/// Walk up from `path` until we find a directory that contains `.git`.
pub(crate) fn find_git_root(path: &Path) -> Option<std::path::PathBuf> {
    let canonical = path_security::canonicalize_existing(path, "repository path").ok()?;
    let mut current = if canonical.is_file() {
        canonical.parent()?.to_path_buf()
    } else {
        canonical
    };
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
    pub applied_profile_id: Option<String>,
}

/// Tauri command: read the local git config of a repo and return the current values.
/// Used by the frontend to prove a profile switch actually landed in `.git/config`.
#[tauri::command]
pub fn get_repo_local_config(app: AppHandle, repo_path: String) -> Result<RepoLocalConfig, String> {
    let path = Path::new(&repo_path);
    let git_root =
        find_git_root(path).ok_or_else(|| format!("Not a git repository: {}", repo_path))?;

    let snapshot = git::read_snapshot(&ProcessGitExecutor::default(), &GitScope::Local(git_root))?;
    let config = store::load_config(&app).map_err(|error| error.to_string())?;
    let applied_profile_id =
        git::unique_matching_profile(&config.profiles, &snapshot).map(|profile| profile.id.clone());
    Ok(RepoLocalConfig {
        user_name: snapshot.user_name,
        user_email: snapshot.user_email,
        user_signingkey: snapshot.user_signingkey,
        commit_gpgsign: snapshot.commit_gpgsign,
        core_ssh_command: snapshot.core_ssh_command,
        applied_profile_id,
    })
}

/// Tauri command: apply a profile to a specific repo directory.
/// Accepts any path inside the repo — walks up to find the .git root.
#[tauri::command]
pub fn apply_profile_to_repo(
    app: AppHandle,
    id: String,
    repo_path: String,
) -> Result<RepoApplyEvent, String> {
    let path = Path::new(&repo_path);
    if !path.exists() {
        return Err(format!("Path does not exist: {}", repo_path));
    }
    let git_root = find_git_root(path).ok_or_else(|| {
        format!(
            "Not a git repository (or any parent directory): {}",
            repo_path
        )
    })?;
    let event = switch_profile_for_repo(app.clone(), id, &git_root, RepoApplySource::Manual, None)?;
    let _ = app.emit("repo-profile-applied", event.clone());
    Ok(event)
}

#[tauri::command]
pub fn restore_repo_snapshot(app: AppHandle, repo_path: String) -> Result<(), String> {
    let path = Path::new(&repo_path);
    if !path.exists() {
        return Err(format!("Path does not exist: {}", repo_path));
    }
    let git_root = find_git_root(path).ok_or_else(|| {
        format!(
            "Not a git repository (or any parent directory): {}",
            repo_path
        )
    })?;

    let _transaction = git::transaction_guard();
    let snapshot = snapshots::repository(&app, &git_root)?
        .ok_or_else(|| "No durable snapshot found for this repository".to_string())?;
    let executor = ProcessGitExecutor::default();
    let scope = GitScope::Local(git_root.clone());
    git::preflight(&executor, &scope)?;
    let current = git::read_snapshot(&executor, &scope)?;
    git::apply_snapshot_transaction(&executor, &scope, "restore", &snapshot, &current)?;

    if let Err(operation_error) = snapshots::swap_repository(&app, &git_root, None) {
        let rollback = git::rollback_to_snapshot(&executor, &scope, &current);
        return Err(
            BackendError::git_transaction("restore", operation_error, rollback).to_string(),
        );
    }

    Ok(())
}

#[tauri::command]
pub fn has_repo_snapshot(app: AppHandle, repo_path: String) -> Result<bool, String> {
    let path = Path::new(&repo_path);
    if !path.exists() {
        return Err(format!("Path does not exist: {}", repo_path));
    }
    let git_root = find_git_root(path).ok_or_else(|| {
        format!(
            "Not a git repository (or any parent directory): {}",
            repo_path
        )
    })?;

    Ok(snapshots::repository(&app, &git_root)?.is_some())
}

#[tauri::command]
pub fn discard_repo_snapshot(app: AppHandle, repo_path: String) -> Result<(), String> {
    let path = Path::new(&repo_path);
    if !path.exists() {
        return Err(format!("Path does not exist: {repo_path}"));
    }
    let git_root = find_git_root(path)
        .ok_or_else(|| format!("Not a git repository (or any parent directory): {repo_path}"))?;
    let _transaction = git::transaction_guard();
    snapshots::swap_repository(&app, &git_root, None)?;
    Ok(())
}

#[derive(Serialize)]
pub struct SshTestResult {
    pub success: bool,
    pub username: Option<String>,
    pub message: String,
}

fn extract_github_username(output: &str) -> Option<String> {
    output
        .split("Hi ")
        .nth(1)
        .and_then(|s| s.split('!').next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn ssh_test_args(key_path: &str) -> Vec<String> {
    vec![
        "-T".to_string(),
        "-i".to_string(),
        key_path.to_string(),
        "-o".to_string(),
        "IdentitiesOnly=yes".to_string(),
        "-o".to_string(),
        "StrictHostKeyChecking=yes".to_string(),
        "-o".to_string(),
        "UpdateHostKeys=no".to_string(),
        "-o".to_string(),
        "BatchMode=yes".to_string(),
        "-o".to_string(),
        "ConnectTimeout=10".to_string(),
        "git@github.com".to_string(),
    ]
}

fn ssh_host_verification_failure(output: &str) -> Option<String> {
    let output = output.to_lowercase();
    if output.contains("remote host identification has changed")
        || output.contains("offending") && output.contains("host key")
    {
        return Some("GitHub's SSH host key differs from your known_hosts entry. Verify the published GitHub fingerprint and repair known_hosts outside GitSwitch before retrying.".to_string());
    }
    if output.contains("host key verification failed")
        || output.contains("host key is known for")
        || output.contains("authenticity of host")
    {
        return Some("GitHub is not trusted in your OpenSSH known_hosts file. Verify GitHub's published fingerprint, connect once with OpenSSH in a terminal, then retry.".to_string());
    }
    None
}

#[tauri::command]
pub fn test_ssh_connection(key_path: String) -> Result<SshTestResult, String> {
    if key_path.trim().is_empty() {
        return Err("SSH key path is required".to_string());
    }

    let resolved_key = path_security::canonical_ssh_key(key_path.trim())?;
    let key_path_str = resolved_key.to_string_lossy().to_string();
    let mut ssh_cmd = Command::new("ssh");
    ssh_cmd.args(ssh_test_args(&key_path_str));
    no_window(&mut ssh_cmd);
    let output = ssh_cmd.output().map_err(|e| {
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
                "Connected to GitHub as {}",
                username.as_deref().unwrap_or("unknown")
            ),
        });
    }

    if let Some(message) = ssh_host_verification_failure(&combined) {
        return Ok(SshTestResult {
            success: false,
            username: None,
            message,
        });
    }

    let combined_lower = combined.to_lowercase();
    if combined_lower.contains("permission denied") || combined_lower.contains("publickey") {
        return Ok(SshTestResult {
            success: false,
            username: None,
            message:
                "Authentication failed — make sure this SSH key is added to your GitHub account"
                    .to_string(),
        });
    }

    if combined_lower.contains("connection refused")
        || combined_lower.contains("no route to host")
        || combined_lower.contains("timed out")
    {
        return Ok(SshTestResult {
            success: false,
            username: None,
            message: "Could not reach GitHub — check your network connection".to_string(),
        });
    }

    Ok(SshTestResult {
        success: false,
        username: None,
        message: if combined.trim().is_empty() {
            "No response from GitHub".to_string()
        } else {
            combined.trim().to_string()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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

    // ── SSH command policy ───────────────────────────────────────
    #[test]
    fn ssh_test_requires_verified_system_known_hosts() {
        let args = ssh_test_args("/home/alice/.ssh/id_ed25519");
        assert!(args.iter().any(|arg| arg == "StrictHostKeyChecking=yes"));
        assert!(args.iter().any(|arg| arg == "UpdateHostKeys=no"));
        assert!(!args
            .iter()
            .any(|arg| arg.eq_ignore_ascii_case("StrictHostKeyChecking=no")));
        assert_eq!(args.last().map(String::as_str), Some("git@github.com"));
    }

    #[test]
    fn ssh_host_verification_failures_are_actionable_and_distinct() {
        let unknown = ssh_host_verification_failure(
            "No ED25519 host key is known for github.com and you have requested strict checking. Host key verification failed.",
        )
        .unwrap();
        let changed = ssh_host_verification_failure(
            "WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED! Offending ED25519 key",
        )
        .unwrap();

        assert!(unknown.contains("not trusted"));
        assert!(unknown.contains("OpenSSH"));
        assert!(changed.contains("differs"));
        assert!(changed.contains("outside GitSwitch"));
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
        assert_eq!(
            extract_github_username(output).as_deref(),
            Some("my-user-name")
        );
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
        assert!(
            found.ends_with("project"),
            "root should be project dir, got: {}",
            found.display()
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn find_git_root_returns_none_at_filesystem_root() {
        // A path with no .git anywhere should return None eventually
        let result = find_git_root(std::path::Path::new("Z:\\nonexistent\\deep\\path"));
        assert!(result.is_none());
    }
}
