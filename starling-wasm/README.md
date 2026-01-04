# StarlingMonkey WASM Investigation

This investigation explores [StarlingMonkey](https://github.com/bytecodealliance/StarlingMonkey), a SpiderMonkey-based JavaScript runtime compiled to WebAssembly.

## Overview

StarlingMonkey is a production-grade JavaScript runtime used by Fastly's Compute platform and Fermyon's Spin JS SDK. It provides a standards-compliant implementation of key web APIs (fetch, streams, text encoding) on top of Mozilla's SpiderMonkey engine, compiled to WebAssembly.

## Key Finding: Server-Side vs Browser

**StarlingMonkey is designed for server-side execution, not browser use.**

| Feature | StarlingMonkey | QuickJS-emscripten |
|---------|---------------|-------------------|
| Target | WASI runtimes (wasmtime, etc.) | Browsers (Emscripten) |
| Component Model | WASI 0.2 | N/A |
| Browser-native | No | Yes |
| Setup complexity | High | Low (CDN include) |
| JS Engine | SpiderMonkey (Firefox) | QuickJS |

## Files Included

- **starling.wasm** - The componentized runtime (10MB), runs JavaScript via wasmtime
- **starling-raw.wasm** - Core WASM module for specialization/inspection
- **demo_starling.py** - Python script demonstrating wasmtime usage
- **notes.md** - Detailed investigation notes
- **test_quickjs.py** - Playwright test for quickjs.html comparison

## starling.wasm vs starling-raw.wasm

StarlingMonkey provides two WASM binaries with different purposes:

### starling-raw.wasm (Core Module)

```
$ python -c "
import wasmtime
module = wasmtime.Module.from_file(wasmtime.Engine(), 'starling-raw.wasm')
print('Exports:', [e.name for e in module.exports])
"
# Exports: ['memory', '_initialize', 'wizer.initialize', 'wizer.resume',
#           'cabi_realloc', 'wasi:cli/run@0.2.3#run',
#           'wasi:http/incoming-handler@0.2.3#handle']
```

- **Type**: Core WebAssembly module
- **Purpose**: Specialization using wizer/componentize for specific JS applications
- **Imports**: 82 functions from WASI Preview 1 + Preview 2
- **Can be inspected** by wasmtime-py
- **Cannot run** JavaScript directly (needs linking)

### starling.wasm (Component)

```bash
$ wasmtime -S http starling.wasm -e "console.log('Hello from SpiderMonkey!')"
Log: Hello from SpiderMonkey!
```

- **Type**: WebAssembly Component (WASI 0.2)
- **Purpose**: Runtime JavaScript execution
- **Self-contained**: All WASI imports resolved
- **Cannot be loaded** by wasmtime-py (uses Component Model)
- **Runs JavaScript** directly via wasmtime CLI

## Usage Examples

### Running JavaScript with wasmtime CLI

```bash
# Simple expression
wasmtime -S http starling.wasm -e "console.log('Hello World!')"

# Multi-line script
wasmtime -S http starling.wasm -e "
function fibonacci(n) {
    const seq = [0, 1];
    for (let i = 2; i < n; i++) {
        seq.push(seq[i-1] + seq[i-2]);
    }
    return seq;
}
console.log(fibonacci(15).join(', '));
"

# Run a file
wasmtime -S http --dir . starling.wasm script.js
```

### Python Demo

```bash
# Requires wasmtime CLI to be installed
uv run demo_starling.py
```

Output:
```
StarlingMonkey WASM JavaScript Interpreter Demo
Using wasmtime-py for inspection, wasmtime CLI for execution

============================================================
WASM Module Inspection using wasmtime-py
============================================================

Module: starling-raw.wasm
  Validated: Yes
  Exports (7):
    - memory: Memory
    - _initialize: Func
    - wizer.initialize: Func
    ...
  Imports (82):
    - wasi:cli/environment@0.2.3: 1 functions
    - wasi:clocks/monotonic-clock@0.2.3: 3 functions
    ...

============================================================
JavaScript Execution via wasmtime CLI
============================================================

--- Hello World ---
Log: Hello from StarlingMonkey!

--- Fibonacci ---
Log: Fibonacci(15): 0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233, 377
...
```

## Browser Compatibility

StarlingMonkey can be transpiled for browser use using [jco](https://github.com/bytecodealliance/jco):

```bash
npm install -g @bytecodealliance/jco
jco transpile starling.wasm -o starling-browser
```

However, this produces a complex output requiring `@bytecodealliance/preview2-shim` for WASI polyfills. This is not as simple as QuickJS-emscripten's single script tag approach.

**For browser-based JS sandboxing, quickjs-emscripten remains the simpler choice.**

## Why Build Failed

Attempts to build StarlingMonkey from source failed due to linker crashes (wasm-ld segfaults). The build process involves:
- WASI SDK (clang/wasm-ld for WebAssembly)
- SpiderMonkey static library (~100MB)
- OpenSSL for web crypto
- Multiple Rust crates

Pre-built binaries from the [GitHub releases](https://github.com/bytecodealliance/StarlingMonkey/releases) work correctly.

## Recommendations

| Use Case | Recommended Solution |
|----------|---------------------|
| Browser JS sandbox | quickjs-emscripten |
| Server-side JS execution | StarlingMonkey + wasmtime |
| Edge computing | StarlingMonkey (Fastly, Fermyon) |
| Embedded JS | QuickJS (smaller, simpler) |

## References

- [StarlingMonkey GitHub](https://github.com/bytecodealliance/StarlingMonkey)
- [StarlingMonkey Documentation](https://bytecodealliance.github.io/StarlingMonkey/)
- [wasmtime](https://wasmtime.dev/)
- [jco - JavaScript Component Tools](https://github.com/bytecodealliance/jco)
- [quickjs-emscripten](https://github.com/nicolo-ribaudo/quickjs-emscripten)
