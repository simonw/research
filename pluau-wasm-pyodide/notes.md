# Research Notes: Luau (pluau) WebAssembly Build

## Goal

Get the Luau programming language (as used by pluau) compiled to WebAssembly, usable from:
1. Browser JavaScript
2. Pyodide (Python in the browser)
3. Python wasmtime

## Exploration

### pluau structure
- pluau is a Python binding for Luau using PyO3/Maturin (Rust)
- Depends on `mluau` (Rust Luau bindings) which depends on `luau0-src` (vendored Luau C++ source)
- The underlying Luau VM is C++ code from Roblox's luau-lang/luau repo

### Approach: PyO3 WASM vs Direct Luau WASM

Two possible approaches:
1. **Compile pluau (Rust+PyO3) to wasm32-unknown-emscripten** - Very complex, requires cross-compiling Rust+C++ to WASM with PyO3 Pyodide support
2. **Compile Luau C++ directly to WASM** - Much simpler, the official Luau repo already has a `LUAU_BUILD_WEB` CMake option

Chose approach #2 since:
- Official Luau repo already has Web.cpp with emscripten support
- Exports `executeScript()` and `checkScript()` C functions
- Well-tested build path

### Official Luau Web Target

Found in luau-lang/luau repo (GitHub issue #177, PR #138):
- `CLI/src/Web.cpp` implements `executeScript()` and `checkScript()`
- CMake option `LUAU_BUILD_WEB=ON`
- Original uses `-sSINGLE_FILE=1` (embeds WASM in JS)
- Original uses `-fexceptions` for C++ exception handling

### Modifications Needed

1. **Modified Web.cpp (LuauWeb.cpp)**:
   - Removed `checkScript()` and the Analysis dependency (reduces size)
   - Added custom `print()` that captures output to a string buffer
   - `executeScript()` returns captured output or "ERROR:" prefixed errors

2. **CMakeLists.txt changes**:
   - Removed `-sSINGLE_FILE=1` (need separate .wasm for relative path loading)
   - Added `-sMODULARIZE=1 -sEXPORT_NAME=createLuau` (clean JS module loading)
   - Removed `Luau.Analysis` dependency
   - Used `-O1` linking to preserve readable import names (needed for wasmtime)

### Build Process

```bash
source /tmp/emsdk/emsdk_env.sh
emcmake cmake /tmp/luau -DLUAU_BUILD_CLI=OFF -DLUAU_BUILD_TESTS=OFF -DLUAU_BUILD_WEB=ON -DCMAKE_BUILD_TYPE=Release
emmake cmake --build . --target Luau.Web
```

Output: `Luau.Web.js` (47KB) + `Luau.Web.wasm` (658KB)

### Emscripten Import Minification

- With `-O2`, emscripten minifies WASM import/export names (module "a", names "a", "b", etc.)
- This makes the WASM unusable with wasmtime since import names are unreadable
- Solution: use `-O1` for linking which preserves readable names
- The JS glue code still maps minified names to their real functions

### Wasmtime Integration

The emscripten WASM requires these imports:
- **invoke_* trampolines** (18 variants): Call through `__indirect_function_table`, catch C++ exceptions
- **C++ exception handling** (`__cxa_throw`, `__cxa_begin_catch`, `__cxa_end_catch`, `__cxa_find_matching_catch_*`, `__resumeException`, `llvm_eh_typeid_for`)
- **WASI imports** (`clock_time_get`, `fd_write`)
- **Emscripten runtime** (`emscripten_get_now`, `emscripten_date_now`, `emscripten_resize_heap`, `_abort_js`, `_tzset_js`, `_localtime_js`, `_gmtime_js`)

Key insight from mquickjs-sandbox: invoke_* functions save the stack, call through the indirect function table, and catch exceptions (setting `setThrew` flag and restoring stack on failure).

For Luau specifically, C++ exceptions are more involved than the simple setjmp/longjmp in mquickjs:
- `__cxa_throw` stores exception info and raises a Python exception
- `invoke_*` catches these and sets the threw flag
- `__cxa_find_matching_catch_*` returns the caught exception pointer
- `__cxa_begin_catch` / `__cxa_end_catch` manage exception lifecycle

Dynamic invoke_* registration: Instead of hardcoding signatures, we iterate the module's imports and auto-generate invoke functions for any `invoke_*` import.

### Pyodide Integration

The WASM module can be used from Pyodide by:
1. Loading `luau.js` as a script tag
2. From Python, importing `createLuau` from `js`
3. Using `module.cwrap()` to get a callable for `executeScript`
4. Passing Luau source code as a string

Works cleanly since the emscripten module is self-contained.

### Playground HTML

Modeled after tools.simonwillison.net/quickjs:
- Single HTML file with inline CSS/JS
- Loads `luau.js` and `luau.wasm` from relative paths
- Luau tab for direct Luau code execution
- Pyodide tab showing Python calling Luau through WASM
- Code saved in URL hash for sharing
- Example buttons for common Luau patterns
- Ctrl/Cmd+Enter to run

### File Sizes

| File | Size |
|------|------|
| luau.wasm | 658 KB |
| luau.js | 47 KB |
| playground.html | 19 KB |
| luau_wasmtime.py | 10 KB |
