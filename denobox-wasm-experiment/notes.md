# Denobox WASM Experiment Notes

## Task
Experiment with using Denobox to run Python and JavaScript code using WASM bundles from simonw/tools.

## Setup
- Cloned simonw/tools to /tmp/tools
- Installed denobox 0.1a2 via pip

## WASM Files Found in /tmp/tools
1. `/tmp/tools/mquickjs.wasm` (228KB) - Original MicroQuickJS
2. `/tmp/tools/mquickjs_optimized.wasm` (148KB) - Optimized MicroQuickJS

Note: MicroPython and regular QuickJS use CDN-hosted WASM (not local files in the repo)
- MicroPython: `@micropython/micropython-webassembly-pyscript@1.26.0`
- QuickJS: `quickjs-emscripten@0.31.0`

## Denobox API Understanding
```python
from denobox import Denobox

with Denobox() as box:
    # Evaluate JavaScript
    result = box.eval("1 + 1")

    # Load WASM from path (read by Python, not Deno)
    wasm = box.load_wasm(path="/path/to/file.wasm")

    # Or from bytes
    wasm = box.load_wasm(wasm_bytes=some_bytes)

    # Call exported functions
    result = wasm.call("function_name", arg1, arg2)
```

## Experiments

### Experiment 1: Basic JavaScript Eval
- **Status**: SUCCESS
- Basic arithmetic, strings, arrays, objects, functions all work
- Error handling via `DenoboxError` works

### Experiment 2: Direct WASM Loading
- **Status**: FAILED for Emscripten modules
- Denobox's `load_wasm()` cannot load Emscripten-compiled WASM
- Error: `WebAssembly.instantiate(): Import #0 "a": module is not an object or function`
- These modules require the Emscripten JavaScript runtime

### Experiment 3: MicroQuickJS via Embedded JS Glue
- **Status**: SUCCESS!
- Approach: Embed both WASM (as base64) and JS glue into eval code
- Key: Wrap the Emscripten glue in a function scope to avoid identifier collisions
- Works with both local files and remote URLs from tools.simonwillison.net

### WASM URLs Found

**MicroQuickJS (from tools.simonwillison.net):**
- WASM: https://tools.simonwillison.net/mquickjs_optimized.wasm (148KB)
- JS Glue: https://tools.simonwillison.net/mquickjs_optimized.js (17KB)

**MicroPython (from jsdelivr CDN):**
- WASM: https://cdn.jsdelivr.net/npm/@micropython/micropython-webassembly-pyscript@1.26.0/micropython.wasm (420KB)
- JS Module: https://cdn.jsdelivr.net/npm/@micropython/micropython-webassembly-pyscript@1.26.0/micropython.mjs (102KB)

**QuickJS (from jsdelivr CDN - component package):**
- WASM: https://cdn.jsdelivr.net/npm/@jitl/quickjs-wasmfile-release-sync@0.31.0/dist/emscripten-module.wasm (507KB)

**SQLite (sql.js from jsdelivr CDN):**
- WASM: https://cdn.jsdelivr.net/npm/sql.js@1.11.0/dist/sql-wasm.wasm (638KB)
- JS Loader: https://cdn.jsdelivr.net/npm/sql.js@1.11.0/dist/sql-wasm.js (48KB)

### Experiment 4: SQLite via sql.js
- **Status**: SUCCESS!
- Full SQLite database running in Denobox
- CREATE TABLE, INSERT, SELECT all work
- Same embedding approach as MicroQuickJS

## Key Finding

The Denobox `load_wasm()` API only works with simple WASM files that have no imports.
Emscripten-compiled modules (MicroQuickJS, MicroPython, SQLite, QuickJS) all require
their JavaScript glue code to provide the Emscripten runtime.

**Solution**: Embed both the WASM (as base64) and the JS glue code into a JavaScript
snippet that runs via `box.eval()`. This allows running complex WASM interpreters inside
the Deno sandbox.

