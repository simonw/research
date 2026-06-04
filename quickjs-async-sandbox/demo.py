"""Demonstration / stress test of the AsyncQuickJSSandbox."""
import asyncio
import time

from sandbox import (
    AsyncQuickJSSandbox,
    SandboxError,
    SandboxTimeout,
    FetchLimits,
)


async def demo_basic():
    async with AsyncQuickJSSandbox() as sb:
        r = await sb.run("40 + 2")
        print(f"[basic]        result={r.value} elapsed={r.elapsed_seconds*1000:.1f}ms")


async def demo_fetch():
    async with AsyncQuickJSSandbox() as sb:
        code = """
        (async () => {
            const r = await fetchAsync('https://httpbin.org/json');
            log('got status', r.status);
            return { status: r.status, bodyLen: r.body.length };
        })()
        """
        r = await sb.run(code)
        print(f"[fetch]        result={r.value}  logs={r.logs}")


async def demo_async_await_multiple():
    """JS awaits several fetches in sequence."""
    async with AsyncQuickJSSandbox() as sb:
        code = """
        (async () => {
            const urls = [
                'https://httpbin.org/status/200',
                'https://httpbin.org/status/404',
                'https://httpbin.org/json',
            ];
            const out = [];
            for (const u of urls) {
                const r = await fetchAsync(u);
                out.push({url: u, status: r.status});
            }
            return out;
        })()
        """
        r = await sb.run(code)
        print(f"[multi-fetch]  result={r.value}  elapsed={r.elapsed_seconds:.2f}s")


async def demo_loop_stays_responsive():
    """
    While JS is awaiting fetchAsync, the asyncio loop must still process
    other tasks concurrently.
    """
    async def ticker():
        n = 0
        while True:
            await asyncio.sleep(0.05)
            n += 1

    sb = AsyncQuickJSSandbox()
    async with sb:
        tick_task = asyncio.create_task(ticker())
        code = """
        (async () => {
            const r1 = await fetchAsync('https://httpbin.org/delay/1');
            const r2 = await fetchAsync('https://httpbin.org/delay/1');
            return [r1.status, r2.status];
        })()
        """
        start = time.monotonic()
        r = await sb.run(code)
        elapsed = time.monotonic() - start
        tick_task.cancel()
    print(f"[responsive]   result={r.value} elapsed={elapsed:.2f}s "
          f"(asyncio ticker ran in parallel)")


async def demo_memory_limit():
    async with AsyncQuickJSSandbox(memory_limit_bytes=1_000_000) as sb:
        code = "(() => { let s='x'; for(let i=0;i<30;i++) s+=s; return s.length; })()"
        try:
            await sb.run(code)
            print("[memory]       UNEXPECTED success")
        except SandboxError as e:
            print(f"[memory]       caught SandboxError: {e}")


async def demo_wall_timeout():
    async with AsyncQuickJSSandbox(wall_timeout_seconds=1.5) as sb:
        code = "(() => { while(true){} })()"
        start = time.monotonic()
        try:
            await sb.run(code)
            print("[timeout]      UNEXPECTED success")
        except SandboxTimeout as e:
            print(f"[timeout]      caught after {time.monotonic()-start:.2f}s: {e}")


async def demo_fetch_size_limit():
    async with AsyncQuickJSSandbox(
        fetch_limits=FetchLimits(max_bytes=200)
    ) as sb:
        code = """
        (async () => {
            try {
                const r = await fetchAsync('https://httpbin.org/bytes/10000');
                return {ok: true, len: r.body.length};
            } catch (e) {
                return {ok: false, err: String(e.message)};
            }
        })()
        """
        r = await sb.run(code)
        print(f"[fetch-size]   result={r.value}")


async def demo_concurrent_sandboxes():
    """Spin up several sandboxes concurrently on the same asyncio loop."""
    async def one(tag, url):
        async with AsyncQuickJSSandbox() as sb:
            code = f"(async () => {{ const r = await fetchAsync({url!r}); return r.status; }})()"
            r = await sb.run(code)
            return (tag, r.value, r.elapsed_seconds)

    start = time.monotonic()
    results = await asyncio.gather(
        one("A", "https://httpbin.org/delay/1"),
        one("B", "https://httpbin.org/delay/1"),
        one("C", "https://httpbin.org/delay/1"),
    )
    elapsed = time.monotonic() - start
    print(f"[concurrent]   total_elapsed={elapsed:.2f}s results={results}")


async def demo_custom_async_callable():
    """Expose a custom async Python function to JS."""
    async def fake_db_query(q: str):
        await asyncio.sleep(0.1)
        return {"query": q, "rows": [1, 2, 3]}

    sb = AsyncQuickJSSandbox(
        extra_async_callables={"db_query": fake_db_query},
    )
    async with sb:
        # add the JS wrapper around the new __py.db_query callable
        code = """
        (async () => {
            const raw = __py.db_query('select *');
            return __unwrap(raw);
        })()
        """
        r = await sb.run(code)
        print(f"[custom-async] result={r.value}")


async def demo_js_error_handling():
    async with AsyncQuickJSSandbox() as sb:
        code = """
        (async () => {
            try {
                const r = await fetchAsync('http://not-a-real-domain-xxxx.invalid/');
                return {ok: true, status: r.status};
            } catch (e) {
                return {ok: false, err: String(e.message).slice(0, 80)};
            }
        })()
        """
        r = await sb.run(code)
        print(f"[js-error]     result={r.value}")


async def main():
    demos = [
        demo_basic,
        demo_fetch,
        demo_async_await_multiple,
        demo_loop_stays_responsive,
        demo_memory_limit,
        demo_wall_timeout,
        demo_fetch_size_limit,
        demo_js_error_handling,
        demo_custom_async_callable,
        demo_concurrent_sandboxes,
    ]
    for d in demos:
        try:
            await d()
        except Exception as e:
            print(f"[{d.__name__}] raised {type(e).__name__}: {e}")


if __name__ == "__main__":
    asyncio.run(main())
