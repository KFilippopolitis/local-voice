# local-voice

local-voice is a small desktop voice scratchpad for terminal and agent workflows.

It is built to be:

- offline after setup
- fast to trigger with a global shortcut
- editable before copy or paste
- disposable by default, with no transcript history

Current implementation:

- Tauri desktop shell
- native Linux microphone capture through `ffmpeg`
- local Python backend managed with `uv`
- `faster-whisper` transcription
- editable transcript panel
- global shortcuts for record toggle and paste-current-transcript
- tray/app-indicator integration

## Current support

This repository is currently prepared for:

- Ubuntu / Debian-like Linux
- source-based local setup
- maintainer-built AppImage packaging on Linux
- offline use after the first setup and local model download

Desktop support status:

- Ubuntu GNOME on X11: tested primary path
- Lubuntu / LXQt on X11: likely supported, but not yet fully desktop-verified
- Wayland sessions: secondary path with more paste/input caveats

## Quick start

From the repository root:

```bash
./setup.sh
./doctor.sh
./run-dev.sh
```

That setup flow will:

- install Linux system packages needed for Tauri and `ffmpeg`
- install Node.js 20.x from NodeSource if a suitable local Node install is missing
- install `rustup` if missing
- install `uv` if missing
- run `npm ci`
- create and sync the Python backend environment
- download the default local `small` model if it is missing

First-time setup is not tiny:

- on a fresh machine, expect several minutes rather than seconds
- the default local `small` model is about `464 MB`
- the heavy part is the one-time toolchain and model download, not normal app usage

## Packaged install

The easiest end-user path is now a Linux AppImage release rather than a source checkout.

Current packaged build status:

- AppImage packaging works on the primary Ubuntu path
- published GitHub Releases can attach the AppImage automatically
- the packaged app bundles the Tauri shell and Python backend
- the packaged backend currently expects a compatible system `python3` on the target machine
- the packaged app still expects system-installed `ffmpeg`
- the packaged app does **not** bundle a speech model by default

For maintainers, build the Linux package from the repo root with:

```bash
npm run package:linux
```

For end users, the intended path is:

1. open the latest GitHub Release
2. download the `.AppImage`
3. ensure `ffmpeg` and a compatible `python3` exist on the machine
4. launch the AppImage
5. download the default model from Settings on first use if needed

Current verified output on this machine:

- `src-tauri/target/release/bundle/appimage/Local Voice_0.1.0_amd64.AppImage`

Important packaged-app first-run note:

- users still need a local model before transcription works
- the packaged app can download the default `small` model from the settings panel
- maintainers can bundle the model with `scripts/build-package-linux.sh --with-model`, but that should be treated as a separate redistribution decision

## Common commands

```bash
./doctor.sh
./run-dev.sh
npm run build
npm run package:linux
source "$HOME/.cargo/env" && cargo check --manifest-path src-tauri/Cargo.toml
npm run smoke:ubuntu
```

## Privacy model

- no cloud APIs
- no telemetry by default
- no transcript history by default
- no backend server
- temp audio and transcript artefacts are cleaned on a schedule

## Linux behavior that matters

- Linux microphone capture is native through `ffmpeg`, not browser-style `getUserMedia`
- FFmpeg is treated as a system dependency and is not bundled in the current source release
- the tray is implemented as an app-indicator, not as a custom GNOME shell widget
- Linux paste automation is best-effort:
  - terminal-like windows use `Ctrl+Shift+V`
  - general GUI apps use `Ctrl+V`
- X11 is the strongest path for cross-app paste behavior right now

## Docs

- [Ubuntu installation details](docs/install-ubuntu.md)
- [Contributing](CONTRIBUTING.md)
- [Permissions and platform notes](docs/permissions-and-platforms.md)
- [Troubleshooting](docs/troubleshooting.md)
- [Desktop verification](docs/desktop-verification.md)
- [Release checklist](docs/releasing.md)
- [Third-party notices](THIRD_PARTY_NOTICES.md)
- [Backend notes](python-backend/README.md)

## Project layout

- `src/`: frontend widget UI and local state
- `src-tauri/`: native shell, tray, shortcuts, cleanup, clipboard, paste integration
- `python-backend/`: FFmpeg + `faster-whisper` worker
- `scripts/`: setup, doctor, smoke-test, and verification helpers
- `docs/`: install, platform notes, troubleshooting, and release workflow

## Known limitations

- Linux is the primary supported platform right now
- paste automation is less reliable on Wayland than on X11
- Lubuntu / LXQt on X11 is a likely good fit, but is not yet marked as fully tested
- the Python worker is still one-shot, so first-transcript latency is higher than a warm worker design
