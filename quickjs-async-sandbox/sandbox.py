"""
Asyncio-friendly QuickJS sandbox with memory + wall-clock limits and
Python-backed callables (including async ones via httpx).

Design notes
------------
- `quickjs.Context` is NOT thread-safe. One Context lives on a dedicated,
  single-worker thread pool. A fresh pool is created per sandbox so that a
  hanging/timed-out run doesn't poison subsequent runs.
- `Context.set_time_limit()` is incompatible with Python callbacks (the C
  extension explicitly rejects the combination with "Can not call into Python
  with a time limit set"), and calling it from another thread does NOT
  interrupt a running eval. So we enforce a wall-clock deadline via
  `asyncio.wait_for`. On timeout, the Python side gives up and discards the
  executor; the worker thread may still be running a tight JS loop — we just
  stop waiting on it.
- `set_memory_limit()` and `set_max_stack_size()` work reliably and will
  raise `JSException` / `StackOverflow` inside the JS run.
- Python callables exposed via `add_callable` **must not raise** — a raised
  exception leaves a pending Python exception on the QuickJS error path that
  surfaces as `SystemError` in the next C API call. Instead, we wrap every
  callable so it catches all exceptions and returns a JSON string describing
  the error.
- To expose "async-looking" functions to JS, we bridge into the caller's
  asyncio loop with `run_coroutine_threadsafe`. The QuickJS worker thread
  blocks on the resulting future, but the asyncio loop is free to run other
  tasks concurrently. On the JS side, `pyFetch` is wrapped in a `fetchAsync`
  helper so user code can `await` it naturally.
"""
from __future__ import annotations

import asyncio
import concurrent.futures
import json
import threading
import time
from dataclasses import dataclass
from typing import Any, Awaitable, Callable

import httpx
import quickjs


# ---------- Bridge helpers ----------

def _safe_callable(fn: Callable[..., Any]) -> Callable[..., str]:
    """
    Wrap a Python callable so it never raises into QuickJS.
    The callable is expected to return a JSON-serialisable value; errors are
    returned as `{"__error": "..."}`.
    """
    def wrapper(*args, **kwargs) -> str:
        try:
            return json.dumps({"ok": fn(*args, **kwargs)})
        except Exception as e:
            return json.dumps({"__error": f"{type(e).__name__}: {e}"})
    wrapper.__name__ = fn.__name__
    return wrapper


def _bridge_async(
    coro_fn: Callable[..., Awaitable[Any]],
    loop: asyncio.AbstractEventLoop,
    overall_timeout: float,
) -> Callable[..., str]:
    """
    Turn an async Python function into a *sync* callable suitable for
    `Context.add_callable`. The resulting function, when called from the
    QuickJS worker thread, dispatches the coroutine to `loop` and blocks
    on the result.
    """
    def sync_wrapper(*args, **kwargs) -> str:
        try:
            fut = asyncio.run_coroutine_threadsafe(
                coro_fn(*args, **kwargs), loop
            )
            result = fut.result(timeout=overall_timeout)
            return json.dumps({"ok": result})
        except Exception as e:
            return json.dumps({"__error": f"{type(e).__name__}: {e}"})
    sync_wrapper.__name__ = getattr(coro_fn, "__name__", "async_bridge")
    return sync_wrapper


# ---------- HTTP fetch implementation ----------

@dataclass
class FetchLimits:
    timeout_seconds: float = 5.0
    max_bytes: int = 256 * 1024
    allowed_schemes: tuple[str, ...] = ("http", "https")


async def _bounded_fetch(
    client: httpx.AsyncClient,
    limits: FetchLimits,
    url: str,
    method: str = "GET",
    headers: dict | None = None,
) -> dict:
    """Size- and time-limited HTTP fetch."""
    # Scheme check
    from urllib.parse import urlparse
    p = urlparse(url)
    if p.scheme not in limits.allowed_schemes:
        raise ValueError(f"scheme {p.scheme!r} not allowed")

    async with client.stream(
        method, url,
        timeout=limits.timeout_seconds,
        headers=headers or {},
    ) as resp:
        chunks = []
        total = 0
        async for chunk in resp.aiter_bytes():
            total += len(chunk)
            if total > limits.max_bytes:
                raise ValueError(
                    f"response exceeded {limits.max_bytes} bytes"
                )
            chunks.append(chunk)
        body = b"".join(chunks).decode("utf-8", errors="replace")
    return {
        "status": resp.status_code,
        "headers": dict(resp.headers),
        "body": body,
        "url": str(resp.url),
    }


# ---------- The sandbox ----------

# JS bootstrap: wraps the raw sync `__py.*` callables into async-friendly
# Promise-returning helpers so user code can `await` them.
_JS_BOOTSTRAP = r"""
    globalThis.__unwrap = function(raw) {
        const r = JSON.parse(raw);
        if (r.__error !== undefined) {
            throw new Error(r.__error);
        }
        return r.ok;
    };
    globalThis.fetchAsync = function(url, options) {
        options = options || {};
        return new Promise((resolve, reject) => {
            try {
                const raw = __py.fetch(
                    url,
                    options.method || 'GET',
                    JSON.stringify(options.headers || {})
                );
                resolve(__unwrap(raw));
            } catch (e) { reject(e); }
        });
    };
    globalThis.sleep = function(ms) {
        // Sleep implemented via Python callable (blocks the QJS thread,
        // but asyncio loop still runs other tasks)
        return new Promise((resolve, reject) => {
            try { __py.sleep(ms); resolve(null); }
            catch (e) { reject(e); }
        });
    };
    globalThis.log = function(...args) {
        __py.log(JSON.stringify(args));
    };
"""


class SandboxTimeout(Exception):
    """Raised when the sandbox run exceeds the wall-clock deadline."""


class SandboxError(Exception):
    """Wraps any other failure (JSException, memory, stack overflow)."""


@dataclass
class SandboxResult:
    value: Any
    elapsed_seconds: float
    memory_stats: dict
    logs: list[str]


class AsyncQuickJSSandbox:
    """
    Run a JavaScript program in a bounded QuickJS sandbox with async support.

    Example
    -------
        async with httpx.AsyncClient() as client:
            sb = AsyncQuickJSSandbox(http_client=client)
            result = await sb.run("(async () => { "
                                  "const r = await fetchAsync(url); "
                                  "return r.status; })()",
                                  globals={"url": "https://httpbin.org/json"})
    """

    def __init__(
        self,
        *,
        http_client: httpx.AsyncClient | None = None,
        memory_limit_bytes: int = 32 * 1024 * 1024,
        stack_limit_bytes: int = 1 * 1024 * 1024,
        wall_timeout_seconds: float = 10.0,
        fetch_limits: FetchLimits | None = None,
        extra_async_callables: dict[str, Callable[..., Awaitable[Any]]] | None = None,
        extra_sync_callables: dict[str, Callable[..., Any]] | None = None,
    ):
        self.memory_limit = memory_limit_bytes
        self.stack_limit = stack_limit_bytes
        self.wall_timeout = wall_timeout_seconds
        self.fetch_limits = fetch_limits or FetchLimits()
        self._http_client = http_client
        self._owns_http_client = http_client is None
        self._extra_async = extra_async_callables or {}
        self._extra_sync = extra_sync_callables or {}

    async def __aenter__(self):
        if self._http_client is None:
            self._http_client = httpx.AsyncClient()
        return self

    async def __aexit__(self, *exc):
        if self._owns_http_client and self._http_client is not None:
            await self._http_client.aclose()

    async def run(
        self,
        code: str,
        *,
        globals: dict[str, Any] | None = None,
    ) -> SandboxResult:
        """Run `code` and return the result (JSON-roundtripped)."""
        assert self._http_client is not None, "Use 'async with' or pass an http_client"
        loop = asyncio.get_running_loop()

        logs: list[str] = []

        async def _fetch(url, method="GET", headers_json="{}"):
            headers = json.loads(headers_json) if headers_json else {}
            return await _bounded_fetch(
                self._http_client, self.fetch_limits, url, method, headers
            )

        async def _sleep(ms):
            await asyncio.sleep(ms / 1000.0)
            return None

        sync_log = _safe_callable(lambda s: (logs.append(s) or None))
        sync_fetch = _bridge_async(_fetch, loop, self.wall_timeout)
        sync_sleep = _bridge_async(_sleep, loop, self.wall_timeout)

        extra_sync = {
            name: _safe_callable(fn) for name, fn in self._extra_sync.items()
        }
        extra_async = {
            name: _bridge_async(fn, loop, self.wall_timeout)
            for name, fn in self._extra_async.items()
        }

        globals = globals or {}

        def worker() -> tuple[Any, dict]:
            ctx = quickjs.Context()
            ctx.set_memory_limit(self.memory_limit)
            ctx.set_max_stack_size(self.stack_limit)
            # Cannot use ctx.set_time_limit together with Python callbacks.

            # Build the __py namespace by registering each callable separately
            # (add_callable only accepts top-level globals).
            ctx.eval("globalThis.__py = {};")
            def register(name, fn):
                ctx.add_callable(f"__py_{name}", fn)
                ctx.eval(f"__py.{name} = __py_{name};")

            register("fetch", sync_fetch)
            register("sleep", sync_sleep)
            register("log", sync_log)
            for n, fn in extra_sync.items():
                register(n, fn)
            for n, fn in extra_async.items():
                register(n, fn)

            ctx.eval(_JS_BOOTSTRAP)

            # Inject user globals via JSON round-trip
            for k, v in globals.items():
                ctx.set(k, ctx.parse_json(json.dumps(v)))

            # Wrap user code so its result (which may be a Promise) is
            # awaited and the final value JSON-serialised back.
            wrapped = (
                "(async () => {\n"
                "  const __v = await (async () => { return ("
                + code +
                "\n); })();\n"
                "  globalThis.__sandboxResult = JSON.stringify({ok: __v});\n"
                "})().catch(e => {"
                "  globalThis.__sandboxResult = "
                "JSON.stringify({__error: String(e && e.message || e)});"
                "});"
            )
            ctx.eval(wrapped)
            # Drain microtasks so Promises resolve
            while ctx.execute_pending_job():
                pass
            raw = ctx.eval("globalThis.__sandboxResult")
            parsed = json.loads(raw) if raw is not None else {"__error": "no result"}
            mem = ctx.memory()
            return parsed, mem

        # Daemon thread so a hung JS sandbox cannot block interpreter exit.
        thread_future: concurrent.futures.Future = concurrent.futures.Future()

        def thread_target():
            try:
                thread_future.set_result(worker())
            except BaseException as e:  # noqa: BLE001
                thread_future.set_exception(e)

        t = threading.Thread(
            target=thread_target, name="quickjs-sandbox", daemon=True
        )
        start = time.monotonic()
        t.start()
        try:
            parsed, mem = await asyncio.wait_for(
                asyncio.wrap_future(thread_future), timeout=self.wall_timeout
            )
        except asyncio.TimeoutError:
            # Daemon thread keeps running in the background until the JS loop
            # happens to hit a memory limit or the process exits.
            raise SandboxTimeout(
                f"JS execution exceeded {self.wall_timeout}s wall clock"
            ) from None

        elapsed = time.monotonic() - start
        if "__error" in parsed:
            raise SandboxError(parsed["__error"])
        return SandboxResult(
            value=parsed["ok"],
            elapsed_seconds=elapsed,
            memory_stats=mem,
            logs=logs,
        )
