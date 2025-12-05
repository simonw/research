#!/bin/bash
# Build jaq (Rust jq clone) as a WASI WebAssembly module
set -ex

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_DIR="${SCRIPT_DIR}/build"

# Ensure Rust is installed with WASI target
if ! command -v rustup &> /dev/null; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

# Add WASI target
rustup target add wasm32-wasip1 || rustup target add wasm32-wasi

# Create build directory
mkdir -p "${BUILD_DIR}"

# Clone jaq if not present
if [ ! -d "${SCRIPT_DIR}/jaq-src" ]; then
    echo "Cloning jaq..."
    git clone --depth 1 https://github.com/01mf02/jaq.git "${SCRIPT_DIR}/jaq-src"
fi

cd "${SCRIPT_DIR}/jaq-src"

# Build jaq for WASI
echo "Building jaq for WASI..."
cargo build --release --target wasm32-wasip1 -p jaq 2>/dev/null || \
cargo build --release --target wasm32-wasi -p jaq

# Find and copy the WASM binary
WASM_FILE=$(find target -name "jaq.wasm" | head -1)
if [ -n "$WASM_FILE" ]; then
    cp "$WASM_FILE" "${BUILD_DIR}/jaq.wasm"
    echo "Build complete! Output: ${BUILD_DIR}/jaq.wasm"
    ls -la "${BUILD_DIR}/jaq.wasm"
else
    echo "Build failed - no jaq.wasm found"
    find target -name "*.wasm" -ls
    exit 1
fi
