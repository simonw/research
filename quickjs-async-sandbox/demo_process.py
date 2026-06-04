"""Demo / stress test for the process-based sandbox."""
import asyncio
import time

from sandbox_process import (
    ProcessQuickJSSandbox,
    SandboxError,
    SandboxTimeout,
    FetchLimits,
)


async def demo_basic():
    async with ProcessQuickJSSandbox() as sb:
        start = time.monotonic()
        r = await sb.run("40 + 2")
        elapsed = time.monotonic() - start
        print(f"[basic]        value={r.value} "
              f"total={elapsed*1000:.0f}ms js={r.elapsed_seconds*1000:.0f}ms")


async def demo_fetch():
    async with ProcessQuickJSSandbox() as sb:
        r = await sb.run("""
            (async () => {
                const r = await fetchAsync('https://httpbin.org/json');
                log('status', r.status);
                return r.status;
            })()
        """)
        print(f"[fetch]        value={r.value} logs={r.logs}")


async def demo_pure_js_infinite_loop_is_killed():
    """This is the case the thread-based sandbox could NOT handle."""
    async with ProcessQuickJSSandbox(wall_timeout_seconds=1.5) as sb:
        start = time.monotonic()
        try:
            await sb.run("(() => { while(true){} })()")
            print("[pure-js-hang] UNEXPECTED success")
        except SandboxTimeout as e:
            elapsed = time.monotonic() - start
            print(f"[pure-js-hang] SIGKILL'd after {elapsed:.2f}s: {e}")


async def demo_memory_limit_os():
    """Even if the script tries to allocate huge arrays past rlimit."""
    async with ProcessQuickJSSandbox(memory_limit_bytes=2_000_000) as sb:
        try:
            await sb.run("(() => { let s='x'; for(let i=0;i<30;i++) s+=s; return s.length; })()")
            print("[memory]       UNEXPECTED success")
        except SandboxError as e:
            print(f"[memory]       SandboxError: {e}")


async def demo_loop_stays_responsive():
    counter = {"n": 0}

    async def tick():
        while True:
            await asyncio.sleep(0.05)
            counter["n"] += 1

    async with ProcessQuickJSSandbox() as sb:
        t = asyncio.create_task(tick())
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
        t.cancel()
    print(f"[responsive]   value={r.value} elapsed={elapsed:.2f}s "
          f"ticks={counter['n']} (loop stayed live)")


async def demo_concurrent():
    async def one(tag, url):
        async with ProcessQuickJSSandbox() as sb:
            r = await sb.run(
                f"(async () => {{ const r = await fetchAsync({url!r}); return r.status; }})()"
            )
            return tag, r.value, r.elapsed_seconds

    start = time.monotonic()
    results = await asyncio.gather(
        one("A", "https://httpbin.org/delay/1"),
        one("B", "https://httpbin.org/delay/1"),
        one("C", "https://httpbin.org/delay/1"),
    )
    elapsed = time.monotonic() - start
    print(f"[concurrent]   total={elapsed:.2f}s results={results}")


async def demo_fetch_size_limit():
    async with ProcessQuickJSSandbox(
        fetch_limits=FetchLimits(max_bytes=200)
    ) as sb:
        r = await sb.run("""
            (async () => {
                try {
                    const r = await fetchAsync('https://httpbin.org/bytes/10000');
                    return {ok: true, len: r.body.length};
                } catch (e) {
                    return {ok: false, err: String(e.message)};
                }
            })()
        """)
        print(f"[fetch-size]   value={r.value}")


async def demo_child_crash():
    """What if the JS throws inside a callback chain?"""
    async with ProcessQuickJSSandbox() as sb:
        r = await sb.run("""
            (async () => {
                try {
                    const r = await fetchAsync('ftp://not-allowed/');
                    return { ok: true };
                } catch (e) {
                    return { ok: false, err: String(e.message) };
                }
            })()
        """)
        print(f"[disallowed]   value={r.value}")


async def main():
    for d in [
        demo_basic,
        demo_fetch,
        demo_fetch_size_limit,
        demo_memory_limit_os,
        demo_pure_js_infinite_loop_is_killed,
        demo_child_crash,
        demo_loop_stays_responsive,
        demo_concurrent,
    ]:
        try:
            await d()
        except Exception as e:
            print(f"[{d.__name__}] raised {type(e).__name__}: {e}")


if __name__ == "__main__":
    asyncio.run(main())
