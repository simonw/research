"""Investigate Promise / await support in QuickJS bindings.

Goal: can we expose a Python async function such that JS can `await` it?
"""
import asyncio
import json
import quickjs


def test_native_promise():
    """Does QuickJS natively support Promises and microtasks?"""
    ctx = quickjs.Context()
    # A top-level Promise chain; needs execute_pending_job to resolve
    ctx.eval("""
        globalThis.__results = [];
        Promise.resolve(1)
          .then(v => { globalThis.__results.push('then1:' + v); return v + 1; })
          .then(v => { globalThis.__results.push('then2:' + v); });
    """)
    # Drive the microtask queue
    jobs = 0
    while True:
        pending = ctx.execute_pending_job()
        if not pending:
            break
        jobs += 1
    r = ctx.eval("JSON.stringify(__results)")
    print(f"NATIVE-PROMISE: {jobs} jobs drained, results={r}")


def test_async_await():
    """Does QuickJS support async/await syntax?"""
    ctx = quickjs.Context()
    ctx.eval("""
        globalThis.__result = null;
        (async () => {
            const a = await Promise.resolve(10);
            const b = await Promise.resolve(20);
            globalThis.__result = a + b;
        })();
    """)
    while ctx.execute_pending_job():
        pass
    print(f"ASYNC-AWAIT: result = {ctx.eval('__result')}")


def test_callback_returning_promise():
    """Can a Python callback return a Promise (so JS can await it)?"""
    ctx = quickjs.Context()

    # Inject a helper that wraps a sync pyFetch in a Promise
    # (we can't return a JS Promise directly from Python, but we can wrap JS-side)
    def raw_fetch(url):
        return json.dumps({"status": 200, "body": f"mock for {url}"})

    ctx.add_callable("rawFetch", raw_fetch)
    ctx.eval("""
        // Wrap the sync callable in a Promise so JS can await it
        globalThis.fetchAsync = function(url) {
            return new Promise((resolve, reject) => {
                try {
                    resolve(JSON.parse(rawFetch(url)));
                } catch (e) {
                    reject(e);
                }
            });
        };
        globalThis.__result = null;
        (async () => {
            const r = await fetchAsync('http://example.com');
            globalThis.__result = r;
        })();
    """)
    while ctx.execute_pending_job():
        pass
    r = ctx.eval("JSON.stringify(__result)")
    print(f"AWAIT-WRAPPED: {r}")


def test_deferred_resolution():
    """
    True async: Python triggers work on another thread, and later resolves
    a JS promise via execute_pending_job being driven by Python.

    Pattern: JS code creates a Promise and stashes its resolve function in
    Python (via a callback). Python does async work, then calls the stashed
    resolve by driving JS to call it.
    """
    ctx = quickjs.Context()

    # Store resolvers keyed by id
    pending = {}

    def register_resolver(pid, resolver_name):
        # resolver_name is the global name where JS stored the resolver
        pending[pid] = resolver_name
        return None

    def trigger_resolve(pid, value):
        # Called by our Python driver to tell JS which promise to resolve
        return json.dumps({"pid": pid, "value": value})

    ctx.add_callable("registerResolver", register_resolver)

    # This is getting complex; simpler test: can we resolve a promise by
    # setting a global and then calling ctx.eval?
    ctx.eval("""
        globalThis.__result = null;
        globalThis.__resolver = null;
        globalThis.__p = new Promise((resolve) => { globalThis.__resolver = resolve; });
        (async () => {
            const v = await globalThis.__p;
            globalThis.__result = 'got:' + v;
        })();
    """)
    # At this point the promise is unresolved; __result is null
    print(f"DEFERRED: before resolve __result = {ctx.eval('__result')}")

    # Now from Python, resolve the promise
    ctx.eval("globalThis.__resolver(123);")
    while ctx.execute_pending_job():
        pass
    print(f"DEFERRED: after resolve __result = {ctx.eval('__result')}")


if __name__ == "__main__":
    test_native_promise()
    print()
    test_async_await()
    print()
    test_callback_returning_promise()
    print()
    test_deferred_resolution()
