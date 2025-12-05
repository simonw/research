#!/bin/bash
# Build jq as a WASM module using Emscripten
# Based on jqkungfu approach
set -ex

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
JQ_SRC_DIR="${SCRIPT_DIR}/jq-src"
BUILD_DIR="${SCRIPT_DIR}/build"

# Clone jq from git if not present
if [ ! -d "${JQ_SRC_DIR}" ]; then
    echo "Cloning jq source..."
    git clone --depth 1 --branch jq-1.6 https://github.com/stedolan/jq.git "${JQ_SRC_DIR}"
    cd "${JQ_SRC_DIR}"
    git submodule update --init --recursive
fi

# Create build directory
mkdir -p "${BUILD_DIR}"

cd "${JQ_SRC_DIR}"

# Generate ./configure file
echo "Generating configure..."
autoreconf -fi

# Run ./configure with emconfigure
echo "Configuring jq for Emscripten..."
emconfigure ./configure \
    --with-oniguruma=builtin \
    --disable-maintainer-mode

# Compile jq and generate .js/.wasm files
echo "Building jq..."
emmake make clean || true
emmake make -j$(nproc) \
    EXEEXT=.js \
    CFLAGS="-O2"

# Move outputs to build directory
cp jq.js jq.wasm "${BUILD_DIR}/"

echo "Build complete!"
ls -la "${BUILD_DIR}/"
file "${BUILD_DIR}/jq.wasm"
