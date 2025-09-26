#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: ${0##*/} [options]

Options:
  -j, --jobs N       Number of parallel jobs for make (default: auto-detect).
  -t, --run-tests    Run the test suite (make check) after the build completes.
  -h, --help         Show this help message and exit.
USAGE
}

# Default settings.
JOBS=${JOBS:-$(sysctl -n hw.ncpu 2>/dev/null || echo 4)}
RUN_TESTS=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    -j|--jobs)
      [[ $# -ge 2 ]] || { echo "${0##*/}: missing value for $1" >&2; exit 1; }
      JOBS="$2"
      shift 2
      ;;
    -t|--run-tests)
      RUN_TESTS=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "${0##*/}: unknown option '$1'" >&2
      usage >&2
      exit 1
      ;;
  esac
done

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

echo "==> Bootstrapping build system"
./autogen.sh all

# Prefer Homebrew-provided toolchain pieces if present.
if command -v brew >/dev/null 2>&1; then
  HOMEBREW_PREFIX="$(brew --prefix)"
  export PATH="$HOMEBREW_PREFIX/opt/texinfo/bin:$HOMEBREW_PREFIX/bin:$PATH"
fi

CONFIG_FLAGS=("--with-ns" "--with-modules")

echo "==> Configuring (FLAGS: ${CONFIG_FLAGS[*]})"
./configure "${CONFIG_FLAGS[@]}"

echo "==> Building (make -j${JOBS})"
make -j"${JOBS}"

if $RUN_TESTS; then
  echo "==> Running test suite (make check)"
  make check
fi

echo "==> Creating app bundle (make install)"
make install

APP_BUNDLE="$ROOT_DIR/nextstep/Emacs.app"
if [[ ! -d "$APP_BUNDLE" ]]; then
  echo "${0##*/}: expected app bundle not found at $APP_BUNDLE" >&2
  exit 1
fi

echo "==> Launching Emacs from $APP_BUNDLE"
open "$APP_BUNDLE"
