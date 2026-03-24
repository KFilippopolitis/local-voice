#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE_NAME="local-voice-ubuntu-smoke"
SKIP_MODEL_DOWNLOAD=1

for arg in "$@"; do
  case "$arg" in
    --with-model)
      SKIP_MODEL_DOWNLOAD=0
      ;;
    *)
      echo "Unknown argument: $arg" >&2
      echo "Usage: bash ./scripts/smoke-test-ubuntu-docker.sh [--with-model]" >&2
      exit 1
      ;;
  esac
done

if ! command -v docker >/dev/null 2>&1; then
  echo "docker is required for the Ubuntu smoke test." >&2
  exit 1
fi

cd "$ROOT_DIR"

docker build \
  -f docker/ubuntu-smoke.Dockerfile \
  --build-arg SKIP_MODEL_DOWNLOAD="$SKIP_MODEL_DOWNLOAD" \
  -t "$IMAGE_NAME" \
  .

echo
echo "Ubuntu smoke test passed in Docker image: $IMAGE_NAME"
if [[ "$SKIP_MODEL_DOWNLOAD" == "1" ]]; then
  echo "Model download was skipped for speed. Use --with-model to include it."
fi
