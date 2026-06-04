# QuickJS Python Sandbox Investigation — Notes

## Goal
Use the `quickjs` PyPI package to run untrusted JS with:
- hard memory limit
- hard time limit
- in a background thread, accessed via asyncio
- with selected Python functions exposed (including time/size limited HTTP fetch)
- ideally with async Python (httpx) exposed to JS

## Environment
- `quickjs` **1.19.4** on PyPI, `uv add quickjs httpx`
- Python 3.12
- Module layout: `quickjs/__init__.py` is a thin wrapper around
  `_quickjs.cpython-*.so` (a C extension).

## API surface discovered
`quickjs.Context`:
- `eval(code)` / `module(code)` / `get` / `set` / `parse_json`
- `add_callable(name, py_callable)` — expose a Python callable as a JS global
- `set_memory_limit(bytes)` — enforced, raises `JSException("out of memory")`
- `set_time_limit(seconds)` — CPU time; raises `JSException("interrupted")`
- `set_max_stack_size(bytes)` — prevents runaway recursion
- `execute_pending_job()` — drives the microtask queue (Promises)
- `memory()` — returns detailed stats dict

`quickjs.Function`: convenience wrapper that runs everything on a
single-threaded `ThreadPoolExecutor(max_workers=1)` because a `Context` is
**not thread-safe** — multiple threads touching the same runtime crash.

## Stress-test findings

### Limits that work
| Limit | Mechanism | Behaviour |
|---|---|---|
| Memory | `set_memory_limit(N)` | raises `JSException("out of memory")` |
| Stack | `set_max_stack_size(N)` / default | raises `quickjs.StackOverflow` |
| CPU time (pure JS only) | `set_time_limit(s)` (C `clock()`) | raises `JSException("interrupted")` |

Context remains usable after any of these errors (verified).

### CRITICAL gotcha: time-limit ⊥ Python callbacks
> `JSException: InternalError: Can not call into Python with a time limit set.`

If `set_time_limit(N)` is active AND the JS invokes any Python callable,
the call fails immediately. `set_time_limit(-1)` disables; `0` enforces
(interprets as "0 seconds of CPU budget").

### CRITICAL gotcha: setting time_limit from another thread doesn't interrupt
Calling `ctx.set_time_limit(0)` from the main thread while the QJS worker
thread is in an infinite `eval` does **not** stop the eval. The limit
appears to be snapshot at eval entry or not threadsafe for mutation.

**Conclusion:** built-in time limit can't be used with callbacks, and can't
be used to externally interrupt. Use **wall-clock timeout via
`asyncio.wait_for`** and **abandon** the thread if it hangs (make the
worker a daemon thread so it doesn't block interpreter exit).

### CRITICAL gotcha: Python exceptions in callbacks poison the context
If a Python callable raises, subsequent `ctx.eval()` calls can raise
`SystemError: <method 'eval'> returned a result with an exception set`.
Even with a JS-side `try/catch`.

**Fix:** wrap every exposed callable so it **never raises**. Return errors
as a JSON payload (`{"__error": "..."}`) and rethrow on the JS side.

### Callback argument / return types
- Accepts: `int`, `str`, `bool`, `None`, `float`
- JS arrays / objects arrive as `quickjs.Object`; use `.json()` to read
- `undefined` → `None`
- **Cannot return a Python `dict` or `list` directly** (raises "Can not
  convert Python result to JS"); return a JSON string and parse on JS side

### Promise / async JS support — it all works
- Native `Promise` / `.then` chains resolve via `execute_pending_job()`
- ES `async` / `await` syntax fully supported
- A Python sync callable wrapped in `new Promise(...)` on the JS side gives
  you an awaitable interface cheaply
- Deferred resolution works: Python stashes a resolver fn in a JS global,
  later `ctx.eval("globalThis.__resolver(val)")` plus draining pending
  jobs resolves the awaiting coroutine

### Exposing async Python to JS — two patterns

**Pattern A (used by `AsyncQuickJSSandbox`): sync-looking bridge**
```
JS: await fetchAsync(url)
JS: resolves a Promise created on the JS side
JS: whose executor synchronously calls __py.fetch(url)
Py: __py.fetch is a sync wrapper that runs
     asyncio.run_coroutine_threadsafe(async_fetch(url), main_loop).result()
```
The QJS worker thread blocks on the async result, but the asyncio loop
continues running concurrent tasks. Verified with a 50-tick ticker running
in parallel with a QJS `await fetchAsync('/delay/1')` chain.

Trade-off: within one QJS `run()`, JS cannot do `Promise.all([a, b])` to
fan out. Each await serialises through the QJS worker thread.

**Pattern B (documented alternative): true deferred resolution**
Have Python return a Promise by storing the resolver function in a JS
global, do async work on the main asyncio loop, then call back into the
Context to invoke the resolver. Allows fan-out but is harder to wire up
safely. Proof-of-concept in `test_promises.py::test_deferred_resolution`.

## Design decisions for `AsyncQuickJSSandbox`
1. **Dedicated daemon thread per run** (not a long-lived executor). If a
   run times out we abandon it; the thread will die on memory limit or
   process exit.
2. **Memory and stack limits** enforced via `set_memory_limit` /
   `set_max_stack_size` (not `set_time_limit`, since we need callbacks).
3. **Wall-clock limit** via `asyncio.wait_for`.
4. **All callables auto-wrapped** to return `{"ok": value}` / `{"__error": msg}`
   JSON and never raise.
5. **HTTP fetch** — size-limited streaming with `httpx.AsyncClient.stream`,
   byte counter raises in Python (caught by wrapper), default 5s HTTPX
   timeout, configurable scheme allow-list, max 256 KB.
6. **JS bootstrap** defines `fetchAsync(url, options)`, `sleep(ms)`,
   `log(...)` — all Promise-returning wrappers so user code can `await`.

## Follow-up: can we force-kill the thread? Process-based variant

### Thread-kill experiments (`test_kill_thread.py`)
- **`ctypes.pythonapi.PyThreadState_SetAsyncExc(tid, SystemExit)`**: the
  call returns 1 (one thread affected), but the exception is pending and
  only raised when Python bytecode executes. For **pure JS infinite loops
  the thread stays alive forever** — QuickJS is in C code with no Python
  frames.
- If the JS periodically calls a Python callable, the injected exception
  IS raised on the next callback (we get `JSException: Python call failed`
  and the thread exits). So you could add an auto-injected `__heartbeat()`
  Python callback — but that's invasive, and malicious code could simply
  avoid calling it.
- **`signal.pthread_kill(tid, SIGUSR1)`** with the default handler kills
  the **entire process** (tested: exit code 138). Installing a Python
  signal handler doesn't help because signals only dispatch in the main
  thread.

**Conclusion**: there is no reliable in-process way to terminate a thread
running pure-JS QuickJS. For hard isolation you need a separate process.

### `ProcessQuickJSSandbox` (`sandbox_process.py`)
Same API surface as the thread-based sandbox, but runs QuickJS in a child
process forked from the parent. Parent stays in asyncio; on timeout it
`SIGTERM`s, waits 50 ms, then `SIGKILL`s the child. A tiny line-delimited
JSON protocol over pipes carries log messages, RPC requests (fetch /
sleep), and the final result.

Additionally applies an OS-level address-space cap via
`resource.setrlimit(RLIMIT_AS, ...)` in the child — belt-and-braces on top
of the QuickJS memory limit.

### Measured overheads (`demo_process.py`, fork start method, Linux)
| scenario | thread sandbox | process sandbox |
|---|---|---|
| `40 + 2` total wall clock | ~2 ms | ~25 ms (~15 ms fork, ~10 ms QJS) |
| `while(true){}` | abandoned thread | **SIGKILL after ~50 ms over budget** |
| 3 concurrent `/delay/1` fetches | ~1.7 s | ~2.9 s (some IPC contention) |
| memory-limit violation | caught (OOM) | caught (OOM) |
| fetch-size violation | caught | caught |

The process version is ~10× slower for trivial runs but is the only
option that truly enforces a wall-clock deadline against hostile JS.

### When to pick which
- **Thread sandbox (`sandbox.py`)**: trusted-ish scripts, plugin code,
  LLM-generated snippets you mostly expect to terminate. Fastest path;
  the rare hung run just leaks a daemon thread.
- **Process sandbox (`sandbox_process.py`)**: adversarial inputs, CTF,
  anything from the public internet. Higher per-call overhead but hard
  SIGKILL guarantee, and a C-level crash in quickjs can't take down the
  parent.

## Files in this folder
- `notes.md` — this file
- `README.md` — final report
- `sandbox.py` — `AsyncQuickJSSandbox` (thread-based) implementation
- `demo.py` — thread sandbox end-to-end demo with 10 scenarios
- `sandbox_process.py` — `ProcessQuickJSSandbox` (fork + SIGKILL)
- `demo_process.py` — process sandbox demo (8 scenarios, incl. pure-JS hang kill)
- `test_limits.py` — memory/time/stack limit probes
- `test_callbacks.py` / `test_callbacks2.py` — callable type & error tests
- `test_time_limit_workaround.py` — what happens when you combine limits
- `test_asyncio.py` — background-thread + run_coroutine_threadsafe tests
- `test_promises.py` — Promise / async / await / deferred resolution
- `test_kill_thread.py` — failed attempts to kill a QJS-running thread

## Demo results (all 10 pass)
```
[basic]        result=42 elapsed=1.9ms
[fetch]        result={'status': 200, 'bodyLen': 429}
[multi-fetch]  3 fetches in 0.85s
[responsive]   2 fetches in 2.39s while asyncio ticker ran in parallel
[memory]       SandboxError: out of memory  (1MB limit)
[timeout]      SandboxTimeout after 1.50s on while(true){}
[fetch-size]   ValueError: response exceeded 200 bytes
[js-error]     404/403 on bogus domain caught in JS
[custom-async] user-supplied async callable returned {'query': 'select *', 'rows': [1,2,3]}
[concurrent]   3 sandboxes fetched /delay/1 in 1.72s total
```
