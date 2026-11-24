import type { ReactNode } from "react";

interface SectionProps {
  title: string;
  description?: string;
  children: ReactNode;
}

export function Section({ title, description, children }: SectionProps) {
  return (
    <section className="rounded-xl border border-white/5 bg-white/[0.02] p-5">
      <header className="mb-1">
        <h2 className="text-sm font-semibold tracking-wide text-zinc-200">{title}</h2>
        {description && <p className="mt-1 text-xs text-zinc-500">{description}</p>}
      </header>
      <div className="mt-2">{children}</div>
    </section>
  );
}
