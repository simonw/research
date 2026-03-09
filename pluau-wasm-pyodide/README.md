# Luau WebAssembly: Browser Playground + Python wasmtime

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

An investigation of compiling [Luau](https://luau.org/) (the language used by [pluau](https://github.com/gluau/pluau)) to WebAssembly for use in browsers (with Pyodide) and in Python via wasmtime.

## What This Is

[Luau](https://github.com/luau-lang/luau) is a fast, small, safe scripting language derived from Lua, created by Roblox. [pluau](https://github.com/gluau/pluau) provides Python bindings via PyO3/Maturin.

This project compiles the Luau VM and compiler to WebAssembly using Emscripten, then provides:

1. **A browser playground** (`playground.html`) for running Luau code interactively
2. **Pyodide integration** for calling Luau from Python running in the browser
3. **A Python wasmtime wrapper** (`luau_wasmtime.py`) for running Luau from Python on the server

## Files

| File | Description |
|------|-------------|
| `playground.html` | Interactive Luau playground (browser) |
| `luau.js` | Emscripten JS glue (47 KB) |
| `luau.wasm` | Luau WASM module (658 KB, works for browser and wasmtime) |
| `luau_wasmtime.py` | Python wasmtime wrapper |
| `LuauWeb.cpp` | Modified Luau Web.cpp with output capture |
| `build_wasm.sh` | Build script |
| `luau-repo-changes.diff` | All changes made to the official Luau repo |

## Browser Playground

Open `playground.html` in a browser. It loads `luau.js` and `luau.wasm` from the same directory using relative paths.

Features:
- Execute Luau code with print output capture
- 11 example snippets (Fibonacci, metatables, string interpolation, etc.)
- Code saved in URL hash for sharing
- Ctrl/Cmd+Enter keyboard shortcut
- Pyodide tab for calling Luau from Python in the browser
- All libraries loaded from relative paths for easy deployment

### Deploying

Copy these files to any static file server:
- `playground.html`
- `luau.js`
- `luau.wasm`

The Pyodide tab loads Pyodide from CDN on first use.

## Python wasmtime Usage

```bash
pip install wasmtime
python luau_wasmtime.py
```

```python
from luau_wasmtime import LuauWasmtime

vm = LuauWasmtime()

output, error = vm.execute('print("Hello from Luau!")')
print(output)  # "Hello from Luau!\n"

output, error = vm.execute('print(2 + 2)')
print(output)  # "4\n"

# Error handling
output, error = vm.execute('print(x.y)')
print(error)  # "stdin:1: attempt to index nil with 'y'\n..."
```

## How It Works

### Building Luau to WASM

The official [luau-lang/luau](https://github.com/luau-lang/luau) repo has a `LUAU_BUILD_WEB` CMake option that compiles `CLI/src/Web.cpp` with Emscripten. We modified this to:

1. **Capture print output** — Custom `print()` function writes to a string buffer instead of stdout
2. **Return output from `executeScript()`** — Returns captured output or `"ERROR:"` + error message
3. **Separate WASM file** — Removed `-sSINGLE_FILE=1`, added `-sMODULARIZE=1`
4. **Readable imports** — Used `-O1` linking to preserve import names for wasmtime compatibility

### wasmtime: Implementing Emscripten Runtime in Python

The Emscripten-compiled WASM requires ~34 imported functions. The key challenge is implementing C++ exception handling:

- **invoke_\* trampolines** (18 variants) — Call through `__indirect_function_table`, catch exceptions, set `setThrew` flag
- **\_\_cxa_\* functions** — C++ exception lifecycle (`throw`, `begin_catch`, `end_catch`, `find_matching_catch`)
- **WASI** — `clock_time_get`, `fd_write`
- **Emscripten runtime** — Time functions, memory resize, abort

The invoke_\* functions are auto-discovered from the WASM module's imports, so the wrapper adapts to different builds.

### Relationship to pluau

pluau wraps Luau in Rust via PyO3. Compiling pluau itself to WASM (Rust + PyO3 → wasm32-unknown-emscripten) would be significantly more complex. Instead, this project compiles the underlying Luau C++ VM directly, providing a simpler path to WASM.

The WASM module provides `executeScript(code)` which creates a fresh sandboxed Luau VM for each call, compiles the code to bytecode, executes it, and returns the output.

## Building from Source

Prerequisites: Emscripten SDK, official Luau repo cloned to `/tmp/luau`

```bash
# Install Emscripten
cd /tmp && git clone https://github.com/emscripten-core/emsdk.git
cd emsdk && ./emsdk install latest && ./emsdk activate latest
source emsdk_env.sh

# Clone Luau
cd /tmp && git clone https://github.com/luau-lang/luau.git

# Build
cd /path/to/this/directory
bash build_wasm.sh
```

## Performance

| Context | Startup | Simple eval |
|---------|---------|------------|
| Browser (Chrome) | ~50ms | <1ms |
| Node.js | ~100ms | <1ms |
| Python wasmtime | ~200ms | ~5ms |

The wasmtime overhead comes from WASM compilation on startup and Python/WASM boundary crossing for each invoke_\* call (C++ exception handling).
