# QuickJS Python Sandbox — Investigation Report

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

Building an asyncio-friendly JavaScript sandbox using the
[`quickjs`](https://pypi.org/project/quickjs/) PyPI package (v1.19.4), with
hard memory and wall-clock limits and selected Python functions — including
async httpx fetches — exposed to the sandboxed JS.

## TL;DR

**It works well, with a few important caveats.** See `sandbox.py` for a
production-ready `AsyncQuickJSSandbox` class and `demo.py` for 10 scenarios
covering fetches, timeouts, memory limits, concurrency, and custom
async callables.

Run the demo with:

```bash
uv run python demo.py
```

## What we verified

| Capability | Status | Notes |
|---|---|---|
| Run JS on a background thread driven by asyncio | ✅ | `loop.run_in_executor` or a daemon `Thread` with a `Future` |
| Hard memory limit | ✅ | `ctx.set_memory_limit(bytes)` → `JSException("out of memory")` |
| Hard wall-clock timeout | ✅ (via `asyncio.wait_for`) | Built-in CPU-time limit is unusable when callbacks are exposed (see below) |
| Stack-overflow limit | ✅ | `ctx.set_max_stack_size(bytes)` → `quickjs.StackOverflow` |
| Expose sync Python functions | ✅ | `ctx.add_callable(name, fn)` |
| Expose **async** Python functions (httpx) | ✅ | Bridge via `asyncio.run_coroutine_threadsafe`; JS gets a Promise-returning wrapper it can `await` |
| Time- and size-limited httpx fetch | ✅ | Streaming with byte counter; `httpx.AsyncClient.stream` |
| Multiple sandboxes running concurrently on one asyncio loop | ✅ | Each gets its own daemon thread + private Context |
| Asyncio loop stays responsive during JS execution | ✅ | Other tasks run while the QJS thread blocks on `.result()` |

## Three critical gotchas

The investigation turned up three non-obvious constraints that shaped the
final design:

### 1. `set_time_limit()` is incompatible with `add_callable()`
If a time limit is active and JS calls any exposed Python function, QuickJS
raises `Can not call into Python with a time limit set.` You must pick
one — so we pick callbacks and enforce time via `asyncio.wait_for`.

### 2. You cannot interrupt a running `eval` from another thread
Calling `ctx.set_time_limit(0)` from the main thread while the worker
thread is in an infinite JS loop has no effect. The library is not
thread-safe for mutation during eval. If a sandbox times out, we abandon
the worker thread (it's a `daemon=True` thread, so it dies with the
process) and move on. The hanging JS will still hit the memory limit
eventually, or be terminated on exit.

### 3. Python exceptions inside callbacks corrupt the context
If an exposed Python callable raises, subsequent `ctx.eval()` calls on
that same context — even with a JS-side `try/catch` — can fail with
`SystemError: returned a result with an exception set`. The fix is to
wrap every callable so it never raises: return `{"__error": msg}` as a
JSON string and `throw` on the JS side. `sandbox.py` does this
automatically with `_safe_callable` and `_bridge_async`.

## Architecture of `AsyncQuickJSSandbox`

```
┌──────────────── asyncio main loop ─────────────────┐
│                                                     │
│   await sb.run(code)                                │
│                │                                    │
│                ▼                                    │
│     asyncio.wait_for(                               │
│        wrap_future(thread_future),                  │
│        timeout=wall_timeout)                        │
│                │                                    │
│    ┌───────────┼────────────────────────────────┐   │
│    │  daemon thread: "quickjs-sandbox"          │   │
│    │  ┌──────────────────────────────────────┐  │   │
│    │  │ quickjs.Context                      │  │   │
│    │  │  set_memory_limit / stack_size       │  │   │
│    │  │  add_callable(__py_fetch, ...)       │  │   │
│    │  │                                      │  │   │
│    │  │  __py.fetch(url) ──┐                 │  │   │
│    │  │                    │ blocks          │  │   │
│    │  └──────────────────┬─┼──────────────────┘  │   │
│    └────────────────────┼─┼──────────────────────┘   │
│                         ▼ │                          │
│   asyncio.run_coroutine_threadsafe(                 │
│       httpx.AsyncClient.stream(...), loop).result() │
│                                                     │
└─────────────────────────────────────────────────────┘
```

The JS bootstrap script (in `sandbox.py::_JS_BOOTSTRAP`) provides
Promise-returning wrappers around the raw synchronous Python callables, so
user JS can write idiomatic `await fetchAsync(url)`.

## Async-function-exposure: two patterns

**Pattern A (used in `AsyncQuickJSSandbox`): block-and-bridge.**
JS calls a sync Python function that blocks the QJS thread while
`run_coroutine_threadsafe` runs the coroutine on the main asyncio loop.
The main loop stays fully responsive; other asyncio tasks keep running.
The only limitation is that within one QJS run, JS can't do
`Promise.all([a, b])` to truly fan out — each await serialises through
the one worker thread.

**Pattern B (demonstrated in `test_promises.py::test_deferred_resolution`): true deferred resolution.**
Python stashes a JS resolver function in a global, kicks off async work,
and later calls `ctx.eval("globalThis.__resolver(value)")` plus drains
`execute_pending_job()`. This *does* support true JS-side concurrency,
but wiring it safely (per-promise resolver IDs, error paths, draining
loop) is substantially more code. Pattern A covers the common case.

## Example usage

```python
import asyncio
from sandbox import AsyncQuickJSSandbox

async def main():
    async with AsyncQuickJSSandbox(
        memory_limit_bytes=32 * 1024 * 1024,
        wall_timeout_seconds=10.0,
    ) as sb:
        code = """
        (async () => {
            log('starting');
            const r = await fetchAsync('https://httpbin.org/json');
            return { status: r.status, len: r.body.length };
        })()
        """
        result = await sb.run(code)
        print(result.value)  # {'status': 200, 'len': 429}
        print(result.logs)   # ['["starting"]']

asyncio.run(main())
```

Expose your own async function:

```python
async def my_async_tool(x: int) -> dict:
    await asyncio.sleep(0.1)
    return {"squared": x * x}

sb = AsyncQuickJSSandbox(extra_async_callables={"tool": my_async_tool})
# In JS: const r = __unwrap(__py.tool(5));  →  {squared: 25}
```

## Files

- `sandbox.py` — `AsyncQuickJSSandbox` implementation (≈180 LoC)
- `demo.py` — 10 end-to-end scenarios
- `test_limits.py`, `test_callbacks.py`, `test_callbacks2.py`,
  `test_time_limit_workaround.py`, `test_asyncio.py`, `test_promises.py` —
  the investigation scripts that produced the findings above
- `notes.md` — working notes with detailed findings
- `pyproject.toml`, `uv.lock`, `.python-version` — `uv` project metadata

## Can we force-kill the worker thread?

**No** — not for pure JS. Experimentally verified in `test_kill_thread.py`:

- `ctypes.pythonapi.PyThreadState_SetAsyncExc(tid, SystemExit)` queues an
  exception that is raised only when Python bytecode executes. QuickJS
  sits entirely in C code, so the injected exception never fires and the
  thread runs forever.
- If the JS *happens* to call a Python callback, the exception is raised
  on the next call and the thread exits — but hostile JS can just avoid
  callbacks (`while(true){}` never yields).
- `signal.pthread_kill` with default handlers terminates the whole
  process; with Python-installed handlers it doesn't help because Python
  only dispatches signals in the main thread.

So: **for hostile inputs, the only real option is a separate process.**

## `ProcessQuickJSSandbox` — hard termination via SIGKILL

`sandbox_process.py` provides the same run-JS-and-get-a-result API, but
runs QuickJS in a forked child. Parent stays in asyncio; on timeout it
`SIGTERM`s the child, waits briefly, then `SIGKILL`s. A small line-delimited
JSON protocol over pipes carries log messages, RPC requests (fetch/sleep
run on the parent's asyncio loop), and the final result.

It also applies an OS-level address-space cap via
`resource.setrlimit(RLIMIT_AS, ...)` in the child — defence in depth on
top of the QuickJS memory limit — and a C-level crash in the quickjs
extension can no longer take down the parent.

### Measured overhead (Linux, fork start method)

| scenario | thread sandbox | process sandbox |
|---|---|---|
| `40 + 2` total wall time | ~2 ms | ~25 ms (~15 ms fork + ~10 ms QJS) |
| `while(true){}` | thread leaked | **SIGKILL'd ~50 ms over budget** |
| 3× concurrent `/delay/1` fetches | ~1.7 s | ~2.9 s |

### When to pick which

- **Thread (`AsyncQuickJSSandbox`)** — trusted-ish plugin code, LLM-generated
  snippets you mostly expect to terminate. ~10× faster per run; a rare
  hung run leaks one daemon thread.
- **Process (`ProcessQuickJSSandbox`)** — adversarial inputs, untrusted
  network payloads. Higher start-up cost but guaranteed termination and
  proper memory + crash isolation.

Run the process demo with:

```bash
uv run python demo_process.py
```

Sample output:

```
[basic]        value=42 total=25ms js=11ms
[fetch]        value=200 logs=['["status", 200]']
[fetch-size]   value={'ok': False, 'err': 'ValueError: response exceeded 200 bytes'}
[memory]       SandboxError: out of memory
[pure-js-hang] SIGKILL'd after 1.57s: JS execution exceeded 1.5s wall clock
[disallowed]   value={'ok': False, 'err': "ValueError: scheme 'ftp' not allowed"}
[responsive]   value=[200, 200] elapsed=2.38s ticks=46 (loop stayed live)
[concurrent]   total=2.92s results=[('A', 200, ...), ('B', 200, ...), ('C', 200, ...)]
```

## Other caveats / future work

- **No fan-out inside a single sandbox run** unless you use Pattern B.
- **Callback arg types** — arrays/objects arrive as `quickjs.Object`
  instances in Python. Both sandboxes sidestep this by using JSON
  strings everywhere.
- **`ctx.module()` not explored** — ES modules should work but the demos
  use plain scripts + IIFEs.
- **`spawn` start method** for the process sandbox (needed on macOS and
  for maximum isolation) is untested — the current code uses `fork`,
  which is faster but shares file descriptors up to the fork point.
- **Process pool** could amortise the fork cost for high-throughput use
  cases; not implemented here.
