# Desktop Verification

Use this document for the real Ubuntu desktop pass that complements the Docker smoke test.

The Docker smoke test validates setup and build. It does not validate actual desktop integration.

## Current desktop support statement

- Ubuntu GNOME on X11: tested primary path
- Lubuntu / LXQt on X11: likely supported, but still needs a real desktop verification pass
- Wayland sessions: secondary path with more paste/input caveats

## What to verify on a real Ubuntu session

1. The tray/app-indicator icon appears in the expected tray/panel area for the desktop.
2. The widget window can be shown and hidden.
3. The global record shortcut starts recording.
4. The same shortcut stops recording.
5. A transcript appears after transcription completes.
6. The transcript can be edited, copied, deleted, and collapsed.
7. The paste shortcut inserts the current transcript into another focused app.
8. The microphone path works from the actual desktop session.
9. The app still works with networking disabled after setup and local model download.

## Current local verification

Local verification was run on `2026-03-24` on this machine.

- session: `ubuntu:GNOME`
- session type: `x11`
- display: `:1`
- tray watcher present: `org.kde.StatusNotifierWatcher`
- registered tray item: `tray_icon_tray_app_voice_tray`
- app process present: `target/debug/local-voice`
- X11 client present: `Local Voice`
- direct microphone path check passed with native `ffmpeg` capture

Concrete microphone check result:

```text
ffmpeg -f pulse -i default -t 1.5 /tmp/...wav
size=298794 bytes
```

That confirms the live Ubuntu desktop session can access the microphone path that the app depends on.

## What remains manual

These items were not fully automated in the current release pass:

- global shortcut toggle end-to-end
- paste shortcut end-to-end into another focused app
- transcript editing flow after a real recording

Reason:

- the current machine does not have `xdotool` installed
- installing it requires interactive `sudo`
- without input automation, those checks still need a maintainer to perform them manually on the desktop

## Recommended X11 helper for future release verification

On Ubuntu X11, install:

```bash
sudo apt-get install -y xdotool
```

That makes it practical to automate:

- shortcut keypress simulation
- focused-window switching
- basic end-to-end desktop flow checks

## Release rule

Do not treat a Docker smoke pass as sufficient for a desktop release.

Before a tagged release, at least one maintainer should complete the real Ubuntu desktop checklist above.

For Lubuntu support to move from "likely supported" to "tested", run the same checklist on a real Lubuntu / LXQt X11 session.
