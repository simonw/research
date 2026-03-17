# syntaqlite Python Extension - Development Notes

## 2026-03-17: Initial research

### What is syntaqlite?
- A parser, formatter, validator, and language server for SQLite SQL
- Built on SQLite's own Lemon-generated grammar and tokenizer
- 99.7% agreement with SQLite's actual parser behavior
- Architecture: C bottom layer (parser/tokenizer), Rust middle layer (formatter/validator/LSP)
- C FFI exports for formatter and validator from Rust

### Key C APIs to expose:
1. **Parser**: parse SQL into AST, get error messages, dump AST as text
2. **Formatter**: format SQL with configurable options (line width, indent, keyword case, semicolons)
3. **Validator**: validate SQL against schemas, get diagnostics with "did you mean?" suggestions
4. **Tokenizer**: split SQL into tokens (for syntax highlighting etc.)

### Build approach:
- syntaqlite produces static libraries via `cargo build --release`:
  - `libsyntaqlite.a` - main Rust library (includes formatter/validator FFI)
  - `libsyntaqlite_engine.a` - C parser/tokenizer engine
  - `libsyntaqlite_sqlite.a` - SQLite dialect (Lemon parser, tokenizer, keywords)
- Python C extension module (`_syntaqlite.c`) wraps the C API
- `setup.py` compiles the C extension and links against the pre-built static libraries
- For WASM: need to cross-compile with emscripten, then use pyodide-build

### Existing examples in this repo:
- `cmarkgfm-in-pyodide/` - Python C API extension → WASM wheel (best pattern to follow)
- `cysqlite-wasm-wheel/` - Cython → WASM with pyodide-build
- `monty-wasm-pyodide/` - Rust/PyO3/Maturin → WASM (shows Rust cross-compilation)
- `pyo3-pyodide-wasm/` - Comprehensive guide for Pyodide version mapping

### Pyodide version mapping (from pyo3-pyodide-wasm):
- Pyodide 0.28.x: Python 3.13.0, Emscripten 4.0.9
- Pyodide 0.29.x: Python 3.13.2, Emscripten 4.0.9

### Python API design:
```python
import syntaqlite

# Parse SQL - returns list of statement dicts with AST dump
stmts = syntaqlite.parse("SELECT 1; SELECT 2")

# Format SQL
formatted = syntaqlite.format_sql("select 1 from foo where bar=1")
formatted = syntaqlite.format_sql("select 1", line_width=80, indent_width=2,
                                   keyword_case="upper", semicolons=True)

# Validate SQL
diagnostics = syntaqlite.validate("SELECT id FROM usr")
diagnostics = syntaqlite.validate("SELECT id FROM usr",
    tables=[{"name": "users", "columns": ["id", "name"]}])

# Tokenize SQL
tokens = syntaqlite.tokenize("SELECT 1 FROM foo")
```

## Build strategy for WASM wheel:
1. Cross-compile Rust to wasm32-unknown-emscripten using cargo + emscripten
2. Compile C extension module with emscripten's emcc
3. Link everything together
4. Package as a Pyodide-compatible wheel

Since syntaqlite already supports emscripten (see build.rs: `-fPIC` for emscripten), the cross-compilation path exists.

For the WASM build, the approach is:
- Install emsdk with correct version for target Pyodide
- Set up Rust wasm32-unknown-emscripten target
- Build the static libs with emscripten
- Build the Python C extension with pyodide-build or direct emcc

## 2026-03-17: Implementation

### C extension module (_syntaqlite.c)
- Wraps 4 core APIs: parse, format_sql, validate, tokenize
- Uses Python C API (compatible with both CPython and Pyodide)
- Links against 3 static libraries: libsyntaqlite.a, libsyntaqlite_engine.a, libsyntaqlite_sqlite.a
- Custom FormatError exception class for format failures
- All memory properly managed (parser/formatter/validator handles created and destroyed per call)

### Test results
- 35 tests, all passing
- Covers: parse (7), format (10), validate (9), tokenize (7), module (2)

### WASM wheel build
- Cross-compiled Rust to wasm32-unknown-emscripten using `cargo build --target wasm32-unknown-emscripten`
- This worked because syntaqlite's build.rs already has `-fPIC` for emscripten targets
- Used pyodide-build 0.25.1 with emsdk 3.1.46

### Issues encountered:
1. **"Argument list too long" error**: Full paths to .a files in extra_link_args were too long.
   Fix: Copy static libs to a short path (/tmp/sqlib) and use -L/-l flags.

2. **wasm-opt version mismatch**: The wasm-opt in emsdk 3.1.46 doesn't support
   `--enable-bulk-memory-opt` which newer Rust LLVM emits.
   Fix: Pass `-sWASM_OPT=0` to skip wasm-opt post-processing.

3. **"did you mean?" suggestions**: These appear in rendered diagnostics output
   (syntaqlite_validator_render_diagnostics) but not in the individual diagnostic
   struct messages. Adjusted test to use render=True mode.

### Final wheel:
- Native: syntaqlite-0.1.0-cp311-cp311-linux_x86_64.whl
- WASM: syntaqlite-0.1.0-cp311-cp311-emscripten_3_1_46_wasm32.whl (164KB)
