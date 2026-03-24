# Permissions and Platform Notes

This app is intentionally small, but a few platform details matter for real-world setup and issue triage.

## Microphone capture on Linux

The widget does not use browser-style `getUserMedia` for Linux microphone capture.

Instead, it records natively with:

- `ffmpeg`
- PulseAudio / PipeWire Pulse compatibility
- the default input source (`-f pulse -i default`)

Why:

- the webview microphone permission path was unreliable for this widget
- native capture avoids browser permission prompts that were causing startup and record failures

What users should expect:

- if the system default microphone works, the widget should record without a browser prompt
- if the system default input is wrong, recording can still fail or capture silence

## Paste automation

Cross-app paste is best-effort.

Current Linux behavior:

- the app copies the transcript to the system clipboard
- it then chooses a synthetic paste shortcut based on the active window when possible
- terminal-like windows use `Ctrl+Shift+V`
- general GUI apps use `Ctrl+V`

Why:

- terminal workflows are a primary target for this app
- general desktop apps typically expect `Ctrl+V`
- old `Shift+Insert` behavior was not a good universal clipboard path

Known limitations:

- active window detection can be incomplete on some desktops or sessions
- when active-window detection is unavailable, Linux falls back to `Ctrl+V`
- Wayland can limit synthetic input and clipboard automation more than X11
- even on X11, some desktops or app sandboxes may still restrict cross-app input

## X11 vs Wayland

Recommended:

- X11 for the smoothest paste-to-focus behavior
- Ubuntu GNOME on X11 is the current tested desktop path
- Lubuntu / LXQt on X11 is a likely-supported path, but not yet fully desktop-verified

Wayland notes:

- global shortcuts can still work
- tray support may still work
- synthetic paste/input automation may be partially limited depending on compositor and desktop environment

## Tray / top bar support

The top-bar integration is implemented as a tray/app-indicator.

This means:

- on Ubuntu GNOME, it should appear through app-indicator/system-tray support
- on Lubuntu / LXQt, it should appear in the LXQt panel tray area
- it is not a custom shell panel widget
- on desktop environments without app-indicator/system tray support, the icon may not appear

## Global shortcuts

Default shortcuts:

- `Ctrl/Cmd + Shift + Space`: record toggle
- `Ctrl/Cmd + Shift + Enter`: paste current transcript

Notes:

- these can collide with user or desktop shortcuts
- if they do, change them in widget settings

## Tauri capability permissions

The widget uses Tauri window APIs for:

- drag handling
- size changes
- position changes

If contributors see errors like:

- `window.set_size not allowed`
- `window.set_position not allowed`

they should check:

- [default.json](../src-tauri/capabilities/default.json)

The required permissions are already configured in this repo.
