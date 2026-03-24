# Third-Party Notices

This project depends on third-party software. Each dependency remains subject to its own license.

This file is intended to document the main third-party components used by the current Linux-first Local Voice release surface, including the source setup and the current Linux AppImage packaging flow.

It is not legal advice.

## Scope

Current scope:

- public source repository
- Ubuntu/Debian-like Linux setup
- Linux AppImage build flow for maintainers
- system-installed FFmpeg
- locally created Python environment
- locally downloaded speech model files

Important:

- this repository does **not** intend to redistribute downloaded model weights by default
- this repository does **not** bundle FFmpeg by default in the current source-based setup flow
- packaged Linux builds may bundle the Python backend and Python dependencies
- packaged Linux builds still do **not** bundle FFmpeg by default
- if maintainers later ship packaged binaries that bundle FFmpeg, models, or additional third-party binaries, this notice file should be reviewed and expanded
- current recommendation: keep FFmpeg as a user-installed system dependency
- packaged artifacts should also be audited for bundled system libraries before public binary distribution

## Application dependencies

### Tauri

- project: Tauri
- role: desktop application framework
- license: `Apache-2.0 OR MIT`
- homepage: https://tauri.app/

### Tauri API

- package: `@tauri-apps/api`
- role: frontend bridge to native desktop APIs
- license: `Apache-2.0 OR MIT`
- homepage: https://github.com/tauri-apps/tauri

### Tauri CLI

- package: `@tauri-apps/cli`
- role: local development and build tooling
- license: `Apache-2.0 OR MIT`
- homepage: https://github.com/tauri-apps/tauri

### Tauri Global Shortcut Plugin

- package: `@tauri-apps/plugin-global-shortcut`
- role: global shortcut registration
- license: `MIT OR Apache-2.0`
- homepage: https://github.com/tauri-apps/plugins-workspace

### Vite

- package: `vite`
- role: frontend dev server and build pipeline
- license: `MIT`
- homepage: https://vite.dev/

### TypeScript

- package: `typescript`
- role: frontend type checking and compilation
- license: `Apache-2.0`
- homepage: https://www.typescriptlang.org/

## Python backend dependencies

### faster-whisper

- package: `faster-whisper`
- role: offline transcription
- license: `MIT`
- homepage: https://github.com/SYSTRAN/faster-whisper

### CTranslate2

- package: `ctranslate2`
- role: inference runtime used by `faster-whisper`
- license: `MIT`
- homepage: https://github.com/OpenNMT/CTranslate2

### tokenizers

- package: `tokenizers`
- role: tokenizer implementation used in the transcription stack
- license: `Apache-2.0`
- homepage: https://github.com/huggingface/tokenizers

### huggingface-hub

- package: `huggingface-hub`
- role: model download support during setup
- license: `Apache-2.0`
- homepage: https://github.com/huggingface/huggingface_hub

## System dependency notes

### FFmpeg

- project: FFmpeg
- role: audio capture and normalization pipeline
- install mode in current setup: system package dependency
- upstream homepage: https://ffmpeg.org/

Important note:

- the Ubuntu FFmpeg build used during development on this machine reports `GPL` licensing
- if the project later distributes bundled FFmpeg binaries, maintainers must review the exact binary build license terms and corresponding redistribution obligations

## Models

Speech model files downloaded during setup are upstream third-party assets.

Current policy:

- models are downloaded locally by the user during setup
- model files are not intended to be committed into this repository
- maintainers should review upstream model licenses or terms separately before redistributing any model weights

## Maintainer note

When adding or bundling new third-party components, update:

- this file
- [README.md](README.md)
- [docs/releasing.md](docs/releasing.md)
