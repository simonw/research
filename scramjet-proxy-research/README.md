# Scramjet Web Proxy - Research Report

**Date:** March 25, 2026

---

## 1. What is Scramjet? How Does It Work Technically?

Scramjet is an **interception-based web proxy** designed to bypass internet censorship and arbitrary web browser restrictions. It is developed by **Mercury Workshop** and serves as the official successor to Ultraviolet, the previous flagship proxy of the Titanium Network ecosystem.

### Architecture

Unlike traditional forward proxies, Scramjet operates by **intercepting and rewriting** web requests and responses within the browser using a **service worker**. The high-level architecture consists of:

- **ScramjetServiceWorker** - Intercepts fetch events in the service worker, routing and rewriting requests/responses
- **ScramjetController** - Client-side controller that manages initialization and configuration
- **BareMux** - A transport layer abstraction that allows pluggable transports (EpoxyTransport, CurlTransport, etc.)
- **Wisp Protocol** - Proxies multiple TCP/UDP sockets over a single WebSocket connection

### Core Technology Stack

| Component | Technology |
|-----------|-----------|
| Primary Language | TypeScript (63.3%) |
| Rewriter Engine | Rust (22.1%), compiled to WebAssembly |
| Build Tools | wasm-bindgen, wasm-opt (Binaryen), wasm-snip |
| Runtime | Node.js with service worker |
| Package Manager | pnpm |

### How the Rewriting Works

The heart of Scramjet is its **Rust-to-WASM compiled rewriter**. The process:

1. Rust code is compiled targeting `wasm32-unknown-unknown`
2. `wasm-bindgen` generates JavaScript bindings
3. `wasm-opt` optimizes the output (configurable: `-O3` for speed, `-Oz` for size)
4. `wasm-snip` eliminates dead code
5. The resulting WASM module runs in the service worker, rewriting HTML/JS/CSS in-flight

Scramjet uses a **Byte Span Rewrite** approach: rather than building a full AST and transforming it, Scramjet creates raw text strings and inserts or replaces them at specific byte locations in the original source. This is significantly faster but forbids AST-based plugins or transformers.

### Bundle Structure (v1.0.2-dev+)

- `scramjet.all.js` - Combined main bundle
- `scramjet.wasm.wasm` - WebAssembly rewriter component
- `scramjet.sync.js` - Synchronous operations

---

## 2. GitHub Repository Details

| Metric | Value |
|--------|-------|
| **Repository** | [MercuryWorkshop/scramjet](https://github.com/MercuryWorkshop/scramjet) |
| **Stars** | 367 |
| **Forks** | 578 |
| **Open Issues** | 21 |
| **License** | AGPL-3.0 |
| **Primary Language** | TypeScript |
| **Created** | May 6, 2024 |
| **Last Commit (main)** | January 16, 2025 |
| **Latest CI Build** | February 16, 2026 (pre-release, commit 6e85a22) |
| **Contributors** | 20+ listed |
| **Dependents** | 200+ projects |
| **NPM Package** | @mercuryworkshop/scramjet |

### Key Maintainers / Contributors

- **r58Playz (Toshit Chawda)** - Verified committer, core developer
- **velzie** - Active committer, merges PRs to main
- **KTibow** - Contributor (PSL fetching improvements, error screen enhancements)
- **Proxy-alt** - Contributor with merged PRs

### Organization

Mercury Workshop is a developer collective focused on free and open-source software. They were previously partnered with Titanium Network but **split and became independent in Summer 2025**. Many members remain active in the broader proxy community. Scramjet continues to be maintained for the community.

---

## 3. Key Features and Capabilities - Comparison with Ultraviolet

### Features

- **CAPTCHA Support** - Preserves the entire request chain and session context during CAPTCHA challenges, allowing normal completion
- **Wide Site Compatibility** - Officially supports Google, Twitter, Instagram, YouTube, Spotify, Discord, Reddit, GeForce NOW, now.gg
- **WASM-Powered Rewriting** - Rust-based rewriter compiled to WebAssembly for high performance
- **Pluggable Transports** - Supports EpoxyTransport (encrypted), CurlTransport, and custom transport implementations via BareMux
- **Middleware Architecture** - Can act as middleware for other open-source projects
- **TypeScript Support** - Full TypeScript definitions available
- **TypeDoc API Documentation** - Comprehensive API docs for developers
- **CI Builds** - Published per successful commit for early access to features

### Scramjet vs Ultraviolet

| Feature | Ultraviolet | Scramjet |
|---------|-------------|---------|
| **Status** | Deprecated / Unmaintained | Active flagship proxy |
| **Developer** | Titanium Network | Mercury Workshop |
| **Rewriting Engine** | JavaScript (htmlparser2) | Rust/WASM (2-3x faster) |
| **CAPTCHA Support** | Yes | Yes (improved) |
| **Site Compatibility** | Good | Better (broader coverage) |
| **Architecture** | Service worker interception | Service worker interception + WASM |
| **JS Rewriting** | Standard AST approach | Byte Span Rewrite (faster, no plugins) |
| **Security Focus** | Standard | Enhanced (header sanitization, TLS preservation) |
| **Developer Experience** | Mature ecosystem | Modern TypeScript-first, TypeDocs |

Ultraviolet's own GitHub description now reads: *"Succeeded by Scramjet."* The Titanium Network team states there is no valid reason for including Ultraviolet in a new proxy site.

---

## 4. Known Limitations, Bugs, and Issues

### Current Open Issues (21 as of March 2026)

The majority are **site-specific compatibility problems**:

- **Google AI Overview** - Broken layout/CSS (#135)
- **Instagram** - Broken (#106)
- **GeForce NOW** - Broken (#50)
- **Google Sign-in (BotGuard)** - Broken, pinned issue (#76)
- **Facebook/X Login on iPad Safari** - Fails (#131)
- **Outlook.com Sign-in on Firefox** - Not working properly (#100)
- **YouTube** - 1-minute playback limit on some machines (#141), mobile video issues (#140)
- **CodeSandbox, Jellyfin, Parsec, MangaDex** - Various breakages

### Known Architectural Limitations

1. **No AST Plugin Support** - The Byte Span Rewrite approach forbids AST manipulation, meaning no custom transformers or plugins can hook into the rewriting pipeline
2. **Function.prototype.toString Leakage** - Rewritten functions reveal their modifications when `.toString()` is called; this is a known challenge
3. **Single IP Traffic Limits** - Heavy traffic on a single IP causes some sites to stop working; IP rotation via Wireguard/wireproxy is recommended
4. **Datacenter IP Problems** - CAPTCHAs and YouTube may not work reliably on datacenter IPs
5. **Login Difficulties** - Some sites have login issues; phone verification may occur with no real solution
6. **CAPTCHA Speed Loop** - Scramjet can be "so fast you might get stuck in a loop" - users need to solve CAPTCHAs slowly
7. **Mobile Compatibility** - Black screen reported on Android; iPad Safari login failures
8. **IDBDatabase Transaction Errors** - IndexedDB issues reported (#132)
9. **URL Encoding Issues** - Search parameters not correctly handled by the encoder (#133)

---

## 5. Performance Characteristics

### Benchmarks and Claims

- **WASM HTML rewriter**: 2-3x faster than htmlparser2 (used by Ultraviolet)
- **Processing time**: Claims "sub-millisecond processing times" for complex pages
- **Blog claims**: 10-100x performance improvements over JavaScript-based rewriting (from BrightCoding article; this figure should be taken as marketing rather than rigorous benchmarking)
- **Optimization levels**: Configurable via wasm-opt (`-O3` for speed, `-Oz` for size)
- **Concurrency**: Designed for concurrent request handling with horizontal scaling support

### Performance Rating

The BrightCoding blog rates Scramjet's speed at 5/5. However, no formal, independently verified benchmarks have been published. Performance claims come from the development team and affiliated documentation.

### Deployment Performance Considerations

- Heavy traffic requires IP rotation
- Recommended to use wisp-server-python for production rather than wisp-js/server
- Horizontal scaling and load balancing supported but require additional infrastructure

---

## 6. How It Handles JavaScript, CSS, Iframes, and WebSockets

### JavaScript Rewriting

JS rewriting is described as "the biggest hurdle" in proxy development. Key aspects:

- **Why it's needed**: `location` objects are non-configurable browser properties and cannot be monkeypatched - assigning to them triggers navigation
- **Method**: Byte Span Rewriting - raw text insertions/replacements at specific byte positions in source
- **Scope**: Must handle `window`, `self`, `globalThis`, `parent`, `top` and all member expression chains
- **Eval handling**: `eval()` calls are intercepted because eval cannot be overridden; arguments are rewritten before execution
- **Known escape vectors that must be mitigated**:
  - `location = "javascript:..."`
  - Destructuring: `const { location: x } = window`
  - Object spreading: `const x = { ...window }`
  - Indirect eval: `(0, eval)("location.href")`
  - Function constructors: `Function("return this")()`
  - Reflect API: `Reflect.get(location, "href", location)`
  - ES6 Proxies wrapping window
  - Error-based throws with destructuring in catch blocks

### CSS Rewriting

CSS rewriting is considered straightforward. Per the Titanium Network documentation: *"Don't even bother [with complex parsing], RegExp is good enough. You only need to rewrite `@import` at-rule statements."* This is the one area where regex-based rewriting is explicitly recommended.

### HTML Rewriting

- Uses DOMUtils libraries for HTML manipulation
- MutationObserver approach is discouraged due to performance (requires continuous DOM observation)
- The WASM rewriter handles HTML parsing and URL rewriting in-flight
- All resource URLs (images, scripts, stylesheets) are rewritten to route through the proxy

### Iframes

Iframe handling is implicit in the interception architecture. Since all requests go through the service worker, iframe sources are rewritten like any other resource URL. The proxy must handle `parent` and `top` references in JS to prevent iframe breakout detection. Specific iframe-related issues (like CodeSandbox pages breaking, #113) suggest this remains an area with edge cases.

### WebSockets

- WebSocket support is handled through the **Wisp protocol** - a low-overhead protocol for proxying multiple TCP/UDP sockets over a WebSocket connection
- WebSocket tests were added to the project on December 30, 2025 (on the v2 branch)
- Transport options: EpoxyTransport (encrypted), CurlTransport, and libcurl.js (WASM-based TLS transport)
- The Wisp protocol is a separate Mercury Workshop project with multiple server implementations (JavaScript, Python)

---

## 7. Community and Ecosystem

### Titanium Network Community

- **Discord**: 33,000+ members ([discord.com/invite/unblock](https://discord.com/invite/unblock))
- **Monthly users**: 5+ million across Titanium Network services
- **Documentation**: [docs.titaniumnetwork.org](https://docs.titaniumnetwork.org)

### Mercury Workshop (Scramjet's Developer)

- Independent organization (split from Titanium Network in Summer 2025)
- Focus shifted from ChromeOS exploits to web proxy development
- Key related projects:
  - **AnuraOS** - Next-gen webOS with full Linux emulation
  - **Wisp** - TCP/UDP-over-WebSocket protocol
  - **Epoxy** - Encrypted proxy server
  - **libcurl.js** - WebAssembly TLS transport
  - **Dreamland** - Reactive JSX rendering library
  - **Workerware** - Middleware for service workers
  - **WispMark** - Benchmarking tool

### Ecosystem / Downstream Projects

Notable projects built on Scramjet:

| Project | Description | Stars |
|---------|-------------|-------|
| [Holy Unblocker](https://holyunblocker.org) | Production proxy service using Scramjet + UV | - |
| [DotGUI](https://github.com/DotLYHiyou/DotGUI) | Game site with Scramjet proxy | 68 |
| [Bolt Unblocker](https://github.com/Bolt-Network/Bolt-Unblocker) | Feature-packed web proxy | 60 |
| [Revision](https://github.com/transicle/Revision) | Self-hosted Scramjet with port-changing | 14 |
| [Civil](https://github.com/civilnetwork-dev/Civil) | Anonymous web proxy | 4 |
| [Scramjet-App](https://github.com/MercuryWorkshop/Scramjet-App) | Official example/deployment template | - |

200+ projects on GitHub depend on the @mercuryworkshop/scramjet npm package.

### Deployment Platforms

Scramjet can be deployed on:
- Self-hosted Node.js servers
- Cloudflare Pages (static variants)
- Vercel
- Netlify

---

## Summary

Scramjet represents the current state-of-the-art in the web proxy unblocker ecosystem. Its core innovation is the Rust-to-WASM rewriter that delivers significantly better performance than its JavaScript-based predecessor Ultraviolet. The project is actively maintained by Mercury Workshop, has a large community through Titanium Network's Discord, and powers 200+ downstream projects.

Key strengths: WASM performance, CAPTCHA support, broad site compatibility, pluggable transport architecture, strong TypeScript support.

Key weaknesses: No AST plugin support due to Byte Span Rewrite design, various site-specific breakages (especially Google sign-in, some social media), mobile compatibility issues, and reliance on non-datacenter IPs for full functionality. The AGPL-3.0 license may also be restrictive for some use cases.

---

## Sources

- [MercuryWorkshop/scramjet - GitHub](https://github.com/MercuryWorkshop/scramjet)
- [Scramjet Documentation - Titanium Network](https://docs.titaniumnetwork.org/proxies/scramjet/)
- [Interception Proxy Rewriting Guide - Titanium Network](https://docs.titaniumnetwork.org/guides/interception-proxy-guide/rewriting)
- [Mercury Workshop Organization - Titanium Network Docs](https://docs.titaniumnetwork.org/organizations/mercury-workshop/)
- [Ultraviolet Repository (Deprecated)](https://github.com/titaniumnetwork-dev/Ultraviolet)
- [Scramjet-App - GitHub](https://github.com/MercuryWorkshop/Scramjet-App)
- [Scramjet: The Web Proxy with CAPTCHA Support - BrightCoding](https://www.blog.brightcoding.dev/2026/03/21/scramjet-the-revolutionary-web-proxy-with-captcha-support)
- [Titanium Network Discord](https://discord.com/invite/unblock)
- [Holy Unblocker - Scramjet Proxy](https://holyunblocker.org/scramjet)
- [Scramjet GitHub Issues](https://github.com/MercuryWorkshop/scramjet/issues)
