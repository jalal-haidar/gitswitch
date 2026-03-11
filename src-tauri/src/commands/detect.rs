use tauri::AppHandle;
use std::{process::Command, fs, env, path::Path};
use uuid::Uuid;

use crate::models::GitProfile;

#[tauri::command]
pub fn detect_identities(_app: AppHandle, directory: Option<String>) -> Result<Vec<GitProfile>, String> {
    // If a directory is provided, run git commands there; otherwise use current dir
    let dir = directory
        .or_else(|| env::var("PWD").ok())
        .unwrap_or_else(|| String::new());
    let path = if dir.is_empty() { Path::new(".") } else { Path::new(&dir) };

    // Helper to run git and capture stdout as trimmed string
    let run_git = |args: &[&str]| -> Option<String> {
        Command::new("git")
            .args(args)
            .current_dir(path)
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    if s.is_empty() { None } else { Some(s) }
                } else {
                    None
                }
            })
    };

    // Try local (repo) values first, fall back to global
    let name = run_git(&["config", "user.name"]).or_else(|| {
        Command::new("git").args(&["config", "--global", "--get", "user.name"]).output().ok()
            .and_then(|o| if o.status.success() { Some(String::from_utf8_lossy(&o.stdout).trim().to_string()) } else { None })
    }).unwrap_or_default();

    let email = run_git(&["config", "user.email"]).or_else(|| {
        Command::new("git").args(&["config", "--global", "--get", "user.email"]).output().ok()
            .and_then(|o| if o.status.success() { Some(String::from_utf8_lossy(&o.stdout).trim().to_string()) } else { None })
    }).unwrap_or_default();

    let signingkey = run_git(&["config", "user.signingkey"]).or_else(|| {
        Command::new("git").args(&["config", "--global", "--get", "user.signingkey"]).output().ok()
            .and_then(|o| if o.status.success() { Some(String::from_utf8_lossy(&o.stdout).trim().to_string()) } else { None })
    }).unwrap_or_default();

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
