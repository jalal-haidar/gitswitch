use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

use crate::models::{AppConfig, GitProfile};

const CONFIG_FILE_NAME: &str = "profiles.json";

fn get_config_path(app_handle: &AppHandle) -> Result<PathBuf> {
    let app_dir = app_handle
        .path()
        .app_config_dir()
        .context("Failed to get app config dir")?;
    
    if !app_dir.exists() {
        fs::create_dir_all(&app_dir).context("Failed to create app config directory")?;
    }
    
    Ok(app_dir.join(CONFIG_FILE_NAME))
}

pub fn load_config(app_handle: &AppHandle) -> Result<AppConfig> {
    let config_path = get_config_path(app_handle)?;
    
    if !config_path.exists() {
        let default_config = AppConfig::default();
        save_config(app_handle, &default_config)?;
        return Ok(default_config);
    }
    
    let contents = fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read config file at {:?}", config_path))?;
        
    let config: AppConfig = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse config file at {:?}", config_path))?;
        
    Ok(config)
}

pub fn save_config(app_handle: &AppHandle, config: &AppConfig) -> Result<()> {
    let config_path = get_config_path(app_handle)?;
    let contents = serde_json::to_string_pretty(config)
        .context("Failed to serialize AppConfig")?;
        
    fs::write(&config_path, contents)
        .with_context(|| format!("Failed to write config file to {:?}", config_path))?;
        
    Ok(())
}
