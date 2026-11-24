# Apple Music RPC

Shows the Apple Music track currently playing in Windows as Discord Rich Presence. Rust + Tauri 2 backend, React dashboard, tray icon. Everything stays on your PC.

## Requirements

- Windows 10 1809+ / Windows 11
- Apple Music (Microsoft Store app) — iTunes classic does not publish SMTC and is not supported
- Discord desktop app running on the same machine
- To build: Rust (MSVC toolchain), Visual Studio C++ Build Tools, Node 20+

## Setup

1. **Create a Discord application** at https://discord.com/developers/applications → *New Application*, name it whatever you want users to see as "Playing …".
2. Copy the **Application ID** into `src-tauri/src/discord/mod.rs`:

```rust
pub const DISCORD_CLIENT_ID: &str = "123456789012345678";
```

3. Optional: upload an image under *Rich Presence → Art Assets* with the key `apple_music` (512×512). Without it the presence simply has no image.

## Develop

```powershell
npm install
npm run tauri dev
```

## Build an installer

```powershell
npm run tauri build
# -> src-tauri/target/release/bundle/nsis/Apple Music RPC_0.1.0_x64-setup.exe
```

## How it works

- `media/smtc.rs` subscribes to Windows SMTC events (`SessionsChanged`, `MediaPropertiesChanged`, `PlaybackInfoChanged`, `TimelinePropertiesChanged`) and only reads sessions whose AUMID starts with `AppleInc.AppleMusicWin`. No polling.
- `app.rs` turns a track + your settings into the desired presence, debounces clears (`clear_delay_ms`), and dedupes identical updates.
- `discord/ipc.rs` owns the Discord named-pipe connection with exponential backoff reconnect (5 → 10 → 20 → 30 s cap).
- Progress in Discord comes from start/end timestamps, so Discord animates the bar itself and the app sends nothing per second.
- Settings live in `%APPDATA%\AppleMusicRPC\config.json` and save on every change.

## Known limits

- **Album artwork in Discord is not sent.** Discord needs a publicly reachable URL for artwork; SMTC only hands over image bytes, and hosting them would mean uploading your listening data somewhere. The dashboard shows the artwork instead.
- Discord renders third-party apps as **"Playing …"**, not "Listening to …".
- Presence mirrors the **Windows media session**. Apple Music occasionally reports no timeline for radio/streams; then the progress bar is omitted rather than guessed.
- Pausing keeps the last presence unless *Clear presence when music is paused* is on.

## Privacy

No telemetry, no history file, no network requests. The only IPC is the local Discord pipe (`\\.\pipe\discord-ipc-*`). Track metadata exists in memory only.
