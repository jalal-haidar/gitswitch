use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;
use uuid::Uuid;

use crate::auto_switch;
use crate::config::store;
use crate::models::DirectoryRule;

fn canonical_rule_path(raw: &str) -> Result<String, String> {
    let path = std::path::Path::new(raw.trim());
    if raw.trim().is_empty() {
        return Err("Rule path is required".to_string());
    }
    if !path.exists() {
        return Err(format!("Directory does not exist: {}", path.display()));
    }
    if !path.is_dir() {
        return Err(format!("Path is not a directory: {}", path.display()));
    }
    auto_switch::normalize_path(path)
        .map(|path| path.to_string_lossy().into_owned())
        .ok_or_else(|| format!("Could not canonicalize directory: {}", path.display()))
}

fn rule_paths_equal(left: &str, right: &str) -> bool {
    let left = auto_switch::normalize_path(std::path::Path::new(left))
        .unwrap_or_else(|| std::path::PathBuf::from(left));
    let right = auto_switch::normalize_path(std::path::Path::new(right))
        .unwrap_or_else(|| std::path::PathBuf::from(right));
    auto_switch::paths_equal(&left, &right)
}

#[tauri::command]
pub fn get_auto_switch_enabled(app: AppHandle) -> Result<bool, String> {
    let config = store::load_config(&app).map_err(|e| e.to_string())?;
    Ok(config.settings.auto_switch)
}

#[tauri::command]
pub fn get_store_sensitive_in_keyring(app: AppHandle) -> Result<bool, String> {
    let config = store::load_config(&app).map_err(|e| e.to_string())?;
    Ok(config.settings.store_sensitive_in_keyring)
}

#[tauri::command]
pub fn set_store_sensitive_in_keyring(app: AppHandle, enabled: bool) -> Result<bool, String> {
    store::update_config(&app, |config| {
        config.settings.store_sensitive_in_keyring = enabled;
        Ok(enabled)
    })
}

#[tauri::command]
pub fn get_start_with_system(app: AppHandle) -> Result<bool, String> {
    let config = store::load_config(&app).map_err(|e| e.to_string())?;
    Ok(config.settings.start_with_system)
}

#[tauri::command]
pub fn set_start_with_system(app: AppHandle, enabled: bool) -> Result<bool, String> {
    store::update_config(&app, |config| {
        config.settings.start_with_system = enabled;
        Ok(())
    })?;

    // Enable/disable OS autostart
    let manager = app.autolaunch();
    if enabled {
        manager
            .enable()
            .map_err(|e| format!("Failed to enable autostart: {}", e))?;
    } else {
        manager
            .disable()
            .map_err(|e| format!("Failed to disable autostart: {}", e))?;
    }

    Ok(enabled)
}

#[tauri::command]
pub fn set_auto_switch_enabled(app: AppHandle, enabled: bool) -> Result<bool, String> {
    store::update_config(&app, |config| {
        config.settings.auto_switch = enabled;
        Ok(enabled)
    })
}

#[tauri::command]
pub fn get_directory_rules(app: AppHandle) -> Result<Vec<DirectoryRule>, String> {
    let config = store::load_config(&app).map_err(|error| error.to_string())?;
    if config
        .directory_rules
        .iter()
        .all(|rule| !rule.id.is_empty())
    {
        return Ok(config.directory_rules);
    }

    store::update_config(&app, |config| {
        for rule in &mut config.directory_rules {
            if rule.id.is_empty() {
                rule.id = Uuid::new_v4().to_string();
            }
        }
        Ok(config.directory_rules.clone())
    })
}

#[tauri::command]
pub fn add_directory_rule(
    app: AppHandle,
    mut rule: DirectoryRule,
) -> Result<DirectoryRule, String> {
    let path = canonical_rule_path(&rule.path)?;

    if rule.id.is_empty() {
        rule.id = Uuid::new_v4().to_string();
    }
    rule.path = path;

    store::update_config(&app, |config| {
        if !config.profiles.iter().any(|p| p.id == rule.profile_id) {
            return Err("Selected profile does not exist".to_string());
        }
        if config
            .directory_rules
            .iter()
            .any(|existing| rule_paths_equal(&existing.path, &rule.path))
        {
            return Err("A directory rule already exists for this path".to_string());
        }
        config.directory_rules.push(rule.clone());
        Ok(rule.clone())
    })
}

#[tauri::command]
pub fn update_directory_rule(app: AppHandle, rule: DirectoryRule) -> Result<DirectoryRule, String> {
    if rule.id.trim().is_empty() {
        return Err("Rule id is required".to_string());
    }

    let path = canonical_rule_path(&rule.path)?;

    store::update_config(&app, |config| {
        if !config.profiles.iter().any(|p| p.id == rule.profile_id) {
            return Err("Selected profile does not exist".to_string());
        }
        if config
            .directory_rules
            .iter()
            .any(|existing| existing.id != rule.id && rule_paths_equal(&existing.path, &path))
        {
            return Err("A directory rule already exists for this path".to_string());
        }

        let existing = config
            .directory_rules
            .iter_mut()
            .find(|existing| existing.id == rule.id)
            .ok_or_else(|| format!("Directory rule not found: {}", rule.id))?;
        existing.path = path.clone();
        existing.profile_id = rule.profile_id.clone();

        Ok(DirectoryRule {
            id: rule.id.clone(),
            path: path.clone(),
            profile_id: rule.profile_id.clone(),
            last_triggered_at: rule.last_triggered_at,
        })
    })
}

#[tauri::command]
pub fn delete_directory_rule(app: AppHandle, id: String) -> Result<(), String> {
    store::update_config(&app, |config| {
        let before = config.directory_rules.len();
        config.directory_rules.retain(|rule| rule.id != id);
        if config.directory_rules.len() == before {
            return Err(format!("Directory rule not found: {id}"));
        }
        Ok(())
    })
}

#[tauri::command]
pub fn get_theme(app: AppHandle) -> Result<String, String> {
    let config = store::load_config(&app).map_err(|e| e.to_string())?;
    Ok(config.settings.theme)
}

#[tauri::command]
pub fn set_theme(app: AppHandle, theme: String) -> Result<String, String> {
    let valid = ["system", "dark", "light"];
    if !valid.contains(&theme.as_str()) {
        return Err(format!(
            "Invalid theme '{}'. Use: system, dark, light",
            theme
        ));
    }
    store::update_config(&app, |config| {
        config.settings.theme = theme.clone();
        Ok(theme.clone())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn test_root() -> std::path::PathBuf {
        let suffix = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "gitswitch-rule-path-{}-{suffix}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("child")).unwrap();
        root
    }

    #[test]
    fn canonical_rule_path_collapses_equivalent_existing_paths() {
        let root = test_root();
        let alias = root.join("child").join("..");
        let canonical = canonical_rule_path(root.to_string_lossy().as_ref()).unwrap();
        let canonical_alias = canonical_rule_path(alias.to_string_lossy().as_ref()).unwrap();

        assert_eq!(canonical, canonical_alias);
        assert!(rule_paths_equal(&canonical, &canonical_alias));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn canonical_rule_path_rejects_non_directories() {
        let root = test_root();
        let file = root.join("file.txt");
        std::fs::write(&file, "content").unwrap();

        assert!(canonical_rule_path(file.to_string_lossy().as_ref())
            .unwrap_err()
            .contains("not a directory"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
