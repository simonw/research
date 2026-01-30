# Python Binding for Eryx Rust Library — Notes

## Investigation

### What is Eryx?

Eryx is a Rust library that provides a Python sandbox powered by WebAssembly. It runs
CPython 3.14 (WASI-compiled) inside wasmtime's WASM engine, providing complete isolation
from the host system.

Key capabilities:
- Sandbox Python execution in WASM
- Async callbacks (Python code calling Rust functions)
- Session state persistence across executions
- State snapshots via pickle serialization
- Execution tracing (line-level via sys.settrace)
- Resource limits (timeout, memory, callback limits)
- TLS networking with host-controlled security
- Package loading (.whl, .tar.gz, native extensions)
- Pre-compiled runtime for fast sandbox creation (~16ms vs 650ms)
- Virtual filesystem (in-memory, sandboxed)

### Existing Python Bindings

The repo already has PyO3 Python bindings in `crates/eryx-python/`:

- `Sandbox` — Main sandbox class, creates via embedded runtime
- `Session` — Persistent state across executions (REPL-style)
- `SandboxFactory` — Pre-initialized factory with packages
- `ExecuteResult` — Execution output with stdout, stderr, stats
- `ResourceLimits` — Timeout, memory, callback limits
- `NetConfig` — Network security policy
- `VfsStorage` — In-memory virtual filesystem
- Exception hierarchy: `EryxError`, `ExecutionError`, `InitializationError`,
  `ResourceLimitError`, `TimeoutError`, `CancelledError`

The bindings use PyO3 with `abi3-py312` for Python 3.12+ stable ABI compatibility.
Maturin is the build backend. The module name is `eryx._eryx` with a Python wrapper
at `python/eryx/__init__.py`.

### Build Approach

The existing `crates/eryx-python/pyproject.toml` configures maturin with:
- `features = ["pyo3/extension-module"]`
- `python-source = "python"` (wrapper code in `python/eryx/`)
- `module-name = "eryx._eryx"`

The Cargo.toml uses `eryx` with features `["embedded", "native-extensions", "vfs"]`.
The `embedded` feature bundles the pre-compiled WASM runtime and Python stdlib,
adding ~32MB to the binary but making it self-contained.

### What I Built

1. A `pyproject.toml` for the test/CI infrastructure using uv
2. A comprehensive pytest test suite (`tests/test_eryx.py`)
3. A build shell script (`build.sh`) that:
   - Clones the eryx repo to /tmp if needed
   - Builds the wheel using `maturin build` from the eryx-python crate
   - Outputs the wheel path
4. Verification using `uv run --with <wheel> python -c ...`

### Challenges and Observations

- The eryx-python crate depends on the `embedded` feature which bundles a large
  pre-compiled WASM runtime. This makes the build take significant time and
  produces a large wheel (~37 MB).
- The build requires the full eryx workspace because eryx-python depends on sibling
  crates (eryx, eryx-runtime, eryx-vfs).
- PyO3 with abi3-py312 means the wheel works on Python 3.12+ without needing to
  rebuild for each Python minor version.
- The maturin build must run from the workspace root (or the crate directory with
  the workspace manifest accessible) so all path dependencies resolve.
- The `native-extensions` feature (which eryx-python enables) triggers the preinit
  code path in eryx-runtime's build script, which requires WASI SDK even when not
  doing a full WASM rebuild. Setting WASI_SDK_PATH during `maturin build` is needed.
- The sandbox strips trailing newlines from stdout — `print("hello")` gives
  `stdout == "hello"` not `"hello\n"`. Tests must account for this.
- `patchelf` is required on Linux for maturin to produce manylinux-compatible wheels.
- The full build pipeline from scratch (first run): clone → WASI SDK download →
  Rust nightly install → build WASM runtime → precompile → maturin wheel build.
  Subsequent runs reuse cached artifacts for all steps.

### Build Times (approximate, single core)

- WASI SDK download + extract: ~40s
- WASM runtime build (`BUILD_ERYX_RUNTIME=1 cargo build -p eryx-runtime`): ~50s
- WASM precompile (`cargo run --example precompile`): ~2m30s (includes compile)
- Maturin wheel build: ~3m (includes compiling eryx + PyO3 bindings)
- Total first-run: ~8-10 minutes
- Subsequent runs (cached artifacts): ~30s (just maturin rebuild if Rust source changed)

### Test Results

All 72 tests pass across 7 test classes:
- TestModuleImport: 7 passed
- TestSandbox: 17 passed
- TestResourceLimits: 7 passed
- TestNetConfig: 9 passed
- TestSession: 14 passed
- TestVfsStorage: 6 passed
- TestIntegration: 12 passed
