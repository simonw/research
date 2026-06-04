# JS Sandbox Research Notes

## Research Goal
Investigate npm packages and approaches for running untrusted JavaScript code in a sandbox.

## Packages Researched
1. isolated-vm
2. vm2
3. quickjs-emscripten / quickjs-ng
4. ShadowRealm (TC39 proposal)
5. Web Workers in Deno/Bun

---

## Research Log

### 2026-03-22

**isolated-vm**
- Searched npm and GitHub for current version, API surface, and maintenance status.
- Found it is in "maintenance mode" — new features not added but existing features supported.
- Key finding: uses V8's Isolate interface directly, so code runs at V8-native speed.
- Memory limit default 128MB, minimum 8MB. These are guidelines, not hard limits (2-3x overshoot possible).
- CPU control via timeout (ms) and interrupt handlers. Also exposes cpuTime/wallTime in nanoseconds.
- No filesystem or network access unless explicitly exposed via Reference/ExternalCopy API.
- Requires --no-node-snapshot flag on Node 20+.
- Requires C++ compiler for native addon installation.
- Used by Screeps (MMO game) and Fly.io CDN.

**vm2**
- Deprecated in July 2023 after repeated sandbox escape CVEs.
- Resurrected in October 2025 with plan to rewrite in TypeScript.
- CVE-2026-22709 (CVSS 9.8) discovered January 2026 — sandbox escape via unsanitized Promise callbacks.
- Fixed in v3.10.2/3.10.3 but maintainer acknowledges "new bypasses will likely be discovered."
- Over 20 known sandbox escapes historically. Fundamentally fragile architecture (same-isolate, prototype-chain attacks).
- Still gets 1M+ weekly downloads despite warnings.

**quickjs-emscripten / quickjs-ng**
- quickjs-emscripten compiles QuickJS to WebAssembly via Emscripten.
- Each WASM module is completely isolated — no shared memory, no syscall interface.
- Supports memory limits (setMemoryLimit), CPU interrupts (setInterruptHandler), max stack size.
- No fs/net access by default; must be explicitly provided.
- Sync variant: ~500KB WASM, fast. Async variant: ~1MB, ~40% slower.
- QuickJS-NG is an actively developed fork with newer JS features, community-driven, CMake build.
- quickjs-emscripten supports both original QuickJS and QuickJS-NG as WASM build variants.
- Performance: significantly slower than V8 JIT (QuickJS is an interpreter, no JIT).
- Known issue: OOM can leave WASM module in unrecoverable state.
- @sebastianwessel/quickjs provides higher-level wrapper with virtual FS, fetch client, TypeScript support.

**ShadowRealm**
- TC39 Stage 2.7 as of March 2025.
- Stage 3 requested Dec 2024, but web integration consensus still needed.
- Node.js has --experimental-shadow-realm flag but NOT enabled by default in Node 22.
- ShadowRealm provides same-thread, synchronous execution with separate global/intrinsics.
- NOT a security sandbox — designed for isolation of globals, not untrusted code.
- API: constructor, evaluate(string), importValue(specifier, bindingName).
- Only primitive values and callables cross the boundary.
- Polyfill available: shadowrealm-api npm package.

**Deno/Bun Web Workers**
- Deno: secure-by-default with permission flags (--allow-net, --allow-read, etc).
- Deno Workers inherit parent permissions by default, but can be restricted per-worker.
- Can set permissions: "none" for fully sandboxed worker.
- Workers cannot escalate beyond parent permissions.
- Bun: no permission system as of 2025.
- Node.js: experimental permission model exists but not mature.
- Deno recommends additional OS-level sandboxing (chroot, cgroups, seccomp) for truly untrusted code.
- Deno workers run in separate V8 isolates with message passing (similar to browser Web Workers).

**Performance comparison notes**
- isolated-vm: V8 JIT speed, low startup overhead (bare isolate), data marshalling cost.
- worker_threads: V8 JIT speed, higher startup (full Node.js env), SharedArrayBuffer support.
- quickjs-emscripten: interpreter speed (no JIT), strongest isolation (WASM sandbox), ~500KB module.
- ShadowRealm: V8 JIT speed, same-thread, minimal overhead, but not a security boundary.
- Deno Workers: V8 JIT speed, process-level permission enforcement, message passing overhead.
