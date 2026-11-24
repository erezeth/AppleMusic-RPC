# Apple Music RPC Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Tauri 2 desktop app that mirrors Apple Music playback (Windows SMTC) into Discord Rich Presence, with a settings dashboard and tray.

**Architecture:** One Rust backend process, three long-lived workers: an SMTC listener thread (WinRT events, no polling), a presence engine (pure decision function + dedupe), and a Discord IPC thread (blocking named pipe, reconnect backoff). A single `Config` struct is the control surface; commands write it, workers read snapshots over channels, UI gets `state` events.

**Tech Stack:** Rust, Tauri 2, React 18 + TypeScript, Tailwind v4, `windows` crate (WinRT `Media.Control`), `discord-rich-presence`, `serde`.

**Spec:** This document's §"Architecture decisions" resolves the spec's open points; the original requirements brief is reproduced in `docs/SPEC.md` (Task 1 copies it there).

## Global Constraints

- Windows 10/11 only. Rust edition 2021+, `rust-version = "1.80"`.
- Backend owns all Windows API, media, Discord, and state work. Frontend renders and calls commands only.
- No microphone/audio capture, no audio fingerprinting, no network egress except the local Discord named pipe.
- No mock data in production paths; no fabricated metadata. Missing field ⇒ omit or "Unknown", never invent.
- No polling loops: SMTC is event-driven; Discord updates only on state change; UI progress extrapolates locally.
- Every toggle in the UI must actually change behavior. (Two spec toggles are cut below as no-ops.)
- All settings persisted to `%APPDATA%\AppleMusicRPC\config.json`, saved on change, validated on load.
- Max 1 RPC update per meaningful change (track / play state / config), debounce 250 ms.

## Review Focus

| # | Input / condition | Expected behavior a user would assume |
|---|---|---|
| 1 | Another app plays media (Spotify, browser video) | Presence shows nothing; only Apple Music's session is mirrored |
| 2 | Apple Music paused, then closed | Paused → presence reflects pause or clears per config; closed → presence cleared, dashboard says "not running" |
| 3 | Discord restarts mid-track | Reconnects within ~5 s, presence restored without duplicate/garbled state |
| 4 | `config.json` truncated/edited by hand | App starts on defaults, bad file kept as `config.json.corrupt`, no crash |
| 5 | Track with no artist/album, or a 200-char title with emoji | No "Unknown" garbage in Discord; title truncated at a char boundary, never panics |
| 6 | Seek / repeat same track | Progress timestamps refresh; no presence flicker |

Each line gets a test in the task that owns the code (noted inline below).

## Environment findings (checked on this machine, 2026-11-23)

| Check | Result | Consequence |
|---|---|---|
| Apple Music (Store app) installed | **No** | Phase 0 gate. Cannot verify SMTC metadata until installed. |
| Active SMTC sessions | 0 | Probe script below ready to run the moment it's installed |
| `rustup` / `cargo` | **Missing** | Phase 0 install |
| MSVC C++ build tools | **Missing** (no VS installer) | Phase 0 install — Tauri/WinRT target needs `link.exe` |
| WebView2 | 157.0.4341.60 | OK |
| Node / npm | v26.13.1 | OK (note: Node 26 is newer than Vite's tested range; if `vite` misbehaves, pin Node 22 LTS) |
| git repo | none | Task 1 initializes |

Earlier probe (PowerShell 5.1, WinRT, read-only) proved the SMTC API path works on this OS and returns sessions — it returned zero because nothing is playing and Apple Music is absent. That probe is the Phase 0 acceptance test.

## Phase 0 — Prerequisites and the Apple Music gate

No product code until step 0.4 passes. Spec rule 3 is explicit: verify SMTC really exposes Apple Music before building around it.

- [ ] **0.1 Install Rust:** `winget install Rustlang.Rustup` then `rustup default stable-x86_64-pc-windows-msvc`; verify `cargo --version`.
- [ ] **0.2 Install MSVC build tools:** `winget install Microsoft.VisualStudio.2022.BuildTools --override "--quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"`; verify `where.exe link.exe` inside a "x64 Native Tools" shell, or that `cargo build` links a hello-world.
- [ ] **0.3 Install Apple Music** from the Microsoft Store (`winget install 9PFHDD62MXS1` — verify the ID in Store first; it has changed between regions). Sign in and start any track.
- [ ] **0.4 Run the SMTC probe** (script kept at `docs/probe-smtc.ps1`, content in Task 2) while a track plays. **Pass = Apple Music's AUMID, title, artist, album, non-zero duration.** Record the exact AUMID in `docs/apple-music-aumid.txt`.
  If it fails: stop and report. Do not build Phases 2–6 on an unverified assumption.
- [ ] **0.5 Create the Discord application** at discord.com/developers → New Application "Apple Music RPC". Copy the **Application ID**. Upload a 512×512 icon as asset key `apple_music`. Paste the ID into `discord/mod.rs` as `DISCORD_CLIENT_ID`.
- [ ] **0.6 Commit** `docs/` findings.

Gate: 0.4 passes. If Apple Music's SMTC is broken on this build (it has regressed in past releases), the fallback is Option B in "Open decisions" — decide before Task 4.

## Architecture decisions

**D1 — Detection: SMTC events, no polling.**
`GlobalSystemMediaTransportControlsSessionManager.RequestAsync()` once at startup, then subscribe:
`SessionsChanged` (Apple Music opened/closed), and per-session `MediaPropertiesChanged`, `PlaybackInfoChanged`, `TimelinePropertiesChanged`. On `SessionsChanged`, re-scan, subscribe to the Apple Music session, unsubscribe the dead one. Each event re-reads that session fully and emits one snapshot. No 1 Hz timer in the backend.

Verified API surface (windows-rs, feature `Media_Control`):
`GlobalSystemMediaTransportControlsSessionManager`, `…Session`, `…SessionMediaProperties`, `TryGetMediaPropertiesAsync`, `GetPlaybackInfo`, `GetTimelineProperties`, `MediaPlaybackStatus`.

**D2 — Threading: three workers, channels between.**
- SMTC thread: dedicated `std::thread`, `RoInitialize(RO_INIT_MULTITHREADED)` via the `windows` crate; WinRT events fire on the thread pool, no message pump needed. Sends `MediaEvent` over `tokio::sync::mpsc::UnboundedSender`.
- Engine: runs on Tauri's async runtime, consumes `MediaEvent` + config changes, owns the dedupe state and the pending-clear timer (`tokio::time`).
- Discord thread: dedicated `std::thread`, blocking crate calls, receives `PresenceCommand` over `std::sync::mpsc`, sends `DiscordStatus` back through a `tokio` sender.

Why threads and not all-async: `discord-rich-presence` is blocking, and WinRT apartment init wants a stable thread. Mixing two blocking things into `spawn_blocking` gives worse shutdown behavior than two owned threads.

**D3 — Progress: Discord renders it, we don't tick.**
Presence carries `start_timestamp = now − position` and `end_timestamp = start + duration`. Discord animates the bar itself. The dashboard extrapolates from the last snapshot with a 1 s frontend ticker. Result: zero per-second IPC traffic, zero backend polling.
Known ceiling: if Apple Music reports `EndTime = 0` (some builds do for streams), progress is omitted rather than faked.

**D4 — Presence is a pure function.**
`build_activity(&Track, &Config, now_ms) -> Option<Activity>` and `decide(&Snapshot, &Config, last_sent) -> Command`. Pure ⇒ unit-testable without Discord, and the answer to "why did my presence show that" is one function.

**D5 — Config: one struct, one command, atomic file.**
`set_config(Config)` replaces the whole struct (the UI holds the full state anyway) — one command instead of 20 toggles. `serde(default)` per field covers missing *and* extra keys across versions; `validate()` clamps `clear_delay_ms` to 0–30 000. Write = temp file + rename, so a crash mid-write can't corrupt. Corrupt load ⇒ rename to `config.json.corrupt`, start on defaults, surface one status line.

**D6 — Album art: in the app, not in Discord.**
Discord activity assets are either uploaded to the Discord app (static) or fetched by Discord's CDN from a public URL. SMTC gives artwork as raw image bytes only. Honoring the "never transmit listening data to external servers" rule means there is no URL to hand Discord. So: artwork renders in the dashboard from bytes; presence uses a static `apple_music` app asset.
`shortcut: Discord presence shows a static app icon instead of per-track art; upgrade path = optional user-supplied public artwork URL, only if the privacy rule is relaxed.`

**D7 — Cuts (spec toggles that would be lies).**
- "Update presence when a new track starts" — inherent to the product; a toggle that disables it makes the app pointless. Removed from the UI.
- "Enable/Disable Rich Presence visibility" (privacy) — duplicates the main toggle. Removed; the main toggle is the single source of truth.
- "Album artwork" toggle now means the dashboard artwork only (see D6), labelled "Show album artwork in dashboard".
Each is one line in the UI's About page so nobody hunts for them.

**D8 — "Listening" activity type is not guaranteed.**
Third-party IPC apps are usually rendered as "Playing <app name>". The `Listening` type is provisioned via the Discord app's configuration, not reliably via IPC. Verify in Task 9; if it renders as Playing, accept it and document — do not fake it.

## File structure

```
src-tauri/src/
  main.rs            tauri::Builder, plugin registration, tray, window close hook
  lib.rs             pub fn run()
  app.rs             Snapshot, AppState, worker wiring, decide(), dedupe, clear timer
  config.rs          Config, load/save/validate, dir resolution
  media/mod.rs       MediaEvent, spawn(), target-app filter
  media/smtc.rs      WinRT session manager, subscriptions, session→Track conversion
  discord/mod.rs     DISCORD_CLIENT_ID, DiscordStatus, spawn(), activity builder
  discord/ipc.rs     connect/reconnect loop, command → crate call
  commands.rs        get_state, set_config, quit
  tray.rs            tray icon + menu + handlers
src/
  main.tsx  App.tsx  index.css
  types.ts                      mirrors Snapshot/Config
  hooks/useAppState.ts          invoke get_state, listen("state"), 1 s progress ticker
  components/{Toggle,StatusRow,TrackCard,ProgressBar,Section}.tsx
  pages/{Dashboard,Settings}.tsx
docs/
  SPEC.md  probe-smtc.ps1  apple-music-aumid.txt
```

Merged vs the spec's tree: `media/metadata.rs` into `smtc.rs` (it was only a conversion), `commands/settings.rs` → `commands.rs`, `models/{track,status}.rs` → `app.rs` types (they are the snapshot), `discord/presence.rs` → `discord/mod.rs` (one builder). Same separation of concerns, four fewer files.

## Dependencies (and what was rejected)

Rust: `tauri 2` (features `tray-icon`), `tauri-plugin-autostart 2`, `tauri-plugin-notification 2` (optional feature, Phase 6), `tauri-plugin-log 2`, `serde`/`serde_json`, `windows 0.61` (`Media_Control`, `Foundation`, `Foundation_Collections`, `Storage_Streams`, `Media`), `discord-rich-presence 0.2`, `log`.
Tokio comes from `tauri::async_runtime` — no direct `tokio` dependency.
Rejected: `tauri-plugin-store` (60 lines of serde beats a plugin), `react-router` (two tabs), `zustand` (one state object), `image`/`base64` (artwork bytes go straight to a `Blob` URL in the UI), `tracing` (`log` + plugin-log is enough), hand-rolled Discord IPC (~150 lines of pipe framing to save one small crate — revisit only if the crate dies).

Frontend: `react`, `react-dom`, `@tauri-apps/api`; dev `vite`, `@vitejs/plugin-react`, `typescript`, `tailwindcss` + `@tailwindcss/vite` (v4, no config file), `@tauri-apps/cli`.

## Data model

```rust
// app.rs
pub struct Track {
  pub id: String,          // "<aumid>|<title>|<artist>|<album>" — dedupe key
  pub title: String,
  pub artist: Option<String>,
  pub album: Option<String>,
  pub art: Option<Vec<u8>>, // encoded bytes straight from SMTC thumbnail, UI only
  pub duration_ms: Option<u64>,
  pub position_ms: u64,
  pub playing: bool,
  pub position_at: u64,     // unix ms when position was sampled
}
pub enum MediaState { NoAppleMusic, Idle, Playing, Paused }   // Idle = session, no metadata
pub enum DiscordState { Disabled, NotConfigured, Connecting, Connected, Error(String) }
pub struct Snapshot { config: Config, track: Option<Track>, media: MediaState, discord: DiscordState }

// discord/mod.rs
pub struct Activity { details: String, state: String, start_ms: Option<u64>, end_ms: Option<u64>, large_image: &'static str, large_text: String }
pub enum PresenceCommand { Set(Activity), Clear }
```

`Config` fields (all `#[serde(default)]`, snake_case, matching the spec's example): `rich_presence_enabled=true, auto_detect=true, discord_enabled=true, start_with_windows=false, start_minimized=false, minimize_to_tray=true, notifications_enabled=true, show_track_title=true, show_artist=true, show_album=true, show_progress=true, show_album_art=true, clear_on_pause=false, clear_on_stop=true, clear_delay_ms=2000, discord_auto_reconnect=true`.

Discord field limits (128 chars) enforced by `truncate_chars` at a `char` boundary.

## Tasks

### Task 1: Scaffold + spec copy + git

**Files:** create `package.json`, `vite.config.ts`, `tsconfig.json`, `src/`, `src-tauri/{Cargo.toml,build.rs,tauri.conf.json,capabilities/default.json,icons/}`, `docs/SPEC.md`, `.gitignore`
**Produces:** a window that opens with "Apple Music RPC" and nothing else; `npm run tauri dev` works.

- [ ] `npm create tauri-app@latest` (React + TS, app name `apple-music-rpc`, identifier `com.applemusicrpc.app`), Tailwind v4 via `@tailwindcss/vite` + `@import "tailwindcss";`.
- [ ] Copy the user's brief verbatim to `docs/SPEC.md`; `git init` + first commit.
- [ ] Verify: `npm run tauri dev` shows an empty dark window (set `theme: "Dark"`, `decorations: true`, 900×640, min 720×520).

### Task 2: SMTC probe script (kept in repo)

**Files:** create `docs/probe-smtc.ps1`
**Produces:** the Phase 0 verifier, reusable in Phase 2 debugging.

- [ ] Script: `RoInitialize`-free WinRT awaits via `System.Runtime.WindowsRuntime::AsTask`, `RequestAsync` the manager, list every session with AUMID, playback status, title/artist/album, thumbnail present, position/end/duration.
- [ ] Run it with Apple Music playing. Record output in `docs/apple-music-aumid.txt`.
- [ ] Commit.

### Task 3: Config load/save/validate

**Files:** create `src-tauri/src/config.rs`; test module inside it.
**Produces:** `Config::default()`, `Config::load(dir: &Path) -> (Config, Option<String>)`, `Config::save(&self, dir: &Path) -> io::Result<()>`, `Config::validate(&mut self)`, `config_dir() -> PathBuf`.

- [ ] Tests first: missing file ⇒ defaults, warning `None`; truncated JSON ⇒ defaults + warning + `config.json.corrupt` exists; `{"clear_delay_ms": 999999}` ⇒ clamped to 30000; unknown key `{"foo":1}` ⇒ loads fine; round-trip save→load equality.
- [ ] Implement with `serde_json`, `#[serde(default)]`, temp-file+rename.
- [ ] Verify `cargo test config`; commit.

### Task 4: Track model + SMTC conversion (pure)

**Files:** create `src-tauri/src/media/mod.rs`, `media/smtc.rs`.
**Produces:** `is_target_app(aumid: &str) -> bool`, `to_track(props: &SmpteProps, playback: PlaybackStatus, timeline_ms: (u64, Option<u64>), now_ms: u64) -> Option<Track>`, `pub enum MediaEvent { Snapshot(Option<Track>), Unavailable }`, `pub fn spawn(tx: UnboundedSender<MediaEvent>) -> JoinHandle<()>`.

- [ ] Tests first (Review Focus 5): `is_target_app` true for `AppleInc.AppleMusicWin_nzyj5cx40ttqa!App`, false for `SpotifyAB.SpotifyMusic_…`, false for `Microsoft.ZuneMusic_…`; empty title ⇒ `None`; artist/album absent ⇒ `None` fields, not `"Unknown"`; `truncate_chars` on a 200-char emoji title ⇒ ≤128 chars, no panic.
- [ ] Implement subscription graph from D1 on the dedicated MTA thread; emit `Unavailable` when no target session exists.
- [ ] Verify `cargo test media`, then dev-run and watch `log` lines appear/disappear as Apple Music opens/closes (this is the Phase 2 acceptance).
- [ ] Commit.

### Task 5: Presence builder + decision function (pure)

**Files:** create `src-tauri/src/discord/mod.rs`, `discord/presence` logic inside it.
**Produces:** `build_activity(&Track, &Config, now_ms) -> Option<Activity>`, `decide(track: Option<&Track>, &Config, last: Option<&Activity>) -> PresenceCommand`.

- [ ] Tests first (Review Focus 2, 6): paused + `clear_on_pause=false` ⇒ `Set` with no timestamps; paused + `clear_on_pause=true` ⇒ `Clear`; `None` track + `clear_on_stop=true` ⇒ `Clear`; disabled ⇒ `Clear`; playing + `show_progress` ⇒ `start == now − position`, `end == start + duration`; `show_artist=false` ⇒ state excludes artist; all-fields-hidden ⇒ details `"Apple Music"`; identical repeat call ⇒ no change detected (dedupe).
- [ ] Implement per D4/D8.
- [ ] Verify `cargo test discord`; commit.

### Task 6: Discord IPC worker

**Files:** `src-tauri/src/discord/ipc.rs`.
**Produces:** `pub fn spawn(rx: Receiver<PresenceCommand>, status: UnboundedSender<DiscordStatus>) -> JoinHandle<()>`, `const DISCORD_CLIENT_ID: &str`.

- [ ] Blocking loop: `DiscordIpcClient::connect(CLIENT_ID)`; on success status `Connected`; on each `Set` call `set_activity`, on `Clear` call `clear_activity`; on any error ⇒ status `Error`, drop client, sleep 5 s, reconnect (only if `discord_auto_reconnect`, else stay `Error` and skip until config re-enables).
- [ ] No test against real Discord; test the pure retry-backoff helper (`next_delay(attempt) -> Duration`, capped at 30 s).
- [ ] Manual: with Discord open, run dev build, see presence; kill Discord, see `Error`; reopen, see `Connected` within ~5 s (Review Focus 3). Commit.

### Task 7: Engine + app state

**Files:** `src-tauri/src/app.rs`, `lib.rs`, `main.rs`, `commands.rs`.
**Produces:** `AppState` in Tauri managed state, `spawn_state_events()`, `#[tauri::command] get_state() -> Snapshot`, `set_config(Config) -> Result<Snapshot, String>`, `quit()`.

- [ ] Wire: SMTC → engine → (presence tx, snapshot store, `app.emit("state", snapshot)`). Config change ⇒ save, re-evaluate presence immediately, emit.
- [ ] `clear_delay_ms` honoring: on `Clear` decision with delay > 0, `tokio::time::sleep` then re-check the current snapshot before sending (prevents clear-then-set flicker on track change — Review Focus 6).
- [ ] Test: `decide_with_delay` returns `Hold` for a 2 s window when a new track arrives at 1 s.
- [ ] Verify `cargo test`; commit.

### Task 8: Dashboard UI

**Files:** `src/types.ts`, `hooks/useAppState.ts`, `components/{StatusRow,TrackCard,ProgressBar,Toggle}.tsx`, `pages/Dashboard.tsx`, `App.tsx`.
**Produces:** the spec's dashboard layout, dark, with artwork, extrapolated progress, main toggle.

- [ ] `useAppState`: `invoke("get_state")` on mount, `listen("state")`, 1 s `setInterval` only while `playing` to recompute the displayed position (never calls the backend).
- [ ] Artwork: bytes ⇒ `Blob` ⇒ `URL.createObjectURL`, revoke on change (leak guard).
- [ ] Status rows: Apple Music (No Apple Music installed / Running, no playback / Paused / Playing), Discord (Disabled / Not configured / Connecting / Connected / Error).
- [ ] Verify manually: toggle off ⇒ presence clears and dashboard says Disabled. Commit.

### Task 9: Settings page

**Files:** `src/pages/Settings.tsx`, `components/Section.tsx`, `App.tsx` (tab switch).
**Produces:** General / Discord / Playback / Privacy / About sections; every control writes `set_config`.

- [ ] Sections per spec §3C, minus the two cut toggles (D7), minus `start_minimized` (folded into "Start with Windows" behavior? no — keep as its own General toggle, it is meaningful).
- [ ] About: version, Discord client ID state, link to README, the D6/D7/D8 one-liners.
- [ ] Verify each toggle round-trips: change, restart app, value persists; and each visibly changes behavior (presence fields, clear-on-pause, reconnect). Commit.

### Task 10: Tray icon + autostart + window behavior

**Files:** `src-tauri/src/tray.rs`, `main.rs` (window close hook), `Cargo.toml`, `capabilities/default.json`.
**Produces:** tray with Show Dashboard / Enable·Disable Rich Presence / status line (disabled menu item) / Settings / Exit; close-to-tray; `--minimized` start.

- [ ] `TrayIconBuilder` + `Menu`; menu built from a snapshot, rebuilt on state change (cheap, avoids stale checkbox).
- [ ] Close: if `minimize_to_tray` ⇒ `prevent_close` + hide, else exit; `Exit` ⇒ `app.exit(0)` and stop workers cleanly (drop channels, join threads with timeout).
- [ ] Autostart plugin: enable/disable mirrors `start_with_windows`, args `--minimized`.
- [ ] Verify: close button hides to tray; tray Exit really quits (no zombie process in Task Manager). Commit.

### Task 11: Notifications (optional, last)

**Files:** `src-tauri/src/main.rs` (plugin + emit on status transition).
**Produces:** one notification on Discord Connected→Error and Error→Connected, only if `notifications_enabled`.

- [ ] Fire only on transitions, never per track. Test the transition predicate (pure). Commit.

### Task 12: Polish, edge cases, long-run check

- [ ] Verify: Apple Music closed mid-track ⇒ presence cleared, dashboard says not running; no duplicate workers after toggling 20× (count threads); 2 h run with Task Manager CPU ≈ 0% idle and RSS flat.
- [ ] Icon set (all sizes) via `tauri icon`; window title; empty states; keyboard focus rings. Commit.

### Task 13: Release

- [ ] `npm run tauri build` ⇒ NSIS installer in `target/release/bundle/nsis/`; install to a clean path, verify autostart entry, tray, presence, config in `%APPDATA%`.
- [ ] `README.md`: install, Discord app setup (client ID/asset), supported Apple Music versions, known limits (D6/D8, no iTunes-classic support), privacy statement. Commit + tag `v0.1.0`.

## Open decisions (need your answer before execution reaches the marked task)

1. **Apple Music is not installed** — install it now so Phase 0.4 can pass? Without it only Tasks 1–3 + 5 (config, pure logic) are buildable. (blocks Task 4)
2. **Rust + MSVC install** — may I run the two `winget` commands in Phase 0? They need no admin for rustup but Build Tools wants elevation.
3. **Discord Application ID** — create the app yourself (Discord dev portal, 2 min) or should I plan a config field instead of a constant so you can paste it later? (blocks Task 6)
4. **Fallback** if Apple Music's SMTC is dead on this build: (a) support the legacy iTunes AUMID too, (b) ship detection for any SMTC source with a settings picker, (c) stop and report. Recommendation: (c), then (a) only if iTunes classic is actually in use.
5. **Execution method** — subagent-driven or native (see below).

## Self-review notes

- Spec §3A/§3B/§3C/§4/§5/§6/§7/§8 each map to a task: detection → 4, presence → 5–6, controls → 8–9, UI → 8–9, config → 3, errors → 3/4/6/12, performance → 4/6/7/12.
- Spec's "notifications_enabled" toggle is a real feature but marked optional in the brief → Task 11, last, so a slip costs nothing.
- Spec's example config omits `clear_delay_ms` and `start_minimized`; both are in §3C's controls, so both are fields.
- Two spec toggles are intentionally absent (D7); if you want them as visible-but-inert UI, say so and I'll add them back as disabled controls with an explanation instead.
