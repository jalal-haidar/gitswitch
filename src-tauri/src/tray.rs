use tauri::{AppHandle, Manager, Emitter};
use tauri::menu::{Menu, MenuItem, CheckMenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

use crate::config::store;

/// Build or rebuild the tray context menu from the current config.
pub fn build_tray_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let config = store::load_config(app).unwrap_or_default();
    let active_id = config.active_profile_id.as_deref().unwrap_or("");

    let menu = Menu::new(app)?;

    // Non-clickable header showing the active profile (or a placeholder)
    let header_text = if let Some(p) = config.profiles.iter().find(|p| p.id == active_id) {
        format!("Active: {}", p.label)
    } else {
        "No active profile".to_string()
    };
    let header = MenuItem::with_id(app, "header", header_text, false, None::<&str>)?;
    menu.append(&header)?;

    menu.append(&PredefinedMenuItem::separator(app)?)?;

    // One CheckMenuItem per profile — checked when it's the active one
    for profile in &config.profiles {
        let is_active = profile.id == active_id;
        let item = CheckMenuItem::with_id(
            app,
            format!("switch-{}", profile.id),
            &profile.label,
            true,
            is_active,
            None::<&str>,
        )?;
        menu.append(&item)?;
    }

    if !config.profiles.is_empty() {
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }

    let show = MenuItem::with_id(app, "show", "Show GitSwitch", true, None::<&str>)?;
    menu.append(&show)?;

    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    menu.append(&quit)?;

    Ok(menu)
}

/// Update the existing tray menu and tooltip to reflect current state.
pub fn refresh_tray(app: &AppHandle) {
    let Some(tray) = app.tray_by_id("main-tray") else {
        return;
    };

    if let Ok(menu) = build_tray_menu(app) {
        let _ = tray.set_menu(Some(menu));
    }

    // Update tooltip
    let config = store::load_config(app).unwrap_or_default();
    let tooltip = if let Some(active_id) = &config.active_profile_id {
        if let Some(p) = config.profiles.iter().find(|p| &p.id == active_id) {
            format!("GitSwitch — {}", p.label)
        } else {
            "GitSwitch".to_string()
        }
    } else {
        "GitSwitch".to_string()
    };
    let _ = tray.set_tooltip(Some(tooltip));
}

/// Set up the system tray. Should be called once from app setup.
pub fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let menu = build_tray_menu(app.handle())?;

    let config = store::load_config(app.handle()).unwrap_or_default();
    let initial_tooltip = if let Some(active_id) = &config.active_profile_id {
        if let Some(p) = config.profiles.iter().find(|p| &p.id == active_id) {
            format!("GitSwitch — {}", p.label)
        } else {
            "GitSwitch".to_string()
        }
    } else {
        "GitSwitch".to_string()
    };

    TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .tooltip(initial_tooltip)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            // Left-click → show & focus the main window
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| {
            on_menu_event(app, event.id().as_ref());
        })
        .build(app)?;

    Ok(())
}

fn on_menu_event(app: &AppHandle, id: &str) {
    match id {
        "show" => show_main_window(app),
        "quit" => app.exit(0),
        id if id.starts_with("switch-") => {
            let profile_id = id.strip_prefix("switch-").unwrap().to_string();
            match crate::commands::profiles::switch_profile_globally(app.clone(), profile_id) {
                Ok(()) => {
                    refresh_tray(app);
                    // Tell the frontend to re-fetch profiles so UI + title bar update
                    let _ = app.emit("profiles-changed", ());
                }
                Err(e) => eprintln!("[tray] switch error: {e}"),
            }
        }
        _ => {}
    }
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
