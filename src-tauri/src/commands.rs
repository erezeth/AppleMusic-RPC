use crate::app::{AppState, Snapshot};
use crate::config::Config;
use std::sync::Arc;
use tauri::Manager;

#[tauri::command]
pub fn get_state(state: tauri::State<'_, Arc<AppState>>) -> Snapshot {
    state.snapshot()
}

#[tauri::command]
pub fn set_config(
    state: tauri::State<'_, Arc<AppState>>,
    config: Config,
) -> Result<Snapshot, String> {
    state.update_config(config)
}

/// Called by the dashboard's "Minimize to tray" button. A command instead of
/// window APIs keeps the capability list short.
#[tauri::command]
pub fn hide_to_tray(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

#[tauri::command]
pub fn quit(app: tauri::AppHandle) {
    app.exit(0);
}
