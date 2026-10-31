# Datasette Lite → ASGI service-worker pattern: working notes

Goal: port https://github.com/simonw/datasette-lite from its current
"ds.client.get() + innerHTML" approach to the service worker + Pyodide ASGI
bridge pattern from `research/pyodide-asgi-browser`, keeping every existing URL
pattern working, with comprehensive Playwright tests (incl. JS plugins and
plugins that POST).

## Starting point

- datasette-lite `main` @ 779b2d4. index.html + webworker.js (Pyodide 0.27.2 from CDN).
  Navigation = click-capture on `<a>`, `ds.client.get(path, follow_redirects=True)`,
  response text dumped into `#output` via innerHTML. Consequences: no `<script>`
  ever runs (no CodeMirror, no table.js, no JS plugins - issue #8), only GET forms
  work, POST impossible, JSON is re-rendered as a `<pre>`.
- URL patterns to keep: `?url=` (repeatable, name collision suffixes `_2`),
  `?csv=` `?json=` `?parquet=` `?sql=` (repeatable, into `data.db`), `?metadata=`,
  `?install=` (repeatable), `?ref=` (version or `pre`), `?memory=1`,
  `?analytics=off`, GitHub/Gist URL rewriting, `#/path?query` routing,
  default fixtures.db + content.db when no data given.
- Stable Datasette on PyPI is now 0.65.5, latest prerelease 1.0a41.

## Environment

- Chromium launched via Playwright needs `proxy={server: $HTTPS_PROXY}` to reach
  pypi.org / files.pythonhosted.org (those hosts are in NO_PROXY and the direct
  route presents a cert Chromium doesn't trust). With the proxy, CDN + PyPI work.

## Pre-existing breakage found on main

`main` currently fails to start at all (checked with an unmodified worktree):
latest `platformdirs` (pulled in by Datasette 0.65.5's vendored pint) does
`from os import geteuid`, which Pyodide 0.27.2's `os` lacks →
`ImportError: cannot import name 'geteuid'`. Fixed with a shim
`os.geteuid = os.getuid` before importing Datasette.
- Also pre-existing: `?ref=pre` (1.0a41) fails to install because the
  `httpx==0.23` pin pulls in Pyodide's bundled idna 3.7 and 1.0a41 needs
  `idna>=3.18`. Now: install Datasette first, fall back to the old h11/httpx
  pins only when resolution fails (keeps `?ref=1.0a11`/older working).
- Also pre-existing: `?ref=pre&csv=...` failed with "Requested
  'sqlite-utils==3.28', but sqlite-utils==4.2.1 is already installed". Now
  only installs sqlite-utils 3.28 if it isn't already importable.

## Review of the research pattern (pyodide-asgi-browser) before porting

Kept: SW intercepts a URL prefix and asks the *shell window* (which owns the
Pyodide worker) to answer, holding no SW state; iframe keeps Pyodide alive;
`base_url` for Datasette; strip `X-Frame-Options`/`frame-ancestors`;
base_url-bug middleware (`/-/jump`, double-prefixed export links, unprefixed
redirects); `#fragment` mirroring.

Problems found in the research version, fixed in the port:

1. **Multiple tabs share one SW** - `findShellClient()` returned the *first*
   non-app window. Two Datasette Lite tabs with different `?url=` data would get
   each other's responses. Fix: per-tab random session id in the prefix
   (`/app/<session>/`, used as Datasette's base_url). SW asks every shell "do you
   own <session>?" (cheap, no body), then sends the request to the owner only.
2. **No cookies at all** - a SW can't read `Cookie` or set `Set-Cookie`
   (forbidden headers), and the research worked around it by hard-wiring an
   `actor_from_request` root plugin. That breaks Datasette 0.65's asgi-csrf
   (cookie + form token), `ds_messages` flash messages, `/-/logout`, and any plugin
   login (datasette-auth-passwords). Fix: a cookie jar inside the Python bridge -
   store `Set-Cookie` from responses (path/Max-Age/Expires aware), synthesise
   `Cookie` on each request. Root sign-in (`?root=1`) now uses Datasette's real
   `/-/auth-token?token=` flow through the bridge so it is a normal ds_actor
   cookie that `/-/logout` can clear.
3. **`raw_path`** was set to the decoded path. Datasette routes on `raw_path`,
   so table names with spaces/`%`/tilde-encoding could mis-route. Now raw path
   from the URL is passed as `raw_path`, decoded one as `path`.
4. **ASGI receive()** returned `http.disconnect` immediately after the body;
   apps that listen for disconnect while streaming would abort. Now blocks until
   the response completes.
5. **Hardcoded root-relative URLs** that ignore base_url escape the prefix.
   The research only special-cased `/-/jump` in HTML. Plugins do this too:
   datasette-auth-passwords' `action="/-/password-tool"`, datasette-vega's
   `/-/static-plugins/datasette_vega/...` asset URLs. Fix in the SW: a
   same-origin request for `/-/...` coming from a Datasette page (subresource:
   `event.clientId` → client URL; navigation: `request.referrer`) is routed to
   that page's session (navigations get a 307 so POST bodies survive).
6. **Back button double entries** - research set `location.hash` after each
   iframe load (an extra history entry per navigation). Now
   `history.replaceState` since the iframe navigation already made an entry.
7. **Opening an app URL in a new tab** (middle click) would render it top-level
   with no Pyodide of its own. SW now 302s top-level navigations to
   `<owner shell URL with same ?options>#<path>`, booting a fresh Datasette Lite.

Verified OK without changes: hard reload (Shift-reload makes the shell
uncontrolled, but the iframe's later navigation is still intercepted), and SW
termination mid-session (CDP `ServiceWorker.stopAllWorkers`).
- `?parquet=` with timestamp columns failed (`KeyError: pandas Timestamp` in
  sqlite-utils 3.28 `COLUMN_TYPE_MAPPING`); now converted to ISO strings. (Can't
  confirm on main since main doesn't boot at all.)

## Bugs found by the browser tests during the port

- Datasette 0.65 table **filter form redirect** double-applies base_url in the
  `Location` header (`/app/X/app/X/data/items?name__exact=...`) → 404. Since the
  prefix now contains a random session id, a doubled prefix is unambiguous, so
  the middleware now collapses `prefix+prefix` everywhere (HTML, JSON, Location)
  instead of the research version's "only this page's own links" heuristic
  (which existed because `/app/app/` is legit for a database called `app`).
- **Subpath deployments** (e.g. GitHub Pages `/datasette-lite/`): the SW only
  receives *navigations* whose target URL is inside its scope (subresource
  fetches from a controlled page always reach it). So a hardcoded
  `action="/-/password-tool"` POST escaped completely when served from `/sub/`.
  Fix: the Python middleware also rewrites root-relative `/-/` URLs in
  `href`/`action`/`src`/`formaction` attributes of HTML responses. JS that does
  `location.href = "/-/..."` at a subpath is still a gap. Test:
  `test_served_from_subpath` (symlinked `/sub/` dir).
- `?install=` of plugins that depend on newer releases of packages Datasette
  already pulled from Pyodide's lock failed: datasette-upload-csvs and
  datasette-edit-schema → latest sqlite-utils → `click>=8.3.1` vs bundled click
  8.1.7 ("Requested 'click>=8.3.1', but click==8.1.7 is already installed").
  micropip won't upgrade in place. Since nothing is imported yet at install time,
  `install_resolving_conflicts()` parses that error, `micropip.uninstall()`s the
  package, installs the requested version and retries.
- 0.65 uses CodeMirror 5 (`.CodeMirror`), 1.0a uses CodeMirror 6 (`.cm-content`).
- datasette-vega: renders `.datasette-vega-inactive` until "Show charting
  options" is clicked; needs a mark radio before it draws.

## Results

Full suite: 68 passed in 7.5 min before the subpath test was added (12 bridge unit
tests then, 5 original, 26 URL pattern, 25 plugin/POST). Final run after adding the
subpath test + attribute-rewrite unit test: **70 passed in 8m21s**.

Demo built by `github-pages.sh` (clone upstream @ 779b2d4 + `git apply` the diff)
verified working served from `/datasette-lite-asgi/demo/` - a nested subpath.
