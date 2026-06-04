#!/bin/bash
# Build Luau as WebAssembly using Emscripten
# Produces luau.js + luau.wasm (works for browser, Node.js, and wasmtime)
#
# Prerequisites:
#   - Emscripten SDK installed and activated (source emsdk_env.sh)
#   - Official Luau repo cloned to /tmp/luau

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
LUAU_DIR="/tmp/luau"
BUILD_DIR="/tmp/luau-wasm-build"

# Source emscripten environment
source /tmp/emsdk/emsdk_env.sh 2>/dev/null

# Copy our modified Web.cpp into the Luau tree
cp "$SCRIPT_DIR/LuauWeb.cpp" "$LUAU_DIR/CLI/src/Web.cpp"

# Apply CMakeLists.txt modifications:
# 1. Remove SINGLE_FILE (we want separate .wasm)
# 2. Remove Analysis dependency (we stripped checkScript)
# 3. Update exported functions
# 4. Add MODULARIZE for clean JS loading
# 5. Use -O1 to keep readable import names (needed for wasmtime)
cd "$LUAU_DIR"
git checkout -- CMakeLists.txt  # Start fresh
sed -i 's/-sSINGLE_FILE=1/-sMODULARIZE=1 -sEXPORT_NAME=createLuau -O1/' CMakeLists.txt
sed -i 's/target_link_libraries(Luau.Web PRIVATE Luau.Compiler Luau.VM Luau.Analysis)/target_link_libraries(Luau.Web PRIVATE Luau.Compiler Luau.VM)/' CMakeLists.txt
sed -i 's/-sEXPORTED_FUNCTIONS=\["_executeScript","_checkScript"\]/-sEXPORTED_FUNCTIONS=["_executeScript"]/' CMakeLists.txt

# Clean and build
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"
cd "$BUILD_DIR"

emcmake cmake "$LUAU_DIR" \
    -DLUAU_BUILD_CLI=OFF \
    -DLUAU_BUILD_TESTS=OFF \
    -DLUAU_BUILD_WEB=ON \
    -DCMAKE_BUILD_TYPE=Release \
    2>&1

emmake cmake --build . --target Luau.Web -j$(nproc) 2>&1

# Copy outputs
cp "$BUILD_DIR/Luau.Web.js" "$SCRIPT_DIR/luau.js"
cp "$BUILD_DIR/Luau.Web.wasm" "$SCRIPT_DIR/luau.wasm"
# Fix the wasm filename reference in JS glue
sed -i 's/Luau.Web.wasm/luau.wasm/g' "$SCRIPT_DIR/luau.js"

echo ""
echo "Build complete!"
ls -lh "$SCRIPT_DIR/luau.js" "$SCRIPT_DIR/luau.wasm"
