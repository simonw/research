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
