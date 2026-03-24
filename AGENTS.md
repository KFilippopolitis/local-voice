# AGENTS.md

## Project

Local Voice

## Intent

This repository contains a small offline desktop voice scratchpad for terminal and agent workflows.

Core flow:

- record with the widget or a global shortcut
- stop with the same shortcut
- transcribe locally with `faster-whisper`
- edit the current transcript inside the widget
- copy it, clear it, or paste it into the currently focused app

The project is privacy-first:

- offline after setup
- no backend server
- no cloud APIs
- no telemetry by default
- no transcript history by default

## Current product reality

The repo is no longer at the pure planning stage. It already implements:

- Tauri desktop shell
- Rust native backend in `src-tauri/src/lib.rs`
- Python transcription worker in `python-backend/offline_voice_worker/cli.py`
- native Linux microphone capture with `ffmpeg`
- local `faster-whisper` transcription
- editable transcript UI
- tray/app-indicator integration
- global shortcuts for record toggle and paste-current-transcript
- temp cleanup under `/tmp/local-voice`

Treat this as a Linux-first productization effort, not as a greenfield prototype.

## Supported path

Primary tested path:

- Ubuntu GNOME on X11

Likely-supported path:

- Lubuntu / LXQt on X11

Secondary path with more caveats:

- Wayland sessions

Assume Linux source setup first. Do not describe the app as fully cross-platform unless that has actually been implemented and verified.

## Setup flow

Preferred source setup from the repo root:

```bash
./setup.sh
./doctor.sh
./run-dev.sh
```

Important details:

- `setup.sh` delegates to `scripts/bootstrap-ubuntu.sh`
- the bootstrap path installs system dependencies, Node.js if needed, Rust if needed, and `uv` if needed
- the backend environment is managed with `uv`
- the default local model path is `python-backend/models/faster-whisper-small`

## Repo map

- `src/`: frontend widget UI and state
- `src-tauri/`: native shell, tray, shortcuts, cleanup, clipboard, paste behavior
- `python-backend/`: transcription worker and model tooling
- `scripts/`: setup, doctor, smoke-test, and verification helpers
- `docs/`: install, permissions, troubleshooting, desktop verification, release notes

## Product constraints

Keep these true unless the user explicitly changes scope:

- offline-first after setup
- no transcript/audio history by default
- no uploads
- no automatic shell execution from transcript text
- temp artefacts must stay under the app temp root
- cleanup must run every 5 minutes and stay within that allowlisted temp root
- single active transcript is enough for the current product

## Current implementation guidance

### Microphone capture

On Linux, microphone capture is native through `ffmpeg`, not browser-style `getUserMedia`.

Do not reintroduce webview microphone capture on Linux as the default path unless there is a clear reason and it is verified to be more reliable.

### Paste behavior

Linux paste automation is best-effort.

Current intended behavior:

- terminal-like windows: `Ctrl+Shift+V`
- general GUI apps: `Ctrl+V`
- X11 is the strongest path
- Wayland may limit synthetic input behavior

Do not document Linux paste as universally reliable across all desktops.

### FFmpeg policy

The current public repo expects system-installed `ffmpeg`.

Do not silently switch to bundled FFmpeg in a release-oriented change without updating:

- `README.md`
- `THIRD_PARTY_NOTICES.md`
- `docs/releasing.md`

### Models

Downloaded model weights are local setup artefacts, not normal repo content.

Do not commit model directories into version control.

## Commands contributors should use

Useful repo-root commands:

```bash
./setup.sh
./doctor.sh
./run-dev.sh
npm run build
npm run smoke:ubuntu
source "$HOME/.cargo/env" && cargo check --manifest-path src-tauri/Cargo.toml
```

## Documentation discipline

If behavior changes, update the relevant docs in the same pass.

Most common files that must stay aligned:

- `README.md`
- `docs/install-ubuntu.md`
- `docs/permissions-and-platforms.md`
- `docs/troubleshooting.md`
- `docs/releasing.md`
- `THIRD_PARTY_NOTICES.md`

## Scope discipline

Do not expand scope during cleanup or release passes.

Still out of scope unless explicitly requested:

- real-time streaming transcription
- transcript history manager
- multi-transcript queue
- diarization
- cloud sync
- account system
- remote APIs

## Quality bar

Prefer:

- small, testable modules
- explicit user-facing error messages
- Linux behavior that is documented honestly
- reliable local setup over ambitious packaging changes
- repo cleanliness suitable for a public GitHub source release
