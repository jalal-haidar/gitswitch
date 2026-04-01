mod commands;
mod config;
mod models;
mod errors;
mod auto_switch;
mod git;
mod tray;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let tauri_app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .setup(|app| {
            auto_switch::start_auto_switch_watcher(app.handle().clone());
            tray::setup_tray(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::profiles::get_profiles,
            commands::profiles::get_active_profile_id,
            commands::profiles::add_profile,
            commands::profiles::update_profile,
            commands::profiles::delete_profile,
            commands::profiles::switch_profile_globally,
            commands::profiles::export_profiles,
            commands::profiles::import_profiles,
            commands::profiles::snapshot_global_git_config,
            commands::profiles::restore_global_git_config,
            commands::profiles::apply_identity,
            commands::profiles::set_active_profile,
            commands::detect::detect_identities,
            commands::rules::get_auto_switch_enabled,
            commands::rules::get_store_sensitive_in_keyring,
            commands::rules::set_store_sensitive_in_keyring,
            commands::rules::get_start_with_system,
            commands::rules::set_start_with_system,
            commands::rules::set_auto_switch_enabled,
            commands::rules::get_last_auto_switch_event,
            commands::rules::get_directory_rules,
            commands::rules::add_directory_rule,
            commands::rules::update_directory_rule,
            commands::rules::delete_directory_rule,
            commands::profiles::test_ssh_connection,
            commands::profiles::apply_profile_to_repo,
            commands::profiles::restore_repo_snapshot,
            commands::profiles::has_repo_snapshot,
            commands::profiles::get_repo_local_config,
            commands::rules::get_theme,
            commands::rules::set_theme,
            commands::detect::scan_repos,
        ])
        .on_window_event(|window, event| {
            // Clicking the X hides the window instead of destroying it.
            // The app keeps running; use "Quit" in the tray menu to exit.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .build(tauri::generate_context!());

    match tauri_app {
        Ok(app) => {
            app.run(|_app, event| {
                // Prevent the process from exiting when no windows are visible.
                if let tauri::RunEvent::ExitRequested { api, .. } = event {
                    api.prevent_exit();
                }
            });
        }
        Err(e) => {
            eprintln!("error while running tauri application: {}", e);
        }
    }
}
