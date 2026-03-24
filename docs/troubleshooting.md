# Troubleshooting

## `ffmpeg: command not found`

Install system dependencies first:

```bash
./setup.sh
```

Or install `ffmpeg` manually:

```bash
sudo apt-get update
sudo apt-get install -y ffmpeg
```

## Microphone starts but no audio is captured

Symptom:

- `No microphone audio was captured. Record for a bit longer and try again.`

What to do:

- hold the recording for at least 1-2 seconds
- verify the system default microphone/input source
- verify another app can record from the same microphone

Why this happens:

- the app records from Pulse's default input source
- if the source is wrong or the record-stop cycle is too short, FFmpeg can finish with no packets

## `faster-whisper` model missing

Download the default local model:

```bash
npm run model:download
```

If you want strict offline behavior, keep a real local model path configured in the widget settings.

## Python backend imports fail

Resync the backend environment:

```bash
npm run backend:sync
```

If that still fails, run:

```bash
cd python-backend
uv sync --reinstall
```

## Top-bar tray icon does not appear

The app uses a tray/app-indicator.

Check:

- you are on Ubuntu or another desktop environment with tray/app-indicator support
- your desktop session is not hiding legacy tray icons

Notes:

- Ubuntu should generally show it in the top bar
- Lubuntu / LXQt should show it in the panel tray area
- some GNOME setups need app-indicator-compatible shell support

## Paste shortcut does nothing

Check:

- a transcript currently exists
- the target app is focused and accepts synthetic paste input
- you are not blocked by Wayland input restrictions

Linux paste note:

- terminal-like windows use `Ctrl+Shift+V`
- general GUI apps use `Ctrl+V`
- X11 is more reliable than Wayland for this path

## Shortcut collisions

If the global shortcuts do not trigger:

- open widget settings
- change the record shortcut
- change the paste shortcut

Desktop environments and editors often reserve common combinations.

## `window.set_size not allowed` or `window.set_position not allowed`

This is a contributor/development issue, not a user runtime issue.

The capability file must include the window permissions used by the widget:

- [default.json](../src-tauri/capabilities/default.json)

This repo already includes those permissions.

## `tauri::tray` unresolved import during build

If contributors hit this during development, the Tauri crate must include the tray feature:

- [Cargo.toml](../src-tauri/Cargo.toml)

This repo already enables `tray-icon`.

## Clean-machine check

Run the doctor:

```bash
npm run doctor
```

It checks:

- Node.js
- npm
- Rust toolchain
- `uv`
- `ffmpeg`
- Tauri Linux system packages
- backend Python imports
- local model presence
