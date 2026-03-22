# Sandboxing Untrusted Code with Node.js Worker Threads

Investigation into options for running untrusted JavaScript code in a sandbox using `import { Worker } from "node:worker_threads"`, focusing on limiting filesystem access, network access, and CPU/memory monopolization.

**Environment:** Node.js v22.22.0, Linux 6.18.5, March 2026

## TL;DR

There is no single built-in mechanism that covers all three concerns (filesystem, network, memory/CPU). The best approaches combine multiple layers:

| Approach | Filesystem | Network | Memory | CPU | Complexity |
|---|---|---|---|---|---|
| **Worker + Permission Model + vm** | ✅ Blocked | ⚠️ Partial | ❌ Not enforced | ✅ vm timeout + terminate | Low |
| **Worker + isolated-vm** | ✅ No API surface | ✅ No API surface | ✅ Hard limit | ✅ Timeout | Medium |
| **Worker + Permission Model + isolated-vm** | ✅ Double blocked | ✅ No API surface | ✅ Hard limit | ✅ Timeout | Medium |

**Recommended approach: Worker + isolated-vm** (with Permission Model as optional defense-in-depth).

## Mechanism Details

### 1. `resourceLimits` — Memory Limits for Workers

```js
new Worker(file, {
  resourceLimits: {
    maxOldGenerationSizeMb: 32,
    maxYoungGenerationSizeMb: 8,
    codeRangeSizeMb: 4,
    stackSizeMb: 2,
  }
});
```

**Verdict: NOT RELIABLE in Node.js 22.** Tested extensively — workers consistently exceed stated limits by 10-20x without triggering OOM. A worker with a 4MB limit reached 88MB+ heap. ArrayBuffer allocations bypass these limits entirely (allocated outside V8 heap).

The `--max-old-space-size` flag cannot be passed via `execArgv` — Node.js rejects it with `ERR_WORKER_INVALID_EXEC_ARGV`.

### 2. Node.js Permission Model — Filesystem & Process Restrictions

```js
new Worker(file, {
  execArgv: [
    '--experimental-permission',
    '--allow-fs-read=/app/sandbox/*',
    // Omit --allow-fs-write → all writes blocked
    // Omit --allow-child-process → spawn/exec blocked
    // Omit --allow-worker → nested workers blocked
  ],
});
```

**Verdict: WORKS WELL for filesystem and process restrictions.** The permission model can be applied per-worker via `execArgv`. Key behaviors:

- **fsWrite blocked:** `ERR_ACCESS_DENIED` when attempting `fs.writeFileSync`
- **child_process blocked:** `ERR_ACCESS_DENIED` when attempting `execSync`
- **Addons blocked by default:** Native addons (`.node` files) require explicit `--allow-addons`
- **Still experimental** in Node.js 22 (requires `--experimental-permission`)
- **No `--allow-net` flag** — network restrictions not available through this mechanism
- **Defense in depth:** Even if code escapes a `vm` sandbox, the permission model still enforces restrictions at the syscall level

**Important caveats:**
- `process.exit()` still works (terminates the worker, not the main process)
- `process.env` is accessible (potential information leak)
- `--allow-addons` carries a security warning: "could invalidate the permission model"

### 3. `vm` Module — Code-Level Isolation

```js
const sandbox = Object.create(null);
sandbox.console = { log: safeLog };
const context = vm.createContext(sandbox);
vm.runInContext(untrustedCode, context, { timeout: 2000 });
```

**Verdict: NOT A SECURITY BOUNDARY.** Any function injected into the sandbox enables escape:

```js
// Inside sandboxed code:
const global = console.log.constructor('return this')();
// global.process, global.require now accessible
```

Even `Object.create(null)` as the sandbox base doesn't help — any injected function (necessary for communication) provides an escape path via its `constructor` chain.

**However,** `vm` provides useful CPU timeout (`{ timeout: ms }`) and is valuable when combined with the permission model as defense in depth.

### 4. `worker.terminate()` — CPU Kill Switch

```js
const worker = new Worker(code, { eval: true });
setTimeout(() => worker.terminate(), 5000); // hard kill after 5s
```

**Verdict: RELIABLE.** Successfully terminates workers stuck in infinite loops. Returns exit code 1. This is the primary mechanism for preventing CPU monopolization at the worker level.

### 5. `isolated-vm` — True V8 Isolate Sandbox

```js
import ivm from 'isolated-vm';

const isolate = new ivm.Isolate({ memoryLimit: 32 }); // HARD 32MB limit
const context = await isolate.createContext();
await context.global.set('log', new ivm.Callback(fn));
const script = await isolate.compileScript(untrustedCode);
const result = await script.run(context, { timeout: 1000 });
isolate.dispose();
```

**Verdict: STRONGEST SANDBOX.** Creates a separate V8 isolate with:

- **No Node.js APIs:** No `process`, `require`, `fetch`, `fs`, `Buffer`, `setTimeout` — nothing unless explicitly injected
- **Hard memory limits:** Isolate is immediately disposed when limit is exceeded (tested: 16MB limit enforced, worker killed at ~2100 batches)
- **CPU timeout:** Clean timeout on infinite loops
- **Escape-proof:** `this.constructor.constructor('return this')()` returns a global object with no `process` or Node.js APIs
- **Works inside worker threads:** Confirmed — can run isolated-vm inside a worker for additional resource management

**Tradeoffs:**
- Requires native addon (C++ compiled `.node` file)
- Incompatible with Permission Model unless `--allow-addons` is used
- No async/await or Promises in sandboxed code (synchronous V8 isolate)
- Data transfer between isolates requires explicit copy/transfer via `ivm.ExternalCopy`

## Recommended Architecture

### Option A: Maximum Security (Worker + Permission Model + isolated-vm)

```js
const worker = new Worker(sandboxRunner, {
  execArgv: [
    '--experimental-permission',
    '--allow-fs-read=*',
    '--allow-addons',  // Required for isolated-vm
  ],
});

// Inside worker: use isolated-vm for code execution
// Permission model provides defense-in-depth
```

**Protections:**
- Filesystem write: Blocked by Permission Model
- Filesystem read: Controlled by Permission Model path allowlist
- Network: No API surface in isolated-vm (no `fetch`, `http`, `net`)
- Child process: Blocked by Permission Model
- Memory: Hard limit via `ivm.Isolate({ memoryLimit })` ✅
- CPU: Timeout via `script.run(ctx, { timeout })` + `worker.terminate()` as fallback ✅

### Option B: Simpler Setup (Worker + isolated-vm, no Permission Model)

```js
const worker = new Worker(sandboxRunner);
// Kill switch for runaway workers
setTimeout(() => worker.terminate(), 30000);

// Inside worker: use isolated-vm for all untrusted code
```

**Protections:**
- All I/O blocked by isolated-vm (no APIs available)
- Memory: Hard limit via isolated-vm
- CPU: Timeout via isolated-vm + worker.terminate()
- Worker code itself has full Node.js access (but only runs trusted orchestration code)

### Option C: No Native Addons (Worker + Permission Model + vm)

```js
const worker = new Worker(sandboxRunner, {
  execArgv: [
    '--experimental-permission',
    '--allow-fs-read=/app/code/*',
  ],
});

// Inside worker: use vm.runInContext with timeout
// vm escapes are caught by Permission Model
```

**Protections:**
- Filesystem: Blocked by Permission Model (even after vm escape) ✅
- Network: Partially blocked (no `--allow-net` flag available) ⚠️
- Child process: Blocked by Permission Model ✅
- Memory: NOT enforced (resourceLimits ineffective) ❌
- CPU: vm timeout + worker.terminate() ✅
- `process.env` accessible after vm escape (information leak) ⚠️

## What's Missing

1. **Memory limits for workers don't work** — `resourceLimits` is unreliable in Node.js 22. Only `isolated-vm` provides hard memory enforcement.

2. **No network restriction built into Node.js** — The Permission Model doesn't have a working `--allow-net` flag. Network access must be prevented by not providing `fetch`/`http` APIs (which `isolated-vm` achieves, but `vm` does not after escape).

3. **`process.env` leaks** — After a vm escape, environment variables are accessible. Use `env: {}` worker option or `isolated-vm` to prevent this.

4. **`process.exit()` works in restricted workers** — Untrusted code can crash its own worker (but not the main process). Handle this with worker error/exit event handlers.

## Alternatives Not Tested

- **quickjs-emscripten** — Runs QuickJS (JS engine) in WebAssembly. Complete sandboxing via WASM memory model. No native addons needed. Slower than V8 for computation but excellent isolation.
- **Deno** — Built-in fine-grained permission model (`--allow-read`, `--allow-net`, `--allow-write`) at the runtime level. If you can switch runtimes, Deno offers the most complete built-in sandboxing.
- **ShadowRealm** — TC39 Stage 3 proposal. Not yet available in Node.js 22.

## Files in This Directory

| File | Description |
|---|---|
| `01-resource-limits.mjs` | Tests resourceLimits with ArrayBuffer (doesn't enforce) |
| `01b-resource-limits-heap.mjs` | Tests resourceLimits with V8 heap objects (doesn't enforce) |
| `01c-resource-limits-unique.mjs` | Tests with unique strings (doesn't enforce) |
| `01d-execargv-memory.mjs` | Tests --max-old-space-size via execArgv (rejected) |
| `01e-resource-limits-measured.mjs` | Tests with process.memoryUsage() measurement (confirms non-enforcement) |
| `01f-tiny-limit.mjs` | Tests 4MB limit (still reaches 88MB+) |
| `02-permission-model.mjs` | Tests Permission Model via worker execArgv (works!) |
| `02b-permission-control.mjs` | Control test without Permission Model |
| `03-eval-worker.mjs` | Tests vm sandbox escapes (escapable) |
| `03b-vm-hardened.mjs` | Tests hardened vm context (still escapable via injected functions) |
| `04-cpu-timeout.mjs` | Tests CPU timeout: worker.terminate(), vm timeout, message-based |
| `05-permission-main-process.mjs` | Tests Permission Model on main process |
| `06-isolated-vm.mjs` | Tests isolated-vm: isolation, memory, CPU, filesystem, network |
| `07-combined-sandbox.mjs` | Combined: Worker + Permission Model + isolated-vm |
| `07b-debug-perms.mjs` | Debug: Permission Model blocks native addons |
| `07c-permission-plus-addons.mjs` | Tests --allow-addons with Permission Model + isolated-vm |
| `07d-permission-plus-vm.mjs` | Tests Permission Model + vm module (pure Node.js) |
| `07e-vm-escape-with-perms.mjs` | Tests: does Permission Model protect after vm escape? |
| `07f-vm-escape-cjs.mjs` | Tests vm escape in CJS worker with Permission Model |
