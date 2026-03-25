# Research Notes: Browser-Based Web Proxies

## Research Process

### Searches Conducted
1. "browser-based web proxy how it works technical architecture" - covered general proxy concepts, CGI proxies, proxy browsers
2. "client-side web proxy service worker URL rewriting JavaScript" - found AKARI proxy, service worker interception patterns, rewriting-proxy on GitHub
3. "web unblocker proxy vs VPN vs SOCKS proxy differences" - good comparison data
4. "web proxy technical challenges CSP cookies SPA dynamic content" - CSP/SPA conflicts, BFF pattern
5. "Ultraviolet proxy Stomp service worker web proxy open source" - Titanium Network ecosystem, Ultraviolet architecture
6. "web proxy security risks MITM fingerprinting detection bypass" - Cloudflare MITMEngine, TLS fingerprinting, JA3
7. "web proxy legal issues circumventing content restrictions legality" - legality varies by country, ToS vs criminal law distinction
8. "CGI proxy vs service worker proxy architecture comparison" - clear architectural differences
9. "Scramjet proxy Titanium Network web proxy evolution history" - evolution from Alloy -> Corrosion -> UV -> Scramjet
10. "web proxy URL rewriting challenges JavaScript dynamic imports fetch XMLHttpRequest" - JS Proxy object interception, double encoding issues
11. "web proxy WebSocket proxying cookie jar cross-origin session management" - CSWSH, cookie-based auth in WebSockets
12. "sites detect block web proxy browser fingerprinting canvas WebGL" - canvas/WebGL fingerprinting, anti-detect browsers
13. "web proxy performance overhead latency JavaScript rewriting benchmark" - ES6 Proxy 10x-700x slower in microbenchmarks, ~7% in real apps
14. "Alloy Corrosion Womginx web proxy history open source CGI proxy PHProxy Glype" - history from PHProxy/Glype/CGIProxy to modern proxies
15. "browser extension proxy vs web proxy comparison architecture" - extension uses proxy API, web proxy is sessionless
16. "web proxy iframe injection technique server-side rewriting architecture pattern" - PHP proxy pattern, Bee Sting, fetch-robot
17. "web proxy use cases school network bypass censorship circumvention privacy" - school filter bypassing, EFF circumvention guide
18. "web proxy authentication login issues HTTPS SSL inspection man in the middle design" - Charles Proxy, mitmproxy, enterprise SSL inspection

### Pages Fetched
- GitHub: titaniumnetwork-dev/Ultraviolet - got architecture overview, bare-mux transport layer
- docs.titaniumnetwork.org/proxies/scramjet/ - Scramjet is UV successor, simpler setup, scramjet.all.js bundle
- deepwiki.com AKARI-Proxy content-rewriting - detailed URL rewriting breakdown (HTML, CSS, JS, MutationObserver, SW fallback, CSP stripping)
- arxiv.org BikiniProxy paper - PDF wasn't readable, but search results gave enough detail

## Key Findings

### Historical Evolution of Browser-Based Web Proxies
1. **CGI era (early 2000s)**: PHProxy (PHP), CGIProxy (Perl), Glype (PHP) - server-side URL rewriting, form-based input
2. **Node.js era (2018-2022)**: Alloy -> Corrosion -> Ultraviolet (Titanium Network)
3. **Service Worker era (2022-present)**: Ultraviolet v2+ -> Scramjet (Mercury Workshop)
4. Scale: Titanium Network claims 5M+ monthly users

### Three Main Architecture Patterns
1. **Server-side rewriting (CGI proxy)**: Server fetches page, rewrites all URLs, serves modified HTML. Simple but breaks JS-heavy sites.
2. **Service Worker interception**: Register SW, intercept all fetch events, rewrite URLs client-side, forward through bare/wisp server. Most modern approach.
3. **iframe injection / embedding**: Load proxied content in iframe, strip X-Frame-Options/CSP, use postMessage for communication.

### URL Rewriting is THE Core Challenge
- Must rewrite: href, src, action, srcset in HTML; url() in CSS; fetch(), XMLHttpRequest, import(), dynamic script creation in JS
- MutationObserver needed for dynamically added DOM elements
- Complex dynamic URL construction (string concatenation) cannot be statically rewritten - requires runtime interception
- Double encoding problem when URLs pass through proxy

### CSP is Stripped by Design
- Web proxies rewrite all URLs to the proxy's origin
- CSP policies would block these rewritten URLs
- Solution: strip CSP headers entirely, which removes a critical security layer

### Performance
- ES6 Proxy objects: 10x-700x slower than direct property access in microbenchmarks
- V8 optimizations improved this 49-74% but still significant overhead
- In practice with I/O: overhead is ~7% or less (network latency dominates)
- BikiniProxy approach: local modifications limit overhead since no centralized server needed

### Detection Methods
- TLS fingerprinting (JA3): mismatch between User-Agent and actual TLS handshake parameters
- Canvas/WebGL fingerprinting: hardware-level rendering differences hard to spoof
- Cloudflare Bot Fight Mode specifically detects proxy TLS mismatches
- Machine learning models recognize bot/proxy patterns

### Legal Landscape
- Generally legal in US, EU, Japan, most of South America
- Restricted/banned in China, Russia, Iran, UAE, North Korea
- ToS violations (Netflix geo-bypass) != criminal offenses
- CFAA: circumventing access controls via proxy may violate federal law (Judge Breyer ruling)

## Additional Research: Other Notable Browser-Based Web Proxy Projects (2026-03-25)

### Stomp
- Could NOT find a browser-based web proxy called "Stomp" anywhere on GitHub or the web
- All "Stomp" results relate to the STOMP messaging protocol (Simple Text Oriented Messaging Protocol), which is completely unrelated
- Searched multiple variations: "Stomp web proxy", "Stomp proxy unblocker", "Stomp.js web proxy", site:github.com searches, titaniumnetwork-related searches
- Possible that the name is incorrect, the project was removed/renamed, or it is extremely obscure/private

### Rammerhead - github.com/binary-person/rammerhead
- 298 stars, 2,416 forks, JavaScript
- Built on testcafe-hammerhead engine (from DevExpress TestCafe testing framework)
- Key innovation: persistent session management with cross-device sync of localStorage/cookies
- Session-based architecture allows custom HTTP proxy configuration per session
- Same author as Womginx (binary-person)
- Demo at demo-opensource.rammerhead.org, browser version at browser.rammerhead.org
- Default password: "sharkie4life"
- Requires Node v16+
- Last release v1.2.62 December 2022 - development slowed but repo still active
- Compatible with most sites except Google logins

### Corrosion - github.com/titaniumnetwork-dev/Corrosion
- 93 stars, 961 forks, MIT license, JavaScript
- ARCHIVED March 8, 2022
- Was the official Titanium Network proxy, succeeded Alloy, was itself succeeded by Ultraviolet
- Features: hCAPTCHA support, URL encoding codecs (base64, plain, xor), middleware system
- Express integration, configurable URL prefix, WebSocket rewriting
- Includes middleware for IP handling and hostname blacklisting
- Detailed setup docs for production with Nginx, SSL, PM2/systemd

### Oxide
- Oxide is NOT a proxy itself - it is the official Titanium Network website (github.com/titaniumnetwork-dev/Oxide)
- Oxide-Docs is the SDK documentation site for TN (github.com/titaniumnetwork-dev/Oxide-Docs)
- Built with NextJS
- Not a separate proxy project despite being listed alongside proxy names

### Alloy - github.com/titaniumnetwork-dev/alloy
- 101 stars, 1,000 forks, CSS/JavaScript/HTML
- ARCHIVED July 29, 2021
- Used node-fetch for server-side requests + client-side JS rewriting for Element.setAttribute, window.fetch(), XMLHttpRequest
- Config: port, SSL, URL prefix, local addresses, hostname blocklist
- One-click Heroku deployment available
- Preceded Corrosion in the Titanium Network lineage
- Deprecated message points to Corrosion

### Womginx - github.com/binary-person/womginx
- 184 stars, 814 forks, AGPL-3.0 license, JavaScript/Shell
- Unique approach: wombat (client-side URL rewriting from Webrecorder project) + nginx backend
- NOT a Node.js proxy - uses nginx natively for all server-side proxying
- nginx handles src/script src tags, client-side wombat handles the rest
- Works: reCAPTCHA, Discord login, WebSocket sites, cookies
- Doesn't work: React sites, minified sites depending on window.location, YouTube UI
- Docker deployment supported
- Creator motivation: learn nginx and demonstrate its speed for proxying

### Additional Notable Projects Discovered

1. **Adrift** (MercuryWorkshop/adrift) - 49 stars, 20 forks, TypeScript/Svelte
   - Decentralized P2P proxy over WebRTC, like "Tor Snowflake for the web"
   - Uses NAT traversal to connect with random exit nodes, no port forwarding needed
   - Firebase-based tracking servers for offer exchange
   - Can integrate with UV or Dynamic frontends
   - npm: @mercuryworkshop/adrift

2. **Interstellar** (UseInterstellar/Interstellar) - 1,948 stars, 22,938 forks, JavaScript, AGPL-3.0
   - Most popular modern web proxy frontend
   - Clean UI with games, tab cloaking, about:blank cloaking
   - Node.js-based, cannot deploy to static hosts
   - Deployable on Railway, Render, etc.

3. **Holy Unblocker** (QuiteAFancyEmerald/Holy-Unblocker) - 1,317 stars, 4,796 forks, JavaScript
   - Flagship Titanium Network proxy site
   - Uses Scramjet + Ultraviolet + Rammerhead
   - Supports Tor/Onion browsing in any browser
   - Ad-blocking, tab cloaking, advanced proxy navigation

4. **Node-Unblocker** (nfriedly/node-unblocker) - 509 stars, 1,074 forks, JavaScript
   - Oldest active project (created 2011), general-purpose proxy library
   - Express-based, streaming proxy (no buffering)
   - Middleware system: charsets, urlPrefixer, metaRobots, contentLength
   - Limitations: no OAuth, no advanced SPA sites

5. **Palladium** (LudicrousDevelopment/Palladium) - 13 stars, 91 forks, JavaScript
   - Archived, was a secondary proxy to Womginx/Corrosion
   - "Many sites not supported" per its own description

6. **Helios** (dinguschan-owo/Helios) - 292 stars, 493 forks, HTML
   - Purely client-side HTML/CSS/JS proxy, no backend needed
   - Uses fetch() to get source code instead of browser rendering
   - Routes through CORS proxy for cross-origin requests
   - Can be saved as offline HTML file - "unblockable" since it's a local file
   - Tab cloaking, URL encryption, anti-tab-close features
   - Archived but still widely forked

### Key Observations
- The TN proxy lineage: Alloy -> Corrosion -> Ultraviolet -> Scramjet
- Mercury Workshop split from Titanium Network in Summer 2025
- The community has 5M+ monthly users across all services
- Most older proxies are archived; Scramjet is the current state of the art
- Rammerhead persists as a complementary option (different approach - server-side with sessions)
- Womginx is unique in using nginx rather than Node.js
- Interstellar has the most GitHub stars of any proxy frontend (1,948)
