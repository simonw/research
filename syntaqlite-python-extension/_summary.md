syntaqlite-python-extension is a Python C extension module that integrates the [syntaqlite](https://github.com/LalitMaganti/syntaqlite) Rust/C SQL toolkit, making high-fidelity SQL parsing, formatting, validation, and tokenization available to Python and Pyodide environments. It wraps syntaqlite's native FFI for both desktop and web, linking against static libraries produced by Rust and employing Emscripten for WASM builds. The extension exposes four key functions—parse, format_sql, validate, and tokenize—enabling error-tolerant parsing, customizable formatting, schema-aware validation (with suggestions), and full tokenization, including whitespace/comments. Rigorous test coverage ensures robustness for various SQL dialect scenarios.

**Key findings and features:**
- Leverages SQLite grammar and tokenizer for high compatibility.
- Provides native wheels and Pyodide-compatible WASM wheels ([Pyodide docs](https://pyodide.org/en/stable/) for integration).
- Supports detailed schema validation with "did you mean?" hints.
- Tested across all API functions for diverse use cases and errors.
