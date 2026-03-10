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
