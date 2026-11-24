use crate::config::{config_dir, Config};
use crate::discord::{
    build_activity, client_id_configured, Activity, DiscordState, PresenceCommand,
};
use crate::media::{now_ms, MediaEvent, MediaState, Track};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::menu::MenuItemKind;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Clone, Serialize)]
pub struct Snapshot {
    pub config: Config,
    pub track: Option<Track>,
    pub media: MediaState,
    pub discord: DiscordState,
    pub config_warning: Option<String>,
}

struct Inner {
    config: Config,
    config_warning: Option<String>,
    track: Option<Track>,
    media: MediaState,
    discord: DiscordState,
    last_activity: Option<Activity>,
}

/// Single owner of app state. Everything the UI shows is derived from here;
/// the Discord worker is told what to display rather than reading state itself.
pub struct AppState {
    inner: Mutex<Inner>,
    app: AppHandle,
    presence_tx: UnboundedSender<PresenceCommand>,
    retry: Arc<AtomicBool>,
    /// Bumped on every recompute, so a pending delayed clear can tell whether
    /// the world moved on (a new track) and cancel itself.
    generation: AtomicU64,
}

impl AppState {
    pub fn new(
        app: AppHandle,
        config: Config,
        config_warning: Option<String>,
        presence_tx: UnboundedSender<PresenceCommand>,
        retry: Arc<AtomicBool>,
    ) -> Self {
        let discord = if !config.discord_enabled {
            DiscordState::Disabled
        } else if client_id_configured() {
            DiscordState::Connecting
        } else {
            DiscordState::NotConfigured
        };
        Self {
            inner: Mutex::new(Inner {
                config,
                config_warning,
                track: None,
                media: MediaState::NoAppleMusic,
                discord,
                last_activity: None,
            }),
            app,
            presence_tx,
            retry,
            generation: AtomicU64::new(0),
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let inner = self.inner.lock().unwrap();
        let mut track = inner.track.clone();
        if let Some(track) = &mut track {
            if !inner.config.show_album_art {
                track.art = None;
            }
        }
        Snapshot {
            config: inner.config.clone(),
            track,
            media: inner.media,
            discord: if inner.config.discord_enabled {
                inner.discord.clone()
            } else {
                DiscordState::Disabled
            },
            config_warning: inner.config_warning.clone(),
        }
    }

    pub fn on_media(self: &Arc<Self>, event: MediaEvent) {
        {
            let mut inner = self.inner.lock().unwrap();
            match event {
                MediaEvent::Unavailable => {
                    inner.media = MediaState::NoAppleMusic;
                    inner.track = None;
                }
                MediaEvent::Snapshot(Some(track)) => {
                    inner.media = if track.playing {
                        MediaState::Playing
                    } else {
                        MediaState::Paused
                    };
                    inner.track = Some(track);
                }
                MediaEvent::Snapshot(None) => {
                    inner.media = MediaState::Idle;
                    inner.track = None;
                }
            }
        }
        self.recompute();
    }

    pub fn set_discord(self: &Arc<Self>, state: DiscordState) {
        self.inner.lock().unwrap().discord = state;
        self.push_state();
    }

    pub fn update_config(self: &Arc<Self>, mut config: Config) -> Result<Snapshot, String> {
        config.validate();
        let autostart_changed = self.inner.lock().unwrap().config.start_with_windows
            != config.start_with_windows;

        config
            .save(&config_dir())
            .map_err(|e| format!("could not save settings: {e}"))?;

        if autostart_changed {
            use tauri_plugin_autostart::ManagerExt;
            let manager = self.app.autolaunch();
            let result = if config.start_with_windows {
                manager.enable()
            } else {
                manager.disable()
            };
            if let Err(err) = result {
                log::warn!("could not update Windows autostart: {err}");
            }
        }

        let auto_reconnect = config.discord_auto_reconnect;
        {
            let mut inner = self.inner.lock().unwrap();
            inner.config = config;
            inner.config_warning = None;
        }
        self.retry.store(auto_reconnect, Ordering::Relaxed);
        self.recompute();
        Ok(self.snapshot())
    }

    pub fn toggle_presence(self: &Arc<Self>) {
        let mut config = self.inner.lock().unwrap().config.clone();
        config.rich_presence_enabled = !config.rich_presence_enabled;
        if let Err(err) = self.update_config(config) {
            log::warn!("{err}");
        }
    }

    pub fn minimize_to_tray(&self) -> bool {
        self.inner.lock().unwrap().config.minimize_to_tray
    }

    pub fn start_minimized(&self) -> bool {
        self.inner.lock().unwrap().config.start_minimized
    }

    /// Recompute the desired presence and push a snapshot to the UI. Cheap;
    /// called on every event and settings change.
    pub fn recompute(self: &Arc<Self>) {
        let mut pending: Option<(u64, u64)> = None;
        {
            let mut inner = self.inner.lock().unwrap();
            let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
            match desired_of(&inner) {
                Some(activity) => {
                    if inner.last_activity.as_ref() != Some(&activity) {
                        inner.last_activity = Some(activity.clone());
                        self.send(PresenceCommand::Set(activity));
                    }
                }
                None => {
                    if inner.last_activity.is_some() {
                        pending = Some((generation, inner.config.clear_delay_ms));
                    }
                }
            }
        }

        match pending {
            None => {}
            Some((generation, 0)) => self.finish_clear(generation),
            Some((generation, delay_ms)) => {
                // Pause/track-change debounce: a new track arriving inside the
                // window bumps the generation and cancels this clear.
                let state = self.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    state.finish_clear(generation);
                });
            }
        }
        self.push_state();
    }

    fn finish_clear(&self, generation: u64) {
        {
            let mut inner = self.inner.lock().unwrap();
            if self.generation.load(Ordering::SeqCst) != generation {
                return;
            }
            if desired_of(&inner).is_some() {
                return;
            }
            inner.last_activity = None;
        }
        self.send(PresenceCommand::Clear);
        self.push_state();
    }

    fn send(&self, command: PresenceCommand) {
        if self.presence_tx.send(command).is_err() {
            log::warn!("Discord worker is gone; presence command dropped");
        }
    }

    fn push_state(&self) {
        let snapshot = self.snapshot();
        if let Err(err) = self.app.emit("state", &snapshot) {
            log::warn!("could not emit state: {err}");
        }
        self.sync_tray(&snapshot);
    }

    fn sync_tray(&self, snapshot: &Snapshot) {
        let Some(tray) = self.app.tray_by_id("main") else {
            return;
        };
        let Some(menu) = tray.menu() else {
            return;
        };
        if let Some(MenuItemKind::MenuItem(item)) = menu.get("tray_status") {
            let _ = item.set_text(format!("Discord: {}", snapshot.discord.label()));
        }
        if let Some(MenuItemKind::Check(item)) = menu.get("tray_toggle") {
            let _ = item.set_checked(snapshot.config.rich_presence_enabled);
        }
    }
}

/// What Discord should show right now. Pure, so the whole policy is testable
/// without a running Discord.
fn desired_of(inner: &Inner) -> Option<Activity> {
    let config = &inner.config;
    if !config.rich_presence_enabled || !config.discord_enabled || !config.auto_detect {
        return None;
    }
    let Some(track) = inner.track.as_ref() else {
        // Nothing playing: keep the last presence unless we clear on stop.
        return if config.clear_on_stop {
            None
        } else {
            inner.last_activity.clone()
        };
    };
    build_activity(track, config, now_ms())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inner(config: Config, playing: bool) -> Inner {
        Inner {
            config,
            config_warning: None,
            track: Some(Track {
                id: "id".into(),
                app_id: "AppleInc.AppleMusicWin_test!App".into(),
                title: "Song".into(),
                artist: Some("Artist".into()),
                album: None,
                art: None,
                duration_ms: Some(200_000),
                position_ms: 1_000,
                playing,
                position_at: 0,
            }),
            media: if playing {
                MediaState::Playing
            } else {
                MediaState::Paused
            },
            discord: DiscordState::Connected,
            last_activity: None,
        }
    }

    #[test]
    fn disabled_presence_shows_nothing() {
        let cfg = Config {
            rich_presence_enabled: false,
            ..Config::default()
        };
        assert!(desired_of(&inner(cfg, true)).is_none());
    }

    #[test]
    fn detection_off_shows_nothing() {
        let cfg = Config {
            auto_detect: false,
            ..Config::default()
        };
        assert!(desired_of(&inner(cfg, true)).is_none());
    }

    #[test]
    fn pause_clears_only_when_asked() {
        let keep = Config::default();
        assert!(desired_of(&inner(keep, false)).is_some());

        let clear = Config {
            clear_on_pause: true,
            ..Config::default()
        };
        assert!(desired_of(&inner(clear, false)).is_none());
    }

    #[test]
    fn stopped_keeps_last_presence_when_clear_on_stop_is_off() {
        let cfg = Config {
            clear_on_stop: false,
            ..Config::default()
        };
        let mut state = inner(cfg, false);
        state.track = None;
        state.last_activity = Some(Activity {
            details: "Song".into(),
            state: "Artist".into(),
            start_ms: None,
            end_ms: None,
            large_text: "Album".into(),
        });
        assert!(desired_of(&state).is_some());

        state.config.clear_on_stop = true;
        assert!(desired_of(&state).is_none());
    }
}
