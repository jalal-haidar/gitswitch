mod commands;
mod config;
mod errors;
mod fsutil;
mod git;
mod identity;
mod managed_block;
mod models;
mod paths;
mod ssh;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::profiles::get_profiles,
            commands::profiles::add_profile,
            commands::profiles::update_profile,
            commands::profiles::delete_profile,
            commands::profiles::switch_profile_globally,
            commands::profiles::apply_identity,
            commands::detect::detect_identities,
            commands::rules::get_directory_rules,
            commands::rules::add_directory_rule,
            commands::rules::remove_directory_rule,
            commands::rules::preview_directory,
            commands::ssh::generate_ssh_key,
            commands::ssh::list_ssh_keys,
            commands::ssh::read_public_key,
            commands::ssh::apply_ssh_alias,
            commands::ssh::test_ssh_connection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
