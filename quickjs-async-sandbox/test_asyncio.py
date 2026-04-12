"""Test running QuickJS in a background thread, driven from asyncio."""
import asyncio
import concurrent.futures
import json
import time
import httpx
import quickjs


async def run_quickjs_basic():
    """Run QuickJS on a worker thread via loop.run_in_executor."""
    loop = asyncio.get_running_loop()

    def work():
        ctx = quickjs.Context()
        ctx.set_memory_limit(10 * 1024 * 1024)
        # NOTE: setting time_limit disables Python callbacks, so we skip it.
        return ctx.eval("40 + 2")

    with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
        result = await loop.run_in_executor(pool, work)
    print(f"BASIC: {result}")


async def run_quickjs_with_async_pyfetch():
    """
    Key idea: JS -> sync pyFetch callable -> blocks QuickJS thread ->
    asyncio.run_coroutine_threadsafe() -> runs httpx async on the main loop ->
    returns result to the QuickJS thread which returns into JS.

    This makes an async Python function appear as a sync function to JS,
    while still using the main event loop for I/O (so other tasks can run).
    """
    loop = asyncio.get_running_loop()
    client = httpx.AsyncClient(timeout=5.0)

    MAX_BYTES = 100 * 1024

    async def async_fetch(url: str) -> dict:
        try:
            async with client.stream("GET", url) as resp:
                chunks = []
                total = 0
                async for chunk in resp.aiter_bytes():
                    total += len(chunk)
                    if total > MAX_BYTES:
                        return {"error": f"body exceeded {MAX_BYTES}"}
                    chunks.append(chunk)
                return {
                    "status": resp.status_code,
                    "body": b"".join(chunks).decode("utf-8", errors="replace")[:500],
                }
        except Exception as e:
            return {"error": f"{type(e).__name__}: {e}"}

    def pyFetch(url: str) -> str:
        # Called from QuickJS worker thread; schedule async_fetch on the main loop
        fut = asyncio.run_coroutine_threadsafe(async_fetch(url), loop)
        # Block the QJS thread until the httpx request completes.
        # Other asyncio tasks continue to run on the main loop.
        return json.dumps(fut.result(timeout=10))

    def work():
        ctx = quickjs.Context()
        ctx.set_memory_limit(50 * 1024 * 1024)
        ctx.add_callable("pyFetch", pyFetch)
        code = """
        (function() {
            const urls = [
                'https://httpbin.org/status/200',
                'https://httpbin.org/json',
                'https://httpbin.org/status/500',
            ];
            const out = [];
            for (const u of urls) {
                const r = JSON.parse(pyFetch(u));
                out.push({url: u, status: r.status, err: r.error});
            }
            return JSON.stringify(out);
        })()
        """
        return ctx.eval(code)

    # Run concurrent Python tasks alongside the QJS worker to show the loop is alive
    async def ticker():
        for i in range(50):
            await asyncio.sleep(0.1)
        return "tick-done"

    with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
        start = time.monotonic()
        js_future = loop.run_in_executor(pool, work)
        tick_task = asyncio.create_task(ticker())
        js_result, tick_result = await asyncio.gather(js_future, tick_task)
        elapsed = time.monotonic() - start
    await client.aclose()
    print(f"ASYNC-FETCH: took {elapsed:.2f}s; ticker={tick_result}")
    print(f"ASYNC-FETCH results: {json.loads(js_result)}")


async def run_with_wall_clock_timeout():
    """
    Enforce a WALL-CLOCK timeout on QuickJS execution (since the built-in
    CPU-time limit is incompatible with Python callbacks).

    Trick: we cannot safely kill the QuickJS thread, but we can call
    `context.set_time_limit()` from another thread (Python-level, lock-protected)
    or use an interrupt handler. The Context C API does check time periodically.
    Let's see if calling set_time_limit from the main thread mid-execution works.
    """
    loop = asyncio.get_running_loop()

    def worker(ctx):
        try:
            return ("ok", ctx.eval("let x = 0; while (true) { x++; }"))
        except Exception as e:
            return ("err", f"{type(e).__name__}: {e}")

    ctx = quickjs.Context()
    # Don't set time limit up front — we have no Python callbacks in this test
    # so we could... but let's test setting it from another thread.
    with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
        start = time.monotonic()
        fut = loop.run_in_executor(pool, worker, ctx)
        await asyncio.sleep(0.5)
        # Try to interrupt by setting a retroactive time limit
        # (this call may need to happen on the same thread — let's see)
        try:
            ctx.set_time_limit(0)  # "past" time limit
            print("WALL: set_time_limit(0) called from main thread OK")
        except Exception as e:
            print(f"WALL: set_time_limit failed: {type(e).__name__}: {e}")
        result = await fut
        elapsed = time.monotonic() - start
    print(f"WALL: result after {elapsed:.2f}s: {result}")


if __name__ == "__main__":
    asyncio.run(run_quickjs_basic())
    print()
    asyncio.run(run_quickjs_with_async_pyfetch())
    print()
    asyncio.run(run_with_wall_clock_timeout())
