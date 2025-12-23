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

**Solution Found**: After deeper investigation, wasmtime CAN work by properly implementing the `invoke_*` trampolines:

1. The invoke_* functions must call through the `__indirect_function_table` export
2. They must catch WASM traps and Python exceptions that signal longjmp
3. They must call `setThrew(1, 0)` when a longjmp occurs
4. They must restore the stack via `stackRestore`

The working implementation:
- Uses the WASM indirect function table to make actual function calls
- Catches both `wasmtime.Trap` and custom `LongjmpException`
- Properly manages the emscripten setjmp/longjmp state

The WASM module works with:
- Node.js (using emscripten JS glue)
- Deno (using emscripten JS glue)
- Browsers (using emscripten JS glue)
- **Python wasmtime** (using custom invoke_* implementation)

### Benchmark Results

| Implementation | Startup | Simple Op | Loop 1000 | Recursion |
|---------------|---------|-----------|-----------|-----------|
| C Extension   | 0.01ms  | 0.002ms   | 0.033ms   | 0.082ms   |
| FFI (ctypes)  | 0.02ms  | 0.007ms   | 0.038ms   | 0.086ms   |
| **Wasmtime**  | 57.6ms  | 2.9ms     | 5.6ms     | 6.2ms     |
| Subprocess    | 0.09ms  | 3.8ms     | 3.8ms     | 3.6ms     |

**Key Findings**:
- C Extension and FFI have similar execution performance (both use same C library)
- C Extension has ~4x faster startup (no ctypes overhead)
- Subprocess is ~500x slower than FFI due to process spawning overhead
- **Wasmtime works but has significant overhead** (~300x slower than FFI)
- Wasmtime has slow startup (~58ms) due to WASM compilation
- For one-off executions: subprocess is acceptable (~4ms)
- For repeated executions: FFI or C Extension is essential
- Wasmtime is viable for security-critical scenarios despite performance cost

### mquickjs Commit Used

Repository: https://github.com/bellard/mquickjs
Commit: 17ce6fe54c1ea4f500f26636bd22058fce2ce61a ("doc update")

