"""Test exposing Python callbacks to QuickJS."""
import quickjs
import httpx
import json
import time


def test_basic_callback():
    ctx = quickjs.Context()

    def add(a, b):
        return a + b

    ctx.add_callable("pyAdd", add)
    result = ctx.eval("pyAdd(3, 4)")
    print(f"BASIC: pyAdd(3,4) = {result}")


def test_callback_types():
    ctx = quickjs.Context()

    def echo(x):
        print(f"  python received: {x!r} ({type(x).__name__})")
        return x

    ctx.add_callable("pyEcho", echo)
    for js_expr in [
        "pyEcho(42)",
        "pyEcho('hello')",
        "pyEcho(true)",
        "pyEcho(null)",
        "pyEcho([1,2,3])",
        "pyEcho({a: 1, b: 'x'})",
        "pyEcho(undefined)",
    ]:
        try:
            r = ctx.eval(js_expr)
            print(f"TYPES: {js_expr} -> {r!r} ({type(r).__name__})")
        except Exception as e:
            print(f"TYPES: {js_expr} FAILED: {type(e).__name__}: {e}")


def test_callback_returning_dict():
    ctx = quickjs.Context()

    def get_data():
        return {"status": 200, "body": "hello"}

    ctx.add_callable("pyGetData", get_data)
    try:
        r = ctx.eval("pyGetData()")
        print(f"DICT: returned {r!r} type={type(r).__name__}")
    except Exception as e:
        print(f"DICT: failed: {type(e).__name__}: {e}")

    # Try returning via JSON string
    def get_data_json():
        return json.dumps({"status": 200, "body": "hello"})

    ctx.add_callable("pyGetDataJson", get_data_json)
    r = ctx.eval("(function() { let s = pyGetDataJson(); return JSON.parse(s).status; })()")
    print(f"DICT-JSON: parsed status = {r}")


def test_http_fetch_sync():
    """Expose a sync, size+time-limited HTTP fetch via httpx."""
    ctx = quickjs.Context()

    MAX_BYTES = 1024 * 100  # 100 KB
    TIMEOUT = 5.0

    def http_get(url):
        with httpx.Client(timeout=TIMEOUT) as client:
            with client.stream("GET", url) as resp:
                chunks = []
                total = 0
                for chunk in resp.iter_bytes():
                    total += len(chunk)
                    if total > MAX_BYTES:
                        raise ValueError(f"response exceeded {MAX_BYTES} bytes")
                    chunks.append(chunk)
                body = b"".join(chunks)
        return json.dumps({
            "status": resp.status_code,
            "headers": dict(resp.headers),
            "body": body.decode("utf-8", errors="replace"),
        })

    ctx.add_callable("httpGet", http_get)
    code = """
    (function() {
        const raw = httpGet('https://httpbin.org/json');
        const r = JSON.parse(raw);
        return { status: r.status, hasBody: r.body.length > 0 };
    })()
    """
    try:
        r = ctx.eval(code)
        # Object -> convert via json
        if isinstance(r, quickjs.Object):
            r = json.loads(r.json())
        print(f"HTTP: {r}")
    except Exception as e:
        print(f"HTTP: FAILED {type(e).__name__}: {e}")


def test_exception_in_callback():
    ctx = quickjs.Context()

    def bad():
        raise RuntimeError("kaboom")

    ctx.add_callable("pyBad", bad)
    try:
        ctx.eval("pyBad()")
        print("EXC: no exception (unexpected)")
    except Exception as e:
        print(f"EXC: got {type(e).__name__}: {e}")

    # Try/catch inside JS
    r = ctx.eval("""
        (function() {
            try { pyBad(); return 'no-err'; }
            catch (e) { return 'caught: ' + e.toString(); }
        })()
    """)
    print(f"EXC-CATCH: {r!r}")


if __name__ == "__main__":
    test_basic_callback()
    print()
    test_callback_types()
    print()
    test_callback_returning_dict()
    print()
    test_exception_in_callback()
    print()
    test_http_fetch_sync()
