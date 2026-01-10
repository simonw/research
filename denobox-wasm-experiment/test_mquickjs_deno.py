#!/usr/bin/env python3
"""
Test loading MicroQuickJS WASM through Denobox by embedding the JS glue code.

The MicroQuickJS WASM files are Emscripten-compiled and need their JS runtime.
We'll pass the WASM bytes and JS glue code to Deno to run.
"""

from denobox import Denobox, DenoboxError
import base64

print("=== Testing MicroQuickJS via Denobox ===\n")

# Read the WASM file
wasm_path = "/tmp/tools/mquickjs_optimized.wasm"
js_path = "/tmp/tools/mquickjs_optimized.js"

with open(wasm_path, 'rb') as f:
    wasm_bytes = f.read()
    wasm_b64 = base64.b64encode(wasm_bytes).decode('ascii')

with open(js_path, 'r') as f:
    js_glue = f.read()

print(f"WASM size: {len(wasm_bytes)} bytes")
print(f"JS glue size: {len(js_glue)} bytes")
print()

# Create a JavaScript snippet that:
# 1. Decodes the WASM bytes from base64
# 2. Loads the Emscripten module with the WASM bytes
# 3. Runs some JS code through the sandbox

js_code = f'''
// Decode the WASM bytes from base64
const wasmB64 = "{wasm_b64}";
const wasmBytes = Uint8Array.from(atob(wasmB64), c => c.charCodeAt(0));

// The Emscripten glue code (will be eval'd)
const glueCode = {repr(js_glue)};

// Setup for Emscripten in non-browser environment
globalThis.window = globalThis;
globalThis.document = {{ currentScript: null }};

// Evaluate the glue code
eval(glueCode);

// The glue code creates a 'createMQuickJS' function
// We need to call it with our WASM bytes
const result = await (async () => {{
    const Module = await createMQuickJS({{
        wasmBinary: wasmBytes.buffer,
    }});

    // Initialize the sandbox
    const sandbox_init = Module.cwrap('sandbox_init', 'number', ['number']);
    const sandbox_eval = Module.cwrap('sandbox_eval', 'string', ['string']);
    const sandbox_get_error = Module.cwrap('sandbox_get_error', 'string', []);
    const sandbox_free = Module.cwrap('sandbox_free', null, []);

    // Initialize with 1MB memory
    if (!sandbox_init(1024 * 1024)) {{
        throw new Error("Failed to initialize sandbox");
    }}

    // Test evaluation
    const testCode = "1 + 2 * 3";
    const evalResult = sandbox_eval(testCode);

    // Clean up
    sandbox_free();

    return {{ evalResult, testCode }};
}})();

result;
'''

print("Attempting to run MicroQuickJS in Denobox...")

with Denobox() as box:
    try:
        result = box.eval(js_code)
        print(f"Result: {result}")
    except DenoboxError as e:
        print(f"DenoboxError: {e}")
    except Exception as e:
        print(f"Error: {type(e).__name__}: {e}")

print("\n=== Test completed ===")
