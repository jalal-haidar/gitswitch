use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard};

use once_cell::sync::Lazy;

use crate::errors::{summarize_failure, BackendError};
use crate::models::{GitConfigSnapshot, GitProfile};

static GIT_TRANSACTION_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

/// Suppress the CMD console window flicker on Windows when spawning child processes.
/// No-op on non-Windows platforms.
#[cfg(windows)]
pub fn no_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000);
}

#[cfg(not(windows))]
pub fn no_window(_cmd: &mut Command) {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GitCommandOutput {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub(crate) trait GitExecutor: Send + Sync {
    fn run(&self, args: &[String], cwd: Option<&Path>) -> Result<GitCommandOutput, String>;
}

#[derive(Clone, Debug)]
pub(crate) struct ProcessGitExecutor {
    executable: OsString,
    environment: BTreeMap<OsString, OsString>,
    removed_environment: Vec<OsString>,
}

impl Default for ProcessGitExecutor {
    fn default() -> Self {
        Self {
            executable: OsString::from("git"),
            environment: BTreeMap::new(),
            removed_environment: Vec::new(),
        }
    }
}

impl ProcessGitExecutor {
    #[cfg(feature = "native-test-support")]
    pub(crate) fn isolated(environment: BTreeMap<OsString, OsString>) -> Self {
        Self {
            executable: OsString::from("git"),
            environment,
            removed_environment: vec![
                OsString::from("GIT_DIR"),
                OsString::from("GIT_WORK_TREE"),
                OsString::from("GIT_INDEX_FILE"),
                OsString::from("GIT_CONFIG"),
                OsString::from("GIT_CONFIG_COUNT"),
            ],
        }
    }
}

impl GitExecutor for ProcessGitExecutor {
    fn run(&self, args: &[String], cwd: Option<&Path>) -> Result<GitCommandOutput, String> {
        let mut command = Command::new(&self.executable);
        command.args(args);
        if let Some(path) = cwd {
            command.current_dir(path);
        }
        for key in &self.removed_environment {
            command.env_remove(key);
        }
        for (key, value) in &self.environment {
            command.env(key, value);
        }
        no_window(&mut command);

        let output = command.output().map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                BackendError::git_not_found().to_string()
            } else {
                BackendError::io_error(format!("Failed to execute git command: {error}"))
                    .to_string()
            }
        })?;

        Ok(GitCommandOutput {
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout)
                .trim_end_matches(['\r', '\n'])
                .to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GitScope {
    Global,
    Local(PathBuf),
}

impl GitScope {
    fn flag(&self) -> &'static str {
        match self {
            Self::Global => "--global",
            Self::Local(_) => "--local",
        }
    }

    fn cwd(&self) -> Option<&Path> {
        match self {
            Self::Global => None,
            Self::Local(path) => Some(path),
        }
    }
}

pub(crate) fn transaction_guard() -> MutexGuard<'static, ()> {
    GIT_TRANSACTION_LOCK.lock().unwrap_or_else(|poisoned| {
        eprintln!("[git] transaction mutex poisoned, recovering");
        poisoned.into_inner()
    })
}

fn output_error(output: GitCommandOutput) -> String {
    let stderr_lower = output.stderr.to_lowercase();
    if stderr_lower.contains("permission denied") || stderr_lower.contains("cannot open") {
        BackendError::permission_denied(output.stderr).to_string()
    } else {
        BackendError::git_failed(output.stderr).to_string()
    }
}

pub(crate) fn execute_checked(
    executor: &dyn GitExecutor,
    args: &[String],
    cwd: Option<&Path>,
) -> Result<(), String> {
    let output = executor.run(args, cwd)?;
    if output.success {
        Ok(())
    } else {
        Err(output_error(output))
    }
}

pub(crate) fn read_value(
    executor: &dyn GitExecutor,
    scope: &GitScope,
    key: &str,
) -> Result<Option<String>, String> {
    let output = executor.run(
        &[
            "config".to_string(),
            scope.flag().to_string(),
            "--get".to_string(),
            key.to_string(),
        ],
        scope.cwd(),
    )?;
    if output.success {
        Ok(Some(output.stdout))
    } else if output.exit_code == Some(1) {
        Ok(None)
    } else {
        Err(output_error(output))
    }
}

fn set_value(
    executor: &dyn GitExecutor,
    scope: &GitScope,
    key: &str,
    value: &str,
) -> Result<(), String> {
    execute_checked(
        executor,
        &[
            "config".to_string(),
            scope.flag().to_string(),
            key.to_string(),
            value.to_string(),
        ],
        scope.cwd(),
    )
}

fn unset_value(executor: &dyn GitExecutor, scope: &GitScope, key: &str) -> Result<(), String> {
    execute_checked(
        executor,
        &[
            "config".to_string(),
            scope.flag().to_string(),
            "--unset".to_string(),
            key.to_string(),
        ],
        scope.cwd(),
    )
}

pub(crate) fn read_snapshot(
    executor: &dyn GitExecutor,
    scope: &GitScope,
) -> Result<GitConfigSnapshot, String> {
    Ok(GitConfigSnapshot {
        user_name: read_value(executor, scope, "user.name")?,
        user_email: read_value(executor, scope, "user.email")?,
        user_signingkey: read_value(executor, scope, "user.signingkey")?,
        commit_gpgsign: read_value(executor, scope, "commit.gpgsign")?,
        core_ssh_command: read_value(executor, scope, "core.sshCommand")?,
    })
}

pub(crate) fn expected_snapshot_for_profile(profile: &GitProfile) -> GitConfigSnapshot {
    let ssh_key_path = profile
        .ssh_key_path
        .as_deref()
        .filter(|value| !value.is_empty());
    let signing_key = profile
        .gpg_key_id
        .as_deref()
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    GitConfigSnapshot {
        user_name: Some(profile.name.clone()),
        user_email: Some(profile.email.clone()),
        user_signingkey: signing_key.clone(),
        commit_gpgsign: Some(
            if signing_key.is_some() {
                "true"
            } else {
                "false"
            }
            .to_string(),
        ),
        core_ssh_command: ssh_key_path.map(|path| {
            format!(
                "ssh -i \"{}\" -o IdentitiesOnly=yes",
                path.replace('\\', "/")
            )
        }),
    }
}

pub(crate) fn snapshot_for_profile(profile: &GitProfile) -> Result<GitConfigSnapshot, String> {
    if let Some(path) = profile
        .ssh_key_path
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        let path = Path::new(path);
        if !path.is_file() {
            return Err(format!(
                "SSH key file not found for profile '{}': {}. Edit the profile to fix the path.",
                profile.label,
                path.display()
            ));
        }
    }
    Ok(expected_snapshot_for_profile(profile))
}

pub(crate) fn unique_matching_profile<'a>(
    profiles: &'a [GitProfile],
    snapshot: &GitConfigSnapshot,
) -> Option<&'a GitProfile> {
    let mut matches = profiles
        .iter()
        .filter(|profile| expected_snapshot_for_profile(profile) == *snapshot);
    let matched = matches.next()?;
    matches.next().is_none().then_some(matched)
}

pub(crate) fn preflight(executor: &dyn GitExecutor, scope: &GitScope) -> Result<(), String> {
    let config_path = match scope {
        GitScope::Local(repo) => {
            let repository =
                executor.run(&args(["rev-parse", "--is-inside-work-tree"]), Some(repo))?;
            if !repository.success || repository.stdout != "true" {
                return Err(format!("Not a Git repository: {}", repo.display()));
            }
            let output = executor.run(&args(["rev-parse", "--git-path", "config"]), Some(repo))?;
            if !output.success || output.stdout.is_empty() {
                return Err(output_error(output));
            }
            let path = PathBuf::from(output.stdout);
            if path.is_absolute() {
                path
            } else {
                repo.join(path)
            }
        }
        GitScope::Global => {
            let output = executor.run(&args(["var", "-l"]), None)?;
            if !output.success {
                return Err(output_error(output));
            }
            output
                .stdout
                .lines()
                .filter_map(|line| line.strip_prefix("GIT_CONFIG_GLOBAL="))
                .map(PathBuf::from)
                .next_back()
                .ok_or_else(|| {
                    "Git did not report a writable global configuration path".to_string()
                })?
        }
    };
    preflight_config_path(&config_path)
}

fn preflight_config_path(config_path: &Path) -> Result<(), String> {
    let parent = config_path.parent().ok_or_else(|| {
        format!(
            "Cannot determine parent directory for Git config {}",
            config_path.display()
        )
    })?;
    if !parent.is_dir() {
        return Err(format!(
            "Git config directory does not exist: {}",
            parent.display()
        ));
    }
    if config_path.exists() {
        OpenOptions::new()
            .write(true)
            .open(config_path)
            .map_err(|error| {
                format!(
                    "Git config is not writable at {}: {error}",
                    config_path.display()
                )
            })?;
    }

    let mut lock_name = config_path.as_os_str().to_os_string();
    lock_name.push(".lock");
    let lock_path = PathBuf::from(lock_name);
    let probe = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .map_err(|error| {
            format!(
                "Git config cannot acquire a write lock at {}: {error}",
                lock_path.display()
            )
        })?;
    drop(probe);
    fs::remove_file(&lock_path).map_err(|error| {
        format!(
            "Failed to remove Git config write probe at {}: {error}",
            lock_path.display()
        )
    })
}

fn write_field(
    executor: &dyn GitExecutor,
    scope: &GitScope,
    key: &str,
    current: &Option<String>,
    desired: &Option<String>,
) -> Result<(), String> {
    if current == desired {
        return Ok(());
    }
    match desired {
        Some(value) => set_value(executor, scope, key, value),
        None => unset_value(executor, scope, key),
    }
}

fn write_snapshot(
    executor: &dyn GitExecutor,
    scope: &GitScope,
    desired: &GitConfigSnapshot,
) -> Result<(), String> {
    let current = read_snapshot(executor, scope)?;
    write_field(
        executor,
        scope,
        "user.name",
        &current.user_name,
        &desired.user_name,
    )?;
    write_field(
        executor,
        scope,
        "user.email",
        &current.user_email,
        &desired.user_email,
    )?;
    write_field(
        executor,
        scope,
        "user.signingkey",
        &current.user_signingkey,
        &desired.user_signingkey,
    )?;
    write_field(
        executor,
        scope,
        "commit.gpgsign",
        &current.commit_gpgsign,
        &desired.commit_gpgsign,
    )?;
    write_field(
        executor,
        scope,
        "core.sshCommand",
        &current.core_ssh_command,
        &desired.core_ssh_command,
    )?;

    let actual = read_snapshot(executor, scope)?;
    if actual == *desired {
        Ok(())
    } else {
        Err(format!(
            "Git verification mismatch. Expected {desired:?}, found {actual:?}"
        ))
    }
}

fn rollback_snapshot(
    executor: &dyn GitExecutor,
    scope: &GitScope,
    baseline: &GitConfigSnapshot,
) -> Option<String> {
    let fields = [
        ("user.name", &baseline.user_name),
        ("user.email", &baseline.user_email),
        ("user.signingkey", &baseline.user_signingkey),
        ("commit.gpgsign", &baseline.commit_gpgsign),
        ("core.sshCommand", &baseline.core_ssh_command),
    ];
    let mut failures = Vec::new();
    for (key, desired) in fields {
        match read_value(executor, scope, key) {
            Ok(current) => {
                if let Err(error) = write_field(executor, scope, key, &current, desired) {
                    failures.push(format!("{key}: {}", summarize_failure(&error)));
                }
            }
            Err(error) => failures.push(format!("{key} read: {}", summarize_failure(&error))),
        }
    }
    match read_snapshot(executor, scope) {
        Ok(actual) if actual == *baseline => {}
        Ok(actual) => failures.push(format!(
            "rollback verification mismatch. Expected {baseline:?}, found {actual:?}"
        )),
        Err(error) => failures.push(format!("rollback verification failed: {error}")),
    }
    (!failures.is_empty()).then(|| failures.join("; "))
}

pub(crate) fn apply_snapshot_transaction(
    executor: &dyn GitExecutor,
    scope: &GitScope,
    operation: &str,
    desired: &GitConfigSnapshot,
    baseline: &GitConfigSnapshot,
) -> Result<(), String> {
    if let Err(operation_failure) = write_snapshot(executor, scope, desired) {
        let rollback_failure = rollback_snapshot(executor, scope, baseline);
        return Err(
            BackendError::git_transaction(operation, operation_failure, rollback_failure)
                .to_string(),
        );
    }
    Ok(())
}

pub(crate) fn apply_profile(
    executor: &dyn GitExecutor,
    scope: &GitScope,
    profile: &GitProfile,
) -> Result<(), String> {
    let _guard = transaction_guard();
    let desired = snapshot_for_profile(profile)?;
    preflight(executor, scope)?;
    let baseline = read_snapshot(executor, scope)?;
    apply_snapshot_transaction(executor, scope, "apply", &desired, &baseline)
}

pub(crate) fn restore_snapshot(
    executor: &dyn GitExecutor,
    scope: &GitScope,
    snapshot: &GitConfigSnapshot,
) -> Result<(), String> {
    let _guard = transaction_guard();
    preflight(executor, scope)?;
    let current = read_snapshot(executor, scope)?;
    apply_snapshot_transaction(executor, scope, "restore", snapshot, &current)
}

pub(crate) fn rollback_to_snapshot(
    executor: &dyn GitExecutor,
    scope: &GitScope,
    snapshot: &GitConfigSnapshot,
) -> Option<String> {
    rollback_snapshot(executor, scope, snapshot)
}

pub(crate) fn args<I, S>(items: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    items
        .into_iter()
        .map(|item| item.as_ref().to_string_lossy().into_owned())
        .collect()
}

#[cfg(test)]
mod matching_tests {
    use super::*;

    fn profile(id: &str, email: &str) -> GitProfile {
        GitProfile {
            id: id.to_string(),
            label: id.to_string(),
            name: "Alice".to_string(),
            email: email.to_string(),
            color: "#123456".to_string(),
            ssh_key_path: Some("C:/Users/Alice/.ssh/id_ed25519".to_string()),
            gpg_key_id: Some("ABC123".to_string()),
            is_default: false,
            remote_url: None,
            remote_service: None,
        }
    }

    #[test]
    fn exact_five_field_snapshot_has_one_match() {
        let profiles = vec![profile("work", "alice@work.test")];
        let snapshot = expected_snapshot_for_profile(&profiles[0]);

        assert_eq!(
            unique_matching_profile(&profiles, &snapshot).map(|profile| profile.id.as_str()),
            Some("work")
        );
    }

    #[test]
    fn any_field_mismatch_has_no_match() {
        let profiles = vec![profile("work", "alice@work.test")];
        let mut snapshot = expected_snapshot_for_profile(&profiles[0]);
        snapshot.commit_gpgsign = Some("false".to_string());

        assert!(unique_matching_profile(&profiles, &snapshot).is_none());
    }

    #[test]
    fn duplicate_exact_profiles_are_ambiguous() {
        let profiles = vec![
            profile("work-a", "alice@work.test"),
            profile("work-b", "alice@work.test"),
        ];
        let snapshot = expected_snapshot_for_profile(&profiles[0]);

        assert!(unique_matching_profile(&profiles, &snapshot).is_none());
    }
}
