#!/usr/bin/env python3
"""
Test MicroQuickJS by embedding both the WASM and JS glue code into Denobox.
"""

from denobox import Denobox, DenoboxError
import base64

print("=== Testing MicroQuickJS via Denobox (embedded approach) ===\n")

# Read files from the research repo
wasm_path = "/tmp/research/mquickjs-sandbox/mquickjs_optimized.wasm"
js_path = "/tmp/research/mquickjs-sandbox/mquickjs_optimized.js"

with open(wasm_path, 'rb') as f:
    wasm_bytes = f.read()
    wasm_b64 = base64.b64encode(wasm_bytes).decode('ascii')

with open(js_path, 'r') as f:
    js_glue = f.read()

print(f"WASM size: {len(wasm_bytes)} bytes")
print(f"JS glue size: {len(js_glue)} bytes")
print()

# JavaScript code that runs MicroQuickJS
# We use an async IIFE and JSON encoding for the result
js_code = '''
(async () => {
    // Decode the WASM bytes from base64
    const wasmB64 = "''' + wasm_b64 + '''";
    const wasmBytes = Uint8Array.from(atob(wasmB64), c => c.charCodeAt(0));

    // Setup minimal environment for Emscripten
    const process = { exit: (code) => { if (code !== 0) throw new Error('Exit: ' + code); } };
    const require = () => ({});

    // The Emscripten glue code
    const glueCode = ''' + repr(js_glue) + ''';

    // Evaluate it to get createMQuickJS
    const createMQuickJS = eval(glueCode);

    // Create the module
    const Module = await createMQuickJS({ wasmBinary: wasmBytes.buffer });

    // Get function wrappers
    const sandbox_init = Module.cwrap('sandbox_init', 'number', ['number']);
    const sandbox_free = Module.cwrap('sandbox_free', null, []);
    const sandbox_eval = Module.cwrap('sandbox_eval', 'string', ['string']);
    const sandbox_get_error = Module.cwrap('sandbox_get_error', 'string', []);

    // Initialize with 1MB memory
    if (!sandbox_init(1024 * 1024)) {
        throw new Error("Failed to initialize sandbox");
    }

    // Run tests
    const tests = [
        { code: "1 + 2", expected: "3" },
        { code: "'hello' + ' world'", expected: "hello world" },
        { code: "Math.sqrt(16)", expected: "4" },
        { code: "[1,2,3].length", expected: "3" },
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

    // Cleanup
    sandbox_free();

    return results;
})()
'''

print("Executing MicroQuickJS in Denobox...")

with Denobox() as box:
    try:
        result = box.eval(js_code)
        print(f"\nResults:")
        for r in result:
            if 'error' in r:
                print(f"  FAIL: {r['code']} -> error: {r['error']}")
            elif r.get('pass'):
                print(f"  PASS: {r['code']} = {r['result']}")
            else:
                print(f"  FAIL: {r['code']} expected {r['expected']}, got {r['result']}")
        print("\nSuccess! MicroQuickJS is running inside Denobox!")
    except DenoboxError as e:
        print(f"DenoboxError: {e}")
    except Exception as e:
        print(f"Error: {type(e).__name__}: {e}")

print("\n=== Test completed ===")
