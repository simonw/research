Monty WASM + Pyodide explores compiling [Monty](https://github.com/pydantic/monty)—a Rust-based, sandboxed Python interpreter—into WebAssembly for seamless browser access. It provides two integration paths: a standalone WASM module accessible directly from JavaScript, and a Pyodide-compatible wheel for usage in Python-in-the-browser environments. The project enables safe, dependency-free Python code execution with features like variable injection, output capturing (including print statements), and robust error handling. Developers can quickly leverage Monty via simple APIs, as demonstrated in the [live browser demos](https://simonw.github.io/research/monty-wasm-pyodide/demo.html), making in-browser Python useful for education, prototyping, or interactive documentation.

**Key Features and Findings:**
- Two deployment modes: Standalone WASM ES module and Pyodide wheel ([demo links](https://simonw.github.io/research/monty-wasm-pyodide/demo.html), [Pyodide demo](https://simonw.github.io/research/monty-wasm-pyodide/pyodide-demo.html)).
- Supports variable inputs, print output capture, error detection, and returns native JavaScript/Python types.
- Straightforward installation and testing workflows for both WASM and Pyodide contexts.
- Enables secure Python execution in browsers with minimal infrastructure or dependencies.
