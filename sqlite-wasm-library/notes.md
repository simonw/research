# SQLite WASM Library Development Notes

## Goal
Build a Python library that provides a `sqlite3_wasm` module which behaves exactly like Python's standard library `sqlite3`, but runs SQLite compiled to WebAssembly using wasmtime.

## Approach

### Key Components Needed:
1. SQLite compiled to WASM
2. wasmtime Python bindings for running WASM
3. Python wrapper that implements the sqlite3 API

### Research Phase
- Need to find pre-compiled SQLite WASM or compile it ourselves
- Study wasmtime Python API for calling WASM functions
- Understand sqlite3 module API to replicate

## Progress Log

### Step 1: Initial Setup
- Created project folder: sqlite-wasm-library
- Started research on SQLite WASM availability

### Step 2: Research Findings
- SQLite WASM is available from various sources:
  - Official SQLite WASM builds (primarily for browser/Emscripten)
  - VMware Labs webassembly-language-runtimes project
  - wapm-packages/sqlite (CLI tool, not library)
- Best approach: compile SQLite ourselves using wasi-sdk for full control over exports
- wasmtime-py provides Python bindings for running WASM modules with WASI support

### Step 3: Project Setup with uv
- Initialized Python project using `uv init sqlite3_wasm --lib`
- Added dependencies:
  - wasmtime>=40.0.0 (runtime)
  - pytest>=9.0.2 (dev dependency for testing)

### Step 4: Compiling SQLite to WASM
- Downloaded wasi-sdk 21.0 (WebAssembly System Interface SDK)
- Downloaded SQLite 3.45.3 source code
- Created build script (build_wasm.sh) that:
  - Uses wasi-sdk clang compiler
  - Exports 60 SQLite C API functions
  - Exports memory management functions (malloc, free, realloc, strlen)
  - Enables features: FTS5, JSON1, RTREE, column metadata
- Successfully compiled sqlite3.wasm (1.4MB)

### Step 5: Implementing sqlite3_wasm Module
- Created Python module that wraps the SQLite WASM binary
- Key classes implemented:
  - `_SQLiteWasm`: Singleton wrapper for WASM module, handles memory management
  - `Connection`: Database connection, supports context manager
  - `Cursor`: SQL statement execution and result fetching
  - `Row`: Named tuple-like access to query results
- Exception classes matching sqlite3:
  - Error, DatabaseError, IntegrityError, OperationalError
  - ProgrammingError, InterfaceError, DataError, NotSupportedError
- Memory management:
  - `write_string()`: Write UTF-8 strings to WASM memory
  - `read_string()`: Read null-terminated strings from WASM memory
  - `write_bytes()`: Write binary data to WASM memory
  - `read_bytes()`: Read binary data from WASM memory

### Step 6: Test Suite
- Created comprehensive test suite with 60 tests
- Tests cover:
  - Module attributes (apilevel, paramstyle, threadsafety)
  - Connection and cursor creation
  - Execute, executemany, executescript
  - Fetch methods (fetchone, fetchmany, fetchall)
  - Data types (integer, float, text, blob, null, unicode)
  - Transactions (commit, rollback, context manager)
  - Row factory and named access
  - Exception handling
  - SQLite functions (LENGTH, UPPER, LOWER, etc.)
  - Aggregates (COUNT, SUM)
  - FTS5 full-text search
  - JSON functions
- All 60 tests pass

### Step 7: Build Configuration
- Updated pyproject.toml to use hatchling build backend
- Configured wheel to include sqlite3.wasm file
- Built wheel successfully: sqlite3_wasm-0.1.0-py3-none-any.whl (1.45MB)
- Verified wheel installs and works in fresh environment

## Challenges and Solutions

### Challenge 1: WASM Export Configuration
- Initially unclear how to export SQLite functions from WASM
- Solution: Used wasi-sdk linker flags: `-Wl,--export=function_name`

### Challenge 2: Memory Management
- Passing strings between Python and WASM requires manual memory management
- Solution: Implemented write_string/read_string helpers that:
  - Allocate memory using exported malloc
  - Copy data to/from WASM linear memory
  - Handle UTF-8 encoding/decoding

### Challenge 3: Opaque Pointers
- SQLite uses opaque pointers (sqlite3*, sqlite3_stmt*)
- Solution: Store 32-bit pointer values in WASM memory, read with read_int32()

### Challenge 4: WASI Support
- SQLite needs some system calls for file I/O
- Solution: Used wasmtime's WASI support with Linker.define_wasi()

## Limitations
- User-defined functions not supported (require callbacks into host)
- Database backup not supported
- Progress handlers and trace callbacks not supported
- Only in-memory databases work fully (WASI file system support limited)

## Files Created
- `sqlite3_wasm/`: Main Python package directory
  - `src/sqlite3_wasm/__init__.py`: Main module implementation (~900 lines)
  - `src/sqlite3_wasm/sqlite3.wasm`: Compiled SQLite WebAssembly binary
  - `tests/test_sqlite3_wasm.py`: Comprehensive test suite (60 tests)
  - `pyproject.toml`: Package configuration
- `build_wasm.sh`: Script to compile SQLite to WASM
- `wasi-sdk-21.0/`: WASI SDK (not committed)
- `sqlite-autoconf-3450300/`: SQLite source (not committed)

## Usage Example
```python
import sqlite3_wasm

# Works just like sqlite3
conn = sqlite3_wasm.connect(':memory:')
cursor = conn.cursor()
cursor.execute('CREATE TABLE users (id INTEGER, name TEXT)')
cursor.execute('INSERT INTO users VALUES (?, ?)', (1, 'Alice'))
cursor.execute('SELECT * FROM users')
print(cursor.fetchall())  # [(1, 'Alice')]
conn.close()
```
