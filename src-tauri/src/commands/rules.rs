use tauri::AppHandle;
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
pub fn set_auto_switch_enabled(app: AppHandle, enabled: bool) -> Result<bool, String> {
    let mut config = store::load_config(&app).map_err(|e| e.to_string())?;
    config.settings.auto_switch = enabled;
    store::save_config(&app, &config).map_err(|e| e.to_string())?;
    Ok(config.settings.auto_switch)
}

#[tauri::command]
pub fn get_last_auto_switch_event() -> Result<Option<auto_switch::AutoSwitchEvent>, String> {
    Ok(auto_switch::get_last_auto_switch_event())
}

#[tauri::command]
pub fn get_directory_rules(app: AppHandle) -> Result<Vec<DirectoryRule>, String> {
    let mut config = store::load_config(&app).map_err(|e| e.to_string())?;
    let mut changed = false;

    for rule in &mut config.directory_rules {
        if rule.id.is_empty() {
            rule.id = Uuid::new_v4().to_string();
            changed = true;
        }
    }

    if changed {
        store::save_config(&app, &config).map_err(|e| e.to_string())?;
    }

    Ok(config.directory_rules)
}

#[tauri::command]
pub fn add_directory_rule(app: AppHandle, mut rule: DirectoryRule) -> Result<DirectoryRule, String> {
    let mut config = store::load_config(&app).map_err(|e| e.to_string())?;

    let path = rule.path.trim().to_string();
    if path.is_empty() {
        return Err("Rule path is required".to_string());
    }

    let has_profile = config.profiles.iter().any(|p| p.id == rule.profile_id);
    if !has_profile {
        return Err("Selected profile does not exist".to_string());
    }

    let duplicate = config
        .directory_rules
        .iter()
        .any(|r| r.path.eq_ignore_ascii_case(&path) && r.profile_id == rule.profile_id);
    if duplicate {
        return Err("A directory rule with the same path and profile already exists".to_string());
    }

    if rule.id.is_empty() {
        rule.id = Uuid::new_v4().to_string();
    }
    rule.path = path;

    config.directory_rules.push(rule.clone());
    store::save_config(&app, &config).map_err(|e| e.to_string())?;

    Ok(rule)
}

#[tauri::command]
pub fn update_directory_rule(app: AppHandle, rule: DirectoryRule) -> Result<DirectoryRule, String> {
    let mut config = store::load_config(&app).map_err(|e| e.to_string())?;

    if rule.id.trim().is_empty() {
        return Err("Rule id is required".to_string());
    }

    let path = rule.path.trim().to_string();
    if path.is_empty() {
        return Err("Rule path is required".to_string());
    }

    let has_profile = config.profiles.iter().any(|p| p.id == rule.profile_id);
    if !has_profile {
        return Err("Selected profile does not exist".to_string());
    }

    let duplicate = config.directory_rules.iter().any(|existing| {
        existing.id != rule.id
            && existing.path.eq_ignore_ascii_case(&path)
            && existing.profile_id == rule.profile_id
    });
    if duplicate {
        return Err("A directory rule with the same path and profile already exists".to_string());
    }

    let mut found = false;
    for existing in &mut config.directory_rules {
        if existing.id == rule.id {
            existing.path = path.clone();
            existing.profile_id = rule.profile_id.clone();
            found = true;
            break;
        }
    }

    if !found {
        return Err("Directory rule not found".to_string());
    }

    store::save_config(&app, &config).map_err(|e| e.to_string())?;

    Ok(DirectoryRule {
        id: rule.id,
        path,
        profile_id: rule.profile_id,
    })
}

#[tauri::command]
pub fn delete_directory_rule(app: AppHandle, id: String) -> Result<(), String> {
    let mut config = store::load_config(&app).map_err(|e| e.to_string())?;

    let before = config.directory_rules.len();
    config.directory_rules.retain(|r| r.id != id);

    if config.directory_rules.len() == before {
        return Err("Directory rule not found".to_string());
    }

    store::save_config(&app, &config).map_err(|e| e.to_string())?;
    Ok(())
}
