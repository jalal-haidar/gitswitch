use std::path::Path;
use std::process::{Command, Output};

use crate::errors::BackendError;

fn exec(args: &[&str], cwd: Option<&Path>) -> Result<Output, BackendError> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            BackendError::git_not_found()
        } else {
            BackendError::io_error(format!("Failed to execute git command: {}", e))
        }
    })
}

fn failure(output: &Output) -> BackendError {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let lower = stderr.to_lowercase();
    if lower.contains("permission denied") || lower.contains("cannot open") {
        BackendError::permission_denied(stderr)
    } else {
        BackendError::git_failed(stderr)
    }
}

/// Runs git and returns trimmed stdout. Any non-zero exit becomes a `BackendError`.
pub fn run_git(args: &[&str], cwd: Option<&Path>) -> Result<String, BackendError> {
    let output = exec(args, cwd)?;
    if !output.status.success() {
        return Err(failure(&output));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Reads a config value (any scope). A missing key yields `None`.
pub fn config_get(key: &str, cwd: Option<&Path>) -> Result<Option<String>, BackendError> {
    let output = exec(&["config", "--get", key], cwd)?;
    match output.status.code() {
        // exit 1: key is not set
        Some(1) if output.stderr.is_empty() => Ok(None),
        _ if output.status.success() => {
            let v = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(if v.is_empty() { None } else { Some(v) })
        }
        _ => Err(failure(&output)),
    }
}

pub fn config_set_global(key: &str, value: &str) -> Result<(), BackendError> {
    run_git(&["config", "--global", key, value], None).map(|_| ())
}

/// Unsets a global key; an absent key (exit code 5) is not an error.
pub fn config_unset_global(key: &str) -> Result<(), BackendError> {
    let output = exec(&["config", "--global", "--unset", key], None)?;
    match output.status.code() {
        Some(0) | Some(5) => Ok(()),
        _ => Err(failure(&output)),
    }
}

/// Applies an identity to the global git config, including signing settings.
pub fn apply_identity_globally(
    name: &str,
    email: &str,
    gpg_key: Option<&str>,
) -> Result<(), BackendError> {
    config_set_global("user.name", name)?;
    config_set_global("user.email", email)?;
    match gpg_key.filter(|k| !k.is_empty()) {
        Some(key) => {
            config_set_global("user.signingkey", key)?;
            config_set_global("commit.gpgsign", "true")?;
        }
        None => {
            config_unset_global("user.signingkey")?;
            config_set_global("commit.gpgsign", "false")?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_git_reports_git_failed_on_bad_args() {
        match run_git(&["not-a-real-subcommand"], None) {
            Err(e) => assert!(matches!(
                e.kind,
                crate::errors::BackendErrorKind::GitFailed
            )),
            Ok(_) => panic!("expected failure"),
        }
    }

    #[test]
    fn config_get_missing_key_is_none() {
        let v = config_get("gitswitch.definitely.not.set", None).unwrap();
        assert!(v.is_none());
    }
}
