"""
Process-based QuickJS sandbox with TRUE termination on timeout.

Architecture
------------
- Each run spawns a child process (via multiprocessing) that runs QuickJS.
- Parent stays in asyncio; on timeout it SIGKILLs the child.
- IPC is a pair of pipes: parent sends the code + optional RPC replies;
  child streams back log lines, RPC requests, and the final result.
- The child speaks a tiny line-delimited JSON protocol:
      {"type": "log", "args": [...]}
      {"type": "rpc", "id": N, "name": "fetch", "args": [...]}
      {"type": "result", "ok": ...}  / {"type": "error", "msg": "..."}
  The parent replies to rpc requests with:
      {"type": "rpc_reply", "id": N, "ok": ...}  / {..., "error": "..."}

Trade-off vs the thread-based sandbox (sandbox.py):
  + Hard SIGKILL on timeout — no orphaned execution
  + OS-level memory cap via resource.setrlimit in the child
  + Process crash doesn't take down the parent
  - ~50-100 ms process-spawn latency per run (fork on Linux; spawn on macOS)
  - IPC serialisation overhead on every async callback
  - Slightly more code
"""
from __future__ import annotations

import asyncio
import json
import multiprocessing
import os
import resource
import signal
import sys
import time
from dataclasses import dataclass
from typing import Any, Awaitable, Callable

import httpx
import quickjs


# ---------- Child-side code ----------

def _child_main(
    conn_read_fd: int,
    conn_write_fd: int,
    code: str,
    globals_json: str,
    memory_limit: int,
    stack_limit: int,
    rlimit_as_bytes: int,
) -> None:
    """Entry point in the child process."""
    # OS-level address-space cap (belt & braces on top of quickjs memory limit)
    try:
        resource.setrlimit(
            resource.RLIMIT_AS, (rlimit_as_bytes, rlimit_as_bytes)
        )
    except (ValueError, OSError):
        pass  # not supported on all platforms

    read_file = os.fdopen(conn_read_fd, "r", buffering=1)
    write_file = os.fdopen(conn_write_fd, "w", buffering=1)

    def send(msg: dict) -> None:
        write_file.write(json.dumps(msg) + "\n")
        write_file.flush()

    def recv() -> dict:
        line = read_file.readline()
        if not line:
            raise EOFError
        return json.loads(line)

    rpc_counter = [0]

    def make_rpc(name: str) -> Callable[..., str]:
        def call(*args) -> str:
            rpc_counter[0] += 1
            rid = rpc_counter[0]
            send({"type": "rpc", "id": rid, "name": name, "args": list(args)})
            while True:
                msg = recv()
                if msg.get("type") == "rpc_reply" and msg.get("id") == rid:
                    if "error" in msg:
                        return json.dumps({"__error": msg["error"]})
                    return json.dumps({"ok": msg["ok"]})
                # Ignore any stray messages
        call.__name__ = f"rpc_{name}"
        return call

    def log_fn(s: str) -> str:
        send({"type": "log", "args": json.loads(s)})
        return json.dumps({"ok": None})

    try:
        ctx = quickjs.Context()
        ctx.set_memory_limit(memory_limit)
        ctx.set_max_stack_size(stack_limit)

        ctx.eval("globalThis.__py = {};")

        def register(name, fn):
            ctx.add_callable(f"__py_{name}", fn)
            ctx.eval(f"__py.{name} = __py_{name};")

        register("fetch", make_rpc("fetch"))
        register("sleep", make_rpc("sleep"))
        register("log", log_fn)

        ctx.eval(_JS_BOOTSTRAP)

        g = json.loads(globals_json)
        for k, v in g.items():
            ctx.set(k, ctx.parse_json(json.dumps(v)))

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
        while ctx.execute_pending_job():
            pass

        raw = ctx.eval("globalThis.__sandboxResult")
        parsed = json.loads(raw) if raw else {"__error": "no result"}
        mem = ctx.memory()
        if "__error" in parsed:
            send({"type": "error", "msg": parsed["__error"], "memory": mem})
        else:
            send({"type": "result", "ok": parsed["ok"], "memory": mem})
    except Exception as e:
        send({"type": "error", "msg": f"{type(e).__name__}: {e}",
              "memory": {}})
    finally:
        write_file.flush()


_JS_BOOTSTRAP = r"""
    globalThis.__unwrap = function(raw) {
        const r = JSON.parse(raw);
        if (r.__error !== undefined) throw new Error(r.__error);
        return r.ok;
    };
    globalThis.fetchAsync = function(url, options) {
        options = options || {};
        return new Promise((resolve, reject) => {
            try {
                const raw = __py.fetch(url, options.method || 'GET',
                                       JSON.stringify(options.headers || {}));
                resolve(__unwrap(raw));
            } catch (e) { reject(e); }
        });
    };
    globalThis.sleep = function(ms) {
        return new Promise((resolve, reject) => {
            try { __py.sleep(ms); resolve(null); }
            catch (e) { reject(e); }
        });
    };
    globalThis.log = function(...args) {
        __py.log(JSON.stringify(args));
    };
"""


# ---------- Parent-side sandbox ----------

class SandboxTimeout(Exception):
    pass


class SandboxError(Exception):
    pass


@dataclass
class SandboxResult:
    value: Any
    elapsed_seconds: float
    memory_stats: dict
    logs: list[str]


@dataclass
class FetchLimits:
    timeout_seconds: float = 5.0
    max_bytes: int = 256 * 1024
    allowed_schemes: tuple[str, ...] = ("http", "https")


class ProcessQuickJSSandbox:
    """
    Process-isolated QuickJS sandbox. Guarantees termination on timeout via
    SIGKILL.
    """

    def __init__(
        self,
        *,
        http_client: httpx.AsyncClient | None = None,
        memory_limit_bytes: int = 32 * 1024 * 1024,
        stack_limit_bytes: int = 1 * 1024 * 1024,
        rlimit_as_bytes: int = 128 * 1024 * 1024,
        wall_timeout_seconds: float = 10.0,
        fetch_limits: FetchLimits | None = None,
    ):
        self.memory_limit = memory_limit_bytes
        self.stack_limit = stack_limit_bytes
        self.rlimit_as = rlimit_as_bytes
        self.wall_timeout = wall_timeout_seconds
        self.fetch_limits = fetch_limits or FetchLimits()
        self._http_client = http_client
        self._owns_client = http_client is None

    async def __aenter__(self):
        if self._http_client is None:
            self._http_client = httpx.AsyncClient()
        return self

    async def __aexit__(self, *exc):
        if self._owns_client and self._http_client is not None:
            await self._http_client.aclose()

    async def _handle_rpc(self, name: str, args: list) -> Any:
        if name == "fetch":
            url, method, headers_json = args
            from urllib.parse import urlparse
            p = urlparse(url)
            if p.scheme not in self.fetch_limits.allowed_schemes:
                raise ValueError(f"scheme {p.scheme!r} not allowed")
            headers = json.loads(headers_json) if headers_json else {}
            async with self._http_client.stream(
                method, url,
                timeout=self.fetch_limits.timeout_seconds,
                headers=headers,
            ) as resp:
                chunks = []
                total = 0
                async for chunk in resp.aiter_bytes():
                    total += len(chunk)
                    if total > self.fetch_limits.max_bytes:
                        raise ValueError(
                            f"response exceeded {self.fetch_limits.max_bytes} bytes"
                        )
                    chunks.append(chunk)
                return {
                    "status": resp.status_code,
                    "headers": dict(resp.headers),
                    "body": b"".join(chunks).decode("utf-8", errors="replace"),
                    "url": str(resp.url),
                }
        if name == "sleep":
            (ms,) = args
            await asyncio.sleep(ms / 1000.0)
            return None
        raise ValueError(f"unknown RPC: {name}")

    async def run(
        self,
        code: str,
        *,
        globals: dict | None = None,
    ) -> SandboxResult:
        loop = asyncio.get_running_loop()
        globals = globals or {}
        logs: list[str] = []

        # Pipes: parent reads from child_write, child reads from parent_write
        parent_read, child_write = os.pipe()
        child_read, parent_write = os.pipe()

        # Use 'spawn' start method for cleanest isolation (no fork state share).
        ctx = multiprocessing.get_context("fork")  # fork is faster on Linux
        proc = ctx.Process(
            target=_child_main,
            args=(
                child_read, child_write,
                code,
                json.dumps(globals),
                self.memory_limit,
                self.stack_limit,
                self.rlimit_as,
            ),
            daemon=True,
        )
        proc.start()
        # Close child's ends on the parent side
        os.close(child_read)
        os.close(child_write)

        # Wrap the parent-side pipe in a non-blocking stream reader / writer.
        read_stream = os.fdopen(parent_read, "r", buffering=1)
        write_stream = os.fdopen(parent_write, "w", buffering=1)

        def readline_blocking() -> str:
            return read_stream.readline()

        def writeline_blocking(s: str) -> None:
            write_stream.write(s + "\n")
            write_stream.flush()

        start = time.monotonic()
        final: dict | None = None

        async def pump() -> dict:
            nonlocal final
            while True:
                line = await loop.run_in_executor(None, readline_blocking)
                if not line:
                    raise SandboxError(
                        "child process exited without result "
                        f"(exitcode={proc.exitcode})"
                    )
                msg = json.loads(line)
                kind = msg.get("type")
                if kind == "log":
                    logs.append(json.dumps(msg["args"]))
                elif kind == "rpc":
                    rid = msg["id"]
                    try:
                        value = await self._handle_rpc(msg["name"], msg["args"])
                        reply = {"type": "rpc_reply", "id": rid, "ok": value}
                    except Exception as e:
                        reply = {"type": "rpc_reply", "id": rid,
                                 "error": f"{type(e).__name__}: {e}"}
                    await loop.run_in_executor(
                        None, writeline_blocking, json.dumps(reply)
                    )
                elif kind in ("result", "error"):
                    return msg
                # else: ignore unknown messages

        try:
            final = await asyncio.wait_for(pump(), timeout=self.wall_timeout)
        except asyncio.TimeoutError:
            # Hard-kill the child with SIGKILL (SIGTERM first for cleanup chance)
            try:
                proc.terminate()
                await asyncio.sleep(0.05)
                if proc.is_alive():
                    os.kill(proc.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            proc.join(timeout=1.0)
            raise SandboxTimeout(
                f"JS execution exceeded {self.wall_timeout}s wall clock"
            ) from None
        finally:
            try:
                read_stream.close()
            except Exception:
                pass
            try:
                write_stream.close()
            except Exception:
                pass
            if proc.is_alive():
                proc.join(timeout=1.0)
                if proc.is_alive():
                    try:
                        os.kill(proc.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass

        elapsed = time.monotonic() - start
        if final["type"] == "error":
            raise SandboxError(final["msg"])
        return SandboxResult(
            value=final["ok"],
            elapsed_seconds=elapsed,
            memory_stats=final.get("memory", {}),
            logs=logs,
        )
