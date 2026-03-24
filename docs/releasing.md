# Releasing

This project should be released Linux-first.

## Release target

Initial release target:

- public GitHub repository
- source-based setup documented and tested
- optional packaged binary after the source path is stable

Current packaging policy:

- do not bundle FFmpeg in the first public release
- require FFmpeg as a system-installed dependency on Linux
- only revisit bundled FFmpeg after a deliberate license/compliance review

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

## Release checklist

1. Update version fields if needed.
2. Confirm `.gitignore` excludes local models, caches, and build outputs.
3. Confirm the README still matches the actual setup path.
4. Confirm [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md) still reflects bundled or documented dependencies.
5. Confirm `scripts/bootstrap-ubuntu.sh` still works.
6. Confirm `scripts/doctor.sh` still reflects current prerequisites.
7. Confirm [desktop-verification.md](desktop-verification.md) still matches the real Ubuntu session behavior.
8. Confirm tray, shortcuts, and paste behavior on the target desktop session.
9. Tag and publish.

## Scope discipline

Do not expand the release scope during the release pass.

Specifically avoid adding:

- new platforms
- real-time streaming
- history management
- cloud integrations

Release the stable Linux-first source path first.
