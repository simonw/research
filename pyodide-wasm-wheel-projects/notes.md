# Notes

- Created `pyodide-wasm-wheel-projects` for an investigation into research projects that compiled WASM wheels for Pyodide.
- Initial repo scan showed existing Pyodide/WASM-looking folders such as `cysqlite-wasm-wheel`, `pluau-wasm-pyodide`, `pyo3-pyodide-wasm`, `monty-wasm-pyodide`, `cmarkgfm-in-pyodide`, `llm-pyodide-openai-plugin`, and related WASM/browser compiler experiments.
- Existing unrelated dirty worktree changes are present under `datasette-duckdb-safety`; I will ignore them.
- Direct wheel artifact scan, excluding `_site`, found Pyodide/Emscripten WASM wheels in `cysqlite-wasm-wheel`, `cmarkgfm-in-pyodide`, `monty-wasm-pyodide`, and `syntaqlite-python-extension`.
- `pyodide-asgi-browser` has many vendored Pyodide wheels, but its README/notes describe downloading Pyodide and dependency wheels locally for offline browser tests, not compiling those wheels.
- `pyo3-pyodide-wasm` is a guide/background report for building PyO3/maturin wheels for Pyodide, not itself a project with a built wheel artifact.
- `pluau-wasm-pyodide` compiled a standalone Emscripten WASM module and used it from Pyodide, but its notes explicitly describe avoiding the PyO3/Pyodide wheel route.
- Added `scan_pyodide_wheels.py` and ran `uv run python pyodide-wasm-wheel-projects/scan_pyodide_wheels.py`; the output confirmed the four in-project WASM wheels plus vendored Pyodide dependency wheels in `pyodide-asgi-browser`.
- Wrote `README.md` report summarizing confirmed projects, excluded related projects, and a compatibility caveat for the `cmarkgfm-in-pyodide` wheel tag/version trail.
