# Denobox WASM Experiment

An investigation into using [Denobox](https://pypi.org/project/denobox/) to run Python and JavaScript code via WebAssembly bundles.

## Summary

**Denobox** is a Python library that executes JavaScript and WebAssembly in a Deno sandbox. This experiment tested whether complex WASM interpreters (MicroQuickJS, MicroPython, SQLite) can run inside Denobox.

### Key Findings

1. **Denobox's `load_wasm()` API** only works with simple WASM files that have no imports. Emscripten-compiled modules fail because they require JavaScript runtime support.

2. **Embedding approach works**: By embedding both the WASM bytes (as base64) and the JavaScript glue code into a single `box.eval()` call, complex WASM interpreters can run successfully inside Denobox.

3. **Successfully tested**:
   - MicroQuickJS (JavaScript interpreter)
   - SQLite (sql.js)
   - Basic JavaScript evaluation

## Working Examples

### MicroQuickJS

Run a JavaScript sandbox within the Denobox sandbox:

```python
from denobox import Denobox
import base64
import urllib.request

# Download the files
with urllib.request.urlopen("https://tools.simonwillison.net/mquickjs_optimized.wasm") as r:
    wasm_b64 = base64.b64encode(r.read()).decode('ascii')
with urllib.request.urlopen("https://tools.simonwillison.net/mquickjs_optimized.js") as r:
    js_glue = r.read().decode('utf-8')

js_code = f'''
(async () => {{
    const wasmBytes = Uint8Array.from(atob("{wasm_b64}"), c => c.charCodeAt(0));

    const getCreateModule = function() {{
        var process = {{ exit: (code) => {{ if (code !== 0) throw new Error('Exit: ' + code); }} }};
        var require = () => ({{}}); var Module; var __filename; var __dirname;
        {js_glue}
        return createMQuickJS;
    }};

    const Module = await getCreateModule()({{ wasmBinary: wasmBytes.buffer }});
    const sandbox_init = Module.cwrap('sandbox_init', 'number', ['number']);
    const sandbox_eval = Module.cwrap('sandbox_eval', 'string', ['string']);
    const sandbox_free = Module.cwrap('sandbox_free', null, []);

    sandbox_init(1024 * 1024);
    const result = sandbox_eval("1 + 2 * 3");
    sandbox_free();
    return result;
}})()
'''

with Denobox() as box:
    result = box.eval(js_code)  # Returns "7"
```

### SQLite

Run SQL queries in an in-memory SQLite database:

```python
from denobox import Denobox
import base64
import urllib.request

# Download sql.js files
with urllib.request.urlopen("https://cdn.jsdelivr.net/npm/sql.js@1.11.0/dist/sql-wasm.wasm") as r:
    wasm_b64 = base64.b64encode(r.read()).decode('ascii')
with urllib.request.urlopen("https://cdn.jsdelivr.net/npm/sql.js@1.11.0/dist/sql-wasm.js") as r:
    js_glue = r.read().decode('utf-8')

js_code = f'''
(async () => {{
    const wasmBytes = Uint8Array.from(atob("{wasm_b64}"), c => c.charCodeAt(0));

    const getInitSqlJs = function() {{
        var process = {{ versions: {{ node: false }} }};
        var exports = {{}}; var module = {{ exports: exports }};
        {js_glue}
        return module.exports;
    }};

    const SQL = await getInitSqlJs()({{ wasmBinary: wasmBytes.buffer }});
    const db = new SQL.Database();

    db.run("CREATE TABLE test (id INTEGER, value TEXT)");
    db.run("INSERT INTO test VALUES (1, 'hello')");
    const result = db.exec("SELECT * FROM test");
    db.close();

    return result;
}})()
'''

with Denobox() as box:
    result = box.eval(js_code)  # Returns query results
```

## WASM URLs

### MicroQuickJS (JavaScript Interpreter)
- WASM: https://tools.simonwillison.net/mquickjs_optimized.wasm (148KB)
- JS Glue: https://tools.simonwillison.net/mquickjs_optimized.js (17KB)

### SQLite (sql.js)
- WASM: https://cdn.jsdelivr.net/npm/sql.js@1.11.0/dist/sql-wasm.wasm (638KB)
- JS Loader: https://cdn.jsdelivr.net/npm/sql.js@1.11.0/dist/sql-wasm.js (48KB)

### MicroPython
- WASM: https://cdn.jsdelivr.net/npm/@micropython/micropython-webassembly-pyscript@1.26.0/micropython.wasm (420KB)
- JS Module: https://cdn.jsdelivr.net/npm/@micropython/micropython-webassembly-pyscript@1.26.0/micropython.mjs (102KB)

### QuickJS (Full)
- WASM: https://cdn.jsdelivr.net/npm/@jitl/quickjs-wasmfile-release-sync@0.31.0/dist/emscripten-module.wasm (507KB)

## Why `load_wasm()` Doesn't Work

Denobox's built-in `load_wasm()` API calls `WebAssembly.instantiate()` with an empty imports object:

```javascript
const instance = await WebAssembly.instantiate(wasmModule, {});
```

Emscripten-compiled WASM modules require imports like:
- `env.invoke_iii`, `env.invoke_vi` - For setjmp/longjmp support
- `env._emscripten_resize_heap` - For memory management
- `a.b`, `a.c` - Minified internal function references

Without these imports, instantiation fails with errors like:
```
WebAssembly.instantiate(): Import #0 "a": module is not an object or function
```

## Solution: Embed Everything

The workaround is to:
1. Download both the WASM file and its JavaScript loader
2. Base64-encode the WASM bytes
3. Embed both into a JavaScript snippet
4. Run via `box.eval()`

This works because:
- The JavaScript loader provides all necessary imports
- The WASM bytes are passed directly to the loader
- Everything runs inside Deno's JavaScript sandbox

## Files in This Experiment

| File | Description |
|------|-------------|
| `test_basic.py` | Basic JavaScript evaluation tests |
| `test_mquickjs_v2.py` | MicroQuickJS with local files |
| `test_remote_wasm.py` | MicroQuickJS with remote URLs |
| `test_sqlite.py` | SQLite (sql.js) test |
| `test_micropython.py` | MicroPython test (requires network) |
| `notes.md` | Detailed experiment notes |

## Limitations

1. **No direct WASM loading**: Must embed JS glue code for Emscripten modules
2. **No network access in sandbox**: Denobox runs Deno with no permissions, so dynamic imports from URLs don't work
3. **MicroPython**: Requires ES module dynamic import which needs `--allow-net` permission
4. **Large payloads**: Embedding WASM as base64 increases message size significantly

## Conclusion

Denobox can successfully run complex WASM interpreters by embedding both the WASM and JavaScript glue code. This enables:
- Running JavaScript inside JavaScript (via MicroQuickJS)
- Running SQL queries (via sql.js)
- Potentially running Python (via MicroPython, with modifications)

The embedding approach works around Denobox's limitation of not supporting WASM modules with imports.
