#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PYTHON_BACKEND_DIR="$ROOT_DIR/python-backend"
MODEL_DIR="$PYTHON_BACKEND_DIR/models/faster-whisper-small"
NODESOURCE_NODE_MAJOR=20

SKIP_SYSTEM_DEPS=0
SKIP_MODEL_DOWNLOAD=0

usage() {
  cat <<'EOF'
Usage: ./setup.sh [--skip-system-deps] [--skip-model-download]

Options:
  --skip-system-deps     Skip apt-based system package installation.
  --skip-model-download  Skip downloading the default local small model.
  -h, --help             Show this help message.
EOF
}

for arg in "$@"; do
  case "$arg" in
    --skip-system-deps)
      SKIP_SYSTEM_DEPS=1
      ;;
    --skip-model-download)
      SKIP_MODEL_DOWNLOAD=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $arg" >&2
      usage >&2
      exit 1
      ;;
  esac
done

have_cmd() {
  command -v "$1" >/dev/null 2>&1
}

apt_run() {
  if [[ "$(id -u)" -eq 0 ]]; then
    apt-get "$@"
  else
    sudo apt-get "$@"
  fi
}

install_file_with_privileges() {
  local src="$1"
  local dest="$2"
  local mode="${3:-0644}"

  if [[ "$(id -u)" -eq 0 ]]; then
    install -m "$mode" "$src" "$dest"
  else
    sudo install -m "$mode" "$src" "$dest"
  fi
}

ensure_dir_with_privileges() {
  local dest="$1"
  local mode="${2:-0755}"

  if [[ "$(id -u)" -eq 0 ]]; then
    install -d -m "$mode" "$dest"
  else
    sudo install -d -m "$mode" "$dest"
  fi
}

node_major_or_zero() {
  if have_cmd node; then
    node -p 'process.versions.node.split(`.`)[0]'
  else
    echo 0
  fi
}

install_nodesource_node() {
  local keyring_tmp repo_tmp
  keyring_tmp="$(mktemp)"
  repo_tmp="$(mktemp)"

  echo "Installing Node.js ${NODESOURCE_NODE_MAJOR}.x from NodeSource."
  ensure_dir_with_privileges /etc/apt/keyrings 0755
  curl -fsSL https://deb.nodesource.com/gpgkey/nodesource-repo.gpg.key \
    | gpg --batch --yes --dearmor -o "$keyring_tmp"
  printf 'deb [signed-by=/etc/apt/keyrings/nodesource.gpg] https://deb.nodesource.com/node_%s.x nodistro main\n' \
    "$NODESOURCE_NODE_MAJOR" >"$repo_tmp"

  install_file_with_privileges "$keyring_tmp" /etc/apt/keyrings/nodesource.gpg 0644
  install_file_with_privileges "$repo_tmp" /etc/apt/sources.list.d/nodesource.list 0644

  apt_run update
  apt_run install -y nodejs
  rm -f "$keyring_tmp" "$repo_tmp"
}

ensure_supported_node() {
  local node_major
  node_major="$(node_major_or_zero)"

  if [[ "$node_major" -lt 18 ]] || ! have_cmd npm; then
    install_nodesource_node
    node_major="$(node_major_or_zero)"
  fi

  if ! have_cmd node || ! have_cmd npm; then
    echo "Node.js or npm is still missing after automatic installation." >&2
    exit 1
  fi

  if [[ "$node_major" -lt 18 ]]; then
    echo "Node.js 18+ is required. Found: $(node --version)" >&2
    exit 1
  fi
}

install_system_deps() {
  if [[ "$SKIP_SYSTEM_DEPS" -eq 1 ]]; then
    return
  fi

  if [[ "${OSTYPE:-}" != linux* ]]; then
    echo "This bootstrap script currently targets Ubuntu/Debian-like Linux systems." >&2
    exit 1
  fi

  if ! have_cmd apt-get; then
    echo "apt-get is required for automatic system dependency installation." >&2
    exit 1
  fi

  if [[ "$(id -u)" -ne 0 ]] && ! have_cmd sudo; then
    echo "sudo is required for automatic system dependency installation when not running as root." >&2
    exit 1
  fi

  apt_run update
  apt_run install -y \
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
}

install_rustup_if_needed() {
  if have_cmd cargo && have_cmd rustc; then
    return
  fi

  curl https://sh.rustup.rs -sSf | sh -s -- -y
}

install_uv_if_needed() {
  if have_cmd uv; then
    return
  fi

  curl -LsSf https://astral.sh/uv/install.sh | sh
}

load_local_toolchains() {
  if [[ -f "$HOME/.cargo/env" ]]; then
    # shellcheck disable=SC1090
    source "$HOME/.cargo/env"
  fi

  if [[ -d "$HOME/.local/bin" ]]; then
    export PATH="$HOME/.local/bin:$PATH"
  fi
}

main() {
  cd "$ROOT_DIR"

  install_system_deps
  install_rustup_if_needed
  install_uv_if_needed
  load_local_toolchains
  ensure_supported_node

  if ! have_cmd cargo || ! have_cmd rustc; then
    echo "Rust toolchain installation did not complete correctly." >&2
    exit 1
  fi

  if ! have_cmd uv; then
    echo "uv installation did not complete correctly." >&2
    exit 1
  fi

  npm ci

  (
    cd "$PYTHON_BACKEND_DIR"
    uv sync --frozen
    if [[ "$SKIP_MODEL_DOWNLOAD" -eq 0 && ! -d "$MODEL_DIR" ]]; then
      uv run python -m offline_voice_worker.cli download-model --model-profile small --output-dir models
    fi
  )

  cat <<'EOF'

Bootstrap complete.

Next steps:
  ./doctor.sh
  ./run-dev.sh

If you want a fully offline setup, make sure the local model exists at:
  python-backend/models/faster-whisper-small
EOF
}

main "$@"
