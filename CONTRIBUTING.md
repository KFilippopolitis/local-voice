# Contributing

This project is currently Linux-first and source-setup-first.

Before changing code, use the repo-root flow:

```bash
./setup.sh
./doctor.sh
./run-dev.sh
```

## Before submitting changes

Run the checks that match the scope of your change.

Common checks:

```bash
npm run build
source "$HOME/.cargo/env" && cargo check --manifest-path src-tauri/Cargo.toml
./doctor.sh
```

If you touch setup or release behavior, also run:

```bash
npm run smoke:ubuntu
```

## Documentation discipline

If behavior changes, update the public docs in the same pass.

Most commonly affected files:

- `README.md`
- `docs/install-ubuntu.md`
- `docs/permissions-and-platforms.md`
- `docs/troubleshooting.md`
- `docs/releasing.md`
- `THIRD_PARTY_NOTICES.md`
- `AGENTS.md`

## Repo hygiene

Do not commit:

- local model downloads under `python-backend/models/`
- virtualenv contents under `python-backend/.venv/`
- build output such as `dist/` or `src-tauri/target/`
- generated Python egg-info or `__pycache__` directories

## Scope discipline

Do not expand scope during cleanup or release passes.

Still out of scope unless explicitly requested:

- real-time streaming transcription
- transcript history
- cloud integrations
- remote APIs
