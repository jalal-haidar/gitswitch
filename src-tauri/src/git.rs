use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::errors::BackendError;
use crate::models::{GitConfigSnapshot, GitProfile};

/// Suppress the CMD console window flicker on Windows when spawning child processes.
/// No-op on non-Windows platforms.
#[cfg(windows)]
pub fn no_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
}

#[cfg(not(windows))]
pub fn no_window(_cmd: &mut Command) {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GitCommandOutput {
    pub success: bool,
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
            stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
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

pub(crate) fn execute_checked(
    executor: &dyn GitExecutor,
    args: &[String],
    cwd: Option<&Path>,
) -> Result<(), String> {
    let output = executor.run(args, cwd)?;
    if output.success {
        return Ok(());
    }

    let stderr_lower = output.stderr.to_lowercase();
    if stderr_lower.contains("permission denied") || stderr_lower.contains("cannot open") {
        Err(BackendError::permission_denied(output.stderr).to_string())
    } else {
        Err(BackendError::git_failed(output.stderr).to_string())
    }
}

pub(crate) fn read_value(
    executor: &dyn GitExecutor,
    scope: &GitScope,
    key: &str,
) -> Result<Option<String>, String> {
    let args = vec![
        "config".to_string(),
        scope.flag().to_string(),
        "--get".to_string(),
        key.to_string(),
    ];
    let output = executor.run(&args, scope.cwd())?;
    if !output.success || output.stdout.is_empty() {
        Ok(None)
    } else {
        Ok(Some(output.stdout))
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

pub(crate) fn apply_profile(
    executor: &dyn GitExecutor,
    scope: &GitScope,
    profile: &GitProfile,
) -> Result<(), String> {
    set_value(executor, scope, "user.name", &profile.name)?;
    set_value(executor, scope, "user.email", &profile.email)?;

    if let Some(gpg_key) = profile
        .gpg_key_id
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        set_value(executor, scope, "user.signingkey", gpg_key)?;
        set_value(executor, scope, "commit.gpgsign", "true")?;
    } else {
        let _ = unset_value(executor, scope, "user.signingkey");
        let _ = set_value(executor, scope, "commit.gpgsign", "false");
    }

    match profile
        .ssh_key_path
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        Some(ssh_path) => {
            // Preserve the current workflow: repo-local switches validate the
            // SSH key after earlier identity writes; global switches do not.
            if matches!(scope, GitScope::Local(_)) && !Path::new(ssh_path).exists() {
                return Err(format!(
                    "SSH key file not found for profile '{}': {}. Edit the profile to fix the path.",
                    profile.label, ssh_path
                ));
            }
            let normalized = ssh_path.replace('\\', "/");
            let command = format!("ssh -i \"{normalized}\" -o IdentitiesOnly=yes");
            set_value(executor, scope, "core.sshCommand", &command)?;
        }
        None => {
            let _ = unset_value(executor, scope, "core.sshCommand");
        }
    }

    Ok(())
}

pub(crate) fn restore_snapshot(
    executor: &dyn GitExecutor,
    scope: &GitScope,
    snapshot: &GitConfigSnapshot,
) -> Result<(), String> {
    match snapshot.user_name.as_deref() {
        Some(value) => set_value(executor, scope, "user.name", value)?,
        None => {
            let _ = unset_value(executor, scope, "user.name");
        }
    }
    match snapshot.user_email.as_deref() {
        Some(value) => set_value(executor, scope, "user.email", value)?,
        None => {
            let _ = unset_value(executor, scope, "user.email");
        }
    }
    match snapshot
        .user_signingkey
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        Some(value) => {
            set_value(executor, scope, "user.signingkey", value)?;
            if matches!(scope, GitScope::Local(_)) {
                set_value(executor, scope, "commit.gpgsign", "true")?;
            } else {
                let _ = set_value(executor, scope, "commit.gpgsign", "true");
            }
        }
        None => {
            let _ = unset_value(executor, scope, "user.signingkey");
            let _ = set_value(executor, scope, "commit.gpgsign", "false");
        }
    }
    match snapshot.commit_gpgsign.as_deref() {
        Some(value) => set_value(executor, scope, "commit.gpgsign", value)?,
        None => {
            let _ = unset_value(executor, scope, "commit.gpgsign");
        }
    }
    match snapshot
        .core_ssh_command
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        Some(value) => set_value(executor, scope, "core.sshCommand", value)?,
        None => {
            let _ = unset_value(executor, scope, "core.sshCommand");
        }
    }
    Ok(())
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
