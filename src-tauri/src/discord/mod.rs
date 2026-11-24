pub mod ipc;

use crate::config::Config;
use crate::media::{truncate_chars, Track};
use serde::Serialize;

/// Your Discord application's ID. Create one at discord.com/developers, then
/// upload a 512x512 icon as asset key `apple_music`.
/// Left as a placeholder on purpose: an empty/invalid ID just shows
/// "Not configured" instead of failing silently.
pub const DISCORD_CLIENT_ID: &str = "PASTE_DISCORD_APP_ID";
pub const APP_ASSET: &str = "apple_music";

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscordState {
    Disabled,
    NotConfigured,
    Connecting,
    Connected,
    Error(String),
}

impl DiscordState {
    pub fn label(&self) -> String {
        match self {
            Self::Disabled => "Disabled".into(),
            Self::NotConfigured => "Not configured".into(),
            Self::Connecting => "Connecting…".into(),
            Self::Connected => "Connected".into(),
            Self::Error(err) => format!("Error: {err}"),
        }
    }
}

pub fn client_id_configured() -> bool {
    DISCORD_CLIENT_ID.len() > 5 && !DISCORD_CLIENT_ID.starts_with("PASTE")
}

/// What Discord should show. Compared field-by-field to avoid redundant RPCs.
#[derive(Clone, Debug, PartialEq)]
pub struct Activity {
    pub details: String,
    pub state: String,
    pub start_ms: Option<u64>,
    pub end_ms: Option<u64>,
    pub large_text: String,
}

pub enum PresenceCommand {
    Set(Activity),
    Clear,
}

/// `None` means "no presence right now" (paused and the user clears on pause).
/// Discord caps details/state at 128 characters.
pub fn build_activity(track: &Track, cfg: &Config, now_ms: u64) -> Option<Activity> {
    if !track.playing && cfg.clear_on_pause {
        return None;
    }

    let details = if cfg.show_track_title {
        truncate_chars(&track.title, 128)
    } else {
        "Apple Music".to_string()
    };

    let mut parts: Vec<String> = Vec::new();
    if cfg.show_artist {
        if let Some(artist) = &track.artist {
            parts.push(artist.clone());
        }
    }
    if cfg.show_album {
        if let Some(album) = &track.album {
            parts.push(album.clone());
        }
    }
    let state = if parts.is_empty() {
        "Apple Music".to_string()
    } else {
        truncate_chars(&parts.join(" · "), 128)
    };

    // Discord animates the progress bar from these two timestamps, so no
    // per-second IPC traffic is needed.
    let (start_ms, end_ms) = match (track.playing, cfg.show_progress, track.duration_ms) {
        (true, true, Some(duration)) if duration > 0 => {
            let start = now_ms.saturating_sub(track.position_ms);
            (Some(start), Some(start + duration))
        }
        _ => (None, None),
    };

    Some(Activity {
        details,
        state,
        start_ms,
        end_ms,
        large_text: truncate_chars(track.album.as_deref().unwrap_or("Apple Music"), 128),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(playing: bool) -> Track {
        Track {
            id: "id".into(),
            app_id: "AppleInc.AppleMusicWin_test!App".into(),
            title: "Song".into(),
            artist: Some("Artist".into()),
            album: Some("Album".into()),
            art: None,
            duration_ms: Some(200_000),
            position_ms: 45_000,
            playing,
            position_at: 1_000_000,
        }
    }

    #[test]
    fn playing_with_progress_gets_timestamps() {
        let a = build_activity(&track(true), &Config::default(), 1_000_000).unwrap();
        assert_eq!(a.details, "Song");
        assert_eq!(a.state, "Artist · Album");
        assert_eq!(a.start_ms, Some(955_000));
        assert_eq!(a.end_ms, Some(1_155_000));
    }

    #[test]
    fn paused_without_clear_on_pause_drops_timestamps() {
        let cfg = Config::default();
        let a = build_activity(&track(false), &cfg, 1_000_000).unwrap();
        assert!(a.start_ms.is_none());
        assert!(a.end_ms.is_none());
    }

    #[test]
    fn paused_with_clear_on_pause_has_no_presence() {
        let cfg = Config {
            clear_on_pause: true,
            ..Config::default()
        };
        assert!(build_activity(&track(false), &cfg, 1_000_000).is_none());
    }

    #[test]
    fn hidden_fields_fall_back_to_app_name() {
        let cfg = Config {
            show_track_title: false,
            show_artist: false,
            show_album: false,
            ..Config::default()
        };
        let a = build_activity(&track(true), &cfg, 1_000_000).unwrap();
        assert_eq!(a.details, "Apple Music");
        assert_eq!(a.state, "Apple Music");
    }

    #[test]
    fn long_titles_are_truncated_to_discord_limit() {
        let mut t = track(true);
        t.title = "🎵".repeat(300);
        let a = build_activity(&t, &Config::default(), 1_000_000).unwrap();
        assert_eq!(a.details.chars().count(), 128);
    }
}
