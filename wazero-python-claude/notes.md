# Wazero Python Binding - Research Notes

## Project Goal
Create a Python binding for wazero, a zero-dependency Go library for running WebAssembly.

## Investigation Started
Date: 2025-11-02

## What is Wazero?

Wazero is a WebAssembly Core Specification 1.0 and 2.0 compliant runtime written in pure Go with zero dependencies. Key features:

- **Zero Dependencies**: No CGO, no shared libraries, preserves cross-compilation
- **Go-Native API**: Safe concurrency, context propagation
- **Production Ready**: v1.0 released March 2023
- **Two modes**: Compiler (AOT, default) and Interpreter (portable)

### Installation
```bash
go get github.com/tetratelabs/wazero@latest
```

### Basic Go API Usage
```go
import "github.com/tetratelabs/wazero"

ctx := context.Background()
r := wazero.NewRuntime(ctx)
defer r.Close(ctx)

mod, err := r.Instantiate(ctx, wasmBinary)
// Call exported functions
result, err := mod.ExportedFunction("name").Call(ctx, args...)
```

### Key Types
- `Runtime`: Main execution environment
- `CompiledModule`: Pre-compiled WASM for reuse
- `Module`: Instantiated module with callable functions
- `RuntimeConfig`, `ModuleConfig`, `FSConfig`: Configuration options

## Hello World Example

Created `hello.go` which demonstrates:
1. Creating a WASM binary in memory (simple "add" function)
2. Creating a wazero Runtime
3. Instantiating the module
4. Calling an exported function (add)
5. Getting results back

Result: Successfully ran `5 + 7 = 12` using wazero v1.9.0

The WASM binary is created manually using byte array for simplicity. In production, WASM files would typically be compiled from C, Rust, Go (with TinyGo), or other languages.

## Advanced Demo (demo.go)

Created a more comprehensive demo showing:
1. **Multiple function calls**: Executed 1000+ WASM function calls
2. **Module compilation**: Pre-compiled WASM for reuse with `CompileModule()`
3. **Multiple instances**: Created 3 separate instances from same compiled module
4. **File I/O**: Saved WASM binary to file and loaded it back
5. **Performance**: Demonstrated that wazero can handle many calls efficiently

Key findings:
- Very fast execution (1000 calls in milliseconds)
- Module can be compiled once and instantiated multiple times
- Each instance maintains separate state
- WASM modules are portable binary files (only 41 bytes for simple add function!)

## Python Binding Architecture Design

### Approach: CGO-based binding

Since wazero is a Go library and we need Python bindings, we'll use CGO to create a C-compatible library that Python can call via ctypes or cffi.

**Architecture:**
1. **Go layer**: Export wazero functions via CGO with C-compatible API
2. **Python layer**: Use ctypes to call the Go shared library
3. **Build system**: Use setuptools with custom build to compile Go code

**Key challenges:**
- CGO will be needed (ironic since wazero itself avoids CGO!)
- Memory management between Go and Python
- Context handling (Go's context.Context doesn't map directly to Python)
- Error handling across language boundary

**Alternative considered:** gRPC server (rejected - too heavy)
**Alternative considered:** subprocess (rejected - too slow)

### C API Design

Core functions to export:
```c
// Runtime management
void* wazero_new_runtime();
void wazero_runtime_close(void* runtime);

// Module operations
void* wazero_instantiate(void* runtime, uint8_t* wasm_bytes, int len, char** error);
void wazero_module_close(void* module);

// Function calls
int wazero_call_function(void* module, char* name, uint64_t* args, int nargs, uint64_t* results, int nresults, char** error);
```

## Implementation Complete!

### Go CGO Library (libwazero.go)
Successfully created a C-shared library that exports wazero functionality:
- Built with: `go build -buildmode=c-shared -o libwazero.so libwazero.go`
- Size: ~7MB (includes full wazero runtime)
- Exports: Runtime creation, module instantiation, function calling

### Python Bindings (wazero/ package)
Created a clean Python API using ctypes:
- `wazero.Runtime()`: Create a WASM runtime
- `runtime.instantiate(bytes)`: Load WASM from bytes
- `runtime.instantiate_file(path)`: Load WASM from file
- `module.call(name, *args)`: Call exported functions
- Context managers supported (with statement)

### Test Results
Initial testing with `test_binding.py`:
```
✓ add(5, 7) = 12
✓ add(100, 200) = 300
✓ add(42, 0) = 42
✓ add(1000, 2000) = 3000
```

All function calls work correctly! The binding successfully:
1. Creates a runtime
2. Loads WASM modules
3. Calls functions with arguments
4. Returns results
5. Properly manages resources

## Testing Complete

Created comprehensive pytest test suite:
- **28 tests** covering all functionality
- Tests for Runtime, Module, error handling
- Integration tests for complex scenarios
- Performance test (1000 function calls)

**All tests pass!** ✓

## Packaging Complete

Successfully built Python wheel:
- Created `pyproject.toml` with all metadata
- Custom `setup.py` that compiles Go library during build
- `MANIFEST.in` for including all necessary files
- Built wheel: `wazero_python-0.1.0-cp311-cp311-linux_x86_64.whl` (3.5 MB)

Command used: `python -m build -w`

The wheel includes:
- Python package (`wazero/__init__.py`, `wazero/runtime.py`)
- Compiled Go library (`libwazero.so`)
- LICENSE and README.md

## Project Structure

```
wazero-python-claude/
├── wazero/                    # Python package
│   ├── __init__.py
│   ├── runtime.py
│   └── libwazero.so
├── tests/                     # Test suite
│   ├── __init__.py
│   ├── conftest.py
│   ├── test_basic.py
│   ├── test_runtime.py
│   └── test_module.py
├── libwazero.go               # Go CGO library source
├── hello.go                   # Simple demo
├── demo.go                    # Advanced demo
├── test_binding.py            # Quick test script
├── pyproject.toml             # Python package metadata
├── setup.py                   # Custom build script
├── MANIFEST.in                # Package file list
├── README.md                  # Documentation
├── LICENSE                    # Apache 2.0
├── notes.md                   # This file!
├── go.mod                     # Go dependencies
├── go.sum
└── add.wasm                   # Test WASM file

dist/
└── wazero_python-0.1.0-cp311-cp311-linux_x86_64.whl  # Built wheel
```

