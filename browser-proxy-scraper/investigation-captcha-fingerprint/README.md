# Captcha and Fingerprint Bypass Investigation Report

## Executive Summary
This investigation analyzed why Cloudflare Turnstile, Google reCAPTCHA, and hCaptcha fail to render as solvable within the current `browser-proxy-scraper` architecture. We also examined why advanced bot detectors on sites like Google and Instagram are flagging the browser proxy setup. 

Our findings indicate that the current `localhost` iframe strategy is fundamentally incompatible with modern Captcha origin verification mechanisms. Additionally, significant gaps exist in our browser surface patching that make the setup highly detectable as a bot.

---

## 1. The Iframe Origin Problem
The current architecture embeds the target site in an `iframe` served from `localhost` and intercepted by a Service Worker. 

**Why it fails:**
- Modern Captchas verify the domain where the captcha was solved via server-side checks and client-side telemetry.
- On the client side, they inspect `window.location.ancestorOrigins` to verify the top-level embedding domain.
- **`ancestorOrigins` cannot be spoofed:** This is a read-only DOMStringList enforced by the browser's security model. As long as our proxy runs as a child frame of `localhost`, the captcha script will securely detect it.
- **Conclusion:** As long as we use an `iframe` hosted on a different domain (`localhost`), these captchas will remain unsolvable. 

## 2. Browser Fingerprinting Deficiencies
While `curl-impersonate` successfully masks our Node backend TLS/HTTP fingerprint (mimicking Chrome 116), the browser rendering the iframe leaks an entirely different fingerprint to JavaScript running in the page. 

**What Instagram and Google detect:**
- **Fingerprint Mismatch:** If `curl-impersonate` sends HTTP requests as Chrome 116 on Windows, but the JS `navigator.userAgent` reports Chrome 130 on macOS (or Safari), detectors immediately flag the session.
- **Missing Anti-Detect Coverage:** Our current `anti-detect.js` removes basic flags like `navigator.webdriver`. However, it misses the heavy-hitters:
  - **WebGL:** Unpatched. Needs vendor/renderer spoofing.
  - **Canvas:** Unpatched. Needs noise injection so identical rendering engines yield unique hashes.
  - **AudioContext & Fonts:** Unpatched. 
  - **Hardware / Device properties:** Hardcoded concurrency and memory spoofing map to generic headless profiles.

## 3. Recommended Architectural Alternatives
If we wish to support Captcha solving and bypass strict bot detectors, we must evolve the architecture beyond standard `localhost` iframes.

### Option A: Local Reverse Proxy with Custom CA (MITM)
Instead of an iframe on `localhost`, modify the system's DNS (e.g. via `/etc/hosts` or a local DNS server) to point the target domain to a local proxy server. By installing a custom local CA (like `mkcert`), we can serve the target site over native HTTPS.
- **Pros:** Captchas see the real origin; Service Workers can install natively on the target domain; no iframe required.
- **Cons:** Requires system-level configuration, breaking the "zero-install browser-in-browser" paradigm.

### Option B: Browser Extension Architecture
Ship the proxy logic as a browser extension. Extensions can use Content Scripts and Declarative Net Request/Web Request to intercept traffic directly on the target's real URL without an iframe.
- **Pros:** Native top-level context; complete access to modify headers and requests.
- **Cons:** Re-architecting into an extension format.

### Option C: Hardened Headless Solutions
Use a fully patched headless browser stack specifically designed for anti-detect (e.g., Puppeteer with Stealth Plugin, or an Anti-Detect Browser like Multilogin APIs) instead of trying to patch standard browsers via injected JS.

## Next Steps
- **Abandon the Iframe Captcha Effort:** Stop sinking time into shimming messages and origins in the `localhost` iframe. It will not work for Turnstile/reCAPTCHA.
- **Enhance `anti-detect.js`:** If you keep the current setup for targets without strict Captchas, you *must* sync `navigator.userAgent` to match your `curl-impersonate` headers, and implement Canvas/WebGL masking to reduce Google/Instagram blocks.
