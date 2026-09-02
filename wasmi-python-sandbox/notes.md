# Notes: calling wasmi v2.0 from Python

Date: 2026-09-02

## Context

- wasmi 2.0.0 was published to crates.io on 2026-09-01 (yesterday). No `wasmi` package on PyPI.
- Blog post https://wasmi-labs.github.io/blog/posts/wasmi-v2.0/ : ~2.2x faster than 1.0, direct-threaded
  dispatch, accumulator registers, stable fuel metering, deterministic profile, C-API mentioned as the
  non-Rust embedding route.
- Environment: Rust 1.94.1 with wasm32-wasip1 / wasm32-unknown-unknown targets, Python 3.11, clang 18,
  cmake, wasm-ld. No wasi-sdk, no emscripten, no wasmtime.

## Plan

1. Inspect wasmi 2.0 API (fuel, ResourceLimiter, call hooks) and the C API surface.
2. Compare routes to Python: PyO3/maturin extension vs C API + ctypes/cffi vs existing packages.
3. Build a small Python library on the best route with fuel/time + memory limits + host functions.
4. Compile MicroPython and QuickJS to wasm32 and run them in the sandbox.

## Findings while reading wasmi 2.0 (crate + C API)

- Rust API (crates/wasmi): `Config::consume_fuel`, `Store::set_fuel/get_fuel`, `Store::limiter(StoreLimits)` with
  `StoreLimitsBuilder::memory_size/table_elements/instances/trap_on_grow_failure`, `Store::call_hook`, and
  **resumable calls**: `Func::call_resumable` returns `ResumableCall::{Finished, HostTrap, OutOfFuel}` — out of fuel is
  resumable in 2.0 (`ResumableCallOutOfFuel::resume`). This is the key to a wall-clock timeout: hand out fuel in
  slices and check the clock between slices. No epoch interruption (unlike wasmtime), no exception-handling proposal.
- C API (crates/c_api): wasm-c-api `wasm.h` plus a small `wasmi.h`: `wasmi_config_consume_fuel_set`,
  `wasmi_context_set_fuel/get_fuel`, compilation mode. No resource limiter, no resumable calls, no call hooks.
  Shipped header still says `WASMI_VERSION "0.35.0"` (stale). Builds with cmake into libwasmi.so/.a in ~3 min.
- No Python package exists for wasmi. Options: (a) PyO3 extension over the Rust crate, (b) ctypes/cffi over
  libwasmi.so, (c) wasmtime-py (already exists, JIT, bigger), (d) wasmi_cli in a subprocess.

## Route chosen: PyO3 + maturin

- Built `wasmi_sandbox` (Rust crate -> `wasmi_sandbox._core`). Store owns Store+Linker+Instance. Host functions are
  Python callables registered with `define_func(module, name, params, results, fn)`.
- Re-entrancy: while a host function runs, a thread-local stack holds a pointer to wasmi's `Caller`, so Python can
  read/write memory and call back into the guest (`call_indirect`) from inside a host function.
- Python exceptions inside host functions are stashed in the store data and rethrown by the outer call.
- Bug found in first version: my fuel loop spun forever when the remaining budget was below the fuel required by the
  next instruction (`ResumableCallOutOfFuel::required_fuel`). Fixed by comparing budget against `required_fuel`.
- Measured: host round trip (1 wasm call + 2 Python host calls) ~1 µs. A 1M-iteration wasm loop takes 6 ms (13M fuel).

## Guests

- wasi-libc refuses `setjmp.h` without Wasm exception handling. Worked around it for MicroPython with LLVM's
  emscripten-style lowering (`-mllvm -enable-emscripten-sjlj`), a 40-line C runtime (`__wasm_setjmp`,
  `__wasm_setjmp_test`, `emscripten_longjmp`, `setThrew`, stack save/restore via `global.get __stack_pointer`)
  and Python-implemented `invoke_*` imports that call back into the guest via the function table and catch the
  unwind. Needed `.globaltype __stack_pointer, i32` in the inline asm or wasm-ld complains about a symbol type mismatch.
- MicroPython embed port + json module + a `host` C module; GC uses the deferred/split-heap scheme of the official
  webassembly port because Wasm locals are invisible to a conservative stack scan.
- quickjs-ng builds for WASI out of the box with the wasi-sdk cmake toolchain; wrote a 200-line reactor
  (qjs_sandbox.c) exposing qjs_init/qjs_eval and JS globals print/console/host.<fn>().

## wasmi 2.0 quirk: lazy compilation fuel is not resumable

With the default `CompilationMode::LazyTranslation`, wasmi charges fuel for translating a function the first time it
is called (`code_map/mod.rs`: `fuel_for_translating_bytes`). If the store does not have enough fuel at that moment
the engine returns `Err(ErrorKind::ResumableOutOfFuel)` — a plain error, even though the name suggests otherwise — and
the call stack is gone, unlike the ordinary out-of-fuel case which comes back as `ResumableCall::OutOfFuel`.
Reproduced with QuickJS: `qjs_init` under a 250k-fuel slice failed with `required_fuel=61992`; my first "retry from
scratch" workaround re-ran `qjs_init` after it had already created the runtime (status -1). Fix: the Python `Engine`
now defaults to eager compilation, so no fuel is charged for translation at run time. Worth reporting upstream.
Init cost for QuickJS (1.3 MB module): eager 37 ms vs lazy 19 ms.

## MicroPython works

- `print`, comprehensions, exceptions (caught inside the guest and propagated out as PythonError), `host.call()`.
- A trivial `try: 1/0 except:` script costs ~80 `invoke_*` round trips through Python and 3 unwinds.

## Fuel metering granularity (big finding)

QuickJS burned ~4.5 G fuel for `fib(20)` (10 ms) while a plain wasm loop burns ~2 G fuel/s. Per-instruction costs
measured with tiny wat modules are all sane (9-14 fuel per loop iteration incl. call/call_indirect/br_table/loads).
The explanation is **where wasmi charges fuel**: it is not per executed basic block. A loop whose body contains a
`br_table` with a never-taken case of 100k instructions costs ~400k fuel per iteration in wasmi 2.0 (and ~105k in
wasmi 1.1 - reproduced with a small Rust program, `fuelcmp/`). Code inside a never-taken `if`, a never-entered
`block`, statically unreachable code after a `br`, and even the body of a nested inner loop that is never entered
are all charged; code placed after the loop (function frame) is not. So a loop iteration is charged for the whole
static body of the loop, nested loops included (verified with wasmi's Rust API directly, not just via my bindings). QuickJS's bytecode interpreter is one giant switch inside one loop, so every
JS bytecode dispatch pays for the entire interpreter body (~35k fuel per bytecode).

Consequences: fuel is still a deterministic, monotonic upper bound on work (good enough as a *limit*), but its rate
in fuel/second varies by 100x+ between modules, so (1) budgets must be calibrated per guest, and (2) fixed-size fuel
slices are a poor timer: I made slicing adaptive (double/halve the slice to keep each slice at ~1-4 ms wall time).

wasmi 1.1 vs 2.0 fuel per iteration for the dead-case test (1000 iterations):

| dead instructions in never-taken case | wasmi 1.1 | wasmi 2.0 |
|---|---|---|
| 10 | 30 | 98 |
| 1,000 | 1,070 | 4,110 |
| 100,000 | 105,020 | 405,357 |

## Native invoke trampolines + host stack guard

- Moving the emscripten-style `invoke_*` trampolines from Python into Rust (`Store.define_invoke`) made MicroPython
  fib(20) go from 334 ms to 71 ms and a 100k-iteration loop from 1.6 s to 0.38 s (964k invokes).
- New hazard: guest recursion re-enters wasmi once per invoke, so unbounded recursion overflowed the *host* native
  stack (segfault) before MicroPython's own 1 MB shadow-stack check fired. Depth 1600 was fine (8 invokes per Python
  call level); unbounded recursion crashed. Fix: `stacker::remaining_stack()` guard in the trampoline - when less than
  768 KB of native stack remains, the trampoline calls the guest's `mp_sandbox_recursion_error` export instead (which
  raises a catchable RecursionError via longjmp) or traps if no such export exists.

## Stack limits and deep recursion

- wasmi defaults: max recursion depth 1000 wasm frames, value stack 1 MiB (`Config::set_max_recursion_depth`,
  `set_max_stack_height`). Exposed on `Engine(max_recursion_depth=, max_stack_height=)`.
- QuickJS: with the wasmi limits relaxed, unbounded JS recursion hit `MemoryOutOfBounds` instead of a JS
  RangeError, because quickjs-ng deliberately disables its stack check on WASI
  (`update_stack_limit`: `#if defined(__wasi__) rt->stack_limit = 0`). `__builtin_frame_address(0)` does return the
  shadow stack pointer on wasm32, so a 6-line patch (guests/quickjs/quickjs-ng.patch) re-enables the check:
  unbounded recursion now gives a catchable `RangeError: Maximum call stack size exceeded` at depth ~1636 with a
  512 KB JS stack limit (203 with 64 KB), and the runtime stays usable.
- Both guests are now linked with `-Wl,--stack-first` so a shadow-stack overflow traps (address wraps below 0)
  instead of silently corrupting static data.

## Benchmarks (final, benchmarks/bench.py)

wasmi 2.0 (interpreter, fuel on): 10M-iteration wasm loop 48 ms (36 ms fuel off); 1M guest->Python->guest host
calls 177 ms; QuickJS compile+init 34 ms; QuickJS fib(27) 292 ms (297 ms with a timeout, adaptive slicing);
20k host.add() from JS 1.46 s; MicroPython init 10 ms, fib(20) 74 ms, 100k-iteration loop 366 ms (964k native
invoke trampolines). wasmtime-py: loop 4.6 ms (7.8 ms fuel on); 1M host calls 10.4 s; QuickJS compile 348 ms,
fib(27) 36 ms. C API via ctypes: 9.8 us per host round trip vs 1.0 us via PyO3.

## Final state

- `wasmi_sandbox` builds with maturin (Rust 1.94, pyo3 0.29, wasmi 2.0.0, wat, stacker), 30 pytest tests pass.
- Prebuilt guests are shipped as package data (micropython.wasm 406 KB, quickjs.wasm 1.3 MB); the build recipes
  (`guests/`) need wasi-sdk 27 plus clones of quickjs-ng and micropython, which are not included. The one change
  made to an upstream repo (quickjs-ng stack check on WASI) is saved as `guests/quickjs/quickjs-ng.patch`.
- Things I would do next: report the two wasmi fuel issues upstream; release the GIL during guest execution;
  cache compiled modules across sandboxes (wasmi `Module` is `Send + Sync`, so one eager-compiled QuickJS module
  could serve many stores); a Rust-side WASI instead of the Python one for guests that print a lot.
