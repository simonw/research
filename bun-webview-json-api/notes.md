# Notes: Bun.WebView JSON API prototype

Goal: explore Bun 1.4's new `Bun.WebView` to build a JSON API service that:
- launches a browser/webview against a URL
- executes JavaScript in the page, returns the JSON result (like `shot-scraper javascript`)
- takes screenshots of loaded pages
- measure minimum RAM needed

## Log

- Installed Bun 1.4.0 via official installer (`curl -fsSL https://bun.sh/install | bash`).
- Cloned simonw/shot-scraper to /tmp/shot-scraper for reference.

## Getting Bun.WebView working on headless Linux

- `Bun.WebView` in Bun 1.4.0. Prototype methods: navigate, evaluate, screenshot, cdp,
  click, type, press, scroll, scrollTo, resize, goBack, goForward, reload, close,
  url, title, loading, onNavigated, onNavigationFailed.
- On Linux there is no native webview backend: `backend: "webkit"` is macOS only.
  Linux uses `backend: "chrome"` — Bun spawns Chrome/Chromium headless and drives it
  over the DevTools Protocol via `--remote-debugging-pipe`.
- Browser discovery: auto-detect, or `BUN_CHROME_PATH`, or `backend.path`.
  Extra flags via `backend.argv` (last-wins for duplicates).
- Default spawn flags per docs: `--remote-debugging-pipe --headless --no-first-run
  --no-default-browser-check --disable-gpu --user-data-dir=<temp>`.
- Chrome is spawned ONCE per Bun process; additional `new Bun.WebView()` instances
  become new tabs (Target.createTarget) in the same Chrome.
- Running as root: needed `--no-sandbox` in argv or Chrome dies instantly
  ("Chrome process closed the pipe").

### Sandbox networking rabbit hole (environment-specific, not Bun's fault)

- This container forces egress through a TLS-intercepting proxy (HTTPS_PROXY).
  Chrome needed `--proxy-server=$HTTPS_PROXY`.
- ERR_CONNECTION_RESET persisted. Netlog analysis (`--log-net-log`):
  - -202 (CERT_AUTHORITY_INVALID) errors were only Chrome's DoH probes to dns.google.
  - The real page failure was -101 (CONNECTION_RESET) *mid TLS handshake* after the
    ClientHello was sent (~1720 bytes — includes X25519MLKEM768 post-quantum key share).
- NSS user db (~/.pki/nssdb) was empty; installed proxy CA with
  `apt-get install libnss3-tools; certutil -d sql:/root/.pki/nssdb -A -t "C,," -n ccr-agent-proxy -i /root/.ccr/agent-proxy-ca.crt`.
  (Required for cert trust, but didn't fix the reset.)
- Tried `--disable-features=UseMLKEM/PostQuantumKyber/X25519MLKEM768/...` — no effect
  on this build (Chromium 141.0.7390.37, Playwright build).
- `--ssl-version-max=tls1.2` FIXED it: the proxy's MITM engine can't parse Chrome's
  TLS 1.3 ClientHello (likely the ML-KEM key share). Cert verification remains on.
  This flag is only needed inside this sandbox; real deployments don't need it.

## Prototype server

Wrote `server.ts`: Bun.serve with POST /javascript, POST /screenshot, GET /healthz.
Each request opens a `new Bun.WebView()` (a tab in the shared Chrome process),
navigates, evaluates/screenshots, then closes the view. This is naturally
concurrency-safe because `evaluate()` allows only one in-flight call *per view*,
and views are per-request.

Verified against real sites through the sandbox proxy (full chromium backend):
- `POST /javascript` on https://datasette.io/ with a `new Promise(done => ...)`
  script returns awaited JSON — same semantics as `shot-scraper javascript`.
- `POST /screenshot` returns PNG/JPEG/WebP (webp/jpeg with quality; Chrome backend).
- Page-side exceptions come back as `{"ok":false,"error":...}`;
  `evaluate()` takes an *expression* (statements like bare `throw` are a syntax error),
  matching shot-scraper.

## RAM benchmarking (cgroup v1 memory limits, binary search)

Method: run server inside /sys/fs/cgroup/memory/bwv with memory.limit_in_bytes set,
serve local test pages (a simple page and a "heavy" page with 500 DOM nodes, a
2000-arc canvas and a 100k-element JS array) from a separate bun process, run 9
requests (3x simple JS, 3x heavy JS, 3x heavy 1280x800 screenshot), record
memory.max_usage_in_bytes and pass/fail. No swap; exceeding the limit = OOM kill.

Results (min limit where all 9 requests repeatedly succeed):

| configuration | works | fails | peak |
|---|---|---|---|
| full chromium, JS+screenshots | 168 MB | 160 MB | ~176 MB unlimited |
| headless_shell, JS+screenshots | 104 MB | 96 MB | ~103 MB unlimited |
| headless_shell + trim flags*, JS+screenshots | 88 MB | 80 MB | |
| headless_shell + trim flags*, JS only | 56 MB | 48 MB | ~60 MB peak |

*trim flags: --no-zygote --renderer-process-limit=1 --js-flags=--max-old-space-size=32
 --disable-dev-shm-usage

- `--single-process` ran JS at 64 MB but screenshots of the heavy page failed even
  at 96 MB — unreliable (and officially unsupported), not pursued.
- PSS accounting of a warm full-chromium service tree: bun ~27 MB, Chrome tree
  ~260 MB across 10 processes (zygotes, GPU, network service, storage, renderers).
  headless_shell spawns far fewer helpers.
- headless_shell couldn't do external HTTPS through THIS sandbox's MITM proxy
  (--ssl-version-max is a Chrome-browser-layer switch not wired in headless_shell),
  so headless_shell benchmarks used localhost pages. Irrelevant outside the sandbox.

## Latency / concurrency

- Sequential: ~64 ms per /javascript request, ~308 ms per heavy /screenshot
  (includes tab create + navigate + capture + tab close each time).
- 8 concurrent /javascript: all ok in 193 ms total.
- 8 concurrent heavy /screenshot: all ok in 1.69 s total, valid PNGs.
- Earlier "hang" at 8 concurrent was a bug in my bash harness (bare `wait` also
  waited on the background server processes), not Bun.

## Gotchas learned

- bash: `pkill -f <pattern>` matched my own compound shell command twice (exit 144)
  because the pattern appeared in the shell's own cmdline. Use pkill -x.
- Screenshot height: full-page height came back 1280x713 for a 1280x800 viewport
  request on some pages (viewport metrics quirk, didn't dig further).
- One evaluate() in flight per view: concurrent evaluate on the SAME view throws
  ERR_INVALID_STATE (per docs) — per-request views avoid this entirely.
