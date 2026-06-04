# WASM REPL CLI Tools

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

Go CLI tools for running Python and JavaScript REPLs via WebAssembly, using the [wazero](https://github.com/tetratelabs/wazero) runtime.

## Overview

This project provides two separate Go binaries:

1. **jsrunner** - JavaScript REPL powered by [QuickJS](https://github.com/quickjs-ng/quickjs) WASI build
2. **pyrunner** - Python REPL powered by [CPython WASI](https://github.com/brettcannon/cpython-wasi-build)

Both tools support:
- Direct code evaluation via command-line flags (`-e` for JS, `-c` for Python)
- Interactive REPL mode
- **JSONL mode** (`--jsonl`) for programmatic code execution with persistent state

## JSONL Mode

The JSONL mode is designed for integrating these language runtimes into other applications. It:
- Reads JSON requests from stdin (one per line)
- Executes the code with **persistent state** across requests
- Returns JSON responses to stdout

### Request Format
```json
{"id": "unique-request-id", "code": "console.log(1+1)"}
```

### Response Format
```json
{"id": "unique-request-id", "output": "2\n"}
```

Or on error:
```json
{"id": "unique-request-id", "error": "ReferenceError: x is not defined"}
```

## Setup

### Download Runtime Files

The WASM binaries are not included in the repository due to their size. Download them first:

```bash
# Download QuickJS WASM (required for jsrunner)
mkdir -p cmd/jsrunner
curl -L -o cmd/jsrunner/qjs.wasm \
  https://github.com/quickjs-ng/quickjs/releases/download/v0.11.0/qjs-wasi.wasm

# Download CPython WASI (required for pyrunner)
mkdir -p runtime
curl -L -o runtime/cpython.zip \
  https://github.com/brettcannon/cpython-wasi-build/releases/download/v3.14.2/python-3.14.2-wasi_sdk-24.zip
unzip -d runtime runtime/cpython.zip
rm runtime/cpython.zip
```

### Building

```bash
# Build both runners
go build -o jsrunner ./cmd/jsrunner/
go build -o pyrunner ./cmd/pyrunner/
```

## Usage

### JavaScript Runner (jsrunner)

```bash
# Evaluate code directly
./jsrunner -e 'console.log(2 + 2)'

# Interactive REPL
./jsrunner

# JSONL mode with persistent state
echo '{"id":"1","code":"var x = 5"}
{"id":"2","code":"console.log(x * 2)"}' | ./jsrunner --jsonl
# Output:
# {"id":"1"}
# {"id":"2","output":"10\n"}
```

### Python Runner (pyrunner)

```bash
# Evaluate code directly
./pyrunner -runtime ./runtime -c 'print(2 + 2)'

# Interactive REPL
./pyrunner -runtime ./runtime

# JSONL mode with persistent state
echo '{"id":"1","code":"x = 5"}
{"id":"2","code":"print(x * 2)"}' | ./pyrunner -runtime ./runtime --jsonl
# Output:
# {"id": "1"}
# {"id": "2", "output": "10\n"}
```

The pyrunner requires the `-runtime` flag to point to the directory containing `python.wasm` and the `lib/` directory.

## Runtime Files

The `runtime/` directory contains:
- `python.wasm` - CPython 3.14.2 WASI build (~30MB)
- `lib/` - Python standard library
- `qjs.wasm` - QuickJS WASI build (~1.4MB, embedded in jsrunner binary)
- `jsonl_runner.py` - Python JSONL processing script

## State Persistence

### JavaScript
The jsrunner achieves state persistence by replaying all previous code (with output suppressed) before executing each new request. This allows variables, functions, and objects to persist between requests.

### Python
The pyrunner runs a persistent Python process that uses a JSONL processing script (`jsonl_runner.py`) to maintain state in a shared global namespace.

## Testing

Tests are written using pytest and managed with uv:

```bash
# Run all tests
uv run pytest tests/ -v

# Run only JavaScript tests
uv run pytest tests/test_runners.py::TestJSRunner -v

# Run only Python tests
uv run pytest tests/test_runners.py::TestPyRunner -v
```

## Technical Details

### wazero
- Zero-dependency WebAssembly runtime for Go
- Full WASI snapshot preview 1 support
- Supports both AOT compilation and interpreter modes
- Used for sandboxed execution of both Python and JavaScript

### QuickJS WASI (v0.11.0)
- Small embeddable JavaScript engine from quickjs-ng project
- ES2020 support
- Self-contained WASM file (~1.4MB)
- Built-in `console.log` and `print` for output

### CPython WASI (v3.14.2)
- Full Python 3.14 implementation compiled to WASI
- Includes standard library
- Requires PYTHONHOME and PYTHONPATH environment variables
- Full support for imports, comprehensions, and Python features

## Limitations

1. **No network access** - WASI sandboxing prevents network operations
2. **Limited filesystem** - Only mounted directories are accessible
3. **No threading** - WASI doesn't support threads
4. **JavaScript stdlib** - QuickJS WASI build doesn't include the `std` or `os` modules
5. **Memory usage** - Each Python invocation creates a new WASM instance (jsrunner) or maintains a persistent process (pyrunner)

## Project Structure

```
wasm-repl-cli/
├── cmd/
│   ├── jsrunner/
│   │   ├── main.go        # JavaScript runner CLI
│   │   ├── qjs.wasm       # Embedded QuickJS WASM
│   │   └── jsonl_runner.js # (unused, kept for reference)
│   └── pyrunner/
│       └── main.go        # Python runner CLI
├── runtime/
│   ├── python.wasm        # CPython WASI build
│   ├── lib/               # Python standard library
│   └── jsonl_runner.py    # Python JSONL processor
├── tests/
│   └── test_runners.py    # Pytest test suite
├── go.mod
├── go.sum
├── pyproject.toml
├── notes.md               # Development notes
└── README.md              # This file
```

## License

This project uses:
- wazero: Apache License 2.0
- QuickJS: MIT License
- CPython WASI Build: Various (PSF License for Python)
