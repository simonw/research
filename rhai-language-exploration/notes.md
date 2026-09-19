# Rhai exploration — working notes

Task: clone https://github.com/rhaiscript/rhai, exercise language features
(pelican-themed example), test runaway-script/RAM/CPU protections, and build
a Python library embedding Rhai.

## Setup

- Cloned rhai (depth 1) to /tmp/rhai
- Toolchain: cargo/rustc 1.94.1, Python 3.11.15

## Part 1 — Pelican language tour (pelican-rhai/)

`scripts/pelican_tour.rhai` (16 sections) + `src/main.rs` host that registers a
custom `Pelican` Rust type, a custom `outfishes` operator, and a Rhai-defined
`colony` module.

Syntax gotchas hit while writing it:
- `call` is a **reserved keyword** — `let call = "SQUAWK"` fails with
  `ErrorParsing(Reserved("call"))`. Renamed to `cry`.
- `switch` guards attach to literal/range cases, not to a binding:
  `n if n > 15 => ...` is a parse error; `16..=1000 if x % 2 == 1 => ...` works.
- `array.sort()` sorts **in place and returns unit** — `let s = a.map(..).sort()`
  silently yields `()`. Same for `map.keys().sort()`.
- `"!".pad(3, '!')` mutates in place too, so it can't be used inline in an
  interpolation.
- Closures stored in a map are called method-style (`percy.gulp(3)`) to bind
  `this`; `percy.gulp.call(3)` does NOT bind `this`.
- To bind `this` to an arbitrary map from a fn pointer: `some_map.call(fn_ptr)`.
- `engine.call_fn` re-evaluates the whole AST body by default; use
  `CallFnOptions::new().eval_ast(false)` or the script runs twice.

## Part 2 — Safety harness (pelican-rhai/src/bin/safety.rs)

All limits work as advertised, EXCEPT one real gap (below). Results in
`safety_output.txt`.

| limit | hostile script | outcome |
|---|---|---|
| `set_max_operations(1M)` | `loop { dives += 1 }` | stopped in ~30ms |
| `on_progress` + clock | `loop { flaps += 1 }` | stopped in 250.0ms, +8 KiB |
| `set_max_array_size(10k)` | push 2M items | stopped in ~46ms, +164 KiB |
| `set_max_string_size(100k)` | `s += s` x40 | stopped in ~200µs |
| `set_max_map_size(5k)` | 1M index-assigns | **NOT stopped during the loop** |
| `set_max_call_levels(64)` | infinite recursion | stopped in ~270µs |
| `set_max_expr_depths(32,32)` | 500-deep nesting | rejected at parse, ~10µs |

Unbounded baseline for contrast: 2M-element array with no limits = 866ms and
+31 MB peak RSS, i.e. an unsandboxed engine really will eat the host's RAM.

### Finding: `max_map_size` is not enforced on index assignment

`mapgap.rs` isolates it. Growing a map with `m[key] = value` never re-checks the
size of the enclosing map. The check at `src/eval/chaining.rs:790`
(`check_data_size(item_ptr.as_ref(), ...)`) inspects the *inserted item*, not
the container. Meanwhile property assignment `m.k = v` hits a different branch
(`chaining.rs:925`, `check_data_size(target.source(), ...)`) which *does* check
the whole map.

Consequences:
- A) `let m = #{}; for i in 0..200_000 { m[`k_${i}`] = i; } m` with
  `max_map_size(1000)` returns a **200,000-entry map**, +29 MB peak RSS.
- B) Adding one `m.tick = i` to the same loop stops it in 4ms — proving the
  index-assignment path is the hole.
- C) Arrays are fine because growth goes through `push()`, and native fn calls
  check `args[0]` (`src/func/call.rs:452`).
- The earlier safety-harness map case only errored because the script ended in
  `nests.len()` — a fn call whose argument gets checked, i.e. *after* the RAM
  was already allocated (+71 MB peak).

Mitigations that do work: `set_max_operations` (D — the real backstop for RAM
too, since allocation costs operations), and calling
`engine.ensure_data_size_within_limits(&value)` on the host side (E) — though
E only detects it post-hoc.

## Part 3 — Python library (pelicanscript/)

PyO3 0.27 + maturin 1.14. `#[pyclass(unsendable)]` is required because Rhai's
`Engine` uses `Rc` internally unless the `sync` feature is on.

API: `Engine(max_operations=, timeout_ms=, max_call_levels=, max_array_size=,
max_map_size=, max_string_size=, max_expr_depth=)`, with `.eval()`, `.run()`,
`.check()`, `.set()`, `.get()`, `.register(name, fn, arity)`, `.output`,
`.clear_output()`, `.check_size()`. Exceptions subclass `RhaiError`:
`ScriptParseError`, `ScriptRuntimeError`, `ScriptTimeout`, `TooManyOperations`,
`DataTooLarge`, `StackOverflow`.

Implementation notes:
- `TypeId::of::<Dynamic>()` is Rhai's wildcard param type — that's how
  `register_raw_fn` accepts arbitrary Python callables (module/mod.rs:562).
- The wall-clock deadline is armed per `eval()` call via `Rc<RefCell<Option<Instant>>>`
  read inside `on_progress`, so one timeout doesn't poison the engine.
- Python exceptions from callbacks are smuggled out as `EvalAltResult::ErrorRuntime`
  carrying the message, then re-raised as `ScriptRuntimeError`.
- PyO3 0.27 deprecates `downcast` in favour of `Bound::cast`.

Gotcha: `'fish'` in Rhai is a *char* literal, so `a.push('fish')` is a parse
error, not a string push. Cost me a bogus test result initially.

### Finding: time/operation budgets do NOT bound memory

`ram_probe.py` runs each bomb in a subprocess with `RLIMIT_AS` at 2 GB.
Discovered when the in-process memory test grew peak RSS by 2.3 GB and then
OOM-killed the interpreter.

| bomb | limits | outcome | peak RAM |
|---|---|---|---|
| string doubling | max_operations=5M | **OOM-KILLED** | >2 GB |
| string doubling | timeout_ms=500 | ScriptTimeout | 577 MB |
| string doubling | size limits | DataTooLarge in 1ms | 1.9 MB |
| array doubling | max_operations=5M | **OOM-KILLED** | >2 GB |
| array doubling | timeout_ms=500 | ScriptTimeout | 385 MB |
| map index-assign | size limits only | **OOM-KILLED** | >2 GB |

`s += s` doubles the buffer in a couple of operations, so a 5M-operation
budget permits ~2^20 doublings' worth of allocation. Operation counts are a
proxy for time, not for bytes. Only the explicit size limits bound RAM, and
`max_map_size` is the one with the hole — hence the map row OOMing.

### Finding: enabling a size limit makes container building O(N^2)

`quadratic_probe.py`. `check_data_size` -> `calc_data_sizes` walks the *whole*
container on every checked operation, so N pushes cost O(N^2) element visits.

| N | no limits | with max_array_size | slowdown | growth per doubling |
|---|---|---|---|---|
| 2,000 | 0.9ms | 3.1ms | 3.3x | — |
| 4,000 | 1.6ms | 10.6ms | 6.7x | 3.48x |
| 8,000 | 3.2ms | 40.4ms | 12.5x | 3.81x |
| 16,000 | 6.1ms | 151.5ms | 25.0x | 3.75x |
| 32,000 | 13.3ms | 597.6ms | 44.9x | 3.94x |

~4x per doubling of N confirms quadratic. Same effect on maps (4.6x slowdown
at N=8,000). Consequence: the RAM defence is itself a CPU amplifier — a script
staying well inside `max_array_size` can burn CPU ~45x faster than the same
script unsandboxed. So a size limit must always be paired with
`max_operations` *and* a wall-clock timeout.

Searched rhaiscript/rhai issues for `max_map_size` — 0 results, so the index
assignment gap appears unreported.
