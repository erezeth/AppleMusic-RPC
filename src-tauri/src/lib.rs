mod app;
mod commands;
mod config;
mod discord;
mod media;
mod tray;

use app::AppState;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::set_config,
            commands::hide_to_tray,
            commands::quit
        ])
        .setup(|app| {
            set_up(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.app_handle().state::<Arc<AppState>>();
                if state.minimize_to_tray() {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Apple Music RPC");
}

fn set_up(handle: &tauri::AppHandle) -> tauri::Result<()> {
    let (config, config_warning) = config::Config::load(&config::config_dir());

    let (presence_tx, presence_rx) = tokio::sync::mpsc::unbounded_channel();
    let (status_tx, mut status_rx) = tokio::sync::mpsc::unbounded_channel();
    let retry = Arc::new(AtomicBool::new(config.discord_auto_reconnect));

    let state = Arc::new(AppState::new(
        handle.clone(),
        config.clone(),
        config_warning,
        presence_tx,
        retry.clone(),
    ));
    handle.manage(state.clone());

    discord::ipc::spawn(presence_rx, retry, status_tx);
    let status_state = state.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(status) = status_rx.recv().await {
            status_state.set_discord(status);
        }
    });

    let (media_tx, mut media_rx) = tokio::sync::mpsc::unbounded_channel();
    media::spawn(media_tx);
    let media_state = state.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(event) = media_rx.recv().await {
            media_state.on_media(event);
        }
    });

    tray::init(handle)?;

    if config.start_minimized {
        if let Some(window) = handle.get_webview_window("main") {
            let _ = window.hide();
        }
    }

    state.recompute();
    Ok(())
}
