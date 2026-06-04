Compiling Rust-based Python extension modules (via PyO3 and maturin) into WebAssembly wheels for Pyodide involves precise coordination of toolchain versions and build flags to ensure compatibility. The process relies on maturin (≥1.0) for packaging, the Emscripten SDK (with the exact version used by Pyodide), and a Rust nightly toolchain matching Pyodide's ABI, particularly the `-Z emscripten-wasm-eh` flag and a compatible sysroot for Python 3.13 (Pyodide 0.28+). Wheels must be served with correct ABI and platform tags, and can be loaded in Pyodide using `micropip.install()` or `pyodide.loadPackage()` if CORS headers are set. PyPI does not currently support uploading wasm wheels, so alternatives like GitHub Releases are used.

Key tools and references:
- [PyO3](https://github.com/PyO3/pyo3): Rust bindings for Python.
- [rust-emscripten-wasm-eh-sysroot](https://github.com/pyodide/rust-emscripten-wasm-eh-sysroot): Prebuilt sysroot for the required Rust/Emscripten versions.

Key takeaways:
- The exact Emscripten, Rust nightly, and sysroot versions must match Pyodide's Python/ABI.
- Use `-sSIDE_MODULE=2` and avoid `-pthread` or `-sSIDE_MODULE=1` for Rust builds.
- Wheels must be manually hosted and loaded due to PyPI support wait; CORS is required for browser fetches.
- [micropip](https://micropip.pyodide.org/en/stable/project/api.html) enables runtime wheel installation from URLs.
