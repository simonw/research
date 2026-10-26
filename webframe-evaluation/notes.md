# webframe evaluation – notes

Target: https://github.com/tractordev/webframe (commit 03155e2, 4 commits total, Oct 2026).
Compared against `shot-scraper javascript` (simonw/shot-scraper docs/javascript.md).

## Setup
- Go 1.27.1 needed (toolchain auto-downloaded by `go`). `make build` -> 5.3MB static-ish binary, CGO_ENABLED=0.
- `apt-get install libwebkit2gtk-4.1-0` (WebKitGTK 2.52.6 on Ubuntu 24.04). Xvfb was already installed.
- `./webframe examples/hello.html` worked first time. No DISPLAY -> it spawns its own private Xvfb.
- Startup ~0.5s for a local file, ~0.7s for https://example.com (through the HTTPS proxy; libsoup honours
  `https_proxy` and system CA store, so it Just Worked).

## Code read
- webframe.go (209 lines): thin wrapper over glaze (progrium fork of crgimenes/glaze, purego bindings to
  WebKitGTK/WKWebView/WebView2). Injects hooks.js as a user script, binds `window.__webframe` for messages.
- hooks.js: patches console.*, error/unhandledrejection listeners, window.close, and defines
  `window.__webframe_eval(id, src)` which does `(0, eval)(src)` *inside the page*, awaits, formats.
- glaze options webframe does NOT use: `Ephemeral`, `SandboxWebContent` (bwrap), `NoBridge`,
  `SchemeHandlers`, `OnNavigationStart`, `OnDownload` (default: cancel all downloads = good),
  `OnNewWindow` (default: drop popups = good), `OnMediaCapture` (default: deny all permissions = good).

## Observations from running it
- Formatting: Node-ish, cycles -> "[Circular]", bigint, symbols, functions OK.
- ORDERING BUG: `alert()` output (`[dialog] hi`) printed before console.log lines that ran earlier.
  Dialogs go through a synchronous native callback, console through the batched async bridge.
- Hidden mode: `innerWidth`/`innerHeight` are 0 unless sized. CLI only sizes via `-w WxH`, which also
  shows a window (harmless under Xvfb). Library Options.Width/Height sizes the hidden viewport.
- `-e` exits 1 on first exception. `-S` silences everything. `WF_FOO=x` -> `window.FOO`. All work.
- `-c` console: one line = one script. Multi-line scripts (shot-scraper style `() => {\n...\n}`) need
  collapsing to one line first.
- Eval timing: first eval on a slow-streaming page (1.5s pause mid-body) returned post-load
  ("complete"), i.e. evals appear to be serviced after the page finishes parsing. But a second eval
  doing `new Promise(r => addEventListener("load", r))` HANGS FOREVER: no eval timeout anywhere.
- CSP: on a page with `script-src 'self'` EVERY eval fails with EvalError ("Refused to evaluate a string
  as JavaScript because 'unsafe-eval'..."), because hooks.js runs user code via page-level eval.
  This makes -c / Eval useless on many real sites (GitHub etc). shot-scraper/Playwright don't have this
  problem for evaluate().
- Redirects followed; final URL visible. 404 pages load normally; no way to see the HTTP status.
- DNS failure: prints "Failed to load ...: Error resolving" to stderr but then HANGS (both with and
  without -c). Only `-e` makes it exit. A scraper needs -e or a timeout wrapper.
- SIGTERM with a page stuck in `while(true){}`: exits in ~16ms, WebKit child processes go away. Good.

## Sandbox-relevant probes
- Storage persists between runs: localStorage + cookies set in run 1 visible in run 2.
  Stored under ~/.local/share/webframe and ~/.cache/webframe. (glaze `Ephemeral` not used.)
- file:// page cannot read other file:// URLs (fetch/XHR/iframe all fail) – good.
- file:// page CAN fetch http://127.0.0.1:... – localhost / LAN / metadata endpoints reachable.
- Process isolation: WebKitWebProcess runs as root, same mount/net/pid namespaces as webframe,
  CapEff full, NoNewPrivs 0, Seccomp 0. i.e. NO bubblewrap sandbox (glaze SandboxWebContent is off
  by default with WebKitGTK 4.1 and webframe doesn't turn it on). Note the Dockerfile comment
  "keeps WebKit's sandbox on" – with webkit2gtk-4.1 that's only true if glaze asks for it.
- Memory: a page allocating 8MB chunks grew to ~2.8GB in 20s with no limit (needs cgroup/ulimit).
- Hostile page can overwrite `window.__webframe_eval` and answer every eval with a lie:
  `echo document.title | webframe -c spoof.html` -> "spoofed by page".
- Hostile page that sets `JSON.stringify = ...` breaks the bridge completely: no console output, no eval
  results, process hangs until killed.

## Exfiltration matrix (page loaded from file://, logging server on 127.0.0.1)
Vectors: fetch, XHR, sendBeacon, <img>, stylesheet, <script src>, WebSocket, EventSource, prefetch,
iframe, form POST, RTCPeerConnection, window.open, top-level `location.href = ...`.
- Plain page: 11 requests reached the server (all but prefetch, window.open, RTC).
  RTCPeerConnection doesn't exist (WebKitGTK disables WebRTC unless glaze OnMediaCapture is set),
  window.open is dropped by glaze.
- Page with `<meta http-equiv=CSP content="default-src 'none'; script-src 'unsafe-inline' 'unsafe-eval'">`:
  only the top-level navigation got out (CSP has no navigation control any more). glaze has
  OnNavigationStart (notification only) but no way to veto a navigation.
- `unshare -n webframe ...` (new empty network namespace; Xvfb is spawned inside it so the abstract
  X socket still works): ZERO requests, fetch to example.com -> "Load failed". Best option.
- bench.html: Total 485ms, score 206 (Xvfb, software rendering, this container).

## Patch to webframe (webframe.diff)
1. Options.Ephemeral -> glaze Ephemeral (CLI `-p`): verified localStorage/cookie from previous run
   NOT visible with -p, visible without.
2. Options.Sandbox -> glaze SandboxWebContent (CLI `-b`): verified WebKitWebProcess runs under
   /usr/bin/bwrap with its own net/mnt/pid namespaces, NoNewPrivs=1, Seccomp=2 (filter), rootfs with
   only dev etc lib lib64 proc sys tmp usr var (no /home). BUT network requests still succeed
   because the WebKitNetworkProcess is outside the sandbox and does the fetching. So -b protects
   against a renderer exploit, not against the page talking to the network. A harmless
   "Can't connect to a11y bus" warning appears on stderr even with -s.
3. Options.NavigationStart callback passthrough.
4. CSP fallback for Eval: hooks.js detects EvalError mentioning unsafe-eval and asks Go to re-run
   the source as `(function(){ return (SRC) })` injected via the web view's native evaluate
   (not subject to page CSP). Three dispatched scripts (begin/run/end) detect a parse failure
   when SRC is statements rather than an expression, and report a clear error instead of hanging.
   Verified on a `script-src 'self'` page: expressions, object literals, async arrow funcs work.
5. Hardening: hooks capture JSON.stringify / eval / Promise at document start and define
   __webframe_eval etc. as non-writable, non-configurable. The spoof page now gets "real title".
   Still broken: page replacing JSON.stringify kills glaze's own bridge (glaze's bridge JS calls
   JSON.stringify late-bound). Same-world design can't be made robust; WebKit "script worlds"
   (isolated worlds) would be the proper fix and glaze doesn't expose them.

## Prototype 1: wfjs (shot-scraper javascript clone, Go, uses patched library)
- Semantics copied from shot-scraper/Playwright: evaluate as global script; if the value is a
  function call it; await promises; JSON-encode; `-r` raw strings; exit 1 + "Error: ..." on throw.
- Waits for the `load` event first (Ready fires at document start).
- Hidden window = 0x0 viewport (GTK never allocates a size to an unmapped window, so the library's
  "Width/Height size the hidden viewport" doesn't hold on Linux). Fix in wfjs: when it owns the
  private Xvfb, use a real window (`Window: true`) - nobody can see it. Then --width/--height work.
- Native noise on stderr (JSC SIGUSR1 warning, libEGL DRI3 warnings): copied webframe's
  silence_linux init and dup'd fd 2 to /dev/null, keeping a private copy for our own errors.
- All shot-scraper javascript doc examples work: title, -r, viewport, object literal, multi-statement
  throw, CI-style `if (...) throw`, setInterval promise, stdin, Readability import from jsdelivr.
- CSP page (GitHub): eval wrapper fails with unsafe-eval -> retry as inlined expression -> works for
  expressions. Statements fail with a clear error. `import()` from a CDN is blocked by CSP and there
  is no --bypass-csp equivalent in WebKitGTK/glaze. shot-scraper handles statements on GitHub fine.
- --fail: WebKit has no PerformanceNavigationTiming.responseStatus and glaze NavigationEvent has no
  status code -> workaround re-fetches location.href with cache: "force-cache". First tried HEAD:
  my test server answered 501 -> false positive. Switched to GET.
- --wait-for EXPR polls every 100ms. --timeout (default 30s) is the only guard against hangs, e.g.
  https://no-such-host.invalid via the egress proxy: CONNECT 502 never surfaces as a load failure.
  Plain http DNS failure does surface ("Failed to load ...") and wfjs exits 1 immediately.
- Persistent profile per program name (~/.local/share/wfjs) with --persist; default is ephemeral.

### Timings vs shot-scraper 1.x (Playwright 1.63 + chrome-headless-shell), 3 runs each, `document.title`
- example.com: shot-scraper 1.40/1.19/1.37s, wfjs 1.17/1.10/1.08s
- datasette.io: shot-scraper 2.50/1.99/2.01s, wfjs 2.19/1.79/1.96s
- simonwillison.net: shot-scraper 3.62/1.94/1.94s, wfjs 2.05/2.81/2.22s
=> basically same ballpark; network dominates.
- RSS during a run: wfjs parent 165MB + WebKitWebProcess 339MB + NetworkProcess 63MB + Xvfb 74MB
  ~ 640MB vs Chrome headless shell processes + python ~ 456MB.

### Output differences vs shot-scraper
- `{b: undefined}`: shot-scraper gives `"b": null`, wfjs drops the key (JSON.stringify).
- `new Date(0)`: "1970-01-01 00:00:00+00:00" vs "1970-01-01T00:00:00.000Z".
- `document.body`: "ref: <Node>" vs {}.
- Non-ASCII: shot-scraper escapes (·), wfjs emits UTF-8.

## Prototype 2: wfsandbox.py (Python, drives patched CLI) + attacks.py
- One fresh `unshare -rn webframe -b -p -s -c sandbox.html` per call; sandbox.html has strict meta
  CSP, captures console in-page, defines a locked `__sandbox_run(code, nonce)` that returns
  nonce + JSON. Python scans stdout for the JSON string starting with the nonce.
- ~430-500ms per call (process + Xvfb + WebKit startup each time).
- attacks.py results (attacks-output.txt):
  - fetch localhost/internet, file://, img, beacon, websocket: blocked (CSP alone already does it)
  - top-level navigation exfil: GOT THROUGH with CSP only (request logged), blocked by netns
  - while(true){} and memory bomb: killed by timeout (memory grows unbounded until then)
  - storage persistence: none with -p, leaks between runs without -p
  - WF_SECRET_TOKEN in host env: stripped by wfsandbox (webframe would otherwise expose it)
  - forged eval result via window.__webframe bridge: webframe prints the forged value as the result
    of eval #1 (ids are sequential/guessable) and drops the real one; the nonce check turns this
    into "no result" (DoS) rather than a spoof.
  - replacing __sandbox_run: blocked by defineProperty.
  - window.close(): no result. alert/confirm: auto-dismissed (confirm -> false).
  - WebAssembly works.
- bwrap inside `unshare -rn` (user namespace) worked here.
