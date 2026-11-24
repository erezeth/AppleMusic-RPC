interface ToggleProps {
  label: string;
  hint?: string;
  checked: boolean;
  onChange: (value: boolean) => void;
  disabled?: boolean;
}

export function Toggle({ label, hint, checked, onChange, disabled }: ToggleProps) {
  return (
    <div
      className={`flex items-center justify-between gap-6 border-b border-white/5 py-3 last:border-b-0 ${
        disabled ? "opacity-40" : ""
      }`}
    >
      <div className="min-w-0">
        <p className="text-sm text-zinc-100">{label}</p>
        {hint && <p className="mt-0.5 text-xs leading-relaxed text-zinc-400">{hint}</p>}
      </div>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={label}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        className={`relative h-6 w-11 shrink-0 rounded-full transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-400 ${
          checked ? "bg-indigo-500" : "bg-zinc-700"
        } ${disabled ? "cursor-not-allowed" : "cursor-pointer"}`}
      >
        <span
          className={`absolute top-0.5 h-5 w-5 rounded-full bg-white shadow transition-transform ${
            checked ? "translate-x-[22px]" : "translate-x-0.5"
          }`}
        />
      </button>
    </div>
  );
}
