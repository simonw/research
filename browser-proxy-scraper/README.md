# Browser Proxy Scraper: E2EE In-Browser Web Scraper & Proxy

A system where TLS terminates entirely inside the user's browser via WebAssembly (Epoxy/Rustls), while the backend acts as a blind TCP relay. The user visits our domain, browses target sites through an iframe-based proxy, and we intercept and extract API data (e.g., Instagram GraphQL) — **without the backend ever seeing plaintext traffic or session cookies**.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  Browser                                                         │
│                                                                  │
│  ┌──────────┐    ┌──────────────┐    ┌──────────────────────┐   │
│  │  Main     │    │  UV Service  │    │  Epoxy-TLS (WASM)    │   │
│  │  Page     │───▶│  Worker      │───▶│  Rustls in browser   │   │
│  │  + iframe │    │  + bare-mux  │    │  TLS terminates here │   │
│  └──────────┘    └──────────────┘    └──────────┬───────────┘   │
│                                                  │ encrypted     │
└──────────────────────────────────────────────────┼───────────────┘
                                                   │ WebSocket
                                                   │ (Wisp protocol)
┌──────────────────────────────────────────────────┼───────────────┐
│  Relay Server                                    │               │
│                                                  ▼               │
│  ┌─────────────────┐    ┌────────────────────────────────────┐  │
│  │  Wisp Server     │───▶│  Blind TCP relay                   │  │
│  │  (wisp-js)       │    │  Only sees encrypted bytes         │  │
│  └─────────────────┘    │  Optional: upstream SOCKS5/HTTP    │  │
│                          └──────────────┬─────────────────────┘  │
└─────────────────────────────────────────┼────────────────────────┘
                                          │ TCP
                                          ▼
                                   ┌──────────────┐
                                   │  Target Site  │
                                   │  (e.g., IG)   │
                                   └──────────────┘
```

**Key property**: The relay server never terminates TLS. It only sees encrypted bytes passing through the WebSocket. All plaintext HTTP content exists only in the browser's memory.

## Components

### Relay Server (`relay/`)

Node.js server with a single WebSocket endpoint (`/wisp/`) that implements the Wisp protocol for blind TCP relaying.

- **wisp-js**: Handles the Wisp protocol (multiplexed TCP streams over WebSocket)
- **Upstream proxy**: Optional SOCKS5 or HTTP CONNECT proxy for IP rotation
- **Hostname whitelist**: Restrict which destinations can be reached
- **Stream limits**: Cap concurrent TCP streams per connection

```bash
cd relay
npm install
npm run dev
# Listening on http://localhost:3000
# Wisp endpoint: ws://localhost:3000/wisp/
```

Environment variables:
- `PORT` — Server port (default: 3000)
- `UPSTREAM_PROXY` — SOCKS5 or HTTP proxy URL (e.g., `socks5://user:pass@proxy:1080`)
- `ALLOWED_HOSTS` — Comma-separated hostname whitelist (empty = allow all)
- `MAX_STREAMS` — Max concurrent streams (-1 = unlimited)

### Client (`client/`)

Vite-based frontend hosting an Ultraviolet Service Worker proxy with Epoxy-TLS transport.

- **Ultraviolet**: Service Worker intercepts all proxied requests
- **bare-mux**: Abstracts the transport layer (SharedWorker-based)
- **epoxy-tls**: WASM TLS implementation using Rustls — encrypts/decrypts in browser
- **Traffic interception**: SW hooks capture GraphQL responses via postMessage
- **Puppet agent**: Injected script enables DOM automation in proxied pages

```bash
cd client
npm install
npm run dev
# Dev server on http://localhost:5173
# Proxies /wisp to relay server
```

## How It Works

1. **Service Worker registers** with root scope, loads Ultraviolet proxy engine
2. **Transport initializes**: bare-mux connects to epoxy-tls WASM module, which opens a WebSocket to the relay's `/wisp/` endpoint
3. **User enters URL** → XOR-encoded → loaded in iframe under `/service/` prefix
4. **UV Service Worker** intercepts all iframe requests, routes them through bare-mux → epoxy-tls
5. **Epoxy-TLS (WASM)** performs the actual TLS handshake with the target site — the relay only sees encrypted bytes
6. **Response interception**: SW hooks match GraphQL-pattern URLs, clone responses, and postMessage parsed JSON to the parent page
7. **Puppet agent** (optional): Injected script enables programmatic DOM interaction via postMessage commands

## Traffic Interception

The Service Worker intercepts responses matching these patterns:
- `/api/graphql`
- `graphql.instagram.com`
- `i.instagram.com/api/v1`
- `/api/v1/users`
- `/api/v1/feed`

Intercepted data appears in the right-side panel as parsed JSON.

## Puppet Agent

DOM automation via postMessage bridge:

```js
// From browser console:
puppet.sendCommand('query', { selector: 'h1' })
puppet.sendCommand('click', { selector: '.btn-primary' })
puppet.sendCommand('type', { selector: '#username', text: 'hello' })
puppet.sendCommand('getPageInfo')
puppet.sendCommand('queryAll', { selector: 'a', limit: 10 })
```

Or via the input field at the bottom: `query h1`, `click .btn`, `getPageInfo`, etc.

## Known Limitations

### TLS Fingerprinting

Epoxy-TLS uses **Rustls**, which produces a static, non-browser JA3/JA4 TLS fingerprint. This is detectable by sophisticated anti-bot systems (Cloudflare, Instagram, etc.):

- Rustls's TLS ClientHello differs from Chrome/Firefox
- No way to customize cipher suites or extensions in current Rustls WASM build
- uTLS (Go) can impersonate browser TLS but cannot compile to browser WASM (needs raw TCP sockets)

**Impact**: Sites with TLS fingerprinting (Instagram, most Cloudflare-protected sites) will likely detect and block the proxy.

**Possible upgrade path**: Server-side Go proxy using uTLS for Chrome TLS impersonation. This partially breaks the "blind relay" constraint but solves the fingerprinting problem.

### COEP/COOP Headers

The client requires `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp` headers for SharedArrayBuffer support (used by epoxy WASM). This means:
- The page cannot open popups to other origins
- Third-party iframes need CORP headers to load

### Service Worker Scope

The SW must be registered at root scope (`/`) to intercept all `/service/*` requests. This means only one UV instance per origin.

## File Structure

```
browser-proxy-scraper/
├── notes.md                          # Investigation journal
├── README.md                         # This file
├── relay/                            # Blind Wisp relay server
│   ├── package.json
│   ├── tsconfig.json
│   └── src/
│       ├── index.ts                  # HTTP server + WS upgrade
│       ├── wisp-server.ts            # Wraps wisp-js with config
│       ├── upstream-proxy.ts         # SOCKS5/HTTP upstream + ProxiedTCPSocket
│       └── config.ts                 # Env-based configuration
└── client/                           # Browser frontend
    ├── package.json
    ├── vite.config.ts                # Static copy + COEP/COOP + UV config override
    ├── tsconfig.json
    ├── index.html                    # Main page with URL input, iframe, data panel
    ├── public/
    │   ├── sw.js                     # Custom SW: UV + response interception hooks
    │   └── puppet-agent.js           # DOM automation agent (injected into proxied pages)
    └── src/
        ├── main.ts                   # Bootstrap: SW → transport → UI
        ├── transport.ts              # bare-mux + epoxy initialization
        ├── ui.ts                     # URL input → XOR encode → iframe navigation
        ├── data-panel.ts             # Renders intercepted API data
        └── puppet.ts                 # Puppet command bridge to iframe
```

## Security Model

| Layer | What it sees |
|-------|-------------|
| Browser page | Plaintext HTTP content (same-origin via SW) |
| Service Worker | Decoded URLs, request/response bodies |
| Epoxy-TLS WASM | Plaintext (performs TLS handshake) |
| WebSocket frames | Encrypted TLS records only |
| Relay server | Encrypted bytes + destination host:port |
| Network observer | WebSocket frames containing encrypted data |

The relay knows *which* hosts the user connects to (from Wisp CONNECT packets) but never sees the plaintext content, cookies, or authentication tokens.
