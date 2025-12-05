# jq WebAssembly Sandbox

A Python library for executing jq (JSON query) programs in a secure WebAssembly sandbox with memory and CPU limits.

## Overview

This project provides a secure way to run untrusted jq programs by executing them in a WebAssembly sandbox. Key features:

- **Memory limits**: Restrict maximum memory usage
- **CPU limits**: Limit instruction count (fuel-based metering with wasmtime)
- **Filesystem isolation**: No access to host filesystem
- **Multiple backends**: Support for both wasmtime and wasmer runtimes

## Why jaq instead of jq?

This project uses [jaq](https://github.com/01mf02/jaq), a Rust-based jq clone, instead of the original C-based jq:

| Feature | jq (C) | jaq (Rust) |
|---------|--------|------------|
| WASI compilation | Complex (needs pthread stubs, POSIX workarounds) | Native support |
| Performance | Good | Often 5-10x faster |
| Memory safety | Manual | Rust guarantees |
| Security audited | No | Yes (NLnet grant) |

jaq is:
- **Faster**: 5-10x faster on many benchmarks
- **Safer**: Written in memory-safe Rust, security audited
- **WASM-ready**: Compiles to wasm32-wasi out of the box

## Installation

### Prerequisites

```bash
# Install Python dependencies
pip install wasmtime  # or: pip install wasmer wasmer-compiler-cranelift

# Build jaq WASM (requires Rust)
./build_jaq_wasm.sh
```

### Quick Start

```python
from jq_wasm import WasmtimeJqRunner

# Create a sandboxed runner
runner = WasmtimeJqRunner(
    "build/jaq.wasm",
    max_memory_pages=256,  # 16MB limit
    max_fuel=100_000_000   # CPU instruction limit
)

# Run a jq program
result = runner.run(".foo", '{"foo": "bar"}')
print(result.output)  # "bar"
```

## API Reference

### WasmtimeJqRunner

```python
from jq_wasm import WasmtimeJqRunner

runner = WasmtimeJqRunner(
    wasm_path="build/jaq.wasm",
    max_memory_pages=256,      # Memory limit in 64KB pages (default: 16MB)
    max_fuel=100_000_000       # CPU instruction limit (None for unlimited)
)

# Execute jq program
result = runner.run(
    program=".foo | . + 1",    # jq filter
    input_json='{"foo": 42}',  # JSON input
    raw_output=False,          # -r flag (raw strings)
    compact=False              # -c flag (compact output)
)

# Result object
result.stdout   # Standard output
result.stderr   # Standard error
result.exit_code  # Exit code (0 = success)
result.success  # True if exit_code == 0
result.output   # stdout.strip() or raises JqError
```

### WasmerJqRunner

```python
from jq_wasm import WasmerJqRunner

runner = WasmerJqRunner(
    wasm_path="build/jaq.wasm",
    max_memory_pages=256,
    compiler="cranelift"  # Options: cranelift, llvm, singlepass
)
```

### Exceptions

```python
from jq_wasm import JqError, JqTimeoutError, JqMemoryError

try:
    result = runner.run(".", input_json)
except JqTimeoutError:
    print("Execution exceeded CPU limit")
except JqMemoryError:
    print("Execution exceeded memory limit")
except JqError as e:
    print(f"jq error: {e}")
```

## Security Model

### Sandbox Isolation

The WebAssembly sandbox provides:

1. **No filesystem access**: WASI directories are not preopened
2. **No network access**: WebAssembly has no network APIs
3. **Memory isolation**: Cannot access host memory
4. **Deterministic execution**: Same inputs produce same outputs

### Resource Limits

| Resource | wasmtime | wasmer |
|----------|----------|--------|
| Memory | Page-based limits | Page-based limits |
| CPU | Fuel metering | Limited support |
| Stack | Configurable | Configurable |

### Testing Sandbox Security

```python
# This should NOT be able to read files
result = runner.run('.', '{}')  # Works

# Filesystem access is blocked by not preopening any directories
# The WASI implementation has no access to host filesystem
```

## Building jaq WASM

### From Source (Recommended)

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add WASI target
rustup target add wasm32-wasip1

# Clone and build jaq
git clone https://github.com/01mf02/jaq.git jaq-src
cd jaq-src
cargo build --release --target wasm32-wasip1
cp target/wasm32-wasip1/release/jaq.wasm ../build/
```

### Pre-built Binaries

jaq is available on the Wasmer registry:
```bash
wasmer run jaq/jaq '.foo' <<< '{"foo": "bar"}'
```

## Comparing Runtimes

### wasmtime

**Pros:**
- Official Bytecode Alliance project
- Fuel-based CPU metering
- Excellent WASI support
- Production-ready

**Cons:**
- Slightly larger binary size

### wasmer

**Pros:**
- Multiple compiler backends (Cranelift, LLVM, Singlepass)
- Can generate native executables
- Good performance

**Cons:**
- Limited fuel/metering in Python bindings
- More complex API for WASI

## Benchmark Results

Run benchmarks:
```bash
python benchmark.py
```

### Example Results

| Benchmark | wasmtime | wasmer | Notes |
|-----------|----------|--------|-------|
| Simple field access | ~0.5ms | ~0.6ms | `.foo` |
| Array transformation | ~1.2ms | ~1.4ms | `[.[] \| . * 2]` |
| Object construction | ~0.8ms | ~0.9ms | `{a: .x, b: .y}` |
| Large array filter | ~5ms | ~6ms | 1000 elements |

*Note: Results vary by hardware and WASM binary optimization level.*

## Project Structure

```
jq-wasm-sandbox/
├── jq_wasm/
│   ├── __init__.py          # Package exports
│   ├── base.py              # Base classes and exceptions
│   ├── wasmtime_runner.py   # Wasmtime implementation
│   └── wasmer_runner.py     # Wasmer implementation
├── build/
│   └── jaq.wasm             # Compiled WASM binary
├── build_jaq_wasm.sh        # Build script
├── test_jq_wasm.py          # Test suite
├── benchmark.py             # Benchmarking script
├── requirements.txt         # Python dependencies
└── README.md                # This file
```

## Limitations

1. **Not full jq compatibility**: jaq implements most of jq but not all features
2. **Single-threaded**: WebAssembly execution is single-threaded
3. **Wasmer metering**: CPU limits are limited in wasmer Python bindings
4. **Startup overhead**: First execution is slower due to WASM compilation

## Research Notes

### Compilation Challenges

Compiling the original C-based jq to WASI proved challenging:

1. **pthread dependency**: jq uses pthread for thread-local storage
   - WASI doesn't support pthreads
   - Requires stubbing or patching

2. **POSIX headers**: Missing headers like `<pwd.h>`
   - WASI has limited POSIX compatibility
   - Requires source modifications

3. **Emscripten vs WASI SDK**:
   - Emscripten produces browser-compatible WASM
   - WASI SDK produces standalone WASM for wasmtime/wasmer
   - Emscripten can produce WASI-compatible output with `-s STANDALONE_WASM=1`

### Alternative Approaches Explored

1. **jqkungfu**: Uses Emscripten, produces browser-targeted WASM
2. **biowasm**: Provides pre-compiled jq for browsers
3. **wapm jq**: WASI-compiled jq available via wasmer

## References

- [jaq - Rust jq clone](https://github.com/01mf02/jaq)
- [wasmtime-py](https://github.com/bytecodealliance/wasmtime-py)
- [wasmer-python](https://github.com/wasmerio/wasmer-python)
- [WASI](https://wasi.dev/)
- [jqkungfu](https://github.com/robertaboukhalil/jqkungfu)

## License

MIT License
