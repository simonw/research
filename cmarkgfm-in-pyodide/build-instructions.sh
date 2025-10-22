#!/bin/bash
#
# Script to build cmarkgfm for Pyodide
# This script documents the build process but requires Emscripten to be installed
#
# Requirements:
# - Linux environment
# - Python 3.11+
# - pyodide-build installed (pip install pyodide-build)
# - Emscripten SDK 3.1.46
#

set -e  # Exit on error

echo "========================================="
echo "cmarkgfm Pyodide Build Script"
echo "========================================="
echo ""

# Check if pyodide-build is installed
if ! command -v pyodide &> /dev/null; then
    echo "❌ Error: pyodide-build not installed"
    echo "Install with: pip install pyodide-build"
    exit 1
fi

echo "✓ pyodide-build found: $(pyodide --version)"

# Check if Emscripten is installed
if ! command -v emcc &> /dev/null; then
    echo "❌ Error: Emscripten not installed"
    echo ""
    echo "Install Emscripten SDK 3.1.46:"
    echo "  git clone https://github.com/emscripten-core/emsdk.git"
    echo "  cd emsdk"
    echo "  ./emsdk install 3.1.46"
    echo "  ./emsdk activate 3.1.46"
    echo "  source ./emsdk_env.sh"
    echo ""
    exit 1
fi

EMCC_VERSION=$(emcc --version | head -1)
echo "✓ Emscripten found: $EMCC_VERSION"

# Check Emscripten version
EXPECTED_VERSION="3.1.46"
if ! emcc --version | grep -q "$EXPECTED_VERSION"; then
    echo "⚠️  Warning: Emscripten version might not match requirement"
    echo "   Expected: $EXPECTED_VERSION"
    echo "   Found: $EMCC_VERSION"
    echo ""
    read -p "Continue anyway? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# Create build directory
BUILD_DIR="$(dirname "$0")/build"
mkdir -p "$BUILD_DIR"
echo "✓ Build directory: $BUILD_DIR"
echo ""

# Build cmarkgfm
echo "Building cmarkgfm..."
echo "This may take several minutes..."
echo ""

cd "$BUILD_DIR"
pyodide build cmarkgfm -o .

if [ $? -eq 0 ]; then
    echo ""
    echo "========================================="
    echo "✓ Build succeeded!"
    echo "========================================="
    echo ""
    echo "Wheel file created:"
    ls -lh *.whl
    echo ""
    echo "To use in Node.js:"
    echo "  const pyodide = await loadPyodide();"
    echo "  await pyodide.loadPackage('file://$(pwd)/$(ls *.whl)');"
    echo ""
else
    echo ""
    echo "========================================="
    echo "❌ Build failed"
    echo "========================================="
    echo ""
    echo "Common issues:"
    echo "  - CMake configuration errors"
    echo "  - Missing C dependencies"
    echo "  - WebAssembly incompatibilities"
    echo ""
    echo "Check the error messages above for details."
    echo ""
    exit 1
fi
