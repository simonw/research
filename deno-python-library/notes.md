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
- Load: `{"id": 1, "type": "load_wasm", "path": "/path/to/module.wasm"}`
- Call: `{"id": 1, "type": "call_wasm", "module": "mod_id", "func": "add", "args": [1, 2]}`

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
