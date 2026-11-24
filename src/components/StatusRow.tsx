interface StatusRowProps {
  label: string;
  value: string;
  tone: "ok" | "warn" | "off";
}

const dot = {
  ok: "bg-emerald-400",
  warn: "bg-amber-400",
  off: "bg-zinc-500",
};

export function StatusRow({ label, value, tone }: StatusRowProps) {
  return (
    <div className="flex items-center justify-between gap-4 py-1.5">
      <span className="text-sm text-zinc-400">{label}</span>
      <span className="flex items-center gap-2 text-sm text-zinc-100">
        <span className={`h-2 w-2 rounded-full ${dot[tone]}`} />
        {value}
      </span>
    </div>
  );
}
