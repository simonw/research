#!/usr/bin/env bash
# Build pydantic-monty as a WebAssembly wheel for Pyodide
#
# Prerequisites:
#   - Rust (via rustup)
#   - Python 3.11+ (for maturin)
#   - pip install maturin
#
# This script will:
#   1. Install Emscripten SDK 4.0.9 (matching Pyodide 0.28/0.29)
#   2. Install Rust nightly-2025-06-27 with wasm32-unknown-emscripten target
#   3. Download the pre-built wasm-eh sysroot from pyodide/rust-emscripten-wasm-eh-sysroot
#   4. Build the wheel using maturin

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MONTY_REPO="${MONTY_REPO:-/tmp/monty}"
EMSDK_DIR="${EMSDK_DIR:-/tmp/emsdk}"
EMSDK_VERSION="4.0.9"
RUST_NIGHTLY="nightly-2025-06-27"
SYSROOT_RELEASE="emcc-4.0.9_nightly-2025-06-27"
PYTHON_TARGET="${PYTHON_TARGET:-3.13}"
OUTPUT_DIR="${OUTPUT_DIR:-$SCRIPT_DIR}"

echo "=== Building pydantic-monty WASM wheel for Pyodide ==="
echo "Monty repo: $MONTY_REPO"
echo "Emsdk dir: $EMSDK_DIR"
echo "Target Python: $PYTHON_TARGET"
echo "Output: $OUTPUT_DIR"
echo ""

# Step 1: Clone monty if needed
if [ ! -d "$MONTY_REPO" ]; then
    echo "--- Cloning monty repo ---"
    git clone https://github.com/pydantic/monty "$MONTY_REPO"
fi

# Step 2: Install Emscripten SDK
if [ ! -d "$EMSDK_DIR" ]; then
    echo "--- Installing Emscripten SDK $EMSDK_VERSION ---"
    git clone https://github.com/emscripten-core/emsdk.git "$EMSDK_DIR"
fi
cd "$EMSDK_DIR"
./emsdk install "$EMSDK_VERSION"
./emsdk activate "$EMSDK_VERSION"
source "$EMSDK_DIR/emsdk_env.sh"

echo "Emscripten version: $(emcc --version | head -1)"

# Step 3: Install Rust nightly toolchain
echo "--- Installing Rust $RUST_NIGHTLY ---"
rustup toolchain install "$RUST_NIGHTLY" --component rust-src
rustup run "$RUST_NIGHTLY" rustup target add wasm32-unknown-emscripten

# Step 4: Download and install pre-built wasm-eh sysroot
TOOLCHAIN_ROOT=$(rustup run "$RUST_NIGHTLY" rustc --print sysroot)
SYSROOT_LIB="$TOOLCHAIN_ROOT/lib/rustlib/wasm32-unknown-emscripten/lib"

# Check if sysroot needs to be installed (look for more than the default number of libs)
SYSROOT_URL="https://github.com/pyodide/rust-emscripten-wasm-eh-sysroot/releases/download/${SYSROOT_RELEASE}/${SYSROOT_RELEASE}.tar.bz2"

echo "--- Installing wasm-eh sysroot ---"
# Remove the standard sysroot to avoid duplicate rlib conflicts
rm -rf "$TOOLCHAIN_ROOT/lib/rustlib/wasm32-unknown-emscripten"
# Download and extract the wasm-eh sysroot
SYSROOT_ARCHIVE="/tmp/${SYSROOT_RELEASE}.tar.bz2"
if [ ! -f "$SYSROOT_ARCHIVE" ]; then
    wget -q "$SYSROOT_URL" -O "$SYSROOT_ARCHIVE"
fi
tar -xf "$SYSROOT_ARCHIVE" --directory="$TOOLCHAIN_ROOT/lib/rustlib"

echo "Sysroot installed: $(find "$SYSROOT_LIB" -name '*.rlib' | wc -l) rlibs"

# Step 5: Install maturin if needed
if ! command -v maturin &>/dev/null; then
    echo "--- Installing maturin ---"
    pip install maturin
fi

# Step 6: Build the wheel
echo "--- Building WASM wheel ---"
export RUSTUP_TOOLCHAIN="$RUST_NIGHTLY"
export RUSTFLAGS="-Z emscripten-wasm-eh \
  -C link-arg=-sSIDE_MODULE=2 \
  -C link-arg=-sWASM_BIGINT \
  -C relocation-model=pic \
  -Z link-native-libraries=no \
  -C symbol-mangling-version=v0"

cd "$MONTY_REPO"
maturin build \
    --release \
    --target wasm32-unknown-emscripten \
    -m crates/monty-python/Cargo.toml \
    --out "$OUTPUT_DIR" \
    -i "$PYTHON_TARGET"

echo ""
echo "=== Build complete ==="
ls -lh "$OUTPUT_DIR"/*.whl 2>/dev/null || echo "No wheel files found in $OUTPUT_DIR"
