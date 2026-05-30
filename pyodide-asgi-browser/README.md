# Running Python ASGI apps in the browser via Pyodide + a service worker

This project captures the **entire server side** of a Python web application and moves it
into the browser. A FastAPI app runs inside [Pyodide](https://pyodide.org/), and a
**service worker** intercepts every same-origin request under `/app/` — link navigations,
form submissions and JavaScript `fetch()` calls alike — and answers them by speaking the
**ASGI** protocol to the Python app. There is no HTTP server behind `/app/`: the only thing
a real server does is hand over the static bootstrap files.

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
| `index.html` | Bootstrap shell page with the app `<iframe>` |
| `bootstrap.js` | Registers the SW, starts the Pyodide worker, brokers SW→worker requests |
| `sw.js` | Service worker; intercepts `/app/*` and proxies it to the shell/worker |
| `worker.js` | Pyodide Web Worker; embeds the ASGI bridge harness + FastAPI demo app + glue |
| `vendor.py` | Downloads Pyodide + the needed wheels into `vendor/` (gitignored) |
| `tests/test_bridge.py` | Pure-Python unit tests of the embedded ASGI harness (fast) |
| `tests/test_browser.py` | Playwright end-to-end tests driving real Chromium |

The Python that runs in the browser is embedded in `worker.js` as two `String.raw` blocks
(`ASGI_BRIDGE_PY`, `APP_PY`). The unit tests **extract those exact blocks** from `worker.js`
and exercise them directly, so the tested code and the shipped code cannot drift apart.

## The demo FastAPI app

- `GET /app/` — home page with links and a form (link navigation + form entry points).
- `GET /app/about` — a page reached by clicking a link.
- `POST /app/submit` — a form submission that returns a `303` redirect to `/app/thanks`.
- `GET /app/thanks?name=…` — the redirect target.
- `GET /app/widget` — a page that runs its **own inline JavaScript**, which `fetch()`es the
  JSON API (that fetch is itself intercepted and answered by FastAPI).
- `GET /app/api/items` — JSON API returning items seeded during lifespan startup.

## Testing (red/green TDD)

Two layers, both written test-first:

1. **`tests/test_bridge.py`** — pure Python, no browser. Verifies scope construction,
   lifespan startup, routing, the `/app` prefix on generated URLs, the `303` form redirect,
   JSON output, and 404s. Sub-second feedback.
2. **`tests/test_browser.py`** — Playwright + Chromium. Boots the real page, waits for
   Pyodide + FastAPI to be ready, then asserts that link navigation, the form POST/redirect,
   a JSON-driven page, and a page running its own `fetch()` are all served by Python in the
   browser.

```bash
pip install playwright pytest fastapi python-multipart
python3 -m playwright install chromium
python3 vendor.py                           # download Pyodide + wheels into ./vendor

python3 -m pytest tests/test_bridge.py      # fast unit layer (no browser)
python3 -m pytest tests/test_browser.py     # end-to-end in real Chromium
```

### Results

- `tests/test_bridge.py`: **8 passed** (scope building, lifespan startup, routing, the
  `/app` prefix on generated URLs, the `303` form redirect, JSON, 404, and the
  synthesized-`Host`-header-with-port regression).
- `tests/test_browser.py`: **4 passed** — home page served by Pyodide, link navigation,
  form POST→303 redirect, and a page running its own intercepted `fetch()`.

## Trying it by hand

```bash
cd pyodide-asgi-browser
python3 vendor.py            # once, to populate ./vendor
python3 -m http.server 8000
# open http://127.0.0.1:8000/ and wait for status "ready", then use the app in the iframe
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
