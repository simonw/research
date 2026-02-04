WASM REPL CLI Tools enable JavaScript and Python REPLs from the command line by leveraging WebAssembly runtimes in Go, built on the [wazero](https://github.com/tetratelabs/wazero) engine. The project supplies separate binaries for each language—one using [QuickJS WASI](https://github.com/quickjs-ng/quickjs) and the other CPython WASI—offering direct code execution, interactive shells, and a JSONL mode. JSONL mode lets external applications submit code for execution while maintaining persistent state across requests, facilitating programmatic integration. Although the WASM runtime files must be downloaded separately due to their size, the solution provides robust sandboxed execution, limited filesystem access, and strict isolation for secure evaluation.

**Key features and findings:**
- Supports direct, interactive, and programmatic (JSONL) code evaluation with persistent state.
- Uses persistent process (Python) or code replay (JavaScript) for state retention between requests.
- Relies on WASI for sandboxing, imposing restrictions on networking and threading.
- Integrates with testing frameworks (pytest, uv) for reliability and maintainability.
