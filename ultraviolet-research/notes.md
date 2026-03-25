# Ultraviolet Web Proxy - Research Notes

## Research Session - 2026-03-25

### Sources Consulted
- GitHub repo: https://github.com/titaniumnetwork-dev/Ultraviolet
- Titanium Network docs: https://docs.titaniumnetwork.org/proxies/ultraviolet
- Various third-party analysis articles (Pixelscan, RapidSeedbox, Bomberbot, MomoProxy, ProxyReviewHub)
- GitHub issues page
- CHANGELOG.md on GitHub
- package.json on GitHub

### Key Findings

**Repo Stats (as of 2026-03-25):**
- 791 stars, 5.7k forks
- License: AGPL-3.0 on GitHub, MIT in package.json (discrepancy noted)
- Language: JavaScript (100%)
- Latest release: v3.2.7 (Aug 1, 2024), latest version in package.json: 3.2.10
- 26 open issues
- Effectively unmaintained - succeeded by Scramjet
- npm package: @titaniumnetwork-dev/ultraviolet

**Architecture:**
- Service worker intercepts HTTP requests in the browser
- Uses BareMux transport layer for backend communication
- Backend uses Bare server or Wisp protocol over WebSockets
- Client-side rewriting of HTML (parse5), JS (meriyah + astring), CSS (was css-tree, removed in v3.2.3)
- URL encoding via xor, base64, or plain codecs
- Follows TompHTTP specification

**Dependencies (production):**
- @mercuryworkshop/bare-mux
- astring (JS code generation)
- events
- idb (IndexedDB wrapper)
- meriyah (JS parser)
- parse5 (HTML parser)
- set-cookie-parser

**Notable: In v3.2.3, css-tree and esotope-hammerhead were removed to reduce bundle size. CSS rewriting was simplified. esotope-hammerhead was replaced by astring for JS code generation, meriyah replaced acorn.js for JS parsing.**

**Issues themes:**
- Social media platforms failing (Instagram, Pinterest, Reddit, WhatsApp)
- Cloudflare protections (Turnstile, Access) breaking sites
- WebSocket errors
- TLS handshake issues
- YouTube blocking
- Tab crashes
- IFrame hijacking vulnerability (#25)
- EventEmitter memory leaks (#85)

**Security/Detection:**
- No end-to-end encryption of traffic (despite some claims)
- Header anomalies detectable by advanced systems
- Service worker patterns identifiable
- Not safe for authenticated sessions
- Browser-tab level only, not system-wide
- iframe hijacking concern noted in issue #25 and Ultraviolet-App issue #11

**Ecosystem:**
- Titanium Network: founded 2016, 31k Discord members, 5M+ users/month
- Projects using UV: Holy Unblocker, Incognito, Nebula, Metallic, AnuraOS, Terbium, Alu
- Successor: Scramjet (WASM-based, actively maintained)
- Related tech: Wisp protocol, bare-mux, EpoxyTransport, libcurl-transport, Rammerhead
- Key people: "Duce" (creator of UV and Incognito), Mercury Workshop (development partner)

**Performance:**
- One source claims 187ms avg response time, 99.8% success rate, 98.5 Mbps bandwidth
- These benchmarks are from a single source and not independently verified
- Client-side rewriting reduces server load compared to server-side proxies
- v3.2.3 reduced bundle size significantly by removing heavy dependencies

**Scramjet comparison:**
- Scramjet uses WASM-based architecture
- Broader site compatibility
- Better developer experience
- Actively maintained
- Same BareMux transport layer
- Easy migration path from UV
