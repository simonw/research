# Research Notes: Sandboxing Untrusted Code with Node.js Worker Threads

## Environment
- Node.js v22.22.0
- Linux 6.18.5
- Date: 2026-03-22

## Key Questions
1. Can `worker_threads` + `resourceLimits` limit memory?
2. Can the Node.js Permission Model restrict filesystem/network access for workers?
3. Can CPU be limited?
4. Can `vm` module provide code isolation inside workers?
5. Can `isolated-vm` provide better isolation?
6. Can these be combined?

---

## Experiment 1: `resourceLimits` (01-resource-limits.mjs through 01f)

**Finding: `resourceLimits` does NOT effectively enforce memory limits in Node.js 22.**

Tested with:
- `maxOldGenerationSizeMb: 16` with ArrayBuffer allocations → worker allocated 996MB
- `maxOldGenerationSizeMb: 16` with heap objects (`'x'.repeat(1000)`) → exceeded limit massively
- `maxOldGenerationSizeMb: 32` with unique random strings → exceeded 117MB before timeout
- `maxOldGenerationSizeMb: 48` with objects + `process.memoryUsage()` → reached 200MB+
- `maxOldGenerationSizeMb: 4` (extreme) → reached 88MB+

ArrayBuffers are allocated outside V8 heap, so `resourceLimits` never applies to them.
Even V8 heap objects exceeded the stated limits by 10-20x without triggering OOM.

V8 appears to use near-heap-limit callbacks to extend the limit dynamically.
The limits appear to be "advisory" rather than hard enforcement in Node.js 22.

**`--max-old-space-size` cannot be passed via `execArgv`** — Node.js rejects it with
`ERR_WORKER_INVALID_EXEC_ARGV`.

## Experiment 2: Permission Model (02-permission-model.mjs, 02b)

**Finding: `--experimental-permission` via `execArgv` WORKS for workers.**

With flags: `--experimental-permission --allow-fs-read=*`:
- fsRead: ALLOWED ✓
- fsWrite: BLOCKED (ERR_ACCESS_DENIED) ✓
- childProcess: BLOCKED (ERR_ACCESS_DENIED) ✓

Without flags (control):
- fsRead: ALLOWED
- fsWrite: ALLOWED
- childProcess: ALLOWED

Available permission flags for Node.js 22:
- `--allow-fs-read=<path>` — restrict file reads
- `--allow-fs-write=<path>` — restrict file writes
- `--allow-child-process` — restrict spawn/exec
- `--allow-worker` — restrict nested worker creation
- `--allow-addons` — restrict native addon loading (blocked by default!)
- Network (`--allow-net`) was not available as a flag in testing

**Key detail: `--allow-addons` is required to load native addons (like `isolated-vm`)
and Node.js explicitly warns this "could invalidate the permission model".**

## Experiment 3: `vm` module isolation (03-eval-worker.mjs, 03b)

**Finding: `vm.createContext` is NOT a security boundary — sandbox escapes are easy.**

Standard escape via `this.constructor.constructor('return this')()`:
- With regular object sandbox → ESCAPED, got access to `process`
- With `Object.create(null)` sandbox → prevented `this.constructor` but escaped via `console.log.constructor`

Any function injected into the sandbox (even `console.log`) can be used to escape:
```js
const g = console.log.constructor('return this')();
// g now has access to the real global scope
```

Node.js docs explicitly state: "The vm module is not a security mechanism. Do not use it to run untrusted code."

**However: vm + permission model = defense in depth (see Experiment 6).**

## Experiment 4: CPU Monopolization (04-cpu-timeout.mjs)

**Finding: Multiple effective CPU timeout mechanisms exist.**

1. `worker.terminate()` — kills worker stuck in infinite loop (returns exit code 1). Reliable.
2. `vm.runInContext(code, ctx, { timeout: 1000 })` — throws timeout error for CPU-bound code. Reliable.
3. Message-based cooperative shutdown — works for well-behaved code only.

`worker.terminate()` is the hard kill switch. It works even on infinite loops.
`vm` timeout works for code running in a vm context.

## Experiment 5: `isolated-vm` (06-isolated-vm.mjs)

**Finding: `isolated-vm` provides the strongest sandbox for JavaScript execution.**

- Separate V8 isolate (not just a separate context)
- No access to process, require, fetch, or any Node.js APIs
- `this.constructor.constructor('return this')()` does NOT escape — the returned global has no `process`
- Memory limit: **hard-enforced** — isolate disposed when exceeding 16MB limit
- CPU timeout: works perfectly on infinite loops
- Works inside worker threads
- Requires native addon (`.node` file)

API is clean and well-designed:
```js
const isolate = new ivm.Isolate({ memoryLimit: 32 }); // hard 32MB limit
const context = await isolate.createContext();
await context.global.set('log', new ivm.Callback(fn));
const script = await isolate.compileScript(code);
const result = await script.run(context, { timeout: 1000 });
isolate.dispose();
```

## Experiment 6: Combined Approaches (07c through 07f)

### Approach A: Permission Model + isolated-vm (07c)
- Requires `--allow-addons` which Node.js warns about
- Both isolated-vm isolation AND permission model restrictions work simultaneously
- fsWrite and exec still blocked by permission model
- Code in isolate has no access to any APIs

### Approach B: Permission Model + vm module (07d, 07e, 07f)
- No native addons needed (pure Node.js)
- vm escape IS possible (via any injected function's constructor)
- BUT permission model still blocks dangerous ops after escape
- Tested: escaped vm, got `require` and `process`, tried `fs.writeFileSync` → **BLOCKED by permission model**
- `process.exit()` still works (terminates worker, not main process)
- `process.env` accessible after escape (information leak)

### Approach C: Worker + isolated-vm (no permission model)
- Clean approach, isolated-vm handles code isolation
- Worker thread provides resource management (terminate, etc.)
- No filesystem/network restrictions from worker itself — rely entirely on isolated-vm's API surface control

## Key Findings Summary

1. **`resourceLimits` is broken/unreliable in Node.js 22** — doesn't enforce stated limits
2. **Permission Model works via `execArgv`** — effective for fs/child_process/worker restrictions
3. **`vm` module is NOT a security boundary** — trivially escapable
4. **`isolated-vm` provides real isolation** with enforced memory limits and timeouts
5. **Permission Model blocks native addons by default** — `isolated-vm` requires `--allow-addons`
6. **Defense in depth works**: even after vm escape, permission model blocks dangerous ops
7. **Worker.terminate() reliably kills runaway workers** including infinite loops
8. **No built-in network restriction** in Node.js 22 permission model (no `--allow-net` flag working)
9. **`process.exit()` works even in restricted contexts** — but only kills the worker, not the main process
