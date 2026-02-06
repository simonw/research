# Monty WASM + Pyodide Build Notes

## What this is

[Monty](https://github.com/pydantic/monty) is a sandboxed Python interpreter written in Rust by the Pydantic team. This project compiles it to WebAssembly in two forms:

1. **Pyodide wheel** (`pydantic_monty-0.0.3-cp313-cp313-emscripten_4_0_9_wasm32.whl`) - A Python wheel that can be loaded into [Pyodide](https://pyodide.org/) and used as a regular Python package
2. **Standalone WASM** (`monty_wasm.js` + `monty_wasm_bg.wasm`) - A browser-native WASM module with JavaScript bindings via `wasm-bindgen`, usable directly from JavaScript without Pyodide

## Build process - Pyodide wheel

### Requirements
- Emscripten SDK 4.0.9 (matches Pyodide 0.29)
- Rust nightly-2025-06-27 (provides Rust 1.90+ required by monty)
- Pre-built wasm-eh sysroot from `pyodide/rust-emscripten-wasm-eh-sysroot`
- maturin (Python build tool for PyO3/Rust extensions)

### Key challenges and solutions

**Duplicate sysroot rlibs**: When adding `wasm32-unknown-emscripten` target via rustup, a standard sysroot is installed. The custom wasm-eh sysroot from Pyodide must *replace* it, not supplement it. Otherwise you get "multiple candidates for `rlib` dependency `core` found" errors. Solution: `rm -rf` the standard sysroot directory before extracting the custom one.

**Invalid export names**: Rust's legacy symbol mangling (v0 vs legacy) produces names with backtick characters (`$LT`, `$GT`, etc.) which emscripten rejects in `SIDE_MODULE=2` exports. Solution: add `-C symbol-mangling-version=v0` to RUSTFLAGS.

**Python version matching**: Pyodide 0.29 uses Python 3.13, so the wheel must be built with `-i 3.13` to get the correct `cp313` tag. Without this flag, maturin defaults to the system Python version.

### RUSTFLAGS for Pyodide wheel

```
-Z emscripten-wasm-eh          # Use native WASM exception handling
-C link-arg=-sSIDE_MODULE=2    # Build as Emscripten side module
-C link-arg=-sWASM_BIGINT      # Enable BigInt support
-C relocation-model=pic        # Position-independent code
-Z link-native-libraries=no    # Don't link native libs
-C symbol-mangling-version=v0  # Avoid backtick chars in exports
```

## Build process - Standalone WASM

### Approach
Created a new crate `monty-wasm` in the monty workspace that wraps the core `monty` crate with `wasm-bindgen` bindings. Built with `wasm-pack build --target web` targeting `wasm32-unknown-unknown`.

### Key challenges

**getrandom crate**: Monty depends on `getrandom` for random number generation. For `wasm32-unknown-unknown`, this requires the `wasm_js` feature and `--cfg getrandom_backend="wasm_js"` RUSTFLAG. Critically, this flag must be set only for the WASM target (using `CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS`) to avoid breaking the native build of `wasm-bindgen-cli`.

**Monty API oddities**:
- `CollectStringPrint.output` is a method (`.into_output()`), not a field
- `DictPairs.iter()` is private, but `IntoIterator for &DictPairs` is public, so `for (k,v) in pairs` works
- `MontyObject` references need careful handling of owned vs borrowed types

**wasm-opt unavailable**: wasm-pack tries to download wasm-opt, which may fail. Disabled via `[package.metadata.wasm-pack.profile.release] wasm-opt = false` in Cargo.toml. Installed binaryen via apt and ran wasm-opt manually afterward.

### Output
- `monty_wasm.js` (14KB) - ES module with `Monty` class and `init()` function
- `monty_wasm_bg.wasm` (2.8MB after wasm-opt) - The WASM binary

## Demo pages

### demo.html (standalone WASM)
Uses `<script type="module">` to load `monty_wasm.js` directly. Styled similar to [tools.simonwillison.net/micropython](https://tools.simonwillison.net/micropython). Features:
- Code editor with Python examples
- Run button with Ctrl/Cmd+Enter shortcut
- Tab key for indentation
- URL hash sharing (code is saved in URL)
- Print output capture + expression result display

### pyodide-demo.html (Pyodide + wheel)
Loads Pyodide from CDN, installs the pydantic_monty wheel via micropip, and provides a Python REPL that uses the full pydantic_monty API.

## Playwright tests

### Configuration
- Uses **Firefox** (Chromium crashes when loading WASM files in this container environment)
- Uses a **Node.js HTTP server** (`serve.js`) instead of Python's `http.server` for correct MIME types (ES module imports require `text/javascript` content type)
- Pyodide runtime files are served locally (downloaded to `pyodide/` directory)
- `HOME=/root` must be set for Firefox to launch in this environment

### Test structure
- `tests/monty-wasm.spec.js` - 7 tests for standalone WASM (loads, UI interaction, API, error handling, multiline code, examples)
- `tests/monty-pyodide.spec.js` - 8 tests for Pyodide wheel (import, arithmetic, inputs, strings, lists, error handling, reuse, multiline)

### Bug found during testing
`demo.html` had a JavaScript string quoting bug - the Python `strings` example contained `{' '.join(...)}` inside a single-quoted JS string, which broke the JS parser. Fixed by switching that example to double-quoted JS string with escaped double quotes.

## File sizes

| File | Size |
|------|------|
| `pydantic_monty-0.0.3-cp313-cp313-emscripten_4_0_9_wasm32.whl` | 4.0 MB |
| `monty_wasm_bg.wasm` | 2.8 MB |
| `monty_wasm.js` | 14 KB |
| `demo.html` | 12 KB |
| `pyodide-demo.html` | 12 KB |
