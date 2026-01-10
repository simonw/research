# Pyodide Deno Wrapper Investigation

## Initial Setup

Starting investigation into denobox and Pyodide integration.

## Exploring denobox architecture

denobox works by:
1. Starting a Deno subprocess running `worker.js`
2. Communicating via NDJSON over stdin/stdout
3. The worker handles: eval, load_wasm, call_wasm, unload_wasm, shutdown commands
4. By default, Deno runs with no permissions (fully sandboxed)

Key insight: The sandboxed Deno has no network access, so `import("npm:pyodide")` fails.

## Approach for PyodideBox

Need to create a custom Deno worker that:
1. Has network permission to download Pyodide (or bundles it)
2. Initializes Pyodide once
3. Exposes Python execution via the same NDJSON protocol

## Implementation Details

### pyodide_worker.js

Created a Deno worker script that:
- Imports Pyodide from npm (with CDN fallback)
- Handles NDJSON communication like denobox
- Supports these request types:
  - `init`: Initialize Pyodide runtime
  - `run_python`: Execute Python code
  - `set_global`: Set a Python global variable
  - `get_global`: Get a Python global variable
  - `install_packages`: Install packages via micropip/loadPackage
  - `run_js`: Execute JavaScript (bonus feature)
  - `shutdown`: Clean shutdown

### Python Wrapper (pyodidebox/)

Created both sync and async versions:
- `PyodideBox` - Synchronous context manager
- `AsyncPyodideBox` - Asynchronous context manager

Key configuration options:
- `allow_net`: Network access (required to download Pyodide)
- `allow_read`: File read access (required for Pyodide to read cached WASM)
- `ignore_cert_errors`: Bypass TLS certificate validation (for testing)

## Challenges Encountered

### 1. Certificate Validation

The test environment has TLS certificate issues. Fixed by adding `ignore_cert_errors` option that passes `--unsafely-ignore-certificate-errors` to Deno.

### 2. File Read Permissions

Pyodide caches its WASM files and needs to read them. Added `--allow-read` permission to Deno.

### 3. Proxy Object Conversion

Pyodide returns proxy objects for Python containers. The worker converts these using `toJs()` with `dict_converter: Object.fromEntries` for proper JSON serialization.

## Test Results

All tests passing:
- Basic math operations
- String operations
- Function definition and calling
- List/dict operations
- Setting/getting global variables
- Standard library (math, json)
- Async version works correctly

## Files Created

- `pyodide_worker.js` - Deno worker script
- `pyodidebox/__init__.py` - Package init
- `pyodidebox/sync_box.py` - Synchronous implementation
- `pyodidebox/async_box.py` - Asynchronous implementation
