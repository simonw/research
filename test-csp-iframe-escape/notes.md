# CSP Meta Tag Iframe Escape Testing

## Goal
Test whether JavaScript inside an iframe with `sandbox="allow-scripts"` can remove or modify a CSP `<meta>` tag to escape its restrictions.

## Option 2 from CSP.md
The approach uses a `<meta http-equiv="Content-Security-Policy">` tag as the first element in `srcdoc` HTML. Claims:
1. Browser parses/enforces CSP before any scripts run
2. Multiple CSP policies intersect (most restrictive wins)
3. Dynamically inserted CSP meta tags via JS are ignored

## Setup
- Local HTTP server: `python -m http.server 8000`
- Browser automation: `uvx rodney` (headless Chrome)
- Canary file at `/canary.txt` containing "CANARY_ESCAPED"
- Tests use `postMessage` to report results to parent page

## Round 1 Results (Tests 1-10) - All PASS
All basic escape attempts blocked:
- Test 1: Baseline fetch blocked ✓
- Test 2: Removing meta tag doesn't help ✓
- Test 3: Modifying meta tag content doesn't help ✓
- Test 4: Adding permissive meta tag ignored ✓
- Test 5: document.write without CSP still blocked ✓
- Test 6: Nested iframe creation blocked (null body in sandbox) ✓
- Test 7: Image beacon blocked ✓
- Test 8: WebSocket blocked ✓
- Test 9: Remove meta + document.write still blocked ✓
- Test 10: Delayed fetch after meta removal still blocked ✓

## Round 2 Results (Tests 11-20) - Two Interesting Findings
- Test 11: PASS - Worker blocked by CSP
- Test 12: PASS - EventSource blocked
- Test 13: PASS - XHR blocked
- Test 14: **sendBeacon returned true** - but see Round 3
- Test 15: PASS - external script blocked
- Test 16: PASS - @import blocked
- Test 17: **meta refresh to data: URI - JS executed!** - but see Round 3
- Test 18: PASS - blob nav fetch blocked
- Test 19: TIMEOUT - document.write with permissive CSP (script didn't run)
- Test 20: PASS - Service Worker blocked by sandbox

## Round 3 Results (Tests 21-28) - Deep Dive
- Test 21: sendBeacon returns true but **no request reaches server** (confirmed via server logs)
- Test 22: Meta refresh lands on data: URI, JS runs, but fetch fails (null origin, relative URLs broken)
- Test 23: TIMEOUT - static meta refresh in srcdoc
- Test 24: PASS - javascript: URI via meta refresh blocked
- Test 25: location.href to data: URI - JS runs but fetch fails (same null origin issue)
- Test 26: sendBeacon with explicit connect-src 'none' still returns true (still no server request)
- Test 27: Confirms JS executes in data: URI after meta refresh
- Test 28: TIMEOUT - document.write with permissive CSP

## Round 4 Results (Tests 29-34) - Absolute URLs from data: URIs
- Test 29: PASS - absolute URL fetch from data: URI blocked
- Test 30: PASS - same via location.href path
- Test 31: PASS - XHR with absolute URL from data: URI blocked
- Test 32: PASS - Image with absolute URL blocked
- Test 33: sendBeacon from data: URI returns true (but **still no server request**)
- Test 34: Sandbox flags: origin=null, cookies=yes (sandbox persists!)

## Key Findings

### The CSP meta tag approach is robust
Once the browser has parsed a CSP meta tag, removing or modifying it via JavaScript has **no effect**. The policy is "baked in" at parse time.

### document.write does NOT reset CSP
Even completely replacing the document via `document.write()` does not remove the original CSP enforcement. The CSP from the original parse persists.

### Navigation to data: URIs is possible but not an escape
JavaScript can navigate the iframe to a `data:` URI (via meta refresh or location.href), and JS executes there, but:
- The sandbox attribute persists across navigation
- The new page has a `null` origin
- Fetch/XHR/Image requests with absolute URLs are blocked
- The CSP from the parent srcdoc does NOT carry over, but the sandbox isolation does

### sendBeacon is a red herring
`navigator.sendBeacon()` returns `true` even when CSP blocks the actual request. Server logs confirm zero beacon requests arrived. This is a quirk of the sendBeacon API, not a CSP bypass.

### dynamically inserted CSP meta tags are ignored
Adding a new `<meta http-equiv="Content-Security-Policy">` via JS has no loosening effect, confirming the spec claim.

## Round 5 Results (Tests 35-42) - Data URI Image Loading
Server logs showed NO requests for canary.txt from CSP-protected data: URI pages.
Tests mostly timed out (onerror didn't fire) but server confirmed no leaks.

## Round 6 Results (Tests 43-50) - Critical Discovery
- Test 46: no-cors fetch from data: URI (NO prior CSP) -> opaque response, SERVER REQUEST LOGGED
- Test 49: srcdoc fetch (no sandbox, no CSP) -> CANARY_ESCAPED (expected baseline)
- Test 50: sandbox without CSP -> fetch blocked
- Server logs showed 3 requests to /canary.txt from tests 46, 49, and one other

**Key insight**: Without CSP, sandbox alone does NOT block no-cors fetch from data: URI pages!

## Round 7 Results (Tests 51-56) - Exfiltration Verification  
- Test 51: CSP meta -> data: URI -> no-cors fetch: BLOCKED, no server request
- Test 56: No CSP, sandbox only -> data: URI -> no-cors fetch: SERVER HIT with exfil URL
- Server log confirmed: `GET /exfil-test-56?secret=NO_CSP_SANDBOX_ONLY`

## Round 8 Results (Tests 57-63) - Blocking Navigation & csp Attribute
- Tests 57-59: Cannot block data: URI navigation (navigate-to, frame-src, sandbox all fail)
- Tests 60-63: CSP meta tag persists across data: URI navigation (blocked, no server hits)
- Test 62: csp attribute alone also blocks in Chrome

## Cross-Browser Testing (Playwright: Chromium + Firefox)
Critical finding: **Firefox ignores the `csp` attribute on iframes**
- Test I (csp attr + data: URI + no-cors): Chromium blocked, Firefox SERVER HIT
- CSP meta tag works identically in both browsers
- Without CSP, both browsers allow requests from data: URI pages in sandboxed iframes

## Conclusion
**The CSP meta tag approach (Option 2) is secure across Chromium and Firefox.** The CSP persists even after data: URI navigation.

Critical caveats:
1. sandbox alone is NOT enough - CSP meta tag is essential
2. The csp iframe attribute is Chromium-only - Firefox ignores it
3. The meta tag must come before any untrusted content
