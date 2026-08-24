//! Feature-gated native integration-test support. This module is not part of
//! production builds; it exposes isolated adapters around the same core used
//! by the Tauri commands.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};

use crate::auto_switch::{self, AutoSwitchDecision, DebounceKey, ResolvedRule};
use crate::config::{
    snapshots,
    store::{self, CredentialStore},
};
use crate::git::{self, GitCommandOutput, GitExecutor, GitScope, ProcessGitExecutor};

pub use crate::models::{AppConfig, AppSettings, DirectoryRule, GitConfigSnapshot, GitProfile};

struct TempRoot {
    directory: tempfile::TempDir,
}

impl TempRoot {
    fn new(label: &str) -> Result<Self> {
        Ok(Self {
            directory: tempfile::Builder::new()
                .prefix(&format!("gitswitch-{label}-"))
                .tempdir()?,
        })
    }

    fn path(&self) -> &Path {
        self.directory.path()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedGitCall {
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
}

struct HarnessGitExecutor {
    process: ProcessGitExecutor,
    fail_next_write: Mutex<Option<String>>,
    mutation_count: Mutex<usize>,
    fail_mutations: Mutex<HashSet<usize>>,
    skip_mutations: Mutex<HashSet<usize>>,
    calls: Mutex<Vec<RecordedGitCall>>,
}

impl HarnessGitExecutor {
    fn mutation_key(args: &[String]) -> Option<&str> {
        if args.first().map(String::as_str) != Some("config") || args.len() < 3 {
            return None;
        }
        match args[2].as_str() {
            "--get" => None,
            "--unset" => args.get(3).map(String::as_str),
            key => Some(key),
        }
    }
}

impl GitExecutor for HarnessGitExecutor {
    fn run(&self, args: &[String], cwd: Option<&Path>) -> Result<GitCommandOutput, String> {
        self.calls.lock().unwrap().push(RecordedGitCall {
            args: args.to_vec(),
            cwd: cwd.map(Path::to_path_buf),
        });

        if let Some(key) = Self::mutation_key(args) {
            let ordinal = {
                let mut count = self.mutation_count.lock().unwrap();
                *count += 1;
                *count
            };
            if self.fail_mutations.lock().unwrap().remove(&ordinal) {
                return Ok(GitCommandOutput {
                    success: false,
                    exit_code: Some(2),
                    stdout: String::new(),
                    stderr: format!("injected Git failure for mutation {ordinal} ({key})"),
                });
            }
            if self.skip_mutations.lock().unwrap().remove(&ordinal) {
                return Ok(GitCommandOutput {
                    success: true,
                    exit_code: Some(0),
                    stdout: String::new(),
                    stderr: String::new(),
                });
            }
            let mut failure = self.fail_next_write.lock().unwrap();
            if failure.as_deref() == Some(key) {
                *failure = None;
                return Ok(GitCommandOutput {
                    success: false,
                    exit_code: Some(2),
                    stdout: String::new(),
                    stderr: format!("injected Git failure for {key}"),
                });
            }
        }

        self.process.run(args, cwd)
    }
}

pub struct GitSandbox {
    root: TempRoot,
    executor: HarnessGitExecutor,
    global_config: PathBuf,
    guard_config: PathBuf,
    environment: BTreeMap<OsString, OsString>,
}

impl GitSandbox {
    pub fn new() -> Result<Self> {
        let root = TempRoot::new("native-git")?;
        let home = root.path().join("home");
        let xdg = root.path().join("xdg");
        fs::create_dir_all(&home)?;
        fs::create_dir_all(&xdg)?;
        let global_config = root.path().join("isolated-global.gitconfig");
        let guard_config = root.path().join("must-not-change.gitconfig");
        fs::write(&guard_config, b"[guard]\n\tvalue = untouched\n")?;

        let environment = BTreeMap::from([
            (OsString::from("HOME"), home.clone().into_os_string()),
            (OsString::from("USERPROFILE"), home.into_os_string()),
            (OsString::from("XDG_CONFIG_HOME"), xdg.into_os_string()),
            (
                OsString::from("GIT_CONFIG_GLOBAL"),
                global_config.clone().into_os_string(),
            ),
            (OsString::from("GIT_CONFIG_NOSYSTEM"), OsString::from("1")),
            (OsString::from("GIT_TERMINAL_PROMPT"), OsString::from("0")),
        ]);
        let executor = HarnessGitExecutor {
            process: ProcessGitExecutor::isolated(environment.clone()),
            fail_next_write: Mutex::new(None),
            mutation_count: Mutex::new(0),
            fail_mutations: Mutex::new(HashSet::new()),
            skip_mutations: Mutex::new(HashSet::new()),
            calls: Mutex::new(Vec::new()),
        };

        Ok(Self {
            root,
            executor,
            global_config,
            guard_config,
            environment,
        })
    }

    pub fn root(&self) -> &Path {
        self.root.path()
    }

    pub fn init_repo(&self, name: &str) -> Result<PathBuf, String> {
        let repo = self.root.path().join(name);
        fs::create_dir_all(&repo).map_err(|error| error.to_string())?;
        git::execute_checked(&self.executor, &git::args(["init", "--quiet"]), Some(&repo))?;
        Ok(repo)
    }

    pub fn create_ssh_key(&self, name: &str) -> Result<PathBuf> {
        let path = self.root.path().join(name);
        fs::write(&path, b"integration-test-key")?;
        Ok(path)
    }

    pub fn set_global(&self, key: &str, value: &str) -> Result<(), String> {
        git::execute_checked(
            &self.executor,
            &git::args(["config", "--global", key, value]),
            None,
        )
    }

    pub fn set_local(&self, repo: &Path, key: &str, value: &str) -> Result<(), String> {
        git::execute_checked(
            &self.executor,
            &git::args(["config", "--local", key, value]),
            Some(repo),
        )
    }

    pub fn read_global(&self) -> Result<GitConfigSnapshot, String> {
        git::read_snapshot(&self.executor, &GitScope::Global)
    }

    pub fn read_local(&self, repo: &Path) -> Result<GitConfigSnapshot, String> {
        git::read_snapshot(&self.executor, &GitScope::Local(repo.to_path_buf()))
    }

    pub fn apply_global(&self, profile: &GitProfile) -> Result<(), String> {
        git::apply_profile(&self.executor, &GitScope::Global, profile)
    }

    pub fn apply_local(&self, repo: &Path, profile: &GitProfile) -> Result<(), String> {
        git::apply_profile(
            &self.executor,
            &GitScope::Local(repo.to_path_buf()),
            profile,
        )
    }

    pub fn restore_global(&self, snapshot: &GitConfigSnapshot) -> Result<(), String> {
        git::restore_snapshot(&self.executor, &GitScope::Global, snapshot)
    }

    pub fn restore_local(&self, repo: &Path, snapshot: &GitConfigSnapshot) -> Result<(), String> {
        git::restore_snapshot(
            &self.executor,
            &GitScope::Local(repo.to_path_buf()),
            snapshot,
        )
    }

    pub fn fail_next_write(&self, key: &str) {
        *self.executor.fail_next_write.lock().unwrap() = Some(key.to_string());
    }

    pub fn fail_mutations(&self, ordinals: &[usize]) {
        *self.executor.mutation_count.lock().unwrap() = 0;
        *self.executor.fail_mutations.lock().unwrap() = ordinals.iter().copied().collect();
    }

    pub fn skip_mutations(&self, ordinals: &[usize]) {
        *self.executor.mutation_count.lock().unwrap() = 0;
        *self.executor.skip_mutations.lock().unwrap() = ordinals.iter().copied().collect();
    }

    pub fn calls(&self) -> Vec<RecordedGitCall> {
        self.executor.calls.lock().unwrap().clone()
    }

    pub fn isolation_environment(&self) -> BTreeMap<String, String> {
        self.environment
            .iter()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().into_owned(),
                    value.to_string_lossy().into_owned(),
                )
            })
            .collect()
    }

    pub fn global_config_path(&self) -> &Path {
        &self.global_config
    }

    pub fn guard_bytes(&self) -> Result<Vec<u8>> {
        Ok(fs::read(&self.guard_config)?)
    }
}

pub struct SnapshotHarness {
    root: TempRoot,
    path: PathBuf,
}

impl SnapshotHarness {
    pub fn new() -> Result<Self> {
        let root = TempRoot::new("native-snapshots")?;
        let path = root.path().join("git-snapshots.json");
        Ok(Self { root, path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn root(&self) -> &Path {
        self.root.path()
    }

    pub fn global(&self) -> Result<Option<GitConfigSnapshot>, String> {
        Ok(snapshots::load_at(&self.path)
            .map_err(|error| error.to_string())?
            .global)
    }

    pub fn swap_global(
        &self,
        replacement: Option<GitConfigSnapshot>,
    ) -> Result<Option<GitConfigSnapshot>, String> {
        let mut document = snapshots::load_at(&self.path).map_err(|error| error.to_string())?;
        let previous = std::mem::replace(&mut document.global, replacement);
        snapshots::persist_at(&self.path, &document).map_err(|error| error.to_string())?;
        Ok(previous)
    }

    pub fn repository(&self, repository: &Path) -> Result<Option<GitConfigSnapshot>, String> {
        let key = snapshots::normalize_repo_key(repository)?;
        Ok(snapshots::load_at(&self.path)
            .map_err(|error| error.to_string())?
            .repositories
            .get(&key)
            .cloned())
    }

    pub fn swap_repository(
        &self,
        repository: &Path,
        replacement: Option<GitConfigSnapshot>,
    ) -> Result<Option<GitConfigSnapshot>, String> {
        let key = snapshots::normalize_repo_key(repository)?;
        let mut document = snapshots::load_at(&self.path).map_err(|error| error.to_string())?;
        let previous = match replacement {
            Some(snapshot) => document.repositories.insert(key, snapshot),
            None => document.repositories.remove(&key),
        };
        snapshots::persist_at(&self.path, &document).map_err(|error| error.to_string())?;
        Ok(previous)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialFailure {
    Read,
    Write,
    Delete,
}

#[derive(Default)]
struct MemoryCredentialStore {
    entries: Mutex<HashMap<String, String>>,
    fail_reads: Mutex<HashSet<String>>,
    fail_writes: Mutex<HashSet<String>>,
    fail_deletes: Mutex<HashSet<String>>,
}

impl CredentialStore for MemoryCredentialStore {
    fn get(&self, account: &str) -> Result<Option<String>> {
        if self.fail_reads.lock().unwrap().contains(account) {
            return Err(anyhow!("injected credential read failure"));
        }
        Ok(self.entries.lock().unwrap().get(account).cloned())
    }

    fn set(&self, account: &str, value: &str) -> Result<()> {
        if self.fail_writes.lock().unwrap().contains(account) {
            return Err(anyhow!("injected credential write failure"));
        }
        self.entries
            .lock()
            .unwrap()
            .insert(account.to_string(), value.to_string());
        Ok(())
    }

    fn delete(&self, account: &str) -> Result<()> {
        if self.fail_deletes.lock().unwrap().contains(account) {
            return Err(anyhow!("injected credential delete failure"));
        }
        self.entries.lock().unwrap().remove(account);
        Ok(())
    }
}

pub struct ConfigHarness {
    root: TempRoot,
    path: PathBuf,
    credentials: MemoryCredentialStore,
}

impl ConfigHarness {
    pub fn new() -> Result<Self> {
        let root = TempRoot::new("native-config")?;
        let path = root.path().join("profiles.json");
        Ok(Self {
            root,
            path,
            credentials: MemoryCredentialStore::default(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn root(&self) -> &Path {
        self.root.path()
    }

    pub fn write(&self, config: &AppConfig) -> Result<()> {
        fs::write(&self.path, serde_json::to_vec_pretty(config)?)?;
        Ok(())
    }

    pub fn load(&self) -> Result<AppConfig, String> {
        store::load_config_at(&self.path, &self.credentials).map_err(|error| error.to_string())
    }

    pub fn persist(&self, config: &AppConfig) -> Result<(), String> {
        store::persist_config_at(&self.path, config, &self.credentials)
            .map_err(|error| error.to_string())
    }

    pub fn fail(&self, operation: CredentialFailure, account: &str) {
        let target = match operation {
            CredentialFailure::Read => &self.credentials.fail_reads,
            CredentialFailure::Write => &self.credentials.fail_writes,
            CredentialFailure::Delete => &self.credentials.fail_deletes,
        };
        target.lock().unwrap().insert(account.to_string());
    }

    pub fn clear_failures(&self) {
        self.credentials.fail_reads.lock().unwrap().clear();
        self.credentials.fail_writes.lock().unwrap().clear();
        self.credentials.fail_deletes.lock().unwrap().clear();
    }

    pub fn entries(&self) -> HashMap<String, String> {
        self.credentials.entries.lock().unwrap().clone()
    }

    pub fn bytes(&self) -> Result<Vec<u8>> {
        Ok(fs::read(&self.path)?)
    }
}

pub fn credential_account(profile_id: &str, field: &str) -> String {
    format!("{profile_id}:{field}")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TestAutoSwitchDecision {
    NoMatch,
    Debounced,
    Disabled,
    AlreadyApplied(PathBuf),
    Apply {
        rule_id: String,
        profile_id: String,
        repo: PathBuf,
    },
}

pub fn decide_auto_switch(
    sandbox: &GitSandbox,
    config: &AppConfig,
    rules: &[DirectoryRule],
    event_paths: &[PathBuf],
    previous_elapsed: Option<Duration>,
) -> TestAutoSwitchDecision {
    let resolved: Vec<_> = rules
        .iter()
        .map(|rule| ResolvedRule {
            root_path: auto_switch::normalize_path(Path::new(&rule.path))
                .unwrap_or_else(|| PathBuf::from(&rule.path)),
            profile_id: rule.profile_id.clone(),
            rule_id: rule.id.clone(),
        })
        .collect();
    let Some(target) = auto_switch::select_best_target(event_paths, &resolved) else {
        return TestAutoSwitchDecision::NoMatch;
    };

    let now = Instant::now();
    let mut debounce = HashMap::new();
    let debounce_key = DebounceKey {
        rule_id: target.rule_id.clone(),
        repository_path: target.repository_path.clone(),
    };
    if let Some(elapsed) = previous_elapsed {
        debounce.insert(debounce_key.clone(), now - elapsed);
    }
    if auto_switch::update_debounce(&mut debounce, debounce_key, now) {
        return TestAutoSwitchDecision::Debounced;
    }

    match auto_switch::evaluate_target(config, &target, |repo, key| {
        git::read_value(&sandbox.executor, &GitScope::Local(repo.to_path_buf()), key)
            .ok()
            .flatten()
    }) {
        AutoSwitchDecision::Disabled => TestAutoSwitchDecision::Disabled,
        AutoSwitchDecision::AlreadyApplied { repo } => TestAutoSwitchDecision::AlreadyApplied(repo),
        AutoSwitchDecision::Apply {
            rule_id,
            profile_id,
            repo,
        } => TestAutoSwitchDecision::Apply {
            rule_id,
            profile_id,
            repo,
        },
    }
}
