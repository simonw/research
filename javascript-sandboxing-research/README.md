# JavaScript Sandboxing Research

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

This README is a combined copy of the three original investigation READMEs. The only additions are the wrapper headings used to glue them together in a single report.

## Investigation 1: JavaScript Sandbox Solutions for Running Untrusted Code

Research conducted 2026-03-22.

## 1. isolated-vm

**GitHub:** [laverdet/isolated-vm](https://github.com/laverdet/isolated-vm)
**npm:** [isolated-vm](https://www.npmjs.com/package/isolated-vm)

### How It Works

isolated-vm exposes V8's Isolate interface to Node.js. Each isolate is an independent V8 execution environment with its own heap, garbage collector, and global object. Isolates share no state — data must be explicitly copied or transferred between them via `ExternalCopy` (structured clone) or `Reference` (cross-isolate object handle).

### API Overview

```js
const ivm = require('isolated-vm');
const isolate = new ivm.Isolate({ memoryLimit: 128 }); // MB
const context = await isolate.createContext();
const jail = context.global;

// Expose a function
await jail.set('log', new ivm.Reference(function(msg) { console.log(msg); }));

// Run code with timeout
const result = await context.eval('1 + 2', { timeout: 1000 });

// Track resource usage
console.log(isolate.cpuTime);   // bigint, nanoseconds
console.log(isolate.wallTime);  // bigint, nanoseconds
```

### What It Can Restrict

| Resource | Restricted? | Details |
|----------|-------------|---------|
| Memory | Yes | `memoryLimit` option (default 128MB, min 8MB). Soft limit — determined attackers may use 2-3x before termination. |
| CPU / Time | Yes | `timeout` option in ms. `cpuTime`/`wallTime` tracking in nanoseconds. |
| Filesystem | Yes | No fs access by default. Only available if explicitly exposed via Reference. |
| Network | Yes | No network access by default. Same as filesystem. |

### Level of Isolation

**Separate V8 isolate, same process.** Each isolate runs in its own thread with its own heap. This is the same isolation boundary used by Cloudflare Workers. No Node.js APIs are available inside the isolate unless explicitly bridged.

### Performance

V8 JIT compilation is available inside isolates, so raw computation runs at native V8 speed. The main overhead comes from data marshalling between isolates (structured clone serialization). Startup cost is low compared to worker_threads since no Node.js environment is initialized.

### Maturity / Maintenance

- **Status:** Maintenance mode. Bug fixes and Node.js version support continue; no new features.
- **Requires:** C++ compiler for installation; `--no-node-snapshot` flag on Node 20+.
- **Users:** Screeps (MMO game), Fly.io CDN.
- **Compatibility with worker_threads:** isolated-vm runs isolates in threads within the same process. It is complementary to worker_threads — you can run isolated-vm inside a worker_thread for additional process-level separation.

---

## 2. vm2

**GitHub:** [patriksimek/vm2](https://github.com/patriksimek/vm2)
**npm:** [vm2](https://www.npmjs.com/package/vm2)

### Current Status: Fragile, Use With Extreme Caution

vm2 was **deprecated in July 2023** after repeated critical sandbox escapes. It was **resurrected in October 2025** with patches and a planned TypeScript rewrite. However, **CVE-2026-22709** (CVSS 9.8) was disclosed in January 2026 — a sandbox escape via unsanitized Promise `.then()`/`.catch()` callbacks. This was fixed in v3.10.2/3.10.3.

The project has had **over 20 known sandbox escape CVEs** throughout its history, including:
- CVE-2022-36067, CVE-2023-29017, CVE-2023-29199, CVE-2023-30547
- CVE-2023-32314, CVE-2023-37466, CVE-2023-37903
- CVE-2026-22709

### Why It Keeps Breaking

vm2 operates within the **same V8 isolate** as the host. It wraps Node's built-in `vm` module with Proxy-based protections. Attackers escape via prototype chain traversal, constructor access through error objects, Symbol protocol hooks, and async execution timing windows. The maintainer openly acknowledges: *"new bypasses will likely be discovered in the future."*

### What It Can Restrict

| Resource | Restricted? | Details |
|----------|-------------|---------|
| Memory | No | Same isolate as host process. |
| CPU / Time | Partial | Timeout option available. |
| Filesystem | Attempted | Proxy-based blocking, but bypasses have been found. |
| Network | Attempted | Same — proxy-based, historically bypassed. |

### Level of Isolation

**Same V8 isolate, same process.** This is the weakest possible isolation level. The sandbox boundary is enforced by JavaScript-level Proxy wrappers, not by V8 engine separation.

### Recommendation

**Do not use vm2 for running untrusted code in production.** The vm2 maintainer and multiple security researchers recommend [isolated-vm](https://github.com/laverdet/isolated-vm) as an alternative with stronger isolation guarantees.

---

## 3. quickjs-emscripten / quickjs-ng

**GitHub:** [justjake/quickjs-emscripten](https://github.com/justjake/quickjs-emscripten)
**npm:** [quickjs-emscripten](https://www.npmjs.com/package/quickjs-emscripten)
**Higher-level wrapper:** [@sebastianwessel/quickjs](https://github.com/sebastianwessel/quickjs)
**QuickJS-NG:** [quickjs-ng/quickjs](https://github.com/quickjs-ng/quickjs)

### How It Works

QuickJS (a lightweight JavaScript engine by Fabrice Bellard) is compiled to WebAssembly via Emscripten. The WASM module runs inside the host JavaScript runtime, providing a completely separate JS execution environment. Each WASM module instance has its own linear memory with no access to the host's heap, filesystem, or network.

**QuickJS-NG** is an actively maintained fork of QuickJS with newer JS features (targeting latest TC39 stage 4 proposals), a CMake build system, and community-driven development with 40+ contributors. quickjs-emscripten supports both original QuickJS and QuickJS-NG as build variants (`@jitl/quickjs-ng-wasmfile-release-sync`, etc.).

### API Overview

```js
import { getQuickJS } from 'quickjs-emscripten';

const QuickJS = await getQuickJS();
const vm = QuickJS.newContext();

// Set memory and CPU limits
const runtime = vm.runtime;
runtime.setMemoryLimit(1024 * 1024);      // 1MB
runtime.setMaxStackSize(1024 * 256);       // 256KB stack
runtime.setInterruptHandler(() => shouldStop); // CPU interrupt

// Evaluate code
const result = vm.evalCode('1 + 2');
if (result.error) {
  console.log('Error:', vm.dump(result.error));
  result.error.dispose();
} else {
  console.log('Result:', vm.dump(result.value));
  result.value.dispose();
}

vm.dispose();
```

### What It Can Restrict

| Resource | Restricted? | Details |
|----------|-------------|---------|
| Memory | Yes | `setMemoryLimit()` — hard limit within WASM linear memory. |
| CPU / Time | Yes | `setInterruptHandler()` — callback invoked regularly during execution; can abort. |
| Filesystem | Yes | No fs access. WASM has no syscall interface. Virtual FS available in @sebastianwessel/quickjs. |
| Network | Yes | No network access. Must be explicitly provided by host. |

### Level of Isolation

**WASM sandbox — the strongest isolation on this list.** The sandboxed code runs in a WebAssembly linear memory space with no access to the host's memory, syscalls, or APIs. Even a complete memory corruption bug in QuickJS cannot escape the WASM sandbox boundary (enforced by the WASM runtime itself).

### Performance

This is the main trade-off. QuickJS is a **pure interpreter with no JIT compilation**. Expect:
- **10-50x slower** than V8 for computational workloads.
- Sync WASM module: ~500KB. Async (ASYNCIFY) variant: ~1MB, ~40% additional overhead.
- Ideal for short-running scripts, config evaluation, simple business logic — not heavy computation.

### Known Issues

- OOM inside WASM can leave the module in an unrecoverable state (must create a new module instance).
- Manual memory management required: all JSValue handles must be explicitly disposed.

### Maturity / Maintenance

- **quickjs-emscripten:** Actively maintained. Latest release uses QuickJS 2025-09-13 (vendored Feb 2026).
- **QuickJS-NG:** Very active. ~2 month release cadence, 40+ contributors, 400+ PRs, extensive CI.
- **@sebastianwessel/quickjs:** Active. Provides virtual FS, fetch client, TypeScript execution, test runner.
- **Compatibility with worker_threads:** Works well — run a WASM module inside a worker_thread for both WASM isolation and OS-thread separation.

---

## 4. ShadowRealm (TC39 Proposal)

**Spec:** [tc39/proposal-shadowrealm](https://github.com/tc39/proposal-shadowrealm)
**Polyfill:** [shadowrealm-api](https://www.npmjs.com/package/shadowrealm-api)

### Current Stage

**Stage 2.7** as of March 2025. A Stage 3 request was made at TC39 in December 2024, but consensus on web integration parts is still pending. The February 2025 TC39 plenary reported that TC39-side open questions are resolved, but browser/web integration work remains.

### Is It Available in Node.js 22?

**Not by default.** Node.js has an `--experimental-shadow-realm` flag that enables basic support (`ShadowRealm` constructor and `evaluate()`), but it is experimental and not enabled by default in any current Node.js release.

### How It Works

ShadowRealm creates a new JavaScript global environment (separate global object, separate intrinsics like `Object`, `Array`, etc.) but runs **synchronously in the same thread and same V8 isolate**. Only primitive values and callable functions can cross the realm boundary.

```js
const realm = new ShadowRealm();
const result = realm.evaluate('1 + 2'); // 3
const fn = realm.evaluate('(x) => x * 2');
fn(5); // 10
```

### What It Can Restrict

| Resource | Restricted? | Details |
|----------|-------------|---------|
| Memory | No | Same isolate/heap as host. |
| CPU / Time | No | No built-in timeout or interrupt mechanism. |
| Filesystem | Partial | No Node.js APIs available by default, but not a security boundary. |
| Network | Partial | Same — no APIs available but not enforced as a security guarantee. |

### Level of Isolation

**Same V8 isolate, same thread.** ShadowRealm provides **global object isolation, not security isolation**. It is designed for code that needs a clean global environment (plugin systems, testing, library isolation) — **not for running untrusted code**. The TC39 proposal does not describe ShadowRealm as a security sandbox.

### Performance

Minimal overhead — same V8 JIT, same thread, no serialization needed for primitives. The lightest-weight option on this list.

### Maturity

- **Stage 2.7** — not yet standardized. API may change.
- Polyfill exists but is not suitable for security-critical use.
- Not recommended for untrusted code execution.

---

## 5. Web Workers in Deno and Bun

### Deno Workers

**Docs:** [Deno Security](https://docs.deno.com/runtime/fundamentals/security/), [Deno Workers](https://docs.deno.com/runtime/manual/runtime/workers/)

Deno's runtime is **secure by default** — no file, network, or environment access without explicit permission flags (`--allow-read`, `--allow-net`, `--allow-env`, etc.).

#### Per-Worker Permission Sandboxing

```ts
const worker = new Worker(new URL("./untrusted.js", import.meta.url).href, {
  type: "module",
  deno: {
    permissions: {
      net: ["api.example.com"],     // Only this domain
      read: ["/data/allowed.txt"],  // Only this file
      write: false,                 // No write access
      run: false,                   // No subprocess spawning
    },
  },
});

// Or fully sandboxed:
const sandboxed = new Worker(url, {
  type: "module",
  deno: { permissions: "none" },
});
```

Workers **cannot escalate** beyond parent permissions. Attempting to do so throws `PermissionDenied`.

#### What Deno Workers Can Restrict

| Resource | Restricted? | Details |
|----------|-------------|---------|
| Memory | No | No per-worker memory limit API. Use OS-level cgroups. |
| CPU / Time | No | No built-in timeout. Must implement externally. |
| Filesystem | Yes | Granular per-path permissions. |
| Network | Yes | Granular per-domain permissions. |

#### Level of Isolation

**Separate V8 isolate, same process, with runtime-enforced permissions.** Workers communicate via message passing. Deno recommends additional OS-level sandboxing (chroot, cgroups, seccomp, gVisor/Firecracker) for truly untrusted code.

### Bun Workers

Bun supports Web Workers but has **no permission system** as of 2025. Workers provide thread-level isolation (separate V8 — actually JavaScriptCore — isolate) but no restrictions on fs/net/env access. Similar to Node.js worker_threads in terms of security posture.

### Node.js worker_threads

For comparison: Node.js worker_threads run in separate V8 isolates but have **full access** to all Node.js APIs (fs, net, child_process, etc.). The experimental `--experimental-permission` flag in Node.js 20+ provides some restriction capabilities, but it is not yet mature or widely adopted.

### Compatibility with Node.js worker_threads

Deno and Bun workers are runtime-specific and do not interoperate with Node.js worker_threads. If you need Node.js compatibility, these are not drop-in replacements.

---

## Comparison Summary

| Feature | isolated-vm | vm2 | quickjs-emscripten | ShadowRealm | Deno Workers |
|---------|------------|-----|-------------------|-------------|--------------|
| **Isolation level** | Separate V8 isolate | Same V8 isolate (Proxy wrappers) | WASM sandbox | Same V8 isolate (separate globals) | Separate V8 isolate + permissions |
| **Memory limits** | Yes (soft, 128MB default) | No | Yes (hard, WASM linear memory) | No | No (use OS cgroups) |
| **CPU/timeout** | Yes (timeout + cpuTime tracking) | Partial (timeout only) | Yes (interrupt handler) | No | No (implement externally) |
| **Filesystem restriction** | Yes (nothing exposed by default) | Attempted (bypassed historically) | Yes (no syscall interface) | Partial (no APIs, but not a security guarantee) | Yes (granular per-path) |
| **Network restriction** | Yes (nothing exposed by default) | Attempted (bypassed historically) | Yes (no syscall interface) | Partial (same caveat) | Yes (granular per-domain) |
| **JS execution speed** | V8 JIT (native speed) | V8 JIT (native speed) | QuickJS interpreter (10-50x slower) | V8 JIT (native speed) | V8 JIT (native speed) |
| **Security track record** | Strong (V8-level boundary) | Very poor (20+ CVEs) | Strong (WASM boundary) | N/A (not a security tool) | Good (Rust runtime, permission model) |
| **Maintenance** | Maintenance mode | Resurrected Oct 2025, fragile | Active | Stage 2.7 proposal | Active (Deno team) |
| **Node.js compatible** | Yes (native addon) | Yes | Yes (WASM, runs anywhere) | Experimental flag only | No (Deno-only) |
| **Startup overhead** | Low (bare V8 isolate) | Minimal | Medium (WASM module init) | Minimal | Medium (worker + permission setup) |

### Recommendations by Use Case

**Highest security (untrusted code from unknown sources):**
Use **quickjs-emscripten** inside a **worker_thread** (or Deno worker with `permissions: "none"`), plus OS-level sandboxing (cgroups, seccomp). The WASM boundary is the strongest JS-level isolation available. Accept the performance cost.

**Good security with V8 performance (semi-trusted plugins, user scripts):**
Use **isolated-vm**. You get V8 JIT speed with separate-isolate isolation. Be aware of the maintenance-mode status and the `--no-node-snapshot` requirement on Node 20+.

**Runtime-level sandboxing (controlled environment):**
Use **Deno workers** with granular permissions. Best developer experience for permission management, but requires running on Deno instead of Node.js.

**Global isolation without security needs (plugins, testing):**
**ShadowRealm** when it reaches Stage 3+ and ships unflagged. Until then, isolated-vm or a fresh `vm.Context` may suffice.

**Do not use:**
**vm2** for anything security-sensitive. Its architecture (same-isolate, Proxy-based) is fundamentally unsuited for sandboxing untrusted code.

## Investigation 2: Node.js worker_threads Sandboxing Capabilities (v22)

Research into sandboxing options available through Node.js built-in modules and popular
npm packages, focused on Node.js v22 (tested on v22.22.0).

## Table of Contents

1. [Worker Constructor Options for Sandboxing](#1-worker-constructor-options-for-sandboxing)
2. [resourceLimits](#2-resourcelimits)
3. [Node.js Permission Model](#3-nodejs-permission-model)
4. [vm Module Inside Workers](#4-vm-module-inside-workers)
5. [Combined Defense-in-Depth Approach](#5-combined-defense-in-depth-approach)
6. [Third-Party Sandboxing Packages](#6-third-party-sandboxing-packages)
7. [Recommendations](#7-recommendations)

---

## 1. Worker Constructor Options for Sandboxing

The `new Worker(filename, options)` constructor accepts several options relevant to sandboxing:

| Option | Type | Default | Sandboxing Role |
|--------|------|---------|-----------------|
| `env` | `Object` | `process.env` | Controls which environment variables are visible to the worker |
| `execArgv` | `string[]` | inherited | Passes Node CLI flags, including `--permission` |
| `eval` | `boolean` | `false` | Run inline code string instead of a file |
| `resourceLimits` | `Object` | none | Constrains V8 heap memory and stack size |
| `workerData` | `any` | `undefined` | Structured-clone data passed to worker (controls input) |
| `trackUnmanagedFds` | `boolean` | `true` | Auto-close file descriptors on worker exit |
| `stdin` | `boolean` | `false` | Controls whether worker gets stdin access |
| `stdout` / `stderr` | `boolean` | `false` | Controls output stream routing |

### Environment Isolation

The `env` option provides three modes:

```js
// Mode 1: Custom env (most restrictive) - worker sees ONLY these vars
new Worker('./worker.js', { env: { ALLOWED_VAR: 'value' } });

// Mode 2: Default (copy) - worker gets a snapshot of parent's env
new Worker('./worker.js');  // env defaults to process.env copy

// Mode 3: Shared env (least restrictive) - changes visible both ways
new Worker('./worker.js', { env: SHARE_ENV });
```

**Verified behavior**: With a custom `env` object, the worker cannot see `PATH`, `HOME`,
or any other parent environment variables. This is confirmed in `example-env-isolation.mjs`.

### eval Mode

When `eval: true`, the first argument is treated as JavaScript source code rather than a
filename. This is useful for sandboxing because you can construct the worker code
programmatically:

```js
new Worker(`
  const { parentPort } = require('node:worker_threads');
  // ... sandboxed code here ...
`, { eval: true });
```

---

## 2. resourceLimits

The `resourceLimits` option controls V8 JavaScript engine memory constraints:

```js
new Worker('./worker.js', {
  resourceLimits: {
    maxOldGenerationSizeMb: 64,     // Main (old) heap size limit
    maxYoungGenerationSizeMb: 16,   // Young generation (nursery) size limit
    codeRangeSizeMb: 32,            // Memory reserved for generated code
    stackSizeMb: 4,                 // Call stack size limit (default: 4)
  }
});
```

### What Each Property Controls

- **`maxOldGenerationSizeMb`**: Limits the main V8 heap where long-lived objects reside. This is the most impactful setting for controlling total memory.
- **`maxYoungGenerationSizeMb`**: Limits the "nursery" where newly allocated objects live before being promoted to old generation.
- **`codeRangeSizeMb`**: Limits memory pre-allocated for JIT-compiled machine code.
- **`stackSizeMb`**: Limits the call stack depth. Prevents stack overflow from deep recursion. Default is 4MB.

### What Happens When Limits Are Exceeded

According to the Node.js documentation, exceeding resource limits causes **termination of
the Worker instance**. The worker exits with a non-zero code.

**Important caveats:**
- These limits affect **only the V8 JS engine heap**, not external allocations like `Buffer`, `ArrayBuffer`, or native addon memory.
- Command-line V8 flags can override these: `--max-old-space-size` overrides `maxOldGenerationSizeMb`.
- The limits are described as "guidelines rather than strict limits" -- a determined script could use 2-3x the limit before termination.
- In testing, the termination can be slow. The V8 GC must detect the overrun.

### Reading Resource Limits at Runtime

Inside a worker, you can read the active limits:

```js
const { resourceLimits } = require('node:worker_threads');
console.log(resourceLimits);
// { maxOldGenerationSizeMb: 64, maxYoungGenerationSizeMb: 16, ... }
```

---

## 3. Node.js Permission Model

### Status in Node.js v22: STABLE

As of **Node.js v22.13.0**, the Permission Model is **Stability 2 (Stable)**. It is no
longer experimental. The flag changed from `--experimental-permission` to `--permission`.

### Available Permission Flags

| Flag | What It Restricts |
|------|-------------------|
| `--permission` | Enables the model; blocks ALL capabilities by default |
| `--allow-fs-read[=path]` | File system read operations |
| `--allow-fs-write[=path]` | File system write operations |
| `--allow-child-process` | Spawning child processes |
| `--allow-worker` | Creating worker threads |
| `--allow-addons` | Loading native addons |
| `--allow-wasi` | WASI usage |

### Path-Specific Restrictions

```bash
# Allow reading only from /tmp and a specific config file
node --permission --allow-fs-read=/tmp/ --allow-fs-read=/app/config.json app.js

# Wildcard support
node --permission --allow-fs-read=/app/data* app.js
```

### Can `--permission` Be Passed via `execArgv` to a Worker?

**Yes!** This was experimentally confirmed on Node.js v22.22.0:

```js
const worker = new Worker(code, {
  eval: true,
  execArgv: ['--permission', '--allow-fs-read=/tmp/allowed-only'],
});
```

**Verified results:**
- `process.permission.has('fs.read')` returns `true` (general)
- `process.permission.has('fs.read', '/etc/passwd')` returns `false` (specific path)
- `fs.readFileSync('/etc/passwd')` throws `ERR_ACCESS_DENIED`
- `child_process.execSync('whoami')` throws `ERR_ACCESS_DENIED`

### Runtime Permission Checking

```js
process.permission.has('fs.read');                          // boolean
process.permission.has('fs.read', '/specific/path');        // boolean
process.permission.has('fs.write');                         // boolean
process.permission.has('child');                            // boolean
process.permission.has('worker');                           // boolean
```

### Limitations

1. **Not a security boundary for malicious code**: The docs call it a "seat belt" for trusted code.
2. **Symbolic links**: Symlinks are followed even outside granted paths.
3. **File descriptors**: Using existing FDs via `node:fs` bypasses the model.
4. **Early file access**: `--env-file` and `--openssl-config` read files before permissions initialize.
5. **Does not inherit to child workers**: Workers retain the same permissions as the spawning process unless `execArgv` explicitly sets new permissions.

---

## 4. vm Module Inside Workers

The `vm` module (`vm.createContext`, `vm.Script`, `vm.runInContext`) can be used inside a
worker for an additional layer of global scope isolation.

### What vm Provides

```js
const vm = require('node:vm');

// Create isolated global scope
const sandbox = {
  console: { log: (...args) => parentPort.postMessage(args) },
  result: null,
};
vm.createContext(sandbox);

// Run untrusted code in that scope
vm.runInContext('result = 2 + 2', sandbox, { timeout: 5000 });
```

- **Global scope isolation**: Code cannot access variables from the outer scope.
- **`require` is not available**: Unless explicitly passed in the sandbox.
- **Timeout enforcement**: `{ timeout: ms }` catches infinite loops.
- **Code generation control**: Can disable `eval()` and `Function()` constructor.

### Critical Security Limitation: Prototype Escape

The vm module is **explicitly documented as NOT a security mechanism**:

> "The `node:vm` module is not a security mechanism. Do not use it to run untrusted code."

The reason is the **prototype escape**:

```js
// Inside a vm context, this reaches the real process object:
const proc = this.constructor.constructor("return process")();

// From there, an attacker can:
const fs = proc.mainModule.require('fs');    // Access file system
proc.exit(1);                                // Kill the process
```

**Verified on Node.js v22.22.0**: The prototype escape successfully retrieves the real
`process` object from within a vm context running inside a worker.

### Using vm Safely (Defense in Depth)

The vm module alone is insufficient, but combined with the Permission Model, the prototype
escape is neutered. Even if an attacker gets `process` and `require('fs')`, the Permission
Model blocks actual operations:

```
vm prototype escape -> gets process -> gets fs -> ERR_ACCESS_DENIED
```

---

## 5. Combined Defense-in-Depth Approach

The strongest built-in sandboxing combines all three layers:

```js
import { Worker } from 'node:worker_threads';

function runSandboxed(untrustedCode, options = {}) {
  return new Promise((resolve, reject) => {
    const worker = new Worker(`
      const { parentPort, workerData } = require('node:worker_threads');
      const vm = require('node:vm');

      const sandbox = {
        console: {
          log: (...args) => parentPort.postMessage({ type: 'log', args }),
        },
        result: undefined,
      };
      vm.createContext(sandbox);

      try {
        vm.runInContext(workerData.code, sandbox, {
          timeout: workerData.timeout || 5000,
          filename: 'sandbox.js',
        });
        parentPort.postMessage({ type: 'result', value: sandbox.result });
      } catch (err) {
        parentPort.postMessage({ type: 'error', message: err.message });
      }
    `, {
      eval: true,
      workerData: { code: untrustedCode, timeout: options.timeout },

      // Layer 1: Strip environment variables
      env: {},

      // Layer 2: Permission model (restrict OS access)
      execArgv: [
        '--permission',
        // Add --allow-fs-read=<path> only if needed
      ],

      // Layer 3: Resource limits (prevent memory exhaustion)
      resourceLimits: {
        maxOldGenerationSizeMb: options.maxMemoryMb || 64,
        maxYoungGenerationSizeMb: 16,
        stackSizeMb: options.stackSizeMb || 4,
      },
    });

    const logs = [];
    worker.on('message', (msg) => {
      if (msg.type === 'log') logs.push(msg.args);
      else if (msg.type === 'result') resolve({ result: msg.value, logs });
      else if (msg.type === 'error') reject(new Error(msg.message));
    });
    worker.on('error', reject);
    worker.on('exit', (code) => {
      if (code !== 0) reject(new Error(`Worker exited with code ${code}`));
    });
  });
}
```

### What This Blocks

| Attack Vector | Blocked By |
|---------------|------------|
| `require('fs')` | vm (not in scope) |
| `process.exit()` | vm (not in scope) |
| Prototype escape to `process` | vm fails to prevent, but... |
| ...then `fs.readFileSync()` | Permission Model (`ERR_ACCESS_DENIED`) |
| ...then `child_process.exec()` | Permission Model (`ERR_ACCESS_DENIED`) |
| ...then `new Worker()` | Permission Model (`ERR_ACCESS_DENIED`) |
| Infinite loop | vm timeout |
| Memory exhaustion | resourceLimits |
| Env var leakage | Custom `env: {}` |

### What This Does NOT Block

- **CPU exhaustion within the timeout window**: A tight loop will consume a full CPU core for up to `timeout` ms.
- **External memory allocation**: `ArrayBuffer` and native memory are not covered by `resourceLimits`.
- **Timing side-channel attacks**: No protection.
- **Permission model bypasses via symlinks or file descriptors**: Known limitations.
- **V8 zero-day exploits**: An attacker could escape the V8 engine entirely.

---

## 6. Third-Party Sandboxing Packages

### vm2 -- AVOID

**Status**: Deprecated (July 2023), briefly revived (Oct 2025), new critical CVE in Jan 2026 (CVE-2026-22709, CVSS 9.8).

vm2 wraps `node:vm` with Proxy-based sanitization to intercept prototype access. However, JavaScript's complexity means new bypasses are regularly found. The maintainers themselves recommend against using it.

### isolated-vm -- Recommended for In-Process Isolation

**Package**: [`isolated-vm`](https://www.npmjs.com/package/isolated-vm)

Uses V8's native Isolate interface for real heap isolation within the same process.

```js
const ivm = require('isolated-vm');

// Create an isolate with 128MB memory limit
const isolate = new ivm.Isolate({ memoryLimit: 128 });
const context = await isolate.createContext();

// Inject a callback
const jail = context.global;
await jail.set('log', new ivm.Callback((msg) => console.log(msg)));

// Run untrusted code
const script = await isolate.compileScript('log("hello from isolate"); 1 + 2');
const result = await script.run(context);
```

**Advantages**:
- Separate V8 heap (real memory isolation)
- No shared prototype chain (no prototype escapes)
- Configurable memory limits
- Used in production by Screeps, Fly.io

**Limitations**:
- Native addon (requires C++ compilation)
- In "maintenance mode" per maintainers
- Memory limits are guidelines (~2-3x overshoot possible)
- Must not expose `isolated-vm` objects to untrusted code

### quickjs-emscripten -- Strongest Isolation

**Package**: [`quickjs-emscripten`](https://www.npmjs.com/package/quickjs-emscripten)

Runs QuickJS (a separate JavaScript engine) compiled to WebAssembly. The untrusted code runs in an entirely different engine inside a WASM sandbox.

```js
import { getQuickJS } from 'quickjs-emscripten';

const QuickJS = await getQuickJS();
const vm = QuickJS.newContext();

// Set up a limited API
const logHandle = vm.newFunction('log', (...args) => {
  const nativeArgs = args.map(vm.dump);
  console.log(...nativeArgs);
});
vm.setProp(vm.global, 'log', logHandle);
logHandle.dispose();

// Run untrusted code
const result = vm.evalCode('log("hello"); 2 + 2');
if (result.error) {
  console.log('Error:', vm.dump(result.error));
  result.error.dispose();
} else {
  console.log('Result:', vm.dump(result.value));
  result.value.dispose();
}

vm.dispose();
```

**Advantages**:
- Entirely separate JS engine (V8 bugs don't apply)
- WASM sandbox provides memory isolation
- No prototype chain sharing
- Works in browsers, Node.js, Deno, Cloudflare Workers

**Limitations**:
- Slower than V8 (interpreted, not JIT-compiled)
- Manual memory management (must `.dispose()` handles)
- No access to Node.js APIs by default (must explicitly bridge)

### @sebastianwessel/quickjs -- Higher-Level Wrapper

A TypeScript-friendly wrapper around quickjs-emscripten with built-in support for:
- Virtual file systems
- Fetch client
- Custom module loading
- Built-in test runner

---

## 7. Recommendations

### For running semi-trusted code (e.g., plugins from known developers)

Use the **built-in defense-in-depth approach**:
- Worker thread + `execArgv: ['--permission']` + vm module + `resourceLimits`
- No third-party dependencies needed
- Good performance (V8 JIT compilation)
- See `example-sandbox-combined.mjs`

### For running untrusted code (e.g., user-submitted scripts)

Use **isolated-vm** or **quickjs-emscripten**:
- `isolated-vm` for best performance (real V8 isolate)
- `quickjs-emscripten` for strongest isolation (WASM boundary)
- Combine with worker threads for crash isolation

### For maximum security

Use **process-level or container-level isolation**:
- Spawn sandboxed code in a separate process with restricted permissions
- Use OS-level sandboxing (seccomp, AppArmor, SELinux)
- Use container runtimes (Docker, gVisor)
- The Node.js Permission Model and vm module alone are not sufficient for adversarial code

### Isolation Strength Ranking (weakest to strongest)

1. `vm` module alone -- easily escaped, do not use for security
2. `vm` + Permission Model via worker `execArgv` -- neutered escapes, good for semi-trusted code
3. `isolated-vm` -- real V8 isolate, good for most untrusted code scenarios
4. `quickjs-emscripten` -- WASM boundary, separate engine, strongest in-process isolation
5. Subprocess + OS sandboxing -- true process boundary
6. Container (gVisor/Docker) -- full hardware-level isolation

---

## Files in This Research

| File | Description |
|------|-------------|
| `README.md` | This report |
| `notes.md` | Raw research notes and log |
| `example-resource-limits.mjs` | Demonstrates Worker resourceLimits |
| `example-env-isolation.mjs` | Demonstrates env isolation modes |
| `example-sandbox-combined.mjs` | Complete defense-in-depth sandboxing example |

## Investigation 3: Sandboxing Untrusted Code with Node.js Worker Threads

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
    '--permission',
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
- **Stable since Node.js v22.13.0** — use `--permission` (the older `--experimental-permission` also works)
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
    '--permission',
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
    '--permission',
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
