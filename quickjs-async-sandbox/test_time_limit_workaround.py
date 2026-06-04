"""Can we get both a time limit AND Python callbacks by toggling the limit?"""
import quickjs
import time


def test_toggle_time_limit_in_callback():
    """
    Strategy: When a Python callback is invoked, temporarily disable the time
    limit inside the callback, then re-enable it after the callback returns.

    BUT: the guard "Can not call into Python with a time limit set" fires at the
    call site, BEFORE the Python callback runs — so this can't work that way.
    """
    ctx = quickjs.Context()

    def my_cb():
        return 42

    ctx.add_callable("myCb", my_cb)
    ctx.set_time_limit(1)
    try:
        r = ctx.eval("myCb()")
        print(f"TOGGLE: got {r}")
    except Exception as e:
        print(f"TOGGLE: {type(e).__name__}: {e}")


def test_unset_then_callback_then_reset():
    """Unset time limit, call JS that calls callback, reset limit."""
    ctx = quickjs.Context()

    def my_cb():
        return 42

    ctx.add_callable("myCb", my_cb)

    # Start with no limit
    r = ctx.eval("myCb()")
    print(f"NO-LIMIT: got {r}")

    # Now set limit and hang
    ctx.set_time_limit(1)
    start = time.monotonic()
    try:
        ctx.eval("while(true){}")
    except Exception as e:
        print(f"HANG: interrupted after {time.monotonic()-start:.2f}s: {type(e).__name__}")

    # Back to no limit
    ctx.set_time_limit(-1)  # Try to "disable" via negative
    try:
        r = ctx.eval("myCb()")
        print(f"AFTER-UNSET(-1): got {r}")
    except Exception as e:
        print(f"AFTER-UNSET(-1): {type(e).__name__}: {e}")


def test_set_time_limit_zero():
    """What values disable the time limit?"""
    ctx = quickjs.Context()

    def cb():
        return 42

    ctx.add_callable("myCb", cb)

    for val in [0, -1]:
        ctx.set_time_limit(val)
        try:
            r = ctx.eval("myCb()")
            print(f"VAL={val}: got {r}")
        except Exception as e:
            print(f"VAL={val}: {type(e).__name__}: {e}")


if __name__ == "__main__":
    test_toggle_time_limit_in_callback()
    print()
    test_unset_then_callback_then_reset()
    print()
    test_set_time_limit_zero()
