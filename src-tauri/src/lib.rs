mod commands;
mod config;
mod models;
mod errors;
mod auto_switch;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            auto_switch::start_auto_switch_watcher(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::profiles::get_profiles,
            commands::profiles::get_active_profile_id,
            commands::profiles::add_profile,
            commands::profiles::update_profile,
            commands::profiles::delete_profile,
            commands::profiles::switch_profile_globally,
            commands::profiles::snapshot_global_git_config,
            commands::profiles::restore_global_git_config,
            commands::profiles::apply_identity,
            commands::profiles::set_active_profile,
            commands::detect::detect_identities,
            commands::rules::get_auto_switch_enabled,
            commands::rules::get_store_sensitive_in_keyring,
            commands::rules::set_store_sensitive_in_keyring,
            commands::rules::set_auto_switch_enabled,
            commands::rules::get_last_auto_switch_event,
            commands::rules::get_directory_rules,
            commands::rules::add_directory_rule,
            commands::rules::update_directory_rule,
            commands::rules::delete_directory_rule,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
