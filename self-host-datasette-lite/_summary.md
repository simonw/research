Datasette Lite, a browser-based SQLite explorer powered by Pyodide and WebAssembly, can be fully self-hosted and used offline by bundling all core files, required Python wheels, and optional sample databases locally instead of relying on external CDNs and PyPI hosts. Achieving this involves downloading Pyodide's core runtime, all necessary wheels for Datasette and its dependencies, modifying key paths in `webworker.js` and `index.html`, and ensuring correct server MIME settings for .wasm files. The minimal offline bundle is around 20–25 MB, while a full Pyodide distribution increases this to about 350 MB and enhances extensibility. Careful dependency resolution and version pinning are needed to avoid runtime conflicts, and users should provide their own databases or include local samples.

**Key findings:**
- Minimal offline bundles (~25 MB) are practical; full Pyodide versions enable more flexibility at a storage cost.
- Dependency wheels must be manually downloaded and installed in correct order; version mismatches (e.g. in httpx) can cause issues.
- [Pyodide](https://pyodide.org/) supports custom hosting, but index paths and MIME types require adjustment ([Pyodide GitHub Issue #4600](https://github.com/pyodide/pyodide/issues/4600)).
- Sample server scripts and static analysis tools help automate the bundling and local hosting process.
