# Captcha and Fingerprint Bypass Investigation Report

## Executive Summary

This investigation analyzed captcha rendering and fingerprint masking within the `browser-proxy-scraper` architecture. **Update (2026-03-11):** E2E browser testing reveals that the origin spoofing implementation (AST sandboxing in SW + HTTP header spoofing) has resolved most captcha rendering issues. reCAPTCHA renders correctly and hCaptcha auto-verifies. Cloudflare Turnstile remains broken. Fingerprint masking is partially effective but has key gaps.

---

## E2E Test Results (2026-03-11)

Testing conducted with Chrome 145 on macOS (Apple M1 Pro) against live captcha providers.

| Test | Result | Details |
|------|--------|---------|
| Basic Proxy | **PASS** | example.com renders in iframe, puppet agent active |
| Fingerprint Self-Test | **5/7 PASS** | WebGL vendor/renderer not spoofed |
| TLS Fingerprint (JA4) | **PASS** | JA4 matches Chrome pattern exactly |
| reCAPTCHA v2 | **PASS** | Widget renders with no domain errors |
| hCaptcha | **PASS** | Auto-verified without showing challenge |
| Cloudflare Turnstile | **FAIL** | Widget loads but verification hangs indefinitely |
| CreepJS Audit | **INFO** | 25-40% headless detection, WebRTC IP leak |

### What Works

1. **Origin Spoofing** - The SW-based origin rewriting convinces reCAPTCHA and hCaptcha that requests originate from the target domain. hCaptcha auto-verified without presenting a challenge, returning `"success": true` with `"hostname": "hcaptcha.com"`. This contradicts the earlier assessment that iframe captcha bypass was impossible.

2. **Canvas Fingerprint Masking** - Canvas noise injection produces different hashes between baseline and patched snapshots, while remaining stable within a single patched session (deterministic noise).

3. **Navigator Property Patching** - Webdriver flag removed, plugin count changed (5→3), architecture spoofed (arm→x86), platform version spoofed (26.2.0→14.0.0), outer dimensions randomized.

4. **TLS Fingerprint (JA4)** - The JA4 hash `t13d1515h2_8daaf6152771_5d45727bf495` matches the expected Chrome pattern exactly, confirming correct TLS extension ordering and cipher suite selection through the Epoxy transport.

### Remaining Gaps

1. **WebGL Not Spoofed** - `anti-detect.js` does not override `WEBGL_debug_renderer_info`. Both vendor ("Google Inc. (Apple)") and renderer ("ANGLE Metal Renderer: Apple M1 Pro") expose real hardware.

2. **WebRTC IP Leak** - STUN connections reveal the user's real public IP. The WebRTC data channel bypasses the proxy's TCP tunnel.

3. **Worker Context Unpatched** - Anti-detect patches only apply to the main thread. CreepJS detected real architecture (arm_64) through worker scope despite main thread reporting x86.

4. **Turnstile Verification Failure** - Cloudflare Turnstile widget renders and begins verification but never completes. Likely caused by Turnstile's deeper environment checks (worker integrity, timing analysis, execution context validation).

5. **Partial Headless Detection** - CreepJS reports 25% "like headless", 33% "headless", 40% "stealth" signals.

---

## Prior Analysis (for context)

### 1. The Iframe Origin Problem (Partially Resolved)
The original architecture embedded the target site in an `iframe` served from `localhost`, which caused captcha domain validation failures. The new SW-based origin spoofing approach (AST sandboxing + HTTP header rewriting) has resolved this for reCAPTCHA and hCaptcha, though Turnstile's deeper checks still detect the proxy.

### 2. Browser Fingerprinting Deficiencies (Partially Resolved)
Canvas and navigator property spoofing have been implemented. WebGL and WebRTC remain unpatched. The worker context is not covered by anti-detect patches.

### 3. Architectural Alternatives
These remain valid options for cases where the current approach falls short:

- **Option A: Local Reverse Proxy with Custom CA** - For Turnstile-protected sites
- **Option B: Browser Extension Architecture** - For native top-level context
- **Option C: Hardened Headless Solutions** - For maximum anti-detect coverage

---

## Priority Recommendations

| Priority | Issue | Suggested Fix |
|----------|-------|---------------|
| High | WebGL not spoofed | Add `getParameter(UNMASKED_VENDOR/RENDERER)` override to anti-detect.js |
| High | WebRTC IP leak | Disable WebRTC or route ICE candidates through proxy |
| Medium | Worker context | Inject anti-detect patches into SW/worker scope |
| Medium | Headless signals | Investigate which specific APIs trigger CreepJS detection |
| Low | Turnstile | Requires deeper research into CF's verification protocol |

# Captcha and Fingerprint Bypass Investigation Notes

## Problem Statement
The current proxy architecture runs the target site inside an `iframe` served from `localhost` using a Service Worker and `curl-impersonate` WASM to bypass TLS fingerprinting. While TLS impersonation works, Cloudflare Turnstile, reCAPTCHA, and hCaptcha still fail or are unsolvable. They detect they are inside an iframe on an unexpected domain (`localhost`) or fail browser fingerprinting checks (e.g., Google/Instagram).

## What we tried previously
- Vendor registry for captcha scripts in the service worker
- Origin shims to expose target-origin values
- `anti-detect.js` to patch browser surface inside the iframe
- Turnstile inner challenge loaded, but failed to render as solvable

## Focus Areas for Investigation
1. **Iframe Top-Level Origin Restriction:** How Turnstile/hCaptcha verify embedding origin and whether it can be tricked from an iframe on `localhost`.
2. **Alternative Proxy Approaches:** Should we drop the `iframe` completely and use a reverse proxy or shadow DOM approach?
3. **Browser Fingerprinting Evasion:** What signals do Google/Instagram check that our current architecture misses?

---

### Research Log
#### Iframe Origin Enforcement (Turnstile / reCAPTCHA / hCaptcha)
1. Captcha widgets run client-side challenges and generate a token that gets validated server-side by checking the Site Key.
2. The verification API verifies that the token was generated on the domain associated with the site key.
3. Inside the browser, the top-level origin is checked against `window.location.ancestorOrigins` and the CSP `frame-ancestors` directive.
4. Because `ancestorOrigins` is a read-only `DOMStringList` maintained securely by the browser, it is impossible to spoof it from inside an iframe.
5. Conclusion: Any proxy architecture using `localhost` iframes will intrinsically fail captchas requiring top-level domain origin verification.

#### Alternative Proxy Architectures
1. **Popup Windows:** Opening the target site in a popup (`window.open`) does not run under the Service Worker's scope if the popup URL is cross-origin.
2. **Reverse Proxying with MITM:** Serving the actual target site domain (e.g. `example.com`) over a local server by modifying `/etc/hosts` and using a trusted custom CA (like `mkcert`). This allows Service Workers to register on the target origin natively, solving the top-level origin problem without iframes.
3. **Browser Extensions:** Injecting content scripts or using `chrome.webRequest` / MV3 Declarative Net Request can intercept requests directly on the real site without needing an iframe proxy.

#### Browser Fingerprinting Signals
We checked how Instagram and Google evaluate browsers for automation.
They review:
- Browser and Device Fingerprinting: WebGL and Canvas output, installed fonts, native plugins, timezone, languages, screen resolution, hardware concurrency, OS version.
- JavaScript feature tests like `navigator.webdriver`.
- Behavioral analysis and Network anomalies.
- TLS/HTTP Fingerprint vs. JS `navigator.userAgent`: Our current setup (`curl-impersonate`) spoofs Chrome 116 for network requests. However, the outer browser executing the `iframe` might be running Safari or Chrome 130! This discrepancy is an instant flag for detection.

Our `anti-detect.js` currently misses:
- WebGL vendor/renderer spoofing.
- Canvas readout scrambling (noise injection).
- User-Agent sync between the proxy JS execution context and `curl-impersonate` HTTP headers.
- Hardware concurrency and memory overrides.
