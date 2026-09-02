# Calling wasmi 2.0 from Python: a sandbox for untrusted JavaScript and Python

[wasmi 2.0](https://wasmi-labs.github.io/blog/posts/wasmi-v2.0/) shipped to crates.io on
2026-09-01 (the day before this investigation). It is a pure-Rust WebAssembly *interpreter*:
no JIT, small binary, deterministic fuel metering, ~2.2x faster than wasmi 1.0 and in the same
league as wasm3. There is no Python package for it yet. This report explores the options for
driving it from Python, builds a working library (`wasmi_sandbox`), and uses it to run untrusted
JavaScript (quickjs-ng) and untrusted Python (MicroPython) with CPU, wall-clock and memory limits
and with Python functions exposed to the sandboxed code.

Everything here was built and measured in one session on a 4-core Linux container with
Rust 1.94, Python 3.11, wasi-sdk 27 and clang 20.

## TL;DR

- **Best route: a PyO3 extension over the Rust crate.** wasmi's interesting features (resource
  limiter for memory, resumable out-of-fuel calls, re-entrant host calls) are Rust-only. The C API
  exposes only the standard `wasm.h` plus fuel get/set, so ctypes cannot do memory limits or
  wall-clock timeouts. `wasmtime-py` already exists and is a fine choice if you want a JIT.
- **wasmi 2.0 makes wall-clock timeouts easy without threads or signals**: an out-of-fuel call is
  *resumable*, so the library hands out fuel in slices and checks the clock between slices.
- **Two wasmi quirks found**: (1) fuel is charged for the *entire static body of a loop* on every
  iteration, including never-taken branches and never-entered nested loops. A switch-based
  interpreter like QuickJS therefore burns ~35,000 fuel per JS bytecode. wasmi 1.1 does the same,
  at a quarter of the rate. (2) With lazy compilation, running out of fuel while translating a
  function mid-execution is reported as a non-resumable error. The library defaults to eager
  compilation because of this.
- **MicroPython on wasmi needed a setjmp/longjmp workaround**: wasmi has no Wasm
  exception-handling, and wasi-libc refuses `setjmp.h` without it. I compiled MicroPython with
  LLVM's emscripten-style lowering and implemented the `invoke_*` trampolines on the host, first in
  Python (works, slow) and then in Rust (4-5x faster). Guest recursion then re-enters the
  interpreter per call, which can overflow the *host* stack; a `stacker`-based guard turns that
  into a catchable `RecursionError` inside the guest.
- **QuickJS was easy**: quickjs-ng builds for WASI out of the box; a 200-line C reactor exposes
  `eval`, `print`/`console.log` and a `host.<fn>()` proxy for calling Python.

## Options for calling wasmi from Python

| Route | Fuel limit | Memory limit | Wall-clock timeout | Host functions | Host->guest round trip | Notes |
|---|---|---|---|---|---|---|
| **PyO3 extension over the `wasmi` crate** (this repo) | yes | yes (`StoreLimits`) | yes (fuel slicing + resume) | Python callables, re-entrant | ~1 µs | Needs a Rust toolchain to build a wheel; no runtime deps |
| **C API (`libwasmi.so`) + ctypes** (`capi_ctypes/`) | yes (`wasmi_context_set_fuel`) | no (no limiter in C API) | no (no resumable calls) | `wasm_func_new_with_env` + ctypes callback | ~10 µs | Pure Python once the .so is built with cmake; wasm.h `own`/vec conventions are fiddly; a wrong NUL terminator panics the library |
| cffi over the C API | same as ctypes | no | no | yes | ~ctypes | Same limitations; nicer typing |
| `wasmtime-py` (existing, Cranelift JIT) | yes | yes | yes (epoch interruption) | yes | ~10 µs measured here | 10x faster code execution, 10x slower module compile, larger install; already exists |
| `wasmi_cli` in a subprocess | `--fuel` | via `ulimit -v` | `subprocess` timeout | no (stdio only) | n/a | Coarse but the strongest isolation |

The C API also lacks any way to instantiate a module against a `wasmi_store_t` (the type that
carries fuel), so fuel and host functions currently cannot be combined from C without the
`wasmi_*` extern API that wasmtime's C API has. The shipped `wasmi.h` still says
`WASMI_VERSION "0.35.0"`.

## The library: `wasmi_sandbox`

`wasmi_sandbox/` is a maturin project: a Rust crate (`src/lib.rs`, ~900 lines) that becomes
`wasmi_sandbox._core`, plus a Python layer:

- `_core.Engine / Module / Store`: thin wrappers. `Store` owns the wasmi store, linker and one
  instance. `Store.define_func(module, name, params, results, callable)` registers a Python host
  function; `Store.call(name, args, timeout=)` runs an export; `memory_read/write`, `global_get/set`,
  `call_indirect` and `fuel` / `fuel_consumed` round it out.
- `Sandbox`: loads a module, resolves imports from a dict or a resolver function, provides a
  small **WASI-lite in Python** (stdio captured in memory, clocks, randomness, args/env; every
  filesystem or network call answers `ENOTCAPABLE`/`ENOSYS`), calls `_initialize` for reactors, and
  has `alloc()`/`free()` helpers using the guest's `malloc`.
- `QuickJS` and `MicroPython`: guest-specific wrappers (`eval`, `exec`, `register`, `@function`).

### How the limits work

- **Fuel**: `Config::consume_fuel(true)`; the store keeps a *budget* and hands it to wasmi per
  call. `OutOfFuel` is raised when the budget is below what the next instruction needs
  (`ResumableCallOutOfFuel::required_fuel`). My first version looped forever here because it
  compared the budget with zero instead of with the required amount.
- **Wall clock**: when a `timeout` is given, `run_call` grants a fuel slice, calls
  `Func::call_resumable`, and on `ResumableCall::OutOfFuel` checks the deadline and resumes.
  Slices are sized adaptively to ~1-4 ms of wall time because fuel rates differ 100x between
  modules. Host functions also check the deadline on entry. Overhead of slicing on QuickJS
  `fib(27)`: 292 ms -> 297 ms.
- **Memory**: `StoreLimitsBuilder::memory_size(max)` installed with `Store::limiter`. Growth
  beyond the cap makes `memory.grow` return -1, so `malloc` fails and the guest sees
  `MemoryError` / `InternalError: out of memory`. `trap_on_grow_failure=True` traps instead.
- **Host functions**: `Linker::func_new` with a closure that acquires the GIL, converts `Val`s,
  calls the Python callable and converts the result back. A Python exception is stashed in the
  store data and rethrown by the outermost `call`, so `raise Boom()` inside a host function
  surfaces as `Boom` in the caller.
- **Re-entrancy**: while a host function runs, a thread-local stack holds a pointer to wasmi's
  `Caller`. Python code inside the callback can read memory or `call_indirect` back into the guest
  through that context instead of the (mutably borrowed) store. This is what makes the
  setjmp/longjmp emulation possible from Python.
- **Native stack guard**: nested guest calls consume host stack. `stacker::remaining_stack()` is
  checked before every re-entry; below 768 KB the trampoline calls the guest's overflow export
  (MicroPython: `mp_sandbox_recursion_error`, which raises a catchable `RecursionError`) or traps.
- **wasm stack limits**: wasmi traps at 1000 nested wasm frames / 1 MiB of value stack by default
  (`Engine(max_recursion_depth=, max_stack_height=)`). The guest wrappers raise these so that the
  guests' own, catchable checks fire first.

### Quick tour

```python
import wasmi_sandbox as ws

js = ws.QuickJS(max_memory=32 << 20, timeout=2.0)

@js.function
def lookup_user(user_id):
    return {"id": user_id, "name": f"user-{user_id}"}

js.eval("const u = host.lookup_user(1); console.log(u.name); u.id * 2")   # -> 2
js.eval("while (true) {}")                    # raises wasmi_sandbox.Timeout after 2 s
js.eval("let a=[]; for(;;) a.push('x'.repeat(65536))")  # raises JSError: InternalError: out of memory
js.eval("host.lookup_user(7)", fuel=10**10)   # deterministic CPU budget for this eval

mp = ws.MicroPython(max_memory=32 << 20, timeout=5.0)
mp.register("get_prices", lambda *symbols: {s: 1.0 for s in symbols})
print(mp.exec("import host\nprint(host.call('get_prices', 'AAPL'))"))   # {'AAPL': 1.0}
mp.exec("def f():\n    return f() + 1\nf()")   # PythonError: RuntimeError: maximum recursion depth exceeded
```

Output of `examples/untrusted_js.py`:

```
[1, 4, 9, True, 'number']
stdout: 'hello from JS, user is {"id":1,"name":"user-1","admin":true}\n'
infinite loop    -> OutOfFuel: fuel budget exhausted after 50009539460 units
                    (0.06s, fuel used 49,999,981,745, wasm memory 1 MB)
memory bomb      -> JSError: InternalError: out of memory
                    (1.07s, fuel used 3,647,169,816, wasm memory 32 MB)
deep recursion   -> JSError: RangeError: Maximum call stack size exceeded
                    (0.00s, fuel used 64,750,183, wasm memory 32 MB)
no filesystem    -> undefined undefined undefined
```

Output of `examples/untrusted_python.py`:

```
prices: {'GOOG': 42.0, 'AAPL': 42.0} total: 84.0
caught: insufficient funds
[0, 1, 2, 3, 4, 5, 6]

infinite loop    -> OutOfFuel: fuel budget exhausted after 20009529149 units
                    (0.10s, fuel used 20,000,000,000, wasm memory 1 MB, invokes 426)
memory bomb      -> PythonError: MemoryError: memory allocation failed, allocating 65537 bytes
                    (1.15s, fuel used 4,792,279,995, wasm memory 17 MB, invokes 1,428)
deep recursion   -> PythonError: RuntimeError: maximum recursion depth exceeded
                    (0.07s, fuel used 303,088,847, wasm memory 17 MB, invokes 17,943)
filesystem       -> PythonError: OSError: 1
imports          -> PythonError: ImportError: no module named 'os'
```

## Guest 1: QuickJS (quickjs-ng)

`guests/quickjs/build.sh` builds `libqjs.a` with quickjs-ng's own CMake and the wasi-sdk
toolchain file, then links `qjs_sandbox.c` (a reactor, `-mexec-model=reactor`) into a 1.3 MB
`quickjs.wasm`. The reactor imports four functions from module `env`, all implemented in Python:

- `host_write(fd, ptr, len)`: fd 1/2 are stdout/stderr; fd 3 carries the JSON of the completion
  value and fd 4 the exception text, so the host never has to parse guest memory itself.
- `host_call(name, name_len, json, json_len) -> len` and `host_take(buf, len)`: the guest passes
  a function name and JSON-encoded arguments, the host answers with the length of a JSON result
  (negative for an error message), and the guest mallocs a buffer and fetches it. Inside JS,
  `globalThis.host` is a `Proxy`, so `host.anything(1, 2)` becomes
  `__host_call("anything", "[1,2]")`. Python exceptions become JS exceptions the guest can catch.
- `host_interrupt() -> i32`: wired to `JS_SetInterruptHandler`, giving an optional *soft* timeout
  that raises `InternalError: interrupted` inside JS and leaves the runtime consistent. The hard
  limits (fuel, timeout, memory) are enforced by wasmi regardless of what the JS does.

Three things bit me: `JS_Eval` needs a NUL-terminated buffer (the first run "saw" the previous
eval's bytes: `SyntaxError: unexpected token '}'`); the quickjs-ng `std`/`os` modules are simply
not linked, so `typeof std` is `undefined` and there is no file or network surface at all; and
quickjs-ng deliberately disables its JS stack-depth check on WASI, so unbounded recursion ran the
shadow stack into an out-of-bounds trap instead of a JS `RangeError`. `__builtin_frame_address(0)`
does return the shadow stack pointer on wasm32, so a 6-line patch (`guests/quickjs/quickjs-ng.patch`)
re-enables the check: recursion now stops at depth ~1600 with a catchable
`RangeError: Maximum call stack size exceeded` and the runtime stays usable. Both guests are linked
with `--stack-first` so a real shadow-stack overflow traps instead of corrupting static data.

## Guest 2: MicroPython, and the setjmp problem

MicroPython uses `setjmp`/`longjmp` (its NLR mechanism) for every Python exception. wasmi 2.0
does not implement the Wasm exception-handling proposal, and wasi-sdk's `setjmp.h` is an `#error`
without it. The official MicroPython `webassembly` port uses Emscripten's JavaScript-based
`invoke_*` scheme. I reproduced that scheme with a plain wasi-sdk toolchain and a Python/Rust host:

1. Compile with `-mllvm -enable-emscripten-sjlj` and a private `setjmp.h`. LLVM then rewrites every
   call inside a setjmp-calling function into an imported `invoke_<sig>(fnptr, args...)`, and
   `longjmp` into `emscripten_longjmp`.
2. `sjlj_runtime.c` (60 lines) provides `__wasm_setjmp`, `__wasm_setjmp_test`,
   `emscripten_longjmp` (records target + value, then calls the imported
   `_emscripten_throw_longjmp`), `setThrew`, and shadow-stack save/restore via inline
   `global.get __stack_pointer` (needs a `.globaltype __stack_pointer, i32` directive or wasm-ld
   reports a symbol type mismatch).
3. The host implements `_emscripten_throw_longjmp` by raising an error that unwinds the nested
   wasmi call, and `invoke_*` by saving `__stack_pointer`, calling `table[fnptr]`, and on unwind
   restoring the stack pointer and calling `setThrew(1, 0)` before returning normally. The
   generated code then consults `__THREW__` and `__wasm_setjmp_test` to jump to the right
   `setjmp` landing pad.

The Python implementation of step 3 (`EmscriptenSjLj(native=False)`, ~30 lines) worked on the
first run: `try: 1/0 except ZeroDivisionError` inside the guest cost 79 trampolines and 3 unwinds.
But a 100k-iteration Python loop made 964k trampoline calls and took 1.6 s, so the same logic now
lives in Rust (`Store.define_invoke` / `define_longjmp_thrower`): 0.37 s for the same loop,
MicroPython `fib(20)` 334 ms -> 71 ms.

Other MicroPython build notes (`guests/micropython/`):

- Built from the `embed` port plus `extmod/modjson.c` and a small `host` C module
  (`host.call(name, *args)` marshals through JSON exactly like the QuickJS guest).
- GC: Wasm locals are invisible to MicroPython's conservative stack scanner, so like the official
  webassembly port the guest uses `MICROPY_GC_SPLIT_HEAP_AUTO`: the heap grows on demand (bounded
  by the wasm memory cap) and real collections run only at the top level after each `exec`.
- `MICROPY_STACK_CHECK` against the 1 MB shadow stack plus the host stack guard give a catchable
  `RuntimeError: maximum recursion depth exceeded`.
- 406 KB `micropython.wasm`; `print`, exceptions, classes, comprehensions, `json` all work.

## Measurements

All numbers from `benchmarks/bench.py` on this container (4 cores, wasmi interpreter with fuel
metering on unless noted; wasmtime-py 48 with Cranelift).

| Workload | wasmi 2.0 (this library) | wasmtime-py |
|---|---|---|
| raw wasm loop, 10M iterations | 48 ms (36 ms with fuel off) | 4.6 ms (7.8 ms with fuel) |
| same, with a 10 s timeout (fuel slicing) | 48 ms | n/a |
| guest->Python->guest host call, 1M iterations | 177 ms (0.18 µs/call) | 10.4 s (10 µs/call) |
| QuickJS: compile 1.3 MB module + init | 34 ms | 348 ms compile |
| QuickJS `fib(27)` | 292 ms | 36 ms |
| QuickJS 20k `host.add()` calls (Proxy + JSON both sides) | 1.46 s (73 µs/call) | - |
| MicroPython init | 10 ms | - |
| MicroPython `fib(20)` | 74 ms (Python trampolines: 334 ms) | - |
| MicroPython 100k-iteration loop (964k trampolines) | 366 ms (Python trampolines: 1.6 s) | - |
| C API via ctypes, host round trip | 9.8 µs/call | - |

Interpretation: wasmi runs code ~8-10x slower than Cranelift but starts ~10x faster and, through
PyO3, host calls are ~50x cheaper than wasmtime-py's ctypes-based ones. For "run a short untrusted
script with a few host callbacks" the interpreter is the better fit; for long number crunching the
JIT wins.

### Fuel is not an instruction count

Measured with `fuelcmp/` (a Rust program using both crates directly) and with the Python bindings:
a 1000-iteration loop containing a `br_table` whose never-taken case holds N instructions:

| N (dead instructions) | wasmi 1.1 fuel/iteration | wasmi 2.0 fuel/iteration |
|---|---|---|
| 10 | 30 | 98 |
| 1,000 | 1,070 | 4,110 |
| 100,000 | 105,020 | 405,357 |

Code inside a never-taken `if`, a never-entered `block`, statically unreachable code after `br`,
and the body of a never-entered nested loop are all charged per iteration of the enclosing loop;
code after the loop is not. Per-instruction costs are otherwise sane (calls, `call_indirect`,
`br_table`, loads/stores, `memory.copy` per 64 bytes all cost 1-4 units). Fuel remains a valid
deterministic *limit* (monotonic, cannot be gamed downwards), but budgets must be calibrated per
guest: QuickJS needs ~5x10^10 fuel for a second of work, a tight wasm loop ~2x10^9.

## What is in this folder

```
wasmi_sandbox/            maturin project: Rust crate + python/wasmi_sandbox package
  src/lib.rs              PyO3 bindings (fuel slicing, limiter, host functions, re-entrancy, sjlj)
  python/wasmi_sandbox/   __init__, sandbox.py (Sandbox + WASI-lite), quickjs.py, micropython.py
  python/wasmi_sandbox/guests/*.wasm   prebuilt guests (406 KB + 1.3 MB)
guests/quickjs/           qjs_sandbox.c reactor, build.sh, quickjs-ng.patch (stack check on WASI)
guests/micropython/       main.c, modhost.c, sjlj_runtime.c, include/setjmp.h, mpconfigport.h, Makefile
capi_ctypes/              wasmi C API from ctypes: wasmi_capi.py + demo.py
fuelcmp/                  wasmi 1.1 vs 2.0 fuel accounting comparison (Rust)
benchmarks/bench.py       numbers above
examples/                 untrusted_js.py, untrusted_python.py
tests/                    30 pytest tests (bindings, limits, both guests)
tools/wasm_sections.py    tiny import/export lister for .wasm files
notes.md                  running log of what was tried
```

Build and test:

```bash
pip install maturin pytest
cd wasmi_sandbox && maturin build --release && pip install target/wheels/*.whl
pytest ../tests
# guests (need wasi-sdk + clones of quickjs-ng and micropython):
QUICKJS_NG=... WASI_SDK=... guests/quickjs/build.sh
make -C guests/micropython MICROPYTHON_TOP=... WASI_SDK=...
```

## Caveats and open questions

- After a hard limit fires (`Timeout`, `OutOfFuel`, `Trap`) the guest was interrupted mid-flight;
  its interpreter state may be inconsistent. The QuickJS runtime survived every such test here, but
  the safe pattern is to discard the sandbox object. Use QuickJS's `soft_timeout` when you need
  the runtime to stay consistent.
- MicroPython's deferred GC means a single `exec` that allocates without pause will hit the memory
  cap before anything is collected. Bump `max_memory` or split work into several `exec` calls.
- Host functions run with the GIL held for the whole guest call; nothing here releases it.
- wasmi 2.0's fuel-for-never-executed-code behaviour and the non-resumable lazy-compilation fuel
  error both look worth reporting upstream.
- Not done: `v128`/reference-type values in host function signatures, multiple instances per
  store, and a GitHub Pages demo (this is a Python-side library, so there is no browser demo).
