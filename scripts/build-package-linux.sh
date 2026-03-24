#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WITH_MODEL=0
TAURI_ARGS=()

usage() {
  cat <<'USAGE'
Usage: bash ./scripts/build-package-linux.sh [--with-model] [-- <tauri build args>]

Examples:
  bash ./scripts/build-package-linux.sh
  bash ./scripts/build-package-linux.sh --with-model
  bash ./scripts/build-package-linux.sh -- --bundles appimage
USAGE
}

while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --with-model)
      WITH_MODEL=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    --)
      shift
      TAURI_ARGS+=("$@")
      break
      ;;
    *)
      TAURI_ARGS+=("$1")
      shift
      ;;
  esac
done

cd "$ROOT_DIR"

if [[ -f "$HOME/.cargo/env" ]]; then
  # shellcheck disable=SC1090
  source "$HOME/.cargo/env"
fi

if [[ -d "$HOME/.local/bin" ]]; then
  export PATH="$HOME/.local/bin:$PATH"
fi

if [[ "$WITH_MODEL" -eq 1 ]]; then
  bash ./scripts/prepare-bundled-backend.sh --with-model
else
  bash ./scripts/prepare-bundled-backend.sh
fi

BUNDLED_BACKEND_DIR="$ROOT_DIR/src-tauri/resources/bundled-backend"
if [[ -d "$BUNDLED_BACKEND_DIR" ]]; then
  mapfile -t bundled_lib_dirs < <(
    find "$BUNDLED_BACKEND_DIR" -type f \( -name '*.so' -o -name '*.so.*' \) -printf '%h\n' | sort -u
  )

  if [[ "${#bundled_lib_dirs[@]}" -gt 0 ]]; then
    bundled_ld_path="$(printf '%s:' "${bundled_lib_dirs[@]}")"
    bundled_ld_path="${bundled_ld_path%:}"
    if [[ -n "${LD_LIBRARY_PATH:-}" ]]; then
      export LD_LIBRARY_PATH="$bundled_ld_path:$LD_LIBRARY_PATH"
    else
      export LD_LIBRARY_PATH="$bundled_ld_path"
    fi
    echo "Configured LD_LIBRARY_PATH for bundled backend (${#bundled_lib_dirs[@]} directories)."
  fi
fi

npm run tauri:build -- "${TAURI_ARGS[@]}"
