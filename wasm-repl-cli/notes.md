# WASM REPL CLI Investigation Notes

## Tue Feb  3 20:16:45 UTC 2026: Starting investigation

### Wazero Overview
- wazero is a Go WebAssembly runtime with zero dependencies
- Has full WASI snapshot preview 1 support
- Supports both compiler (AOT) and interpreter modes
- Can mount filesystem directories into WASM modules

### Downloads
- CPython WASI: v3.14.2 with WASI SDK 24 from brettcannon/cpython-wasi-build
  - python.wasm: ~30MB
  - lib/ directory with Python stdlib
  - Requires PYTHONHOME=/lib and PYTHONPATH=/lib environment variables

- QuickJS WASI: v0.11.0 from quickjs-ng/quickjs
  - qjs-wasi.wasm: ~1.4MB
  - Self-contained, no additional files needed

### Testing with wazero CLI
- QuickJS works: `wazero run qjs.wasm -- -e 'console.log("Hello")'`
- Python works: `wazero run --env PYTHONHOME=/lib --env PYTHONPATH=/lib --mount lib:/lib python.wasm -- -c 'print(1+1)'`

### Architecture
Building two separate Go binaries:
1. pyrunner - Python WASM runner with --jsonl mode
2. jsrunner - JavaScript WASM runner with --jsonl mode

## Implementation Details

### jsrunner
- Embeds the QuickJS WASM binary (~1.4MB) directly in the Go binary using `//go:embed`
- Self-contained, no external runtime files needed
- State persistence in JSONL mode achieved by replaying previous code with suppressed output

### pyrunner
- Requires external runtime directory with python.wasm (~30MB) and lib/ (~14MB)
- Too large to embed practically
- State persistence in JSONL mode achieved via a Python script (`jsonl_runner.py`) that maintains a global namespace

### JSONL Mode Implementation
- jsrunner: Each request compiles and instantiates a fresh WASM module, but replays all previous code first
- pyrunner: Single persistent Python process running the JSONL processor script

### Key Challenges Solved
1. **QuickJS WASI stdin limitation**: The QuickJS WASI build doesn't have the `std` module for reading stdin line by line. Solved by doing JSONL processing in Go and passing code via command-line arguments.

2. **State persistence**: WASM module state doesn't persist between instantiations. For jsrunner, solved by code replay. For pyrunner, solved by running a persistent Python process.

3. **Python environment setup**: CPython WASI needs PYTHONHOME and PYTHONPATH environment variables, plus the lib/ directory mounted at the right path.

## Test Results
All 18 pytest tests pass:
- 7 jsrunner tests (eval, variables, functions, JSONL mode, state persistence, error handling)
- 9 pyrunner tests (eval, variables, functions, imports, JSONL mode, state persistence, error handling)
- 2 integration tests (math comparison, UUID tracking)
