use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use notify::{Config as NotifyConfig, Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::commands::profiles::{find_git_root, switch_profile_for_repo};
use crate::config::store;

#[derive(Clone, Debug)]
pub(crate) struct ResolvedRule {
    pub(crate) root_path: PathBuf,
    pub(crate) profile_id: String,
    pub(crate) rule_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AutoSwitchDecision {
    Disabled,
    MissingRepository { path: PathBuf },
    AlreadyApplied { repo: PathBuf },
    Apply {
        rule_id: String,
        profile_id: String,
        repo: PathBuf,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoSwitchEvent {
    pub profile_id: String,
    pub path: String,
    pub occurred_at_epoch_ms: u64,
}

static LAST_EVENT: OnceLock<Mutex<Option<AutoSwitchEvent>>> = OnceLock::new();

fn last_event_store() -> &'static Mutex<Option<AutoSwitchEvent>> {
    LAST_EVENT.get_or_init(|| Mutex::new(None))
}

pub fn get_last_auto_switch_event() -> Option<AutoSwitchEvent> {
    match last_event_store().lock() {
        Ok(guard) => (*guard).clone(),
        Err(poisoned) => {
            // Recover from poisoned mutex — a panic in another thread shouldn't
            // permanently break last-event reads.
            eprintln!("[auto-switch] last-event mutex was poisoned, recovering");
            (*poisoned.into_inner()).clone()
        }
    }
}

fn set_last_auto_switch_event(profile_id: String, path: String) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);

    let event = Some(AutoSwitchEvent {
        profile_id,
        path,
        occurred_at_epoch_ms: now,
    });

    match last_event_store().lock() {
        Ok(mut guard) => {
            *guard = event;
        }
        Err(poisoned) => {
            eprintln!("[auto-switch] last-event mutex was poisoned, recovering for write");
            *poisoned.into_inner() = event;
        }
    }
}

pub fn start_auto_switch_watcher(app: AppHandle) {
    thread::spawn(move || {
        let mut backoff = Duration::from_secs(1);
        loop {
            match run_watcher_loop(app.clone()) {
                Ok(()) => break, // intentional shutdown — never happens currently
                Err(error) => {
                    eprintln!("[auto-switch] watcher loop stopped: {error}, restarting in {}s", backoff.as_secs());
                    let _ = app.emit("auto-switch-error", error);
                    thread::sleep(backoff);
                    backoff = (backoff * 2).min(Duration::from_secs(30));
                }
            }
        }
    });
}

fn run_watcher_loop(app: AppHandle) -> Result<(), String> {
    let (tx, rx) = mpsc::channel();

    let mut watcher: Option<RecommendedWatcher> = None;
    let mut config_signature = String::new();
    let mut resolved_rules: Vec<ResolvedRule> = Vec::new();
    // Debounce state: rule_id → last-switch instant
    let mut last_switch: HashMap<String, Instant> = HashMap::new();

    loop {
        if let Ok(config) = store::load_config(&app) {
            let next_signature = build_signature(&config);

            if next_signature != config_signature {
                config_signature = next_signature;
                last_switch.clear(); // rules changed — reset debounce state

                if config.settings.auto_switch {
                    let (new_watcher, next_rules) = build_watcher_and_rules(tx.clone(), &config)
                        .map_err(|e| format!("failed to build watcher: {e}"))?;
                    watcher = Some(new_watcher);
                    resolved_rules = next_rules;
                } else {
                    watcher = None;
                    resolved_rules.clear();
                }
            }
        }

        let _watcher_alive = watcher.as_ref();

        match rx.recv_timeout(Duration::from_secs(2)) {
            Ok(Ok(event)) => {
                handle_event(&app, &resolved_rules, &event, &mut last_switch);
            }
            Ok(Err(error)) => {
                eprintln!("[auto-switch] watcher event error: {error}");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("auto-switch channel disconnected".to_string());
            }
        }
    }
}

/// Returns true for known-noisy paths that should not trigger a profile switch:
/// temp files, editor swaps, OS metadata, build artefacts.
fn should_ignore_path(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    let path_lower = path_str.to_lowercase();

    // Skip anything under node_modules or .git internals that aren't meaningful
    if path_lower.contains("/node_modules/") || path_lower.contains("\\node_modules\\") {
        return true;
    }
    // Changes inside .git sub-directories (objects, refs, logs) are noisy;
    // we still want COMMIT_EDITMSG and HEAD changes so only filter the deep internals.
    if (path_lower.contains("/.git/objects") || path_lower.contains("\\.git\\objects"))
        || (path_lower.contains("/.git/refs") || path_lower.contains("\\.git\\refs"))
        || (path_lower.contains("/.git/logs") || path_lower.contains("\\.git\\logs"))
    {
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

/// How long to suppress repeat switches for the same rule after one fires.
/// 1500 ms covers IDE batch-save storms (e.g. "save all") and cargo build
/// which can emit thousands of file events in quick succession.
const DEBOUNCE_DURATION: Duration = Duration::from_millis(1500);

fn handle_event(
    app: &AppHandle,
    rules: &[ResolvedRule],
    event: &Event,
    last_switch: &mut HashMap<String, Instant>,
) {
    if rules.is_empty() {
        return;
    }

    let Some((match_rule, matched_path)) = select_best_rule(&event.paths, rules) else {
        return;
    };

    // Debounce: if this rule already fired recently, skip.
    let now = Instant::now();
    if update_debounce(last_switch, &match_rule.rule_id, now) {
        return;
    }

    let Ok(config) = store::load_config(app) else {
        return;
    };

    let decision = evaluate_match(&config, match_rule, &matched_path, |repo, key| {
        crate::commands::profiles::read_local_git_config(repo, key)
    });

    let (git_root, profile_id) = match decision {
        AutoSwitchDecision::Disabled | AutoSwitchDecision::AlreadyApplied { .. } => return,
        AutoSwitchDecision::MissingRepository { path } => {
            eprintln!(
                "[auto-switch] no git root found for {}, skipping",
                path.display()
            );
            let _ = app.emit(
                "auto-switch-failed",
                format!(
                    "No git repository found containing the changed file \"{}\"",
                    path.display()
                ),
            );
            return;
        }
        AutoSwitchDecision::Apply {
            repo, profile_id, ..
        } => (repo, profile_id),
    };

    if let Err(error) = switch_profile_for_repo(app.clone(), profile_id.clone(), &git_root) {
        eprintln!("[auto-switch] failed to switch profile: {error}");
        let _ = app.emit("auto-switch-failed", error);
    } else {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        // Use the git root as the canonical "affected path" so the UI shows
        // the repo, not an internal temp file that happened to trigger the event.
        let event_path = git_root.to_string_lossy().to_string();
        set_last_auto_switch_event(
            match_rule.profile_id.clone(),
            event_path.clone(),
        );
        let _ = app.emit("auto-switch-triggered", AutoSwitchEvent {
            profile_id: match_rule.profile_id.clone(),
            path: event_path,
            occurred_at_epoch_ms: now_ms,
        });
        // Stamp last_triggered_at on the directory rule that fired
        if let Err(error) = store::update_config(app, |config| {
            if let Some(rule) = config
                .directory_rules
                .iter_mut()
                .find(|rule| rule.id == match_rule.rule_id)
            {
                rule.last_triggered_at = Some(now_ms);
            }
            Ok(())
        }) {
            eprintln!(
                "[auto-switch] failed to stamp last_triggered_at for rule {}: {error}",
                match_rule.rule_id
            );
        }
    }
}

pub(crate) fn select_best_rule<'a>(
    event_paths: &[PathBuf],
    rules: &'a [ResolvedRule],
) -> Option<(&'a ResolvedRule, PathBuf)> {
    let mut best_match: Option<(&ResolvedRule, PathBuf)> = None;
    for event_path in event_paths {
        if should_ignore_path(event_path) {
            continue;
        }
        let normalized = normalize_path(event_path).unwrap_or_else(|| event_path.clone());
        for rule in rules {
            if path_starts_with_ci(&normalized, &rule.root_path)
                && best_match.as_ref().is_none_or(|(current, _)| {
                    rule.root_path.components().count()
                        > current.root_path.components().count()
                })
            {
                best_match = Some((rule, normalized.clone()));
            }
        }
    }
    best_match
}

pub(crate) fn update_debounce(
    last_switch: &mut HashMap<String, Instant>,
    rule_id: &str,
    now: Instant,
) -> bool {
    if last_switch
        .get(rule_id)
        .is_some_and(|previous| now.duration_since(*previous) < DEBOUNCE_DURATION)
    {
        return true;
    }
    last_switch.insert(rule_id.to_string(), now);
    false
}

pub(crate) fn evaluate_match<F>(
    config: &crate::models::AppConfig,
    rule: &ResolvedRule,
    matched_path: &Path,
    mut read_local: F,
) -> AutoSwitchDecision
where
    F: FnMut(&Path, &str) -> Option<String>,
{
    if !config.settings.auto_switch {
        return AutoSwitchDecision::Disabled;
    }
    let Some(repo) = find_git_root(matched_path) else {
        return AutoSwitchDecision::MissingRepository {
            path: matched_path.to_path_buf(),
        };
    };

    if let Some(profile) = config.profiles.iter().find(|profile| profile.id == rule.profile_id) {
        let expected_ssh = profile.ssh_key_path.as_deref().and_then(|path| {
            (!path.is_empty()).then(|| {
                format!(
                    "ssh -i \"{}\" -o IdentitiesOnly=yes",
                    path.replace('\\', "/")
                )
            })
        });
        if read_local(&repo, "user.name").as_deref() == Some(profile.name.as_str())
            && read_local(&repo, "user.email").as_deref() == Some(profile.email.as_str())
            && read_local(&repo, "core.sshCommand") == expected_ssh
        {
            return AutoSwitchDecision::AlreadyApplied { repo };
        }
    }

    AutoSwitchDecision::Apply {
        rule_id: rule.rule_id.clone(),
        profile_id: rule.profile_id.clone(),
        repo,
    }
}

fn build_watcher_and_rules(
    tx: mpsc::Sender<notify::Result<Event>>,
    config: &crate::models::AppConfig,
) -> Result<(RecommendedWatcher, Vec<ResolvedRule>), notify::Error> {
    let mut watcher = RecommendedWatcher::new(
        move |result| {
            let _ = tx.send(result);
        },
        NotifyConfig::default(),
    )?;

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

        let normalized = normalize_path(&path).unwrap_or(path.clone());

        watcher.watch(&normalized, RecursiveMode::Recursive)?;
        resolved_rules.push(ResolvedRule {
            root_path: normalized,
            profile_id: rule.profile_id.clone(),
            rule_id: rule.id.clone(),
        });
    }

    Ok((watcher, resolved_rules))
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
        if let Some(stripped) = s.strip_prefix("\\\\?\\" ) {
            return Some(PathBuf::from(stripped));
        }
    }
    Some(canonical)
}

/// Case-insensitive path prefix check used on Windows;
/// falls back to the standard `starts_with` on other platforms.
/// Ensures the prefix ends on a path-separator boundary so that
/// `C:\work` does NOT match `C:\work2`.
fn path_starts_with_ci(path: &Path, prefix: &Path) -> bool {
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

    format!(
        "auto:{}::{}",
        config.settings.auto_switch,
        rules.join(";")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // ── should_ignore_path ───────────────────────────────────────

    #[test]
    fn ignores_node_modules() {
        assert!(should_ignore_path(Path::new("/project/node_modules/foo/index.js")));
        assert!(should_ignore_path(Path::new("C:\\project\\node_modules\\bar.js")));
    }

    #[test]
    fn ignores_git_objects() {
        assert!(should_ignore_path(Path::new("/repo/.git/objects/ab/cdef123")));
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
    fn allows_git_head_and_commit_msg() {
        // COMMIT_EDITMSG and HEAD at top level of .git should NOT be ignored
        assert!(!should_ignore_path(Path::new("/repo/.git/COMMIT_EDITMSG")));
        assert!(!should_ignore_path(Path::new("/repo/.git/HEAD")));
    }

    // ── path_starts_with_ci ──────────────────────────────────────

    #[test]
    fn prefix_exact_match() {
        let path = PathBuf::from("C:\\work\\project");
        let prefix = PathBuf::from("C:\\work\\project");
        assert!(path_starts_with_ci(&path, &prefix));
    }

    #[test]
    fn prefix_child_path() {
        let path = PathBuf::from("C:\\work\\project\\src\\main.rs");
        let prefix = PathBuf::from("C:\\work\\project");
        assert!(path_starts_with_ci(&path, &prefix));
    }

    #[test]
    fn prefix_rejects_partial_dir_name() {
        // C:\work should NOT match C:\work2
        let path = PathBuf::from("C:\\work2\\file.rs");
        let prefix = PathBuf::from("C:\\work");
        assert!(!path_starts_with_ci(&path, &prefix));
    }

    #[test]
    fn prefix_case_insensitive_on_windows() {
        let path = PathBuf::from("C:\\WORK\\PROJECT\\file.rs");
        let prefix = PathBuf::from("c:\\work\\project");
        #[cfg(windows)]
        assert!(path_starts_with_ci(&path, &prefix));
        #[cfg(not(windows))]
        {
            // On non-Windows, paths are case-sensitive
            let _ = (path, prefix);
        }
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

        assert_eq!(sig_a, sig_b, "signature should be stable regardless of rule order");
    }

    // ── last event store ─────────────────────────────────────────

    #[test]
    fn last_event_roundtrip() {
        set_last_auto_switch_event("profile-test-001".into(), "/test/path".into());
        let event = get_last_auto_switch_event();
        assert!(event.is_some());
        let event = event.unwrap();
        assert_eq!(event.profile_id, "profile-test-001");
        assert_eq!(event.path, "/test/path");
        assert!(event.occurred_at_epoch_ms > 0);
    }
}
