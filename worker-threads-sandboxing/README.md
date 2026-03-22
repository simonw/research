# Node.js worker_threads Sandboxing Capabilities (v22)

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

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
