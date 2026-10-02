//! SSH helpers: key generation via `ssh-keygen`, `~/.ssh/config` host blocks.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::errors::BackendError;
use crate::fsutil;
use crate::managed_block;
use crate::models::GitProfile;

pub const DEFAULT_HOSTNAME: &str = "github.com";

pub fn ssh_dir() -> Result<PathBuf, BackendError> {
    crate::paths::home_dir()
        .map(|h| h.join(".ssh"))
        .ok_or_else(|| BackendError::io_error("Could not locate the home directory"))
}

pub fn host_block_id(profile_id: &str) -> String {
    format!("ssh-{}", profile_id)
}

/// Lowercase alphanumerics separated by single dashes; never empty.
pub fn slugify(label: &str) -> String {
    let mut out = String::new();
    for c in label.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() {
        "profile".to_string()
    } else {
        out
    }
}

/// Hosts and aliases end up on an `ssh` command line and in a config file.
pub fn validate_host(host: &str) -> Result<(), BackendError> {
    let ok = !host.is_empty()
        && !host.starts_with('-')
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    if ok {
        Ok(())
    } else {
        Err(BackendError::invalid_input(format!(
            "Invalid host name: {:?} (use letters, digits, '.', '-' and '_')",
            host
        )))
    }
}

/// `Host` block for `~/.ssh/config`. Needs an alias and a key on the profile.
pub fn render_host_block(profile: &GitProfile) -> Result<String, BackendError> {
    let alias = profile
        .ssh_host_alias
        .as_deref()
        .filter(|a| !a.is_empty())
        .ok_or_else(|| BackendError::invalid_input("Set a host alias first"))?;
    let key = profile
        .ssh_key_path
        .as_deref()
        .filter(|k| !k.is_empty())
        .ok_or_else(|| BackendError::invalid_input("Choose or generate an SSH key first"))?;
    let hostname = profile
        .ssh_hostname
        .as_deref()
        .filter(|h| !h.is_empty())
        .unwrap_or(DEFAULT_HOSTNAME);
    validate_host(alias)?;
    validate_host(hostname)?;

    Ok(format!(
        "Host {}\n    HostName {}\n    User git\n    IdentityFile \"{}\"\n    IdentitiesOnly yes\n",
        alias,
        hostname,
        key.replace('\\', "/")
    ))
}

fn read_or_empty(path: &Path) -> Result<String, BackendError> {
    match fs::read_to_string(path) {
        Ok(t) => Ok(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.into()),
    }
}

/// Adds or replaces the profile's block in the ssh config file at `config_path`.
pub fn write_host_block(config_path: &Path, profile: &GitProfile) -> Result<(), BackendError> {
    let body = render_host_block(profile)?;
    let current = read_or_empty(config_path)?;
    let updated = managed_block::upsert_block(&current, &host_block_id(&profile.id), &body);
    if updated != current {
        if let Some(dir) = config_path.parent() {
            fs::create_dir_all(dir)?;
        }
        fsutil::backup_once(config_path)?;
        fsutil::write_atomic(config_path, &updated)?;
    }
    Ok(())
}

/// Removes the profile's block, if present.
pub fn remove_host_block(config_path: &Path, profile_id: &str) -> Result<(), BackendError> {
    let current = read_or_empty(config_path)?;
    let updated = managed_block::remove_block(&current, &host_block_id(profile_id));
    if updated != current {
        fsutil::backup_once(config_path)?;
        fsutil::write_atomic(config_path, &updated)?;
    }
    Ok(())
}

/// Re-renders an existing block after the profile changed (or removes it if the
/// profile no longer has an alias/key). Does nothing when no block was ever written.
pub fn refresh_host_block(config_path: &Path, profile: &GitProfile) -> Result<(), BackendError> {
    let current = read_or_empty(config_path)?;
    if !managed_block::has_block(&current, &host_block_id(&profile.id)) {
        return Ok(());
    }
    match render_host_block(profile) {
        Ok(_) => write_host_block(config_path, profile),
        Err(_) => remove_host_block(config_path, &profile.id),
    }
}

pub fn run_tool(program: &str, args: &[&str]) -> Result<Output, BackendError> {
    Command::new(program).args(args).output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            BackendError::new(
                crate::errors::BackendErrorKind::SshFailed,
                format!("`{}` was not found on PATH", program),
            )
            .with_hint("Install OpenSSH (included with Git for Windows and recent Windows 10/11)")
        } else {
            BackendError::io_error(format!("Failed to run {}: {}", program, e))
        }
    })
}

/// Generates `<dir>/<file_name>` (+ `.pub`) as ed25519. Never overwrites existing keys.
pub fn generate_key(
    dir: &Path,
    file_name: &str,
    comment: &str,
    passphrase: &str,
) -> Result<PathBuf, BackendError> {
    fs::create_dir_all(dir)?;
    let key = dir.join(file_name);
    let mut pub_name = key.as_os_str().to_owned();
    pub_name.push(".pub");
    if key.exists() || Path::new(&pub_name).exists() {
        return Err(BackendError::invalid_input(format!(
            "A key already exists at {}",
            key.display()
        )));
    }

    let key_str = key.to_string_lossy();
    let out = run_tool(
        "ssh-keygen",
        &["-q", "-t", "ed25519", "-C", comment, "-f", &key_str, "-N", passphrase],
    )?;
    if !out.status.success() {
        return Err(BackendError::ssh_failed(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ));
    }
    Ok(key)
}

/// Private keys in `dir` that have a matching `.pub`.
pub fn list_keys(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return vec![];
    };
    let mut keys: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "pub"))
        .map(|p| p.with_extension(""))
        .filter(|p| p.is_file())
        .collect();
    keys.sort();
    keys
}

/// Reads the public half of `key_path` (accepts the private or `.pub` path).
pub fn read_public_key(key_path: &str) -> Result<String, BackendError> {
    let p = if key_path.ends_with(".pub") {
        PathBuf::from(key_path)
    } else {
        PathBuf::from(format!("{}.pub", key_path))
    };
    Ok(fs::read_to_string(&p)?.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("gs-ssh-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn profile() -> GitProfile {
        GitProfile {
            id: "p1".into(),
            label: "Work".into(),
            name: "W".into(),
            email: "w@x.dev".into(),
            ssh_key_path: Some("C:\\Users\\me\\.ssh\\gitswitch_work".into()),
            ssh_host_alias: Some("github-work".into()),
            ..Default::default()
        }
    }

    #[test]
    fn slugify_cases() {
        assert_eq!(slugify("Work Account!"), "work-account");
        assert_eq!(slugify("  --A__b  "), "a-b");
        assert_eq!(slugify("日本語"), "profile");
    }

    #[test]
    fn host_validation_blocks_option_injection() {
        assert!(validate_host("github.com").is_ok());
        assert!(validate_host("github-work").is_ok());
        assert!(validate_host("-oProxyCommand=evil").is_err());
        assert!(validate_host("a b").is_err());
        assert!(validate_host("").is_err());
    }

    #[test]
    fn host_block_rendering() {
        let s = render_host_block(&profile()).unwrap();
        assert!(s.starts_with("Host github-work\n"));
        assert!(s.contains("HostName github.com"));
        assert!(s.contains("IdentityFile \"C:/Users/me/.ssh/gitswitch_work\""));
        assert!(s.contains("IdentitiesOnly yes"));

        let mut p = profile();
        p.ssh_host_alias = None;
        assert!(render_host_block(&p).is_err());
        let mut p = profile();
        p.ssh_key_path = None;
        assert!(render_host_block(&p).is_err());
    }

    #[test]
    fn config_block_lifecycle_preserves_user_content() {
        let dir = temp("cfg");
        let cfg = dir.join("config");
        let user = "Host mine\n    HostName example.com\n";
        fs::write(&cfg, user).unwrap();

        write_host_block(&cfg, &profile()).unwrap();
        let text = fs::read_to_string(&cfg).unwrap();
        assert!(text.starts_with(user) && text.contains("Host github-work"));
        assert!(dir.join("config.gitswitch.bak").exists());

        // refresh follows profile edits and drops the block when alias is removed
        let mut p = profile();
        p.ssh_hostname = Some("gitlab.com".into());
        refresh_host_block(&cfg, &p).unwrap();
        assert!(fs::read_to_string(&cfg).unwrap().contains("HostName gitlab.com"));
        p.ssh_host_alias = None;
        refresh_host_block(&cfg, &p).unwrap();
        assert_eq!(fs::read_to_string(&cfg).unwrap(), user);

        // refresh never creates a block on its own
        refresh_host_block(&cfg, &profile()).unwrap();
        assert_eq!(fs::read_to_string(&cfg).unwrap(), user);

        write_host_block(&cfg, &profile()).unwrap();
        remove_host_block(&cfg, "p1").unwrap();
        assert_eq!(fs::read_to_string(&cfg).unwrap(), user);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn generates_lists_and_reads_key_without_overwriting() {
        if run_tool("ssh-keygen", &["-?"]).is_err() {
            eprintln!("ssh-keygen not available; skipping");
            return;
        }
        let dir = temp("keys");
        let key = generate_key(&dir, "gitswitch_work", "w@x.dev", "").unwrap();
        assert!(key.is_file());
        assert_eq!(list_keys(&dir), vec![key.clone()]);

        let public = read_public_key(&key.to_string_lossy()).unwrap();
        assert!(public.starts_with("ssh-ed25519 ") && public.ends_with("w@x.dev"));

        let again = generate_key(&dir, "gitswitch_work", "w@x.dev", "");
        assert!(matches!(
            again,
            Err(ref e) if matches!(e.kind, crate::errors::BackendErrorKind::InvalidInput)
        ));
        fs::remove_dir_all(&dir).ok();
    }
}
