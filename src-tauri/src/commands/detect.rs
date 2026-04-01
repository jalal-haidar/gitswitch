use tauri::AppHandle;
use std::{process::Command, fs, env, path::{Path, PathBuf}};
use crate::errors::BackendError;
use crate::git::no_window;
use uuid::Uuid;

use crate::models::{GitProfile, ScannedRepo};

/// Classify a remote URL into a known service name.
pub fn detect_remote_service(url: &str) -> String {
    let lower = url.to_lowercase();
    if lower.contains("github.com") {
        "github".to_string()
    } else if lower.contains("gitlab.com") || lower.contains("gitlab.") {
        "gitlab".to_string()
    } else if lower.contains("bitbucket.org") {
        "bitbucket".to_string()
    } else {
        "other".to_string()
    }
}

/// Run a single `git config` command in `dir`, returning trimmed stdout or None.
fn git_config_in_dir(dir: &Path, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(dir);
    no_window(&mut cmd);
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

#[tauri::command]
pub fn detect_identities(_app: AppHandle, directory: Option<String>) -> Result<Vec<GitProfile>, String> {
    // If a directory is provided, run git commands there; otherwise use current dir
    let dir = directory
        .or_else(|| env::var("PWD").ok())
        .unwrap_or_else(|| String::new());
    let path = if dir.is_empty() { Path::new(".") } else { Path::new(&dir) };

    // Helper to run git and capture stdout as trimmed string, returning detailed error on failure
    let run_git = |args: &[&str]| -> Result<Option<String>, BackendError> {
        let mut cmd = Command::new("git");
        cmd.args(args).current_dir(path);
        no_window(&mut cmd);
        let output = cmd.output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                BackendError::git_not_found()
            } else {
                BackendError::io_error(format!("failed to spawn git: {}", e))
            }
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

            // `git config --get ...` exits non-zero when value is unset.
            // Treat this as "no value" instead of a hard error.
            let is_get_lookup = args.iter().any(|arg| *arg == "--get");
            if is_get_lookup && stderr.is_empty() {
                return Ok(None);
            }

            // Map permission-related errors
            let stderr_l = stderr.to_lowercase();
            if stderr_l.contains("permission denied") || stderr_l.contains("cannot open") {
                return Err(BackendError::permission_denied(stderr));
            }
            return Err(BackendError::git_failed(stderr));
        }

        let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if s.is_empty() {
            Ok(None)
        } else {
            Ok(Some(s))
        }
    };

    // Try local (repo) values first, fall back to global
    // Try local (repo) values first, fall back to global
    let name = match run_git(&["config", "user.name"]) {
        Ok(Some(v)) => v,
        Ok(None) => match run_git(&["config", "--global", "--get", "user.name"]) {
            Ok(Some(v)) => v,
            Ok(None) => String::new(),
            Err(e) => return Err(e.to_string()),
        },
        Err(e) => {
            // If git executable missing or permission issues, abort.
            match e.kind {
                crate::errors::BackendErrorKind::GitNotFound
                | crate::errors::BackendErrorKind::PermissionDenied => return Err(e.to_string()),
                _ => {
                    // Non-fatal git failure (e.g., not a git repository) — try global
                    match run_git(&["config", "--global", "--get", "user.name"]) {
                        Ok(Some(v)) => v,
                        Ok(None) => String::new(),
                        Err(e2) => return Err(e2.to_string()),
                    }
                }
            }
        }
    };

    let email = match run_git(&["config", "user.email"]) {
        Ok(Some(v)) => v,
        Ok(None) => match run_git(&["config", "--global", "--get", "user.email"]) {
            Ok(Some(v)) => v,
            Ok(None) => String::new(),
            Err(e) => return Err(e.to_string()),
        },
        Err(e) => {
            match e.kind {
                crate::errors::BackendErrorKind::GitNotFound
                | crate::errors::BackendErrorKind::PermissionDenied => return Err(e.to_string()),
                _ => match run_git(&["config", "--global", "--get", "user.email"]) {
                    Ok(Some(v)) => v,
                    Ok(None) => String::new(),
                    Err(e2) => return Err(e2.to_string()),
                },
            }
        }
    };

    let signingkey = match run_git(&["config", "user.signingkey"]) {
        Ok(Some(v)) => v,
        Ok(None) => match run_git(&["config", "--global", "--get", "user.signingkey"]) {
            Ok(Some(v)) => v,
            Ok(None) => String::new(),
            Err(e) => return Err(e.to_string()),
        },
        Err(e) => {
            match e.kind {
                crate::errors::BackendErrorKind::GitNotFound
                | crate::errors::BackendErrorKind::PermissionDenied => return Err(e.to_string()),
                _ => match run_git(&["config", "--global", "--get", "user.signingkey"]) {
                    Ok(Some(v)) => v,
                    Ok(None) => String::new(),
                    Err(e2) => return Err(e2.to_string()),
                },
            }
        }
    };

    // Also detect remote.origin.url (best-effort; no fallback to global)
    let remote_url = run_git(&["config", "--get", "remote.origin.url"]).ok().flatten();
    let remote_service = remote_url.as_deref().map(detect_remote_service);

    // Detect simple SSH key presence in ~/.ssh (look for common private key names)
    let home = env::var("HOME").or_else(|_| env::var("USERPROFILE")).unwrap_or_default();
    let mut ssh_key_path: Option<String> = None;
    if !home.is_empty() {
        let ssh_dir = Path::new(&home).join(".ssh");
        if ssh_dir.exists() && ssh_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(&ssh_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if let Some(fname) = p.file_name().and_then(|s| s.to_str()) {
                        // look for common private key filenames; we will store the path to the private key
                        if fname == "id_ed25519" || fname == "id_rsa" || fname == "id_ecdsa" || fname == "id_dsa" {
                            ssh_key_path = Some(p.to_string_lossy().into_owned());
                            break;
                        }
                    }
                }
            }
        }
    }

    // Build minimal profile if we found anything
    if name.is_empty() && email.is_empty() && signingkey.is_empty() && ssh_key_path.is_none() {
        return Ok(vec![]);
    }

    let label = if !name.is_empty() && !email.is_empty() {
        format!("{} <{}>", name, email)
    } else if !name.is_empty() {
        name.clone()
    } else {
        email.clone()
    };

    let profile = GitProfile {
        id: Uuid::new_v4().to_string(),
        label,
        name,
        email,
        color: "#6A5ACD".to_string(),
        ssh_key_path,
        gpg_key_id: if signingkey.is_empty() { None } else { Some(signingkey) },
        is_default: false,
        remote_url,
        remote_service,
    };

    Ok(vec![profile])
}

/// Recursively walk `dir` up to `max_depth` levels deep, collecting git repos.
fn collect_repos(dir: &Path, depth: u32, max_depth: u32, results: &mut Vec<PathBuf>) {
    if depth > max_depth {
        return;
    }
    // If this directory is itself a git repo, record it and stop recursing.
    if dir.join(".git").exists() {
        results.push(dir.to_path_buf());
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        // Skip hidden dirs, large build/dependency dirs
        if name.starts_with('.') ||
           name == "node_modules" ||
           name == "target" ||
           name == "dist" ||
           name == "build" ||
           name == ".git" {
            continue;
        }
        collect_repos(&path, depth + 1, max_depth, results);
    }
}

/// Scan `root` recursively (up to `max_depth`, default 5) for git repositories
/// and return identity + remote information for each one found.
#[tauri::command]
pub fn scan_repos(app: AppHandle, root: String, max_depth: Option<u32>) -> Result<Vec<ScannedRepo>, String> {
    let max_depth = max_depth.unwrap_or(5).min(10);
    let root_path = Path::new(&root);
    if !root_path.exists() {
        return Err(format!("Root path does not exist: {}", root));
    }
    if !root_path.is_dir() {
        return Err(format!("Root path is not a directory: {}", root));
    }

    // Validate scan root is within the user's home directory to prevent scanning system directories
    #[cfg(not(windows))]
    {
        let home = env::var("USERPROFILE")
            .or_else(|_| env::var("HOME"))
            .ok()
            .map(PathBuf::from);
        if let Some(ref home_dir) = home {
            let canonical_root = std::fs::canonicalize(root_path)
                .unwrap_or_else(|_| root_path.to_path_buf());
            let canonical_home = std::fs::canonicalize(home_dir)
                .unwrap_or_else(|_| home_dir.clone());
            if !canonical_root.starts_with(&canonical_home) {
                return Err("Scan root must be inside your home directory".to_string());
            }
        }
    }

    // Load profiles once for matching
    let config = crate::config::store::load_config(&app).map_err(|e| e.to_string())?;

    let mut repo_paths: Vec<PathBuf> = Vec::new();
    collect_repos(root_path, 0, max_depth, &mut repo_paths);
    // Cap at 200 repos to avoid overwhelming the UI
    repo_paths.truncate(200);

    let mut repos: Vec<ScannedRepo> = Vec::new();
    for repo_path in &repo_paths {
        let user_name  = git_config_in_dir(repo_path, &["config", "--local", "--get", "user.name"]);
        let user_email = git_config_in_dir(repo_path, &["config", "--local", "--get", "user.email"]);
        let remote_url = git_config_in_dir(repo_path, &["config", "--get", "remote.origin.url"]);
        // Capture repo-local core.sshCommand if present so UI can show real per-repo SSH command
        let core_ssh_cmd = git_config_in_dir(repo_path, &["config", "--local", "--get", "core.sshCommand"]);
        let remote_service = remote_url.as_deref().map(detect_remote_service);

        let name = repo_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Find the first GitSwitch profile where name+email match the repo's local identity
        let matched_profile_id = match (&user_name, &user_email) {
            (Some(uname), Some(uemail)) => {
                config.profiles.iter().find(|p| {
                    p.name.trim().to_lowercase() == uname.trim().to_lowercase()
                    && p.email.trim().to_lowercase() == uemail.trim().to_lowercase()
                }).map(|p| p.id.clone())
            }
            _ => None,
        };

        repos.push(ScannedRepo {
            path: repo_path.to_string_lossy().into_owned(),
            name,
            user_name,
            user_email,
            remote_url,
            remote_service,
            matched_profile_id,
            ssh_command: core_ssh_cmd,
        });
    }

    Ok(repos)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── detect_remote_service ────────────────────────────────────

    #[test]
    fn detects_github_https() {
        assert_eq!(detect_remote_service("https://github.com/user/repo.git"), "github");
    }

    #[test]
    fn detects_github_ssh() {
        assert_eq!(detect_remote_service("git@github.com:user/repo.git"), "github");
    }

    #[test]
    fn detects_gitlab_https() {
        assert_eq!(detect_remote_service("https://gitlab.com/user/repo.git"), "gitlab");
    }

    #[test]
    fn detects_gitlab_self_hosted() {
        assert_eq!(detect_remote_service("https://gitlab.mycompany.com/user/repo"), "gitlab");
    }

    #[test]
    fn detects_bitbucket() {
        assert_eq!(detect_remote_service("git@bitbucket.org:user/repo.git"), "bitbucket");
    }

    #[test]
    fn detects_other_for_unknown() {
        assert_eq!(detect_remote_service("https://example.com/user/repo.git"), "other");
    }

    #[test]
    fn detects_case_insensitive() {
        assert_eq!(detect_remote_service("https://GITHUB.COM/User/Repo"), "github");
        assert_eq!(detect_remote_service("https://GITLAB.COM/User/Repo"), "gitlab");
    }

    // ── collect_repos ────────────────────────────────────────────

    #[test]
    fn collect_repos_respects_max_depth_zero() {
        // With depth=0 and max_depth=0 on a non-git directory, should find nothing
        let tmp = std::env::temp_dir().join("gitswitch_test_collect_repos_depth0");
        let _ = fs::create_dir_all(&tmp);
        let mut results = Vec::new();
        collect_repos(&tmp, 0, 0, &mut results);
        // Depth 0 just checks the top-level dir; should not recurse into children
        // The tmp dir itself is not a git repo so results should be empty
        assert!(results.is_empty());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn collect_repos_finds_git_dir() {
        let tmp = std::env::temp_dir().join("gitswitch_test_collect_repos_find");
        let repo = tmp.join("my_repo");
        let _ = fs::create_dir_all(repo.join(".git"));

        let mut results = Vec::new();
        collect_repos(&tmp, 0, 3, &mut results);
        assert!(!results.is_empty(), "should find the git repo");
        assert!(results.iter().any(|p| p.ends_with("my_repo")));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn collect_repos_skips_node_modules() {
        let tmp = std::env::temp_dir().join("gitswitch_test_collect_repos_skip");
        let nm_repo = tmp.join("node_modules").join("some_pkg");
        let _ = fs::create_dir_all(nm_repo.join(".git"));

        let mut results = Vec::new();
        collect_repos(&tmp, 0, 5, &mut results);
        // The repo inside node_modules should be skipped
        assert!(
            !results.iter().any(|p| p.to_string_lossy().contains("node_modules")),
            "should skip repos inside node_modules"
        );

        let _ = fs::remove_dir_all(&tmp);
    }
}
