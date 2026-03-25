# Ultraviolet Web Proxy - Research Report

**Date:** 2026-03-25

---

## 1. What is Ultraviolet?

Ultraviolet is an open-source, client-side web proxy developed by **Titanium Network** for evading internet censorship and accessing websites in a controlled sandbox environment. It operates entirely within the browser using **Service Workers** and JavaScript-based content rewriting, rather than routing all traffic through a remote server like traditional proxies or VPNs.

**Current Status:** Ultraviolet is effectively **deprecated and unmaintained**. It has been officially succeeded by **Scramjet**, a WASM-based proxy from the same team. The Titanium Network documentation explicitly states: "There's no valid reason for including Ultraviolet in your proxy site; it will only lead to unnecessary tech debt."

### Architecture

Ultraviolet's architecture consists of several layers:

1. **Service Worker Layer** (`uv.sw.js`): Intercepts all HTTP requests made within the browser tab. This is the core mechanism that enables transparent proxying without requiring any browser extensions or system-level configuration.

2. **Rewriting Engine** (`uv.bundle.js`, `uv.handler.js`): Transforms HTML, JavaScript, and CSS content so that all URLs, resource references, and dynamic code point back through the proxy rather than directly to the origin server.

3. **Transport Layer** (BareMux): Handles the actual communication between the client-side service worker and the backend server. Supports multiple transport backends:
   - **Bare-Client**: The original HTTP-based transport
   - **EpoxyTransport**: An alternative transport
   - **CurlTransport**: libcurl-based transport
   - **Wisp Protocol**: WebSocket-based protocol for tunneling TCP/UDP connections

4. **Backend Server** (Bare Server / Wisp Server): A server-side component that receives encoded requests from the client, makes the actual HTTP requests to target websites, and returns the responses. This is necessary because browsers cannot make arbitrary cross-origin requests directly.

5. **URL Encoding** (`uv.config.js`): URLs are encoded using one of three codecs (XOR, Base64, or Plain) to obscure what the user is accessing from casual network monitoring.

### Request Flow

```
User enters URL
  -> Service Worker intercepts the request
    -> URL is encoded (xor/base64/plain)
      -> Request sent to BareMux transport
        -> Transport forwards to Bare/Wisp server
          -> Bare/Wisp server fetches from target website
        -> Response returned to transport
      -> Response passed to rewriting engine
    -> HTML/JS/CSS rewritten to proxy all embedded URLs
  -> Rewritten content rendered in browser
```

### Adherence to TompHTTP Specification

Ultraviolet follows the **TompHTTP specification**, which defines standardized behavior for how browser-based proxies should intercept, encode, and relay HTTP requests through service workers.

---

## 2. GitHub Repository Details

| Attribute | Value |
|-----------|-------|
| **Repository** | [titaniumnetwork-dev/Ultraviolet](https://github.com/titaniumnetwork-dev/Ultraviolet) |
| **Stars** | ~791 |
| **Forks** | ~5,700 |
| **Open Issues** | 26 |
| **Language** | JavaScript (100%) |
| **License** | AGPL-3.0 (GitHub) / MIT (package.json) |
| **npm Package** | `@titaniumnetwork-dev/ultraviolet` |
| **Latest npm Version** | 3.2.10 |
| **Latest GitHub Release** | v3.2.7 (August 1, 2024) |
| **Created** | ~2022 |
| **Archived** | No (but effectively unmaintained) |
| **Topics** | nodejs, javascript, typescript, service-worker, proxy, web-proxy, unblocker |

### Key Maintainers / Contributors

- **Titanium Network** (`titaniumnetwork-dev`) - Organization
- **"Duce"** - Primary creator of Ultraviolet (also creator of Incognito proxy)
- **Mercury Workshop** - Development partner organization (`@mercuryworkshop`)

### Version History Highlights

- **v1.0.x** (2022): Initial releases with CommonJS builds, multi-server support
- **v2.0.0**: Required Bare server v3, dropped older server versions
- **v3.0.0**: Major release adding bare-mux transport support (EpoxyTransport, CurlTransport, Bare-Client)
- **v3.2.3**: Significant optimization - removed `css-tree`, `esotope-hammerhead`, and `mime-db` to reduce bundle size; added Safari compatibility
- **v3.2.7**: Optimized XOR and Base64 codecs; bare-mux upgrade enabling remote transports
- **v3.2.10**: Latest version (NPM versioning fix)

---

## 3. Key Features and Capabilities

- **Service Worker-Based Interception**: Operates entirely client-side in the browser without plugins or extensions
- **CAPTCHA/hCAPTCHA Support**: Can handle sites requiring CAPTCHA verification
- **Configurable URL Encoding**: XOR, Base64, or Plain encoding to obscure browsed URLs
- **Multiple Transport Backends**: Switchable via BareMux (Bare-Client, EpoxyTransport, CurlTransport, Wisp)
- **Blacklist Settings**: Hosting operators can block specific sites from being proxied
- **Leak Prevention**: Measures to prevent the real URL from leaking through referrer headers or other channels
- **HTML Injection Support** (v3.2.5+): Configurable HTML injection into proxied pages
- **Broad Site Compatibility**: Works with Google, YouTube, Spotify, Discord, Reddit, GeForce NOW, now.gg

### Production Dependencies

| Dependency | Purpose |
|-----------|---------|
| `@mercuryworkshop/bare-mux` | Transport layer abstraction |
| `meriyah` | JavaScript parser (replaced Acorn.js) |
| `astring` | JavaScript code generator (replaced esotope-hammerhead) |
| `parse5` | HTML parser and serializer |
| `set-cookie-parser` | Cookie handling |
| `idb` | IndexedDB wrapper for client-side state |
| `events` | Node.js-style event emitter |

---

## 4. Known Limitations, Bugs, and Issues

### Active Issues (from 26 open GitHub issues)

**Site Compatibility Failures:**
- Instagram does not work (#173, #186)
- Pinterest broken (#183)
- Reddit functionality issues (#134)
- WhatsApp access problems (#118)
- YouTube blocking (#157)
- Scratch.mit.edu projects (#171)
- Discord login failing (#176)
- Xbox Game Cloud signin broken (#109)

**Technical Issues:**
- WebSocket errors (#168)
- TLS handshake EOF errors (#175)
- EventEmitter memory leaks (#85)
- Tab crashes/unexpected exits (#188, #189)
- Error processing requests (#166)
- hCaptcha loading failures (#128)
- Cloudflare Turnstile verification failures (#121)
- Cloudflare Access breaking connectivity (#74)

**Security:**
- IFrame hijacking vulnerability (#25)

### Fundamental Limitations

1. **Browser-Tab Scope Only**: Only proxies traffic within the specific browser tab; does not protect system-wide traffic
2. **No True Encryption**: Does not provide end-to-end encryption like a VPN; URL encoding is obfuscation, not encryption
3. **Detectable**: Advanced network monitoring can detect header anomalies and service worker patterns
4. **Not Safe for Authentication**: Users should not log into sensitive accounts through the proxy
5. **Hosting Platform Instability**: Frequently banned from platforms like Replit; serverless platforms (Vercel) cannot support WebSocket-dependent features
6. **Unmaintained**: No active development; bugs will not be fixed

---

## 5. Content Rewriting Details

### JavaScript Rewriting

Ultraviolet parses JavaScript using **meriyah** (a fast, spec-compliant JS parser) and regenerates code using **astring** (a fast JS code generator). The rewriting process:

- Intercepts `import` declarations and `dynamic import()` calls to rewrite module URLs
- Rewrites property accesses that could leak the real URL (e.g., `window.location`, `document.URL`)
- Handles `javascript:` protocol URLs
- Transforms script `src` attributes in HTML

Previously used `esotope-hammerhead` (from the TestCafe project) for code generation, but this was replaced with `astring` in v3.2.3 for smaller bundle size.

### CSS Rewriting

CSS rewriting handles:
- `url()` references within stylesheets
- `@import` statements
- Any CSS property that references external resources

Originally used the `css-tree` library for full AST-based CSS parsing and transformation. In **v3.2.3**, `css-tree` was removed and CSS rewriting was simplified (likely to regex-based URL replacement) to reduce bundle size significantly.

### HTML Rewriting

Uses **parse5** to parse HTML into a DOM tree, then rewrites:
- All element attributes containing URLs (`href`, `src`, `action`, `srcset`, etc.)
- `<meta>` refresh tags
- `<base>` elements
- Inline event handlers
- Injects the UV handler script into `<head>` for runtime interception

### iframe Handling

- Proxied content can be loaded inside iframes on the host page
- iframe `src` attributes are rewritten to point through the proxy
- Cross-origin iframe concerns exist (see issue #25 about iframe hijacking)
- The `postMessage` API is used for communication between the iframe and parent window

### WebSocket Handling

- WebSocket connections are proxied through the Wisp protocol or Bare server
- The Wisp protocol tunnels TCP/UDP sockets over a single WebSocket connection
- v3.1.x series included multiple fixes for WebSocket handling
- WebSocket wrapper was refined in v3.2.8
- **Limitation**: WebSockets require a persistent server (cannot work on serverless platforms like Vercel)

---

## 6. Performance Characteristics

### Claims (from third-party source, not independently verified)
- Average response time: ~187ms
- Success rate: ~99.8%
- Bandwidth: ~98.5 Mbps

**Caveat:** These figures come from a single source (Bomberbot) and may not reflect standardized testing conditions. Independent benchmarks are not publicly available.

### Architectural Performance Factors

**Advantages:**
- Client-side rewriting offloads processing from the server
- Service worker caching can reduce redundant requests
- v3.2.3 significantly reduced bundle size by removing heavy dependencies (`css-tree`, `esotope-hammerhead`, `mime-db`)
- XOR and Base64 codecs were optimized in v3.2.7

**Disadvantages:**
- JavaScript AST parsing and regeneration on every script adds latency
- All requests must route through the Bare/Wisp server, adding a network hop
- Complex pages with many resources require extensive rewriting
- EventEmitter memory leak issue (#85) can degrade long-session performance

---

## 7. Detection and Blocking Vulnerabilities

### How Ultraviolet Can Be Detected

1. **HTTP Header Anomalies**: Requests routed through Bare/Wisp servers may have unusual header patterns that deep packet inspection (DPI) can identify
2. **Service Worker Registration Patterns**: The registration of a service worker with specific scope and script patterns can be fingerprinted
3. **URL Pattern Recognition**: Even with XOR/Base64 encoding, the URL structure (`/service/` prefix by default) is recognizable
4. **Traffic Analysis**: All traffic goes to a single backend server rather than diverse origins, which is anomalous
5. **TLS Fingerprinting**: The Bare/Wisp server's TLS characteristics may differ from expected origin servers
6. **WebSocket Detection**: Wisp protocol's WebSocket connections have identifiable patterns
7. **DNS Patterns**: All DNS resolution happens at the Bare/Wisp server, so the client makes no DNS queries for target sites

### Known Blocking Vectors

- **Cloudflare Protections**: Turnstile and Cloudflare Access frequently break through the proxy (issues #74, #121)
- **Platform Bans**: Hosting platforms (Replit, etc.) actively detect and ban UV instances
- **Site-Specific Blocks**: YouTube, Instagram, and other major platforms increasingly detect and block proxy access
- **Content Security Policy (CSP)**: Sites with strict CSP headers can prevent the service worker injection from functioning

### What Ultraviolet Does NOT Protect Against

- Network-level traffic analysis
- ISP logging (the connection to the Bare/Wisp server is visible)
- Browser fingerprinting
- Malicious proxy operators (they can see all traffic)
- Sophisticated web application firewalls (WAFs)

---

## 8. Community and Ecosystem

### Titanium Network

- **Founded**: 2016
- **Discord Members**: ~31,000
- **Monthly Users**: 5+ million (across all services)
- **Partners**: Mercury Workshop (primary development partner)
- **Services**: Proxy dispenser bot, community-maintained proxy links, documentation site

### Projects Built on Ultraviolet

| Project | Description |
|---------|-------------|
| **Holy Unblocker LTS** | The most well-known UV frontend; now also supports Scramjet |
| **Incognito** | Privacy-focused proxy frontend (by UV creator "Duce") |
| **Nebula** | Popular proxy frontend |
| **Metallic** | TypeScript-based proxy frontend (194 stars, 1.2k forks) |
| **AnuraOS** | Web-based OS that uses UV for web browsing |
| **Terbium** | Another proxy frontend |
| **Alu** | Community proxy site |
| **Nano** | Minimalist proxy frontend by Titanium Network |

### Related Ecosystem Technologies

| Technology | Role |
|-----------|------|
| **Scramjet** | UV's successor; WASM-based, actively maintained |
| **Wisp Protocol** | Low-overhead WebSocket protocol for TCP/UDP tunneling |
| **BareMux** | Transport abstraction layer (shared by UV and Scramjet) |
| **EpoxyTransport** | WebSocket-based transport implementation |
| **libcurl-transport** | libcurl-based transport for broader protocol support |
| **Rammerhead** | Alternative proxy engine, often bundled alongside UV |
| **TompHTTP** | Specification that UV implements |

### Scramjet (The Successor)

Scramjet is the actively maintained successor to Ultraviolet, also by Mercury Workshop / Titanium Network. Key differences:

- **WASM-based architecture** instead of pure JS
- **Broader site compatibility** (handles more modern web apps)
- **Better developer experience** and documentation
- **Actively maintained** with CI builds per commit
- **Shares BareMux** transport layer, making migration straightforward
- **Same ecosystem** (Wisp, EpoxyTransport, libcurl-transport)

---

## Sources

- [Ultraviolet GitHub Repository](https://github.com/titaniumnetwork-dev/Ultraviolet)
- [Titanium Network Documentation - Ultraviolet](https://docs.titaniumnetwork.org/proxies/ultraviolet)
- [Titanium Network Documentation - Scramjet](https://docs.titaniumnetwork.org/proxies/scramjet/)
- [Ultraviolet Proxy Guide (RapidSeedbox)](https://www.rapidseedbox.com/blog/ultraviolet-proxy)
- [Ultraviolet Proxy Explained (Pixelscan)](https://pixelscan.net/blog/ultraviolet-proxy/)
- [Comprehensive Guide to Ultraviolet Proxy (Bomberbot)](https://www.bomberbot.com/proxy/unveiling-the-magic-a-comprehensive-guide-to-ultraviolet-proxy/)
- [Ultraviolet Proxy (MomoProxy)](https://momoproxy.com/blog/ultraviolet-proxy)
- [Ultraviolet Proxy Review (ProxyReviewHub)](https://proxyreviewhub.com/ultraviolet-proxy/)
- [Holy Unblocker Documentation](https://docs.titaniumnetwork.org/services/holy-unblocker/)
- [Titanium Network GitHub Organization](https://github.com/titaniumnetwork-dev)
