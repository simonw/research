# syntaqlite Python Extension

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

A Python C extension module that exposes the core features of [syntaqlite](https://github.com/LalitMaganti/syntaqlite) — a parser, formatter, validator, and language server for SQLite SQL built on SQLite's own grammar and tokenizer.

## Features

- **Parse** SQL into AST with error recovery
- **Format** SQL with configurable line width, indentation, keyword casing, and semicolons
- **Validate** SQL against schemas with "did you mean?" suggestions
- **Tokenize** SQL into individual tokens (including whitespace and comments)

## Architecture

syntaqlite is a Rust/C project with a layered design:
- **C layer**: Parser engine and SQLite tokenizer (Lemon-generated grammar)
- **Rust layer**: Formatter, semantic validator, and language server
- **C FFI**: Rust exposes formatter/validator via `extern "C"` functions

This Python extension:
1. Links against pre-built static libraries from `cargo build --release`
2. Wraps the C API using Python's C Extension API
3. Produces both native wheels and Pyodide-compatible WASM wheels

### Static libraries used:
| Library | Contents |
|---------|----------|
| `libsyntaqlite.a` | Rust crate (formatter FFI, validator FFI) |
| `libsyntaqlite_engine.a` | C parser/tokenizer engine |
| `libsyntaqlite_sqlite.a` | SQLite dialect (Lemon parser, tokenizer, keywords) |

## API

### `syntaqlite.parse(sql: str) -> list[dict]`

Parse SQL into a list of statement results.

```python
>>> import syntaqlite
>>> syntaqlite.parse("SELECT 1; SELECT 2")
[{'ok': True, 'ast': 'Select\n  result_columns: ...\n', 'error': None},
 {'ok': True, 'ast': 'Select\n  result_columns: ...\n', 'error': None}]

>>> syntaqlite.parse("SELECT FROM")
[{'ok': False, 'ast': None, 'error': 'near "FROM": syntax error',
  'error_offset': 7, 'error_length': 4}]
```

### `syntaqlite.format_sql(sql, *, line_width=80, indent_width=2, keyword_case="upper", semicolons=True) -> str`

Format SQL with configurable options.

```python
>>> syntaqlite.format_sql("select a,b from t where x=1")
'SELECT\n  a,\n  b\nFROM\n  t\nWHERE\n  x = 1;\n'

>>> syntaqlite.format_sql("SELECT 1", keyword_case="lower")
'select\n  1;\n'
```

Raises `syntaqlite.FormatError` on parse errors.

### `syntaqlite.validate(sql, *, tables=None, render=False) -> list[dict] | str`

Validate SQL against an optional schema.

```python
>>> syntaqlite.validate(
...     "SELECT id FROM usr",
...     tables=[{"name": "users", "columns": ["id", "name"]}]
... )
[{'severity': 'error', 'message': "unknown table 'usr'",
  'start_offset': 15, 'end_offset': 18}]

>>> syntaqlite.validate(
...     "SELECT id FROM usr",
...     tables=[{"name": "users", "columns": ["id", "name"]}],
...     render=True
... )
"error: unknown table 'usr'\n --> <input>:1:16\n  |\n1 | SELECT id FROM usr\n  |                ^~~\n  = help: did you mean 'users'?\n"
```

### `syntaqlite.tokenize(sql: str) -> list[dict]`

Tokenize SQL into individual tokens.

```python
>>> syntaqlite.tokenize("SELECT 1")
[{'text': 'SELECT', 'offset': 0, 'length': 6, 'type': 133},
 {'text': ' ', 'offset': 6, 'length': 1, 'type': 187},
 {'text': '1', 'offset': 7, 'length': 1, 'type': 152}]
```

## Building

### Prerequisites

- Rust toolchain (`rustup`)
- Python 3.10+ with development headers
- The syntaqlite source at `/tmp/syntaqlite` (or set `SYNTAQLITE_SRC`)

### Native build

```bash
# 1. Clone and build syntaqlite
git clone https://github.com/LalitMaganti/syntaqlite /tmp/syntaqlite
cd /tmp/syntaqlite && cargo build --release

# 2. Build and install the Python extension
cd syntaqlite-python-extension
pip install -e .

# 3. Run tests
uv run pytest test_syntaqlite.py -v
```

### WASM wheel (Pyodide)

```bash
# 1. Install Emscripten SDK 3.1.46
git clone https://github.com/emscripten-core/emsdk.git /tmp/emsdk
cd /tmp/emsdk && ./emsdk install 3.1.46 && ./emsdk activate 3.1.46

# 2. Install pyodide-build
pip install pyodide-build

# 3. Add Rust WASM target
rustup target add wasm32-unknown-emscripten

# 4. Run the build script
./build_wasm.sh
```

The WASM wheel is written to `dist/syntaqlite-0.1.0-cp311-cp311-emscripten_3_1_46_wasm32.whl`.

### Using in Pyodide

```javascript
const pyodide = await loadPyodide();
await pyodide.loadPackage("micropip");
const micropip = pyodide.pyimport("micropip");
await micropip.install("http://your-server/syntaqlite-0.1.0-cp311-cp311-emscripten_3_1_46_wasm32.whl");
pyodide.runPython(`
    import syntaqlite
    print(syntaqlite.format_sql("select 1 from foo"))
`);
```

## WASM Build Details

The WASM build follows this process:

1. **Cross-compile Rust** to `wasm32-unknown-emscripten` using `cargo build --target wasm32-unknown-emscripten`. syntaqlite's build.rs already supports this (adds `-fPIC` for emscripten).

2. **Build Python C extension** with `pyodide build`, which invokes `setup.py` with emscripten's `emcc` compiler. The setup.py detects emscripten and:
   - Copies static libraries to a short path (avoids "argument list too long" errors)
   - Links with `-L`/`-l` flags against the WASM static libraries
   - Adds `-sWASM_OPT=0` to skip wasm-opt (version mismatch between emsdk 3.1.46 and Rust's LLVM)

3. **Package** as a standard Pyodide wheel (`*-emscripten_3_1_46_wasm32.whl`)

### Version compatibility

| Pyodide | Python | Emscripten | pyodide-build |
|---------|--------|------------|---------------|
| 0.25.x  | 3.11.3 | 3.1.46     | 0.25.1        |

For newer Pyodide versions (0.28+/0.29+), update emsdk to 4.0.9 and use the corresponding pyodide-build version.

## Test Coverage

35 tests covering all 4 API functions:
- **Parse** (7 tests): simple, multiple statements, syntax errors, empty input, complex queries
- **Format** (10 tests): keyword casing, semicolons, line width, indentation, error handling
- **Validate** (9 tests): schema validation, "did you mean?" suggestions, rendered diagnostics, multiple tables
- **Tokenize** (7 tests): token text/offsets/types, whitespace, comments, multi-statement

## Files

| File | Description |
|------|-------------|
| `_syntaqlite.c` | Python C extension module (~370 lines) |
| `setup.py` | Build configuration (handles native + WASM) |
| `pyproject.toml` | Python packaging metadata |
| `test_syntaqlite.py` | Test suite (35 tests) |
| `build_wasm.sh` | WASM wheel build script |
| `notes.md` | Development notes and learnings |
