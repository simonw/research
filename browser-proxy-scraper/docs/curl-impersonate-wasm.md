# curl-impersonate WASM — Build Notes

## Problem
The E2EE web proxy (UV + bare-mux + epoxy-tls/Rustls WASM + Wisp relay) is blocked by Cloudflare because Rustls produces a non-browser JA3/JA4 TLS fingerprint. Server-side TLS termination is rejected (Data Processor liability). rquest/wreq cannot compile to wasm32-unknown-unknown (boring-sys C FFI, tokio net, BoringSSL syscalls are hard blockers).

## Solution
Compile curl-impersonate (libcurl + patched BoringSSL + patched nghttp2) to WASM via Emscripten, with a custom POSIX socket bridge that routes encrypted bytes through the Wisp WebSocket.

## Build Process

### Dependencies installed
- Emscripten SDK (emsdk latest)
- cmake, ninja (via homebrew)
- autoconf, automake, libtool (via homebrew)

### Libraries compiled to WASM
1. **Brotli 1.0.9** — compression library
   - Built with `emcmake cmake` + `emmake make`
   - Required `-DCMAKE_POLICY_VERSION_MINIMUM=3.5` for cmake compat
   - `cmake --install` didn't work with emcmake; manually copied .a files and headers

2. **BoringSSL** (commit `1b7fdbd9101dedc3e0aa3fcf4ff74eacddb34ecc`)
   - Applied `boringssl-old-ciphers.patch` from curl-impersonate (Chrome cipher suite)
   - Key flag: `-DOPENSSL_NO_ASM=1` (required for WASM — no x86/ARM assembly)
   - Built with `emcmake cmake` + `emmake make ssl crypto`
   - Set up lib/ symlinks and include/ copy for curl's `--with-openssl`

3. **nghttp2 1.56.0** — HTTP/2 library
   - Built with `emconfigure ./configure --host=wasm32-unknown-emscripten` + `emmake make`
   - Used `--enable-lib-only --disable-shared --enable-static`

4. **libcurl 8.1.1** (curl-impersonate Chrome variant)
   - Applied patches: `curl-CVE-2023-38545.patch`, `curl-impersonate.patch`
   - Ran `autoreconf -fi` to regenerate configure after patching
   - Configured with `emconfigure ./configure --host=wasm32-unknown-emscripten`
   - Disabled unneeded protocols (FTP, LDAP, SMTP, etc.)
   - Output: `libcurl-impersonate-chrome.a`

### Socket Shim (Critical)
The biggest challenge was routing curl's POSIX socket calls through Wisp:
- curl resolves DNS via `getaddrinfo()`, then calls `socket()` → `connect()` → `send()`/`recv()`
- Created `socket_shim.c` that provides custom implementations of all these
- **DNS trick**: `getaddrinfo()` stores hostname→fake-IP mapping (127.0.1.x), returns fake sockaddr. When `connect()` is called with that fake IP, looks up the real hostname and connects via Wisp.
- **Wisp bridge**: JS library (`wisp-socket-bridge.js`) provides actual network transport via Wisp WebSocket protocol
- Used `Asyncify` to suspend WASM execution during async recv() calls

### Linking
- All static libs linked with `emcc -O2`
- `-s ASYNCIFY` for suspending WASM on async operations
- `-s ERROR_ON_UNDEFINED_SYMBOLS=0` for some unused POSIX stubs
- `-s ALLOW_TABLE_GROWTH=1` for function pointers (curl callbacks)
- Output: `curl-impersonate.js` (94K) + `curl-impersonate.wasm` (2.1M)

## Issues Encountered
1. cmake too new for brotli's cmake_minimum_required — fixed with `-DCMAKE_POLICY_VERSION_MINIMUM=3.5`
2. autoconf not properly linked on macOS — needed `brew reinstall autoconf`
3. Environment variables lost between bash tool invocations — moved to shell scripts
4. First link attempt failed: Emscripten's SOCKFS/FS modules required — solved by providing custom socket shim in C that replaces POSIX socket APIs entirely
5. `-pthread` caused "C compiler cannot create executables" during curl configure — removed, not needed for single-threaded WASM

## Other Changes Made

### Phase 1: SOCKS5 Auth
- Added RFC 1929 username/password auth to `relay/src/upstream-proxy.ts`
- Now offers both no-auth (0x00) and user/pass (0x02) methods
- Credentials parsed from URL: `decodeURIComponent(new URL(proxyUrl).username)`

### Phase 4: Anti-Detection
- Created `client/public/anti-detect.js` — hides webdriver flag, mocks chrome.runtime, plugins, languages, permissions
- Updated `client/public/sw.js` to inject anti-detect.js before puppet-agent.js in proxied HTML
- Added Turnstile bypass: `challenges.cloudflare.com` and `turnstile.cloudflare.com` fetched directly (not through UV)
