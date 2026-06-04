# Epsilon Python Wrapper

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

Python bindings for the [epsilon](https://github.com/ziggy42/epsilon) WebAssembly runtime.

## About Epsilon

**Epsilon** is a pure Go WebAssembly runtime with zero dependencies, created by Google. Key features:

- **Pure Go Implementation**: No cgo dependencies for the core runtime, runs on any Go-supported architecture
- **WebAssembly 2.0 Complete**: Full specification support including SIMD (v128) instructions
- **Zero External Dependencies**: Self-contained implementation
- **Clean API**: Simple, intuitive interface for embedding WASM in Go applications
- **Apache 2.0 Licensed**: Permissive open-source license

### Epsilon Capabilities

| Feature | Support |
|---------|---------|
| WebAssembly 2.0 | ✅ Full |
| SIMD (v128) | ✅ Full |
| Multiple Memories | ✅ Experimental |
| Host Functions | ✅ Full |
| Tables | ✅ Full |
| Globals | ✅ Full |
| Memory Import/Export | ✅ Full |
| Interactive REPL | ✅ Included |

### Architecture

Epsilon uses a bytecode interpreter design:

```
┌─────────────────────────────────────────────────────┐
│                     Runtime                          │
│  ┌─────────────────────────────────────────────────┐ │
│  │                      VM                          │ │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────────┐  │ │
│  │  │  Store   │  │  Stack   │  │  Call Stack  │  │ │
│  │  │ (funcs,  │  │ (values) │  │  (frames)    │  │ │
│  │  │ memories,│  │          │  │              │  │ │
│  │  │ tables,  │  │          │  │              │  │ │
│  │  │ globals) │  │          │  │              │  │ │
│  │  └──────────┘  └──────────┘  └──────────────┘  │ │
│  └─────────────────────────────────────────────────┘ │
│                                                       │
│  ┌─────────────────────────────────────────────────┐ │
│  │              Module Instance                     │ │
│  │  - Function addresses                            │ │
│  │  - Memory addresses                              │ │
│  │  - Table addresses                               │ │
│  │  - Global addresses                              │ │
│  │  - Exports                                       │ │
│  └─────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────┘
```

## Installation

### Prerequisites

- Python 3.8+
- Go 1.24+ (for building)

### Building from Source

```bash
# Clone this repository
git clone <this-repo>
cd epsilon-python-wrapper

# Install in development mode
pip install -e .

# Or build the wheel
pip install build
python -m build
```

## Quick Start

### Basic Usage

```python
from epsilon import Runtime

# Load and run a WebAssembly module
with Runtime() as runtime:
    wasm_bytes = open("add.wasm", "rb").read()
    with runtime.instantiate(wasm_bytes) as module:
        result = module.call("add", 5, 37)
        print(result)  # [42]
```

### Memory Access

```python
with Runtime() as runtime:
    with runtime.instantiate(wasm_bytes) as module:
        # Read memory
        data = module.read_memory(offset=0, length=100, memory_name="memory")

        # Write memory
        module.write_memory(offset=0, data=b"Hello, WASM!", memory_name="memory")

        # Get memory size (in pages, 1 page = 64KB)
        size = module.get_memory_size("memory")
```

### Export Inspection

```python
with Runtime() as runtime:
    with runtime.instantiate(wasm_bytes) as module:
        exports = module.get_export_names()
        print(exports)  # ['add', 'memory', 'global_counter', ...]
```

### Typed Function Calls

```python
with Runtime() as runtime:
    with runtime.instantiate(wasm_bytes) as module:
        # Call with typed arguments
        result = module.call_typed("compute", [
            (42, 'i32'),
            (3.14, 'f64'),
            (1000000, 'i64'),
        ])
```

## Resource Limiting

### Memory Limits

```python
# Limit memory to 256 pages (16 MB)
with Runtime(max_memory_pages=256) as runtime:
    module = runtime.instantiate(wasm_bytes)

# Or per-module
with Runtime() as runtime:
    module = runtime.instantiate(wasm_bytes, max_memory_pages=128)
```

### Execution Timeout

```python
# Set a timeout (in milliseconds)
result = module.call("slow_function", 42, timeout_ms=5000)
```

**Important limitations of the timeout mechanism:**

1. **Non-preemptive**: Epsilon does not support context cancellation. The timeout uses a Go context wrapper, but WASM execution cannot be interrupted mid-instruction.

2. **Works for returning functions**: If a function completes before the timeout, results are returned normally. If it takes longer, a `EpsilonTimeoutError` is raised.

3. **Infinite loops**: An infinite loop in WASM will NOT be interrupted by the timeout. The Go goroutine will continue running indefinitely.

### Call Stack Depth

Epsilon has a hardcoded call stack depth limit of 1000 frames. This protects against stack overflow from deeply nested recursion.

## Resource Limiting Options Analysis

### What Epsilon Provides

| Resource | Built-in Limit | Configurable |
|----------|---------------|--------------|
| Memory (pages) | 32,768 max (2GB) | ✅ Via Limits.Max |
| Call Stack | 1,000 frames | ❌ Hardcoded |
| CPU Time | None | ❌ Not available |
| Fuel/Gas | None | ❌ Not available |
| Instructions | None | ❌ Not available |

### Comparison with Other Runtimes

| Feature | Epsilon | wazero | wasmtime |
|---------|---------|--------|----------|
| Memory Limits | ✅ | ✅ | ✅ |
| Context Cancellation | ❌ | ✅ | ✅ |
| Fuel Metering | ❌ | ❌ | ✅ |
| Instruction Counting | ❌ | ❌ | ✅ |
| Pure Go | ✅ | ✅ | ❌ (Rust) |
| CGo Free | ✅ | ✅ | ❌ |

### Alternative Approaches for CPU Limiting

If you need strict CPU/time limits, consider these approaches:

1. **Process-level limits** (Linux):
   ```python
   import resource
   resource.setrlimit(resource.RLIMIT_CPU, (5, 5))  # 5 second CPU limit
   ```

2. **Subprocess with timeout**:
   ```python
   import subprocess
   result = subprocess.run(
       ["python", "run_wasm.py"],
       timeout=5.0,
       capture_output=True
   )
   ```

3. **Multiprocessing**:
   ```python
   from multiprocessing import Process, Queue

   def run_wasm(q):
       result = module.call("function")
       q.put(result)

   q = Queue()
   p = Process(target=run_wasm, args=(q,))
   p.start()
   p.join(timeout=5.0)
   if p.is_alive():
       p.terminate()
   ```

4. **Signal-based timeout** (Unix only):
   ```python
   import signal

   def timeout_handler(signum, frame):
       raise TimeoutError("Execution timed out")

   signal.signal(signal.SIGALRM, timeout_handler)
   signal.alarm(5)  # 5 second timeout
   try:
       result = module.call("function")
   finally:
       signal.alarm(0)
   ```

## API Reference

### `epsilon.Runtime`

```python
class Runtime:
    PAGE_SIZE = 65536  # 64KB
    MAX_PAGES = 32768  # 2GB total

    def __init__(self, max_memory_pages: int = 0):
        """Create a new WebAssembly runtime.

        Args:
            max_memory_pages: Default max memory for modules (0 = no limit)
        """

    def instantiate(self, wasm_bytes, max_memory_pages=None) -> Module:
        """Instantiate a WASM module from bytes."""

    def instantiate_file(self, file_path, max_memory_pages=None) -> Module:
        """Instantiate a WASM module from a file."""

    def close(self):
        """Close the runtime and free resources."""
```

### `epsilon.Module`

```python
class Module:
    def call(self, func_name: str, *args, timeout_ms=None) -> List[int]:
        """Call an exported function."""

    def call_typed(self, func_name: str, args: List[tuple], timeout_ms=None) -> List[int]:
        """Call with typed arguments: [(value, 'i32'|'i64'|'f32'|'f64'), ...]"""

    def get_export_names(self) -> List[str]:
        """Get names of all exports."""

    def get_memory_size(self, memory_name="memory") -> int:
        """Get memory size in pages."""

    def read_memory(self, offset, length, memory_name="memory") -> bytes:
        """Read bytes from linear memory."""

    def write_memory(self, offset, data, memory_name="memory") -> int:
        """Write bytes to linear memory."""

    def get_global(self, global_name: str) -> int:
        """Get the value of an exported global."""

    def close(self):
        """Close the module and free resources."""
```

### Exceptions

```python
class EpsilonError(Exception):
    """Base exception for epsilon errors."""

class EpsilonTimeoutError(EpsilonError):
    """Raised when execution times out."""
```

### Utility Functions

```python
def version() -> str:
    """Get the epsilon library version."""

def wrapper_version() -> str:
    """Get the Python wrapper version."""
```

## Epsilon Technical Details

### Memory Model

- **Page size**: 64 KiB (65,536 bytes)
- **Maximum pages**: 32,768 (2 GiB total)
- **Limits**: Configurable min/max via `MemoryType.Limits`

### Value Types Supported

| Type | Description | Size |
|------|-------------|------|
| i32 | 32-bit integer | 4 bytes |
| i64 | 64-bit integer | 8 bytes |
| f32 | 32-bit float | 4 bytes |
| f64 | 64-bit float | 8 bytes |
| v128 | 128-bit SIMD vector | 16 bytes |
| funcref | Function reference | pointer |
| externref | External reference | pointer |

### VM Limits

| Limit | Value | Configurable |
|-------|-------|--------------|
| Call stack depth | 1,000 | No |
| Value stack | Unlimited* | No |
| Memory pages | 32,768 | Yes |
| Table size | 2^32-1 | Yes |

*Value stack is implemented as a Go slice and grows as needed

## Known Limitations

1. **No true timeout support**: WASM execution cannot be preemptively interrupted
2. **No fuel metering**: Cannot limit instruction count or CPU cycles
3. **Single-threaded**: Epsilon does not support WebAssembly threads proposal
4. **No WASI**: Epsilon is a pure runtime without WASI support
5. **Fixed call depth**: 1000 frame limit is hardcoded

## License

Apache 2.0 - see [LICENSE](LICENSE)

## Acknowledgments

- [Epsilon](https://github.com/ziggy42/epsilon) by Google
- Inspired by [wazero-python](https://github.com/user/wazero-python) wrapper design
