# Datasette Lite on the ASGI + service worker pattern

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

This ports [Datasette Lite](https://github.com/simonw/datasette-lite) over to the
"real ASGI app behind a service worker" pattern first prototyped in
[pyodide-asgi-browser](../pyodide-asgi-browser/). It also reviews that pattern,
fixes the problems I found in it, and has Playwright tests for every existing
Datasette Lite URL pattern plus a range of plugins, including ones that ship
JavaScript and ones that POST.

- **The change:** [`datasette-lite-asgi.diff`](datasette-lite-asgi.diff), against
  datasette-lite `main` @ `779b2d4`.
- **Live demo** (built from that diff by [`github-pages.sh`](github-pages.sh)):
  https://simonw.github.io/research/datasette-lite-asgi/demo/
  - [CSV + table page](https://simonw.github.io/research/datasette-lite-asgi/demo/?csv=tests/fixtures/items.csv#/data/items)
  - [datasette-cluster-map (JavaScript plugin)](https://simonw.github.io/research/datasette-lite-asgi/demo/?install=datasette-cluster-map&csv=tests/fixtures/places.csv#/data/places)
  - [datasette-write (POST + CSRF, signed in as root)](https://simonw.github.io/research/datasette-lite-asgi/demo/?root=1&install=datasette-write&csv=tests/fixtures/items.csv#/data/-/write)
  - [Datasette 1.0 alpha signed in as root](https://simonw.github.io/research/datasette-lite-asgi/demo/?ref=pre&root=1&csv=tests/fixtures/items.csv#/data/items)
- **Tests:** 70 tests: 13 pure-Python bridge tests and 57 Playwright/Chromium tests.
  All pass (see [Results](#results)).

## Before and after

Datasette Lite used to call `ds.client.get(path)` inside the worker and assign the
response HTML to `#output.innerHTML`. A click handler caught `<a>` clicks and GET
form submissions. So:

| | Old Datasette Lite | This version |
|---|---|---|
| `<script>` in pages (CodeMirror, table.js column menus, "Format SQL") | never runs | runs |
| Plugins that ship JS/CSS ([issue #8](https://github.com/simonw/datasette-lite/issues/8)) | broken | work (cluster-map, vega tested) |
| POST forms / `fetch()` POST | impossible | work, including CSRF |
| Cookies (sign-in, CSRF tokens, flash messages) | none | per-tab cookie jar |
| File uploads / downloads | no | multipart upload and `?_dl=1` download work |
| JSON pages | re-rendered as a `<pre>` | served as JSON |
| `#/path?query` URLs, back button | yes | yes |

## How it works

```
 index.html + shell.js (the "shell" page)            webworker.js (Web Worker)
 ┌──────────────────────────────────────┐   relay   ┌───────────────────────────┐
 │ owns the Pyodide worker ─────────────┼──────────►│ Pyodide                   │
 │                                      │◄──────────┼ Datasette ASGI app        │
 │ <iframe src="app/<session>/...">     │           │  + base_url fix middleware│
 │   real Datasette pages, JS, forms    │           │ ASGIBridge + CookieJar    │
 └──────┬─────────────────────▲─────────┘           └───────────────────────────┘
        │ every request        │ Response
        ▼                      │
 sw.js: "which tab owns <session>?" → posts the request to that tab, which relays it
```

1. `shell.js` picks a random per-tab `<session>` id, starts the Pyodide worker
   (same Pyodide 0.27.2, same data loading code as before), and registers `sw.js`.
2. The worker starts Datasette with `base_url = <scope>app/<session>/` and
   `num_sql_threads = 0`, then wraps `ds.app()` in an `ASGIBridge`.
3. The shell points a full-window iframe at `app/<session>/<path from #fragment>`.
4. Every request under that prefix (navigations, form posts, `fetch()`, static
   assets) hits the service worker. The SW asks each open shell window whether it
   owns `<session>`, then sends `{method, url, headers, body}` to the owner over a
   one-shot `MessageChannel`. The shell relays it to its worker, which runs one ASGI
   `http` cycle and returns `{status, headers, body}`. The SW builds a `Response`.
5. On every iframe `load` the shell `history.replaceState`s its own `#fragment` to
   the iframe's path, copies the title, opens external links in a new window, and
   injects the "Load SQLite DB by URL / CSV / JSON / Parquet / SQL" buttons.

## Review of the original pattern, and what I changed

What I kept from `pyodide-asgi-browser`: the SW keeps no state and re-finds the
shell window client on every request (so it survives SW termination), the iframe
keeps Pyodide alive across navigations, `base_url`, stripping `X-Frame-Options` /
`frame-ancestors`, and a middleware for Datasette's base_url bugs.

Problems I found in it, all fixed in the port:

1. **Multiple tabs got each other's responses.** `findShellClient()` returned the
   first non-app window. With two Datasette Lite tabs open (different `?url=`), one
   tab's requests could be answered by the other tab's Pyodide. I fixed this with
   the per-tab `<session>` in the URL prefix plus an "owner?" handshake.
   Test: `test_two_tabs_are_isolated`.
2. **No cookies.** A service worker can't read `Cookie` or set `Set-Cookie`
   (forbidden headers). The research version hard-wired an `actor_from_request`
   plugin to make everyone root. That breaks Datasette 0.65's `asgi-csrf` (it needs
   a cookie *and* a form token), `ds_messages` flash messages, `/-/logout`, and any
   plugin login. I added a **cookie jar in the Python bridge** that stores
   `Set-Cookie` (Path, Max-Age and Expires aware) and builds a `Cookie` header for
   each request. Root sign-in is now opt-in with `?root=1` and goes through
   Datasette's real `/-/auth-token?token=` flow, so `ds_actor` is an ordinary
   cookie that log out can clear.
3. **`raw_path` held the decoded path.** Datasette routes on `raw_path`, so table
   names that need percent or tilde encoding could route to the wrong place. The
   bridge now passes the raw URL path as `raw_path` and the decoded path as `path`.
4. **`receive()` sent `http.disconnect` straight after the body.** That is wrong
   for apps that watch for disconnect while streaming. It now waits until the
   response has finished.
5. **Hardcoded root-relative URLs escape the prefix.** The research version only
   special-cased `/-/jump`. Plugins do this too, for example
   datasette-auth-passwords' `<form action="/-/password-tool">`. Two fixes, because
   the SW only sees **navigations** whose *target* URL is inside its scope, while
   it sees *all* subresource fetches from pages it controls:
   - SW: a same-origin `/-/…` request from a Datasette page is routed back to that
     page's session. Subresources are identified by `event.clientId` and
     navigations by `request.referrer`. Navigations get a 307 so POST bodies
     survive.
   - Middleware: root-relative `/-/` URLs in HTML `href`/`action`/`src`/`formaction`
     attributes get the prefix. This is needed when Datasette Lite is served from a
     subdirectory (`test_served_from_subpath`).
6. **The double-`base_url` workaround was page-specific**, because a blanket
   `/app/app/` → `/app/` would break a database called `app`. I also found another
   instance: in Datasette 0.65 the **table filter form redirect** sends
   `Location: /app/X/app/X/data/items?...`, which gives a 404. The prefix now
   includes the random session id, so `prefix+prefix` can only ever be this bug.
   It is now collapsed everywhere: HTML, JSON and `Location`.
7. **The back button needed two presses per page.** The research version set
   `location.hash` after each iframe load, which adds a second history entry on top
   of the iframe's own. It now uses `history.replaceState`.
8. **Opening an app URL in a new tab** (middle-click) used to render it top-level
   with no Pyodide behind it. The SW now 302s top-level GET navigations to the
   owning shell's URL, with the same `?options`, plus `#path`. That boots a fresh
   Datasette Lite (`test_app_url_opened_in_new_tab_redirects_to_shell`).

Checked and fine without changes: hard reload (Shift-reload leaves the shell
uncontrolled, but the iframe's later navigation is still intercepted), and the SW
being killed mid-session (`ServiceWorker.stopAllWorkers` over CDP).

## Bugs found on `main` along the way (fixed in the diff)

- **`main` doesn't currently start at all.** The latest `platformdirs`, imported by
  Datasette 0.65.5's vendored pint, does `from os import geteuid`, and Pyodide 0.27.2
  has no `os.geteuid`. I added the shim `os.geteuid = os.getuid` before import.
  So lite.datasette.io is probably broken right now, independent of this work.
- **`?ref=pre` fails.** The old `httpx==0.23` pin pulls in Pyodide's idna 3.7, and
  1.0a41 needs `idna>=3.18`. Datasette now installs first, and the h11/httpx pins
  are only used as a fallback (`?ref=1.0a11` still passes).
- **`?ref=pre&csv=…` fails**: "Requested 'sqlite-utils==3.28', but 4.2.1 is
  already installed". It now uses whatever sqlite-utils is already installed.
- **`?parquet=` with timestamp columns** failed with a `KeyError` on the pandas
  `Timestamp` type in sqlite-utils. Those columns are now stored as ISO strings.
- **`?install=` conflicts.** datasette-upload-csvs and datasette-edit-schema pull
  in the latest sqlite-utils, which needs `click>=8.3.1`, while Datasette already
  installed Pyodide's click 8.1.7. micropip can't upgrade in place. Nothing has been
  imported yet at that point, so `install_resolving_conflicts()` uninstalls the
  conflicting package and retries.

## URL pattern compatibility

Every pattern in the Datasette Lite README is covered by a test
(`tests/test_url_patterns.py` and `tests/test_datasette_lite.py`):

| Pattern | Test(s) |
|---|---|
| default `fixtures.db` + `content.db` | `test_has_two_databases`, `test_navigate_to_database` |
| `?url=` (repeatable, `_2` suffixes) | `test_url_loads_database_named_from_filename`, `test_url_repeated_gets_suffixes`, `test_load_multiple_database_urls` |
| `?csv=` (type detection, `;` delimiter, repeat → `_1`) | `test_csv_with_type_detection`, `test_csv_semicolon_and_repeated` |
| `?json=` (list, largest nested list, keyed object → `_key`, NDJSON) | `test_json_variants` |
| `?sql=` (runs before CSV) | `test_sql_runs_before_csv` |
| `?parquet=` | `test_parquet` (network) |
| `?metadata=` JSON/YAML, default "Datasette Lite" about link | `test_metadata_json`, `test_metadata_yaml`, `test_default_metadata_about_link` |
| `?install=` (repeatable) | `test_install_plugin`, `test_install_multiple_plugins` |
| `?ref=1.0a11`, `?ref=pre` | `test_ref`, `test_ref_pre_installs_prerelease` |
| `?memory=1` | `test_memory` |
| GitHub `blob` / Gist URL rewriting | `test_github_and_gist_urls_rewritten`, `test_csv_from_github_blob_url` (network) |
| `#/path?query`, `#path` without a slash | `test_hash_with_query_string`, `test_hash_without_leading_slash` |
| navigation updates `#`, back/forward, editing `#` | `test_navigation_updates_hash_and_back_button`, `test_editing_hash_navigates` |
| GET forms, external links in a new window, Load buttons | `test_get_form_submission`, `test_external_links_open_in_new_window`, `test_load_buttons` |
| load errors shown | `test_load_error_is_displayed` |
| new: `?config=` (1.0a; refused with a clear error on 0.x), `?root=1` | `test_config_with_ref_pre`, `test_config_requires_1_0`, `test_root_sign_in_and_logout` |

`?analytics=off` and the Plausible calls are kept as they were. Page views are now
sent on iframe navigations.

## Plugins and POST flows tested

| Plugin / feature | What is exercised | Datasette |
|---|---|---|
| core JS | CodeMirror 5 (0.65) and 6 (1.0a), "Format SQL", table.js column cog menu | both |
| [datasette-cluster-map](https://datasette.io/plugins/datasette-cluster-map) | plugin JS + Leaflet from `/-/static-plugins/`, `fetch()` of `.json`, markers | 0.65 |
| [datasette-vega](https://datasette.io/plugins/datasette-vega) | React app, reads the export link (needs the double-prefix fix), draws a chart | 0.65 |
| [datasette-copyable](https://datasette.io/plugins/datasette-copyable) | custom output renderer `.copyable` | 0.65 |
| [datasette-packages](https://datasette.io/plugins/datasette-packages), haversine, statistics | `register_routes`, SQL functions | 0.65 |
| core `/-/logout` | POST + asgi-csrf token + cookie deletion + `ds_messages` | 0.65 |
| core `/-/messages` | root-only POST debug form | 0.65 |
| [datasette-write](https://datasette.io/plugins/datasette-write) | POST form + CSRF + redirect + flash message; "Permission denied" without root | 0.65 |
| [datasette-auth-passwords](https://datasette.io/plugins/datasette-auth-passwords) | login POST, bad password, `ds_actor` cookie → `/-/actor.json`; password tool form with hardcoded `action="/-/password-tool"` | 0.65 |
| [datasette-upload-csvs](https://datasette.io/plugins/datasette-upload-csvs) | multipart file upload through the plugin's XHR JS, background `asyncio` task, progress polling | 0.65 |
| [datasette-edit-schema](https://datasette.io/plugins/datasette-edit-schema) | POST rename table → redirect | 0.65 |
| 1.0 alpha JSON write API | `fetch()` POST `/-/insert` and `/-/create` (201), with CrossOriginProtection | 1.0a41 |
| 1.0 alpha "Jump to" | hardcoded `/-/jump` `fetch()` routed back to the session | 1.0a41 |
| downloads | `.csv?_dl=1` → a real browser download | 0.65 |

## Gaps and potential problems

These remain open.

**Platform requirements**

- **Service workers are required.** Firefox private windows, and some locked-down
  or embedded browsers, have none. Old Datasette Lite worked there. This version
  shows an error instead. A fallback to the old innerHTML mode is possible but
  means keeping two code paths.
- **A secure context is required**: `https:` or `localhost`. Loading over plain
  `http://` from a LAN IP won't work.
- **Embedding Datasette Lite in a third-party iframe.** Service workers in
  cross-site iframes are storage-partitioned, and some browsers (Safari especially)
  can refuse to register them. I haven't tested this.

**URL handling**

- **Hardcoded URLs are only partly handled.** Root-relative `/-/…` URLs are routed
  correctly (SW + HTML attribute rewrite). Still not handled:
  - root-relative URLs outside `/-/` (e.g. a plugin linking to `/db/table` without
    `urls.*`);
  - JS that sets `location.href = "/-/…"` when deployed at a subpath;
  - URLs split across streamed body chunks: the rewrite is per-chunk, and Datasette
    usually sends HTML in one chunk.
- **The HTML rewriting is byte-level regex.** It could alter text that looks like a
  `href="/-/…"` attribute. Datasette escapes quotes in cell values, so table data
  is safe in practice.
- **Absolute URLs Datasette generates contain the session id.** Examples are JSON
  `next_url` and copy-able links. They only work while that tab is open. After that
  the SW redirect falls back to a Datasette Lite with default options, because the
  `?url=` options are known only to the owning tab.

**Cookies**

- **The cookie jar is Python-side only.** JavaScript `document.cookie` can't see
  those cookies, and cookies set from JS never reach Datasette. Cookies, like data,
  are lost on reload.
- **The jar ignores `Domain`, `Secure` and `SameSite`.** That is fine for a single
  origin, but not a full RFC 6265 implementation.

**Security**

- **Stripping `X-Frame-Options` / `frame-ancestors` for every app response is
  deliberate**, because the app has to live in an iframe. It means Datasette's
  anti-clickjacking protection for write pages is gone. A hostile site can't frame
  `app/<session>/` usefully, since the SW only answers for an owning shell in the
  same origin, but this deserves thought.
- **`?root=1` is opt-in, so default behaviour is unchanged.** Links that use it
  give full write access to the in-browser copy. The data is ephemeral, but a
  malicious link could combine `?root=1&install=<package>`. `?install=` already
  allowed arbitrary code, so this isn't new.

**Performance and robustness**

- **Everything is buffered.** Each request and response body is copied several
  times: WASM heap → JS → transfer → SW `Response`. Large CSV exports or database
  downloads are held fully in memory, and `_stream=on` isn't really streamed.
- **No caching of static assets.** Every `/-/static/*.js` and `.css` round-trips
  through Pyodide on each page view. The SW could cache them (they're versioned).
  This would be an easy win.
- **The SW waits indefinitely** for the shell's reply. If the Pyodide worker
  crashes outright, iframe requests hang instead of failing. Python exceptions
  are caught and return a 500. A timeout or `worker.onerror` handling should be
  added.
- **There is no message protocol version** between `shell.js` and `sw.js`.
  `skipWaiting()` + `clients.claim()` means a new deploy's SW immediately serves
  tabs still running the old `shell.js`. Fine for now; a version field would make
  future changes safer.

**Plugin and Datasette compatibility**

- **Plugins must be Pyodide-compatible**: pure Python or available as Pyodide
  packages, and no threads, subprocesses or sockets. `install_resolving_conflicts`
  helps with version clashes, but could in theory swap a dependency in a way that
  breaks something else.
- **WebSockets aren't bridged.** Datasette doesn't use them.
- **Old Datasette versions.** `?ref=` versions that predate a working `base_url`
  would break URL generation. Only the default (0.65.5), `1.0a11` and `pre`
  (1.0a41) were tested; `0.64.x` and earlier were not.

## Testing

```bash
cd datasette-lite                       # with datasette-lite-asgi.diff applied
uv run --python 3.12 --with-requirements dev-requirements.txt python -m playwright install
uv run --python 3.12 --with-requirements dev-requirements.txt python -m pytest
# skip tests that hit third-party hosts beyond Pyodide/PyPI/default DBs:
uv run ... python -m pytest -m "not network"
```

`CHROMIUM_EXECUTABLE=/path/to/chrome` uses an existing Chromium. If the browser
needs an HTTPS proxy, set `DATASETTE_LITE_TEST_PROXY`; `HTTPS_PROXY` is also
picked up. In this sandbox Chromium could only reach PyPI when
`--proxy-server=https=…` was passed explicitly.

Test files:

- `tests/test_bridge.py` (13): pure-Python tests of the bridge, cookie jar and
  middleware, using the exact `String.raw` Python extracted from `webworker.js`.
  Includes a real Datasette 0.65 run of root login → asgi-csrf-protected logout
  POST → flash message.
- `tests/test_datasette_lite.py` (5): the original tests, moved into the iframe.
- `tests/test_url_patterns.py` (26): every URL pattern, using small local fixture
  files.
- `tests/test_plugins_and_post.py` (26): JS, plugins, POST, cookies,
  upload/download, 1.0a, multiple tabs, new tab, SW restart, hard reload, subpath
  deployment.

### Results

Final full run: **70 passed in 8m21s** (Chromium, real Pyodide 0.27.2 from the
CDN, real PyPI installs):

| File | Passed |
|---|---|
| `test_bridge.py` | 13 |
| `test_datasette_lite.py` | 5 |
| `test_url_patterns.py` | 26 |
| `test_plugins_and_post.py` | 26 |

Each browser test boots its own Pyodide and Datasette, which takes 8 to 15 seconds,
plus plugin installs.

## Files

- `datasette-lite-asgi.diff`: `git diff main` of the modified datasette-lite repo.
  It adds `sw.js` and `shell.js`, rewrites `index.html` and `webworker.js`, adds
  the tests and fixtures, updates the README, and deletes the now-unused `app.css`.
- `github-pages.sh`: clones datasette-lite at the base commit, applies the diff,
  and copies the app into `demo/` for GitHub Pages.
- `notes.md`: working notes.
