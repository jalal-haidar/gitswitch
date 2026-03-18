use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use notify::{Config as NotifyConfig, Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::commands::profiles::switch_profile_for_repo;
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
    last_event_store().lock().ok().and_then(|guard| (*guard).clone())
}

fn set_last_auto_switch_event(profile_id: String, path: String) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);

    if let Ok(mut guard) = last_event_store().lock() {
        *guard = Some(AutoSwitchEvent {
            profile_id,
            path,
            occurred_at_epoch_ms: now,
        });
    }
}

pub fn start_auto_switch_watcher(app: AppHandle) {
    thread::spawn(move || {
        if let Err(error) = run_watcher_loop(app.clone()) {
            eprintln!("[auto-switch] watcher loop stopped: {error}");
            // Inform the frontend so it can surface a warning to the user
            let _ = app.emit("auto-switch-error", error);
        }
    });
}

fn run_watcher_loop(app: AppHandle) -> Result<(), String> {
    let (tx, rx) = mpsc::channel();

    let mut watcher: Option<RecommendedWatcher> = None;
    let mut config_signature = String::new();
    let mut resolved_rules: Vec<ResolvedRule> = Vec::new();

    loop {
        if let Ok(config) = store::load_config(&app) {
            let next_signature = build_signature(&config);

            if next_signature != config_signature {
                config_signature = next_signature;

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
                handle_event(&app, &resolved_rules, &event);
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

fn handle_event(app: &AppHandle, rules: &[ResolvedRule], event: &Event) {
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
            if normalized_event_path.starts_with(&rule.root_path) {
                if let Some((current_best, _)) = best_match.as_ref() {
                    if rule.root_path.as_os_str().len() > current_best.root_path.as_os_str().len() {
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

    let Ok(config) = store::load_config(app) else {
        return;
    };

    if !config.settings.auto_switch {
        return;
    }

    if config.active_profile_id.as_deref() == Some(match_rule.profile_id.as_str()) {
        return;
    }

    if let Err(error) = switch_profile_for_repo(app.clone(), match_rule.profile_id.clone(), &match_rule.root_path) {
        eprintln!("[auto-switch] failed to switch profile: {error}");
    } else {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let event_path = matched_path.to_string_lossy().to_string();
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
                let _ = store::save_config(app, &cfg);
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
    fs::canonicalize(path).ok()
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
