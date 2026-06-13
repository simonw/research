# Projects That Compiled WASM Wheels for Pyodide

This scan looked through the research repo for projects that actually produced Python wheel artifacts targeting Pyodide/WebAssembly. I counted a project only when both were true:

- The repo contains, or the report names, a wheel with an Emscripten/Pyodide WASM platform tag.
- The project's own README/notes/build script describe compiling that wheel, rather than only downloading or vendoring a dependency wheel.

I excluded `_site/` generated copies and this investigation folder. The reproducible artifact scan is in `scan_pyodide_wheels.py`:

```bash
uv run python pyodide-wasm-wheel-projects/scan_pyodide_wheels.py
```

## Confirmed projects

| Project | Produced wheel | Size | How it was built |
| --- | --- | ---: | --- |
| [`cmarkgfm-in-pyodide`](../cmarkgfm-in-pyodide/README.md) | `dist/cmarkgfm_pyodide-2025.10.22-cp312-cp312-emscripten_3_1_46_wasm32.whl` | 90,539 bytes | Reworked `cmarkgfm` away from CFFI to a Python C API extension, compiled the C extension and cmark-gfm C sources to WASM, then packaged a Pyodide wheel. |
| [`cysqlite-wasm-wheel`](../cysqlite-wasm-wheel/README.md) | `cysqlite-0.1.4-cp311-cp311-emscripten_3_1_46_wasm32.whl` | 704,623 bytes | Used `pyodide build` with Emscripten 3.1.46 to compile Cython-generated C plus SQLite amalgamation into a Pyodide wheel. |
| [`monty-wasm-pyodide`](../monty-wasm-pyodide/README.md) | `pydantic_monty-0.0.3-cp313-cp313-emscripten_4_0_9_wasm32.whl` | 4,165,724 bytes | Built the Rust/PyO3 `pydantic_monty` package as an Emscripten side-module wheel for Pyodide 0.29/Python 3.13 using Rust nightly, the Pyodide wasm-eh sysroot, and maturin. |
| [`syntaqlite-python-extension`](../syntaqlite-python-extension/README.md) | `dist/syntaqlite-0.1.0-cp311-cp311-emscripten_3_1_46_wasm32.whl` | 167,180 bytes | Cross-compiled Rust/C static libraries to `wasm32-unknown-emscripten`, then used `pyodide build` to compile and package the Python C extension. |

## Related projects not counted

| Project | Reason |
| --- | --- |
| [`pyodide-asgi-browser`](../pyodide-asgi-browser/README.md) | Contains many local `vendor/*.whl` files, including Pyodide WASM wheels such as `pydantic_core`, `sqlite3`, `ssl`, `markupsafe`, and `pyyaml`, but the project documents downloading those from Pyodide/PyPI for offline browser use, not compiling them. |
| [`pyo3-pyodide-wasm`](../pyo3-pyodide-wasm/README.md) | A guide to building PyO3/maturin Rust extension wheels for Pyodide. It does not appear to be a concrete project with its own built wheel artifact. |
| [`pluau-wasm-pyodide`](../pluau-wasm-pyodide/README.md) | Built a standalone Emscripten `luau.wasm` and integrated that from Pyodide, but the notes say it chose this instead of compiling the PyO3 package as a Pyodide wheel. |
| [`wazero-python-claude`](../wazero-python-claude/README.md) | Contains a native macOS wheel, not a Pyodide/Emscripten WASM wheel. |

## Notes

- `cmarkgfm-in-pyodide` has an unusual version trail: the wheel filename is `cp312` with `emscripten_3_1_46_wasm32`, while its documentation references both Pyodide 0.26.4/Python 3.12 and pyodide-build 0.25.1/Emscripten 3.1.46. It is still a clear local example of a project compiling a Pyodide-targeted wheel, but I would re-test compatibility before reusing that exact artifact.
- `monty-wasm-pyodide` is the only confirmed project in this scan using the newer Emscripten 4.0.9 / Python 3.13 Pyodide wheel target.
- `cysqlite-wasm-wheel`, `cmarkgfm-in-pyodide`, and `syntaqlite-python-extension` all target the older `emscripten_3_1_46_wasm32` wheel tag.
