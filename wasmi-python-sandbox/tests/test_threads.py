"""The GIL is released while the guest runs, so Python threads make progress
and independent sandboxes execute in parallel."""

import threading
import time

import wasmi_sandbox as ws

FIB = "function fib(n){return n<2?n:fib(n-1)+fib(n-2)}; fib(25)"


def test_python_thread_progresses_during_guest_call():
    js = ws.QuickJS(timeout=10)
    ticks = []
    stop = threading.Event()

    def ticker():
        while not stop.is_set():
            ticks.append(time.monotonic())
            time.sleep(0.001)

    t = threading.Thread(target=ticker)
    t.start()
    try:
        started = time.monotonic()
        js.eval(FIB)  # ~100 ms of pure guest execution
        elapsed = time.monotonic() - started
    finally:
        stop.set()
        t.join()
    during = [x for x in ticks if started < x < started + elapsed]
    assert elapsed > 0.02
    # With the GIL held the ticker would be starved for the whole eval
    # (CPython's switch interval is 5 ms; a held GIL never yields).
    assert len(during) >= 5, (elapsed, len(during))


def test_sandboxes_run_in_parallel_on_threads():
    def solo():
        js = ws.QuickJS(timeout=30)
        t = time.monotonic()
        js.eval(FIB)
        return time.monotonic() - t

    single = solo()

    results = []

    def worker():
        results.append(solo())

    threads = [threading.Thread(target=worker) for _ in range(2)]
    t = time.monotonic()
    for th in threads:
        th.start()
    for th in threads:
        th.join()
    wall = time.monotonic() - t
    assert len(results) == 2
    # Two guests on two cores should take well under 2x a single run.
    assert wall < 1.7 * single, (wall, single, results)


def test_host_functions_reacquire_gil_from_other_thread():
    calls = []
    out = []

    def worker():
        js = ws.QuickJS(timeout=10)
        js.register("note", lambda x: calls.append(x) or x * 2)
        out.append(js.eval("let s = 0; for (let i = 0; i < 50; i++) s += host.note(i); s"))

    th = threading.Thread(target=worker)
    th.start()
    th.join()
    assert out == [sum(i * 2 for i in range(50))]
    assert calls == list(range(50))


def test_store_is_thread_affine():
    # The Rust Store is an `unsendable` pyclass: using it from a thread other
    # than the one that created it is refused by PyO3 (as a PanicException,
    # which derives from BaseException).
    js = ws.QuickJS(timeout=10)
    errors = []

    def worker():
        try:
            js.eval("1 + 1")
        except BaseException as e:  # noqa: BLE001
            errors.append(f"{type(e).__name__}: {e}")

    th = threading.Thread(target=worker)
    th.start()
    th.join()
    assert errors and "unsendable" in errors[0]
