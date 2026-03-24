# Releasing

This project should be released Linux-first.

## Release target

Initial release target:

- public GitHub repository
- source-based setup documented and tested
- Linux AppImage packaging available for maintainers

Current packaging policy:

- do not bundle FFmpeg in the first public release
- require FFmpeg as a system-installed dependency on Linux
- only revisit bundled FFmpeg after a deliberate license/compliance review
- AppImage builds may bundle the Python backend, but do not bundle model weights by default
- published GitHub Releases should build and attach the AppImage automatically

## Before cutting a release

1. Verify the public repo surface.
2. Run the clean Ubuntu smoke test.
3. Run the setup and doctor scripts on a real clean Ubuntu machine or VM.
4. Verify the app works offline after setup.
5. Verify recording, transcription, edit, copy, paste, tray, and shortcuts.
6. Confirm the troubleshooting docs match real failures.
7. Confirm the real desktop checklist in [desktop-verification.md](desktop-verification.md) has been completed.

## Verification commands

From the repo root:

```bash
npm run smoke:ubuntu
npm run doctor
npm run build
source "$HOME/.cargo/env" && cargo check --manifest-path src-tauri/Cargo.toml
```

For a local desktop build:

```bash
npm run tauri:build
```

For a packaged Linux build:

```bash
npm run package:linux
```

GitHub Actions release automation:

- `.github/workflows/release-appimage.yml` runs on `release.published`
- it bootstraps the supported Ubuntu build path
- it builds the AppImage
- it uploads the `.AppImage` and `.sha256` file back to the GitHub Release

Current packaged-build expectations:

- the AppImage bundles the Tauri app and the Python backend
- the bundled backend currently assumes a compatible host `python3`, so this is not yet a fully self-contained Python runtime
- the AppImage still expects `ffmpeg` to be installed on the target Linux system
- the AppImage does not bundle the default whisper model unless a maintainer explicitly uses `scripts/build-package-linux.sh --with-model`
- if a maintainer chooses to bundle model weights later, they must review redistribution terms and update [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md)

## Release checklist

1. Update version fields if needed.
2. Confirm `.gitignore` excludes local models, caches, and build outputs.
3. Confirm the README still matches the actual setup path.
4. Confirm [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md) still reflects bundled or documented dependencies.
5. Confirm `scripts/bootstrap-ubuntu.sh` still works.
6. Confirm `scripts/doctor.sh` still reflects current prerequisites.
7. Confirm [desktop-verification.md](desktop-verification.md) still matches the real Ubuntu session behavior.
8. Confirm tray, shortcuts, and paste behavior on the target desktop session.
9. If publishing packaged artifacts, verify `npm run package:linux` succeeds and the AppImage launches on a clean Ubuntu machine with system `ffmpeg` installed.
10. Tag and publish.

## Scope discipline

Do not expand the release scope during the release pass.

Specifically avoid adding:

- new platforms
- real-time streaming
- history management
- cloud integrations

Release the stable Linux-first path first. Packaged artifacts are useful, but they should not silently expand the dependency or licensing surface.
