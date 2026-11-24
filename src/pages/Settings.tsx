import { Section } from "../components/Section";
import { Toggle } from "../components/Toggle";
import type { Config, Snapshot } from "../types";

interface SettingsProps {
  snapshot: Snapshot;
  updateConfig: (config: Config) => void;
}

const delayOptions = [
  { value: 0, label: "Immediately" },
  { value: 1000, label: "After 1 second" },
  { value: 2000, label: "After 2 seconds" },
  { value: 5000, label: "After 5 seconds" },
];

export function Settings({ snapshot, updateConfig }: SettingsProps) {
  const config = snapshot.config;
  const set = (patch: Partial<Config>) => updateConfig({ ...config, ...patch });

  return (
    <div className="space-y-5">
      <Section title="General" description="Startup and tray behavior.">
        <Toggle
          label="Automatic music detection"
          hint="Watch Windows media sessions for Apple Music. Turning this off freezes the presence."
          checked={config.auto_detect}
          onChange={(value) => set({ auto_detect: value })}
        />
        <Toggle
          label="Launch on Windows startup"
          hint="Registers the app for the current Windows user."
          checked={config.start_with_windows}
          onChange={(value) => set({ start_with_windows: value })}
        />
        <Toggle
          label="Start minimized to tray"
          hint="No window appears at launch."
          checked={config.start_minimized}
          onChange={(value) => set({ start_minimized: value })}
        />
        <Toggle
          label="Minimize to tray when closing"
          hint="The close button hides the window; use tray → Exit to quit."
          checked={config.minimize_to_tray}
          onChange={(value) => set({ minimize_to_tray: value })}
        />
      </Section>

      <Section title="Discord" description="What gets sent to the Discord desktop app.">
        <Toggle
          label="Discord integration"
          hint="When off, the app disconnects its presence and stops updating Discord."
          checked={config.discord_enabled}
          onChange={(value) => set({ discord_enabled: value })}
        />
        <Toggle
          label="Show track title"
          checked={config.show_track_title}
          onChange={(value) => set({ show_track_title: value })}
        />
        <Toggle
          label="Show artist"
          checked={config.show_artist}
          onChange={(value) => set({ show_artist: value })}
        />
        <Toggle
          label="Show album"
          checked={config.show_album}
          onChange={(value) => set({ show_album: value })}
        />
        <Toggle
          label="Show playback progress"
          hint="Discord animates the progress bar from start/end timestamps."
          checked={config.show_progress}
          onChange={(value) => set({ show_progress: value })}
        />
        <Toggle
          label="Show album artwork in the dashboard"
          hint="Artwork never leaves this machine: Discord gets a static app icon, not your album cover."
          checked={config.show_album_art}
          onChange={(value) => set({ show_album_art: value })}
        />
        <Toggle
          label="Automatically reconnect to Discord"
          hint="Retries every few seconds while Discord is closed."
          checked={config.discord_auto_reconnect}
          onChange={(value) => set({ discord_auto_reconnect: value })}
        />
      </Section>

      <Section title="Playback" description="When the presence goes away.">
        <Toggle
          label="Clear presence when music is paused"
          checked={config.clear_on_pause}
          onChange={(value) => set({ clear_on_pause: value })}
        />
        <Toggle
          label="Clear presence when Apple Music is closed"
          checked={config.clear_on_stop}
          onChange={(value) => set({ clear_on_stop: value })}
        />
        <div className="flex items-center justify-between gap-6 py-3">
          <div>
            <p className="text-sm text-zinc-100">Delay before clearing</p>
            <p className="mt-0.5 text-xs text-zinc-400">
              Prevents flicker when one track ends and the next begins.
            </p>
          </div>
          <select
            value={config.clear_delay_ms}
            onChange={(event) => set({ clear_delay_ms: Number(event.target.value) })}
            className="rounded-lg border border-white/10 bg-[#12151b] px-3 py-1.5 text-sm text-zinc-200"
          >
            {delayOptions.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </div>
      </Section>

      <Section
        title="Privacy"
        description="Everything stays on this PC. No history, no telemetry, no uploads."
      >
        <Toggle
          label="Show listening activity"
          hint="The master switch. Off means nothing is sent to Discord."
          checked={config.rich_presence_enabled}
          onChange={(value) => set({ rich_presence_enabled: value })}
        />
        <p className="py-3 text-xs leading-relaxed text-zinc-500">
          The app talks only to the Discord desktop app over a local named pipe. Metadata is read
          from Windows media sessions in memory and is never written to disk or sent anywhere else.
        </p>
      </Section>

      <Section title="About">
        <dl className="space-y-2 py-2 text-xs text-zinc-400">
          <div className="flex justify-between gap-6">
            <dt>Version</dt>
            <dd className="text-zinc-300">0.1.0</dd>
          </div>
          <div className="flex justify-between gap-6">
            <dt>Discord application</dt>
            <dd className="text-zinc-300">{snapshot.discord === "not_configured" ? "ID not set" : "Configured"}</dd>
          </div>
        </dl>
        <ul className="mt-2 space-y-1.5 border-t border-white/5 pt-3 text-xs leading-relaxed text-zinc-500">
          <li>Album art in Discord is not possible without hosting it; the dashboard shows it instead.</li>
          <li>Discord renders third-party apps as "Playing", not "Listening to".</li>
          <li>Only the Apple Music Store app is detected; iTunes classic is not supported.</li>
        </ul>
      </Section>
    </div>
  );
}
