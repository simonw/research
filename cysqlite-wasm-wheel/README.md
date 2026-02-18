# cysqlite WebAssembly Wheel

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

A pre-built WebAssembly wheel of [cysqlite](https://github.com/coleifer/cysqlite) (v0.1.4) that runs in [Pyodide](https://pyodide.org/) in the browser.

cysqlite is a fast Cython-based SQLite3 binding by Charles Leifer. This project cross-compiles it to WebAssembly using Emscripten so it can be loaded in Pyodide without any native dependencies.

## What's included

| File | Description |
|------|-------------|
| `cysqlite-0.1.4-cp311-cp311-emscripten_3_1_46_wasm32.whl` | Pre-built wheel for Pyodide 0.25.x (Python 3.11, Emscripten 3.1.46) |
| `demo.html` | Interactive test page that loads the wheel in Pyodide and runs 115+ upstream tests |
| `build.sh` | Shell script to rebuild the wheel from source |

## Quick start

Serve this directory and open the demo page:

```bash
python3 -m http.server 8765
# open http://localhost:8765/demo.html
```

The demo page will:
1. Load Pyodide from the CDN
2. Install the cysqlite wheel via micropip
3. Fetch the upstream test suite from GitHub
4. Run all compatible tests (115 tests across 21 test classes)
5. Display results with pass/fail status for each test

## Building the wheel

The `build.sh` script automates the entire build. Prerequisites:

- **git** - to clone cysqlite
- **Python 3** - to run the SQLite fetch script
- **pyodide-build** - `pip install pyodide-build`
- **Emscripten SDK** - version must match pyodide-build expectations (3.1.46 for pyodide-build 0.25.1)

```bash
# Install emscripten (if not already installed)
git clone https://github.com/emscripten-core/emsdk.git
cd emsdk && ./emsdk install 3.1.46 && ./emsdk activate 3.1.46
source emsdk_env.sh

# Install pyodide-build
pip install pyodide-build

# Build the wheel
./build.sh
```

The build process:
1. Clones cysqlite from GitHub (or uses existing checkout in `/tmp/cysqlite`)
2. Downloads the SQLite amalgamation (sqlite3.c + sqlite3.h)
3. Uses `pyodide build` which runs Cython, then cross-compiles with emcc to wasm32
4. Produces a `.whl` file with platform tag `emscripten_3_1_46_wasm32`

## Using the wheel in your own Pyodide project

```javascript
const pyodide = await loadPyodide();
await pyodide.loadPackage("micropip");
await pyodide.runPythonAsync(`
    import micropip
    await micropip.install("http://your-server/cysqlite-0.1.4-cp311-cp311-emscripten_3_1_46_wasm32.whl")

    from cysqlite import Connection
    db = Connection(":memory:")
    db.connect()
    db.execute("CREATE TABLE demo (id INTEGER PRIMARY KEY, name TEXT)")
    db.execute("INSERT INTO demo (name) VALUES (?)", ("hello from WASM!",))
    print(db.execute("SELECT * FROM demo").fetchall())
`);
```

## Test results

All 115 upstream tests pass. The only exclusion is `TestThreading` (3 tests) since Pyodide runs single-threaded, and one test gated behind `SLOW_TESTS` is skipped by default.

| Test class | Tests | Status |
|-----------|-------|--------|
| TestBackup | 1 | all pass |
| TestBlob | 6 | all pass |
| TestCheckConnection | 4 | all pass |
| TestDataTypesTableFunction | 1 | all pass |
| TestDatabaseSettings | 6 | 5 pass, 1 skip |
| TestExecute | 21 | all pass |
| TestLargeValues | 2 | all pass |
| TestMedianUDF | 2 | all pass |
| TestModule | 1 | all pass |
| TestOpenConnection | 3 | all pass |
| TestQueryExecution | 5 | all pass |
| TestQueryTypes | 3 | all pass |
| TestRankUDFs | 1 | all pass |
| TestRowFactory | 5 | all pass |
| TestStatementUsage | 10 | all pass |
| TestStringDistanceUDFs | 2 | all pass |
| TestTableFunction | 12 | all pass |
| TestTransactions | 7 | all pass |
| TestUserDefinedCallbacks | 24 | all pass |

## Environment

- **Pyodide**: 0.25.1
- **Python**: 3.11.3
- **cysqlite**: 0.1.4
- **SQLite**: 3.51.2
- **Emscripten**: 3.1.46
- **Platform**: Emscripten-3.1.46-wasm32-32bit
