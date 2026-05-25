# PyodideBox

A Python library for running Python code inside Pyodide (Python compiled to WebAssembly) within a Deno sandbox. Inspired by [denobox](https://pypi.org/project/denobox/).

## Overview

PyodideBox allows you to execute Python code in an isolated environment by:
1. Starting a Deno subprocess
2. Loading Pyodide (Python compiled to WASM) in that subprocess
3. Communicating via JSON over stdin/stdout

This provides a sandboxed Python runtime that runs in WebAssembly, separate from your main Python process.

## Installation

The library requires `deno` to be available. Install via pip alongside denobox (which provides the deno binary):

```bash
pip install denobox
```

Then copy the `pyodidebox/` directory and `pyodide_worker.js` to your project.

## Usage

### Synchronous API

```python
from pyodidebox import PyodideBox

with PyodideBox() as box:
    # Initialize Pyodide (downloads on first run)
    info = box.init()
    print(f"Pyodide version: {info['version']}")

    # Run Python code
    result = box.run("1 + 1")
    print(result)  # 2

    # Define and use functions
    box.run('''
def greet(name):
    return f"Hello, {name}!"
''')
    result = box.run('greet("World")')
    print(result)  # "Hello, World!"

    # Set and get global variables
    box.set_global('data', [1, 2, 3, 4, 5])
    result = box.run('sum(data)')
    print(result)  # 15

    # Use standard library
    result = box.run('''
import math
math.sqrt(144)
''')
    print(result)  # 12.0
```

### Asynchronous API

```python
import asyncio
from pyodidebox import AsyncPyodideBox

async def main():
    async with AsyncPyodideBox() as box:
        await box.init()

        result = await box.run("2 ** 10")
        print(result)  # 1024

        # Define a function
        await box.run('''
def factorial(n):
    if n <= 1:
        return 1
    return n * factorial(n - 1)
''')
        result = await box.run('factorial(10)')
        print(result)  # 3628800

asyncio.run(main())
```

## Configuration Options

Both `PyodideBox` and `AsyncPyodideBox` accept these options:

| Option | Default | Description |
|--------|---------|-------------|
| `allow_net` | `True` | Allow network access (required to download Pyodide) |
| `allow_read` | `True` | Allow file reading (required for Pyodide to read cached WASM) |
| `ignore_cert_errors` | `False` | Bypass TLS certificate validation (for testing environments) |

## API Reference

### PyodideBox / AsyncPyodideBox

#### Methods

- **`init()`** - Initialize Pyodide runtime (called automatically on first `run()` if needed)
- **`run(code: str)`** - Execute Python code and return the result
- **`set_global(name: str, value: Any)`** - Set a global variable in the Python namespace
- **`get_global(name: str)`** - Get a global variable from the Python namespace
- **`install(*packages: str)`** - Install Python packages (uses Pyodide's package system)
- **`run_js(code: str)`** - Execute JavaScript in the Deno runtime

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│ Python (your code)                                      │
│   PyodideBox                                            │
│     └── subprocess.Popen(["deno", "run", ...])          │
├─────────────────────────────────────────────────────────┤
│ Deno Runtime                                            │
│   pyodide_worker.js                                     │
│     ├── NDJSON stdin/stdout communication               │
│     └── Pyodide (Python WASM)                           │
│           └── Your Python code runs here                │
└─────────────────────────────────────────────────────────┘
```

## How It Works

1. **Startup**: PyodideBox spawns a Deno subprocess running `pyodide_worker.js`
2. **Initialization**: The worker imports Pyodide from npm and loads the Python runtime
3. **Communication**: Python code is sent as JSON over stdin, results come back over stdout
4. **Execution**: Python code runs inside Pyodide (WebAssembly) within Deno's sandbox
5. **Serialization**: Results are converted from Python → JavaScript → JSON → Python

## Comparison with denobox

| Feature | denobox | PyodideBox |
|---------|---------|------------|
| Language | JavaScript | Python (via Pyodide) |
| WASM Support | Direct WebAssembly loading | Pyodide's Python runtime |
| Runtime | Deno | Deno + Pyodide |
| Sandbox | Deno permissions | Deno + WASM isolation |

## Offline / Cached Mode

You can run PyodideBox without network access after caching Pyodide:

### Step 1: Cache Pyodide (requires network once)

```bash
# Using deno from the denobox package
deno run --allow-read --allow-net -e "
import { loadPyodide } from 'npm:pyodide@0.27.5';
const py = await loadPyodide();
console.log('Cached Pyodide version:', py.version);
"
```

### Step 2: Use the cached version

```python
from pyodidebox_cached import PyodideBox

with PyodideBox() as box:
    result = box.run("1 + 1")
    print(result)  # 2
```

The cached version uses `--cached-only` flag with Deno, so it won't attempt any network access.

### Files for Cached Mode

- `pyodide_cached_worker.js` - Worker that uses cached npm:pyodide
- `pyodidebox_cached.py` - Python wrapper for cached mode

## Limitations

- First run downloads Pyodide (~20MB) and caches it (or use cached mode)
- Not all Python packages work in Pyodide (C extensions need to be compiled to WASM)
- Performance overhead from WASM and JSON serialization
- Results must be JSON-serializable

## License

MIT
