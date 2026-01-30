# Python Binding for the Eryx Rust Library

This project builds Python bindings for [eryx](https://github.com/eryx-org/eryx), a Rust
library that provides a Python sandbox powered by WebAssembly. The sandbox runs CPython 3.14
inside wasmtime's WASM engine, providing complete isolation from the host system.

## What Eryx Provides

- **Sandbox execution** — run untrusted Python code in complete isolation
- **Session persistence** — maintain Python state across multiple executions (REPL-style)
- **State snapshots** — capture and restore Python state via pickle serialization
- **Resource limits** — timeouts, memory limits, callback invocation limits
- **Network control** — TLS networking with host-controlled security policy
- **Virtual filesystem** — in-memory sandboxed filesystem
- **Package loading** — load .whl/.tar.gz packages including native extensions

## Python API

The built wheel exposes an `eryx` Python package with:

| Class | Description |
|---|---|
| `Sandbox` | Execute isolated Python code (~1-5ms creation) |
| `Session` | Persistent state across executions |
| `SandboxFactory` | Pre-initialized factory with custom packages |
| `ExecuteResult` | Execution output (stdout, stderr, stats) |
| `ResourceLimits` | Timeout, memory, callback limits |
| `NetConfig` | Network security policy |
| `VfsStorage` | In-memory virtual filesystem |

Exceptions: `EryxError`, `ExecutionError`, `InitializationError`, `ResourceLimitError`, `TimeoutError`

### Quick Example

```python
import eryx

# Basic sandbox
sandbox = eryx.Sandbox()
result = sandbox.execute('print("Hello from sandbox!")')
print(result.stdout)  # "Hello from sandbox!"

# Session with persistent state
session = eryx.Session()
session.execute('x = 42')
result = session.execute('print(x)')
print(result.stdout)  # "42"

# Resource limits
limits = eryx.ResourceLimits(execution_timeout_ms=5000)
sandbox = eryx.Sandbox(resource_limits=limits)
```

## Building

### Prerequisites

- Rust stable toolchain (1.90+) — https://rustup.rs/
- Rust nightly toolchain with `wasm32-wasip1` target (installed automatically by build.sh)
- Python 3.12+
- uv — https://docs.astral.sh/uv/

### Build the Wheel

```bash
./build.sh
```

The build script handles everything:

1. **Clones** the eryx repo to `/tmp/eryx` (if not already present)
2. **Installs** prerequisites (WASI SDK, Rust nightly, maturin, patchelf)
3. **Builds** the WASM runtime (`runtime.wasm`) by compiling the Rust WASM runtime with WASI SDK
4. **Pre-compiles** to native code (`runtime.cwasm`) using wasmtime
5. **Builds** the Python wheel with maturin (PyO3 bindings + embedded runtime)

The resulting wheel is a ~37MB manylinux wheel for Python 3.12+ (abi3 stable ABI).

### Test the Wheel

```bash
# Quick smoke test
uv run --python 3.12 --with /tmp/eryx/target/wheels/pyeryx-*.whl \
  python -c 'import eryx; sb = eryx.Sandbox(); print(sb.execute("print(2+2)").stdout)'

# Run the full test suite (72 tests)
uv run --python 3.12 \
  --with /tmp/eryx/target/wheels/pyeryx-*.whl \
  --with pytest \
  pytest tests/ -v
```

## Project Structure

```
python-rust-binding/
├── build.sh              # Build script (clones repo, builds wheel)
├── pyproject.toml        # Test project configuration (uv/pytest)
├── tests/
│   └── test_eryx.py      # 72 pytest tests covering the full API
├── notes.md              # Investigation notes
└── README.md             # This file
```

## Architecture

The Python bindings use [PyO3](https://pyo3.rs/) to wrap the Rust `eryx` crate.
The build uses [maturin](https://github.com/PyO3/maturin) as the build backend.

```
┌──────────────────┐
│  Python (eryx)   │  __init__.py re-exports from _eryx
├──────────────────┤
│  PyO3 (_eryx)    │  Rust → Python FFI layer (eryx-python crate)
├──────────────────┤
│  eryx (Rust)     │  Core sandbox library
├──────────────────┤
│  wasmtime        │  WASM execution engine
├──────────────────┤
│  CPython 3.14    │  WASI-compiled Python runtime (embedded in binary)
└──────────────────┘
```

The `embedded` feature bundles the pre-compiled WASM runtime and Python stdlib
directly into the shared library, making the package self-contained (~37MB wheel)
but requiring no external runtime files.

## Build Pipeline

```
WASI SDK + Rust nightly
        │
        ▼
eryx-wasm-runtime (Rust → wasm32-wasip1 staticlib)
        │
        ▼
libpython3.14.so + libc.so + etc. (pre-built WASI libraries)
        │
        ▼
runtime.wasm (linked WASM component, ~32MB)
        │
        ▼
runtime.cwasm (wasmtime pre-compiled, ~33MB)
        │
        ▼
eryx crate (embedded feature includes runtime.cwasm)
        │
        ▼
eryx-python crate (PyO3 bindings)
        │
        ▼
pyeryx-*.whl (maturin wheel, ~37MB)
```

## Test Coverage

The test suite (`tests/test_eryx.py`) covers 72 tests across:

- **Module imports** (7 tests) — version, public classes, exception hierarchy
- **Sandbox** (17 tests) — creation, execution, stdout/stderr, isolation, errors, repr
- **ResourceLimits** (7 tests) — defaults, custom, unlimited, setters, timeout behavior
- **NetConfig** (9 tests) — defaults, allowed/blocked hosts, permissive, chaining
- **Session** (14 tests) — state persistence, functions, reset, snapshots, timeouts
- **VfsStorage** (6 tests) — creation, session mount, file read/write
- **Integration** (12 tests) — multiple sandboxes, edge cases, complex workflows
