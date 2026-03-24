# Ubuntu Installation

This project is currently tuned for Ubuntu / Debian-like Linux.

## Fast path

From the repository root:

```bash
./setup.sh
./doctor.sh
./run-dev.sh
```

That is the intended source-based setup path for contributors and testers.

## Setup script flags

The bootstrap wrapper passes arguments through to `scripts/bootstrap-ubuntu.sh`.

Supported flags:

- `--skip-system-deps`: skip `apt` installs if the machine already has the required system packages
- `--skip-model-download`: skip the default local model download if you only want the toolchain first

Examples:

```bash
./setup.sh --skip-system-deps
./setup.sh --skip-model-download
```

## What the setup script installs

System side:

- `ffmpeg`
- Tauri Linux prerequisites such as `webkit2gtk`, GTK, AppIndicator, and `pkg-config`
- Node.js 20.x from NodeSource when the local Node install is missing or too old

Developer toolchains:

- Rust via `rustup` if missing
- `uv` if missing

Project dependencies:

- frontend dependencies with `npm ci`
- backend Python environment with `uv sync --frozen`
- the default local `small` model unless `--skip-model-download` is used

## Manual setup

### 1. Install Linux system dependencies

```bash
sudo apt-get update
sudo apt-get install -y \
  ffmpeg \
  libwebkit2gtk-4.1-dev \
  build-essential \
  ca-certificates \
  curl \
  gnupg \
  wget \
  file \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libgtk-3-dev \
  pkg-config
```

### 2. Install Node.js

The project needs Node.js 18+, but the setup script installs Node.js 20.x from NodeSource.

```bash
sudo mkdir -p /etc/apt/keyrings
curl -fsSL https://deb.nodesource.com/gpgkey/nodesource-repo.gpg.key \
  | sudo gpg --dearmor -o /etc/apt/keyrings/nodesource.gpg
echo "deb [signed-by=/etc/apt/keyrings/nodesource.gpg] https://deb.nodesource.com/node_20.x nodistro main" \
  | sudo tee /etc/apt/sources.list.d/nodesource.list >/dev/null
sudo apt-get update
sudo apt-get install -y nodejs
```

### 3. Install Rust

```bash
curl https://sh.rustup.rs -sSf | sh
source "$HOME/.cargo/env"
```

### 4. Install `uv`

```bash
curl -LsSf https://astral.sh/uv/install.sh | sh
export PATH="$HOME/.local/bin:$PATH"
```

### 5. Install project dependencies

```bash
npm ci
cd python-backend && uv sync --frozen && cd ..
npm run model:download
```

### 6. Verify and run

```bash
./doctor.sh
./run-dev.sh
```

## Clean-machine smoke test

To validate the setup path in Docker:

```bash
npm run smoke:ubuntu
```

That smoke test covers the clean Ubuntu source setup and build path. It does not replace a real desktop verification pass.
