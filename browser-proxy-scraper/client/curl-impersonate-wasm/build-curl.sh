#!/usr/bin/env bash
set -euo pipefail

BASE="$(cd "$(dirname "$0")" && pwd)"
export EMSDK_QUIET=1
source "$BASE/emsdk/emsdk_env.sh"

cd "$BASE/build/curl-8.1.1"

BSSL_DIR="$BASE/build/boringssl/build"
NGHTTP2_DIR="$BASE/build/nghttp2-installed"
BROTLI_DIR="$BASE/build/brotli-installed"
CURL_INSTALL="$BASE/build/curl-installed"

# Remove -pthread (not supported for wasm32 configure tests)
# Use LDFLAGS to point the linker to our WASM libs
emconfigure ./configure \
  --prefix="$CURL_INSTALL" \
  --host=wasm32-unknown-emscripten \
  --with-openssl="$BSSL_DIR" \
  --with-nghttp2="$NGHTTP2_DIR" \
  --with-brotli="$BROTLI_DIR" \
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
  --disable-ntlm \
  --disable-unix-sockets \
  --disable-threaded-resolver \
  --without-librtmp \
  --without-libidn2 \
  --without-libpsl \
  --without-libssh2 \
  --enable-websockets \
  CFLAGS="-I$BSSL_DIR/include -I$NGHTTP2_DIR/include -I$BROTLI_DIR/include" \
  LDFLAGS="-L$BSSL_DIR/lib -L$NGHTTP2_DIR/lib -L$BROTLI_DIR/lib" \
  LIBS="-lssl -lcrypto -lnghttp2 -lbrotlidec-static -lbrotlicommon-static"

emmake make -j$(sysctl -n hw.ncpu 2>/dev/null || nproc)
emmake make install

echo "curl built successfully at $CURL_INSTALL"
