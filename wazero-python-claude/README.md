# Wazero Python Bindings

Python bindings for [wazero](https://wazero.io/), a zero-dependency WebAssembly runtime written in Go.

## Overview

This project provides Python bindings to the wazero WebAssembly runtime, allowing you to run WebAssembly modules directly from Python. Wazero is a WebAssembly Core Specification 1.0 and 2.0 compliant runtime that requires no external dependencies.

## Features

- **Zero dependencies** (aside from the Go runtime embedded in the library)
- **Pure Python API** - clean, Pythonic interface
- **Fast execution** - leverages wazero's compiler mode for near-native performance
- **Easy to use** - instantiate WASM modules and call functions with just a few lines of code
- **Context manager support** - automatic resource cleanup
- **Well-tested** - comprehensive test suite with 28+ tests

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/yourusername/wazero-python
cd wazero-python

# Build and install
python -m pip install .
```

### Building a Wheel

```bash
# Install build dependencies
pip install build

# Build the wheel
python -m build -w

# Install the wheel
pip install dist/wazero_python-0.1.0-py3-none-any.whl
```

## Prerequisites

- Python 3.8 or later
- Go 1.18 or later (for building from source)

## Quick Start

```python
import wazero

# Create a runtime
runtime = wazero.Runtime()

# Load a WASM module
module = runtime.instantiate_file("my_module.wasm")

# Call an exported function
result = module.call("add", 5, 7)
print(f"Result: {result[0]}")  # Output: Result: 12

# Clean up
module.close()
runtime.close()
```

### Using Context Managers

```python
import wazero

with wazero.Runtime() as runtime:
    with runtime.instantiate_file("my_module.wasm") as module:
        result = module.call("add", 10, 20)
        print(f"10 + 20 = {result[0]}")
```

### Loading WASM from Bytes

```python
import wazero

# WASM binary as bytes
wasm_bytes = open("module.wasm", "rb").read()

with wazero.Runtime() as runtime:
    module = runtime.instantiate(wasm_bytes)
    result = module.call("my_function", 42)
    print(result)
    module.close()
```

## API Reference

### `wazero.Runtime()`

Creates a new WebAssembly runtime.

**Methods:**
- `instantiate(wasm_bytes: bytes) -> Module` - Instantiate a module from bytes
- `instantiate_file(file_path: str) -> Module` - Instantiate a module from a file
- `close()` - Close the runtime and free resources

### `wazero.Module`

Represents an instantiated WebAssembly module.

**Methods:**
- `call(func_name: str, *args: int) -> List[int]` - Call an exported function
- `close()` - Close the module and free resources

### `wazero.WazeroError`

Exception raised for wazero-related errors.

### `wazero.version() -> str`

Returns the version of the wazero bindings.

## Examples

See the `examples/` directory for more examples (if available).

## Development

### Running Tests

```bash
# Install development dependencies
pip install pytest

# Run tests
pytest tests/ -v
```

### Building the Go Library

```bash
go build -buildmode=c-shared -o libwazero.so libwazero.go
```

## Architecture

This binding uses CGO to create a C-compatible shared library from the Go wazero package. The Python code uses `ctypes` to interface with this library, providing a clean Pythonic API.

**Components:**
1. **libwazero.go** - Go code that exports wazero functions via CGO
2. **wazero/runtime.py** - Python wrapper using ctypes
3. **libwazero.so** - Compiled shared library (Linux) or .dylib (macOS) or .dll (Windows)

## Performance

The binding is designed to minimize overhead between Python and the WASM runtime. In benchmarks:

- **1000 function calls** complete in milliseconds
- Minimal overhead for argument/result marshaling
- Near-native performance for compute-intensive WASM code

## Limitations

- Currently supports only integer (i32/i64) arguments and return values
- No support for WASI filesystem operations yet
- Memory access is not directly exposed to Python (though WASM functions can use memory internally)

## Contributing

Contributions are welcome! Please feel free to submit pull requests or open issues.

## License

Apache 2.0 License - see LICENSE file for details.

## Credits

- [wazero](https://github.com/tetratelabs/wazero) - The excellent Go WebAssembly runtime
- Built with inspiration from other language bindings for WASM runtimes

## See Also

- [wazero documentation](https://wazero.io/)
- [WebAssembly specification](https://webassembly.github.io/spec/)
- [WASI](https://wasi.dev/) - WebAssembly System Interface
