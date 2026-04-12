"""Stress test memory and time limits on quickjs.Context."""
import quickjs
import time


def test_memory_limit():
    ctx = quickjs.Context()
    ctx.set_memory_limit(1_000_000)  # 1 MB
    try:
        # Try to allocate a huge string
        ctx.eval("let s = 'x'; for (let i = 0; i < 30; i++) s += s;")
        print("MEMORY: no exception raised (unexpected)")
    except quickjs.JSException as e:
        print(f"MEMORY: got JSException as expected: {e!r}")
    except Exception as e:
        print(f"MEMORY: got other exception: {type(e).__name__}: {e}")


def test_time_limit_cpu():
    ctx = quickjs.Context()
    ctx.set_time_limit(1)  # 1 second of CPU
    start = time.monotonic()
    try:
        ctx.eval("while (true) {}")
        print("TIME: no exception raised (unexpected)")
    except quickjs.JSException as e:
        elapsed = time.monotonic() - start
        print(f"TIME: got JSException after {elapsed:.2f}s: {e!r}")
    except Exception as e:
        elapsed = time.monotonic() - start
        print(f"TIME: got {type(e).__name__} after {elapsed:.2f}s: {e}")


def test_time_limit_with_pycallback():
    """Does a blocking Python callback count against CPU time?"""
    ctx = quickjs.Context()
    ctx.set_time_limit(1)

    def slow_py():
        time.sleep(3)
        return 42

    ctx.add_callable("slowPy", slow_py)
    start = time.monotonic()
    try:
        result = ctx.eval("slowPy()")
        elapsed = time.monotonic() - start
        print(f"TIME-PYCB: returned {result} after {elapsed:.2f}s (limit was 1s)")
    except Exception as e:
        elapsed = time.monotonic() - start
        print(f"TIME-PYCB: exception after {elapsed:.2f}s: {type(e).__name__}: {e}")


def test_stack_overflow():
    ctx = quickjs.Context()
    try:
        ctx.eval("function r() { return r(); } r();")
        print("STACK: no exception (unexpected)")
    except quickjs.StackOverflow as e:
        print(f"STACK: StackOverflow raised: {e!r}")
    except Exception as e:
        print(f"STACK: {type(e).__name__}: {e}")


def test_memory_stats():
    ctx = quickjs.Context()
    ctx.eval("let arr = new Array(10000).fill(0);")
    print("MEMORY STATS:", ctx.memory())


def test_reuse_after_error():
    ctx = quickjs.Context()
    ctx.set_time_limit(1)
    try:
        ctx.eval("while(true){}")
    except Exception as e:
        print(f"REUSE: first eval errored: {type(e).__name__}")
    # Try to use the same context again
    try:
        result = ctx.eval("1 + 1")
        print(f"REUSE: second eval returned {result}")
    except Exception as e:
        print(f"REUSE: second eval errored: {type(e).__name__}: {e}")


if __name__ == "__main__":
    test_memory_limit()
    test_time_limit_cpu()
    test_time_limit_with_pycallback()
    test_stack_overflow()
    test_memory_stats()
    test_reuse_after_error()
