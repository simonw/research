#!/bin/bash
# Build jq as a WASI WebAssembly module
set -ex

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WASI_SDK_PATH="${SCRIPT_DIR}/wasi-sdk"
JQ_VERSION="${JQ_VERSION:-1.6}"
JQ_SRC_DIR="${SCRIPT_DIR}/jq-${JQ_VERSION}"
BUILD_DIR="${SCRIPT_DIR}/build"

# Determine architecture
ARCH=$(uname -m)
if [ "$ARCH" = "arm64" ] || [ "$ARCH" = "aarch64" ]; then
    WASI_ARCH="arm64"
else
    WASI_ARCH="x86_64"
fi

# Ensure WASI SDK is available
if [ ! -d "${WASI_SDK_PATH}" ]; then
    echo "Downloading WASI SDK..."
    WASI_VERSION=24
    WASI_VERSION_FULL=${WASI_VERSION}.0
    wget -q https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-${WASI_VERSION}/wasi-sdk-${WASI_VERSION_FULL}-${WASI_ARCH}-linux.tar.gz
    tar xzf wasi-sdk-${WASI_VERSION_FULL}-${WASI_ARCH}-linux.tar.gz
    mv wasi-sdk-${WASI_VERSION_FULL}-${WASI_ARCH}-linux wasi-sdk
    rm -f wasi-sdk-${WASI_VERSION_FULL}-${WASI_ARCH}-linux.tar.gz
fi

# Ensure jq source is available
if [ ! -d "${JQ_SRC_DIR}" ]; then
    echo "Downloading jq source..."
    if [ "${JQ_VERSION}" = "1.6" ]; then
        wget -q https://github.com/stedolan/jq/releases/download/jq-${JQ_VERSION}/jq-${JQ_VERSION}.tar.gz
    else
        wget -q https://github.com/jqlang/jq/releases/download/jq-${JQ_VERSION}/jq-${JQ_VERSION}.tar.gz
    fi
    tar xzf jq-${JQ_VERSION}.tar.gz
    rm -f jq-${JQ_VERSION}.tar.gz
fi

# Create build directory
mkdir -p "${BUILD_DIR}"

# Update config.sub and config.guess to recognize wasm32
echo "Updating config.sub and config.guess..."
wget -q https://raw.githubusercontent.com/gcc-mirror/gcc/master/config.sub -O "${JQ_SRC_DIR}/config/config.sub" || \
curl -sL 'https://raw.githubusercontent.com/gcc-mirror/gcc/master/config.sub' -o "${JQ_SRC_DIR}/config/config.sub"
wget -q https://raw.githubusercontent.com/gcc-mirror/gcc/master/config.guess -O "${JQ_SRC_DIR}/config/config.guess" || \
curl -sL 'https://raw.githubusercontent.com/gcc-mirror/gcc/master/config.guess' -o "${JQ_SRC_DIR}/config/config.guess"
chmod +x "${JQ_SRC_DIR}/config/config.sub" "${JQ_SRC_DIR}/config/config.guess"

# Set up environment for WASI SDK
export CC="${WASI_SDK_PATH}/bin/clang"
export AR="${WASI_SDK_PATH}/bin/llvm-ar"
export RANLIB="${WASI_SDK_PATH}/bin/llvm-ranlib"
# WASI is single-threaded, so don't use pthread TLS
export CFLAGS="--sysroot=${WASI_SDK_PATH}/share/wasi-sysroot -O2 -D_WASI_EMULATED_SIGNAL -D_WASI_EMULATED_PROCESS_CLOCKS"
export LDFLAGS="-lwasi-emulated-signal -lwasi-emulated-process-clocks"

# Stub out pthread functions since WASI doesn't have them
# Create a minimal pthread stub to satisfy the linker
cat > "${BUILD_DIR}/pthread_stub.c" << 'PTHREAD_STUB'
/* Minimal pthread stubs for WASI (single-threaded) */
typedef int pthread_key_t;
typedef int pthread_once_t;

int pthread_key_create(pthread_key_t *key, void (*destructor)(void*)) {
    (void)key; (void)destructor;
    return -1; /* Indicate failure so jq falls back to global */
}
int pthread_once(pthread_once_t *once_control, void (*init_routine)(void)) {
    (void)once_control; (void)init_routine;
    return -1;
}
void* pthread_getspecific(pthread_key_t key) {
    (void)key;
    return 0;
}
int pthread_setspecific(pthread_key_t key, const void *value) {
    (void)key; (void)value;
    return -1;
}
PTHREAD_STUB

# Compile pthread stub
"${CC}" ${CFLAGS} -c "${BUILD_DIR}/pthread_stub.c" -o "${BUILD_DIR}/pthread_stub.o"

cd "${JQ_SRC_DIR}"

# Build oniguruma first
echo "Building oniguruma..."
cd modules/oniguruma
if [ -f configure.ac ]; then
    autoreconf -fi 2>/dev/null || true
fi
./configure \
    --host=wasm32-wasi \
    --prefix="${BUILD_DIR}/oniguruma" \
    --enable-static \
    --disable-shared \
    CFLAGS="${CFLAGS}" \
    CC="${CC}" \
    AR="${AR}" \
    RANLIB="${RANLIB}"
make clean || true
make -j$(nproc)
make install

cd "${JQ_SRC_DIR}"

# Build jq
echo "Building jq..."
./configure \
    --host=wasm32-wasi \
    --with-oniguruma="${BUILD_DIR}/oniguruma" \
    --enable-static \
    --disable-shared \
    --disable-maintainer-mode \
    CFLAGS="${CFLAGS} -I${BUILD_DIR}/oniguruma/include" \
    LDFLAGS="${LDFLAGS} -L${BUILD_DIR}/oniguruma/lib" \
    CC="${CC}" \
    AR="${AR}" \
    RANLIB="${RANLIB}"

make clean || true
make -j$(nproc) LDFLAGS="${LDFLAGS} -L${BUILD_DIR}/oniguruma/lib ${BUILD_DIR}/pthread_stub.o -all-static"

# Copy the output
cp jq "${BUILD_DIR}/jq.wasm"
echo "Build complete! Output: ${BUILD_DIR}/jq.wasm"
ls -la "${BUILD_DIR}/jq.wasm"
