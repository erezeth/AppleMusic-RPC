use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

/// User preferences. Every field is `serde(default)` so a config written by an
/// older or newer build still loads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub rich_presence_enabled: bool,
    pub auto_detect: bool,
    pub discord_enabled: bool,
    pub start_with_windows: bool,
    pub start_minimized: bool,
    pub minimize_to_tray: bool,
    pub show_track_title: bool,
    pub show_artist: bool,
    pub show_album: bool,
    pub show_progress: bool,
    pub show_album_art: bool,
    pub clear_on_pause: bool,
    pub clear_on_stop: bool,
    /// Wait this long before clearing presence, so a track change does not flicker.
    pub clear_delay_ms: u64,
    pub discord_auto_reconnect: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            rich_presence_enabled: true,
            auto_detect: true,
            discord_enabled: true,
            start_with_windows: false,
            start_minimized: false,
            minimize_to_tray: true,
            show_track_title: true,
            show_artist: true,
            show_album: true,
            show_progress: true,
            show_album_art: true,
            clear_on_pause: false,
            clear_on_stop: true,
            clear_delay_ms: 2000,
            discord_auto_reconnect: true,
        }
    }
}

impl Config {
    pub fn validate(&mut self) {
        self.clear_delay_ms = self.clear_delay_ms.min(30_000);
    }

    /// Missing file is normal (first run). A corrupt file is kept aside so the
    /// user can inspect it, and defaults are used.
    pub fn load(dir: &Path) -> (Self, Option<String>) {
        let path = dir.join("config.json");
        let Ok(raw) = std::fs::read_to_string(&path) else {
            return (Self::default(), None);
        };
        match serde_json::from_str::<Config>(&raw) {
            Ok(mut cfg) => {
                cfg.validate();
                (cfg, None)
            }
            Err(err) => {
                let _ = std::fs::rename(&path, dir.join("config.json.corrupt"));
                (
                    Self::default(),
                    Some(format!(
                        "config.json could not be read ({err}). Defaults restored; the old file was kept as config.json.corrupt."
                    )),
                )
            }
        }
    }

    /// Write to a temp file, then rename, so a crash cannot leave half a file.
    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let tmp = dir.join("config.json.tmp");
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(json.as_bytes())?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, dir.join("config.json"))
    }
}

pub fn config_dir() -> PathBuf {
    match std::env::var_os("APPDATA") {
        Some(appdata) => PathBuf::from(appdata).join("AppleMusicRPC"),
        None => PathBuf::from("."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("amrpc-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn defaults_when_file_missing() {
        let dir = tmpdir("missing");
        let (cfg, warning) = Config::load(&dir);
        assert_eq!(cfg, Config::default());
        assert!(warning.is_none());
        assert!(cfg.rich_presence_enabled);
        assert_eq!(cfg.clear_delay_ms, 2000);
    }

    #[test]
    fn truncated_file_is_quarantined() {
        let dir = tmpdir("corrupt");
        std::fs::write(dir.join("config.json"), "{\"show_artist\": tr").unwrap();
        let (cfg, warning) = Config::load(&dir);
        assert_eq!(cfg, Config::default());
        assert!(warning.is_some());
        assert!(dir.join("config.json.corrupt").exists());
        assert!(!dir.join("config.json").exists());
    }

    #[test]
    fn clear_delay_is_clamped() {
        let dir = tmpdir("clamp");
        std::fs::write(dir.join("config.json"), "{\"clear_delay_ms\": 999999}").unwrap();
        let (cfg, _) = Config::load(&dir);
        assert_eq!(cfg.clear_delay_ms, 30_000);
    }

    #[test]
    fn unknown_and_missing_keys_are_tolerated() {
        let dir = tmpdir("partial");
        std::fs::write(
            dir.join("config.json"),
            "{\"show_artist\": false, \"from_the_future\": 42}",
        )
        .unwrap();
        let (cfg, warning) = Config::load(&dir);
        assert!(warning.is_none());
        assert!(!cfg.show_artist);
        assert!(cfg.show_track_title);
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tmpdir("roundtrip");
        let mut cfg = Config::default();
        cfg.show_album = false;
        cfg.clear_delay_ms = 750;
        cfg.save(&dir).unwrap();
        let (loaded, warning) = Config::load(&dir);
        assert!(warning.is_none());
        assert_eq!(loaded, cfg);
        assert!(!dir.join("config.json.tmp").exists());
    }
}
