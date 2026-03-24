#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BACKEND_DIR="$ROOT_DIR/python-backend"
RESOURCE_ROOT="$ROOT_DIR/src-tauri/resources"
BUNDLE_DIR="$RESOURCE_ROOT/bundled-backend"
INCLUDE_MODEL=0

usage() {
  cat <<'USAGE'
Usage: bash ./scripts/prepare-bundled-backend.sh [--with-model]

Options:
  --with-model  Include the local faster-whisper-small model in the packaged backend resources.
  -h, --help    Show this help message.
USAGE
}

for arg in "$@"; do
  case "$arg" in
    --with-model)
      INCLUDE_MODEL=1
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

if [[ ! -d "$BACKEND_DIR/.venv" ]]; then
  echo "python-backend/.venv is missing. Run ./setup.sh or npm run backend:sync first." >&2
  exit 1
fi

rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR"

cp -a "$BACKEND_DIR/.venv" "$BUNDLE_DIR/.venv"
cp -a "$BACKEND_DIR/offline_voice_worker" "$BUNDLE_DIR/offline_voice_worker"
cp -a "$BACKEND_DIR/pyproject.toml" "$BUNDLE_DIR/pyproject.toml"
cp -a "$BACKEND_DIR/uv.lock" "$BUNDLE_DIR/uv.lock"
cp -a "$BACKEND_DIR/README.md" "$BUNDLE_DIR/README.md"

if [[ "$INCLUDE_MODEL" -eq 1 ]]; then
  MODEL_DIR="$BACKEND_DIR/models/faster-whisper-small"
  if [[ ! -d "$MODEL_DIR" ]]; then
    echo "Requested --with-model but $MODEL_DIR is missing." >&2
    exit 1
  fi
  mkdir -p "$BUNDLE_DIR/models"
  cp -a "$MODEL_DIR" "$BUNDLE_DIR/models/faster-whisper-small"
fi

find "$BUNDLE_DIR" \( -type d \( -name '__pycache__' -o -name '*.egg-info' \) -o -type f \( -name '*.pyc' -o -name '*.pyo' \) \) -print0 \
  | xargs -0r rm -rf

echo "Prepared bundled backend resources at: $BUNDLE_DIR"
if [[ "$INCLUDE_MODEL" -eq 1 ]]; then
  echo "Included bundled model: faster-whisper-small"
else
  echo "Model not bundled. Packaged app will expect a local model download or configured model path."
fi
