# Research Notes: Building PyO3/Maturin Rust Extensions as WASM Wheels for Pyodide

## Research Process

### Sources Consulted
- Pyodide ABI documentation (v0.29.2 stable, v0.30.0.dev0 latest)
- Pyodide blog post on Rust/PyO3 support
- Maturin GitHub repo (PRs #974, #1484, issue #904, #2549)
- PyO3 GitHub issue #2412 (wasm32-emscripten support)
- pyodide/rust-emscripten-wasm-eh-sysroot repository
- Rustc platform support docs for wasm32-unknown-emscripten
- pydantic-core CI workflow (real-world example)
- Emscripten SDK documentation
- micropip documentation
- Python.org PEP 783 discussions (Emscripten packaging)

### Key Findings Timeline

1. Maturin added wasm32-unknown-emscripten support in PR #974 (late 2022), minimum version 0.14.14
2. PyO3 has had wasm32-emscripten support tracked since issue #2412
3. January 15, 2025: Rust nightly gained `-Z emscripten-wasm-eh` flag
4. February 2025: Pre-built sysroot released for nightly-2025-02-01
5. July 2025: Pyodide 0.28.0 released with Python 3.13 and Emscripten 4.0.9 ABI
6. October 2025: Pyodide 0.29.x is current stable with Python 3.13.2
7. There is an ongoing Rust compiler-team MCP (#920) to make emscripten-wasm-eh the default

### Important Discoveries

- The Emscripten ABI is NOT stable across versions - you MUST match the exact emsdk version
- Two different ABIs exist: older Pyodide (emscripten_3_1_58_wasm32) and newer (pyodide_2025_0 with emsdk 4.0.9)
- The sysroot problem is the biggest pain point - Rust's shipped sysroot doesn't have emscripten-wasm-eh
- `-Zbuild-std` is an alternative to the custom sysroot but has bugs (doesn't work with panic=abort, cargo vendor)
- pydantic-core is the best real-world reference project for this workflow
- maturin handles most of the complexity but RUSTFLAGS still need to be set correctly
- PyPI does not yet accept wasm32 wheels (PEP 783 in progress); custom hosting required
- A recent Rust nightly regression (v1.88.0-nightly) broke emscripten builds with "--enable-bulk-memory-opt" error

### Pyodide Version / Python Version / Emscripten Version Mapping

| Pyodide | Python  | Emscripten | Platform Tag                    |
|---------|---------|------------|---------------------------------|
| 0.26.x  | 3.12.1  | 3.1.45     | emscripten_3_1_45_wasm32        |
| 0.27.x  | 3.12.7  | 3.1.58     | emscripten_3_1_58_wasm32        |
| 0.28.x  | 3.13    | 4.0.9      | emscripten_4_0_9_wasm32 (new ABI) |
| 0.29.x  | 3.13.2  | 4.0.9      | emscripten_4_0_9_wasm32         |

### Two ABI Eras

**Pre-2025 ABI (Pyodide 0.27.x and earlier):**
- Emscripten 3.1.58
- JS-based exception handling
- Simpler Rust setup (no -Z emscripten-wasm-eh needed)
- Platform tag: `emscripten_3_1_58_wasm32`

**2025 ABI (Pyodide 0.28.x+):**
- Emscripten 4.0.9
- WASM-native exception handling
- Requires `-Z emscripten-wasm-eh` and custom sysroot
- Platform tag: `emscripten_4_0_9_wasm32` (future: `pyodide_2025_0_wasm32`)

### Tricky Parts Learned

1. maturin sets RUSTFLAGS env var which causes cargo to ignore .cargo/config.toml files
2. `-sSIDE_MODULE=1` doesn't work with Rust due to lib.rmeta files; must use `=2`
3. `-pthread` at compile or link time will cause resulting libraries to not load at all
4. The `.wasm` vs `.so` extension mismatch can trip up setuptools-rust (maturin handles it)
5. When using a workspace, Cargo doesn't read config from individual crates
6. CORS headers are required when serving wheels from a custom URL for micropip
