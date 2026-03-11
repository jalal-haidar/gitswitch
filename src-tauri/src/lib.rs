mod commands;
mod config;
mod models;
mod errors;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::profiles::get_profiles,
            commands::profiles::add_profile,
            commands::profiles::update_profile,
            commands::profiles::delete_profile,
            commands::profiles::switch_profile_globally,
            commands::profiles::apply_identity,
            commands::detect::detect_identities,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
