import { invoke } from "@tauri-apps/api/core";
import { ProgressBar } from "../components/ProgressBar";
import { StatusRow } from "../components/StatusRow";
import { elapsedMs, TrackCard } from "../components/TrackCard";
import { Toggle } from "../components/Toggle";
import { discordLabel, mediaLabel, type Config, type Snapshot } from "../types";

interface DashboardProps {
  snapshot: Snapshot;
  now: number;
  updateConfig: (config: Config) => void;
  onOpenSettings: () => void;
}

export function Dashboard({ snapshot, now, updateConfig, onOpenSettings }: DashboardProps) {
  const { config, track, media, discord } = snapshot;

  const discordTone =
    discord === "connected" ? "ok" : discord === "connecting" ? "warn" : discord === "disabled" ? "off" : "warn";
  const mediaTone =
    media === "playing" ? "ok" : media === "paused" || media === "idle" ? "warn" : "off";

  return (
    <div className="space-y-5">
      {snapshot.config_warning && (
        <p className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-200">
          {snapshot.config_warning}
        </p>
      )}

      <section className="rounded-xl border border-white/5 bg-white/[0.02] p-5">
        <div className="flex items-baseline justify-between">
          <h2 className="text-sm font-semibold tracking-wide text-zinc-200">Status</h2>
          <span className="text-xs text-zinc-500">Application running</span>
        </div>
        <div className="mt-2 divide-y divide-white/5">
          <StatusRow label="Apple Music" value={mediaLabel(media, track !== null)} tone={mediaTone} />
          <StatusRow label="Discord" value={discordLabel(discord)} tone={discordTone} />
        </div>
      </section>

      <section className="rounded-xl border border-white/5 bg-white/[0.02] p-5">
        <h2 className="mb-4 text-sm font-semibold tracking-wide text-zinc-200">Now playing</h2>
        {track ? (
          <div className="space-y-5">
            <TrackCard track={track} now={now} showArt={config.show_album_art} />
            {config.show_progress && (
              <ProgressBar
                positionMs={elapsedMs(track, now)}
                durationMs={track.duration_ms ?? 0}
                playing={track.playing}
              />
            )}
          </div>
        ) : (
          <p className="py-8 text-center text-sm text-zinc-500">
            Nothing playing. Start a track in Apple Music.
          </p>
        )}
      </section>

      <section className="rounded-xl border border-white/5 bg-white/[0.02] px-5 py-3">
        <Toggle
          label="Rich Presence"
          hint={
            config.rich_presence_enabled
              ? "Your listening activity is shown on Discord."
              : "Discord is not showing your listening activity."
          }
          checked={config.rich_presence_enabled}
          onChange={(value) => updateConfig({ ...config, rich_presence_enabled: value })}
        />
      </section>

      <div className="flex gap-3">
        <button
          type="button"
          onClick={onOpenSettings}
          className="rounded-lg border border-white/10 bg-white/5 px-4 py-2 text-sm text-zinc-200 hover:bg-white/10"
        >
          Settings
        </button>
        <button
          type="button"
          onClick={() => invoke("hide_to_tray")}
          className="rounded-lg border border-white/10 bg-white/5 px-4 py-2 text-sm text-zinc-200 hover:bg-white/10"
        >
          Minimize to tray
        </button>
      </div>
    </div>
  );
}
