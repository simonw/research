#!/usr/bin/env bash
# Build quickjs-ng as a wasm32 reactor for the wasmi sandbox.
#
#   QUICKJS_NG=/path/to/quickjs-ng WASI_SDK=/path/to/wasi-sdk ./build.sh
#
# Step 1 builds libqjs.a with quickjs-ng's own CMake setup and the wasi-sdk
# toolchain file. Step 2 links our small reactor (qjs_sandbox.c) against it.
set -euo pipefail
cd "$(dirname "$0")"

QUICKJS_NG=${QUICKJS_NG:-../../../../quickjs-ng}
WASI_SDK=${WASI_SDK:-/opt/wasi-sdk}
BUILD=${BUILD:-$QUICKJS_NG/build-wasi}

if [ ! -f "$BUILD/libqjs.a" ]; then
  cmake -S "$QUICKJS_NG" -B "$BUILD" \
    -DCMAKE_TOOLCHAIN_FILE="$WASI_SDK/share/cmake/wasi-sdk-p1.cmake" \
    -DCMAKE_BUILD_TYPE=Release -DQJS_BUILD_EXAMPLES=OFF -DQJS_BUILD_LIBC=OFF
  cmake --build "$BUILD" --target qjs -j"$(nproc)"
fi

"$WASI_SDK/bin/clang" --target=wasm32-wasip1 -O2 -DNDEBUG -std=gnu11 \
  -D_GNU_SOURCE -D_WASI_EMULATED_PROCESS_CLOCKS -D_WASI_EMULATED_SIGNAL \
  -I"$QUICKJS_NG" \
  -mexec-model=reactor \
  -Wl,--export=malloc -Wl,--export=free \
  -Wl,-z,stack-size=1048576 -Wl,--stack-first \
  -o quickjs.wasm qjs_sandbox.c "$BUILD/libqjs.a" \
  -lwasi-emulated-process-clocks -lwasi-emulated-signal

mkdir -p ../../wasmi_sandbox/python/wasmi_sandbox/guests && cp quickjs.wasm ../../wasmi_sandbox/python/wasmi_sandbox/guests/
ls -la quickjs.wasm
