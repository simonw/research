#!/usr/bin/env bash
# Build curl-impersonate (Chrome variant) as a WASM module via Emscripten.
# Produces: dist/curl-impersonate.js + dist/curl-impersonate.wasm
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BUILD_DIR="$SCRIPT_DIR/build"
DIST_DIR="$SCRIPT_DIR/dist"
SRC_DIR="$SCRIPT_DIR/curl-impersonate-src"
EMSDK_DIR="$SCRIPT_DIR/emsdk"
PATCHES_DIR="$SRC_DIR/chrome/patches"

# Versions from curl-impersonate's Makefile.in
BORING_SSL_COMMIT="1b7fdbd9101dedc3e0aa3fcf4ff74eacddb34ecc"
NGHTTP2_VERSION="1.56.0"
CURL_VERSION="8.1.1"
BROTLI_VERSION="1.0.9"

# Activate Emscripten
export EMSDK_QUIET=1
source "$EMSDK_DIR/emsdk_env.sh"

mkdir -p "$BUILD_DIR" "$DIST_DIR"

# ============================================================
# Step 1: Build Brotli for WASM
# ============================================================
build_brotli() {
  echo "=== Building Brotli $BROTLI_VERSION ==="
  local brotli_dir="$BUILD_DIR/brotli-$BROTLI_VERSION"
  local brotli_install="$BUILD_DIR/brotli-installed"

  if [ -f "$brotli_install/lib/libbrotlidec-static.a" ]; then
    echo "Brotli already built, skipping."
    return
  fi

  cd "$BUILD_DIR"
  if [ ! -d "brotli-$BROTLI_VERSION" ]; then
    curl -L "https://github.com/google/brotli/archive/refs/tags/v${BROTLI_VERSION}.tar.gz" \
      -o "brotli-${BROTLI_VERSION}.tar.gz"
    tar xf "brotli-${BROTLI_VERSION}.tar.gz"
  fi

  mkdir -p "$brotli_dir/out"
  cd "$brotli_dir/out"

  emcmake cmake \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="$brotli_install" \
    -DCMAKE_INSTALL_LIBDIR=lib \
    ..

  cmake --build . -j$(nproc 2>/dev/null || sysctl -n hw.ncpu)

  # Brotli's CMakeLists skips install() when BROTLI_EMSCRIPTEN is detected,
  # so we manually copy the static libraries and headers.
  mkdir -p "$brotli_install/lib" "$brotli_install/include"
  cp -f libbrotlidec-static.a "$brotli_install/lib/"
  cp -f libbrotlienc-static.a "$brotli_install/lib/"
  cp -f libbrotlicommon-static.a "$brotli_install/lib/"
  cp -Rf "$brotli_dir/c/include/brotli" "$brotli_install/include/"
  echo "Brotli built successfully."
}

# ============================================================
# Step 2: Build BoringSSL for WASM
# ============================================================
build_boringssl() {
  echo "=== Building BoringSSL (patched) ==="
  local bssl_dir="$BUILD_DIR/boringssl"
  local bssl_build="$bssl_dir/build"

  if [ -f "$bssl_build/lib/libssl.a" ]; then
    echo "BoringSSL already built, skipping."
    return
  fi

  cd "$BUILD_DIR"
  if [ ! -d "boringssl" ]; then
    curl -L "https://github.com/google/boringssl/archive/${BORING_SSL_COMMIT}.zip" \
      -o boringssl.zip
    unzip -q -o boringssl.zip
    mv "boringssl-${BORING_SSL_COMMIT}" boringssl

    # Apply curl-impersonate Chrome cipher patches
    cd boringssl
    for p in "$PATCHES_DIR"/boringssl-*.patch; do
      echo "Applying patch: $(basename "$p")"
      patch -p1 < "$p"
    done
  fi

  mkdir -p "$bssl_build"
  cd "$bssl_build"

  # Key flags:
  # -DOPENSSL_NO_ASM=1: Required for WASM (no x86/ARM assembly)
  # -DCMAKE_SYSTEM_NAME=Generic: Cross-compiling for a non-OS target
  emcmake cmake \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_POSITION_INDEPENDENT_CODE=on \
    -DOPENSSL_NO_ASM=1 \
    -DCMAKE_C_FLAGS="-Wno-unknown-warning-option -Wno-stringop-overflow -Wno-array-bounds -pthread" \
    -DCMAKE_CXX_FLAGS="-Wno-unknown-warning-option -pthread" \
    ..

  emmake make -j$(nproc 2>/dev/null || sysctl -n hw.ncpu) ssl crypto

  # Set up directory structure for curl's --with-openssl
  mkdir -p lib
  ln -sf ../crypto/libcrypto.a lib/libcrypto.a
  ln -sf ../ssl/libssl.a lib/libssl.a
  cp -Rf ../include .

  echo "BoringSSL built successfully."
}

# ============================================================
# Step 3: Build nghttp2 for WASM
# ============================================================
build_nghttp2() {
  echo "=== Building nghttp2 $NGHTTP2_VERSION ==="
  local nghttp2_dir="$BUILD_DIR/nghttp2-$NGHTTP2_VERSION"
  local nghttp2_install="$BUILD_DIR/nghttp2-installed"

  if [ -f "$nghttp2_install/lib/libnghttp2.a" ]; then
    echo "nghttp2 already built, skipping."
    return
  fi

  cd "$BUILD_DIR"
  if [ ! -d "nghttp2-$NGHTTP2_VERSION" ]; then
    curl -L "https://github.com/nghttp2/nghttp2/releases/download/v${NGHTTP2_VERSION}/nghttp2-${NGHTTP2_VERSION}.tar.bz2" \
      -o "nghttp2-${NGHTTP2_VERSION}.tar.bz2"
    tar xf "nghttp2-${NGHTTP2_VERSION}.tar.bz2"
  fi

  cd "$nghttp2_dir"

  emconfigure ./configure \
    --prefix="$nghttp2_install" \
    --host=wasm32-unknown-emscripten \
    --with-pic \
    --enable-lib-only \
    --disable-shared \
    --enable-static \
    --disable-python-bindings

  emmake make -j$(nproc 2>/dev/null || sysctl -n hw.ncpu)
  emmake make install

  echo "nghttp2 built successfully."
}

# ============================================================
# Step 4: Build libcurl for WASM
# ============================================================
build_curl() {
  echo "=== Building curl $CURL_VERSION ==="
  local curl_dir="$BUILD_DIR/curl-$CURL_VERSION"
  local curl_install="$BUILD_DIR/curl-installed"
  local bssl_build="$BUILD_DIR/boringssl/build"
  local nghttp2_install="$BUILD_DIR/nghttp2-installed"
  local brotli_install="$BUILD_DIR/brotli-installed"

  if [ -f "$curl_install/lib/libcurl-impersonate-chrome.a" ]; then
    echo "curl already built, skipping."
    return
  fi

  cd "$BUILD_DIR"
  if [ ! -d "curl-$CURL_VERSION" ]; then
    curl -L "https://curl.se/download/curl-${CURL_VERSION}.tar.xz" \
      -o "curl-${CURL_VERSION}.tar.xz"
    tar xf "curl-${CURL_VERSION}.tar.xz"

    # Apply curl-impersonate patches
    cd "curl-$CURL_VERSION"
    for p in "$PATCHES_DIR"/curl-*.patch; do
      echo "Applying patch: $(basename "$p")"
      patch -p1 < "$p"
    done
    # Re-generate configure script after patching
    autoreconf -fi
  fi

  cd "$curl_dir"

  # Configure curl with BoringSSL + nghttp2 + brotli for WASM
  # Disable everything we don't need to minimize WASM size
  emconfigure ./configure \
    --prefix="$curl_install" \
    --host=wasm32-unknown-emscripten \
    --with-openssl="$bssl_build" \
    --with-nghttp2="$nghttp2_install" \
    --with-brotli="$brotli_install" \
    --enable-static \
    --disable-shared \
    --disable-ftp \
    --disable-ldap \
    --disable-ldaps \
    --disable-rtsp \
    --disable-dict \
    --disable-telnet \
    --disable-tftp \
    --disable-pop3 \
    --disable-imap \
    --disable-smb \
    --disable-smtp \
    --disable-gopher \
    --disable-mqtt \
    --disable-manual \
    --disable-docs \
    --disable-ntlm \
    --disable-unix-sockets \
    --disable-threaded-resolver \
    --without-librtmp \
    --without-libidn2 \
    --without-libpsl \
    --without-libssh2 \
    --enable-websockets \
    CFLAGS="-pthread" \
    LIBS="-pthread"

  emmake make -j$(nproc 2>/dev/null || sysctl -n hw.ncpu)
  emmake make install

  echo "curl built successfully."
}

# ============================================================
# Step 5: Link final WASM module
# ============================================================
link_wasm() {
  echo "=== Linking final WASM module ==="
  local curl_install="$BUILD_DIR/curl-installed"
  local bssl_build="$BUILD_DIR/boringssl/build"
  local nghttp2_install="$BUILD_DIR/nghttp2-installed"
  local brotli_install="$BUILD_DIR/brotli-installed"

  echo "=== Compiling socket shim ==="
  emcc -O2 -c "$SCRIPT_DIR/src/socket_shim.c" \
    -I"$curl_install/include" \
    -o "$BUILD_DIR/socket_shim.o"

  echo "=== Compiling curl wrappers ==="
  emcc -O2 -c "$SCRIPT_DIR/src/curl_wrappers.c" \
    -I"$curl_install/include" \
    -o "$BUILD_DIR/curl_wrappers.o"

  echo "=== Linking final WASM binary ==="
  emcc \
    -O2 \
    "$BUILD_DIR/socket_shim.o" \
    "$BUILD_DIR/curl_wrappers.o" \
    "$curl_install/lib/libcurl-impersonate-chrome.a" \
    "$bssl_build/lib/libssl.a" \
    "$bssl_build/lib/libcrypto.a" \
    "$nghttp2_install/lib/libnghttp2.a" \
    "$brotli_install/lib/libbrotlidec-static.a" \
    "$brotli_install/lib/libbrotlicommon-static.a" \
    -I"$curl_install/include" \
    --js-library "$SCRIPT_DIR/src/wisp-socket-bridge.js" \
    -s MODULARIZE=1 \
    -s EXPORT_ES6=1 \
    -s EXPORT_NAME="CurlImpersonate" \
    -s EXPORTED_FUNCTIONS='["_curl_easy_init","_curl_easy_setopt","_curl_easy_perform","_curl_easy_cleanup","_curl_easy_getinfo","_curl_easy_strerror","_curl_impersonate_chrome116","_curl_slist_append","_curl_slist_free_all","_curl_global_init","_curl_global_cleanup","_curl_setopt_string","_curl_setopt_long","_curl_setopt_ptr","_curl_setopt_cb","_curl_getinfo_long","_malloc","_free"]' \
    -s EXPORTED_RUNTIME_METHODS='["ccall","cwrap","UTF8ToString","stringToUTF8","stringToNewUTF8","writeArrayToMemory","HEAPU8","getValue","setValue","addFunction","removeFunction"]' \
    -s ASYNCIFY \
    -s 'ASYNCIFY_IMPORTS=["wispSocketSend","wispSocketRecv","wispSocketClose","wispSocketWaitForData","poll","__syscall_poll"]' \
    -s ASYNCIFY_STACK_SIZE=65536 \
    -s ALLOW_MEMORY_GROWTH=1 \
    -s INITIAL_MEMORY=16777216 \
    -s TOTAL_STACK=1048576 \
    -s ENVIRONMENT=web,worker \
    -s NO_EXIT_RUNTIME=1 \
    -s ALLOW_TABLE_GROWTH=1 \
    -s ERROR_ON_UNDEFINED_SYMBOLS=0 \
    -Wl,--wrap=socket \
    -Wl,--wrap=connect \
    -Wl,--wrap=send \
    -Wl,--wrap=recv \
    -Wl,--wrap=write \
    -Wl,--wrap=read \
    -Wl,--wrap=close \
    -Wl,--wrap=fcntl \
    -Wl,--wrap=ioctl \
    -Wl,--wrap=select \
    -Wl,--wrap=getsockopt \
    -Wl,--wrap=setsockopt \
    -Wl,--wrap=getpeername \
    -Wl,--wrap=getsockname \
    -Wl,--wrap=getaddrinfo \
    -Wl,--wrap=freeaddrinfo \
    -Wl,--wrap=gai_strerror \
    -o "$DIST_DIR/curl-impersonate.js"

  echo "=== Build complete ==="
  echo "Output: $DIST_DIR/curl-impersonate.js"
  echo "Output: $DIST_DIR/curl-impersonate.wasm"
  ls -lh "$DIST_DIR/"
}

# ============================================================
# Main
# ============================================================
echo "Building curl-impersonate for WASM"
echo "Working directory: $BUILD_DIR"

build_brotli
build_boringssl
build_nghttp2
build_curl
link_wasm

echo "Done!"
