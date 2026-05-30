# Running Python ASGI apps in the browser via Pyodide + a service worker

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

This project captures the **entire server side** of a Python web application and moves it
into the browser. An ASGI app runs inside [Pyodide](https://pyodide.org/), and a
**service worker** intercepts every same-origin request under `/app/` — link navigations,
form submissions and JavaScript `fetch()` calls alike — and answers them by speaking the
**ASGI** protocol to the Python app. There is no HTTP server behind `/app/`: the only thing
a real server does is hand over the static bootstrap files.

Two apps run on the **same** bridge:

- **FastAPI demo** (`index.html`) — links, a form POST→303 redirect, a JSON API, and a page
  that runs its own intercepted `fetch()`.
- **Datasette** (`datasette.html`) — the full [Datasette](https://datasette.io/) ASGI app,
  with database/table navigation, SQL, and its `.json` API, all answered in the browser.

```
 Browser tab
 ┌────────────────────────────────────────────────────────────────────────┐
 │  Shell page  (index.html + bootstrap.js)                                 │
 │    • registers the service worker                                        │
 │    • starts the long-lived Pyodide Web Worker                            │
 │    • brokers each captured request from the SW to the worker             │
 │    • shows the app in an <iframe src="/app/">                            │
 │                                                                          │
 │   ┌─────────────────┐   postMessage + reply port   ┌──────────────────┐  │
 │   │  Service worker │  ───────────────────────────>│  Shell (window)  │  │
 │   │  (sw.js)        │   (SW finds the shell via     │   ──┐            │  │
 │   │  intercepts     │    clients.matchAll each      │     │ MessageChan│  │
 │   │  fetch() for    │<───────────────────────────── │   <─┘ to worker  │  │
 │   │  /app/*         │       request response        └────────┬─────────┘  │
 │   └─────────────────┘                                        v            │
 │            ▲  intercepts navigations / forms / ┌──────────────────────┐   │
 │            │  fetch from the iframe            │  Web Worker          │   │
 │   ┌────────┴───────────────────────────────┐  │  Pyodide + FastAPI   │   │
 │   │  <iframe src="/app/">  the FastAPI app  │  │  + ASGIBridge        │   │
 │   └─────────────────────────────────────────┘ └──────────────────────┘   │
 └────────────────────────────────────────────────────────────────────────┘
```

## How a request flows

1. The user clicks a link, submits a form, or a page's JS calls `fetch()` inside the
   `/app/` iframe.
2. The **service worker** (`sw.js`) sees the `fetch` event. If the URL is same-origin and
   under `/app`, it serializes `{method, url, headers, body}`, locates the shell window
   client and posts it there with a one-shot reply port; otherwise it falls through to the
   network.
3. The **shell** relays the request over its private `MessageChannel` to the **Web Worker**
   (`worker.js`), which builds an ASGI `http` scope and drives the FastAPI app via the
   `ASGIBridge` harness, collecting the `http.response.start` / `http.response.body` messages.
4. The response `{status, headers, body}` flows back through the shell to the SW, which turns
   it into a real `Response`; the browser renders it — including following `303` redirects
   and running any inline `<script>` in returned HTML.

## Design decisions

- **Service worker + Pyodide in a dedicated Web Worker**, with the shell page brokering each
  request between them (see the SW-restart note below for why the SW keeps no port itself).
- **An `<iframe>` hosts the app.** The shell page owns the long-lived Pyodide worker, so the
  app must live in an iframe — otherwise an in-app navigation would replace the page and
  destroy the Python runtime. The iframe keeps Pyodide alive across navigations.
- **The app is served under the `/app/` prefix** using the ASGI `root_path` mechanism. The
  SW intercepts `/app/*`; everything else (`/`, `sw.js`, `worker.js`, `bootstrap.js`, the
  local `vendor/` assets) passes through. A short pure-Python probe confirmed that with
  `scope["root_path"] = "/app"`, Starlette routes on the un-prefixed path **and**
  `request.url_for(...)` / `RedirectResponse` emit URLs that include `/app`, so every
  generated link, form action and redirect naturally stays inside the intercepted scope —
  the FastAPI app code itself stays completely prefix-agnostic.
- **HTTP + lifespan.** `ASGIBridge.startup()` runs the ASGI lifespan protocol once, so
  FastAPI's lifespan/startup events fire (the demo seeds in-memory data there). Each HTTP
  request is then a fresh `app(scope, receive, send)` cycle.
- **FastAPI installed at runtime** via micropip (`python-multipart` is required for
  `Form(...)`). The Python source for the bridge and the demo app is embedded inline in
  `worker.js`.
- **The service worker holds no long-lived state.** It would be tempting to cache a
  `MessageChannel` port to the worker, but service workers are killed when idle and the port
  would be lost. Instead, on *every* request the SW finds the shell window client fresh via
  `clients.matchAll()` and hands it a one-shot reply port; the shell (which owns the
  long-lived Pyodide worker) brokers the request. This survives SW restarts.

### Vendoring (Pyodide + wheels served locally)

In the sandbox this was built in, the *browser* had no outbound network even though the
host did — so loading Pyodide / wheels from a CDN failed inside the Web Worker. `vendor.py`
downloads the Pyodide runtime, FastAPI's bundled dependency closure (so micropip resolves
pydantic/anyio/etc. from the **local** Pyodide lock), and the three non-bundled pure-Python
wheels (`fastapi`, `starlette`, `python-multipart`) plus FastAPI 0.136's extra deps
(`annotated-doc`, `typing-inspection`) into `vendor/`. The worker loads Pyodide with
`indexURL = vendor/` and installs the wheels from local URLs. This also makes the demo fully
offline-capable. `vendor/` is gitignored; run `python3 vendor.py` to reproduce it.

## Files

| File | Role |
|------|------|
| `index.html` / `datasette.html` | Bootstrap shells (FastAPI demo / Datasette) with the app `<iframe>` |
| `bootstrap.js` | Registers the SW, starts the chosen Pyodide worker, brokers SW→worker requests |
| `sw.js` | Service worker; intercepts `/app/*` and proxies it to the shell/worker |
| `bridge-python.js` | The shared ASGI bridge harness (`ASGI_BRIDGE_PY`), used by both workers |
| `worker-runtime.js` | Shared worker runtime: loads Pyodide, installs wheels, serves requests |
| `worker.js` | FastAPI Web Worker (inline FastAPI app + glue) |
| `worker-datasette.js` | Datasette Web Worker (inline Datasette setup + glue) |
| `vendor.py` | Downloads Pyodide + the needed wheels into `vendor/` (gitignored) |
| `tests/test_bridge.py` | Pure-Python unit tests of the FastAPI bridge path |
| `tests/test_datasette_bridge.py` | Pure-Python unit tests of the Datasette bridge path |
| `tests/test_browser.py` | Playwright end-to-end tests (FastAPI) in real Chromium |
| `tests/test_datasette_browser.py` | Playwright end-to-end tests (Datasette) in real Chromium |

The Python that runs in the browser lives in `String.raw` blocks (`ASGI_BRIDGE_PY` in
`bridge-python.js`; `APP_PY` in `worker.js`; `DATASETTE_PY` in `worker-datasette.js`). The
unit tests **extract those exact blocks** and exercise them directly, so the tested code and
the shipped code cannot drift apart.

## The demo FastAPI app

- `GET /app/` — home page with links and a form (link navigation + form entry points).
- `GET /app/about` — a page reached by clicking a link.
- `POST /app/submit` — a form submission that returns a `303` redirect to `/app/thanks`.
- `GET /app/thanks?name=…` — the redirect target.
- `GET /app/widget` — a page that runs its **own inline JavaScript**, which `fetch()`es the
  JSON API (that fetch is itself intercepted and answered by FastAPI).
- `GET /app/api/items` — JSON API returning items seeded during lifespan startup.

## Datasette on the same bridge

`datasette.html` runs the full **Datasette 1.0 alpha** ASGI app through the identical bridge —
proving the mechanism isn't FastAPI-specific. Details:

- **`base_url="/app/"`** — Datasette's own setting for running under a path prefix makes every
  link, static asset and `.json` URL root-relative under `/app/`, so they all stay inside the
  intercepted scope. (No ASGI `root_path` needed; the bridge passes the full path through.)
- **`num_sql_threads=0`** — Pyodide has no threads, so SQLite runs inline on the event loop.
- **Logged in as root** — the demo is the in-browser equivalent of `datasette --root`: it sets
  `ds.root_enabled = True` (without which the root actor is denied) and registers a tiny
  `actor_from_request` plugin that returns `{"id": "root"}` for every request. A service worker
  cannot read the `Cookie` header, so cookie sessions can't round-trip the bridge — the hook
  authenticates directly instead. This unlocks Datasette's **write/POST features** (create
  table, insert/edit rows, …). 1.0a's new Sec-Fetch-Site-based CSRF passes because the
  requests are genuinely same-origin.
- **Fullscreen + shareable URLs** — the iframe fills the viewport, and `bootstrap.js`
  hash-routing mirrors the in-app location into the parent URL `#fragment`
  (`/app/demo/items` ↔ `#/demo/items`), restoring it on load so Datasette URLs are
  bookmarkable and the back button works.
- **Vendoring** — Datasette 1.0a is pure-Python. Its heavy/compiled deps (MarkupSafe, PyYAML,
  Jinja2, httpx, …) and the `sqlite3` package (unvendored from the stdlib in Pyodide) come
  from the local Pyodide lock; ~13 pure-Python wheels (datasette, sqlite-utils, asyncinject,
  uvicorn, …) are fetched from PyPI (with `--pre`) into `vendor/` with a `datasette.json`
  manifest.

The Datasette demo seeds an in-memory `demo` database with an `items` table, then lets you
navigate database → table pages, run SQL, insert rows, and hit `/app/demo/items.json` — all
answered in the browser.

Two leaks of the iframe/prefix model that needed handling (the general lesson: prefix-scoped
interception assumes the app honours `base_url` for *every* URL, which isn't guaranteed):

- **Hardcoded client-side URL.** Datasette's `base.html` hardcodes the "Jump to" search
  endpoint as `url="/-/jump"` *without* the `base_url` prefix, so its `fetch()` escapes the
  `/app/` scope. A small Datasette-side ASGI middleware rewrites `"/-/jump"` → `"/app/-/jump"`
  in HTML responses. (Genuinely a Datasette bug — to be fixed upstream.)
- **Frame-busting headers.** Datasette's write/execute-write pages send `X-Frame-Options: DENY`
  and `Content-Security-Policy: frame-ancestors 'none'`, which the browser enforces on the
  framed document and would block. The **service worker** strips `X-Frame-Options` and the
  `frame-ancestors` CSP directive from every app response — the iframe requirement is imposed
  by this mechanism, so neutralising frame-busting belongs at the bridge, not in a per-app
  plugin.
- **Double-applied `base_url`.** Datasette builds table/row/query export links as
  `urls.path(path_with_format(request=request, …))`; `path_with_format` derives from
  `request.path` (which already includes `base_url`) and `urls.path()` then prepends it again,
  yielding `/app/app/…` — a 404. The HTML-rewrite middleware collapses `/app/app/` → `/app/`.
  (Also a genuine Datasette bug, to be fixed upstream.)

## Testing (red/green TDD)

Two layers per app, all written test-first:

1. **Pure-Python unit tests** (no browser): extract the embedded harness/app blocks and drive
   the ASGI app directly. Verify scope construction, lifespan startup, routing, the `/app`
   prefixing of generated URLs, the `303` form redirect, JSON, 404s, and — for Datasette —
   `base_url` prefixing and query-string passthrough returning real rows. Sub-second feedback.
2. **Playwright + Chromium end-to-end tests**: boot the real page, wait for Pyodide + the app
   to be ready, then assert link navigation, the form POST/redirect, JSON-driven pages, and
   page-initiated `fetch()` are all served by Python in the browser.

```bash
pip install playwright pytest fastapi python-multipart
pip install --pre datasette              # 1.0 alpha (used by the unit tests too)
python3 -m playwright install chromium
python3 vendor.py                        # download Pyodide + all wheels into ./vendor

python3 -m pytest tests/                  # everything (unit + browser, both apps)
```

### Results

All **27 tests pass**:

- `tests/test_bridge.py`: **8** (FastAPI bridge — incl. the synthesized-`Host`-header-with-port
  regression).
- `tests/test_browser.py`: **4** (FastAPI in Chromium — home, link nav, form→303, own `fetch()`).
- `tests/test_datasette_bridge.py`: **7** (Datasette bridge — prefixed links, table data,
  query-string passthrough, root login, an authorized POST insert, the `/-/jump` rewrite, and
  the double-`base_url` export-link fix).
- `tests/test_datasette_browser.py`: **8** (Datasette in Chromium — home, db→table navigation,
  intercepted `.json` `fetch()`, prefixed+intercepted `/-/jump`, the json export link resolving,
  the execute-write page rendering despite its frame-busting headers, `#fragment` hash-routing,
  and root + POST write).

## Trying it by hand

```bash
cd pyodide-asgi-browser
python3 vendor.py            # once, to populate ./vendor
python3 -m http.server 8000
# FastAPI demo:  http://127.0.0.1:8000/
# Datasette:     http://127.0.0.1:8000/datasette.html
# wait for status "ready", then use the app in the iframe
```

## Limitations / notes

- **HTTP + lifespan only** — no WebSocket scope (it was out of scope for v1).
- Requests are non-streaming: the request body is delivered as a single
  `http.request` message and the response is buffered before being returned.
- The bridge keeps **one** Pyodide worker (single-threaded). Concurrent requests are
  interleaved on Pyodide's asyncio event loop, which is fine for a demo but not tuned for
  heavy concurrency.
- A secure context is required for service workers; `http://localhost` / `127.0.0.1`
  qualifies, so a plain `python3 -m http.server` is enough for local testing.
