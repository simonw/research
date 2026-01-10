# denobox

A Python library for executing JavaScript and WebAssembly in a Deno sandbox.

## Overview

`denobox` provides a simple interface to run JavaScript code and WebAssembly modules from Python using Deno as the runtime. It communicates with a Deno subprocess using a newline-delimited JSON (NDJSON) protocol over stdin/stdout.

## Features

- Execute JavaScript code in a **fully sandboxed** Deno environment
- Load and call WebAssembly modules
- Both synchronous and async APIs
- JSON-based data exchange between Python and JavaScript
- Promise resolution handled automatically
- Thread-safe (sync) and concurrent (async) execution support

## Security

Deno runs with **zero permissions** - maximum sandboxing:

- **No file system access** - WASM files are read by Python and sent as base64
- **No network access** - cannot make HTTP requests or open sockets
- **No subprocess spawning** - cannot execute shell commands
- **No environment access** - cannot read environment variables

The sandbox is enforced by Deno's permission system. Any attempt to access restricted resources will raise an error.

## Installation

```bash
pip install denobox
```

Or with uv:

```bash
uv add denobox
```

## Requirements

- Python 3.10+
- The `deno` PyPI package (automatically installed)

## Usage

### JavaScript Execution

#### Synchronous API

```python
from denobox import DenoBox

with DenoBox() as box:
    # Simple expressions
    result = box.eval("1 + 1")
    print(result)  # 2

    # Strings
    result = box.eval("'hello' + ' ' + 'world'")
    print(result)  # "hello world"

    # Arrays and objects
    result = box.eval("[1, 2, 3].map(x => x * 2)")
    print(result)  # [2, 4, 6]

    result = box.eval("({name: 'test', value: 42})")
    print(result)  # {'name': 'test', 'value': 42}

    # State persists between evals
    box.eval("var x = 10")
    box.eval("var y = 20")
    result = box.eval("x + y")
    print(result)  # 30

    # Promises are automatically resolved
    result = box.eval("Promise.resolve(42)")
    print(result)  # 42

    # Async functions work too
    result = box.eval("(async () => { return 'async result'; })()")
    print(result)  # "async result"
```

#### Asynchronous API

```python
import asyncio
from denobox import AsyncDenoBox

async def main():
    async with AsyncDenoBox() as box:
        result = await box.eval("1 + 1")
        print(result)  # 2

        # Concurrent execution
        results = await asyncio.gather(
            box.eval("1 + 1"),
            box.eval("2 + 2"),
            box.eval("3 + 3"),
        )
        print(results)  # [2, 4, 6]

asyncio.run(main())
```

### WebAssembly

#### Synchronous API

```python
from denobox import DenoBox

with DenoBox() as box:
    # Load a WASM module from a file (Python reads and sends to Deno)
    module = box.load_wasm("path/to/module.wasm")

    # Or load from raw bytes
    wasm_bytes = open("path/to/module.wasm", "rb").read()
    module = box.load_wasm(wasm_bytes=wasm_bytes)

    # Check available exports
    print(module.exports)  # {'add': 'function', 'multiply': 'function'}

    # Call exported functions
    result = module.call("add", 3, 4)
    print(result)  # 7

    result = module.call("multiply", 5, 6)
    print(result)  # 30

    # Unload when done (optional, cleaned up on box close)
    module.unload()
```

#### Asynchronous API

```python
import asyncio
from denobox import AsyncDenoBox

async def main():
    async with AsyncDenoBox() as box:
        module = await box.load_wasm("path/to/module.wasm")

        # Concurrent calls
        results = await asyncio.gather(
            module.call("add", 1, 2),
            module.call("add", 3, 4),
            module.call("multiply", 5, 6),
        )
        print(results)  # [3, 7, 30]

        await module.unload()

asyncio.run(main())
```

### Error Handling

```python
from denobox import DenoBox, DenoBoxError

with DenoBox() as box:
    try:
        box.eval("throw new Error('Something went wrong')")
    except DenoBoxError as e:
        print(f"JavaScript error: {e}")

    try:
        box.eval("invalid javascript {{{")
    except DenoBoxError as e:
        print(f"Syntax error: {e}")
```

## Architecture

### NDJSON Protocol

Communication between Python and the Deno subprocess uses newline-delimited JSON:

**Requests:**
```json
{"id": 1, "type": "eval", "code": "1 + 1"}
{"id": 2, "type": "load_wasm", "bytes": "<base64-encoded-wasm>"}
{"id": 3, "type": "call_wasm", "moduleId": "wasm_0", "func": "add", "args": [1, 2]}
{"id": 4, "type": "unload_wasm", "moduleId": "wasm_0"}
{"id": 5, "type": "shutdown"}
```

Note: WASM modules are sent as base64-encoded bytes. Python reads the file and encodes it, so Deno doesn't need file system access.

**Responses:**
```json
{"id": 1, "result": 2}
{"id": 2, "result": {"moduleId": "wasm_0", "exports": {"add": "function"}}}
{"id": 3, "result": 3}
{"id": 4, "result": true}
{"id": 5, "result": true, "shutdown": true}
```

**Errors:**
```json
{"id": 1, "error": "ReferenceError: x is not defined", "stack": "..."}
```

### Components

1. **DenoBox** - Synchronous wrapper using subprocess.Popen with thread-safe locking
2. **AsyncDenoBox** - Asynchronous wrapper using asyncio.create_subprocess_exec with a background reader task
3. **WasmModule / AsyncWasmModule** - Wrappers for loaded WebAssembly modules
4. **worker.js** - Deno script that handles the NDJSON protocol

## Development

```bash
# Clone and setup
git clone <repo>
cd denobox
uv sync --all-extras

# Run tests
uv run pytest

# Run tests with verbose output
uv run pytest -v
```

## License

MIT
