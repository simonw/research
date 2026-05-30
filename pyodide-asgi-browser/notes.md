# Pyodide ASGI in the browser — working notes

Goal: run a Python ASGI app (FastAPI) entirely in the browser via Pyodide, with a
service worker intercepting **all** GET/POST/fetch traffic for the app and routing it
through the ASGI protocol. Link navigations, form submissions and `fetch()` calls should
all be served by Python running in Pyodide — no real HTTP server behind them.

## Decisions (from clarifying questions)
- ASGI scope: **HTTP + lifespan** (FastAPI startup/shutdown events must fire).
- Architecture: **service worker** intercepts `fetch` → **MessageChannel** → **Pyodide in a
  dedicated Web Worker** runs the ASGI app → response flows back.
- Demo: FastAPI app exercising link navigation, a form POST (303 redirect), a JSON API hit
  via `fetch()`, and pages that run their **own inline JavaScript** (which itself does
  intercepted `fetch()` calls).
- App loading: app code inline, FastAPI installed at runtime via `micropip`.

## Environment findings
- Outbound network OK to `cdn.jsdelivr.net` (Pyodide) and `pypi.org` (micropip).
- Pyodide CDN versions available incl. 0.27.x, 0.28.x, 0.29.0. Using **0.28.3**.
- Playwright not preinstalled → `pip install playwright` + `playwright install chromium`.
- Service workers require a *secure context*; `http://localhost` / `127.0.0.1` counts, so a
  plain `python3 -m http.server` is fine for testing.

## Architecture worked out
Chicken-and-egg problem: the top page hosts the long-lived Pyodide worker, but if a
navigation replaced that page we'd lose Pyodide. Solution: **iframe**.

- `/` (`index.html`) = bootstrap **shell**. Registers the SW, spins up the Pyodide Web
  Worker, installs FastAPI, runs ASGI lifespan startup, wires a MessageChannel between SW
  and worker, then points an `<iframe>` at `/app/`.
- The FastAPI app is served under the **`/app/` prefix**. The SW intercepts every
  same-origin request whose path starts with `/app` and routes it to Pyodide. Everything
  else (`/`, `/sw.js`, `/worker.js`, `/bootstrap.js`, Pyodide CDN assets) passes through to
  the network.
- SW ↔ worker bridge: shell creates a `MessageChannel`, hands `port2` to the worker and
  `port1` to the SW. SW serializes each intercepted request `{id,method,url,headers,body}`,
  posts it over the port, and awaits `{id,status,headers,body}`.

### root_path probe (pure Python, before touching the browser)
Starlette 1.2.0 / FastAPI 0.136:
- With `scope["root_path"]="/app"` routing matches the un-prefixed route (`/about`) whether
  or not `scope["path"]` includes the prefix, and `request.url_for(...)` / `RedirectResponse`
  emit absolute URLs **including** `/app` (e.g. `http://host/app/thanks?name=Simon`).
- => Worker passes the *full* browser path as `scope["path"]` with `root_path="/app"`; the
  app code stays prefix-agnostic and all generated links stay inside the intercepted scope.
- Must forward the real request **host + scheme** into the scope so absolute URLs match the
  test origin (127.0.0.1:PORT).
- `Form(...)` needs **python-multipart** installed too.

## Gotcha: the browser sandbox has no outbound network
First browser run failed: the Pyodide Web Worker's `importScripts(...cdn.jsdelivr.net...)`
threw "failed to load", and a probe (`page.evaluate(fetch(...))`) showed even
`https://pypi.org` returns "Failed to fetch" from inside headless Chromium — while host
`curl`/`pip` reach both fine. So the host egresses via a transparent proxy that the browser
sandbox can't use.

**Fix:** vendor Pyodide + the wheel closure locally and serve from the same `http.server`
(the browser *can* reach `127.0.0.1`). `vendor.py` downloads:
- the Pyodide runtime (`pyodide.js`, `pyodide.asm.js/.wasm`, `python_stdlib.zip`, lock);
- the bundled dep closure for FastAPI (pydantic, pydantic_core wasm, anyio, idna, sniffio,
  typing_extensions, annotated_types) + micropip/ssl — so micropip resolves them from the
  *local* lock;
- `fastapi`, `starlette`, `python-multipart` wheels from PyPI (not bundled by Pyodide).
The worker loads Pyodide with `indexURL = vendor/` and `micropip.install([local wheel urls])`.
`vendor/` is gitignored (large, not committed); `vendor.py` reproduces it.

## Bridge robustness: service worker restarts
First cut held the SW↔worker `MessageChannel` port in service-worker memory. Service
workers are killed when idle, which would drop the port. Re-architected so the SW holds
**no** long-lived state: on every intercepted request it locates the shell window client
fresh via `clients.matchAll()`, hands it a one-shot reply `MessagePort`, and the shell
(which owns the long-lived Pyodide worker) brokers the request. Survives SW restarts.

## Debugging the "second request fails" symptom (the real bug)
Browser run: `test_home` passed but link-click, form POST and the widget's own fetch all
failed. Pattern was actually "parent `iframe.src` navigations work, in-iframe-initiated
requests fail". A targeted Playwright probe (console + requestfailed listeners + a manual
in-iframe fetch) showed:
- the iframe **is** SW-controlled and a manual relative `fetch('/app/api/items')` returns
  `200` — interception works fine;
- but the widget's inline JS contained `fetch("http://127.0.0.1/app/api/items")` — **no
  port** -> `ERR_CONNECTION_REFUSED`.

Root cause: a service worker cannot forward the browser-managed `Host` header, so the
bridge synthesizes one — and it was using the bare hostname without the port. So every
`request.url_for(...)` URL (the about link, the form action, the widget's fetch target)
came out port-less and pointed at `:80`. `test_home` only passed because it loaded `/app/`
via a relative `iframe.src` and never followed those generated links.

Fix: synthesize `Host` as `host:port` for non-default ports. Added a pure-Python
regression test (`test_synthesized_host_header_includes_nondefault_port`) first (red),
then fixed the harness (green). One-line root cause, caught by the e2e layer.

## TDD plan
1. Pure-Python unit tests of the ASGI bridge harness (scope building, receive/send,
   header/body round-trip) — fast, no browser. RED then GREEN.
2. Playwright tests driving real Chromium: SW serves navigation HTML, form POST redirect,
   JSON fetch, and a page whose own inline JS does an intercepted fetch. RED then GREEN.
