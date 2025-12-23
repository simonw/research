# mquickjs Sandbox Investigation Notes

## Goal
Investigate mquickjs as a safe sandboxing environment for running untrusted JavaScript code with:
- Memory limits
- CPU/time limits
- No file system access
- No network access

## Approaches to Test
1. Python FFI bindings (cffi/ctypes)
2. Python C extension
3. WebAssembly + Node.js
4. WebAssembly + Deno
5. WebAssembly + Python (wasmer)
6. WebAssembly + Python (wasmtime)

---

## Investigation Log

### Initial Setup
- Date: 2024-12-23
- Cloning mquickjs from https://github.com/bellard/mquickjs to /tmp

### Key Findings from Code Review

#### Sandboxing Features (Built-in)
1. **Memory isolation**: Engine allocates within a user-provided buffer - no system malloc inside
   - `JS_NewContext(mem_buf, mem_size, &js_stdlib)` - memory buffer is explicit
   - Memory limit is absolute - OOM is a clean exception

2. **Interrupt handler**: `JS_SetInterruptHandler(ctx, handler)` - called periodically
   - Can implement time/CPU limits by returning non-zero to halt execution

3. **Minimal runtime**: No file I/O, no network, no OS access in core engine
   - The `mqjs_stdlib` adds `print`, `setTimeout`, `Date.now`, etc.
   - For sandbox, we can create a minimal stdlib with no dangerous functions

4. **No async/await or modules** - simpler execution model

#### Key API Functions
```c
JSContext *JS_NewContext(void *mem_start, size_t mem_size, const JSSTDLibraryDef *stdlib_def);
JSValue JS_Eval(JSContext *ctx, const char *input, size_t input_len, const char *filename, int eval_flags);
void JS_SetInterruptHandler(JSContext *ctx, JSInterruptHandler *handler);
char *JS_GetErrorStr(JSContext *ctx, char *buf, size_t buf_size);
void JS_FreeContext(JSContext *ctx);
```

#### JavaScript Subset (Key Differences from Full JS)
- ES5-like with strict mode always enabled
- No `with`, no direct `eval` (only indirect/global eval)
- Arrays cannot have holes (no sparse arrays)
- No value boxing (`new Number(1)` not supported)
- Date: only `Date.now()` is supported
- String: `toLowerCase`/`toUpperCase` only handle ASCII
- RegExp: case folding only works with ASCII
- No async/await, no Promises
- No modules (import/export)
- No generators/iterators (except simple `for...of` on arrays)
- No WeakMap/WeakSet
- No Proxy/Reflect
- No Symbol
- Limited Math functions
- `for...in` only iterates own properties

#### Security Analysis: ReDoS (Regex Denial of Service)

**Finding**: mquickjs uses a **backtracking NFA regex engine** (see `lre_exec` function in mquickjs.c)

**Risk**: Potentially vulnerable to ReDoS with pathological patterns like:
- `(a+)+$` against "aaaaaaaaaaaaaaaaaaaaX"
- `(a|a)+$` against many 'a's
- Nested quantifiers generally

**Mitigation**: The regex engine calls `LRE_POLL_INTERRUPT()` during backtracking (line 16928),
which invokes the interrupt handler. This means:
- Time limits WILL work to interrupt pathological regex execution
- The sandbox can stop ReDoS attacks via timeout
- This is a reasonable mitigation but regex-heavy sandboxes should set appropriate time limits

**Recommendation**: For production use:
- Keep time limits low (100-1000ms)
- Consider validating/rejecting suspicious regex patterns before execution
- Document that regex is a potential DoS vector (even with timeouts, it wastes CPU)

### WASM Runtime Comparison: Wasmer vs Wasmtime

Both wasmer and wasmtime struggle with the emscripten-generated WASM module due to:

1. **setjmp/longjmp dependencies**: mquickjs uses setjmp/longjmp for exception handling, which requires emscripten's `invoke_*` trampolines
2. **Emscripten runtime imports**: The module requires `env.setTempRet0`, `env.getTempRet0`, `_emscripten_throw_longjmp`, etc.

**Wasmer observations**:
- Package installed but the Python bindings require native compiler support not available in this environment
- `ImportError: Wasmer is not available on this system`

**Wasmtime observations**:
- Successfully loads module and inspects imports/exports
- Attempted to implement stub functions for emscripten imports
- Stubs don't work properly because `invoke_*` functions need to call through the indirect function table with exception handling

**Conclusion**: For Python WASM integration, the emscripten JS glue is required. Direct wasmtime/wasmer use would require either:
1. Building a truly standalone WASM module without setjmp/longjmp (may require mquickjs source changes)
2. Implementing the full emscripten runtime in Python (very complex)

The WASM module works perfectly with:
- Node.js (using emscripten JS glue)
- Deno (using emscripten JS glue)
- Browsers (using emscripten JS glue)

### Benchmark Results

| Implementation | Startup | Simple Op | Loop 1000 | Recursion |
|---------------|---------|-----------|-----------|-----------|
| C Extension   | 0.01ms  | 0.002ms   | 0.035ms   | 0.085ms   |
| FFI (ctypes)  | 0.04ms  | 0.007ms   | 0.039ms   | 0.086ms   |
| Subprocess    | 0.12ms  | 4.3ms     | 4.7ms     | 4.6ms     |

**Key Findings**:
- C Extension and FFI have similar execution performance (both use same C library)
- C Extension has ~4x faster startup (no ctypes overhead)
- Subprocess is ~500x slower due to process spawning overhead
- For one-off executions: subprocess is still acceptable (~5ms)
- For repeated executions: FFI or C Extension is essential

### mquickjs Commit Used

Repository: https://github.com/bellard/mquickjs
Commit: 17ce6fe54c1ea4f500f26636bd22058fce2ce61a ("doc update")

