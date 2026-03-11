# Browser Proxy Scraper

An E2EE in-browser web scraper where TLS terminates entirely inside the browser via WebAssembly. The backend acts as a blind TCP relay — it never sees plaintext traffic, cookies, or session tokens. Users browse target sites through an iframe-based proxy, and the system intercepts and extracts API data (e.g., GraphQL responses) client-side.

The primary transport uses **curl-impersonate compiled to WASM**, which produces a Chrome-matching TLS fingerprint (JA3/JA4) to bypass Cloudflare and similar bot detection. An epoxy-tls (Rustls) fallback is available when the WASM module fails to load.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│  Browser                                                            │
│                                                                     │
│  ┌───────────┐   ┌──────────────┐    ┌───────────────────────────┐  │
│  │  Main     │   │  UV Service  │    │  curl-impersonate (WASM)  │  │
│  │  Page     │──▶│  Worker      │───▶│  BoringSSL + Chrome TLS   │  │
│  │  + iframe │   │  + bare-mux  │    │  TLS terminates here      │  │
│  └───────────┘   └──────────────┘    └────────────┬──────────────┘  │
│       │                                           │ encrypted       │
│       │  ┌─────────────────────────────┐          │                 │
│       └──│  Injected into proxied page │          │                 │
│          │  • anti-detect.js           │          │                 │
│          │  • puppet-agent.js          │          │                 │
│          └─────────────────────────────┘          │                 │
└───────────────────────────────────────────────────┼─────────────────┘
                                                    │ WebSocket
                                                    │ (Wisp protocol)
┌───────────────────────────────────────────────────┼─────────────────┐
│  Relay Server                                     │                 │
│                                                   ▼                 │
│  ┌─────────────────┐    ┌────────────────────────────────────────┐  │
│  │  Wisp Server    │───▶│  Blind TCP relay                       │  │
│  │  (wisp-js)      │    │  Only sees encrypted bytes             │  │
│  └─────────────────┘    │  Optional: upstream SOCKS5/HTTP proxy  │  │
│                         └───────────────┬────────────────────────┘  │
└─────────────────────────────────────────┼───────────────────────────┘
                                          │ TCP
                                          ▼
                                   ┌──────────────┐
                                   │  Target Site │
                                   └──────────────┘
```

### Transport Fallback

| Priority | Transport | TLS Library | Fingerprint | Status |
|----------|-----------|-------------|-------------|--------|
| Primary | curl-impersonate WASM | BoringSSL (patched) | Chrome-matching JA3/JA4 | Working, serialized requests |
| Fallback | epoxy-tls | Rustls | Non-browser (detectable) | Stable, blocked by Cloudflare |

## How It Works

### Boot Sequence

1. Service Worker registers at root scope (`/`), pre-fetches `anti-detect.js` and `puppet-agent.js`
2. Transport initializes: tries curl-impersonate WASM first, falls back to epoxy-tls if it fails
3. User enters a URL → XOR-encoded → loaded in an iframe under the `/service/` prefix
4. UV Service Worker intercepts all iframe requests and routes them through the active transport → Wisp WebSocket → relay → target

### Origin Spoofing

The SW rewrites outbound requests and inbound responses to convince target sites (and their captcha widgets) that the page is running on the target's own origin:

- **`Sec-Fetch-Site` header**: Recomputed based on the decoded target URL and referer (same-origin/same-site/cross-site)
- **`ancestorOrigins`**: All JS responses are AST-rewritten to replace `.ancestorOrigins` with `.fakeAncestorOrigins`, which returns a DOMStringList containing the target origin
- **Referrer/location**: `anti-detect.js` patches `document.referrer` and `Location.prototype` to expose the target origin

### Anti-Detect Fingerprinting

`anti-detect.js` is injected into every proxied page before `</head>`. It patches:

| Surface | What It Does |
|---------|-------------|
| `navigator.webdriver` | Removed (set to `undefined`) |
| `navigator.plugins` | Replaced with 3 Chrome PDF plugins |
| `navigator.userAgentData` | Synthesized with correct brands, `getHighEntropyValues()` support |
| Canvas | Deterministic per-pixel noise (+/-1 RGB at 64px stride, session-seeded) |
| WebGL | `UNMASKED_VENDOR/RENDERER` sanitized (headless/SwiftShader strings replaced) |
| `window.chrome` | Stubbed `chrome.runtime`, `chrome.app`, `chrome.csi()`, `chrome.loadTimes()` |
| Window metrics | `outerWidth`/`outerHeight` ensured larger than inner dimensions |
| UV leaks | `__uv$config` deleted from proxied page scope |

### Captcha Bypass

Captcha vendor scripts (Turnstile, reCAPTCHA, hCaptcha) receive special treatment in the SW:

- `.ancestorOrigins` access rewritten to the fake version
- `window.top !== window.self` comparisons rewritten to `false` (so captcha scripts think they're top-level)
- Vendor JS is fetched and served with these rewrites applied

## Getting Started

### Prerequisites

- Node.js 18+
- npm

### Relay Server

```bash
cd relay
npm install
npm run dev
# Listening on http://localhost:3000
# Wisp endpoint: ws://localhost:3000/wisp/
```

Environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `3000` | Server port |
| `UPSTREAM_PROXY` | none | SOCKS5 or HTTP proxy URL (e.g., `socks5://user:pass@proxy:1080`) |
| `ALLOWED_HOSTS` | all | Comma-separated hostname whitelist |
| `MAX_STREAMS` | unlimited | Max concurrent Wisp streams |

### Client

```bash
cd client
npm install
npm run dev
# Dev server on http://localhost:5173
# Proxies /wisp to relay server
```

The Vite dev server adds `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp` headers (required for SharedArrayBuffer, which the WASM transport needs).

### Building curl-impersonate WASM (optional)

Pre-built artifacts are included. To rebuild from source:

```bash
cd client/curl-impersonate-wasm

# Install Emscripten + build deps
git clone https://github.com/emscripten-core/emsdk.git
cd emsdk && ./emsdk install latest && ./emsdk activate latest && cd ..
brew install cmake ninja autoconf automake libtool

# Build
chmod +x build.sh build-curl.sh link-wasm.sh
./build.sh

# Output: dist/curl-impersonate.js (94K) + dist/curl-impersonate.wasm (2.1M)
```

## Connector Runner

Site-specific scraping scripts modeled after [vana-com/data-connectors](https://github.com/vana-com/data-connectors). Select a connector from the dropdown and click "Run", or paste custom script code.

### Page API

```js
await page.goto('https://example.com');
await page.evaluate('document.title');
await page.captureNetwork({ urlPattern: '/graphql', key: 'gql' });
await page.sleep(2000);
const responses = await page.getCapturedResponse('gql');
await page.setData(responses);
```

### Traffic Interception

The SW maintains a dynamic capture registry:

- **Register**: `page.captureNetwork({ urlPattern, key })` — regex pattern matching against decoded URLs
- **Retrieve**: `page.getCapturedResponse(key)` — returns array of matched JSON responses
- **Intercept all**: Toggle the "Intercept All JSON" checkbox to broadcast every JSON response to the data panel

## Puppet Agent

DOM automation via postMessage bridge, injected into every proxied page:

```js
puppet.sendCommand('query', { selector: 'h1' })
puppet.sendCommand('click', { selector: '.btn-primary' })
puppet.sendCommand('type', { selector: '#username', text: 'hello' })
puppet.sendCommand('getPageInfo')
puppet.sendCommand('queryAll', { selector: 'a', limit: 10 })
puppet.sendCommand('evaluate', { code: 'document.title' })
```

Or via the input field: `query h1`, `click .btn`, `getPageInfo`, etc.

## E2E Test Results (2026-03-11)

Tested with Chrome 145 on macOS against live services.

| Test | Result | Notes |
|------|--------|-------|
| Basic proxy (example.com) | **PASS** | Page renders, puppet agent active |
| Fingerprint self-test | **5/7 PASS** | WebGL vendor/renderer not spoofed in test (now patched) |
| TLS fingerprint (JA4) | **PASS** | `t13d1515h2_8daaf6152771_5d45727bf495` matches Chrome |
| reCAPTCHA v2 | **PASS** | Widget renders with no domain errors |
| hCaptcha | **PASS** | Auto-verified without showing a challenge |
| Cloudflare Turnstile | **FAIL** | Widget loads but verification hangs |
| CreepJS audit | **INFO** | 25-40% headless/stealth signals detected |

## Security Model

| Layer | What It Sees |
|-------|-------------|
| Browser page | Plaintext HTTP content (same-origin via SW) |
| Service Worker | Decoded URLs, request/response bodies |
| curl-impersonate WASM | Plaintext (performs TLS handshake with BoringSSL) |
| WebSocket frames | Encrypted TLS records only |
| Relay server | Encrypted bytes + destination host:port |
| Network observer | WebSocket frames containing encrypted data |

The relay knows *which* hosts the user connects to (from Wisp CONNECT packets) but never sees plaintext content, cookies, or auth tokens.

## Known Limitations

### Cloudflare Turnstile

The Turnstile widget renders and begins verification but never completes. Turnstile performs deeper environment checks (worker integrity, timing analysis, execution context) that detect the proxy. reCAPTCHA and hCaptcha work.

### WebRTC IP Leak

STUN connections bypass the proxy's TCP tunnel and expose the user's real public IP. Needs WebRTC disabling or ICE candidate routing through the proxy.

### Worker Context Unpatched

`anti-detect.js` only patches the main thread. Web Workers report real hardware info (e.g., real CPU architecture) which CreepJS and similar tools can detect.

### WASM Request Serialization

The Asyncify-enabled curl-impersonate WASM module is not re-entrant. All `wasmFetch()` calls are serialized through a queue, so parallel subresource loads are sequential. This affects page load speed but not correctness.

### Dependency Patches

`@mercuryworkshop/epoxy-transport` v3.0.1 and `@titaniumnetwork-dev/ultraviolet` v3.2.10 have compatibility bugs fixed via `patch-package` (auto-applied on `npm install`):

- epoxy-transport: Headers iteration bug (`for...of` on plain objects) + certificate validation option
- ultraviolet: rawHeaders format mismatch with epoxy-transport

### COEP/COOP Headers

SharedArrayBuffer support requires `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp`. This means no cross-origin popups and third-party iframes need CORP headers.

## File Structure

```
browser-proxy-scraper/
├── README.md
├── notes.md                              # Investigation journal
├── docs/
│   ├── captcha-fingerprint-bypass.md     # Captcha & fingerprint analysis
│   ├── curl-impersonate-wasm.md          # WASM build notes
│   └── e2e-test-notes.md                 # Detailed test logs
├── relay/                                # Blind Wisp relay server
│   ├── package.json
│   ├── tsconfig.json
│   ├── .env.example
│   └── src/
│       ├── index.ts                      # HTTP server + WS upgrade
│       ├── wisp-server.ts                # Wisp protocol handler
│       ├── upstream-proxy.ts             # SOCKS5/HTTP upstream + auth
│       └── config.ts                     # Env-based configuration
└── client/                               # Browser frontend
    ├── package.json
    ├── vite.config.ts                    # Static copy + COEP/COOP + dev proxy
    ├── tsconfig.json
    ├── index.html
    ├── patches/                          # patch-package fixes
    ├── curl-impersonate-wasm/            # WASM build workspace
    │   ├── build.sh                      # Full build script
    │   ├── build-curl.sh                 # curl-only build
    │   ├── link-wasm.sh                  # Final WASM linking
    │   ├── dist/                         # Built artifacts (.js + .wasm)
    │   └── src/
    │       ├── socket_shim.c             # POSIX socket → Wisp bridge
    │       ├── wisp-socket-bridge.js     # Wisp WebSocket transport
    │       └── curl-wasm-fetch.js        # fetch()-like API wrapper
    ├── public/
    │   ├── sw.js                         # SW: UV + origin spoof + captcha bypass + injection
    │   ├── anti-detect.js                # Fingerprint patches (navigator, canvas, WebGL, etc.)
    │   ├── puppet-agent.js               # DOM automation agent
    │   ├── fingerprint-self-test.html    # Local fingerprint verification page
    │   └── connectors/
    │       └── codepen.js                # CodePen GraphQL capture connector
    └── src/
        ├── main.ts                       # Bootstrap: SW → transport → UI
        ├── transport.ts                  # curl-impersonate primary, epoxy fallback
        ├── ui.ts                         # URL input → XOR encode → iframe navigation
        ├── connector-runner.ts           # Executes connector scripts with page API
        ├── page-api.ts                   # Vana-compatible page API
        ├── data-panel.ts                 # Renders intercepted API data
        └── puppet.ts                     # Puppet command bridge to iframe
```
