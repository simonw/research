# Research Notes: Node.js worker_threads Sandboxing

## Session started: 2026-03-22

### Goal
Research Node.js worker_threads module sandboxing capabilities for Node.js v22.

### Areas to investigate
1. Worker constructor options relevant to sandboxing
2. resourceLimits details
3. Behavior when resource limits exceeded
4. execArgv and permission flags
5. Node.js Permission Model status in v22
6. vm module inside workers
7. npm sandboxing packages

---

## Research Log

### Worker constructor options
- Fetched Node.js v22 docs for worker_threads
- Key sandboxing-relevant options: env, execArgv, eval, resourceLimits, trackUnmanagedFds
- `env` can be set to a custom object (stripping parent env vars) or `SHARE_ENV`
- `execArgv` accepts Node CLI options; V8 options and process-affecting options not supported
- Confirmed `eval: true` lets you pass code as string instead of filename

### resourceLimits
- Controls: maxOldGenerationSizeMb, maxYoungGenerationSizeMb, codeRangeSizeMb, stackSizeMb
- These affect V8 heap only, not external allocations like ArrayBuffers
- Docs say exceeding limits leads to worker termination
- In testing (Node 22.22.0), the worker was very slow to terminate even with 4-8MB limits
- The limits are described as "guidelines, not strict limits" by isolated-vm docs

### Permission Model (key finding!)
- In Node.js v22.13.0, the Permission Model became **stable** (Stability 2) - no longer experimental!
- The flag is now `--permission` (not `--experimental-permission`)
- Tested: `--permission` CAN be passed via `execArgv` to workers - this works!
- With `eval: true` + `execArgv: ['--permission']`, the worker has permission model active
- `process.permission.has('fs.read')` returns true generally but false for specific paths
- fs.readFileSync blocked with ERR_ACCESS_DENIED
- child_process blocked with ERR_ACCESS_DENIED
- Path-specific restrictions work: `--allow-fs-read=/tmp/allowed-only` restricts to that path

### vm module inside workers
- Tested: vm.createContext + vm.runInContext works inside workers
- require is NOT available in vm context by default (good)
- BUT: prototype escape works: `this.constructor.constructor("return process")()`
- This gives access to the real process object from inside the vm context
- vm timeout works: `{ timeout: 100 }` catches infinite loops
- The vm module docs explicitly say: "Do not use it to run untrusted code"

### Combined approach (worker + permission + vm)
- Worker with `execArgv: ['--permission']` + vm.createContext
- Even though prototype escape gives process access, the permission model blocks:
  - fs operations (ERR_ACCESS_DENIED)
  - child_process spawning (ERR_ACCESS_DENIED)
  - worker creation (ERR_ACCESS_DENIED)
- This is a viable defense-in-depth strategy!

### npm packages
- **vm2**: Deprecated July 2023, revived Oct 2025, new CVE Jan 2026 (CVE-2026-22709, CVSS 9.8). Avoid.
- **isolated-vm**: Uses V8 Isolate interface, real heap isolation. Maintenance mode but recommended.
- **quickjs-emscripten**: QuickJS compiled to WASM. Strongest isolation (separate engine entirely).
- **@sebastianwessel/quickjs**: Higher-level wrapper around quickjs-emscripten with TS support.

### Sources consulted
- Node.js v22 docs: worker_threads, permissions, vm
- npm/GitHub: isolated-vm, vm2, quickjs-emscripten
- Various security research articles on vm2 bypasses
