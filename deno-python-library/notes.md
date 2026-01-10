# Deno Python Library Development Notes

## Initial Planning

### Requirements
1. Execute JavaScript strings in a deno sandbox, return results via JSON
2. Load .wasm modules and provide access to their functionality
3. Use newline-delimited JSON (NDJSON) protocol over stdin/stdout
4. Both sync and asyncio variants
5. Use `deno` PyPI package as dependency

### Library Name Ideas
- `denobox` - emphasizes sandboxed execution, clean and memorable
- `denopy` - simple but generic
- `pydenobridge` - descriptive but long

**Decision**: Going with `denobox` - it's short, memorable, and emphasizes the sandbox aspect.

### Architecture Design

#### NDJSON Protocol
Each message is a JSON object on a single line:
- Request: `{"id": 1, "type": "eval", "code": "1 + 1"}`
- Response: `{"id": 1, "result": 2}` or `{"id": 1, "error": "..."}`

For WASM:
- Load: `{"id": 1, "type": "load_wasm", "bytes": "<base64-encoded-wasm>"}`
- Call: `{"id": 1, "type": "call_wasm", "moduleId": "wasm_0", "func": "add", "args": [1, 2]}`

Note: WASM bytes are base64-encoded and sent via JSON. Python reads files and encodes them, so Deno doesn't need file system access.

#### Components
1. `DenoBox` - sync wrapper class
2. `AsyncDenoBox` - async wrapper class
3. `deno_worker.js` - JavaScript worker that handles NDJSON communication

## Development Log

### Step 1: Project Setup
- Created project folder
- Set up uv project with pyproject.toml
- Installed dependencies: deno, pytest, pytest-asyncio

### Step 2: Deno Worker Script
- Created `worker.js` - the JavaScript side of the NDJSON protocol
- Handles `eval`, `load_wasm`, `call_wasm`, `unload_wasm`, and `shutdown` commands
- Tested manually with stdin/stdout

### Step 3: Sync JavaScript Execution (TDD)
- Wrote 13 tests for synchronous JavaScript evaluation
- Tests covered: simple values, arrays, objects, null/undefined, booleans, multiple evals, error handling, promises
- Implemented `DenoBox` class with `eval()` method
- All tests passed

### Step 4: Async JavaScript Execution (TDD)
- Wrote 14 tests for asynchronous JavaScript evaluation
- Added test for concurrent evaluations using `asyncio.gather`
- Implemented `AsyncDenoBox` class with background reader task for response handling
- Used asyncio.Future to correlate requests with responses
- All tests passed

### Step 5: Sync WASM Loading (TDD)
- Created test WASM module (math.wasm) with `add` and `multiply` functions
- Wrote 8 tests for synchronous WASM loading and calling
- Implemented `WasmModule` class with `call()` and `unload()` methods
- Added `load_wasm()` method to `DenoBox`
- All tests passed

### Step 6: Async WASM Loading (TDD)
- Wrote 9 tests for asynchronous WASM loading
- Added test for concurrent WASM calls
- Implemented `AsyncWasmModule` class
- Added `load_wasm()` method to `AsyncDenoBox`
- All tests passed

### Final Test Run
- 44 tests total
- All passing

### Step 7: Sandbox Lockdown
- Removed `--allow-all` from Deno subprocess command
- Changed WASM loading: Python now reads files and sends base64-encoded bytes to Deno
- This means Deno runs with NO permissions:
  - No file system access (`--allow-read`, `--allow-write`)
  - No network access (`--allow-net`)
  - No subprocess spawning (`--allow-run`)
  - No environment variable access (`--allow-env`)
- Added 10 sandbox tests that verify Deno cannot:
  - Read files (`Deno.readTextFileSync`, `Deno.readTextFile`)
  - Write files (`Deno.writeTextFileSync`, `Deno.writeTextFile`)
  - Make network requests (`fetch`)
  - Spawn subprocesses (`Deno.Command`)
  - Access environment variables (`Deno.env.get`)
- Pure JavaScript computation (Math, strings, arrays, JSON) still works

### Final Test Count
- 54 tests total (44 original + 10 sandbox)
- All passing
