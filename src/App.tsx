import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { useAppState } from "./hooks/useAppState";
import { Dashboard } from "./pages/Dashboard";
import { Settings } from "./pages/Settings";

type Page = "dashboard" | "settings";

export default function App() {
  const { snapshot, now, updateConfig } = useAppState();
  const [page, setPage] = useState<Page>("dashboard");

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<string>("navigate", (event) => {
      if (event.payload === "settings") setPage("settings");
    }).then((fn) => (unlisten = fn));
    return () => unlisten?.();
  }, []);

  return (
    <div className="mx-auto flex min-h-screen max-w-3xl flex-col gap-6 px-6 py-7">
      <header className="flex items-start justify-between gap-4">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-indigo-400 to-fuchsia-500 text-lg text-white shadow-lg shadow-indigo-500/20">
            ♪
          </div>
          <div>
            <h1 className="text-lg font-semibold tracking-tight text-zinc-50">Apple Music RPC</h1>
            <p className="text-xs text-zinc-500">Listening activity for Discord</p>
          </div>
        </div>
        <nav className="flex rounded-lg border border-white/10 bg-white/5 p-1 text-sm">
          {(["dashboard", "settings"] as const).map((item) => (
            <button
              key={item}
              type="button"
              onClick={() => setPage(item)}
              className={`rounded-md px-3 py-1.5 capitalize transition-colors ${
                page === item ? "bg-white/10 text-zinc-50" : "text-zinc-400 hover:text-zinc-200"
              }`}
            >
              {item}
            </button>
          ))}
        </nav>
      </header>

      {!snapshot ? (
        <p className="py-20 text-center text-sm text-zinc-500">Loading…</p>
      ) : page === "dashboard" ? (
        <Dashboard
          snapshot={snapshot}
          now={now}
          updateConfig={updateConfig}
          onOpenSettings={() => setPage("settings")}
        />
      ) : (
        <Settings snapshot={snapshot} updateConfig={updateConfig} />
      )}
    </div>
  );
}
