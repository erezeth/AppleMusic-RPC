import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import type { Config, Snapshot } from "../types";

export function useAppState() {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    invoke<Snapshot>("get_state").then(setSnapshot).catch(console.error);
    let unlisten: UnlistenFn | undefined;
    listen<Snapshot>("state", (event) => setSnapshot(event.payload)).then(
      (fn) => (unlisten = fn),
    );
    return () => unlisten?.();
  }, []);

  // Playback position is derived on the client: the backend never polls
  // Windows for a ticking clock.
  const playing = snapshot?.track?.playing ?? false;
  useEffect(() => {
    if (!playing) return;
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, [playing, snapshot?.track?.id]);

  const updateConfig = useCallback(async (config: Config) => {
    try {
      setSnapshot(await invoke<Snapshot>("set_config", { config }));
    } catch (error) {
      console.error(error);
    }
  }, []);

  return { snapshot, now, updateConfig };
}
