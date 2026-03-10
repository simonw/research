#!/usr/bin/env bash
set -euo pipefail

BASE="/Users/kahtaf/Documents/workspace_kahtaf/research/browser-proxy-scraper/client/curl-impersonate-wasm"
export EMSDK_QUIET=1
source "$BASE/emsdk/emsdk_env.sh"

CURL_LIB="$BASE/build/curl-installed/lib/libcurl-impersonate-chrome.a"
BSSL_DIR="$BASE/build/boringssl/build"
NGHTTP2_DIR="$BASE/build/nghttp2-installed"
BROTLI_DIR="$BASE/build/brotli-installed"
DIST_DIR="$BASE/dist"

mkdir -p "$DIST_DIR"

echo "=== Compiling socket shim ==="
emcc -O2 -c "$BASE/src/socket_shim.c" \
  -I"$BASE/build/curl-installed/include" \
  -o "$BASE/build/socket_shim.o"

echo "=== Linking final WASM module ==="

emcc \
  -O2 \
  "$BASE/build/socket_shim.o" \
  "$CURL_LIB" \
  "$BSSL_DIR/lib/libssl.a" \
  "$BSSL_DIR/lib/libcrypto.a" \
  "$NGHTTP2_DIR/lib/libnghttp2.a" \
  "$BROTLI_DIR/lib/libbrotlidec-static.a" \
  "$BROTLI_DIR/lib/libbrotlicommon-static.a" \
  -I"$BASE/build/curl-installed/include" \
  --js-library "$BASE/src/wisp-socket-bridge.js" \
  -s MODULARIZE=1 \
  -s EXPORT_ES6=1 \
  -s EXPORT_NAME="CurlImpersonate" \
  -s EXPORTED_FUNCTIONS='["_curl_easy_init","_curl_easy_setopt","_curl_easy_perform","_curl_easy_cleanup","_curl_easy_getinfo","_curl_easy_strerror","_curl_slist_append","_curl_slist_free_all","_curl_global_init","_curl_global_cleanup","_malloc","_free"]' \
  -s EXPORTED_RUNTIME_METHODS='["ccall","cwrap","UTF8ToString","stringToUTF8","stringToNewUTF8","writeArrayToMemory","HEAPU8","getValue","setValue","addFunction","removeFunction"]' \
  -s ASYNCIFY \
  -s 'ASYNCIFY_IMPORTS=["wispSocketConnect","wispSocketSend","wispSocketRecv","wispSocketClose","wispBridgeInit"]' \
  -s ALLOW_MEMORY_GROWTH=1 \
  -s INITIAL_MEMORY=16777216 \
  -s TOTAL_STACK=1048576 \
  -s ENVIRONMENT=web,worker \
  -s NO_EXIT_RUNTIME=1 \
  -s ALLOW_TABLE_GROWTH=1 \
  -s ERROR_ON_UNDEFINED_SYMBOLS=0 \
  -Wl,--wrap=write \
  -Wl,--wrap=read \
  -Wl,--wrap=close \
  -Wl,--wrap=fcntl \
  -Wl,--wrap=ioctl \
  -o "$DIST_DIR/curl-impersonate.js"

echo "=== Build complete ==="
ls -lh "$DIST_DIR/"
