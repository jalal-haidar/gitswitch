use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;
use uuid::Uuid;

use crate::auto_switch;
use crate::config::store;
use crate::models::DirectoryRule;

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
pub fn get_last_auto_switch_event() -> Result<Option<auto_switch::AutoSwitchEvent>, String> {
    Ok(auto_switch::get_last_auto_switch_event())
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
    let path = rule.path.trim().to_string();
    if path.is_empty() {
        return Err("Rule path is required".to_string());
    }

    if !std::path::Path::new(&path).exists() {
        return Err(format!("Directory does not exist: {}", path));
    }
    if !std::path::Path::new(&path).is_dir() {
        return Err(format!("Path is not a directory: {}", path));
    }

    if rule.id.is_empty() {
        rule.id = Uuid::new_v4().to_string();
    }
    rule.path = path;

    store::update_config(&app, |config| {
        if !config.profiles.iter().any(|p| p.id == rule.profile_id) {
            return Err("Selected profile does not exist".to_string());
        }
        if config.directory_rules.iter().any(|existing| {
            existing.path.eq_ignore_ascii_case(&rule.path) && existing.profile_id == rule.profile_id
        }) {
            return Err(
                "A directory rule with the same path and profile already exists".to_string(),
            );
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

    let path = rule.path.trim().to_string();
    if path.is_empty() {
        return Err("Rule path is required".to_string());
    }

    if !std::path::Path::new(&path).exists() {
        return Err(format!("Directory does not exist: {}", path));
    }
    if !std::path::Path::new(&path).is_dir() {
        return Err(format!("Path is not a directory: {}", path));
    }

    store::update_config(&app, |config| {
        if !config.profiles.iter().any(|p| p.id == rule.profile_id) {
            return Err("Selected profile does not exist".to_string());
        }
        if config.directory_rules.iter().any(|existing| {
            existing.id != rule.id
                && existing.path.eq_ignore_ascii_case(&path)
                && existing.profile_id == rule.profile_id
        }) {
            return Err(
                "A directory rule with the same path and profile already exists".to_string(),
            );
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
