#!/bin/bash
#
# Build syntaqlite Python extension as a Pyodide-compatible WASM wheel.
#
# Prerequisites:
#   - Rust toolchain with wasm32-unknown-emscripten target
#   - Emscripten SDK 3.1.46 (matches pyodide-build 0.25.1)
#   - pyodide-build (pip install pyodide-build)
#
# The script:
#   1. Cross-compiles syntaqlite Rust libraries for wasm32-unknown-emscripten
#   2. Uses pyodide build to compile the C extension and package the wheel
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SYNTAQLITE_SRC="${SYNTAQLITE_SRC:-/tmp/syntaqlite}"
EMSDK_DIR="${EMSDK:-/tmp/emsdk}"
WHEEL_DIR="${SCRIPT_DIR}/dist"

echo "=== syntaqlite WASM wheel builder ==="
echo ""

# ── Check dependencies ────────────────────────────────────────────

check_cmd() {
    if ! command -v "$1" &>/dev/null; then
        echo "ERROR: '$1' not found."
        echo "$2"
        exit 1
    fi
}

check_cmd cargo "Install Rust: https://rustup.rs"
check_cmd pyodide "Install: pip install pyodide-build"

# Activate emscripten
if [ -f "$EMSDK_DIR/emsdk_env.sh" ]; then
    source "$EMSDK_DIR/emsdk_env.sh" 2>/dev/null
else
    echo "ERROR: emsdk not found at $EMSDK_DIR"
    echo "Install: git clone https://github.com/emscripten-core/emsdk.git $EMSDK_DIR"
    echo "         cd $EMSDK_DIR && ./emsdk install 3.1.46 && ./emsdk activate 3.1.46"
    exit 1
fi

check_cmd emcc "Emscripten not found after sourcing emsdk_env.sh"

echo "Emscripten: $(emcc --version 2>&1 | head -1)"
echo "Rust: $(rustc --version)"
echo "pyodide-build: $(pyodide --version)"
echo ""

# ── Verify emscripten version ────────────────────────────────────

EXPECTED_EM=$(pyodide config list 2>/dev/null | grep '^emscripten_version=' | cut -d= -f2)
ACTUAL_EM=$(emcc --version 2>/dev/null | head -1 | grep -oP '\d+\.\d+\.\d+' || echo "unknown")
if [ -n "$EXPECTED_EM" ] && [ "$ACTUAL_EM" != "$EXPECTED_EM" ]; then
    echo "WARNING: emscripten version mismatch"
    echo "  pyodide-build expects: $EXPECTED_EM"
    echo "  emcc reports:          $ACTUAL_EM"
    echo ""
fi

# ── Ensure wasm32-unknown-emscripten target ──────────────────────

if ! rustup target list --installed 2>/dev/null | grep -q wasm32-unknown-emscripten; then
    echo "Adding wasm32-unknown-emscripten target..."
    rustup target add wasm32-unknown-emscripten
fi

# ── Cross-compile Rust libraries ─────────────────────────────────

echo "Cross-compiling syntaqlite for wasm32-unknown-emscripten..."
cd "$SYNTAQLITE_SRC"
cargo build --release --target wasm32-unknown-emscripten -p syntaqlite 2>&1

# Find the build output directories
WASM_RELEASE="$SYNTAQLITE_SRC/target/wasm32-unknown-emscripten/release"
WASM_SYNTAX_OUT=""
for entry in "$WASM_RELEASE/build"/syntaqlite-syntax-*/out; do
    if [ -f "$entry/libsyntaqlite_engine.a" ]; then
        WASM_SYNTAX_OUT="$entry"
        break
    fi
done

if [ -z "$WASM_SYNTAX_OUT" ]; then
    echo "ERROR: Could not find cross-compiled syntax libraries"
    exit 1
fi

echo "  libsyntaqlite.a:        $WASM_RELEASE/libsyntaqlite.a"
echo "  libsyntaqlite_engine.a: $WASM_SYNTAX_OUT/libsyntaqlite_engine.a"
echo "  libsyntaqlite_sqlite.a: $WASM_SYNTAX_OUT/libsyntaqlite_sqlite.a"
echo ""

# ── Build the wheel with pyodide ─────────────────────────────────

echo "Building WASM wheel with pyodide build..."
cd "$SCRIPT_DIR"

# Export paths so setup.py can find the WASM libraries
export SYNTAQLITE_SRC
# Override the target directory for WASM builds
export SYNTAQLITE_WASM_RELEASE="$WASM_RELEASE"
export SYNTAQLITE_WASM_SYNTAX_OUT="$WASM_SYNTAX_OUT"

rm -rf "$WHEEL_DIR"
mkdir -p "$WHEEL_DIR"

pyodide build --outdir "$WHEEL_DIR" 2>&1

# ── Report results ───────────────────────────────────────────────

WHEEL=$(ls "$WHEEL_DIR"/*.whl 2>/dev/null | head -1)
if [ -z "$WHEEL" ]; then
    echo "ERROR: No wheel produced"
    exit 1
fi

echo ""
echo "=== Success! ==="
echo "Wheel: $WHEEL"
echo "Size:  $(ls -lh "$WHEEL" | awk '{print $5}')"
echo ""
echo "To use in Pyodide:"
echo "  import micropip"
echo "  await micropip.install('file:///path/to/$(basename "$WHEEL")')"
echo "  import syntaqlite"
