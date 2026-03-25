# Browser-Based Web Proxy Projects Beyond Ultraviolet & Scramjet

Research conducted 2026-03-25 on notable browser-based web proxy projects in the Titanium Network ecosystem and beyond.

---

## Titanium Network Proxy Lineage

The evolution of Titanium Network's official proxy technology follows a clear succession:

**Alloy** (2020-2021) → **Corrosion** (2021-2022) → **Ultraviolet** (2022-2024) → **Scramjet** (2024-present)

Each generation improved site compatibility, performance, and architecture. Mercury Workshop, the team behind Scramjet, split from Titanium Network in Summer 2025 to operate independently, though Scramjet continues to be maintained for the broader proxy community.

---

## 1. Stomp

**Status: NOT FOUND**

Despite thorough searching across GitHub, web search engines, and the Titanium Network ecosystem, no browser-based web proxy project called "Stomp" was identified. All search results for "Stomp" relate to the STOMP messaging protocol (Simple Text Oriented Messaging Protocol), which is entirely unrelated to web proxying. The project may have been renamed, removed, or may be known by a different name.

---

## 2. Rammerhead

| Detail | Value |
|--------|-------|
| **Repository** | [binary-person/rammerhead](https://github.com/binary-person/rammerhead) |
| **Stars** | 298 |
| **Forks** | 2,416 |
| **Language** | JavaScript (96.7%) |
| **License** | Not specified |
| **Created** | August 2021 |
| **Status** | Active but development slowed (last release v1.2.62, Dec 2022) |

### What It Is & How It Works

Rammerhead is a web proxy powered by **testcafe-hammerhead**, an HTTP proxy engine originally built for the TestCafe browser testing framework by DevExpress. It acts as a traditional server-side proxy: requests go to the Rammerhead server, which fetches the target site, rewrites URLs and injects scripts, then serves the modified content back to the client.

The key architectural difference from Ultraviolet/Scramjet is that Rammerhead operates entirely server-side with session management, rather than using service workers for client-side interception.

### Key Features

- **Persistent sessions**: Users can create sessions that sync localStorage and cookies across devices, enabling login persistence
- **Custom proxy per session**: Each session can be configured with its own upstream HTTP proxy
- **Wide site compatibility**: Works with most sites except Google login flows
- **Password protection**: Default access gated by password
- **Demo instances**: Available at `demo-opensource.rammerhead.org` and `browser.rammerhead.org`

### Limitations

- No Google login support (OAuth flows break)
- Server-side architecture means higher server resource usage than client-side approaches
- Development has slowed significantly since late 2022

### Comparison to Ultraviolet/Scramjet

| Aspect | Rammerhead | UV/Scramjet |
|--------|-----------|-------------|
| Architecture | Server-side (testcafe-hammerhead) | Client-side (service workers) |
| Session management | Built-in, cross-device | Not built-in |
| Server load | Higher (all rewriting server-side) | Lower (rewriting in browser) |
| Setup complexity | Simpler (standard Node.js) | More complex (bare/wisp servers, SW registration) |
| Site compatibility | Good (except Google auth) | Better (especially Scramjet) |

---

## 3. Corrosion

| Detail | Value |
|--------|-------|
| **Repository** | [titaniumnetwork-dev/Corrosion](https://github.com/titaniumnetwork-dev/Corrosion) |
| **Stars** | 93 |
| **Forks** | 961 |
| **Language** | JavaScript |
| **License** | MIT |
| **Created** | ~2021 |
| **Status** | **ARCHIVED** (March 8, 2022) - Succeeded by Ultraviolet |

### What It Is & How It Works

Corrosion was the official Titanium Network proxy that succeeded Alloy. It is a Node.js-based server-side proxy that rewrites HTTP requests and responses, with integration support for Express and other Node.js web frameworks. It processes requests on the server, rewrites URLs, handles cookies, and supports WebSocket proxying.

### Key Features

- **hCAPTCHA support**: Enhanced compatibility with CAPTCHA challenges (a significant improvement over Alloy)
- **URL encoding codecs**: Base64, plain text, and XOR encryption for URL obfuscation
- **Middleware system**: Customizable request/response processing pipelines
- **IP handling middleware**: Built-in support for managing client IP addresses
- **Hostname blacklisting**: Block specific domains from being proxied
- **Production deployment docs**: Detailed guides for Nginx reverse proxy, SSL, PM2/systemd

### Limitations

- Archived and no longer maintained
- Server-side only architecture (no service worker support)
- Superseded by Ultraviolet's more capable service worker approach

### Comparison to Ultraviolet/Scramjet

Corrosion was Ultraviolet's direct predecessor. Ultraviolet improved on Corrosion by moving URL rewriting to the client side via service workers, reducing server load and improving compatibility with JavaScript-heavy sites that construct URLs dynamically at runtime.

---

## 4. Oxide

| Detail | Value |
|--------|-------|
| **Repository** | [titaniumnetwork-dev/Oxide](https://github.com/titaniumnetwork-dev/Oxide) |
| **Status** | **NOT a proxy** |

**Oxide is not a web proxy.** It is the official website for Titanium Network, built with Next.js. There is also an **Oxide-Docs** repository ([titaniumnetwork-dev/Oxide-Docs](https://github.com/titaniumnetwork-dev/Oxide-Docs)) which serves as the SDK documentation site for TN's various proxy technologies. Despite sometimes being listed alongside proxy project names, Oxide itself provides no proxying functionality.

---

## 5. Alloy

| Detail | Value |
|--------|-------|
| **Repository** | [titaniumnetwork-dev/alloy](https://github.com/titaniumnetwork-dev/alloy) |
| **Stars** | 101 |
| **Forks** | 1,000 |
| **Language** | CSS (80.6%), JavaScript (18.4%), HTML (1.0%) |
| **License** | Not specified |
| **Created** | ~2020 |
| **Status** | **ARCHIVED** (July 29, 2021) - Succeeded by Corrosion |

### What It Is & How It Works

Alloy was the first generation of Titanium Network's proxy technology. It uses **node-fetch** (a server-side implementation of the browser's `fetch` API) to make requests to target websites on behalf of the client. The response is modified with URL rewrites applied to HTML attributes, and the modified content is served back to the user.

A portion of the rewriting happens client-side via injected JavaScript: `Element.setAttribute`, `window.fetch()`, `XMLHttpRequest`, and other browser APIs are monkey-patched to route through the proxy.

### Key Features

- HTTP/HTTPS proxy with WebSocket support
- Configurable port, SSL, and URL prefix
- IP selection for outbound requests
- Hostname blocking/filtering
- One-click Heroku deployment

### Limitations

- Archived and deprecated
- Limited site compatibility compared to successors
- Hybrid server/client rewriting was less reliable than later approaches
- README explicitly directs users to Corrosion

### Comparison to Ultraviolet/Scramjet

Alloy was two generations behind the current technology. Its hybrid approach (server-side fetch + client-side monkey-patching) was a stepping stone toward Ultraviolet's full service worker architecture. Each successor generation brought better site compatibility and more sophisticated URL interception.

---

## 6. Womginx

| Detail | Value |
|--------|-------|
| **Repository** | [binary-person/womginx](https://github.com/binary-person/womginx) |
| **Stars** | 184 |
| **Forks** | 814 |
| **Language** | JavaScript (68.2%), Shell, HTML, CSS, Docker |
| **License** | AGPL-3.0 |
| **Created** | January 2021 |
| **Status** | Active (still receives forks/issues) |

### What It Is & How It Works

Womginx is a unique web proxy that combines **Wombat** (a client-side URL rewriting library from the Webrecorder/web archiving project) with **nginx** as the backend server. The name is a portmanteau: **wom**bat + n**ginx**.

Unlike virtually every other proxy in this ecosystem, Womginx does NOT use Node.js. Instead:
- **nginx** handles all server-side proxying: URL rewriting for `src` and `script src` tags, cookie management, and request forwarding
- **Wombat** (client-side JavaScript) handles runtime URL rewriting in the browser, intercepting `window.location`, DOM APIs, and dynamically constructed URLs

This architecture offloads CPU from the server and leverages nginx's high performance for concurrent request handling.

### Key Features

- Fastest proxy due to nginx backend (designed for high concurrency)
- reCAPTCHA support
- Discord login (credential-based, no QR required)
- WebSocket support
- Cookie handling
- Docker/Docker Compose deployment

### Limitations

- Does NOT work with React sites (SPA framework)
- Breaks on minified sites that depend on `window.location`
- YouTube UI does not work (direct video links do)
- Requires nginx, certbot, Node.js (for building wombat), a domain, and a VPS

### Comparison to Ultraviolet/Scramjet

| Aspect | Womginx | UV/Scramjet |
|--------|---------|-------------|
| Backend | nginx (C) | Node.js |
| Client rewriting | Wombat library | Service worker interception |
| Performance | Very fast (nginx) | Fast (but Node.js overhead) |
| SPA support | Poor (React breaks) | Good (especially Scramjet) |
| Setup | Requires nginx + domain | Node.js + bare/wisp server |
| Server resources | Low (nginx is efficient) | Low (client does most work) |

---

## 7. Other Significant Projects

### Adrift (Decentralized P2P Proxy)

| Detail | Value |
|--------|-------|
| **Repository** | [MercuryWorkshop/adrift](https://github.com/MercuryWorkshop/adrift) |
| **Stars** | 49 |
| **Forks** | 20 |
| **Language** | TypeScript (60.7%), JavaScript, Svelte |
| **Status** | Active development |

Adrift is a decentralized web proxy network that uses **WebRTC** for transport - conceptually similar to Tor Snowflake but for general web browsing. Clients connect to tracking servers to exchange WebRTC "offers", then use NAT traversal to connect with volunteer exit nodes. No port forwarding is required. This distributes load across many nodes and makes the network harder to block. Can integrate with Ultraviolet or Dynamic frontends via npm.

### Interstellar (Most Popular Proxy Frontend)

| Detail | Value |
|--------|-------|
| **Repository** | [UseInterstellar/Interstellar](https://github.com/UseInterstellar/Interstellar) |
| **Stars** | 1,948 |
| **Forks** | 22,938 |
| **Language** | JavaScript |
| **License** | AGPL-3.0 |
| **Status** | Actively maintained |

The most-starred and most-forked web proxy project. Interstellar is a proxy frontend (not a proxy engine itself) with a clean UI, built-in games, tab cloaking, and about:blank cloaking. It wraps existing proxy backends (Ultraviolet, Scramjet) with a polished user experience. Cannot be deployed to static hosts - requires a Node.js server.

### Holy Unblocker LTS (Flagship TN Site)

| Detail | Value |
|--------|-------|
| **Repository** | [QuiteAFancyEmerald/Holy-Unblocker](https://github.com/QuiteAFancyEmerald/Holy-Unblocker) |
| **Stars** | 1,317 |
| **Forks** | 4,796 |
| **Language** | JavaScript |
| **Status** | Actively maintained |

The official flagship Titanium Network proxy site. Integrates Scramjet, Ultraviolet, and Rammerhead as proxy backends. Supports browsing Tor/Onion sites in any browser, ad-blocking, tab cloaking, and has extensive site compatibility including YouTube, Discord, and GeForce NOW.

### Node-Unblocker (Pioneer Project)

| Detail | Value |
|--------|-------|
| **Repository** | [nfriedly/node-unblocker](https://github.com/nfriedly/node-unblocker) |
| **Stars** | 509 |
| **Forks** | 1,074 |
| **Language** | JavaScript |
| **Status** | Active (created 2011) |

The oldest active web proxy in the Node.js ecosystem, originally inspired by CGIProxy/PHProxy/Glype. Built on Express with a streaming architecture (no buffering). Features a middleware system for URL rewriting, charset conversion, and robot meta tag injection. Does not support OAuth login flows, Discord, YouTube, or other advanced SPAs.

### Helios (Client-Side Only Proxy)

| Detail | Value |
|--------|-------|
| **Repository** | [dinguschan-owo/Helios](https://github.com/dinguschan-owo/Helios) |
| **Stars** | 292 |
| **Forks** | 493 |
| **Language** | HTML |
| **Status** | Archived but widely forked |

A purely client-side proxy written entirely in HTML/CSS/JS with no backend server. Uses `fetch()` to retrieve webpage source code and routes through a CORS proxy. Can be saved as a single offline HTML file, making it effectively "unblockable" since it runs as a local file. Features tab cloaking, URL encryption, and anti-tab-close protections. Limited in rendering capability since it bypasses the browser's normal rendering pipeline.

### Palladium (Minor/Legacy)

| Detail | Value |
|--------|-------|
| **Repository** | [LudicrousDevelopment/Palladium](https://github.com/LudicrousDevelopment/Palladium) |
| **Stars** | 13 |
| **Forks** | 91 |
| **Status** | Archived |

A minor proxy intended as a secondary/fallback option alongside Womginx or Corrosion. Self-described as having "many sites not supported." Not widely adopted.

---

## Summary Comparison Table

| Project | Stars | Architecture | Status | Key Differentiator |
|---------|-------|-------------|--------|-------------------|
| **Scramjet** | ~800+ | Service worker interception | **Active** (current flagship) | Most advanced, best site compat |
| **Ultraviolet** | ~4,000+ | Service worker + bare-mux | **Deprecated** (succeeded by Scramjet) | Pioneered SW proxy approach |
| **Interstellar** | 1,948 | Frontend (wraps UV/Scramjet) | **Active** | Most popular frontend, huge fork count |
| **Holy Unblocker** | 1,317 | Frontend (UV + Scramjet + RH) | **Active** | Flagship TN site, Tor support |
| **Node-Unblocker** | 509 | Express streaming proxy | **Active** | Oldest, general-purpose library |
| **Rammerhead** | 298 | testcafe-hammerhead (server) | **Slowed** | Session management, cross-device sync |
| **Helios** | 292 | Client-side fetch + CORS proxy | **Archived** | No backend needed, offline-capable |
| **Womginx** | 184 | nginx + wombat (client) | **Active** | Only nginx-based proxy, very fast |
| **Alloy** | 101 | node-fetch + client patching | **Archived** | First TN proxy generation |
| **Corrosion** | 93 | Node.js server-side | **Archived** | hCAPTCHA support, middleware system |
| **Adrift** | 49 | WebRTC P2P decentralized | **Active** | Decentralized, hard to block |
| **Palladium** | 13 | Server-side | **Archived** | Minor/legacy |
| **Stomp** | - | - | **Not found** | Could not locate this project |

---

## Architectural Classification

1. **Server-side rewriting**: Alloy, Corrosion, Rammerhead, Node-Unblocker, Palladium
2. **Service worker interception**: Ultraviolet, Scramjet
3. **Hybrid (server + client rewriting)**: Womginx (nginx + wombat)
4. **Client-side only**: Helios (fetch + CORS proxy)
5. **Decentralized/P2P**: Adrift (WebRTC transport)
6. **Frontends** (not proxy engines): Interstellar, Holy Unblocker

## Sources

- [Rammerhead GitHub](https://github.com/binary-person/rammerhead)
- [Rammerhead Proxy Guide](https://www.rapidseedbox.com/blog/rammerhead-proxy)
- [Corrosion GitHub](https://github.com/titaniumnetwork-dev/Corrosion)
- [Alloy GitHub](https://github.com/titaniumnetwork-dev/alloy)
- [Womginx GitHub](https://github.com/binary-person/womginx)
- [Adrift GitHub](https://github.com/MercuryWorkshop/adrift)
- [Interstellar GitHub](https://github.com/UseInterstellar/Interstellar)
- [Holy Unblocker GitHub](https://github.com/QuiteAFancyEmerald/Holy-Unblocker)
- [Node-Unblocker GitHub](https://github.com/nfriedly/node-unblocker)
- [Helios GitHub](https://github.com/dinguschan-owo/Helios)
- [Palladium GitHub](https://github.com/LudicrousDevelopment/Palladium)
- [Titanium Network Docs](https://docs.titaniumnetwork.org/)
- [Titanium Network GitHub Org](https://github.com/titaniumnetwork-dev)
- [Mercury Workshop GitHub Org](https://github.com/MercuryWorkshop)
