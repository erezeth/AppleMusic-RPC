import type { Track } from "../types";

interface TrackCardProps {
  track: Track;
  now: number;
  showArt: boolean;
}

export function elapsedMs(track: Track, now: number): number {
  const extra = track.playing ? Math.max(0, now - track.position_at) : 0;
  const elapsed = track.position_ms + extra;
  return track.duration_ms ? Math.min(elapsed, track.duration_ms) : elapsed;
}

export function TrackCard({ track, now, showArt }: TrackCardProps) {
  return (
    <div className="flex items-center gap-5">
      <div className="relative h-28 w-28 shrink-0 overflow-hidden rounded-xl bg-white/5 ring-1 ring-white/10">
        {showArt && track.art ? (
          <img src={track.art} alt="" className="h-full w-full object-cover" />
        ) : (
          <div className="flex h-full w-full items-center justify-center text-2xl text-zinc-600">
            ♪
          </div>
        )}
      </div>
      <div className="min-w-0">
        <p className="truncate text-xl font-semibold text-zinc-50" title={track.title}>
          {track.title}
        </p>
        <p className="mt-1 truncate text-sm text-zinc-300">
          {track.artist ?? "Unknown artist"}
        </p>
        <p className="truncate text-sm text-zinc-500">{track.album ?? "Unknown album"}</p>
        <p className="mt-2 text-[11px] uppercase tracking-widest text-zinc-600">
          {track.playing ? "Now playing" : "Paused"}
        </p>
      </div>
      <span className="sr-only">{elapsedMs(track, now)}</span>
    </div>
  );
}
