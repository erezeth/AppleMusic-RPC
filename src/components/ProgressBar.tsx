import { formatTime } from "../types";

interface ProgressBarProps {
  positionMs: number;
  durationMs: number;
  playing: boolean;
}

export function ProgressBar({ positionMs, durationMs, playing }: ProgressBarProps) {
  const ratio =
    durationMs > 0 ? Math.min(1, Math.max(0, positionMs / durationMs)) : 0;

  return (
    <div className="w-full">
      <div className="h-1 w-full overflow-hidden rounded-full bg-white/10">
        <div
          className={`h-full rounded-full ${playing ? "bg-indigo-400" : "bg-zinc-500"}`}
          style={{ width: `${ratio * 100}%` }}
        />
      </div>
      <div className="mt-1.5 flex justify-between font-mono text-[11px] text-zinc-500">
        <span>{formatTime(positionMs)}</span>
        <span>{durationMs > 0 ? formatTime(durationMs) : "--:--"}</span>
      </div>
    </div>
  );
}
