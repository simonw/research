# StarlingMonkey WASM Investigation Notes

## Goal
Build starling.wasm from StarlingMonkey and create a playground.html that uses it similar to the QuickJS executor.

## Progress

### Step 1: Clone and Setup
- Cloned StarlingMonkey to /tmp/StarlingMonkey
- Downloaded quickjs.html from https://tools.simonwillison.net/quickjs

### Step 2: Understanding StarlingMonkey
- StarlingMonkey is a SpiderMonkey-based JavaScript runtime for WebAssembly
- Uses WASI 0.2.0 (Component Model) for the event loop
- Designed for server-side runtimes (Fastly Compute, Fermyon Spin)
- NOT designed for browser execution

### Step 3: Build Attempts
- Attempted to build from source using cmake
- Build failed with linker segmentation fault (wasm-ld crashing)
- Tried multiple configurations:
  - Release build: Failed (linker crash)
  - Debug build: Failed (linker crash)
  - Minimal build without crypto: Failed (missing symbols)
- Root cause: LLVM/wasm-ld memory issues in the environment

### Step 4: Pre-built Binary
- Found pre-built releases at https://github.com/bytecodealliance/StarlingMonkey/releases
- Downloaded starling.wasm (10MB) from v0.2.1 release
- Also downloaded starling-raw.wasm for comparison

### Step 5: Testing with wasmtime
- Successfully tested starling.wasm with wasmtime CLI
- Command: `wasmtime -S http starling.wasm -e "console.log('Hello!')"`
- All JavaScript execution tests passed

### Step 6: Python wasmtime Demo
- Created demo_starling.py using:
  - wasmtime-py for module inspection
  - wasmtime CLI for JavaScript execution
- wasmtime-py doesn't support WASI 0.2 Component Model
- Documented the difference between starling.wasm and starling-raw.wasm

### Step 7: Browser Compatibility Investigation
- Installed jco (JavaScript Component Tools)
- Successfully transpiled starling.wasm to browser-compatible JS
- Output: starling-browser/ directory with:
  - starling.core.wasm (9.6MB)
  - starling.js (331KB)
  - Type definitions
- Requires @bytecodealliance/preview2-shim for WASI polyfills
- Complex setup, not suitable for simple HTML playground

### Step 8: Playwright Testing
- quickjs.html loads quickjs-emscripten from CDN
- Network issues in test environment prevented full testing
- QuickJS uses Emscripten (browser-native), StarlingMonkey uses WASI (server-native)

## Key Findings

### starling.wasm vs starling-raw.wasm

| Aspect | starling-raw.wasm | starling.wasm |
|--------|-------------------|---------------|
| Type | Core WASM Module | WASI Component |
| Size | ~10MB | ~10MB |
| Loadable by wasmtime-py | Yes (inspection only) | No |
| Executable by wasmtime CLI | No (needs linking) | Yes |
| Use Case | Specialization with wizer | Runtime JS execution |
| Imports Required | WASI P1 + P2 (82 functions) | Built-in |

### Browser Compatibility

StarlingMonkey is NOT designed for browser execution because:
1. Uses WASI 0.2 Component Model (not supported by browsers natively)
2. Requires server-side WASI implementations
3. jco transpilation possible but complex (requires preview2-shim)

QuickJS-emscripten IS browser-native because:
1. Compiled with Emscripten for browser/JS environment
2. No WASI dependencies
3. Simple script tag inclusion

## Recommendations

For browser-based JavaScript sandboxing:
- Use quickjs-emscripten (simpler, browser-native)
- Or engine262-wasm (if available)

For server-side JavaScript execution:
- Use StarlingMonkey with wasmtime
- Provides full SpiderMonkey engine with modern JS support
- Supports HTTP fetch, timers, and other Web APIs
