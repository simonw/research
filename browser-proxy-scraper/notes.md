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

## 2026-03-09: Bug Fixes & Connector Runner

### Bugs Found & Fixed
1. **XOR encoding**: `i % 1 === 0` always true — XOR'd ALL chars instead of odd-indexed. Fixed to match UV codec exactly.
2. **No `inject` array**: puppet-agent.js was never injected into proxied pages. Added to UV config.
3. **Hardcoded Instagram patterns**: Replaced with dynamic capture registry. SW now supports `CAPTURE_REGISTER`/`CAPTURE_GET` messages.
4. **Fallback matched encoded URLs**: Removed fallback, added proper `xorDecode()` to decode original URLs before matching.

### Connector Runner Architecture
Modeled after [vana-com/data-connectors](https://github.com/vana-com/data-connectors):
- `page-api.ts`: Implements Vana-compatible `page` global (goto, evaluate, captureNetwork, getCapturedResponse, setData, sleep)
- `connector-runner.ts`: Executes connector scripts with `page` API injected
- Connector scripts are standalone JS files, one per site
- Example: `codepen.js` captures `/graphql` responses from codepen.io

### UVServiceWorker Internal Details (from source analysis)
- `UVServiceWorker` extends Node.js `EventEmitter` — `on()`, `emit()` work
- Three hooks: `"request"` (before fetch), `"beforemod"` (after fetch, before rewrite), `"response"` (after rewrite)
- `inject` config: `[{ host: "regex", injectTo: "head"|"body", html: "..." }]`
- Inject runs BEFORE `rewriteHtml()`, using naive `indexOf("<head>")`
- XOR codec: odd-indexed chars XOR'd with 2, even chars pass through, then `encodeURIComponent`

---

## 2026-03-09: E2E Test & Bug Fixes

### Bugs Found During E2E Testing

1. **epoxy-transport v3.0.1 / bare-mux v2.1.8 incompatibility**: `for (let [key, value] of headers)` in epoxy-transport's `request()` and `connect()` methods fails because bare-mux sends headers as a plain object (not iterable). Fixed via patch-package: use `Object.entries()` fallback.

2. **TLS certificate validation**: epoxy-tls (Rustls WASM) doesn't ship root CA certificates. `disable_certificate_validation` option exists in EpoxyClientOptions but wasn't exposed in the `opts` array. Fixed via patch-package + transport config option.

3. **rawHeaders format mismatch**: UV's BareClient in uv.bundle.js sets `s.rawHeaders = t.headers` where `t.headers` is an array of `[key, value]` pairs (from epoxy-transport), but UV's response handler expects a plain object `{key: value}`. This caused UV to skip HTML rewriting (no content-type header found), which meant no puppet-agent injection and no UV client scripts. Fixed via patch-package: convert array to object.

4. **Puppet-agent injection via UV `inject` config**: UV's inject inserts HTML BEFORE `<head>`, but then UV's `rewriteHtml()` wraps everything in its own `<head>` structure, creating invalid nested `<head>` elements. Fixed by moving puppet-agent injection to sw.js's fetch handler AFTER UV processing — inserts inline `<script>` before `</head>`.

5. **Service Worker controller null**: `navigator.serviceWorker.controller` is null on first page load after SW registration. Fixed by adding `self.skipWaiting()` + `self.clients.claim()` to SW, and waiting for `controllerchange` event in main.ts.

### E2E Test Results

**Proxy test (example.com)**: PASS
- UV proxy loads external sites through epoxy/wisp transport
- XOR encoding/decoding works correctly
- COEP/COOP headers + SharedArrayBuffer functional

**Puppet agent**: PASS
- Inline injection via SW fetch handler works
- PUPPET_READY message received by parent frame
- DOM commands (query, evaluate, getPageInfo) functional

**CodePen connector**: PARTIAL
- Connector runner executes scripts correctly
- Network capture registration and retrieval works
- CodePen blocked by Cloudflare bot protection ("Performing security verification")
- No GraphQL data captured due to Cloudflare challenge page

### Patches Created (via patch-package)
- `patches/@mercuryworkshop+epoxy-transport+3.0.1.patch` — headers iteration fix + disable_certificate_validation option
- `patches/@titaniumnetwork-dev+ultraviolet+3.2.10.patch` — rawHeaders format normalization

---

## Key Takeaways

1. **wisp-js is well-designed** — supports custom socket classes, making upstream proxy integration clean
2. **UV + bare-mux + epoxy** stack has version incompatibilities between packages — headers format (plain object vs array of pairs) differs between bare-mux v2.1.8 and epoxy-transport v3.0.1
3. **Static file management** is the main build complexity — config path overrides needed careful handling
4. **TLS fingerprinting** is the primary limitation for anti-bot evasion (Rustls fingerprint is non-browser)
5. **COEP/COOP headers** required for SharedArrayBuffer — epoxy WASM won't work without them
6. **Cloudflare bot protection** blocks headless/proxy access to CodePen — the Rustls TLS fingerprint is likely flagged
7. **UV inject config** inserts before `<head>` which breaks with UV's own HTML rewriting — inject AFTER UV processing instead

---

## 2026-03-10: Curl/Wisp Hardening Pass

### Changes Implemented
- Added runtime debug toggles for the curl/Wisp path:
  - `window.__CURL_WASM_DEBUG = true`
  - `window.__WISP_BRIDGE_DEBUG = true`
- Reworked `client/curl-impersonate-wasm/src/wisp-socket-bridge.js`:
  - per-socket state now tracks `connState`, `closeReason`, `sendCredits`, `pendingSends`, queued recv chunks, and waiter lists
  - added Wisp `CONTINUE` handling for stream `0` and per-stream credits
  - `poll(timeout=0)` now returns synchronously
  - implemented a Wisp-aware `__syscall_poll` because the final WASM imports `env.__syscall_poll`, not `env.poll`
  - immediate recv path is synchronous when bytes are already queued
- Updated `client/curl-impersonate-wasm/src/socket_shim.c`:
  - added a JS-controlled debug gate for shim logging
  - switched non-blocking `connect()` back to `EINPROGRESS`
  - kept `getsockopt(SO_ERROR)` zero until the bridge records an actual error
- Updated `client/curl-impersonate-wasm/src/curl-wasm-fetch.js`:
  - added request/header/body/status debug logs
  - made libcurl verbose output conditional on debug mode
  - fixed cleanup so callbacks/allocations are released on error paths too
- Generalized captcha handling in `client/public/sw.js`:
  - replaced Turnstile-only bypass rules with a vendor registry for Turnstile, reCAPTCHA, and hCaptcha
  - HTML rewriting now injects direct-origin captcha scripts for known vendor URLs
  - UV bypass now applies only to requests whose decoded target host is a known captcha vendor host

### Current Debug Findings
- `wasmFetch('https://example.com')` now progresses past the old deadlock point:
  - curl creates the socket
  - non-blocking `connect()` returns `EINPROGRESS`
  - `__syscall_poll` is hit and reports `POLLOUT` / `POLLIN` correctly
  - the ClientHello is sent
  - Wisp DATA frames containing server TLS records arrive
- The request still does **not** complete:
  - the proxied iframe remains blank
  - direct `wasmFetch('https://example.com')` times out
  - browser errors still show `unreachable`
  - longer waits can hit `RuntimeError: function signature mismatch`

### Likely Remaining Blocker
- There is still a post-connect ABI/runtime issue after data starts flowing:
  - either an Asyncify edge in the recv/poll path
  - or another callback/import signature mismatch deeper in curl/BoringSSL
- Important evidence:
  - the final WASM imports `env.__syscall_poll`
  - the transport now reaches TLS record receipt
  - the failure is no longer “cannot connect”; it is now “data received but curl never completes / traps later”

## 2026-03-10 22:18:37 EDT
- Identified remaining runtime failure as `RuntimeError: function signature mismatch` in wasm function `_emscripten_timeout` during `curl_easy_perform`.
- Confirmed Emscripten `_setitimer_js` schedules `__emscripten_timeout(which, now)` from JS timers.
- Confirmed libcurl supports `CURLOPT_NOSIGNAL` (option 99) specifically to disable signal/alarm handlers.
- Patched `curl-wasm-fetch.js` to set `CURLOPT_NOSIGNAL=1` on every request before retesting.
- Reworked poll waiting: timed Wisp polls no longer Asyncify-suspend inside `__syscall_poll`; instead they report synthetic readiness and defer the actual async wait to `recv()` / `read()`.
- Removed non-blocking `EAGAIN` short-circuit for virtual Wisp fds so `recv()` can be the suspension point after a synthetic poll wake.
- Identified that async imports were returning synchronously on Asyncify rewind once data was already queued; added `maybeAsyncReturn()` so rewind paths still go through `Asyncify.handleAsync(Promise.resolve(...))` for `recv`, `send`, wait, and poll immediate returns.
- After making async imports rewind-safe, direct `wasmFetch(https://example.com)` no longer traps. It now fails cleanly with libcurl `CURLE_SSL_CONNECT_ERROR (35)`, indicating the remaining blocker is handshake/transport semantics rather than Asyncify control flow.
- Added Wisp DATA hex previews to inspect whether the relay is returning valid TLS records or plaintext/proxy responses.

## 2026-03-10 22:34:57 EDT
- Found the core TLS corruption bug in the Asyncify import design: wispSocketRecv() was consuming bytes once in the async promise resolution path and again on rewind, so a single logical recv() advanced two chunks.
- Reworked wisp-socket-bridge async imports to be rewind-safe by storing resume state (recvResume/sendResume/waitResume) and only consuming bytes on the rewind pass.

- Observed that the rewind-safe recv patch changes the latest run behavior: generated dist now shows recvResume-based logic, and the newest console trace reaches `recv async ready` + `recv resumed` instead of the earlier immediate double-consume path.
- Fixed a follow-up rewind bug: recvResolver must be cleared before storing recvResume, otherwise a later CLOSE packet can invoke the stale resolver for the same logical recv and wedge the rewind.
- New hypothesis confirmed in generated Asyncify runtime: rewind-time completion still needs Asyncify.handleAsync(Promise.resolve(value)); the sync resume return alone leaves the async thunk suspended. Patched resumed recv/send/wait returns to use an explicit asyncifyResumeReturn helper.
- Patched async promise resolutions so Asyncify.handleSleepReturnValue carries the real syscall result (recv byte count / EOF / error / send result / wait result). Without that, rewind completed but C observed result=0.
- Breakthrough: direct example.com probe now completes the full TLS handshake. Console shows TLSv1.3 negotiated, ALPN h2 accepted, certificate parsed, and multiple post-handshake recv() calls returning expected byte counts.

## 2026-03-10 23:30:12 EDT
- Reworked the post-handshake I/O model again:
  - restored non-blocking `recv()` / `read()` semantics in `socket_shim.c` (`EAGAIN` when no data is queued),
  - moved the actual async wait back into `__syscall_poll` / `poll` with saved resume state in `wisp-socket-bridge.js`,
  - disabled `_setitimer_js` in the JS library to prevent Emscripten signal timer callbacks from re-entering `_emscripten_timeout`.
- Confirmed direct `wasmFetch('https://example.com')` now completes end-to-end:
  - status `200`,
  - HTML body returned,
  - iframe test page no longer blank.
- Found and fixed a cleanup-order bug in `curl-wasm-fetch.js`:
  - `curlEasyCleanup(handle)` must run before freeing `curl_slist` header lists and removing callback function-table entries.
  - The previous order caused memory corruption / out-of-bounds traps after successful requests with custom headers.
- Found and fixed the remaining multi-request corruption:
  - the Asyncify-enabled WASM module is not re-entrant,
  - parallel proxied subresource loads were corrupting shared state,
  - added a request queue in `curl-wasm-fetch.js` so `wasmFetch()` calls are serialized through one module instance.
- Validation after the queueing fix:
  - proxied `https://example.com/` renders correctly in the iframe,
  - proxied `https://tls.peet.ws/api/all` renders JSON in the iframe,
  - observed TLS fields include `http_version: "h2"`, JA3 `771,4867-4865-4866-49195-49199-49196-49200-52393-52392-49171-49172-156-157-47-53,...`, JA3 hash `afbd0a4609da8f5101cdcde48d53e590`, and JA4 `t13d1515h2_8daaf6152771_5d45727bf495`.

## 2026-03-10 23:58:41 EDT
- reCAPTCHA validation:
  - tested `https://www.google.com/recaptcha/api2/demo` in the headed `agent-browser` session,
  - proxied iframe rendered the demo page,
  - nested reCAPTCHA iframe loaded with title `reCAPTCHA`,
  - widget visible at `304x78`.
- hCaptcha validation:
  - tested `https://accounts.hcaptcha.com/demo`,
  - proxied iframe rendered the demo page,
  - checkbox iframe and challenge iframe both loaded from `https://newassets.hcaptcha.com/.../hcaptcha.html`,
  - widget titles observed: `Widget containing checkbox for hCaptcha security challenge` and `hCaptcha challenge`.
- Turnstile path:
  - initial demo pages failed because the Turnstile script tag was still getting UV-rewritten in the HTML,
  - updated `sw.js` to reinsert captcha scripts via inline runtime loaders instead of static `src=` tags,
  - added an origin shim so proxied pages can expose the decoded target origin to page scripts (`window.origin`, `document.URL`, `document.documentURI`, `document.baseURI`, `document.referrer`, `document.location` getter).
- Turnstile current status:
  - tested `https://turnstiledemo.lusostreams.com/`,
  - direct Turnstile vendor script now loads as `https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit`,
  - `window.turnstile` exists in the proxied page,
  - page render code (`turnstile.render(...)`) is present,
  - user-reported intercepted challenge response from Cloudflare shows `{\"err\": 110200}`.
- Interpretation of the remaining Turnstile blocker:
  - transport and vendor script loading are working,
  - Cloudflare is rejecting the widget at the challenge flow stage with a domain-authorization error,
  - this is consistent with the proxied page still fundamentally executing under the localhost/browser-proxy origin rather than the site's true origin for host-bound Turnstile sitekeys.
## 2026-03-10 Turnstile follow-up

- Reused headed `agent-browser` session `proxy-headed3` throughout.
- Confirmed the main transport remains fixed:
  - proxied pages render,
  - `tls.peet.ws` renders,
  - reCAPTCHA and hCaptcha still render in the proxied iframe.
- Turnstile-specific work in `client/public/sw.js`:
  - moved captcha loader injection after the origin shim,
  - replaced raw Turnstile `<script src=...>` with a wrapped loader that executes `api.js` against a proxied `window/document/location/top/parent`,
  - added a generic DOM shim so captcha vendor-owned script `src` assignments stay direct,
  - added a Turnstile child-frame shim for `challenge-platform` HTML that normalizes inbound `message.origin` using the decoded parent referrer.
- Key live findings from headed browser debugging:
  - `window.turnstile` now initializes in the proxied page.
  - The page’s inline `turnstile.render(...)` calls run.
  - When forcing shadow roots open for debugging, the inner Turnstile challenge iframe is created and the parent receives `init` / `requestExtraParams`.
  - Despite that, the visible widget area remains blank and Turnstile logs `Turnstile Widget seem to have hung`.
  - Manual probing showed multiple plausible failure points:
    - UV client-side rewriting of nested iframe `src`,
    - child/parent `MessageEvent.origin` mismatch,
    - child-frame handshake stalling after `requestExtraParams`.
- Important constraint learned:
  - The remaining blocker appears tied to Turnstile’s child-frame handshake needing the real embedding origin semantics, not just spoofed `window.location` or direct network access.
  - This is qualitatively different from reCAPTCHA/hCaptcha, which now work under the current proxy model.
- Current conclusion:
  - Within the existing `localhost` iframe proxy architecture, Turnstile is still not reliably renderable even after:
    - Chrome-116 TLS impersonation,
    - service-worker vendor bypass,
    - direct Turnstile script execution,
    - nested frame URL handling,
    - message-origin normalization experiments.
  - The next viable path likely requires an architectural change so the embedded page has a real browser origin that matches the target site more closely, instead of a `localhost` iframe origin with shims layered on top.

---

## 2026-03-11: E2E Browser Testing of Captcha & Fingerprint Bypass

### Setup
- Relay on port 3000, client dev server on port 5175
- Chrome 145.0.7632.160 on macOS (Apple M1 Pro)
- Tested via browser automation tools against live services

### Results Summary
| Test | Result |
|------|--------|
| Basic proxy (example.com) | PASS |
| Fingerprint self-test | 5/7 pass (WebGL not spoofed) |
| TLS fingerprint | JA4 matches, JA3 differs (version) |
| reCAPTCHA v2 | PASS - widget renders, no domain error |
| hCaptcha | PASS - auto-verified! |
| Cloudflare Turnstile | FAIL - widget loads, verification hangs |
| CreepJS audit | 25-40% headless/stealth signals |

### Key Findings
1. **Origin spoofing works for reCAPTCHA and hCaptcha** - hCaptcha auto-verified with `"success": true, "hostname": "hcaptcha.com"`. This was the strongest result.
2. **Turnstile still broken** - Widget renders but can't complete verification. Confirms prior analysis.
3. **WebGL is the biggest fingerprint gap** - vendor/renderer expose real hardware (Apple M1 Pro via ANGLE).
4. **WebRTC leaks real IP** - STUN connection reveals public IP through the proxy.
5. **Worker context unpatched** - anti-detect.js only patches main thread; workers report real arch (arm_64).
6. **JA4 TLS fingerprint matches** - `t13d1515h2_8daaf6152771_5d45727bf495` confirms Chrome-like TLS.

### Detailed logs
See `investigation-captcha-fingerprint/e2e-test-notes.md`
