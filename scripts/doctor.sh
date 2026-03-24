#!/usr/bin/env bash

set -uo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PYTHON_BACKEND_DIR="$ROOT_DIR/python-backend"
BACKEND_PYTHON="$PYTHON_BACKEND_DIR/.venv/bin/python"
MODEL_DIR="$PYTHON_BACKEND_DIR/models/faster-whisper-small"

FAILURES=0
WARNINGS=0

pass() {
  printf '[pass] %s\n' "$1"
}

warn() {
  WARNINGS=$((WARNINGS + 1))
  printf '[warn] %s\n' "$1"
}

fail() {
  FAILURES=$((FAILURES + 1))
  printf '[fail] %s\n' "$1"
}

have_cmd() {
  command -v "$1" >/dev/null 2>&1
}

if [[ -f "$HOME/.cargo/env" ]]; then
  # shellcheck disable=SC1090
  source "$HOME/.cargo/env"
fi

if [[ -d "$HOME/.local/bin" ]]; then
  export PATH="$HOME/.local/bin:$PATH"
fi

check_cmd() {
  local cmd="$1"
  local label="$2"

  if have_cmd "$cmd"; then
    pass "$label: $(command -v "$cmd")"
  else
    fail "$label is missing"
  fi
}

echo "Local Voice doctor"
echo "Repo: $ROOT_DIR"
echo

if [[ "${OSTYPE:-}" == linux* ]]; then
  pass "Linux detected: $(uname -sr)"
else
  warn "This doctor script is tuned for Linux. Current OS: ${OSTYPE:-unknown}"
fi

SESSION_TYPE="${XDG_SESSION_TYPE:-unknown}"
DESKTOP_ENV="${XDG_CURRENT_DESKTOP:-unknown}"

pass "Session type: $SESSION_TYPE"
pass "Desktop environment: $DESKTOP_ENV"

if [[ "$SESSION_TYPE" == "wayland" ]]; then
  warn "Wayland detected. Cross-app paste automation can be limited compared with X11."
fi

if [[ "$DESKTOP_ENV" == *GNOME* && "$DESKTOP_ENV" != *ubuntu* && "$DESKTOP_ENV" != *Ubuntu* ]]; then
  warn "GNOME detected. Tray/appindicator visibility can depend on installed shell extensions."
fi

echo
check_cmd node "Node.js"
check_cmd npm "npm"
check_cmd cargo "cargo"
check_cmd rustc "rustc"
check_cmd uv "uv"
check_cmd ffmpeg "FFmpeg"

if have_cmd pkg-config; then
  if pkg-config --exists webkit2gtk-4.1; then
    pass "webkit2gtk-4.1 development package found"
  else
    fail "webkit2gtk-4.1 development package is missing"
  fi

  if pkg-config --exists ayatana-appindicator3-0.1 || pkg-config --exists appindicator3-0.1; then
    pass "AppIndicator development package found"
  else
    fail "AppIndicator development package is missing"
  fi
else
  fail "pkg-config is missing"
fi

echo
if [[ -x "$BACKEND_PYTHON" ]]; then
  pass "Backend virtualenv Python found"
  if "$BACKEND_PYTHON" -c "import faster_whisper, ctranslate2" >/dev/null 2>&1; then
    pass "Backend imports: faster_whisper and ctranslate2"
  else
    fail "Backend Python environment exists but faster_whisper/ctranslate2 imports failed"
  fi
else
  fail "Backend virtualenv is missing. Run: (cd python-backend && uv sync --frozen)"
fi

if [[ -d "$MODEL_DIR" ]]; then
  pass "Local model found: $MODEL_DIR"
else
  warn "Local model not found. Offline-first usage is best with a downloaded local model."
fi

echo
if [[ "$FAILURES" -gt 0 ]]; then
  printf 'Doctor finished with %d failure(s) and %d warning(s).\n' "$FAILURES" "$WARNINGS"
  exit 1
fi

printf 'Doctor finished with %d warning(s).\n' "$WARNINGS"
