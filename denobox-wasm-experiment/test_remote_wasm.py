#!/usr/bin/env python3
"""
Test loading MicroQuickJS WASM from remote URLs.
The files should be at https://tools.simonwillison.net/
"""

from denobox import Denobox, DenoboxError
import urllib.request
import base64

print("=== Testing MicroQuickJS from Remote URLs ===\n")

# URLs for the WASM and JS files
base_url = "https://tools.simonwillison.net"
wasm_url = f"{base_url}/mquickjs_optimized.wasm"
js_url = f"{base_url}/mquickjs_optimized.js"

print(f"Downloading from: {base_url}")

# Download the files
print(f"  Downloading {wasm_url}...")
with urllib.request.urlopen(wasm_url) as response:
    wasm_bytes = response.read()
    wasm_b64 = base64.b64encode(wasm_bytes).decode('ascii')
print(f"    WASM size: {len(wasm_bytes)} bytes")

print(f"  Downloading {js_url}...")
with urllib.request.urlopen(js_url) as response:
    js_glue = response.read().decode('utf-8')
print(f"    JS size: {len(js_glue)} bytes")
print()

# JavaScript code to run MicroQuickJS
js_code = '''
(async () => {
    // Decode the WASM bytes from base64
    const wasmB64 = "''' + wasm_b64 + '''";
    const wasmBytes = Uint8Array.from(atob(wasmB64), c => c.charCodeAt(0));

    // Create isolated scope for Emscripten
    const getCreateModule = function() {
        var process = { exit: (code) => { if (code !== 0) throw new Error('Exit: ' + code); } };
        var require = () => ({});
        var Module;
        var __filename;
        var __dirname;

        ''' + js_glue + '''

        return createMQuickJS;
    };

    const createMQuickJS = getCreateModule();
    const Module = await createMQuickJS({ wasmBinary: wasmBytes.buffer });

    // Get function wrappers
    const sandbox_init = Module.cwrap('sandbox_init', 'number', ['number']);
    const sandbox_free = Module.cwrap('sandbox_free', null, []);
    const sandbox_eval = Module.cwrap('sandbox_eval', 'string', ['string']);
    const sandbox_get_error = Module.cwrap('sandbox_get_error', 'string', []);

    // Initialize
    if (!sandbox_init(1024 * 1024)) {
        throw new Error("Failed to initialize sandbox");
    }

    // Run tests
    const tests = [
        { code: "1 + 2 * 3", expected: "7" },
        { code: "'Hello, World!'", expected: "Hello, World!" },
        { code: "Math.PI.toFixed(4)", expected: "3.1416" },
        { code: "JSON.stringify({a: 1, b: 2})", expected: '{"a":1,"b":2}' },
        { code: "function fib(n) { return n < 2 ? n : fib(n-1) + fib(n-2); } fib(10)", expected: "55" },
    ];

    const results = [];
    for (const test of tests) {
        const result = sandbox_eval(test.code);
        if (result === null) {
            results.push({ code: test.code, error: sandbox_get_error() });
        } else {
            results.push({ code: test.code, result, expected: test.expected, pass: result === test.expected });
        }
    }

    sandbox_free();
    return results;
})()
'''

print("Executing MicroQuickJS (loaded from remote URLs) in Denobox...")

with Denobox() as box:
    try:
        result = box.eval(js_code)
        print(f"\nResults:")
        passed = 0
        failed = 0
        for r in result:
            if 'error' in r:
                print(f"  FAIL: {r['code'][:30]}... -> error: {r['error']}")
                failed += 1
            elif r.get('pass'):
                print(f"  PASS: {r['code'][:30]}... = {r['result']}")
                passed += 1
            else:
                print(f"  FAIL: {r['code'][:30]}... expected {r['expected']}, got {r['result']}")
                failed += 1
        print(f"\nTotal: {passed} passed, {failed} failed")
        print("\nSuccess! MicroQuickJS loaded from remote URLs works!")
    except DenoboxError as e:
        print(f"DenoboxError: {e}")
    except Exception as e:
        print(f"Error: {type(e).__name__}: {e}")

print("\n=== Test completed ===")
