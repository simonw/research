# sqlite3-wasm Investigation Report

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

This folder contains a complete implementation of a Python library that provides an `sqlite3_wasm` module - a drop-in replacement for Python's standard `sqlite3` module that runs SQLite compiled to WebAssembly using wasmtime.

## Summary

**Goal**: Build a Python library with wasmtime as a dependency that provides a `sqlite3_wasm` module which behaves exactly like the default Python standard library `sqlite3`, but runs a version of SQLite compiled to WASM.

**Result**: Successfully implemented and tested. The library:
- Compiles SQLite 3.45.3 to WebAssembly using wasi-sdk
- Wraps the WASM module with a Python API matching `sqlite3`
- Passes 60 comprehensive tests
- Builds a working wheel with `uv build` that includes the WASM binary

## Project Structure

```
sqlite-wasm-library/
├── README.md                    # This report
├── notes.md                     # Development notes and progress log
├── build_wasm.sh                # Script to compile SQLite to WASM
├── sqlite3_wasm/                # Python package
│   ├── pyproject.toml           # Package configuration
│   ├── README.md                # Package documentation
│   ├── src/
│   │   └── sqlite3_wasm/
│   │       ├── __init__.py      # Main module (~900 lines)
│   │       ├── sqlite3.wasm     # Compiled SQLite (1.4MB)
│   │       └── py.typed         # Type hints marker
│   ├── tests/
│   │   ├── __init__.py
│   │   └── test_sqlite3_wasm.py # 60 test cases
│   └── dist/
│       └── sqlite3_wasm-0.1.0-py3-none-any.whl
├── wasi-sdk-21.0/               # WASI SDK (not committed)
└── sqlite-autoconf-3450300/     # SQLite source (not committed)
```

## Implementation Approach

### 1. Compiling SQLite to WASM

Used wasi-sdk 21.0 to compile SQLite 3.45.3 to WebAssembly with WASI support:

- Compiler: wasi-sdk clang with `--target=wasm32-wasi`
- Exported 60 SQLite C API functions plus memory management (malloc, free)
- Enabled features: FTS5, JSON1, RTREE, column metadata
- Used `--no-entry` for library mode (no main function)
- Output: 1.4MB WASM file

### 2. Python Wrapper Architecture

```
┌────────────────────────────────────────────┐
│            sqlite3_wasm module             │
│  connect() → Connection → Cursor → Row    │
├────────────────────────────────────────────┤
│              _SQLiteWasm                   │
│   - Loads WASM via wasmtime               │
│   - Memory management helpers             │
│   - Function call wrappers                │
├────────────────────────────────────────────┤
│           wasmtime runtime                 │
│   - WASI support (stdio, basic syscalls)  │
│   - Linear memory access                  │
├────────────────────────────────────────────┤
│          sqlite3.wasm binary               │
│   - SQLite 3.45.3 compiled to WASM        │
└────────────────────────────────────────────┘
```

### 3. Key Technical Challenges

1. **Memory Management**: Implemented helpers to copy data between Python and WASM linear memory
2. **String Handling**: UTF-8 encode/decode with null termination
3. **Opaque Pointers**: Store sqlite3*/sqlite3_stmt* as 32-bit integers
4. **WASI Configuration**: Set up stdout/stderr inheritance for error messages

## Usage Example

```python
import sqlite3_wasm

# Works just like sqlite3
conn = sqlite3_wasm.connect(':memory:')
cursor = conn.cursor()

cursor.execute('CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)')
cursor.execute('INSERT INTO users (name) VALUES (?)', ('Alice',))
cursor.execute('SELECT * FROM users')

print(cursor.fetchall())  # [(1, 'Alice')]
conn.close()
```

## Test Results

All 60 tests pass:

```
tests/test_sqlite3_wasm.py ..............................  [100%]
============================== 60 passed in 0.52s ==============================
```

Tests cover:
- Module attributes and constants
- Connection and cursor lifecycle
- All fetch methods (fetchone, fetchmany, fetchall)
- Data types (integer, float, text, blob, null, unicode)
- Transactions (commit, rollback, context managers)
- Row factory and named column access
- Exception handling
- SQLite built-in functions
- Aggregate functions
- FTS5 full-text search
- JSON functions

## Build Verification

```bash
cd sqlite3_wasm
uv run pytest        # All 60 tests pass
uv build             # Creates wheel with WASM included

# Wheel contents:
# sqlite3_wasm/__init__.py     (31KB)
# sqlite3_wasm/sqlite3.wasm    (1.4MB)
```

## Limitations

Due to WebAssembly execution constraints:
- No user-defined functions/aggregates (require host callbacks)
- No progress handlers or trace callbacks
- No database backup (requires file system)
- In-memory databases work best

## Files for Commit

The following files should be committed:
- `README.md` - This report
- `notes.md` - Development notes
- `build_wasm.sh` - WASM build script
- `sqlite3_wasm/` - Complete Python package (excluding dist/, .venv/)

The `wasi-sdk-21.0/` and `sqlite-autoconf-3450300/` directories are not committed as they are downloaded dependencies.
