# curl-impersonate WASM — Cloudflare Bot Management Bypass

## Overview

This investigation compiled [curl-impersonate](https://github.com/lwthiker/curl-impersonate) (libcurl + patched BoringSSL + patched nghttp2) to WebAssembly via Emscripten, enabling Chrome-matching TLS fingerprints (JA3/JA4) from within a browser context.

The goal: replace epoxy-tls (Rustls WASM) in the browser-proxy-scraper's E2EE web proxy stack, which gets blocked by Cloudflare because Rustls produces a non-browser TLS fingerprint.

## Architecture

```
Browser iframe request
  → UV Service Worker (sw.js)
  → bare-mux SharedWorker
  → curl-impersonate WASM          ← NEW (replaces epoxy-tls)
    → BoringSSL (Chrome cipher suite, Chrome TLS extensions)
    → nghttp2 (Chrome HTTP/2 SETTINGS)
    → socket_shim.c (POSIX → Wisp bridge)
    → wisp-socket-bridge.js (Wisp WebSocket transport)
  → WebSocket → Wisp relay → target:443
```

**Key insight**: BoringSSL inside the WASM module handles all TLS encryption with Chrome-matching parameters. The socket bridge only transports raw encrypted bytes — it never sees plaintext. This preserves the E2EE property.

## What Was Built

### WASM Libraries (all compiled to static WASM archives)

| Library | Version | Size | Purpose |
|---------|---------|------|---------|
| BoringSSL | `1b7fdbd` | ~1.5M | TLS with Chrome cipher suite (patched) |
| nghttp2 | 1.56.0 | ~200K | HTTP/2 with Chrome SETTINGS |
| Brotli | 1.0.9 | ~300K | Compression |
| libcurl | 8.1.1 | ~700K | HTTP client (impersonate patches) |

### Final Output

| File | Size | Description |
|------|------|-------------|
| `curl-impersonate.js` | 94K | Emscripten JS glue code |
| `curl-impersonate.wasm` | 2.1M | WASM module |

### Custom Components

1. **`socket_shim.c`** — Replaces POSIX socket APIs with Wisp-routed equivalents
   - `getaddrinfo()`: Maps hostnames to fake IPs (127.0.1.x) for later lookup
   - `socket()` / `connect()` / `send()` / `recv()` / `close()`: Route through JS bridge
   - `poll()` / `select()`: Simplified stubs (always report ready)

2. **`wisp-socket-bridge.js`** — Emscripten JS library for Wisp transport
   - Implements Wisp protocol (CONNECT, DATA, CLOSE packets)
   - Per-fd receive queues with Asyncify suspension for blocking recv()
   - Connected to the relay's Wisp WebSocket

3. **`curl-wasm-fetch.js`** — High-level `wasmFetch(url, options)` API
   - fetch()-like interface wrapping curl_easy_* calls
   - Handles headers, request body, response parsing
   - Returns Response-like object with .text(), .json(), .arrayBuffer()

## Other Changes

### SOCKS5 Authentication (Phase 1)
Added RFC 1929 username/password auth to `relay/src/upstream-proxy.ts` for residential proxy support.

### Anti-Detection (Phase 4)
- `client/public/anti-detect.js`: Hides webdriver flag, mocks chrome.runtime, plugins, languages, permissions
- `client/public/sw.js`: Injects anti-detect before puppet-agent; bypasses UV for Cloudflare Turnstile domains

## Verification Checklist

- [x] BoringSSL compiles to WASM with `-DOPENSSL_NO_ASM=1`
- [x] nghttp2 compiles to WASM
- [x] Brotli compiles to WASM
- [x] libcurl (curl-impersonate Chrome variant) compiles to WASM
- [x] All libraries link into a single WASM module
- [x] SOCKS5 auth added to relay
- [x] Anti-detect script created
- [x] Turnstile bypass added to SW
- [ ] End-to-end test: TLS fingerprint matches Chrome (requires running instance)
- [ ] End-to-end test: Turnstile renders and passes
- [ ] End-to-end test: CodePen connector works without Cloudflare block

## Build Instructions

```bash
cd client/curl-impersonate-wasm

# One-time: install Emscripten + build deps
git clone https://github.com/emscripten-core/emsdk.git
cd emsdk && ./emsdk install latest && ./emsdk activate latest && cd ..
brew install cmake ninja autoconf automake libtool

# Build everything
chmod +x build.sh build-curl.sh link-wasm.sh
./build.sh   # or run build-curl.sh + link-wasm.sh separately

# Output in dist/
ls dist/  # curl-impersonate.js, curl-impersonate.wasm
```

## Files Modified/Created

| File | Action | Description |
|------|--------|-------------|
| `relay/src/upstream-proxy.ts` | Modified | SOCKS5 username/password auth |
| `relay/.env.example` | Created | Documents proxy config |
| `client/curl-impersonate-wasm/` | Created | WASM build workspace |
| `client/curl-impersonate-wasm/src/socket_shim.c` | Created | POSIX socket → Wisp bridge (C) |
| `client/curl-impersonate-wasm/src/wisp-socket-bridge.js` | Created | Wisp WebSocket transport (JS) |
| `client/curl-impersonate-wasm/src/curl-wasm-fetch.js` | Created | fetch()-like API wrapper |
| `client/curl-impersonate-wasm/build.sh` | Created | Full build script |
| `client/curl-impersonate-wasm/build-curl.sh` | Created | curl-only build script |
| `client/curl-impersonate-wasm/link-wasm.sh` | Created | Final WASM linking script |
| `client/src/transport.ts` | Modified | Tries curl-impersonate WASM, falls back to epoxy |
| `client/vite.config.ts` | Modified | Copies curl-impersonate artifacts |
| `client/public/sw.js` | Modified | Anti-detect injection + Turnstile bypass |
| `client/public/anti-detect.js` | Created | Browser environment mocking |

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
