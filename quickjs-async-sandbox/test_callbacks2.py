"""More callback tests - exception recovery and http fetch."""
import quickjs
import httpx
import json


def test_exception_recovery():
    """Check whether context is usable after a Python exception in callback."""
    ctx = quickjs.Context()

    def bad():
        raise RuntimeError("kaboom")

    ctx.add_callable("pyBad", bad)

    # First attempt: let JS try/catch handle it
    try:
        r = ctx.eval("""
            (function() {
                try { pyBad(); return 'no-err'; }
                catch (e) { return 'caught: ' + e.toString(); }
            })()
        """)
        print(f"TRY-CATCH-IN-JS: {r!r}")
    except Exception as e:
        print(f"TRY-CATCH-IN-JS: eval raised {type(e).__name__}: {e}")

    # Can we still use it?
    try:
        r = ctx.eval("1+1")
        print(f"RECOVER: context still works: 1+1={r}")
    except Exception as e:
        print(f"RECOVER: context broken: {type(e).__name__}: {e}")


def test_http_with_json_return():
    """Test httpbin-based fetch."""
    ctx = quickjs.Context()
    MAX_BYTES = 100 * 1024

    def http_get(url):
        try:
            with httpx.Client(timeout=5.0) as client:
                with client.stream("GET", url) as resp:
                    chunks = []
                    total = 0
                    for chunk in resp.iter_bytes():
                        total += len(chunk)
                        if total > MAX_BYTES:
                            return json.dumps({"error": f"response > {MAX_BYTES} bytes"})
                        chunks.append(chunk)
                    body = b"".join(chunks)
                    return json.dumps({
                        "status": resp.status_code,
                        "body": body.decode("utf-8", errors="replace")[:500],
                    })
        except Exception as e:
            return json.dumps({"error": f"{type(e).__name__}: {e}"})

    ctx.add_callable("httpGet", http_get)

    code = """
    (function() {
        const raw = httpGet('https://httpbin.org/json');
        const r = JSON.parse(raw);
        if (r.error) return { ok: false, error: r.error };
        return { ok: true, status: r.status, body_len: r.body.length };
    })()
    """
    r = ctx.eval(code)
    if isinstance(r, quickjs.Object):
        r = json.loads(r.json())
    print(f"HTTP: {r}")


def test_multiple_fetches():
    """Several fetches in one JS program."""
    ctx = quickjs.Context()

    def http_get(url):
        try:
            resp = httpx.get(url, timeout=5.0)
            return json.dumps({"status": resp.status_code, "len": len(resp.content)})
        except Exception as e:
            return json.dumps({"error": str(e)})

    ctx.add_callable("httpGet", http_get)

    code = """
    (function() {
        const results = [];
        const urls = [
            'https://httpbin.org/status/200',
            'https://httpbin.org/status/404',
            'https://httpbin.org/json',
        ];
        for (const url of urls) {
            const r = JSON.parse(httpGet(url));
            results.push({ url: url, status: r.status });
        }
        return JSON.stringify(results);
    })()
    """
    r = ctx.eval(code)
    print(f"MULTI-FETCH: {json.loads(r)}")


if __name__ == "__main__":
    test_exception_recovery()
    print()
    test_http_with_json_return()
    print()
    test_multiple_fetches()
