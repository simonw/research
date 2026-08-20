# A shot-scraper-style JSON API on Bun 1.4's new Bun.WebView

**Verdict: entirely feasible, with a pleasantly small amount of code.** Bun 1.4's
built-in `Bun.WebView` gives you navigate / evaluate-JS-returning-JSON /
screenshot with no Puppeteer or Playwright dependency, and the whole service —
HTTP API included — fits in one ~150-line TypeScript file
([server.ts](server.ts)) with **zero npm dependencies**. Minimum RAM for a
reliable service is **~104 MB** (Chromium headless shell), **~88 MB** with
aggressive Chrome flags, or **~56 MB** if you only need JavaScript execution
and no screenshots. A full desktop Chromium binary needs ~168 MB.

## What Bun.WebView is

New in the Bun 1.3.12+ line and shipped in Bun 1.4 (still marked experimental):
a headless browser API built into the runtime.

- **macOS**: uses the system WKWebView — nothing to install at all.
- **Linux/Windows**: Bun spawns an installed Chrome/Chromium headless with
  `--remote-debugging-pipe` and drives it over the Chrome DevTools Protocol.
  Discovery is automatic, or via `BUN_CHROME_PATH` / `backend.path`, with extra
  flags via `backend.argv`.
- One Chrome process per Bun process; each `new Bun.WebView()` after the first
  is just a new tab (`Target.createTarget`), so per-request views are cheap.
- API surface: `navigate`, `evaluate` (JSON-serializes results and awaits
  promises — exactly the semantics of `shot-scraper javascript`), `screenshot`
  (PNG/JPEG/WebP `Blob`), `click`/`type`/`press`/`scroll` as *trusted* input
  events, and a raw `.cdp(method, params)` escape hatch.

## The prototype

[server.ts](server.ts) exposes:

```
POST /javascript  {"url": "...", "javascript": "...", "wait_ms": 500}
                  → {"ok": true, "result": <json>}
POST /screenshot  {"url": "...", "width": 1280, "height": 800,
                   "format": "png|jpeg|webp", "quality": 80,
                   "javascript": "...", "b64": false}
                  → image bytes (or base64 JSON)
GET  /healthz     → {"ok": true}
```

Each request opens a fresh WebView (= a Chrome tab), navigates, does its work
and closes it. That makes the service concurrency-safe for free: `evaluate()`
only allows one in-flight call *per view*, and views are per-request.

Working examples from this session (real sites, via the sandbox's egress proxy):

```
$ curl -X POST localhost:8044/javascript -d '{"url":"https://datasette.io/",
    "javascript":"new Promise(done => done({title: document.title,
                   h2: document.querySelector(\"h2\")?.textContent}))"}'
{"ok":true,"result":{"title":"Datasette: An open source multi-tool for
 exploring and publishing data","h2":"Exploratory data analysis"}}
```

Screenshots captured through the API: [datasette.png](datasette.png) (PNG,
1280px) and [example.jpg](example.jpg) (JPEG quality 60). Page-side exceptions
and navigation failures come back as `{"ok":false,"error":"..."}`.

Run it:

```
BUN_CHROME_PATH=/path/to/chromium CHROME_EXTRA_ARGS="--no-sandbox" bun server.ts
```

(No env vars needed on macOS — system WebKit — or when Chrome is in a standard
location and you aren't root.)

## Resource usage — the headline numbers

Measured by running the service inside a cgroup with a hard `memory.limit_in_bytes`
(no swap, so over-limit = OOM kill) and binary-searching the minimum limit at
which a mixed workload — JS evaluation against simple and heavy pages plus
1280×800 screenshots of a heavy page (500 DOM nodes, big canvas, 100k-element
JS array) — keeps succeeding. See [bench.sh](bench.sh) and
[testpages.ts](testpages.ts); full data in [notes.md](notes.md).

| Configuration | Minimum reliable limit | First failing limit |
|---|---|---|
| Full desktop Chromium, JS + screenshots | **168 MB** | 160 MB |
| Chromium `headless_shell`, JS + screenshots | **104 MB** | 96 MB |
| `headless_shell` + trim flags¹, JS + screenshots | **88 MB** | 80 MB |
| `headless_shell` + trim flags¹, JS only | **56 MB** | 48 MB |

¹ `--no-zygote --renderer-process-limit=1 --js-flags=--max-old-space-size=32 --disable-dev-shm-usage`

Notes:

- The Bun process itself is only ~27 MB PSS; Chrome dominates everything.
- Playwright's `headless_shell` build is the big win: it skips the GPU
  process, most utility processes and UI baggage of full Chrome (~103 MB peak
  vs ~176 MB for the identical workload).
- `--single-process` gets JS-only down to ~64 MB but screenshots become
  unreliable; it's officially unsupported, so not recommended.
- Practical guidance: **a 128 MB container comfortably runs a
  headless_shell-based screenshot+JS service for light pages; budget 192–256 MB
  for full Chrome or heavy real-world pages.** Memory scales with page
  complexity — a JS-heavy site will need more than these floors.

## Performance

On this container (16 GB, shared CPUs), per request including tab
create/navigate/close: ~64 ms for `/javascript`, ~308 ms for a heavy-page
`/screenshot`. 8 concurrent JS requests completed in 193 ms total and
8 concurrent heavy screenshots in 1.7 s — the shared-Chrome/tab-per-request
model parallelizes well.

## Caveats found along the way

- **Root needs `--no-sandbox`** or Chrome dies instantly ("Chrome process
  closed the pipe").
- **TLS-intercepting proxies**: this sandbox's egress proxy couldn't parse
  Chrome's TLS 1.3 ClientHello (post-quantum X25519MLKEM768 key share,
  ~1.7 KB). Fixed for full Chrome with `--ssl-version-max=tls1.2` (cert
  verification stays on); `headless_shell` doesn't wire up that switch, so its
  benchmarks used localhost pages. Also had to add the proxy CA to the NSS db
  (`certutil -d sql:$HOME/.pki/nssdb -A -t "C,," ...`). None of this applies
  outside sandboxed environments.
- `evaluate()` takes an **expression** (statements like a bare `throw` are a
  syntax error) and rejects with the page-side error — same contract as
  `shot-scraper javascript`.
- The API is experimental; method names or option shapes may change.

## Files

- [server.ts](server.ts) — the JSON API service (zero dependencies)
- [testpages.ts](testpages.ts) — local simple/heavy benchmark pages
- [bench.sh](bench.sh) — cgroup memory-limit benchmark harness
- [notes.md](notes.md) — full working notes including the netlog debugging
- [datasette.png](datasette.png), [example.jpg](example.jpg) — screenshots
  produced by the API
