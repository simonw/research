"""
Can we force-kill a thread that is running `ctx.eval("while(true){}")`?

Two approaches to try:
1. ctypes PyThreadState_SetAsyncExc — inject SystemExit into the target thread
2. Signals from the main thread (signal.SIGALRM / pthread_kill)
"""
import ctypes
import os
import signal
import threading
import time

import quickjs


def inject_exception(thread: threading.Thread, exc_type):
    """
    Use the CPython C API to inject an exception into another thread.
    Returns the number of threads affected (should be 1 on success).
    Only takes effect when Python bytecode runs in that thread — if the
    thread is in a C extension, the exception fires only when the C code
    returns to the interpreter.
    """
    tid = ctypes.c_long(thread.ident)
    res = ctypes.pythonapi.PyThreadState_SetAsyncExc(
        tid, ctypes.py_object(exc_type)
    )
    return res


def test_inject_on_pure_js_loop():
    """Pure JS infinite loop — no Python frames to trap."""
    ctx = quickjs.Context()
    result = {}

    def run():
        try:
            ctx.eval("while (true) {}")
            result["status"] = "finished"
        except BaseException as e:
            result["status"] = f"{type(e).__name__}: {e}"

    t = threading.Thread(target=run, daemon=True)
    t.start()
    time.sleep(0.3)
    print(f"Thread alive before inject: {t.is_alive()}")
    n = inject_exception(t, SystemExit)
    print(f"inject_exception returned: {n}")
    t.join(timeout=2.0)
    print(f"Thread alive after 2s: {t.is_alive()}")
    print(f"Result: {result}")


def test_inject_on_js_with_callback_poll():
    """
    JS that calls a Python callback in its loop — the callback gives the
    interpreter a chance to raise the injected exception.
    """
    ctx = quickjs.Context()
    result = {}

    def poll():
        return 1  # no-op; just yields control back to Python

    ctx.add_callable("poll", poll)

    def run():
        try:
            ctx.eval("while (true) { poll(); }")
            result["status"] = "finished"
        except BaseException as e:
            result["status"] = f"{type(e).__name__}: {str(e)[:80]}"

    t = threading.Thread(target=run, daemon=True)
    t.start()
    time.sleep(0.3)
    print(f"Thread alive before inject: {t.is_alive()}")
    n = inject_exception(t, SystemExit)
    print(f"inject_exception returned: {n}")
    t.join(timeout=2.0)
    print(f"Thread alive after 2s: {t.is_alive()}")
    print(f"Result: {result}")


def test_signal_to_thread():
    """
    signal.pthread_kill — sending SIGINT to a worker thread. In CPython,
    signal handlers can only run in the main thread, so this won't
    interrupt the worker directly.
    """
    if not hasattr(signal, "pthread_kill"):
        print("no pthread_kill available")
        return

    ctx = quickjs.Context()
    result = {}

    def run():
        try:
            ctx.eval("while (true) {}")
            result["status"] = "finished"
        except BaseException as e:
            result["status"] = f"{type(e).__name__}: {e}"

    t = threading.Thread(target=run, daemon=True)
    t.start()
    time.sleep(0.3)
    try:
        signal.pthread_kill(t.ident, signal.SIGUSR1)
        print("pthread_kill sent")
    except Exception as e:
        print(f"pthread_kill failed: {e}")
    t.join(timeout=2.0)
    print(f"Thread alive after 2s: {t.is_alive()}")
    print(f"Result: {result}")


if __name__ == "__main__":
    print("=== inject on pure JS loop ===")
    test_inject_on_pure_js_loop()
    print()
    print("=== inject on JS loop with callback ===")
    test_inject_on_js_with_callback_poll()
    print()
    print("=== signal.pthread_kill to worker ===")
    test_signal_to_thread()
    # Use os._exit to avoid waiting on any hung daemon threads at atexit.
    os._exit(0)
