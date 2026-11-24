# Untested on this machine

Built on a Windows laptop on 2026-11-23 **without Apple Music, without Discord desktop, and without the MSVC linker**. No `.exe` was produced here, so nothing runtime-related has been executed. This file is the checklist for the PC that has Apple Music.

## Verified here (static only)

| Check | Result |
|---|---|
| `npx tsc --noEmit` | passes |
| `npm run build` (Vite 6 + Tailwind v4) | passes, bundle 157 kB |
| `cargo check --all-targets` | see below |
| `cargo test` | see below |
| Icons generated (`src-tauri/icons/*`) | present |

## Never executed — test these on the Apple Music PC

Run `docs/probe-smtc.ps1` first. It shows what Windows actually exposes, independent of this app.

| # | Untested thing | Why | How to test | Pass looks like |
|---|---|---|---|---|
| 1 | Apple Music AUMID | Apple Music not installed here; the prefix `AppleInc.AppleMusicWin` is from public reports, not observed | `docs/probe-smtc.ps1` with a track playing | `Aumid` starts with `AppleInc.AppleMusicWin` |
| 2 | SMTC metadata (title/artist/album) | no session ever seen | same probe | non-empty Title/Artist/Album |
| 3 | Thumbnail read (`DataReader` stream → bytes) | code path never ran | dashboard shows the cover art | artwork appears while playing |
| 4 | Event-driven updates (pause, resume, next track, seek) | needs a live session | play/pause/skip and watch the dashboard without touching the app | dashboard follows within ~1 s; presence changes once |
| 5 | Apple Music closed mid-track | needs a live session | close Apple Music | presence clears after the configured delay, status row says "Not running" |
| 6 | Discord connection | no Discord running, `DISCORD_CLIENT_ID` is still the placeholder | paste a real Application ID in `src-tauri/src/discord/mod.rs`, start Discord, then the app | status "Connected", Discord shows the track |
| 7 | Discord reconnect | needs Discord | kill Discord, reopen it | dashboard shows Error then Connected within ~30 s (backoff cap) and presence comes back |
| 8 | "Playing" vs "Listening to" | Discord decides this per app | look at your profile | expected: "Playing Apple Music RPC"; if it says something else, that is Discord's choice |
| 9 | Presence field toggles | needs Discord | turn off artist/album/title one by one | Discord text drops each field, falls back to "Apple Music" |
| 10 | Progress bar in Discord | needs Discord | play a track, seek | bar appears and moves; seeking updates it |
| 11 | `clear_delay_ms` debounce | needs Discord + a track change | switch tracks quickly | no clear/set flicker in Discord |
| 12 | Tray icon + menu (show, toggle, status line, exit) | needs a window | right-click the tray icon | status line says Connected/Error; toggling updates the dashboard; Exit leaves no process in Task Manager |
| 13 | Close button → tray | needs a window | click X | window hides, tray remains; with the setting off, the app quits |
| 14 | Start minimized (`--minimized` / toggle) | needs a window | enable, restart | no window at launch |
| 15 | Launch on Windows startup | needs an installed exe | enable, reboot/sign out | app starts; check `HKCU\...\Run` for `apple-music-rpc` |
| 16 | `%APPDATA%\AppleMusicRPC\config.json` write/read | needs the app | change a setting, close, reopen | value persists; no `.tmp` file left |
| 17 | Corrupt config recovery | unit-tested only | truncate the JSON, start app | app starts on defaults, shows the warning, keeps `config.json.corrupt` |
| 18 | `Not configured` Discord state | placeholder ID path | run without an Application ID | status "Not configured", no crash, no retry spam |
| 19 | Autostart plugin on a dev build | dev vs installed path differs | only test on the installed build | — |
| 20 | NSIS installer | no MSVC linker, never built | `npm run tauri build` on a machine with Build Tools | installer in `src-tauri/target/release/bundle/nsis/` |
| 21 | Long-run CPU/RAM | not measurable here | leave it 2 h with a track playing | ~0 % CPU idle, RSS flat |
| 22 | Duplicate workers | needs runtime | toggle the presence switch 20× | still one `smtc` and one `discord-ipc` thread |
| 23 | Non-Latin / emoji track names | unit-tested for truncation only | play such a track | no panic, readable text |

## Deliberately not built (spec items cut, with reason)

- **Album artwork in Discord presence** — Discord needs a public URL; we only have bytes, and uploading them would break the privacy rule. Dashboard shows the art instead.
- **Desktop notifications** — marked optional in the brief; omitted rather than shipped untested.
- **"Update presence on new track" toggle** — would be a switch that disables the product's core function.
- **"Rich Presence visibility" toggle (Privacy)** — duplicate of the main toggle.
- **iTunes classic support** — it does not publish SMTC sessions.

## First run on the PC, in order

1. Install Rust + VS Build Tools (`rustup-init.exe`, then `rustup default stable-msvc`; Build Tools with the C++ workload).
2. Install Apple Music from the Store, play something.
3. `powershell -ExecutionPolicy Bypass -File docs\probe-smtc.ps1` → confirm the AUMID and metadata.
4. `npm install`, paste your Discord Application ID, `npm run tauri dev`.
5. Walk rows 4–15 above; then `npm run tauri build` (row 20) and re-test on the installed copy.
