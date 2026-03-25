# Browser-Based Web Proxies: A Comprehensive Research Report

## Table of Contents
1. [What Are Browser-Based Web Proxies?](#what-are-browser-based-web-proxies)
2. [How They Work](#how-they-work)
3. [Web Proxies vs. Traditional Proxies](#web-proxies-vs-traditional-proxies)
4. [Major Web Proxy Projects](#major-web-proxy-projects)
5. [Head-to-Head Comparison](#head-to-head-comparison)
6. [Technical Deep Dive](#technical-deep-dive)
7. [Pitfalls and Limitations](#pitfalls-and-limitations)
8. [When to Use Web Proxies](#when-to-use-web-proxies)
9. [The Titanium Network Ecosystem](#the-titanium-network-ecosystem)

---

## What Are Browser-Based Web Proxies?

Browser-based web proxies (also called "web unblockers" or "client-side web proxies") are systems that allow users to browse the web through an intermediary **entirely within a browser tab**. Unlike traditional proxies that operate at the network/OS level, these work by:

1. **Intercepting** browser requests via Service Workers or server-side middleware
2. **Rewriting** URLs, HTML, JavaScript, and CSS to route through a proxy server
3. **Returning** modified content so it renders correctly in the user's browser

The user simply visits a URL, types in a target website, and browses as if they were visiting the site directly — but all traffic flows through the proxy server.

### Key Characteristics
- **Zero installation** — works in any modern browser
- **No admin privileges needed** — nothing to install on locked-down devices
- **Tab-scoped** — only proxies traffic within that browser tab
- **MITM by design** — the proxy server sees and rewrites all traffic

---

## How They Work

### Service Worker Architecture (Modern Approach)

Most modern web proxies (Ultraviolet, Scramjet) use the following architecture:

```
User's Browser                          Proxy Backend
┌─────────────────────────┐            ┌──────────────┐
│  Browser Tab             │            │              │
│  ┌───────────────────┐   │            │  Bare/Wisp   │
│  │  Rendered Page     │   │            │  Server      │
│  │  (rewritten HTML)  │   │            │              │
│  └───────┬───────────┘   │            │  Fetches     │
│          │ request        │            │  target site │
│  ┌───────▼───────────┐   │            │  on behalf   │
│  │  Service Worker    │   │  encoded   │  of user     │
│  │  (intercepts all   │───┼──request──►│              │
│  │   fetch requests)  │◄──┼──response──│              │
│  │                    │   │  rewritten │              │
│  └───────────────────┘   │            └──────────────┘
└─────────────────────────┘
```

1. **Service Worker Registration** — The proxy registers a Service Worker that intercepts all network requests from the page
2. **URL Encoding** — Target URLs are encoded (XOR, Base64, or plain) to disguise them
3. **Transport Layer** — Requests are sent to a backend (Bare or Wisp server) via BareMux
4. **Server-Side Fetch** — The backend fetches the target site and returns the raw response
5. **Client-Side Rewriting** — HTML, JS, CSS, and headers are rewritten to replace all URLs with proxied versions
6. **Rendering** — The rewritten content is rendered in the browser tab

### Server-Side Architecture (Older Approach)

Older proxies (Rammerhead, Node-Unblocker, Womginx) do rewriting on the server:

1. User requests a URL through the proxy
2. Server fetches the target page
3. Server rewrites all content (URLs, scripts, styles)
4. Server sends the fully-rewritten page to the browser
5. Injected client-side scripts handle dynamic URL mutations

---

## Web Proxies vs. Traditional Proxies

| Feature | Browser Web Proxy | HTTP/HTTPS Forward Proxy | SOCKS Proxy | VPN | Reverse Proxy |
|---|---|---|---|---|---|
| **Scope** | Single browser tab | System or app-wide | System or app-wide | All system traffic | Server-side only |
| **Installation** | None (visit a URL) | Configure OS/browser settings | Configure OS/app | Install client app | Server config (nginx, etc.) |
| **User-facing?** | Yes | Yes | Yes | Yes | No (transparent to user) |
| **Encryption** | TLS to proxy server only | TLS to proxy (CONNECT) | Optional | Full encrypted tunnel | N/A |
| **Protocol support** | HTTP/HTTPS only | HTTP/HTTPS | Any TCP (SOCKS5: +UDP) | All IP traffic | HTTP/HTTPS |
| **Performance overhead** | High (content rewriting) | Low (passthrough) | Low | Moderate (encryption) | Low |
| **Detection difficulty** | Easiest to detect | Moderate | Harder | Hardest | N/A |
| **IP masking** | Yes (server's IP) | Yes | Yes | Yes | N/A |
| **Admin access needed?** | No | Usually | Usually | Yes | N/A |
| **Works on locked devices?** | Yes | Rarely | Rarely | No | N/A |

### Why Use a Web Proxy Over a VPN/SOCKS/Forward Proxy?

1. **Zero-install access** — The killer feature. On school Chromebooks, work laptops, library computers, or any device where you can't install software, a web proxy just works in the browser.
2. **No configuration** — No proxy settings to change, no client to install, no credentials to enter.
3. **Shareable** — You can share a link. No setup instructions needed.
4. **Harder to block by endpoint software** — There's no identifiable client binary running. It's just a website.
5. **Ephemeral** — Close the tab and it's gone. No traces left on the device (beyond normal browser history).

### Why NOT Use a Web Proxy?

1. **Not system-wide** — Only protects traffic in that one tab
2. **Breakage** — Complex sites frequently break due to incomplete JS/CSS rewriting
3. **Security risk** — The proxy operator can see ALL your traffic (passwords, tokens, everything)
4. **Detectable** — Easy for networks to identify and block the proxy domain
5. **Performance** — Content rewriting adds significant latency
6. **Not suitable for privacy** — Fingerprinting, header anomalies, and traffic patterns expose proxy use

---

## Major Web Proxy Projects

### Scramjet (Current Leader)
| | |
|---|---|
| **GitHub** | [MercuryWorkshop/scramjet](https://github.com/MercuryWorkshop/scramjet) |
| **Stars** | ~367 |
| **Forks** | ~578 |
| **License** | AGPL-3.0 |
| **Language** | TypeScript (63%), Rust (22%) |
| **Status** | **Active** — current flagship proxy |
| **Maintainer** | Mercury Workshop (split from Titanium Network, Summer 2025) |

**Architecture:** Service Worker + WASM-compiled Rust rewriter. Uses "Byte Span Rewrite" — directly inserting replacement text at byte offsets rather than building and regenerating a full AST. This is significantly faster but sacrifices AST plugin extensibility.

**Key Strengths:**
- 2-3x faster HTML rewriting than Ultraviolet's htmlparser2
- WASM-powered rewriting engine (Rust compiled to WebAssembly)
- Active development with regular updates
- Handles most modern websites well
- Wisp protocol for WebSocket/TCP/UDP tunneling

**Key Weaknesses:**
- No AST plugin support (trade-off of Byte Span approach)
- Google sign-in broken (BotGuard detection)
- Mobile compatibility issues
- Requires non-datacenter IPs for CAPTCHAs/YouTube
- 21+ open site-specific issues

---

### Ultraviolet (Deprecated)
| | |
|---|---|
| **GitHub** | [titaniumnetwork-dev/Ultraviolet](https://github.com/titaniumnetwork-dev/Ultraviolet) |
| **Stars** | ~791 |
| **Forks** | ~5,700 |
| **License** | AGPL-3.0 |
| **Language** | JavaScript |
| **Status** | **Deprecated** — last release Aug 2024 |
| **Maintainer** | Titanium Network |

**Architecture:** Service Worker + full AST-based JS rewriting. Uses `meriyah` to parse JavaScript into an AST, transforms it, and regenerates with `astring`. HTML parsed with `parse5`.

**Key Strengths:**
- Mature and well-understood codebase
- Large ecosystem of frontends (Holy Unblocker, Nebula, Metallic, etc.)
- AST-based rewriting allows plugins/extensibility
- Massive community (5,700+ forks)

**Key Weaknesses:**
- Unmaintained since August 2024
- Slower than Scramjet (full AST parse/regenerate cycle)
- Increasingly broken on major sites (Instagram, YouTube, WhatsApp)
- EventEmitter memory leaks
- Detectable via service worker fingerprinting and header analysis
- Cloudflare protections break through it

---

### Rammerhead
| | |
|---|---|
| **GitHub** | [binary-person/rammerhead](https://github.com/binary-person/rammerhead) |
| **Stars** | ~298 |
| **Forks** | ~2,416 |
| **License** | — |
| **Language** | JavaScript (Node.js) |
| **Status** | **Low activity** — slowed since late 2022 |
| **Maintainer** | binary-person |

**Architecture:** Server-side proxy built on [testcafe-hammerhead](https://github.com/nicholasgasior/testcafe-hammerhead) (a browser automation testing tool's proxy engine). All rewriting happens server-side.

**Key Strengths:**
- Persistent session management with cross-device localStorage/cookie sync
- Server-side rewriting means less client-side breakage
- Integrated into Holy Unblocker
- Sessions survive page refreshes and device switches

**Key Weaknesses:**
- Higher server resource usage (all rewriting on server)
- Development has largely stalled
- Depends on testcafe-hammerhead (which has its own limitations)

---

### Womginx
| | |
|---|---|
| **GitHub** | [binary-person/womginx](https://github.com/binary-person/womginx) |
| **Stars** | ~184 |
| **Forks** | ~814 |
| **License** | AGPL-3.0 |
| **Language** | nginx config + JavaScript |
| **Status** | **Maintained** |
| **Maintainer** | binary-person |

**Architecture:** Unique hybrid — uses **nginx** for server-side proxying and URL rewriting, combined with **Wombat** (from the Webrecorder/web archiving project) for client-side URL rewriting. The only major proxy NOT built on Node.js.

**Key Strengths:**
- Very fast (nginx is highly optimized for proxying)
- Lightweight — no Node.js runtime needed
- Leverages battle-tested nginx reverse proxy capabilities

**Key Weaknesses:**
- Cannot handle React/SPA sites
- YouTube UI doesn't work
- Limited JavaScript rewriting compared to AST-based approaches

---

### Node-Unblocker
| | |
|---|---|
| **GitHub** | [nfriedly/node-unblocker](https://github.com/nfriedly/node-unblocker) |
| **Stars** | ~509 |
| **Status** | **Active** (since 2011 — oldest in the ecosystem) |

Express.js middleware-based streaming proxy. Simple, reliable, but limited JS rewriting. Best for simple sites. The grandfather of the Node.js web proxy scene.

---

### Helios
| | |
|---|---|
| **GitHub** | ~292 stars |
| **Status** | Active |

**Purely client-side** HTML/CSS/JS proxy. No backend server needed — can even run as a standalone offline HTML file. Extremely lightweight but limited in what sites it can handle.

---

### Adrift
| | |
|---|---|
| **GitHub** | ~49 stars |
| **Status** | Experimental |

**Decentralized P2P proxy over WebRTC.** Similar in concept to Tor's Snowflake — volunteer nodes relay traffic for other users. Novel architecture but small and experimental.

---

## Head-to-Head Comparison

| Feature | Scramjet | Ultraviolet | Rammerhead | Womginx | Node-Unblocker |
|---|---|---|---|---|---|
| **Architecture** | SW + WASM | SW + AST | Server-side | nginx + Wombat | Express middleware |
| **JS Rewriting** | Byte Span (WASM) | Full AST (meriyah) | testcafe-hammerhead | Wombat (limited) | Basic |
| **HTML Rewriting** | WASM parser | parse5 | Server-side | nginx sub_filter | Streaming |
| **Speed** | Fastest | Moderate | Moderate | Fast | Fast |
| **Site Compatibility** | Best | Good (degrading) | Good | Poor (no React) | Poor (simple sites) |
| **WebSocket Support** | Wisp protocol | Wisp protocol | Native | Limited | No |
| **Plugin System** | No | Yes (AST) | No | No | Yes (Express middleware) |
| **Server Load** | Low (client rewrite) | Low (client rewrite) | High (server rewrite) | Moderate | High |
| **Active Development** | Yes | No (deprecated) | Minimal | Minimal | Minimal |
| **Google Sign-in** | Broken | Broken | Partial | Broken | Broken |
| **YouTube** | Works (non-DC IPs) | Degraded | Partial | Broken | No |
| **Mobile Support** | Issues | Better | Good | Limited | Good |

---

## Technical Deep Dive

### The Hardest Problem: JavaScript Rewriting

The core challenge of any web proxy is rewriting JavaScript so that all URL references, location accesses, and network requests go through the proxy. This is extraordinarily difficult because:

**What must be intercepted:**
- `window.location` (read and write — href, origin, hostname, pathname, etc.)
- `document.URL`, `document.referrer`, `document.domain`
- `fetch()`, `XMLHttpRequest.open()`
- `<a>.href`, `<img>.src`, `<script>.src` (dynamically created)
- `window.open()`, `history.pushState()`, `history.replaceState()`
- `eval()`, `new Function()` (must rewrite the string argument)
- `import()` dynamic imports
- `Reflect.get()`, `Reflect.set()` on location-like objects
- `Object.defineProperty()` on window/document
- `postMessage()` origin checks
- Web Workers and Shared Workers
- `document.cookie` get/set

**Approaches:**

| Approach | Used By | Pros | Cons |
|---|---|---|---|
| **Full AST Parse + Transform** | Ultraviolet (meriyah + astring) | Accurate, extensible, plugin support | Slow (parse → transform → regenerate), large bundles |
| **Byte Span Rewrite** | Scramjet (Rust/WASM) | 2-3x faster, low overhead | No plugin support, less flexible |
| **Monkey-patching** | Alloy, older proxies | Simple to implement | Easily bypassed, misses static references |
| **Server-side rewrite** | Rammerhead, Womginx | No client overhead | Higher server load, misses dynamic JS |

### URL Encoding Schemes

Web proxies encode target URLs to disguise them. Common schemes:

- **XOR encoding** — XOR each character with a key. Fast, reversible, obscures URLs from casual inspection
- **Base64** — Standard Base64 encoding. Easy to decode but widely supported
- **Plain/None** — URL passed as-is (simplest but most detectable)
- **Custom codecs** — Some proxies support custom encoding functions

Example: `https://google.com` might become `/service/hvklu~8--iqqing.eqo/` (XOR) or `/service/aHR0cHM6Ly9nb29nbGUuY29t/` (Base64)

### The Wisp Protocol

Modern TN proxies use the **Wisp protocol** for transport — a multiplexed protocol that tunnels TCP and UDP connections over a single WebSocket. This allows:
- WebSocket proxying
- Raw TCP connections (for protocols beyond HTTP)
- UDP support (for DNS, gaming, etc.)
- Connection multiplexing (multiple streams over one WebSocket)

### Content Security Policy (CSP) Handling

CSP headers can block proxy-injected scripts. Proxies must:
1. Strip or modify `Content-Security-Policy` headers from responses
2. Add the proxy's origin to allowed sources
3. Handle `nonce` and `hash` based CSP directives
4. Deal with `<meta http-equiv="Content-Security-Policy">` tags in HTML

---

## Pitfalls and Limitations

### 1. Security — MITM by Design
The proxy server **sees all traffic in plaintext**. This means:
- Passwords, tokens, session cookies are visible to the operator
- The operator could inject malicious scripts
- There's no way for the user to verify the proxy isn't tampering with content
- **Never use a web proxy for sensitive activities** (banking, email, etc.)

### 2. Detection and Blocking
Web proxies are detectable through multiple vectors:

| Detection Method | How It Works |
|---|---|
| **Domain blocking** | Simply block the proxy's domain/IP |
| **Service Worker fingerprinting** | Detect unusual SW registration patterns |
| **URL structure analysis** | Encoded URLs have recognizable patterns |
| **Header anomalies** | Missing or extra headers reveal proxy use |
| **TLS fingerprinting (JA3/JA4)** | Backend server's TLS fingerprint differs from a real browser |
| **DNS query absence** | Client never makes DNS queries for target domains |
| **JavaScript checks** | `window.location` vs actual URL discrepancies |
| **Timing analysis** | Added latency from rewriting is measurable |

### 3. Site Compatibility Issues
- **Google Services** — BotGuard/reCAPTCHA detection breaks sign-in
- **Cloudflare-protected sites** — Turnstile challenges detect proxy
- **OAuth/SSO flows** — Multi-domain redirects break URL rewriting
- **SPAs (React/Angular/Vue)** — Dynamic DOM manipulation may bypass rewriting
- **WebRTC** — Can leak real IP addresses
- **Service Worker conflicts** — Target sites' own SWs conflict with proxy SW

### 4. Performance Overhead
- Every page load requires parsing and rewriting HTML, JS, and CSS
- Full AST parsing (Ultraviolet) adds 50-200ms per script
- WASM byte-span (Scramjet) reduces this but still adds overhead
- Images and media must be proxied through the backend server
- WebSocket connections add latency through the Wisp tunnel

### 5. Legal and Ethical Considerations
- Proxy operators may be liable for content accessed through their service
- Circumventing network restrictions may violate terms of service
- AGPL-3.0 licensing (most TN proxies) requires sharing source code of modifications
- Proxy could be used to bypass content filters, access restricted content

---

## When to Use Web Proxies

### Good Use Cases
- **Locked-down devices** where you can't install software (school/library computers)
- **Quick, temporary access** to a blocked site without setting up a VPN
- **Testing** how a site looks from a different origin
- **Web archiving** and research
- **Educational purposes** — understanding web security, proxy technology

### Bad Use Cases
- **Anything requiring security** — banking, email, sensitive accounts
- **Persistent daily use** — VPN or SOCKS proxy is more reliable
- **Streaming** — performance overhead makes video quality poor
- **Privacy-critical browsing** — too many detection/fingerprinting vectors
- **Mobile use** — compatibility is inconsistent

---

## The Titanium Network Ecosystem

The dominant ecosystem for browser-based web proxies is **Titanium Network (TN)**, a community of developers and users focused on web freedom tools.

### Evolution Timeline

```
2020-2021: Alloy (first-gen, basic monkey-patching)
    │
    ▼
2021-2022: Corrosion (middleware system, codec support)
    │
    ▼
2022-2024: Ultraviolet (Service Worker + full AST rewriting)
    │
    ▼
2024-present: Scramjet (WASM + Byte Span rewriting)
```

### Ecosystem Components

| Component | Purpose |
|---|---|
| **Scramjet / Ultraviolet** | Core proxy engines |
| **Bare Server** | Backend that fetches target sites (older protocol) |
| **Wisp Server** | Modern backend with multiplexed TCP/UDP over WebSocket |
| **BareMux** | Transport abstraction layer |
| **EpoxyTransport** | WASM-based transport implementation |
| **Libcurl Transport** | Alternative transport using libcurl compiled to WASM |

### Popular Frontends

| Frontend | Stars | Description |
|---|---|---|
| **Interstellar** | ~1,948 | Most popular proxy frontend, wraps UV/Scramjet |
| **Holy Unblocker** | ~1,317 | Flagship TN site, integrates Scramjet + UV + Rammerhead + Tor |
| **Nebula** | — | Clean UI, UV-based |
| **Metallic** | — | Minimalist UV frontend |
| **AnuraOS** | — | Full web-based OS that includes UV |

### Community Scale
- **33,000+ Discord members** in Titanium Network
- **5M+ monthly users** across TN-based services
- Mercury Workshop split from TN in Summer 2025 but continues maintaining Scramjet

---

## Conclusion

Browser-based web proxies occupy a unique niche: **zero-install web access on locked-down devices**. They trade security, performance, and reliability for the convenience of working entirely within a browser tab.

**Scramjet** is the current state-of-the-art, with its WASM-powered Byte Span rewriting offering the best combination of speed and site compatibility. **Ultraviolet** remains widely deployed but is deprecated and increasingly broken. **Rammerhead** offers the best session persistence. **Womginx** is the fastest for simple sites but can't handle modern SPAs.

The fundamental tension in web proxy design is between **completeness of rewriting** (catching every URL reference in JavaScript) and **performance** (not adding seconds of latency to every page load). Scramjet's Byte Span approach currently represents the best balance, but the ongoing arms race with bot detection systems (Cloudflare Turnstile, Google BotGuard) means no web proxy can guarantee reliable access to all sites.

For serious privacy or security needs, a VPN or SOCKS proxy remains the better choice. Web proxies are best suited for casual, temporary, non-sensitive browsing on devices where other solutions aren't available.
