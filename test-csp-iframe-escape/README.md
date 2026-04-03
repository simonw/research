# Can JavaScript Escape a CSP Meta Tag Inside an Iframe?

Testing whether JavaScript running inside a `sandbox="allow-scripts"` iframe can remove, modify, or circumvent a `<meta http-equiv="Content-Security-Policy">` tag to escape network restrictions.

## Background

When embedding untrusted code in an iframe using `srcdoc`, one approach to prevent network access is to inject a CSP meta tag as the first element:

```html
<iframe sandbox="allow-scripts" srcdoc="
  <meta http-equiv='Content-Security-Policy'
        content='default-src none; script-src unsafe-inline; style-src unsafe-inline;'>
  <!-- untrusted content here -->
"></iframe>
```

The claim is that:
1. The browser enforces CSP before any scripts run
2. Multiple CSP policies intersect (most restrictive wins)
3. Dynamically inserted/modified CSP meta tags via JS are ignored

## Test Setup

- Local HTTP server via `python -m http.server`
- Headless Chrome automation via [Rodney](https://github.com/nicois/rodney)
- A `canary.txt` file served at the root — any successful fetch of this file indicates a CSP escape
- 34 tests across 4 rounds, each attempting a different bypass technique

## Demo Pages

- [Round 1: Basic Escape Attempts (Tests 1-10)](https://simonw.github.io/research/test-csp-iframe-escape/index.html)
- [Round 2: Advanced Techniques (Tests 11-20)](https://simonw.github.io/research/test-csp-iframe-escape/round2.html)
- [Round 3: Deep Dive on Findings (Tests 21-28)](https://simonw.github.io/research/test-csp-iframe-escape/round3-deep-dive.html)
- [Round 4: Absolute URLs from data: URIs (Tests 29-34)](https://simonw.github.io/research/test-csp-iframe-escape/round4-absolute-urls.html)

**Note:** The demo pages need to be served from an HTTP server to work (the iframes make requests to the server). On GitHub Pages they will show the test structure but fetch/beacon tests will target the GitHub Pages server rather than localhost.

## Results Summary

### Round 1: Basic Escape Attempts — All Blocked

![Round 1 results](results-screenshot.png)

| Test | Technique | Result |
|------|-----------|--------|
| 1 | Baseline: fetch from inside CSP iframe | Blocked |
| 2 | Remove the meta tag, then fetch | Blocked |
| 3 | Modify meta tag content to permissive policy | Blocked |
| 4 | Add a second, permissive CSP meta tag | Blocked |
| 5 | `document.write()` to replace entire document (no CSP) | Blocked |
| 6 | Create nested iframe without CSP | Blocked (null body) |
| 7 | Image beacon exfiltration | Blocked |
| 8 | WebSocket connection | Blocked |
| 9 | Remove meta + `document.write()` | Blocked |
| 10 | Delayed fetch after meta removal | Blocked |

### Round 2: Advanced Techniques — Two Interesting Findings

![Round 2 results](round2-screenshot.png)

| Test | Technique | Result |
|------|-----------|--------|
| 11 | Web Worker creation | Blocked by CSP |
| 12 | EventSource (SSE) | Blocked |
| 13 | XMLHttpRequest | Blocked |
| 14 | `navigator.sendBeacon()` | **Returns `true`** (see below) |
| 15 | Dynamic `<script src>` injection | Blocked |
| 16 | CSS `@import` exfiltration | Blocked |
| 17 | Meta refresh to `data:` URI | **JS executes** (see below) |
| 18 | Blob URL navigation | Fetch blocked |
| 19 | `document.write` with permissive CSP | Script didn't execute |
| 20 | Service Worker registration | Blocked by sandbox |

### Round 3: Deep Dive — False Alarms Investigated

![Round 3 results](round3-screenshot.png)

| Test | Technique | Result |
|------|-----------|--------|
| 21 | sendBeacon — check server logs | **No request reached server** |
| 22 | Meta refresh to data: URI, then fetch | Fetch fails (null origin) |
| 23 | Static meta refresh in srcdoc | Timeout (didn't navigate) |
| 24 | Meta refresh to `javascript:` URI | Blocked |
| 25 | `location.href` to data: URI, then fetch | Fetch fails (null origin) |
| 26 | sendBeacon with explicit `connect-src 'none'` | Returns true, **no server request** |
| 27 | Verify data: URI JS execution | Confirmed — JS runs but is isolated |
| 28 | document.write with permissive CSP + delay | Script didn't execute |

### Round 4: Absolute URLs from data: URIs — All Blocked

![Round 4 results](round4-screenshot.png)

| Test | Technique | Result |
|------|-----------|--------|
| 29 | Absolute URL fetch from data: URI | Blocked |
| 30 | Same via location.href path | Blocked |
| 31 | XHR with absolute URL from data: URI | Blocked |
| 32 | Image with absolute URL from data: URI | Blocked |
| 33 | sendBeacon from data: URI | Returns true, **no server request** |
| 34 | Check sandbox flags after data: navigation | origin=null, sandbox persists |

## Key Findings

### 1. CSP Meta Tag Is Enforced at Parse Time and Cannot Be Removed

Once Chrome parses a `<meta http-equiv="Content-Security-Policy">` tag, the policy is permanently enforced for that document. JavaScript can remove or modify the DOM element, but the CSP engine ignores this — the policy was already "baked in."

### 2. `document.write()` Does NOT Reset CSP

Completely replacing the document content via `document.write()` — even writing a new, more permissive CSP meta tag — does not lift the original CSP restrictions. The original policy persists across document replacement.

### 3. Navigation to `data:` URIs Is Possible But Not an Escape

This was the most interesting finding. JavaScript inside the sandbox can navigate to a `data:` URI via `location.href` or dynamically-inserted `<meta http-equiv="refresh">`, and **JavaScript executes in the new page**. However:

- The `sandbox` attribute persists across navigation (the iframe is still sandboxed)
- The new page has a `null` origin
- **All network requests (fetch, XHR, Image, WebSocket) with absolute URLs are blocked**
- The original CSP doesn't carry over, but the `null` origin and sandbox isolation prevent any useful network access

This means the data: URI navigation is **not a CSP escape** — the sandbox provides a second layer of defense.

### 4. `sendBeacon()` Is a Red Herring

`navigator.sendBeacon()` returns `true` even when CSP blocks the actual network request. Server logs confirmed that **zero beacon requests arrived at the server** across all tests. This is a quirk of the sendBeacon API (it reports whether the request was queued, not whether it succeeded), not a CSP bypass.

### 5. Dynamically Inserted CSP Meta Tags Are Ignored

Adding a new `<meta http-equiv="Content-Security-Policy">` tag via JavaScript has no loosening effect, confirming the spec. Multiple CSP policies can only intersect (tighten), never loosen.

## Conclusion

**The CSP meta tag approach is secure against JavaScript escape in Chrome.** Across 34 different bypass techniques, none successfully made a network request escape the sandbox.

The combination of `sandbox="allow-scripts"` + CSP meta tag provides defense in depth:
- CSP blocks network requests at the content policy level
- The sandbox attribute provides origin isolation and persists across navigation
- Even if code navigates away from the CSP-protected document (via data: URIs), the sandbox prevents network access

For production use, combining this with the `csp` attribute on the iframe element (Option 3 in the original document) provides an additional layer that is entirely parent-controlled and cannot be influenced by iframe content at all.
