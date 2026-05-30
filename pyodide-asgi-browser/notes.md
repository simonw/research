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

## Datasette (follow-up: "get Datasette working in that")
Datasette is itself an ASGI app, so it drops into the same bridge.

- **Feasibility probe (pure Python, reusing the embedded harness):** `Datasette(memory=True,
  settings={"base_url": "/app/"})`, add a memory DB + rows, `bridge.startup()` → lifespan
  works (Datasette supports the lifespan protocol). `base_url="/app/"` makes every generated
  link / static asset / `.json` URL root-relative under `/app/` (`/app/demo`,
  `/app/-/static/app.css`, `/app/demo/items`), so no `root_path` needed and everything stays
  in the intercepted scope. HTML + JSON both render real data.
- **Pyodide specifics:** set `num_sql_threads=0` (Pyodide has no threads; run SQLite inline).
- **Vendoring:** Datasette 0.65.2 is pure-Python. Its closure: ~15 bundled-in-Pyodide deps
  (jinja2, markupsafe, pyyaml, httpx/httpcore/h11/certifi, click, pluggy, anyio, ...) resolved
  from the local lock, + 14 non-bundled pure-Python wheels (datasette, aiofiles, asgi-csrf,
  asgiref, click-default-group, flexcache, flexparser, hupper, itsdangerous, janus, mergedeep,
  pip, python-multipart, uvicorn) downloaded from PyPI into `vendor/` with a `datasette.json`
  install manifest.
- **Refactor to share code:** moved the bridge harness into `bridge-python.js` and the JS
  plumbing into `worker-runtime.js`; `worker.js` (FastAPI) and `worker-datasette.js` both
  `importScripts` them and just supply their app + install manifest. `bootstrap.js` picks the
  worker from `self.ASGI_WORKER`; `datasette.html` sets it to `worker-datasette.js`.
- Tests: `test_datasette_bridge.py` (pure Python, 3) + `test_datasette_browser.py` (Playwright:
  home, db→table navigation, and an intercepted `fetch()` of the `.json` API).

## Datasette 1.0 alpha + root login (follow-up)
Upgraded the Datasette demo to the latest 1.0 alpha (`datasette==1.0a31`, pulls
`sqlite-utils` 4.0a1 too, so `vendor.py` downloads that set with `--pre`). Closure changed:
asgi-csrf/janus/flexparser dropped; asyncinject/sqlite-utils/sqlite-fts4/tabulate/
python-dateutil added.

**Log in as root (equiv. of `datasette --root`):** found in 1.0a source that
`Datasette.__init__` sets `self.root_enabled = False`, and `default_permissions/root.py`
only grants the root actor permissions when `datasette.root_enabled` is true (the `--root`
CLI flag sets it). So:
- set `ds.root_enabled = True`, and
- register a tiny plugin implementing the `actor_from_request` hook that returns
  `{"id": "root"}` for every request.

Why a hook instead of the real auth-token cookie flow: **a service worker cannot read the
`Cookie` request header** (forbidden header), so cookie-based sessions can't round-trip
through this bridge. The hook authenticates every request as root directly — no cookies.

**CSRF / POST:** 1.0a replaced asgi-csrf with `CrossOriginProtectionMiddleware`
(Sec-Fetch-Site/Origin based, Filippo Valsorda's algorithm). It allows unsafe methods when
the request looks same-origin (`Sec-Fetch-Site: same-origin`) OR carries no browser headers
at all. Both hold here (real same-origin fetch in the browser; header-less request in the
pure-Python tests), so POST writes are accepted. Verified end-to-end: `GET /-/actor.json` ->
`{"id":"root"}` and `POST /demo/items/-/insert` -> `201 {"ok": true}`.

## Fullscreen + URL #fragment routing (follow-up)
`datasette.html` makes the iframe fill the viewport (100vw/100vh, fixed) with a boot overlay
that hides via `#boot:has(#status[data-state="ready"])`. `bootstrap.js` gained opt-in
hash-routing (`self.ASGI_HASH_ROUTING`): on iframe `load` it mirrors the iframe path into the
parent `#fragment` (e.g. `/app/demo/items` -> `#/demo/items`), and a `hashchange` handler
drives the iframe from the parent URL. A `syncing` guard breaks the feedback loop. On boot the
starting path is read from the `#fragment`, so Datasette URLs are shareable/bookmarkable and
the back button works.

## Two follow-up Datasette fixes
- **`/-/jump` escaping the SW scope:** Datasette's `base.html` hardcodes the navigation-search
  ("Jump to") endpoint as `url="/-/jump"` without the base_url prefix (the line above it
  correctly uses `{{ urls.static(...) }}`). Its client-side `fetch("/-/jump?q=...")` therefore
  hits the origin root, outside `/app/`, so the SW ignores it. Cheap fix: a small ASGI
  middleware (`jump_base_url_fix`) rewrites `"/-/jump"` -> `"/app/-/jump"` in HTML responses
  (and drops the now-stale content-length). Real fix belongs upstream in Datasette.
- **Anti-clickjacking headers on write pages:** `views/query_helpers.py:_block_framing` sets
  `X-Frame-Options: DENY` + `Content-Security-Policy: frame-ancestors 'none'` on the
  execute-write page, which the browser enforces on the framed document (even though the SW
  synthesised it), blocking the iframe. Fix lives in the **service worker** (not a Datasette
  plugin): it strips `X-Frame-Options` and the `frame-ancestors` CSP directive from every app
  response. Rationale: the iframe requirement is imposed by *our* architecture, so neutralising
  frame-busting belongs at the bridge for any hosted app, not per-app. (Datasette's CodeMirror
  hides the raw textarea, so the browser test asserts it is *attached*, not visible.)

## TDD plan
1. Pure-Python unit tests of the ASGI bridge harness (scope building, receive/send,
   header/body round-trip) — fast, no browser. RED then GREEN.
2. Playwright tests driving real Chromium: SW serves navigation HTML, form POST redirect,
   JSON fetch, and a page whose own inline JS does an intercepted fetch. RED then GREEN.
