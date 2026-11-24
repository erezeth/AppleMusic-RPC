pub mod smtc;

use base64::Engine;
use serde::Serialize;
use tokio::sync::mpsc::UnboundedSender;

/// Apple Music (the Store app). Its AUMID is
/// `AppleInc.AppleMusicWin_nzyj5cx40ttqa!App` — the hash changes between
/// installs, so only the stable prefix is matched.
pub const APPLE_MUSIC_AUMID_PREFIX: &str = "AppleInc.AppleMusicWin";

/// One observation from Windows SMTC. `position_ms` is the elapsed time when
/// `position_at` was sampled; nobody polls, consumers extrapolate.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Track {
    pub id: String,
    pub app_id: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// `data:` URL, ready for an `<img src>`. Dropped from the snapshot when
    /// the user hides artwork.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub art: Option<String>,
    pub duration_ms: Option<u64>,
    pub position_ms: u64,
    pub playing: bool,
    pub position_at: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaState {
    /// No Apple Music session at all: not running, or not installed.
    NoAppleMusic,
    /// Session exists, nothing readable (idle / loading).
    Idle,
    Playing,
    Paused,
}

pub enum MediaEvent {
    /// `None` = Apple Music session present but no track metadata right now.
    Snapshot(Option<Track>),
    Unavailable,
}

pub fn is_target_app(aumid: &str) -> bool {
    aumid.starts_with(APPLE_MUSIC_AUMID_PREFIX)
}

pub fn track_id(app_id: &str, title: &str, artist: Option<&str>, album: Option<&str>) -> String {
    format!(
        "{app_id}|{title}|{}|{}",
        artist.unwrap_or_default(),
        album.unwrap_or_default()
    )
}

/// Discord caps activity text at 128 characters. Cut on a char boundary.
pub fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}

/// SMTC hands us encoded image bytes; wrap them for the webview. The mime is
/// sniffed from the magic bytes because SMTC does not report it.
pub fn art_data_url(bytes: Vec<u8>) -> String {
    let mime = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        "image/jpeg"
    } else {
        "image/jpeg"
    };
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    format!("data:{mime};base64,{encoded}")
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Owns the SMTC listener thread. Events flow to the engine; the thread lives
/// as long as the process.
pub fn spawn(tx: UnboundedSender<MediaEvent>) {
    std::thread::Builder::new()
        .name("smtc".into())
        .spawn(move || smtc::run(tx))
        .expect("failed to spawn SMTC thread");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_apple_music_sessions_are_stolen() {
        assert!(is_target_app("AppleInc.AppleMusicWin_nzyj5cx40ttqa!App"));
        assert!(is_target_app("AppleInc.AppleMusicWin"));
        assert!(!is_target_app("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"));
        assert!(!is_target_app("Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic"));
        assert!(!is_target_app(""));
    }

    #[test]
    fn truncation_is_char_safe() {
        let title = "😀".repeat(200);
        let cut = truncate_chars(&title, 128);
        assert_eq!(cut.chars().count(), 128);
        assert_eq!(truncate_chars("short", 128), "short");
    }

    #[test]
    fn artwork_mime_is_sniffed() {
        assert!(art_data_url(vec![0x89, b'P', b'N', b'G', 1]).starts_with("data:image/png;base64,"));
        assert!(art_data_url(vec![0xFF, 0xD8, 0xFF, 1]).starts_with("data:image/jpeg;base64,"));
    }

    #[test]
    fn track_id_changes_with_album() {
        let a = track_id("app", "Song", Some("Artist"), Some("Album A"));
        let b = track_id("app", "Song", Some("Artist"), Some("Album B"));
        assert_ne!(a, b);
    }
}
