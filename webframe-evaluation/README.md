# webframe: evaluated as a scraper and a sandbox

[tractordev/webframe](https://github.com/tractordev/webframe) (commit `03155e2`) runs a web page as a command-line program. It loads the page in a hidden native webview (WebKitGTK on Linux, through [glaze](https://github.com/progrium/glaze) and purego, with no cgo), sends `console.*` to stdout and exceptions to stderr, and with `-c` evaluates each line of stdin in the page. The CLI is about 460 lines of Go and JS, and it builds to a 5.3 MB binary.

I built it on Ubuntu 24.04 with WebKitGTK 2.52 and Xvfb and ran it through a set of probes. Then I built three prototypes:

1. **[`webframe.diff`](webframe.diff)**: a patch to webframe that exposes glaze's ephemeral profile and bubblewrap sandbox, adds an eval fallback for pages with a Content-Security-Policy, and hardens the injected hooks.
2. **[`wfjs/`](wfjs/)**: a clone of `shot-scraper javascript` in about 350 lines of Go, built on the patched library.
3. **[`sandbox/wfsandbox.py`](sandbox/wfsandbox.py)**: a Python `run_js(code)` that runs untrusted JavaScript in webframe, plus [`attacks.py`](sandbox/attacks.py), which tries 20 hostile snippets against it ([output](sandbox/attacks-output.txt)).

The step-by-step log is in [notes.md](notes.md).

## Verdict

- **For scraping:** webframe works and is about as fast as shot-scraper. As a base for a `shot-scraper javascript` replacement it's promising, but rough. Out of the box, three things get in the way:
  - Evaluation fails on any page whose CSP blocks `eval`, which includes GitHub.
  - The viewport is 0×0.
  - A failed page load can hang forever.

  My patch and wrapper work around the first two and put a timeout on the third. The remaining gaps are:
  - no CSP bypass, so `import()` from a CDN fails on strict sites
  - no HTTP status code
  - no screenshots, user-agent setting or cookie injection
  - the page and webframe's hooks share one JavaScript world, so a hostile page can tamper with results
- **For sandboxing:** webframe is not a sandbox by itself. By default:
  - the page has full network access, including localhost
  - storage persists between runs
  - WebKit's renderer sandbox is off
  - there are no time or memory limits
  - a page can spoof eval results

  webframe works as one useful layer inside a sandbox: it gives you a real browser JavaScript engine (WASM, `crypto.subtle`, DOM) with no `fs` or `process` objects. The isolation has to come from outside. The combination that held up against every attack I tried was:
  - `unshare -rn` (an empty network namespace)
  - an ephemeral profile and bubblewrap (both from my patch)
  - a timeout with a process-group kill
  - a fresh process per call
  - a nonce-tagged result

## Running it

```
apt install libwebkit2gtk-4.1-0 xvfb      # plus Go 1.27
git clone https://github.com/tractordev/webframe && cd webframe
make build && ./webframe examples/hello.html    # → Hello, world!
```

When there's no `DISPLAY`, webframe starts its own private Xvfb. Startup takes about 0.5 s. The REPL, `WF_*` globals, `-e`/`-s`/`-S`, SIGTERM and output formatting all behaved as documented. SIGTERM exits in about 16 ms even while the page is stuck in `while(true){}`.

## What I found in the stock tool

| Finding | Impact |
|---|---|
| hooks.js runs `-c`/`Eval` code with page-level `(0, eval)(src)`, so a page with `script-src 'self'` (no `'unsafe-eval'`) rejects **every** evaluation with an EvalError | Can't scrape GitHub and many other sites |
| The hidden window is never given a size, so `innerWidth`/`innerHeight` are 0, even when the library's Width/Height are set | Layout-dependent scraping breaks |
| A failed load (DNS error) prints `Failed to load …` and then waits forever unless `-e` is set. A CONNECT failure through an HTTP proxy doesn't even print that | Scripts need `-e` or a timeout |
| An eval that never settles (for example, waiting on a `load` event that already fired) hangs forever. There is no eval timeout | Same |
| localStorage, cookies and cache persist in `~/.local/share/<program>`. Ephemeral mode is in glaze but not exposed | State carries over between runs |
| The WebKit renderer runs with no bubblewrap or seccomp, in the same namespaces, with full capabilities. glaze's `SandboxWebContent` is off by default on WebKitGTK 4.1 and webframe doesn't enable it | No defense if the renderer is exploited |
| A page can overwrite `window.__webframe_eval` and answer every eval with a lie, or call the `window.__webframe` bridge with a fake result for a guessable sequential id | Results from hostile pages can't be trusted |
| A page that replaces `JSON.stringify` silently breaks glaze's bridge: no output and a hang | Same |
| An `alert()` line is printed before `console.log` lines that ran earlier | Minor ordering bug |
| WebRTC is absent, `window.open` is dropped, downloads are cancelled, and permissions are denied. A `file://` page can't read other `file://` URLs | Good defaults |

### The patch (`webframe.diff`, +97/−17)

- `Options.Ephemeral` (CLI `-p`). Verified: storage written in one run isn't visible in the next.
- `Options.Sandbox` (CLI `-b`) turns on glaze's bubblewrap support. Verified: `WebKitWebProcess` runs under `bwrap` with its own net, mount and pid namespaces, `NoNewPrivs=1`, a seccomp filter and a minimal root filesystem.

  **However, the network is not cut off.** WebKit's NetworkProcess sits outside bwrap and does the fetching, so this protects against a compromised renderer, not against a page that wants to reach the network.
- `Options.NavigationStart` passes through glaze's navigation-start callback.
- **CSP fallback for `Eval`.** When the page-level eval is refused because of `unsafe-eval`, Go re-injects the source as `(function(){ return (SRC) })` using WebKit's native evaluate, which page CSP doesn't apply to. It does this as three scripts: begin, run, end. If the source isn't a single expression, the middle script fails to parse and the end script reports a clear error instead of hanging.
- **Hardening.** `JSON.stringify`, `eval` and `Promise` are captured at document start, and `__webframe_eval` and the new helpers are defined non-writable and non-configurable. The spoofing page from the table now returns the real title.

  The `JSON.stringify` attack still works, because glaze's own bridge calls the late-bound global. The real fix would be a WebKit isolated script world, which glaze doesn't expose.

## Prototype: `wfjs`, a `shot-scraper javascript` clone

```
cd wfjs && go build -o wfjs .   # go.mod expects a patched checkout at ../webframe-src

wfjs https://datasette.io/ "document.title"
wfjs -r https://datasette.io/ "document.title"
wfjs --width 390 --height 844 https://datasette.io/ "({width: innerWidth, height: innerHeight})"
wfjs https://www.example.com/ "() => { ... }"           # functions are called, promises awaited
wfjs --wait-for "document.querySelector('.late')" URL "..."
wfjs --fail URL "..."     # exit 1 on HTTP >= 400
wfjs -i script.js URL     # or JS on stdin
```

It matches shot-scraper's evaluation rules:
- the code runs as a global script
- if the result is a function, it's called
- the result is awaited, then JSON-encoded
- `throw` produces `Error: …` and exit code 1

It waits for `load` first. By default it uses an ephemeral profile; `--persist` keeps one at `~/.local/share/wfjs`. Other flags are `--timeout`, `--wait`, `--log-console` and `--sandbox`.

**Every example in shot-scraper's `docs/javascript.md` that doesn't depend on a shot-scraper-only flag ran unchanged.** That includes:
- titles, raw output, the viewport, object literals, a multi-statement throw, and a CI-style `if (…) throw`
- the `setInterval` promise and stdin input
- the Readability.js `import()` from jsdelivr, which returned `byline: "Simon Willison"` and `length: 8639` in 2.3 s

How I worked around the problems in the stock tool:
- **0×0 viewport:** when wfjs owns the private Xvfb, it shows a real window, which nobody can see.
- **CSP:** it retries with the code inlined as an expression.
- **No status code:** WebKit has no `responseStatus` and glaze doesn't report one, so `--fail` re-fetches the URL with `cache: "force-cache"`. That's a hack: my first version used HEAD and got a false positive from a server without HEAD support.

Wall-clock time for `document.title`, three runs each:

| URL | shot-scraper (Playwright 1.63, headless shell) | wfjs |
|---|---|---|
| example.com | 1.40 / 1.19 / 1.37 s | 1.17 / 1.10 / 1.08 s |
| datasette.io | 2.50 / 1.99 / 2.01 s | 2.19 / 1.79 / 1.96 s |
| simonwillison.net | 3.62 / 1.94 / 1.94 s | 2.05 / 2.81 / 2.22 s |

Memory is in the same range: about 640 MB of RSS across wfjs, WebKit and Xvfb, against about 456 MB for Chrome plus Python.

What shot-scraper still does that wfjs can't:
- statements and `--bypass-csp` imports on CSP sites
- `--auth` cookie files
- `--user-agent`
- HTTP status
- screenshots

There are also small differences in output:
- an `undefined` property is dropped instead of becoming `null`
- dates come out in ISO format
- DOM nodes come out as `{}`
- non-ASCII text isn't escaped

One upside: because profiles persist per program name, you could log in once in a visible window (`webframe -w`, or wfjs on a desktop) and then scrape as that user with `--persist`.

## Prototype: `wfsandbox.py`, running untrusted JS

```python
from wfsandbox import run_js     # WEBFRAME=/path/to/patched/webframe
run_js("[1,2,3].map(x => x * 2)")
# {'ok': True, 'result': [2, 4, 6], 'error': None, 'logs': [], 'ms': 1, 'wall_ms': 430}
```

Each call starts a fresh `unshare -rn webframe -b -p -s -c sandbox.html`. That page has a strict meta CSP, collects console output inside the page, and defines a locked `__sandbox_run(code, nonce)` function. Python returns only the stdout line that starts with the random nonce. Host `WF_*` environment variables are stripped, because webframe would otherwise expose them to the page as globals. Each call takes about 450 ms.

Results from [`attacks.py`](sandbox/attacks.py):

| Attack | CSP page only | + `unshare -rn` |
|---|---|---|
| fetch/XHR/img/beacon/WebSocket to localhost or the internet, `file:///etc/passwd` | blocked | blocked |
| top-level `location.href = "http://127.0.0.1/…?stolen"` | **request arrived** | blocked |
| `while(true){}`, memory bomb | killed by timeout (memory unbounded until then) | same |
| localStorage written in one run, read in the next | `null` with `-p` (leaks without `-p`) | — |
| forged result via the `window.__webframe` bridge | webframe prints the forgery; the nonce check rejects it, so the attack becomes denial of service, not a spoof | — |
| overwrite the runner / `window.close()` / `alert` | locked / no result / auto-dismissed | — |
| WebAssembly | works | works |

What I'd want before relying on this for anything serious:
- a cgroup memory and CPU limit (`RLIMIT_AS` doesn't work, because WebKit reserves about 75 GB of virtual memory)
- an isolated script world so the bridge can't be reached from page code
- a long-lived worker mode to avoid paying the 450 ms startup on every call

## Files

- `notes.md`: working notes
- `webframe.diff`: the patch against tractordev/webframe@03155e2
- `wfjs/`: the shot-scraper-style CLI (Go)
- `sandbox/wfsandbox.py`, `sandbox/attacks.py`, `sandbox/attacks-output.txt`: the sandbox runner, attack suite and its output
- `probes/`: the HTML probe pages (formatting, spoofing, exfiltration, file access, busy loop, memory) and `server.py`, the test server used against stock webframe
