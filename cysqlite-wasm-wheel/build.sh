#!/bin/bash
#
# Build cysqlite as a Pyodide-compatible WebAssembly wheel.
#
# This script:
#   1. Checks for required dependencies (git, python3, pyodide-build, emcc)
#   2. Clones cysqlite from GitHub to /tmp if not already present
#   3. Fetches the SQLite amalgamation
#   4. Builds a .whl for Pyodide (emscripten/wasm32)
#   5. Copies the wheel to the current directory
#
set -euo pipefail

REPO_URL="https://github.com/coleifer/cysqlite"
CLONE_DIR="/tmp/cysqlite"
WHEEL_DIR="/tmp/cysqlite-wheels"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# ── dependency checks ─────────────────────────────────────────────

check_cmd() {
    if ! command -v "$1" &>/dev/null; then
        echo "ERROR: '$1' is not installed."
        echo ""
        echo "$2"
        exit 1
    fi
}

check_cmd git "Install git: https://git-scm.com/downloads"

check_cmd python3 "Install Python 3: https://www.python.org/downloads/"

check_cmd pyodide \
    "Install pyodide-build:
    pip install pyodide-build
  https://pyodide.org/en/stable/development/building-and-testing-packages.html"

check_cmd emcc \
    "Install the Emscripten SDK (emsdk):
    git clone https://github.com/emscripten-core/emsdk.git
    cd emsdk && ./emsdk install 3.1.46 && ./emsdk activate 3.1.46
    source emsdk_env.sh
  The version MUST match pyodide-build's expected version.
  Check with:  pyodide config list | grep emscripten_version
  https://emscripten.org/docs/getting_started/downloads.html"

# Verify emscripten version matches pyodide-build expectations
EXPECTED_EM=$(pyodide config list 2>/dev/null | grep '^emscripten_version=' | cut -d= -f2)
ACTUAL_EM=$(emcc --version 2>/dev/null | head -1 | grep -oP '\d+\.\d+\.\d+' || echo "unknown")
if [ -n "$EXPECTED_EM" ] && [ "$ACTUAL_EM" != "$EXPECTED_EM" ]; then
    echo "WARNING: emscripten version mismatch."
    echo "  pyodide-build expects: $EXPECTED_EM"
    echo "  emcc reports:          $ACTUAL_EM"
    echo "  The build may fail. Install the matching version:"
    echo "    cd emsdk && ./emsdk install $EXPECTED_EM && ./emsdk activate $EXPECTED_EM"
    echo ""
fi

# ── clone / update cysqlite ──────────────────────────────────────

if [ -d "$CLONE_DIR/.git" ]; then
    echo "Using existing cysqlite checkout at $CLONE_DIR"
    cd "$CLONE_DIR"
    git pull --ff-only 2>/dev/null || true
else
    echo "Cloning cysqlite from $REPO_URL ..."
    git clone "$REPO_URL" "$CLONE_DIR"
    cd "$CLONE_DIR"
fi

# ── fetch SQLite amalgamation ────────────────────────────────────

if [ ! -f "$CLONE_DIR/sqlite3.c" ] || [ ! -f "$CLONE_DIR/sqlite3.h" ]; then
    echo "Fetching SQLite amalgamation..."
    python3 scripts/fetch_sqlite
else
    echo "SQLite amalgamation already present."
fi

# ── build the wheel ──────────────────────────────────────────────

echo ""
echo "Building cysqlite wheel for Pyodide (emscripten/wasm32)..."
rm -rf "$WHEEL_DIR"
pyodide build --outdir "$WHEEL_DIR"

# ── copy result ──────────────────────────────────────────────────

WHEEL=$(ls "$WHEEL_DIR"/*.whl 2>/dev/null | head -1)
if [ -z "$WHEEL" ]; then
    echo "ERROR: No wheel produced in $WHEEL_DIR"
    exit 1
fi

cp "$WHEEL" "$SCRIPT_DIR/"
BASENAME=$(basename "$WHEEL")

echo ""
echo "Success! Wheel built:"
echo "  $SCRIPT_DIR/$BASENAME"
echo ""
echo "To test, serve this directory and open demo.html:"
echo "  cd $SCRIPT_DIR && python3 -m http.server 8765"
echo "  open http://localhost:8765/demo.html"
