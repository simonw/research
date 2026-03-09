Luau WebAssembly explores compiling the Luau scripting language (used by Roblox) to WebAssembly for interactive browser environments and Python integration via wasmtime. By leveraging Emscripten, the project creates a streamlined WASM module that runs in the browser (with a playground and Pyodide integration) and server-side Python. Key technical adaptations include custom output capture, flexible WASM imports for wasmtime, and Python wrappers that handle C++ exception lifecycles. The result is a compact setup enabling Luau scripts to execute reliably across platforms with minimal performance overhead, without the heavier Rust-based pluau bindings.

**Key findings:**
- Luau compiled to WASM runs efficiently (<1ms eval in browser, ~5ms in Python with wasmtime).
- Custom Emscripten imports allow flexible deployment and simplified C++ exception handling in Python.
- The playground enables sharing code via URL hashes and supports Python/Luau interaction in-browser via Pyodide.
- Compiling Luau VM directly is much simpler than porting the full pluau Rust bindings to WASM.

References:
- [Luau language](https://luau.org/)
- [wasmtime Python library](https://github.com/wasmtime/wasmtime-py)
