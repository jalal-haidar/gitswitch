use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use notify::{Config as NotifyConfig, Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use tauri::AppHandle;

use crate::commands::profiles::switch_profile_for_repo;
use crate::config::store;

#[derive(Clone, Debug)]
struct ResolvedRule {
    root_path: PathBuf,
    profile_id: String,
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
        if let Err(error) = run_watcher_loop(app) {
            eprintln!("[auto-switch] watcher loop stopped: {error}");
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

fn handle_event(app: &AppHandle, rules: &[ResolvedRule], event: &Event) {
    if rules.is_empty() {
        return;
    }

    let mut best_match: Option<(&ResolvedRule, PathBuf)> = None;

    for event_path in &event.paths {
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
        set_last_auto_switch_event(
            match_rule.profile_id.clone(),
            matched_path.to_string_lossy().to_string(),
        );
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
