export type MediaState = "no_apple_music" | "idle" | "playing" | "paused";

export type DiscordState =
  | "disabled"
  | "not_configured"
  | "connecting"
  | "connected"
  | { error: string };

export interface Track {
  id: string;
  app_id: string;
  title: string;
  artist: string | null;
  album: string | null;
  art?: string;
  duration_ms: number | null;
  position_ms: number;
  playing: boolean;
  position_at: number;
}

export interface Config {
  rich_presence_enabled: boolean;
  auto_detect: boolean;
  discord_enabled: boolean;
  start_with_windows: boolean;
  start_minimized: boolean;
  minimize_to_tray: boolean;
  show_track_title: boolean;
  show_artist: boolean;
  show_album: boolean;
  show_progress: boolean;
  show_album_art: boolean;
  clear_on_pause: boolean;
  clear_on_stop: boolean;
  clear_delay_ms: number;
  discord_auto_reconnect: boolean;
}

export interface Snapshot {
  config: Config;
  track: Track | null;
  media: MediaState;
  discord: DiscordState;
  config_warning: string | null;
}

export function discordLabel(state: DiscordState): string {
  if (typeof state === "object") return `Error: ${state.error}`;
  switch (state) {
    case "disabled":
      return "Disabled";
    case "not_configured":
      return "Not configured";
    case "connecting":
      return "Connecting…";
    case "connected":
      return "Connected";
  }
}

export function mediaLabel(state: MediaState, hasTrack: boolean): string {
  switch (state) {
    case "no_apple_music":
      return "Not running";
    case "idle":
      return "Running · idle";
    case "paused":
      return "Paused";
    default:
      return hasTrack ? "Playing" : "Running · idle";
  }
}

export function formatTime(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const minutes = Math.floor(total / 60);
  const seconds = total % 60;
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}
