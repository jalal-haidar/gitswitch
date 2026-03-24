use tauri::{AppHandle, Manager, Emitter};
use tauri::menu::{Menu, MenuItem, CheckMenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use image::{ImageBuffer, ImageEncoder, Rgba, RgbaImage};

use crate::config::store;

/// Parse a hex color string (e.g., "#7C3AED") into RGB components
fn parse_hex_color(hex: &str) -> (u8, u8, u8) {
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(124);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(58);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(237);
        (r, g, b)
    } else {
        // Default purple if parsing fails
        (124, 58, 237)
    }
}

/// Generate a colored tray icon (32x32 circle on transparent background)
fn generate_colored_icon(color: &str) -> Option<tauri::image::Image<'static>> {
    let size = 32u32;
    let radius = 12.0f32;
    let center = (size / 2) as f32;
    
    let (r, g, b) = parse_hex_color(color);
    let mut img: RgbaImage = ImageBuffer::new(size, size);
    
    // Draw a filled circle
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let distance = (dx * dx + dy * dy).sqrt();
            
            if distance <= radius {
                // Filled circle with profile color
                img.put_pixel(x, y, Rgba([r, g, b, 255]));
            } else if distance <= radius + 1.0 {
                // Anti-aliasing edge
                let alpha = ((radius + 1.0 - distance) * 255.0) as u8;
                img.put_pixel(x, y, Rgba([r, g, b, alpha]));
            }
            // else: transparent (default)
        }
    }
    
    // Encode to PNG bytes
    let mut png_bytes = Vec::new();
    if image::codecs::png::PngEncoder::new(&mut png_bytes)
        .write_image(&img, size, size, image::ColorType::Rgba8.into())
        .is_err()
    {
        return None;
    }
    
    tauri::image::Image::from_bytes(&png_bytes).ok()
}

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

    // Update tooltip and icon based on active profile
    let config = store::load_config(app).unwrap_or_default();
    let tooltip = if let Some(active_id) = &config.active_profile_id {
        if let Some(p) = config.profiles.iter().find(|p| &p.id == active_id) {
            // Update icon with profile color
            if let Some(icon) = generate_colored_icon(&p.color) {
                let _ = tray.set_icon(Some(icon));
            }
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
    
    // Get initial icon and tooltip from active profile
    let (initial_icon, initial_tooltip) = if let Some(active_id) = &config.active_profile_id {
        if let Some(p) = config.profiles.iter().find(|p| &p.id == active_id) {
            let icon = generate_colored_icon(&p.color)
                .unwrap_or_else(|| app.default_window_icon().unwrap().clone());
            (icon, format!("GitSwitch — {}", p.label))
        } else {
            (app.default_window_icon().unwrap().clone(), "GitSwitch".to_string())
        }
    } else {
        (app.default_window_icon().unwrap().clone(), "GitSwitch".to_string())
    };

    TrayIconBuilder::with_id("main-tray")
        .icon(initial_icon)
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
            if let Some(s) = id.strip_prefix("switch-") {
                let profile_id = s.to_string();
                match crate::commands::profiles::switch_profile_globally(app.clone(), profile_id) {
                Ok(()) => {
                    refresh_tray(app);
                    // Tell the frontend to re-fetch profiles so UI + title bar update
                    let _ = app.emit("profiles-changed", ());
                }
                Err(e) => eprintln!("[tray] switch error: {e}"),
                }
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
