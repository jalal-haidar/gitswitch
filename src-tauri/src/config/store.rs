use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

use crate::errors::BackendError;
use crate::models::AppConfig;

const CONFIG_FILE_NAME: &str = "profiles.json";

pub fn config_dir(app_handle: &AppHandle) -> Result<PathBuf, BackendError> {
    let dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| BackendError::io_error(format!("Failed to get app config dir: {}", e)))?;
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn load_config(app_handle: &AppHandle) -> Result<AppConfig, BackendError> {
    let config_path = config_dir(app_handle)?.join(CONFIG_FILE_NAME);

    if !config_path.exists() {
        let default_config = AppConfig::default();
        save_config(app_handle, &default_config)?;
        return Ok(default_config);
    }

    let contents = fs::read_to_string(&config_path)?;
    serde_json::from_str(&contents).map_err(|e| {
        BackendError::io_error(format!("Failed to parse config file at {:?}", config_path))
            .with_details(e.to_string())
    })
}

pub fn save_config(app_handle: &AppHandle, config: &AppConfig) -> Result<(), BackendError> {
    let config_path = config_dir(app_handle)?.join(CONFIG_FILE_NAME);
    let contents = serde_json::to_string_pretty(config)
        .map_err(|e| BackendError::io_error(format!("Failed to serialize config: {}", e)))?;
    crate::fsutil::write_atomic(&config_path, &contents)?;
    Ok(())
}
