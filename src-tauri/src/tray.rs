use crate::app::AppState;
use std::sync::Arc;
use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager};

pub const TRAY_ID: &str = "main";

pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Dashboard", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let status = MenuItem::with_id(app, "tray_status", "Discord: …", false, None::<&str>)?;
    let toggle = CheckMenuItem::with_id(
        app,
        "tray_toggle",
        "Rich Presence",
        true,
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Exit", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[&show, &status, &toggle, &separator, &settings, &quit],
    )?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("Apple Music RPC");
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.on_menu_event(on_menu).build(app)?;
    Ok(())
}

fn on_menu(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        "show" => show_dashboard(app),
        "settings" => {
            show_dashboard(app);
            let _ = app.emit("navigate", "settings");
        }
        "tray_toggle" => {
            if let Some(state) = app.try_state::<Arc<AppState>>() {
                state.toggle_presence();
            }
        }
        "quit" => app.exit(0),
        _ => {}
    }
}

pub fn show_dashboard(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
