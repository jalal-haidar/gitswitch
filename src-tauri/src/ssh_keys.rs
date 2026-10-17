//! SSH key helpers: generate ed25519 keys with `ssh-keygen`, list and read keys in `~/.ssh`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::git::no_window;

/// Lowercase alphanumerics separated by single dashes; never empty.
pub(crate) fn slugify(label: &str) -> String {
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

fn run_keygen(args: &[&str]) -> Result<Output, String> {
    let mut cmd = Command::new("ssh-keygen");
    cmd.args(args);
    no_window(&mut cmd);
    cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "ssh-keygen executable not found — install OpenSSH or Git for Windows".to_string()
        } else {
            format!("Failed to run ssh-keygen: {e}")
        }
    })
}

pub(crate) fn keygen_available() -> bool {
    run_keygen(&["-?"]).is_ok()
}

fn pub_path(key: &Path) -> PathBuf {
    let mut name = key.as_os_str().to_owned();
    name.push(".pub");
    PathBuf::from(name)
}

/// Generates `<dir>/<file_name>` (+ `.pub`) as a passphrase-less ed25519 key.
/// Never overwrites an existing key.
pub(crate) fn generate_key(dir: &Path, file_name: &str, comment: &str) -> Result<PathBuf, String> {
    fs::create_dir_all(dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    let key = dir.join(file_name);
    if key.exists() || pub_path(&key).exists() {
        return Err(format!("A key already exists at {}", key.display()));
    }

    let key_str = key.to_string_lossy();
    let out = run_keygen(&["-q", "-t", "ed25519", "-C", comment, "-f", &key_str, "-N", ""])?;
    if !out.status.success() {
        return Err(format!(
            "ssh-keygen failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(key)
}

/// Private keys in `dir` that have a matching `.pub`, sorted.
pub(crate) fn list_keys(dir: &Path) -> Vec<PathBuf> {
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

/// Reads the public half of an already-validated private key path.
pub(crate) fn read_public_key(key: &Path) -> Result<String, String> {
    let path = pub_path(key);
    fs::read_to_string(&path)
        .map(|text| text.trim().to_string())
        .map_err(|e| format!("Could not read {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("gs-sshkeys-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn slugify_cases() {
        assert_eq!(slugify("Work Account!"), "work-account");
        assert_eq!(slugify("  --A__b  "), "a-b");
        assert_eq!(slugify("日本語"), "profile");
    }

    #[test]
    fn list_keys_requires_matching_pub_file() {
        let dir = temp("list");
        fs::write(dir.join("a"), "k").unwrap();
        fs::write(dir.join("a.pub"), "p").unwrap();
        fs::write(dir.join("orphan"), "k").unwrap();
        fs::write(dir.join("only.pub"), "p").unwrap();
        assert_eq!(list_keys(&dir), vec![dir.join("a")]);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn generates_reads_and_refuses_to_overwrite() {
        if !keygen_available() {
            eprintln!("ssh-keygen not available; skipping");
            return;
        }
        let dir = temp("gen");
        let key = generate_key(&dir, "gitswitch_work", "w@x.dev").unwrap();
        assert!(key.is_file());
        assert_eq!(list_keys(&dir), vec![key.clone()]);

        let public = read_public_key(&key).unwrap();
        assert!(public.starts_with("ssh-ed25519 ") && public.ends_with("w@x.dev"));

        let err = generate_key(&dir, "gitswitch_work", "w@x.dev").unwrap_err();
        assert!(err.contains("already exists"));
        fs::remove_dir_all(&dir).ok();
    }
}
