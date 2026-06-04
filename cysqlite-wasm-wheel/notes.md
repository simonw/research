# cysqlite WASM Wheel - Investigation Notes

## Goal

Compile [cysqlite](https://github.com/coleifer/cysqlite) (a Cython-based SQLite3 binding by Charles Leifer) to WebAssembly and bundle it as a wheel that Pyodide can load in the browser. Run the upstream test suite to verify it works.

## Approach

### Understanding cysqlite

- cysqlite is a Cython extension module wrapping SQLite3 via the C API
- Source: `src/cysqlite.pyx` (~107K) with helpers in `src/cysqlite.pxd` and `src/sqlite3.pxi`
- The build system (setup.py) supports bundling the SQLite amalgamation (`sqlite3.c` + `sqlite3.h`) for a self-contained build
- Test suite: `tests.py` with ~2574 lines covering connections, queries, transactions, UDFs, table functions, blobs, FTS, and more

### Build toolchain

- **pyodide-build 0.25.1** provides the `pyodide build` command
- This version targets **Python 3.11.3** and **Emscripten 3.1.46**
- The Pyodide cross-build environment (xbuildenv) includes Python headers and libraries compiled for wasm32
- **Emscripten SDK 3.1.46** provides `emcc` (the C/C++ to WASM compiler)

### Build process

1. Clone cysqlite to `/tmp/cysqlite`
2. Run `python scripts/fetch_sqlite` to download the SQLite amalgamation (3.51.2)
3. Run `pyodide build --outdir /tmp/cysqlite-wheels` from the cysqlite directory
4. pyodide-build handles:
   - Running Cython to convert `.pyx` -> `.c`
   - Compiling `sqlite3.c` and `cysqlite.c` with emcc (cross-compiling to wasm32)
   - Linking into a `.so` (actually a wasm side module)
   - Packaging into a wheel with the correct platform tag

The resulting wheel: `cysqlite-0.1.4-cp311-cp311-emscripten_3_1_46_wasm32.whl` (~688 KB)

### Key discovery: micropip install method

Initial attempt used `pyodide.FS.writeFile()` to write the wheel to the emscripten FS, then `micropip.install("file:///tmp/...")`. This failed because micropip's `file://` handler goes through the browser's fetch API, which can't access the emscripten virtual filesystem.

**Solution**: Serve the wheel via HTTP from the same origin and use `micropip.install("http://localhost:PORT/wheel.whl")`. micropip fetches it normally over HTTP.

### Test compatibility

The test suite has 22 test classes. Of these:
- **TestThreading** was skipped because Pyodide runs single-threaded (no real OS threads)
- **1 test** (`test_for_leaks`) is gated behind `SLOW_TESTS` env var and skipped by default
- All remaining **115 tests passed** without modification

Tests that use file-based databases (`/tmp/cysqlite.db`) work fine because Pyodide's emscripten FS supports `/tmp` as a writable in-memory filesystem.

Features verified working in WASM:
- Basic connections and queries
- Parameterized queries (qmark and named)
- Transactions (begin, commit, rollback, savepoints, atomic context manager)
- User-defined functions (scalar, aggregate, window, collation)
- Table functions (virtual tables implemented in Python)
- Blob I/O
- FTS3/FTS4/FTS5 full-text search
- Statement caching and reuse
- Backup API
- Row factories and result types
- Database metadata and pragmas
- Large value handling
- String distance UDFs (Levenshtein, Damerau-Levenshtein)
- Median aggregate/window function
- BM25 and Lucene ranking functions

## Tools used

- `rodney` - Chrome automation CLI, used to test the demo page headlessly
- `pyodide-build` 0.25.1 - Pyodide package build tool
- Emscripten SDK 3.1.46 - C/C++ to WebAssembly compiler
- Pyodide 0.25.1 (CDN) - Python runtime for the browser
