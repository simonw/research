Exploring sandboxing strategies for untrusted JavaScript code in Node.js v22, the project systematically tested worker threads, the Node.js Permission Model, and both the built-in `vm` module and the [isolated-vm](https://github.com/laverdet/isolated-vm) library. Results showed that Node's built-in memory limits (`resourceLimits`) are ineffective, and the Permission Model reliably restricts filesystem/process access but cannot block network calls or memory consumption. The standard `vm` module is not escape-proof, so only `isolated-vm`—which leverages native V8 isolates—enforces hard memory/CPU limits while removing all I/O and Node.js API surface. The recommended setup is using a worker thread combined with `isolated-vm`, optionally bolstered by the Permission Model for defense-in-depth, or considering [Deno](https://deno.com/) for more robust built-in permissions if switching runtimes is feasible.

**Key findings:**
- Node.js `resourceLimits` do not reliably enforce memory boundaries; workers can exceed limits by ~20x.
- The Permission Model blocks filesystem writes/child process creation but cannot restrict network access.
- `isolated-vm` offers robust isolation: no Node APIs, enforced memory/CPU limits, and proven resistance to common escape patterns.
- Standard `vm` is not secure; injected functions enable escape to global scope.
- For critical production sandboxing, only approaches using V8 isolates (`isolated-vm` or QuickJS/WASM) are viable in Node.js.
