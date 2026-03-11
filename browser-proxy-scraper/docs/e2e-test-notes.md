# E2E Browser Testing - Captcha & Fingerprint Bypass

## Test Date: 2026-03-11

## Setup
- Relay server: already running on port 3000
- Client dev server: started on port 5175 (5173/5174 were occupied)
- Vite proxies `/wisp` to `ws://localhost:3000`
- Service worker registered and activated on first page load
- Browser: Chrome 145.0.7632.160 on macOS (Apple M1 Pro)

## Tests

### Test 1: Basic Proxy - example.com
**Result: PASS**
- Navigated to `https://example.com` through proxy URL bar
- Page rendered correctly in iframe showing "Example Domain"
- Puppet agent loaded and reported "Puppet ready"
- First click on Go button didn't register (coordinate-based click hit wrong area), ref-based click worked

### Test 2: Fingerprint Self-Test Page
**Result: PARTIAL PASS (5/7 checks)**

Navigated to `http://localhost:5175/fingerprint-self-test.html`

Diff summary:
```json
{
  "webdriverChanged": true,        // PASS - webdriver flag removed
  "pluginsChanged": true,           // PASS - realistic plugins injected (5→3)
  "canvasHashChanged": true,        // PASS - canvas noise active
  "canvasHashStableWithinPatchedSnapshot": true,  // PASS - noise is deterministic
  "webglVendorChanged": false,      // FAIL - GPU vendor not spoofed
  "webglRendererChanged": false,    // FAIL - GPU renderer not spoofed
  "uaChanged": false,               // N/A - same Chrome version, not a failure
  "outerMetricsChanged": true       // PASS - outer dimensions spoofed (1920→1932)
}
```

Additional observations from baseline vs patched:
- Architecture spoofed: arm → x86
- Platform version spoofed: 26.2.0 → 14.0.0
- Brand ordering changed between baseline and patched
- Canvas imageData pixel values show subtle differences (e.g., 3rd byte: 39→40)
- Canvas hashes completely different between snapshots

**Issue**: WebGL vendor/renderer NOT spoofed. Both show real hardware:
- Vendor: "Google Inc. (Apple)"
- Renderer: "ANGLE (Apple, ANGLE Metal Renderer: Apple M1 Pro, Unspecified Version)"
- This is a significant fingerprinting leak - anti-detect.js needs WebGL spoofing

### Test 3: TLS Fingerprint (tls.peet.ws)
**Result: PARTIAL PASS**

Navigated to `https://tls.peet.ws/api/all` through proxy.

- **JA3 hash**: `40e1c8311fb03963ffc6734555a51666`
  - Does NOT match expected Chrome 116 hash (`afbd0a4609da8f5101cdcde48d53e590`)
  - This is the native browser TLS (Epoxy/Rustls), not curl-impersonate
  - Expected since we're using Epoxy transport, not curl-WASM for page loads
- **JA4**: `t13d1515h2_8daaf6152771_5d45727bf495`
  - **MATCHES** the expected value exactly
  - Confirms TLS extension ordering and cipher suite selection matches Chrome pattern
- HTTP/2 connection confirmed (h2)
- User-Agent consistent: Chrome/145.0.0.0

**Note**: JA3 mismatch is expected since JA3 is known to vary by Chrome version.
JA4 match is the more meaningful signal - it was designed to be more stable across versions.

### Test 4: reCAPTCHA Demo
**Result: PASS**

Navigated to `https://www.google.com/recaptcha/api2/demo`

- Form rendered correctly with all fields (First Name, Last Name, Email)
- reCAPTCHA v2 widget rendered with "I'm not a robot" checkbox visible
- **No "Invalid Domain" error** - origin spoofing working correctly
- reCAPTCHA logo and Privacy/Terms branding rendered properly
- Pre-filled demo values visible (Jane, Smith, stopallbots@gmail.com)

### Test 5: hCaptcha Demo
**Result: PASS (auto-verified!)**

Navigated to `https://accounts.hcaptcha.com/demo`

- Page showed "Verification Success!" immediately
- Server response: `{"success": true, "challenge_ts": "2026-03-11T15:00:31.000000Z", "hostname": "hcaptcha.com"}`
- The origin spoofing convinced hCaptcha the request came from its own domain
- Full P1 verification token generated
- **This is the strongest result** - hCaptcha auto-passed without even showing a challenge

### Test 6: Cloudflare Turnstile
**Result: PARTIAL (known limitation)**

Navigated to `https://turnstiledemo.lusostreams.com/`

- Demo page loaded correctly with all mode tabs (Managed, Non-Interactive, Invisible, etc.)
- Turnstile widget rendered with Cloudflare branding
- Widget shows "Verifying..." spinner but hangs
- After ~20 seconds shows "Stuck? Troubleshoot" link
- Server response stuck at "Waiting for verification..."
- **Confirms known limitation** - Turnstile loads visually but can't complete the challenge
- Likely due to Turnstile's deeper browser environment checks (worker scope, timing, etc.)

### Test 7: CreepJS Fingerprint Audit
**Result: INFORMATIONAL**

Navigated to `https://abrahamjuliot.github.io/creepjs/`

Key findings:
- **Headless detection**: "chromium: true, 25% like headless, 33% headless, 40% stealth"
  - Not flagged as definitively headless, but some signals detected
  - The stealth percentage (40%) suggests some anti-detect measures are visible
- **WebRTC leak**: Real IP exposed (99.231.224.58) via STUN connection
  - Host connection shows local candidate with private IP
  - STUN connection reveals public IP through relay
- **GPU confidence**: HIGH - real hardware exposed (Apple M1 Pro via ANGLE)
- **Resistance**: All "unknown" (privacy, security, mode) - good, not detected as privacy mode
- **Devices**: mic, audio, webcam (3 devices detected)
- **Worker context**: Shows real arch (arm_64) despite anti-detect patching main context to x86
  - Worker scope not covered by anti-detect.js patching

## Summary

| Test | Status | Notes |
|------|--------|-------|
| Basic proxy (example.com) | PASS | Page renders, puppet ready |
| Fingerprint self-test | PARTIAL | 5/7 checks pass, WebGL not spoofed |
| TLS fingerprint | PARTIAL | JA4 matches, JA3 differs (version-dependent) |
| reCAPTCHA | PASS | Widget renders, no domain error |
| hCaptcha | PASS | Auto-verified! Origin spoof convincing |
| Turnstile | PARTIAL | Widget loads but hangs on verification |
| CreepJS audit | INFO | 25-40% headless signals, WebRTC IP leak |

## Key Issues to Address
1. **WebGL spoofing missing** - anti-detect.js doesn't override WebGL vendor/renderer
2. **WebRTC IP leak** - STUN connections reveal real IP address
3. **Worker context not patched** - anti-detect.js only patches main thread, not worker scope
4. **Turnstile verification hangs** - deeper browser checks needed
5. **Headless signals** - CreepJS detects 25-40% headless/stealth indicators
