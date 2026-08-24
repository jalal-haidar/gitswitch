use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use notify::event::ModifyKind;
use notify::{
    Config as NotifyConfig, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};
use tauri::{AppHandle, Emitter};

use crate::commands::profiles::{find_git_root, switch_profile_for_repo};
use crate::config::store;
use crate::models::{
    AutoSwitchFailureEvent, RepoApplySource, WatcherLifecycleEvent, WatcherLifecycleState,
};

#[derive(Clone, Debug)]
pub(crate) struct ResolvedRule {
    pub(crate) root_path: PathBuf,
    pub(crate) profile_id: String,
    pub(crate) rule_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedTarget {
    pub(crate) rule_id: String,
    pub(crate) profile_id: String,
    pub(crate) repository_path: PathBuf,
    pub(crate) matched_path: PathBuf,
    rule_root: PathBuf,
    rule_depth: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct DebounceKey {
    pub(crate) rule_id: String,
    pub(crate) repository_path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AutoSwitchDecision {
    Disabled,
    AlreadyApplied {
        repo: PathBuf,
    },
    Apply {
        rule_id: String,
        profile_id: String,
        repo: PathBuf,
    },
}

struct WatcherLoopFailure {
    message: String,
    was_healthy: bool,
}

#[derive(Debug)]
struct RestartSupervisor {
    next_delay: Duration,
    recovering: bool,
}

impl Default for RestartSupervisor {
    fn default() -> Self {
        Self {
            next_delay: Duration::from_secs(1),
            recovering: false,
        }
    }
}

impl RestartSupervisor {
    fn record_failure(&mut self, was_healthy: bool) -> (bool, Duration) {
        if was_healthy {
            self.next_delay = Duration::from_secs(1);
            self.recovering = false;
        }
        let emit_degraded = !self.recovering;
        let delay = self.next_delay;
        self.recovering = true;
        self.next_delay = (self.next_delay * 2).min(Duration::from_secs(30));
        (emit_degraded, delay)
    }
}

fn emit_lifecycle(
    app: &AppHandle,
    state: WatcherLifecycleState,
    message: impl Into<String>,
    retry_in: Option<Duration>,
) {
    let _ = app.emit(
        "auto-switch-lifecycle",
        WatcherLifecycleEvent {
            state,
            message: message.into(),
            retry_in_ms: retry_in.map(|duration| duration.as_millis() as u64),
        },
    );
}

pub fn start_auto_switch_watcher(app: AppHandle) {
    thread::spawn(move || {
        let mut supervisor = RestartSupervisor::default();
        loop {
            match run_watcher_loop(app.clone(), supervisor.recovering) {
                Ok(()) => {
                    emit_lifecycle(
                        &app,
                        WatcherLifecycleState::Stopped,
                        "Repository activity watcher stopped.",
                        None,
                    );
                    break;
                }
                Err(failure) => {
                    let (emit_degraded, retry_delay) =
                        supervisor.record_failure(failure.was_healthy);
                    eprintln!(
                        "[auto-switch] watcher loop stopped: {error}, restarting in {}s",
                        retry_delay.as_secs(),
                        error = failure.message,
                    );
                    if emit_degraded {
                        emit_lifecycle(
                            &app,
                            WatcherLifecycleState::Degraded,
                            failure.message.clone(),
                            None,
                        );
                    }
                    emit_lifecycle(
                        &app,
                        WatcherLifecycleState::Restarting,
                        failure.message,
                        Some(retry_delay),
                    );
                    thread::sleep(retry_delay);
                }
            }
        }
    });
}

fn run_watcher_loop(app: AppHandle, mut recovering: bool) -> Result<(), WatcherLoopFailure> {
    let (tx, rx) = mpsc::channel();

    let mut watcher: Option<RecommendedWatcher> = None;
    let mut config_signature = String::new();
    let mut resolved_rules: Vec<ResolvedRule> = Vec::new();
    let mut last_switch: HashMap<DebounceKey, Instant> = HashMap::new();
    let mut was_healthy = false;
    let mut was_stopped = false;

    loop {
        let config = store::load_config(&app).map_err(|error| WatcherLoopFailure {
            message: format!("failed to load watcher configuration: {error}"),
            was_healthy,
        })?;
        let next_signature = build_signature(&config);

        if next_signature != config_signature {
            config_signature = next_signature;
            last_switch.clear();

            if config.settings.auto_switch {
                let (new_watcher, next_rules, watched_root_count) =
                    build_watcher_and_rules(tx.clone(), &config).map_err(|error| {
                        WatcherLoopFailure {
                            message: format!("failed to build watcher: {error}"),
                            was_healthy,
                        }
                    })?;
                watcher = new_watcher;
                resolved_rules = next_rules;
                if watched_root_count == 0 {
                    if !was_stopped {
                        emit_lifecycle(
                            &app,
                            WatcherLifecycleState::Stopped,
                            "No valid directory rules are available to watch.",
                            None,
                        );
                    }
                    was_stopped = true;
                    recovering = false;
                    was_healthy = true;
                } else {
                    if recovering || was_stopped {
                        emit_lifecycle(
                            &app,
                            WatcherLifecycleState::Recovered,
                            format!(
                                "Repository activity watcher is running across {watched_root_count} root{}.",
                                if watched_root_count == 1 { "" } else { "s" }
                            ),
                            None,
                        );
                    }
                    recovering = false;
                    was_stopped = false;
                    was_healthy = true;
                }
            } else {
                watcher = None;
                resolved_rules.clear();
                if !was_stopped {
                    emit_lifecycle(
                        &app,
                        WatcherLifecycleState::Stopped,
                        "Repository activity enforcement is disabled.",
                        None,
                    );
                }
                was_stopped = true;
                recovering = false;
                was_healthy = true;
            }
        }

        let _watcher_alive = watcher.as_ref();

        match rx.recv_timeout(Duration::from_secs(2)) {
            Ok(Ok(event)) => {
                handle_event(&app, &resolved_rules, &event, &mut last_switch).map_err(
                    |message| WatcherLoopFailure {
                        message,
                        was_healthy,
                    },
                )?;
            }
            Ok(Err(error)) => {
                return Err(WatcherLoopFailure {
                    message: format!("watcher event error: {error}"),
                    was_healthy,
                });
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(WatcherLoopFailure {
                    message: "auto-switch channel disconnected".to_string(),
                    was_healthy,
                });
            }
        }
    }
}

/// Returns true for known-noisy paths that should not trigger a profile switch:
/// temp files, editor swaps, OS metadata, build artefacts.
fn should_ignore_path(path: &Path) -> bool {
    const IGNORED_DIRECTORIES: [&str; 9] = [
        ".git",
        "node_modules",
        "target",
        "dist",
        "build",
        "out",
        ".next",
        ".turbo",
        "coverage",
    ];
    if path.components().any(|component| {
        let Component::Normal(name) = component else {
            return false;
        };
        IGNORED_DIRECTORIES
            .iter()
            .any(|ignored| name.to_string_lossy().eq_ignore_ascii_case(ignored))
    }) {
        return true;
    }

    // Filter by file extension / name
    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
        let name_lower = file_name.to_lowercase();
        // OS metadata
        if name_lower == ".ds_store" || name_lower == "thumbs.db" || name_lower == "desktop.ini" {
            return true;
        }
        // Temp / swap / lock files
        if name_lower.ends_with(".tmp")
            || name_lower.ends_with(".lock")
            || name_lower.ends_with(".swp")
            || name_lower.ends_with(".swo")
            || name_lower.ends_with(".bak")
            || name_lower.ends_with('~')
        {
            return true;
        }
    }

    false
}

fn is_relevant_event_kind(kind: EventKind) -> bool {
    matches!(
        kind,
        EventKind::Any
            | EventKind::Create(_)
            | EventKind::Remove(_)
            | EventKind::Modify(ModifyKind::Any | ModifyKind::Data(_) | ModifyKind::Name(_))
    )
}

/// How long to suppress repeat switches for the same rule after one fires.
/// 1500 ms covers IDE batch-save storms (e.g. "save all") and cargo build
/// which can emit thousands of file events in quick succession.
const DEBOUNCE_DURATION: Duration = Duration::from_millis(1500);

fn handle_event(
    app: &AppHandle,
    rules: &[ResolvedRule],
    event: &Event,
    last_switch: &mut HashMap<DebounceKey, Instant>,
) -> Result<(), String> {
    if rules.is_empty() || !is_relevant_event_kind(event.kind) {
        return Ok(());
    }

    let Some(target) = select_best_target(&event.paths, rules) else {
        return Ok(());
    };

    let now = Instant::now();
    let debounce_key = DebounceKey {
        rule_id: target.rule_id.clone(),
        repository_path: target.repository_path.clone(),
    };
    if update_debounce(last_switch, debounce_key, now) {
        return Ok(());
    }

    let config = store::load_config(app)
        .map_err(|error| format!("failed to load automatic-apply configuration: {error}"))?;

    let decision = evaluate_target(&config, &target, |repo, key| {
        crate::commands::profiles::read_local_git_config(repo, key)
    });

    let (git_root, profile_id, rule_id) = match decision {
        AutoSwitchDecision::Disabled | AutoSwitchDecision::AlreadyApplied { .. } => return Ok(()),
        AutoSwitchDecision::Apply {
            repo,
            profile_id,
            rule_id,
        } => (repo, profile_id, rule_id),
    };

    match switch_profile_for_repo(
        app.clone(),
        profile_id.clone(),
        &git_root,
        RepoApplySource::Auto,
        Some(&rule_id),
    ) {
        Err(error) => {
            eprintln!("[auto-switch] failed to switch profile: {error}");
            let _ = app.emit(
                "auto-switch-failed",
                AutoSwitchFailureEvent {
                    rule_id,
                    profile_id,
                    repository_path: git_root.to_string_lossy().into_owned(),
                    message: error,
                },
            );
        }
        Ok(event) => {
            let _ = app.emit("repo-profile-applied", event);
        }
    }
    Ok(())
}

pub(crate) fn select_best_target(
    event_paths: &[PathBuf],
    rules: &[ResolvedRule],
) -> Option<ResolvedTarget> {
    let mut best_match: Option<ResolvedTarget> = None;
    for event_path in event_paths {
        if should_ignore_path(event_path) {
            continue;
        }
        let normalized = normalize_event_path(event_path);
        let Some(repository_path) =
            find_git_root(&normalized).map(|repo| normalize_event_path(&repo))
        else {
            continue;
        };
        for rule in rules {
            if !path_starts_with_platform_rules(&normalized, &rule.root_path) {
                continue;
            }
            let candidate = ResolvedTarget {
                rule_id: rule.rule_id.clone(),
                profile_id: rule.profile_id.clone(),
                repository_path: repository_path.clone(),
                matched_path: normalized.clone(),
                rule_root: rule.root_path.clone(),
                rule_depth: rule.root_path.components().count(),
            };
            if best_match
                .as_ref()
                .is_none_or(|current| target_precedes(&candidate, current))
            {
                best_match = Some(candidate);
            }
        }
    }
    best_match
}

fn target_precedes(candidate: &ResolvedTarget, current: &ResolvedTarget) -> bool {
    candidate.rule_depth > current.rule_depth
        || (candidate.rule_depth == current.rule_depth
            && (
                comparison_key(&candidate.rule_root),
                candidate.rule_id.as_str(),
                comparison_key(&candidate.repository_path),
                comparison_key(&candidate.matched_path),
            ) < (
                comparison_key(&current.rule_root),
                current.rule_id.as_str(),
                comparison_key(&current.repository_path),
                comparison_key(&current.matched_path),
            ))
}

pub(crate) fn update_debounce(
    last_switch: &mut HashMap<DebounceKey, Instant>,
    key: DebounceKey,
    now: Instant,
) -> bool {
    if last_switch
        .get(&key)
        .is_some_and(|previous| now.duration_since(*previous) < DEBOUNCE_DURATION)
    {
        return true;
    }
    last_switch.insert(key, now);
    false
}

pub(crate) fn evaluate_target<F>(
    config: &crate::models::AppConfig,
    target: &ResolvedTarget,
    mut read_local: F,
) -> AutoSwitchDecision
where
    F: FnMut(&Path, &str) -> Option<String>,
{
    if !config.settings.auto_switch {
        return AutoSwitchDecision::Disabled;
    }
    let repo = target.repository_path.clone();

    let snapshot = crate::models::GitConfigSnapshot {
        user_name: read_local(&repo, "user.name"),
        user_email: read_local(&repo, "user.email"),
        user_signingkey: read_local(&repo, "user.signingkey"),
        commit_gpgsign: read_local(&repo, "commit.gpgsign"),
        core_ssh_command: read_local(&repo, "core.sshCommand"),
    };
    if crate::git::unique_matching_profile(&config.profiles, &snapshot)
        .is_some_and(|profile| profile.id == target.profile_id)
    {
        return AutoSwitchDecision::AlreadyApplied { repo };
    }

    AutoSwitchDecision::Apply {
        rule_id: target.rule_id.clone(),
        profile_id: target.profile_id.clone(),
        repo,
    }
}

fn build_watcher_and_rules(
    tx: mpsc::Sender<notify::Result<Event>>,
    config: &crate::models::AppConfig,
) -> Result<(Option<RecommendedWatcher>, Vec<ResolvedRule>, usize), notify::Error> {
    let mut resolved_rules = Vec::new();

    for rule in &config.directory_rules {
        let path_str = rule.path.trim();
        if path_str.is_empty() {
            continue;
        }

        let path = PathBuf::from(path_str);
        if !path.exists() {
            continue;
        }

        let normalized = normalize_path(&path).unwrap_or(path);
        resolved_rules.push(ResolvedRule {
            root_path: normalized,
            profile_id: rule.profile_id.clone(),
            rule_id: rule.id.clone(),
        });
    }

    resolved_rules.sort_by(|left, right| {
        comparison_key(&left.root_path)
            .cmp(&comparison_key(&right.root_path))
            .then_with(|| left.rule_id.cmp(&right.rule_id))
    });

    let mut watched_roots: Vec<PathBuf> = Vec::new();
    for rule in &resolved_rules {
        if watched_roots
            .iter()
            .any(|root| path_starts_with_platform_rules(&rule.root_path, root))
        {
            continue;
        }
        watched_roots.push(rule.root_path.clone());
    }

    if watched_roots.is_empty() {
        return Ok((None, resolved_rules, 0));
    }

    let mut watcher = RecommendedWatcher::new(
        move |result| {
            let _ = tx.send(result);
        },
        NotifyConfig::default(),
    )?;
    for root in &watched_roots {
        watcher.watch(root, RecursiveMode::Recursive)?;
    }

    let watched_root_count = watched_roots.len();
    Ok((Some(watcher), resolved_rules, watched_root_count))
}

pub(crate) fn normalize_path(path: &Path) -> Option<PathBuf> {
    let canonical = fs::canonicalize(path).ok()?;
    // On Windows, canonicalize() returns \\?\-prefixed extended-length paths.
    // Strip that prefix so starts_with comparisons work correctly.
    #[cfg(windows)]
    {
        let s = canonical.to_string_lossy();
        // The prefix is exactly 4 chars: \, \, ?, \
        // Using a regular string literal so the escape sequence is unambiguous.
        if let Some(stripped) = s.strip_prefix("\\\\?\\") {
            return Some(PathBuf::from(stripped));
        }
    }
    Some(canonical)
}

fn normalize_event_path(path: &Path) -> PathBuf {
    if let Some(normalized) = normalize_path(path) {
        return normalized;
    }

    let mut ancestor = path;
    let mut suffix: Vec<OsString> = Vec::new();
    while let Some(name) = ancestor.file_name() {
        suffix.push(name.to_os_string());
        let Some(parent) = ancestor.parent() else {
            break;
        };
        ancestor = parent;
        if ancestor.exists() {
            break;
        }
    }

    let Some(mut rebuilt) = normalize_path(ancestor) else {
        return path.to_path_buf();
    };
    for component in suffix.into_iter().rev() {
        rebuilt.push(component);
    }
    rebuilt
}

fn comparison_key(path: &Path) -> String {
    let value = path.to_string_lossy().replace('\\', "/");
    #[cfg(windows)]
    {
        value.to_lowercase()
    }
    #[cfg(not(windows))]
    {
        value
    }
}

pub(crate) fn paths_equal(left: &Path, right: &Path) -> bool {
    comparison_key(left) == comparison_key(right)
}

/// Case-insensitive path prefix check used on Windows;
/// falls back to the standard `starts_with` on other platforms.
/// Ensures the prefix ends on a path-separator boundary so that
/// `C:\work` does NOT match `C:\work2`.
fn path_starts_with_platform_rules(path: &Path, prefix: &Path) -> bool {
    #[cfg(windows)]
    {
        let path_s = path.to_string_lossy().to_lowercase();
        let prefix_s = prefix.to_string_lossy().to_lowercase();
        if !path_s.starts_with(prefix_s.as_str()) {
            return false;
        }
        let remainder = &path_s[prefix_s.len()..];
        // Exact match, or next char is a path separator
        remainder.is_empty() || remainder.starts_with('/') || remainder.starts_with('\\')
    }
    #[cfg(not(windows))]
    {
        path.starts_with(prefix)
    }
}

fn build_signature(config: &crate::models::AppConfig) -> String {
    let mut rules = config
        .directory_rules
        .iter()
        .map(|rule| format!("{}|{}|{}", rule.id, rule.path, rule.profile_id))
        .collect::<Vec<_>>();

    rules.sort();

    format!("auto:{}::{}", config.settings.auto_switch, rules.join(";"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn test_root(name: &str) -> PathBuf {
        let suffix = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "gitswitch-auto-switch-{name}-{}-{suffix}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn resolved_rule(id: &str, root: &Path, profile_id: &str) -> ResolvedRule {
        ResolvedRule {
            root_path: normalize_path(root).unwrap(),
            profile_id: profile_id.to_string(),
            rule_id: id.to_string(),
        }
    }

    // ── should_ignore_path ───────────────────────────────────────

    #[test]
    fn ignores_node_modules() {
        assert!(should_ignore_path(Path::new(
            "/project/node_modules/foo/index.js"
        )));
        assert!(should_ignore_path(Path::new(
            "C:\\project\\node_modules\\bar.js"
        )));
    }

    #[test]
    fn ignores_common_build_output_directories() {
        for directory in [
            "target", "dist", "build", "out", ".next", ".turbo", "coverage",
        ] {
            assert!(
                should_ignore_path(&Path::new("C:\\repo").join(directory).join("asset.js")),
                "directory: {directory}"
            );
        }
    }

    #[test]
    fn ignores_git_objects() {
        assert!(should_ignore_path(Path::new(
            "/repo/.git/objects/ab/cdef123"
        )));
        assert!(should_ignore_path(Path::new("/repo/.git/refs/heads/main")));
        assert!(should_ignore_path(Path::new("/repo/.git/logs/HEAD")));
    }

    #[test]
    fn ignores_os_metadata() {
        assert!(should_ignore_path(Path::new("/project/.DS_Store")));
        assert!(should_ignore_path(Path::new("C:\\project\\Thumbs.db")));
        assert!(should_ignore_path(Path::new("C:\\project\\desktop.ini")));
    }

    #[test]
    fn ignores_temp_and_swap_files() {
        assert!(should_ignore_path(Path::new("/project/file.tmp")));
        assert!(should_ignore_path(Path::new("/project/.file.swp")));
        assert!(should_ignore_path(Path::new("/project/file.lock")));
        assert!(should_ignore_path(Path::new("/project/file.bak")));
        assert!(should_ignore_path(Path::new("/project/file~")));
    }

    #[test]
    fn allows_normal_source_files() {
        assert!(!should_ignore_path(Path::new("/project/src/main.rs")));
        assert!(!should_ignore_path(Path::new("/project/README.md")));
        assert!(!should_ignore_path(Path::new("C:\\project\\src\\App.tsx")));
    }

    #[test]
    fn ignores_all_git_internal_activity() {
        assert!(should_ignore_path(Path::new("/repo/.git/COMMIT_EDITMSG")));
        assert!(should_ignore_path(Path::new("/repo/.git/HEAD")));
    }

    #[test]
    fn only_mutating_event_kinds_are_relevant() {
        assert!(is_relevant_event_kind(EventKind::Any));
        assert!(is_relevant_event_kind(EventKind::Create(
            notify::event::CreateKind::File
        )));
        assert!(is_relevant_event_kind(EventKind::Modify(ModifyKind::Data(
            notify::event::DataChange::Any
        ))));
        assert!(is_relevant_event_kind(EventKind::Remove(
            notify::event::RemoveKind::File
        )));
        assert!(!is_relevant_event_kind(EventKind::Access(
            notify::event::AccessKind::Any
        )));
        assert!(!is_relevant_event_kind(EventKind::Modify(
            ModifyKind::Metadata(notify::event::MetadataKind::Any)
        )));
    }

    // ── path matching ─────────────────────────────────────────────

    #[test]
    fn prefix_exact_match() {
        let path = PathBuf::from("C:\\work\\project");
        let prefix = PathBuf::from("C:\\work\\project");
        assert!(path_starts_with_platform_rules(&path, &prefix));
    }

    #[test]
    fn prefix_child_path() {
        let path = PathBuf::from("C:\\work\\project\\src\\main.rs");
        let prefix = PathBuf::from("C:\\work\\project");
        assert!(path_starts_with_platform_rules(&path, &prefix));
    }

    #[test]
    fn prefix_rejects_partial_dir_name() {
        // C:\work should NOT match C:\work2
        let path = PathBuf::from("C:\\work2\\file.rs");
        let prefix = PathBuf::from("C:\\work");
        assert!(!path_starts_with_platform_rules(&path, &prefix));
    }

    #[test]
    fn prefix_case_insensitive_on_windows() {
        let path = PathBuf::from("C:\\WORK\\PROJECT\\file.rs");
        let prefix = PathBuf::from("c:\\work\\project");
        #[cfg(windows)]
        assert!(path_starts_with_platform_rules(&path, &prefix));
        #[cfg(not(windows))]
        {
            // On non-Windows, paths are case-sensitive
            let _ = (path, prefix);
        }
    }

    #[test]
    fn longest_overlapping_rule_wins_for_the_actual_repository() {
        let root = test_root("overlap");
        let repo = root.join("nested");
        let event_path = repo.join("src").join("main.rs");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir_all(event_path.parent().unwrap()).unwrap();
        fs::write(&event_path, "fn main() {}").unwrap();
        let rules = vec![
            resolved_rule("parent", &root, "parent-profile"),
            resolved_rule("nested", &repo, "nested-profile"),
        ];

        let target = select_best_target(&[event_path], &rules).unwrap();
        assert_eq!(target.rule_id, "nested");
        assert_eq!(target.profile_id, "nested-profile");
        assert!(paths_equal(&target.repository_path, &repo));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn event_path_order_does_not_change_the_selected_repository() {
        let root = test_root("stable-order");
        let repo_a = root.join("a-repo");
        let repo_b = root.join("b-repo");
        fs::create_dir_all(repo_a.join(".git")).unwrap();
        fs::create_dir_all(repo_b.join(".git")).unwrap();
        let event_a = repo_a.join("a.rs");
        let event_b = repo_b.join("b.rs");
        fs::write(&event_a, "a").unwrap();
        fs::write(&event_b, "b").unwrap();
        let rules = vec![resolved_rule("parent", &root, "profile")];

        let first = select_best_target(&[event_b.clone(), event_a.clone()], &rules).unwrap();
        let second = select_best_target(&[event_a, event_b], &rules).unwrap();
        assert!(paths_equal(&first.repository_path, &second.repository_path));
        assert!(paths_equal(&first.repository_path, &repo_a));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn debounce_is_independent_per_rule_and_repository() {
        let now = Instant::now();
        let mut state = HashMap::new();
        let key_a = DebounceKey {
            rule_id: "parent".to_string(),
            repository_path: PathBuf::from("C:\\work\\repo-a"),
        };
        let key_b = DebounceKey {
            rule_id: "parent".to_string(),
            repository_path: PathBuf::from("C:\\work\\repo-b"),
        };

        assert!(!update_debounce(&mut state, key_a.clone(), now));
        assert!(!update_debounce(&mut state, key_b, now));
        assert!(update_debounce(
            &mut state,
            key_a,
            now + Duration::from_millis(100)
        ));
    }

    #[test]
    fn restart_supervisor_degrades_once_and_resets_after_health() {
        let mut supervisor = RestartSupervisor::default();
        assert_eq!(
            supervisor.record_failure(false),
            (true, Duration::from_secs(1))
        );
        assert_eq!(
            supervisor.record_failure(false),
            (false, Duration::from_secs(2))
        );
        assert_eq!(
            supervisor.record_failure(true),
            (true, Duration::from_secs(1))
        );
    }

    // ── build_signature ──────────────────────────────────────────

    #[test]
    fn signature_changes_with_auto_switch_toggle() {
        let mut config = crate::models::AppConfig::default();
        config.settings.auto_switch = true;
        let sig_a = build_signature(&config);

        config.settings.auto_switch = false;
        let sig_b = build_signature(&config);

        assert_ne!(sig_a, sig_b);
    }

    #[test]
    fn signature_changes_with_rules() {
        let mut config = crate::models::AppConfig::default();
        let sig_a = build_signature(&config);

        config.directory_rules.push(crate::models::DirectoryRule {
            id: "r1".into(),
            path: "/projects/work".into(),
            profile_id: "p1".into(),
            last_triggered_at: None,
        });
        let sig_b = build_signature(&config);

        assert_ne!(sig_a, sig_b);
    }

    #[test]
    fn signature_stable_regardless_of_rule_order() {
        let mut config = crate::models::AppConfig::default();
        config.directory_rules.push(crate::models::DirectoryRule {
            id: "r1".into(),
            path: "/a".into(),
            profile_id: "p1".into(),
            last_triggered_at: None,
        });
        config.directory_rules.push(crate::models::DirectoryRule {
            id: "r2".into(),
            path: "/b".into(),
            profile_id: "p2".into(),
            last_triggered_at: None,
        });
        let sig_a = build_signature(&config);

        // Reverse the order
        config.directory_rules.reverse();
        let sig_b = build_signature(&config);

        assert_eq!(
            sig_a, sig_b,
            "signature should be stable regardless of rule order"
        );
    }
}
