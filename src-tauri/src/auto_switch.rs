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
struct ResolvedRule {
    root_path: PathBuf,
    profile_id: String,
    rule_id: String,
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

    let mut best_match: Option<(&ResolvedRule, PathBuf)> = None;

    for event_path in &event.paths {
        // Skip noisy temp/OS/build files — no need to switch for these
        if should_ignore_path(event_path) {
            continue;
        }

        let normalized_event_path = normalize_path(event_path).unwrap_or_else(|| event_path.to_path_buf());

        for rule in rules {
            if path_starts_with_ci(&normalized_event_path, &rule.root_path) {
                if let Some((current_best, _)) = best_match.as_ref() {
                    // Compare by component count (path depth), not byte length.
                    // Byte length breaks for Unicode path segments on Windows.
                    if rule.root_path.components().count() > current_best.root_path.components().count() {
                        best_match = Some((rule, normalized_event_path.clone()));
                    }
                } else {
                    best_match = Some((rule, normalized_event_path.clone()));
                }
            }
        }
    }

    let Some((match_rule, matched_path)) = best_match else {
        return;
    };

    // Debounce: if this rule already fired recently, skip.
    let now = Instant::now();
    if let Some(&prev) = last_switch.get(&match_rule.rule_id) {
        if now.duration_since(prev) < DEBOUNCE_DURATION {
            return;
        }
    }
    last_switch.insert(match_rule.rule_id.clone(), now);

    let Ok(config) = store::load_config(app) else {
        return;
    };

    if !config.settings.auto_switch {
        return;
    }

    // Find the actual git root from the triggering file path.
    // The rule's root_path may be a parent directory containing many repos;
    // we must apply the profile to the specific repo that owns the changed file.
    let git_root = match find_git_root(&matched_path) {
        Some(root) => root,
        None => {
            eprintln!(
                "[auto-switch] no git root found for {}, skipping",
                matched_path.display()
            );
            let _ = app.emit(
                "auto-switch-failed",
                format!(
                    "No git repository found containing the changed file \"{}\"",
                    matched_path.display()
                ),
            );
            return;
        }
    };

    // Skip only if this specific repo's local identity already fully matches the profile,
    // to avoid redundant git writes. A missing local config (None) means we still apply.
    // We check name, email AND ssh key so that an SSH-only identity change is not skipped.
    let profile_opt = config.profiles.iter().find(|p| p.id == match_rule.profile_id);
    if let Some(profile) = profile_opt {
        let local_name  = crate::commands::profiles::read_local_git_config(&git_root, "user.name");
        let local_email = crate::commands::profiles::read_local_git_config(&git_root, "user.email");
        let local_ssh   = crate::commands::profiles::read_local_git_config(&git_root, "core.sshCommand");

        // Build what the expected sshCommand would be for this profile.
        let expected_ssh = profile.ssh_key_path.as_deref().and_then(|p| {
            if p.is_empty() { None } else {
                Some(format!("ssh -i \"{}\" -o IdentitiesOnly=yes", p.replace('\\', "/")))
            }
        });

        let name_matches  = local_name.as_deref()  == Some(profile.name.as_str());
        let email_matches = local_email.as_deref() == Some(profile.email.as_str());
        let ssh_matches   = local_ssh == expected_ssh;

        if name_matches && email_matches && ssh_matches {
            // Repo-local config already fully matches — nothing to do.
            return;
        }
    }

    if let Err(error) = switch_profile_for_repo(app.clone(), match_rule.profile_id.clone(), &git_root) {
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
        if let Ok(mut cfg) = store::load_config(app) {
            if let Some(rule) = cfg.directory_rules.iter_mut().find(|r| r.id == match_rule.rule_id) {
                rule.last_triggered_at = Some(now_ms);
                if let Err(e) = store::save_config(app, &cfg) {
                    eprintln!("[auto-switch] failed to stamp last_triggered_at for rule {}: {e}", match_rule.rule_id);
                }
            }
        }
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

fn normalize_path(path: &Path) -> Option<PathBuf> {
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
