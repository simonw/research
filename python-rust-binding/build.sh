#!/usr/bin/env bash
#
# Build script for the eryx Python bindings.
#
# This script:
#   1. Clones the eryx repo to /tmp/eryx if it doesn't already exist
#   2. Installs prerequisites (WASI SDK, Rust nightly, patchelf)
#   3. Builds the WASM runtime (runtime.wasm)
#   4. Pre-compiles the runtime to native code (runtime.cwasm)
#   5. Builds the Python wheel using maturin
#   6. Prints the path to the resulting wheel
#
# Usage:
#   ./build.sh
#
# Prerequisites:
#   - Rust toolchain (rustc, cargo) — https://rustup.rs/
#   - maturin — install via: uv tool install maturin
#   - uv — https://docs.astral.sh/uv/
#   - Python 3.12+
#   - Internet access (for downloading WASI SDK on first run)
#
# The wheel will be written to /tmp/eryx/target/wheels/

set -euo pipefail

REPO_URL="https://github.com/eryx-org/eryx"
REPO_DIR="/tmp/eryx"
CRATE_DIR="${REPO_DIR}/crates/eryx-python"
WASI_SDK_VERSION="27.0"
WASI_SDK_DIR="/tmp/wasi-sdk-${WASI_SDK_VERSION}-x86_64-linux"

# ---- Step 1: Clone the repo if it doesn't exist ----
if [ ! -d "${REPO_DIR}" ]; then
    echo "==> Cloning eryx repository to ${REPO_DIR}..."
    git clone "${REPO_URL}" "${REPO_DIR}"
else
    echo "==> eryx repository already exists at ${REPO_DIR}"
fi

# ---- Step 2: Verify and install prerequisites ----
if ! command -v cargo &> /dev/null; then
    echo "ERROR: cargo (Rust) is not installed."
    echo "Install it from: https://rustup.rs/"
    exit 1
fi

if ! command -v maturin &> /dev/null; then
    echo "==> Installing maturin..."
    if command -v uv &> /dev/null; then
        uv tool install maturin
    else
        pip install maturin
    fi
fi

# Install patchelf (needed for manylinux wheel creation)
if ! command -v patchelf &> /dev/null; then
    echo "==> Installing patchelf..."
    pip install patchelf
fi

# Install Rust nightly with wasm32-wasip1 target (needed for WASM compilation)
if ! rustup toolchain list 2>/dev/null | grep -q nightly; then
    echo "==> Installing Rust nightly toolchain..."
    rustup toolchain install nightly --component rust-src
fi
if ! rustup target list --toolchain nightly 2>/dev/null | grep -q "wasm32-wasip1 (installed)"; then
    echo "==> Adding wasm32-wasip1 target to nightly..."
    rustup target add wasm32-wasip1 --toolchain nightly
fi

# Install WASI SDK (needed to build the WASM runtime)
if [ ! -d "${WASI_SDK_DIR}" ]; then
    echo "==> Downloading WASI SDK ${WASI_SDK_VERSION}..."
    WASI_SDK_TAR="/tmp/wasi-sdk-${WASI_SDK_VERSION}-x86_64-linux.tar.gz"
    if [ ! -f "${WASI_SDK_TAR}" ]; then
        curl -L -o "${WASI_SDK_TAR}" \
            "https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-27/wasi-sdk-${WASI_SDK_VERSION}-x86_64-linux.tar.gz"
    fi
    echo "==> Extracting WASI SDK..."
    tar xzf "${WASI_SDK_TAR}" -C /tmp
fi

export WASI_SDK_PATH="${WASI_SDK_DIR}"

# ---- Step 3: Build the WASM runtime (if not already built) ----
RUNTIME_WASM="${REPO_DIR}/crates/eryx-runtime/runtime.wasm"
if [ ! -f "${RUNTIME_WASM}" ]; then
    echo "==> Building WASM runtime (runtime.wasm)..."
    echo "    (this compiles the Rust WASM runtime with WASI SDK)"
    cd "${REPO_DIR}"
    BUILD_ERYX_RUNTIME=1 cargo build --package eryx-runtime --release
    cd -
else
    echo "==> WASM runtime already exists at ${RUNTIME_WASM}"
fi

# ---- Step 4: Pre-compile the runtime to native code (if not already done) ----
RUNTIME_CWASM="${REPO_DIR}/crates/eryx-runtime/runtime.cwasm"
if [ ! -f "${RUNTIME_CWASM}" ]; then
    echo "==> Pre-compiling WASM to native code (runtime.cwasm)..."
    echo "    (this uses wasmtime to compile WASM to native machine code)"
    cd "${REPO_DIR}"
    cargo run -p eryx --example precompile --release
    cd -
else
    echo "==> Pre-compiled runtime already exists at ${RUNTIME_CWASM}"
fi

# ---- Step 5: Build the Python wheel ----
echo "==> Building eryx Python wheel with maturin..."
echo "    (this compiles the PyO3 bindings with the embedded WASM runtime)"

maturin build \
    --release \
    --manifest-path "${CRATE_DIR}/Cargo.toml" \
    --out "${REPO_DIR}/target/wheels"

# ---- Step 6: Report the result ----
WHEEL=$(ls -t "${REPO_DIR}/target/wheels"/pyeryx-*.whl 2>/dev/null | head -1)

if [ -z "${WHEEL}" ]; then
    echo "ERROR: No wheel found after build."
    exit 1
fi

echo ""
echo "============================================"
echo "  Build successful!"
echo "============================================"
echo ""
echo "  Wheel: ${WHEEL}"
echo ""
echo "  Test with:"
echo "    uv run --python 3.12 --with '${WHEEL}' python -c 'import eryx; print(eryx.__version__)'"
echo ""
echo "  Run pytest:"
echo "    uv run --python 3.12 --with '${WHEEL}' --with pytest pytest tests/"
echo ""
