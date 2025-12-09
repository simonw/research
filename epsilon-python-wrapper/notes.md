# Epsilon Python Wrapper - Development Notes

## Initial Exploration (2025-12-09)

### Epsilon Overview
- Repository: https://github.com/ziggy42/epsilon
- Version: 0.0.1
- License: Apache 2.0
- Pure Go WebAssembly runtime with zero dependencies
- Fully supports WebAssembly 2.0 Specification
- Runs on any Go-supported architecture without CGo

### Key Source Files Examined

1. **runtime.go** - Main API
   - `Runtime` struct wraps a VM
   - `NewRuntime()` creates a new runtime
   - `InstantiateModule()` parses and instantiates WASM from io.Reader
   - `InstantiateModuleFromBytes()` convenience method for byte slices
   - `ImportBuilder` for building imports (host functions, memory, tables, globals)

2. **instance.go** - Module instances
   - `ModuleInstance` contains types, function/table/memory/global addresses, exports
   - `Invoke(name, args...)` calls exported functions
   - `GetMemory()`, `GetTable()`, `GetGlobal()`, `GetFunction()` accessors
   - `ExportNames()` lists all exports

3. **vm.go** - Virtual machine implementation
   - `maxCallStackDepth = 1000` - hardcoded limit
   - Supports all WebAssembly 2.0 instructions including SIMD (v128)
   - Interprets bytecode instruction-by-instruction
   - No built-in timeout/fuel mechanism

4. **memory.go** - Linear memory
   - `pageSize = 65536` (64 KiB)
   - `maxPages = 32768` (2 GiB maximum total memory)
   - `Limits` struct with Min/Max fields for memory constraints
   - Operations: Grow, Get, Set, Load/Store variants

5. **types.go** - Type definitions
   - Value types: I32, I64, F32, F64, V128
   - Reference types: FuncRef, ExternRef
   - `FunctionType`, `MemoryType`, `TableType`, `GlobalType`
   - `Limits` struct for min/max constraints

6. **experimental_features.go**
   - `MultipleMemories` flag for WASM 3.0 feature

### Resource Limiting Analysis

#### Memory Limits
- **Built-in**: Memory `Limits.Max` field can restrict maximum memory pages
- When creating a module, memory limits are enforced at:
  - Module level (declared in WASM)
  - Import level (via `ImportBuilder.AddMemory()`)
- Max possible: 32768 pages = 2 GiB

#### CPU/Time Limits
- **No built-in mechanism** for:
  - Execution timeouts
  - Fuel/gas metering
  - Instruction counting

- **Call stack depth**: Hardcoded to 1000 in vm.go line 30
- **Options for limiting CPU time**:
  1. Go context with timeout (at the wrapper level)
  2. OS-level process limits (ulimit)
  3. Custom fuel implementation (would require modifying epsilon source)
  4. Python signal-based timeout (Unix only)

#### Comparison with wazero
- wazero has built-in `RuntimeConfig.WithCloseOnContextCancel()` for timeouts
- wazero has `RuntimeConfig.WithMemoryLimitPages()`
- epsilon lacks these convenience methods - must be implemented at wrapper level

### Python Wrapper Design

Based on wazero-python-claude reference:
1. Create Go shared library using cgo with `//export` directives
2. Python ctypes wrapper to call the shared library
3. Classes: `Runtime`, `Module` with context managers

### API Design for epsilon-python

```python
# Basic usage
from epsilon import Runtime

runtime = Runtime()
module = runtime.instantiate(wasm_bytes)
result = module.call("add", 5, 37)  # Returns [42]
module.close()
runtime.close()

# With context managers
with Runtime() as runtime:
    with runtime.instantiate(wasm_bytes) as module:
        result = module.call("add", 5, 37)

# With memory limits
runtime = Runtime(max_memory_pages=256)  # 16 MiB max

# With timeout (Python-level)
import signal
# ... timeout implementation
```

### Implementation Plan

1. Create `libepsilon.go` with cgo exports:
   - `epsilon_new_runtime(max_memory_pages) -> runtime_id`
   - `epsilon_runtime_close(runtime_id)`
   - `epsilon_instantiate(runtime_id, wasm_bytes, len) -> module_id`
   - `epsilon_module_close(module_id)`
   - `epsilon_call_function(module_id, func_name, args, nargs, results, max_results) -> result_count`
   - `epsilon_get_export_names(module_id) -> names`
   - `epsilon_version() -> string`
   - `epsilon_get_error() -> string`

2. Create Python `epsilon/` package:
   - `__init__.py` - exports
   - `runtime.py` - main wrapper classes
   - Shared library loading

3. Package files:
   - `setup.py` with custom build for Go
   - `pyproject.toml`
   - Tests

## Observations

### Epsilon Strengths
- Pure Go - no cgo dependency for the runtime itself
- WebAssembly 2.0 complete support
- SIMD support (v128 instructions)
- Clean API design

### Epsilon Limitations for Sandboxing
- No built-in timeout mechanism
- No fuel/gas metering
- Fixed call stack depth (1000)
- Memory limits only via WASM module limits
- Would need external mechanisms for full resource control

### Building the Wrapper
Even though epsilon itself is pure Go, our wrapper will use cgo for the shared library.
This is necessary to expose the API to Python via ctypes.

## Implementation Notes (2025-12-09)

### Go Shared Library
Successfully built `libepsilon.so` with the following exports:
- `epsilon_new_runtime` - Create a new runtime
- `epsilon_runtime_close` - Close a runtime
- `epsilon_instantiate` - Instantiate a module from WASM bytes
- `epsilon_instantiate_with_memory_limit` - Instantiate with memory constraints
- `epsilon_module_close` - Close a module
- `epsilon_call_function` - Call an exported function (i32 args)
- `epsilon_call_function_i64` - Call with typed arguments
- `epsilon_call_function_with_timeout` - Call with Go context timeout
- `epsilon_get_export_names` - List module exports
- `epsilon_get_memory_size` - Get memory size in pages
- `epsilon_read_memory` - Read from linear memory
- `epsilon_write_memory` - Write to linear memory
- `epsilon_get_global` - Get global variable value
- `epsilon_version` - Get epsilon version
- `epsilon_wrapper_version` - Get wrapper version
- `epsilon_get_error` - Get last error message

### Build Requirements
- Epsilon requires Go 1.25.1, but can be built with Go 1.24 after modifying go.mod
- The shared library is about 3.6MB

### Testing
Successfully tested:
- Basic function calls (add example)
- Export name listing
- Timeout functionality

### Resource Limiting Implementation

1. **Memory Limits**:
   - Implemented via `epsilon_instantiate_with_memory_limit()`
   - Creates a pre-configured memory import with max pages limit
   - Works for modules that import memory as "env.memory"

2. **Timeout (Partial)**:
   - Implemented via `epsilon_call_function_with_timeout()`
   - Uses Go context with timeout
   - **Important limitation**: Cannot interrupt WASM execution mid-instruction
   - Timeout only applies if the function returns; infinite loops won't be interrupted

3. **Alternative Approaches Considered**:
   - Signal-based timeout in Python (Unix only, race conditions)
   - multiprocessing with timeout (heavy overhead)
   - Modifying epsilon source for fuel metering (requires upstream changes)
