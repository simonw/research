# JavaScript Sandbox Solutions for Running Untrusted Code

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

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
