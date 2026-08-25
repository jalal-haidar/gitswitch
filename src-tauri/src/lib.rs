mod auto_switch;
mod commands;
mod config;
mod errors;
mod git;
mod models;
mod path_security;
mod tray;

#[cfg(feature = "native-test-support")]
#[doc(hidden)]
pub mod native_test_support;

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
            if let Err(error) = commands::profiles::migrate_legacy_active_state(app.handle()) {
                eprintln!("[migration] failed to remove legacy active profile state: {error}");
            }
            if let Err(error) = commands::profiles::normalize_stored_profile_paths(app.handle()) {
                eprintln!("[migration] failed to canonicalize stored SSH key paths: {error}");
            }
            tray::setup_tray(app)?;
            auto_switch::start_auto_switch_watcher(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::profiles::get_profiles,
            commands::profiles::get_global_active_profile_id,
            commands::profiles::add_profile,
            commands::profiles::update_profile,
            commands::profiles::delete_profile,
            commands::profiles::switch_profile_globally,
            commands::profiles::export_profiles,
            commands::profiles::import_profiles,
            commands::profiles::has_global_snapshot,
            commands::profiles::restore_global_snapshot,
            commands::profiles::discard_global_snapshot,
            commands::profiles::apply_identity,
            commands::profiles::get_last_repo_activity,
            commands::detect::detect_identities,
            commands::rules::get_auto_switch_enabled,
            commands::rules::get_store_sensitive_in_keyring,
            commands::rules::set_store_sensitive_in_keyring,
            commands::rules::get_start_with_system,
            commands::rules::set_start_with_system,
            commands::rules::set_auto_switch_enabled,
            commands::rules::get_directory_rules,
            commands::rules::add_directory_rule,
            commands::rules::update_directory_rule,
            commands::rules::delete_directory_rule,
            commands::profiles::test_ssh_connection,
            commands::profiles::apply_profile_to_repo,
            commands::profiles::restore_repo_snapshot,
            commands::profiles::has_repo_snapshot,
            commands::profiles::discard_repo_snapshot,
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

#[cfg(test)]
mod security_config_tests {
    use serde_json::Value;

    #[test]
    fn production_csp_is_restrictive_and_remote_free() {
        let config: Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let security = &config["app"]["security"];
        let csp = security["csp"].as_object().expect("production CSP object");

        assert_eq!(csp["default-src"], "'self'");
        assert_eq!(csp["connect-src"], "ipc: http://ipc.localhost");
        assert_eq!(csp["object-src"], "'none'");
        assert_eq!(csp["frame-ancestors"], "'none'");
        let serialized = serde_json::to_string(csp).unwrap();
        assert!(!serialized.contains("https://"));
        assert!(!serialized.contains("http://") || serialized.contains("http://ipc.localhost"));
        assert!(!serialized.contains("unsafe-eval"));
        assert!(security["devCsp"].is_null());
        assert!(!include_str!("../../src/styles/index.css").contains("fonts.googleapis.com"));
    }

    #[test]
    fn main_window_capability_is_exact_and_scoped() {
        let capability: Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        let permissions = capability["permissions"].as_array().unwrap();
        let identifiers = permissions
            .iter()
            .map(|permission| {
                permission
                    .as_str()
                    .or_else(|| permission["identifier"].as_str())
                    .unwrap()
            })
            .collect::<Vec<_>>();

        assert_eq!(
            identifiers,
            vec![
                "core:app:allow-version",
                "core:event:allow-listen",
                "core:event:allow-unlisten",
                "core:window:allow-set-title",
                "dialog:allow-open",
                "dialog:allow-save",
                "updater:allow-check",
                "updater:allow-download-and-install",
                "opener:allow-open-url",
            ]
        );
        assert_eq!(
            permissions[8]["allow"][0]["url"],
            "https://git-scm.com/downloads"
        );
        assert_eq!(capability["local"], true);
        assert!(capability.get("remote").is_none());
    }
}
