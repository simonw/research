# mquickjs Sandbox Investigation

An investigation of [mquickjs](https://github.com/bellard/mquickjs) as a safe sandboxing environment for running untrusted JavaScript code.

**mquickjs commit used**: `17ce6fe54c1ea4f500f26636bd22058fce2ce61a`

## Overview

mquickjs is a minimal JavaScript engine by Fabrice Bellard, designed to run in as little as 10KB of RAM. This investigation explores using it as a secure sandbox for executing untrusted code with:

- **Memory limits** - Engine allocates within a user-provided buffer
- **Time limits** - Interrupt handler for CPU time control
- **No file/network access** - Minimal runtime with no OS access

## Implementations

This project provides multiple ways to use mquickjs as a sandbox:

### 1. FFI Bindings (ctypes)

**Files**: `mquickjs_ffi.py`, `build_ffi.py`, `libmquickjs_sandbox.so`

```python
from mquickjs_ffi import MQuickJSFFI, execute_js

# One-shot execution
result = execute_js("1 + 2")  # Returns 3

# Reusable sandbox
sandbox = MQuickJSFFI(memory_limit_bytes=1024*1024, time_limit_ms=1000)
sandbox.execute("var x = 42")
result = sandbox.execute("x * 2")  # Returns 84
sandbox.close()
```

### 2. Python C Extension

**Files**: `mquickjs_ext.c`, `setup.py`

```python
import mquickjs_ext

sandbox = mquickjs_ext.Sandbox(memory_limit_bytes=1024*1024, time_limit_ms=1000)
result = sandbox.execute("Math.sqrt(16)")  # Returns 4.0
sandbox.close()
```

Build with: `python3 setup.py build_ext --inplace`

### 3. Subprocess Wrapper

**Files**: `mquickjs_subprocess.py`

Uses the unmodified `mqjs` binary via subprocess. Simpler but slower (~500x) due to process spawning.

```python
from mquickjs_subprocess import execute_js

result = execute_js("1 + 2")  # Returns 3
```

### 4. WebAssembly (Node.js/Deno/Browser)

**Files**: `build_wasm.py`, `mquickjs.js`, `mquickjs.wasm`

```javascript
// Node.js
const createMQuickJS = require('./mquickjs.js');
const Module = await createMQuickJS({ wasmBinary: fs.readFileSync('./mquickjs.wasm') });

const sandbox_init = Module.cwrap('sandbox_init', 'number', ['number']);
const sandbox_eval = Module.cwrap('sandbox_eval', 'string', ['string']);

sandbox_init(1024 * 1024);
const result = sandbox_eval("1 + 2");  // Returns "3"
```

### 5. Browser/Pyodide Integration

**Files**: `test_wasm_browser.html`, `mquickjs_pyodide.py`

The WASM module can be loaded directly in browsers and used with Pyodide for Python-in-browser scenarios.

## Benchmark Results

| Implementation | Startup | Simple Op | Loop 1000 | Recursion (fib 15) |
|---------------|---------|-----------|-----------|-----------|
| **C Extension** | 0.01ms | **0.002ms** | **0.035ms** | **0.085ms** |
| **FFI (ctypes)** | 0.04ms | 0.007ms | 0.039ms | 0.086ms |
| **Subprocess** | 0.12ms | 4.3ms | 4.7ms | 4.6ms |

**Key Findings**:
- C Extension and FFI have near-identical execution performance
- C Extension has ~4x faster startup (no ctypes overhead)
- Subprocess is ~500x slower due to process spawn overhead
- For interactive/repeated use: prefer FFI or C Extension
- For one-off use: subprocess is acceptable (~5ms latency)

## Security Analysis

### Sandboxing Strengths

1. **Memory Isolation**: Engine allocates within user-provided buffer only
   - `JS_NewContext(mem_buf, mem_size, &js_stdlib)` - explicit memory bounds
   - OOM results in clean exception, not crash

2. **Time Limits**: Interrupt handler called during execution
   - `JS_SetInterruptHandler(ctx, handler)` - return non-zero to halt
   - Works during regex matching (prevents ReDoS)

3. **No Dangerous APIs**: Core engine has no file/network access
   - Can provide minimal stdlib with only safe functions

### ReDoS Vulnerability Analysis

**Finding**: mquickjs uses a backtracking NFA regex engine

**Risk**: Vulnerable to pathological patterns like `(a+)+$`

**Mitigation**: The interrupt handler is called during regex backtracking (`LRE_POLL_INTERRUPT()`), so time limits will interrupt pathological patterns.

**Recommendation**:
- Keep time limits low (100-1000ms)
- Consider pre-validating regex patterns if accepting user input
- Document that regex is a potential DoS vector

### JavaScript Dialect Differences

mquickjs implements a subset of JavaScript (ES5-like):

| Feature | mquickjs | Standard JS |
|---------|----------|-------------|
| Strict mode | Always on | Optional |
| Sparse arrays | Not supported | Supported |
| `new Number(1)` | Not supported | Supported |
| Date | `Date.now()` only | Full Date API |
| toLowerCase/toUpperCase | ASCII only | Unicode |
| async/await | No | Yes |
| Promises | No | Yes |
| Modules | No | Yes |
| Generators | No | Yes |
| WeakMap/WeakSet | No | Yes |
| Proxy/Reflect | No | Yes |
| Symbol | No | Yes |
| `for...in` | Own props only | Inherited too |

## WASM Runtime Notes: Wasmer vs Wasmtime

Attempted to run the WASM module directly with Python WASM runtimes:

**Wasmer**: Python bindings require native compiler support not available in all environments.

**Wasmtime**: Successfully implemented! Required implementing proper `invoke_*` trampolines that:
1. Call through the `__indirect_function_table`
2. Catch exceptions signaling `longjmp`
3. Manage the emscripten setjmp/longjmp state via `setThrew`/`stackRestore`

The WASM module works with:
- Node.js (via emscripten JS glue)
- Deno (via emscripten JS glue)
- Browsers (via emscripten JS glue)
- **Python wasmtime** (via custom invoke_* implementation)

See the Appendix for a detailed explanation of the wasmtime implementation.

## Files Included

| File | Description |
|------|-------------|
| `api_design.py` | Base classes and consistent API |
| `mquickjs_ffi.py` | FFI (ctypes) implementation |
| `build_ffi.py` | Build script for FFI shared library |
| `mquickjs_ext.c` | C Extension source |
| `setup.py` | C Extension build script |
| `mquickjs_subprocess.py` | Subprocess wrapper |
| `mquickjs_wasmtime.py` | Wasmtime Python implementation |
| `build_wasm.py` | WASM build script |
| `build_wasm_wasmtime.py` | Alternative WASM build for wasmtime |
| `mquickjs.js` | Emscripten JS glue |
| `mquickjs.wasm` | WASM binary (for Node/Deno/Browser) |
| `mquickjs_standalone.wasm` | Standalone WASM (for wasmtime) |
| `test_sandbox.py` | Pytest tests (35 tests) |
| `test_wasm_node.js` | Node.js WASM tests |
| `test_wasm_deno.ts` | Deno WASM tests |
| `test_wasm_browser.py` | Playwright browser tests |
| `benchmark.py` | Performance benchmarks |
| `notes.md` | Investigation notes |

## Running Tests

```bash
# FFI/C Extension tests
python3 -m pytest test_sandbox.py -v

# Node.js WASM tests
node test_wasm_node.js

# Deno WASM tests
deno run --allow-read test_wasm_deno.ts

# Browser WASM tests (requires Playwright)
python3 -m pytest test_wasm_browser.py -v

# Benchmarks
python3 benchmark.py
```

## Building

```bash
# Build FFI library
python3 build_ffi.py

# Build C Extension
python3 setup.py build_ext --inplace

# Build WASM
python3 build_wasm.py
```

## Dependencies

- Python 3.8+
- GCC (for FFI and C Extension)
- Emscripten (for WASM build)
- Node.js 18+ (for Node tests)
- Deno (for Deno tests)
- Playwright (for browser tests)

## Conclusion

mquickjs is an excellent choice for JavaScript sandboxing:

1. **Security**: Built-in memory bounds, interrupt handling, minimal attack surface
2. **Performance**: Microsecond-level execution times with FFI/C Extension
3. **Portability**: Runs on Linux, macOS, Windows, and WASM
4. **Simplicity**: ~400KB of C code, easy to audit

**Recommended approach**: Use the FFI implementation for most cases. It provides good performance without requiring compilation and works across platforms.

---

## Appendix: Wasmtime Integration Deep Dive

### The Challenge

Getting mquickjs to run in Python via wasmtime required solving a complex problem: mquickjs uses `setjmp`/`longjmp` for error handling during JavaScript parsing. When compiled to WASM, emscripten translates these to special `invoke_*` functions that wrap indirect calls with exception handling.

### How setjmp/longjmp Works in Emscripten WASM

1. **setjmp** saves the current stack state to a buffer
2. **longjmp** restores that state and continues from setjmp
3. In WASM, this is implemented via:
   - `invoke_*` trampolines that catch exceptions
   - `_emscripten_throw_longjmp` to signal a longjmp
   - `setThrew`/`testSetjmp` for state management

The emscripten-generated WASM exports these imports:
```
env.invoke_iii(index, a1, a2) -> i32
env.invoke_iiii(index, a1, a2, a3) -> i32
env.invoke_vi(index, a1) -> void
... etc
env.setTempRet0(value)
env.getTempRet0() -> i32
env._emscripten_throw_longjmp()
```

### Failed Attempts

1. **SUPPORT_LONGJMP=wasm**: Emscripten's native WASM exceptions use the WASM exception handling proposal, but wasmtime requires `legacy_exceptions` feature which isn't widely available.

2. **Stub invoke_* functions**: Simply returning 0 from invoke_* doesn't work because the actual function calls never happen.

3. **Removing setjmp/longjmp from source**: Would require extensive modifications to the parser's error handling throughout mquickjs.c.

### Working Solution

The key insight is that `invoke_*` functions must:
1. Call through the `__indirect_function_table`
2. Catch any "longjmp" exceptions (WASM traps or Python exceptions)
3. Set the `threw` flag via `setThrew` export
4. Return control to allow the WASM code to handle the error

Implementation in Python:

```python
def make_invoke_fn(return_type, param_count):
    def invoke_fn(*args):
        index = args[0]
        params = args[1:]

        # Save stack
        stack_ptr = self._stackSave(self.store)

        try:
            # Call through indirect table
            func = self._table.get(self.store, index)
            result = func(self.store, *params)
            return result

        except (LongjmpException, wasmtime.Trap):
            # longjmp was called
            self._setThrew(self.store, 1, 0)
            self._stackRestore(self.store, stack_ptr)
            return 0

    return invoke_fn
```

### Performance Characteristics

| Implementation | Startup | Simple Op | Recursion |
|---------------|---------|-----------|-----------|
| C Extension   | 0.01ms  | 0.002ms   | 0.082ms   |
| FFI (ctypes)  | 0.02ms  | 0.007ms   | 0.086ms   |
| **Wasmtime**  | 57.6ms  | 2.9ms     | 6.2ms     |
| Subprocess    | 0.09ms  | 3.8ms     | 3.6ms     |

Wasmtime is ~300x slower than FFI for simple operations due to:
- WASM compilation on startup (~58ms)
- Python/WASM boundary crossing for each invoke_* call
- Exception handling overhead for longjmp support

### When to Use Wasmtime

Despite the performance cost, wasmtime may be preferable when:
- You need the security guarantees of WASM isolation
- You can't install shared libraries (e.g., serverless functions)
- You want to run the same WASM in Python and browsers
- Startup time is amortized over many executions

### Files

- `mquickjs_wasmtime.py` - Python wasmtime implementation with proper invoke_* support
- `mquickjs_standalone.wasm` - Standalone WASM module (used with wasmtime)
- `build_wasm_wasmtime.py` - Alternative build script for wasmtime experiments
