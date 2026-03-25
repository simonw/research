# Web Proxies Research - Notes

## Research Started: 2026-03-25

### Objective
Compare popular browser-based web proxies (Ultraviolet, Scramjet, others) and do deep research on what web proxies are, how they compare to traditional proxies, their pitfalls and limitations, and use cases.

### Research Approach
- Launched 4 parallel research agents:
  1. Ultraviolet web proxy - COMPLETED
  2. Scramjet web proxy - COMPLETED
  3. Other notable web proxies - COMPLETED
  4. Web proxy concepts in general - TIMED OUT (covered manually)

---

## Key Findings

### Ultraviolet
- **Status:** Deprecated in favor of Scramjet
- **GitHub:** ~791 stars, ~5,700 forks, AGPL-3.0
- **Architecture:** Service Worker intercepts requests → URL encoded (XOR/Base64/Plain) → BareMux transport → Bare/Wisp backend server → fetches target site → response rewritten client-side
- **JS Rewriting:** Uses `meriyah` parser + `astring` code generator (full AST transformation)
- **HTML:** Parsed with `parse5`, rewrites all URL-bearing attributes
- **CSS:** Was `css-tree` (removed in v3.2.3), now simplified
- **WebSockets:** Tunneled via Wisp protocol
- **Limitations:** Browser-tab only, detectable by DPI/headers/SW fingerprinting, Cloudflare breaks it, many major sites increasingly fail, unmaintained since Aug 2024
- **Ecosystem:** Titanium Network (31K Discord, 5M+ monthly users)

### Scramjet
- **Status:** Active, current flagship proxy of Titanium Network ecosystem
- **GitHub:** 367 stars, 578 forks, AGPL-3.0, TypeScript (63%) + Rust (22%)
- **Key Innovation:** Rust-to-WASM compiled rewriter using "Byte Span Rewrite" (direct text insertion at byte offsets) instead of full AST transformation
- **Performance:** 2-3x faster HTML rewriting than htmlparser2
- **JS:** Byte Span Rewriting for location objects, eval, destructuring, Reflect API
- **CSS:** Simple RegExp rewriting (only @import needs handling)
- **WebSockets:** Via Wisp protocol (TCP/UDP over WebSocket)
- **Limitations:** No AST plugin support, Google sign-in broken (BotGuard), mobile compatibility issues, requires non-datacenter IPs for CAPTCHAs/YouTube
- **Community:** Mercury Workshop (split from TN in Summer 2025)

### Other Notable Proxies

**Rammerhead** (298 stars, 2,416 forks)
- Server-side proxy built on testcafe-hammerhead
- Unique: persistent session management with cross-device localStorage/cookie sync
- Development slowed since late 2022, still in use (Holy Unblocker)

**Corrosion** (93 stars, 961 forks, MIT) - ARCHIVED March 2022
- Predecessor to Ultraviolet, successor to Alloy
- Featured hCAPTCHA support, URL encoding codecs, middleware system

**Alloy** (101 stars, 1,000 forks) - ARCHIVED July 2021
- First-generation TN proxy, node-fetch server-side + client-side JS monkey-patching

**Womginx** (184 stars, 814 forks, AGPL-3.0)
- Unique: nginx (server-side) + Wombat (client-side URL rewriting from Webrecorder project)
- Only proxy not built on Node.js, very fast
- Cannot handle React sites or YouTube UI

**Interstellar** (1,948 stars, 22,938 forks)
- Most popular proxy frontend, wraps UV/Scramjet

**Holy Unblocker** (1,317 stars)
- Flagship TN site, integrates Scramjet + UV + Rammerhead, supports Tor

**Node-Unblocker** (509 stars)
- Oldest active project (since 2011), Express-based streaming proxy

**Helios** (292 stars)
- Purely client-side HTML/CSS/JS proxy, no backend needed, can run as offline HTML file

**Adrift** (49 stars)
- Decentralized P2P proxy over WebRTC (like "Tor Snowflake for the web")

### TN Proxy Lineage
Alloy (2020-2021) → Corrosion (2021-2022) → Ultraviolet (2022-2024) → Scramjet (2024-present)

### Stomp & Oxide
- **Stomp:** Could not be found as a browser-based web proxy (only STOMP messaging protocol results)
- **Oxide:** Not a proxy - it's the Titanium Network website/SDK documentation site

---

## Web Proxy Concepts (Manual Research)

### What Are Browser-Based Web Proxies?
Browser-based web proxies (also called "web unblockers" or "client-side web proxies") are systems that allow users to browse the web through an intermediary, entirely within a browser tab. Unlike traditional proxies that operate at the network level, these work by:
1. Intercepting browser requests via Service Workers
2. Rewriting URLs, HTML, JavaScript, and CSS to route through a proxy server
3. Returning the modified content so it renders correctly in the user's browser

### How They Differ from Traditional Proxies

| Feature | Web Proxy | HTTP/HTTPS Forward Proxy | SOCKS Proxy | VPN |
|---------|-----------|-------------------------|-------------|-----|
| Scope | Single browser tab | System/app-wide | System/app-wide | System-wide |
| Installation | None (visit a URL) | Configure OS/browser | Configure OS/app | Install client |
| Encryption | TLS to proxy server | TLS to proxy | Optional | Full tunnel |
| Protocol support | HTTP/HTTPS only | HTTP/HTTPS | Any TCP/UDP | All traffic |
| Performance | Higher overhead (rewriting) | Lower overhead | Low overhead | Moderate overhead |
| Detection difficulty | Easier to detect | Moderate | Harder | Hardest |
| IP masking | Yes (server IP) | Yes | Yes | Yes |

### Key Technical Challenges
1. **JavaScript rewriting** - The hardest problem. Must intercept `window.location`, `document.URL`, `fetch()`, `XMLHttpRequest`, dynamic imports, `eval()`, `new Function()`, property accessors via Proxy/Reflect
2. **Dynamic content / SPAs** - React, Angular, Vue apps constantly manipulate the DOM; proxy must handle dynamically injected URLs
3. **Cookie/session management** - Cookies are domain-scoped; proxy must map cookies across origins
4. **CSP handling** - Content Security Policy headers can block proxy's injected scripts
5. **WebSocket proxying** - Requires persistent connections; solved via Wisp protocol in TN ecosystem
6. **Service Worker conflicts** - Target sites may register their own service workers that conflict with the proxy's

### Why Use Web Proxies Over Regular Proxies/VPNs?
1. **Zero installation** - Just visit a URL, works on locked-down devices (school/work computers)
2. **No admin privileges needed** - No software to install
3. **Device agnostic** - Works on any device with a modern browser
4. **Harder to block by software** - No identifiable client software to detect
5. **Shareable** - Can share a link, no configuration needed

### Pitfalls and Limitations
1. **Security concerns** - MITM by design; proxy operator can see all traffic
2. **Site detection** - Sophisticated sites can detect proxies via JS fingerprinting, header analysis, TLS fingerprinting
3. **Performance overhead** - All content must be parsed and rewritten
4. **JavaScript-heavy sites breaking** - Complex SPAs, Google services, sites with bot protection
5. **Authentication issues** - OAuth flows, SSO, multi-domain auth often break
6. **Fingerprinting** - Service worker patterns, URL structures, header anomalies reveal proxy use
7. **Not system-wide** - Only protects traffic in that browser tab
8. **Cloudflare/bot protection** - Major blocking systems increasingly detect and block web proxies
