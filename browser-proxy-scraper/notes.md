# Browser Proxy Scraper - Investigation Notes

## 2026-03-09: Project Start

### Goal
Build an E2EE in-browser web scraper where TLS terminates inside browser WASM (via Epoxy/Rustls), and the backend acts as a blind TCP relay only. User visits our domain, opens an iframe loading a target site via proxy, logs in, and we extract GraphQL profile data without the backend seeing plaintext.

### Architecture
```
Browser Page → UV Service Worker → bare-mux → Epoxy-TLS (WASM/Rustls)
    → Wisp WebSocket → Relay Server (blind TCP) → target:443
```

---

## Phase 1: Relay Server

### wisp-js API Research
- Package: `@mercuryworkshop/wisp-js` v0.4.1
- Server entrypoint: `@mercuryworkshop/wisp-js/server`
- Key exports: `server.routeRequest(request, socket, head, connOptions)`, `server.options`
- `routeRequest` handles WebSocket upgrade internally via `ws.WebSocketServer({ noServer: true })`
- `ServerConnection` constructor accepts `{TCPSocket, UDPSocket}` for custom socket classes
- `NodeTCPSocket` interface: `connect()`, `recv()`, `send(data)`, `close()`, `pause()`, `resume()`
- Options include: `hostname_whitelist`, `hostname_blacklist`, `stream_limit_total`, `allow_private_ips`, etc.
- Wisp v2 support with extensions (UDP, MOTD)
- `logging` module: `set_level(level)` where level is numeric (0=DEBUG, 1=INFO, 2=WARN, etc.)

### Implementation
- Created custom `ProxiedTCPSocket` class matching the `NodeTCPSocket` interface
- SOCKS5 handshake implemented manually (no-auth method, RFC 1928)
- HTTP CONNECT proxy also implemented
- Relay starts successfully, `/health` returns 200
- WebSocket upgrades on `/wisp/` path

### Findings
- wisp-js path must end with `/` for Wisp protocol (connection.mjs checks `path.endsWith("/")`)
- The server creates its own `ws.WebSocketServer({ noServer: true })` internally
- No need for external ws dependency in our code — wisp-js handles it

---

## Phase 2: Frontend Proxy Setup

### Package Research
- `@titaniumnetwork-dev/ultraviolet` v3.2.10 — Service Worker proxy
- `@mercuryworkshop/bare-mux` v2.1.8 — Transport abstraction (SharedWorker-based)
- `@mercuryworkshop/epoxy-transport` v3.0.1 — Bare transport using epoxy-tls
- `@mercuryworkshop/epoxy-tls` v2.1.19-1 — WASM TLS (Rustls), ships with `full/` and `minimal/` variants

### UV Configuration
- Default UV config puts handler/client/bundle at root (`/uv.handler.js`)
- Our config puts them under `/uv/` prefix (`/uv/uv.handler.js`)
- Static copy from npm overwrites custom config — solved with `closeBundle` plugin hook
- During dev, custom middleware serves our config at `/uv/uv.config.js`

### UVServiceWorker API
- Has `route(event)`, `fetch(event)`, `on(event, handler)`, `emit(event, data)` methods
- `uv.on("response", handler)` works for intercepting responses
- Fallback: intercept in the fetch handler after `uv.fetch(event)` returns

### Build Verification
- All static assets copy correctly: uv (378KB bundle), bare-mux (2.9KB worker), epoxy (1.7MB with WASM)
- Dev server with COEP/COOP headers
- Vite proxy forwards `/wisp` WebSocket to relay on port 3000

---

## Phase 3: Traffic Interception

### Design
- SW `uv.on("response", handler)` hook intercepts matched URLs
- Pattern matching against: `/api/graphql`, `graphql.instagram.com`, `i.instagram.com/api/v1`
- Clone response, parse JSON, postMessage to all client windows
- Message format: `{ type: "INTERCEPTED_DATA", url, timestamp, payload }`
- Data panel in UI renders intercepted responses in real-time

### Notes
- Response must be cloned before reading body (original consumed by UV)
- Content-Type check prevents parsing non-JSON responses
- Fallback interception catches cases where `uv.on` isn't available

---

## Phase 4: Puppet Agent

### Design
- `puppet-agent.js` injected into proxied pages
- Commands via postMessage: query, queryAll, click, type, scroll, getPageInfo, evaluate
- Request/response correlation using `crypto.randomUUID()` with 10s timeout
- Agent signals readiness via `PUPPET_READY` message

### Injection Strategy
- UV config supports `inject` array for HTML injection
- Alternative: add `<script src="/puppet-agent.js"></script>` to UV config
- Note: UV rewrites `postMessage` — use `window.top.postMessage` as fallback

---

## Phase 5: TLS Fingerprint Analysis

### Known Limitation
- Epoxy-TLS uses Rustls which produces a static, non-browser JA3/JA4 fingerprint
- This is detectable by Instagram/Cloudflare anti-bot systems
- Rustls GitHub issues #1501, #2498 discuss this but no fix yet
- uTLS (Go) can impersonate Chrome's TLS handshake but cannot compile to browser WASM

### Testing Required
- Load `tls.peet.ws/api/all` through proxy to capture JA3/JA4 hash
- Compare against real Chrome JA3
- Test Instagram through proxy, document failure mode

### Upgrade Path (not implemented)
- Server-side Go proxy using uTLS that impersonates Chrome's TLS handshake
- Partially breaks "blind relay" constraint but solves fingerprinting
- Could use split architecture: uTLS for TLS, but still relay encrypted application data

---

## Key Takeaways

1. **wisp-js is well-designed** — supports custom socket classes, making upstream proxy integration clean
2. **UV + bare-mux + epoxy** stack is mature and works well together
3. **Static file management** is the main build complexity — config path overrides needed careful handling
4. **TLS fingerprinting** is the primary limitation for anti-bot evasion (Rustls fingerprint is non-browser)
5. **COEP/COOP headers** required for SharedArrayBuffer — epoxy WASM won't work without them
