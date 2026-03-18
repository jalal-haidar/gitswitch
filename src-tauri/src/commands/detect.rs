use tauri::AppHandle;
use std::{process::Command, fs, env, path::Path};
use crate::errors::BackendError;
use uuid::Uuid;

/// Suppress the CMD console window that briefly flickers on Windows
/// whenever a child process is spawned. No-op on non-Windows platforms.
#[cfg(windows)]
fn no_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
}
#[cfg(not(windows))]
fn no_window(_cmd: &mut Command) {}

use crate::models::GitProfile;

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
    };

    Ok(vec![profile])
}
