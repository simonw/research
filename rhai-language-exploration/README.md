# Putting Rhai through its paces

[Rhai](https://github.com/rhaiscript/rhai) is an embedded scripting language for
Rust — a small, dynamically typed language designed to be dropped into a host
application so users can supply logic without being handed the keys to the
process. This investigation used **Rhai 1.25.1** to do three things:

1. Write a pelican-themed script exercising as much of the language as possible.
2. Attack the engine with deliberately hostile scripts to see whether its
   resource limits actually hold.
3. Build a Python extension module that embeds Rhai, and drive it from Python.

The headline result is that Rhai's sandbox works, but **not in the way the
knob names suggest**. Three findings, in descending order of importance:

- **Operation and time budgets do not bound memory.** A script under a
  5,000,000-operation budget allocated more than 2 GB and got OOM-killed. Only
  the explicit size limits bound RAM.
- **`max_map_size` is not enforced on `m[key] = value`.** With
  `max_map_size(1000)` a script still built a 200,000-entry map. This appears
  to be unreported upstream.
- **Turning on a size limit makes container building O(N²)** — 45× slower at
  N=32,000 — so the memory defence is itself a CPU amplifier.

Everything below is reproducible; captured outputs are checked in alongside the
code.

---

## Part 1 — The Great Pelican Tour

[`pelican-rhai/scripts/pelican_tour.rhai`](pelican-rhai/scripts/pelican_tour.rhai)
is a 16-section script covering variables and shadowing, integer/float/hex/
octal/binary literals, ranges, string interpolation, arrays with the functional
methods (`map`/`filter`/`reduce`/`some`/`all`), object maps, the full control
flow set (`if`/`switch` with guards/`while`/`do-while`/`loop`-with-break-value/
`for` with index destructuring), functions, recursion, function pointers,
closures with capture and currying, map-based OOP with `this`, `throw`/
`try`-`catch`, type reflection, timestamps, modules, and `eval`.

[`pelican-rhai/src/main.rs`](pelican-rhai/src/main.rs) is the Rust host. It
registers a custom `Pelican` struct with methods and getters/setters, a custom
`outfishes` infix operator, and a `colony` module written in Rhai and exposed
through a `StaticModuleResolver`. It then pulls a typed object map back out of
the script and calls script functions from Rust.

```console
$ cd pelican-rhai && cargo run --release --bin pelican_tour
```

Full output: [`tour_output.txt`](tour_output.txt). A taste:

```
>>> 13. Custom Rust type: Pelican struct
  SQUAWK! I am Gulliver, wingspan 3.10m, 8 fish down the pouch!
  Gulliver outfishes Beaky? true
>>> 15. Back in Rust: typed result extracted from the script
  champion = Gulliver (8 fish), mood = delighted
```

### Syntax surprises worth knowing

Writing the tour turned up a handful of things that will bite anyone coming
from JavaScript or Python:

- **`call` is a reserved word.** `let call = "SQUAWK";` fails with
  `ErrorParsing(Reserved("call"))`.
- **`'fish'` is a char literal, not a string.** A multi-character single-quoted
  literal is a parse error. This produced a bogus test result before I caught it.
- **`switch` guards attach to literals or ranges, never to a binding.**
  `n if n > 15 => ...` does not parse; `16..=1000 if catch % 2 == 1 => ...` does.
- **`sort()` mutates in place and returns unit.** `let s = a.map(..).sort();`
  silently binds `()`. Same for `map.keys().sort()` and `String::pad`.
- **Closures in maps need method-call syntax to bind `this`.** `percy.gulp(3)`
  binds `this`; `percy.gulp.call(3)` does not. To bind `this` explicitly from a
  function pointer, invert it: `some_map.call(fn_ptr)`.
- **`engine.call_fn` re-runs the whole script body by default.** Without
  `CallFnOptions::new().eval_ast(false)` the entire script executes a second
  time — which I only noticed because the tour printed itself twice.

---

## Part 2 — Can Rhai stop a runaway script?

[`pelican-rhai/src/bin/safety.rs`](pelican-rhai/src/bin/safety.rs) throws one
hostile script at each limit and reports how it died, how long that took, and
the peak RSS it cost (read from `/proc/self/status` `VmHWM`, with the
high-water mark reset per case).

```console
$ cargo run --release --bin safety
```

Full output: [`safety_output.txt`](safety_output.txt).

| Limit | Hostile script | Outcome |
|---|---|---|
| `set_max_operations(1M)` | `loop { dives += 1; }` | stopped in 30 ms |
| `on_progress` + clock | `loop { flaps += 1; }` | stopped in 250.0 ms, +8 KiB |
| `set_max_array_size(10k)` | push 2M items | stopped in 46 ms, +164 KiB |
| `set_max_string_size(100k)` | `s += s` × 40 | stopped in 200 µs |
| `set_max_map_size(5k)` | 1M index assignments | **not stopped during the loop** |
| `set_max_call_levels(64)` | infinite recursion | stopped in 270 µs |
| `set_max_expr_depths(32,32)` | 500-deep nesting | rejected at parse, 10 µs |

For contrast, the same array bomb with *no* limits completes: 2,000,000
elements, 866 ms, +31 MB peak RSS. An unsandboxed engine really will eat the
host's memory, so these knobs are not optional.

Six of the seven work exactly as advertised. The seventh does not.

### Finding 1: `max_map_size` is not enforced on index assignment

[`pelican-rhai/src/bin/mapgap.rs`](pelican-rhai/src/bin/mapgap.rs) isolates it.
Growing a map with `m[key] = value` never re-checks the size of the map being
grown. The check at `src/eval/chaining.rs:790` inspects the **inserted item**:

```rust
Ok(ref mut item_ptr) => {
    self.eval_op_assignment(global, caches, op_info, root, item_ptr, new_val)?;
    self.check_data_size(item_ptr.as_ref(), op_info.position())?;   // <- the item
    None
}
```

Property assignment takes a different branch (`chaining.rs:925`) which checks
`target.source()` — the whole enclosing map — and is therefore enforced
correctly. Since Rhai property names are static identifiers, the only way to
build a map with dynamic keys is index assignment, which is precisely the
unchecked path.

Measured, from [`mapgap_output.txt`](mapgap_output.txt):

```
A) index-assign, return map directly: max_map_size=1000 but map has 200000
   entries (792.73ms, peak +29500 KiB) -> LIMIT BYPASSED
B) index-assign + a property-assign in the loop: STOPPED after 4.12ms
C) array push, max_array_size=1000: STOPPED after 995.24µs (enforced promptly)
```

Arrays are unaffected because arrays grow through `push()`, and native function
calls do check their arguments (`src/func/call.rs:452`). That is also why the
map case in the safety table *appeared* to be caught: the script ended in
`nests.len()`, and the error fired on that call — after 1,000,000 entries had
already been allocated.

The practical mitigations, both verified: an operations budget bounds the loop
regardless of the gap, and `engine.ensure_data_size_within_limits(&value)` lets
the host re-check a returned value (though only once the RAM has been spent).

A search of the rhaiscript/rhai issue tracker for `max_map_size` returns no
results, so this looks unreported.

---

## Part 3 — `pelicanscript`: embedding Rhai in Python

[`pelicanscript/`](pelicanscript/) is a CPython extension module built with
PyO3 0.27 and maturin, exposing the sandbox as a Python API.

```python
import pelicanscript as ps

engine = ps.Engine(max_operations=1_000_000, timeout_ms=500,
                   max_array_size=10_000, max_string_size=100_000)

engine.set("sightings", [{"species": "Brown", "count": 12}])
engine.register("fish_stock", lambda site: FISH_DB.get(site, 0), arity=1)

result = engine.eval("sightings.filter(|s| s.count > 5).map(|s| s.species)")
# ['Brown']

try:
    engine.eval("let a = []; loop { a.push(1); }")
except ps.DataTooLarge as e:
    ...   # the Python process is unharmed
```

Values round-trip between the two type systems: Python `int`/`float`/`bool`/
`str`/`None`/`list`/`tuple`/`dict` map onto Rhai's `INT`/`FLOAT`/`bool`/string/
unit/array/object-map and back. Rhai errors are raised as a typed exception
hierarchy under `RhaiError`: `ScriptParseError`, `ScriptRuntimeError`,
`ScriptTimeout`, `TooManyOperations`, `DataTooLarge`, `StackOverflow`.

### Building and running

```console
$ cd pelicanscript && maturin build --release
$ pip install ../wheels/pelicanscript-0.1.0-cp311-cp311-manylinux_2_35_x86_64.whl
$ python examples/demo.py
```

A prebuilt wheel for CPython 3.11 / manylinux_2_35 x86-64 is in
[`wheels/`](wheels/). The four Python programs:

| Script | What it shows | Captured output |
|---|---|---|
| [`examples/demo.py`](pelicanscript/examples/demo.py) | Eight scenarios: values in and out, Python callbacks, output capture, hostile scripts, a user-supplied-rules sandbox | [`demo_output.txt`](demo_output.txt) |
| [`examples/test_sandbox.py`](pelicanscript/examples/test_sandbox.py) | 20 assertive tests behind every claim here | [`test_output.txt`](test_output.txt) |
| [`examples/ram_probe.py`](pelicanscript/examples/ram_probe.py) | How much RAM each bomb reaches under each limit combination | [`ram_probe_output.txt`](ram_probe_output.txt) |
| [`examples/quadratic_probe.py`](pelicanscript/examples/quadratic_probe.py) | The O(N²) cost of enabling a size limit | [`quadratic_output.txt`](quadratic_output.txt) |

All 20 tests pass, including the seven sandbox limits, per-call timeout
semantics, and 60 consecutive hostile scripts leaving peak RSS flat.

Implementation notes for anyone doing the same: the `pyclass` must be
`unsendable` because Rhai's `Engine` holds `Rc`s unless the `sync` feature is
enabled; `TypeId::of::<Dynamic>()` is Rhai's wildcard parameter type, which is
how `register_raw_fn` accepts arbitrary-arity Python callables; and the
wall-clock deadline is stored in an `Rc<RefCell<Option<Instant>>>` read from
inside `on_progress` and re-armed per `eval()`, so a script that times out
doesn't poison the engine for subsequent calls.

### Finding 2: operation and time budgets do not bound memory

This surfaced by accident. A memory-containment test that set
`max_operations`, `max_array_size` and `timeout_ms` — but not
`max_string_size` — grew peak RSS by 2.3 GB, and a follow-up probe was
OOM-killed outright.

[`ram_probe.py`](pelicanscript/examples/ram_probe.py) runs each bomb in a fresh
subprocess with `RLIMIT_AS` capped at 2 GB, so a case that would exhaust the
machine is reported instead of taking the runner down. From
[`ram_probe_output.txt`](ram_probe_output.txt):

| Bomb | Limits | Outcome | Peak RAM |
|---|---|---|---|
| string doubling | none | OOM-KILLED | >2 GB |
| string doubling | `max_operations=5M` | **OOM-KILLED** | **>2 GB** |
| string doubling | `timeout_ms=500` | ScriptTimeout | 577 MB |
| string doubling | size limits | DataTooLarge in 1.0 ms | 1.9 MB |
| array doubling | `max_operations=5M` | **OOM-KILLED** | **>2 GB** |
| array doubling | `timeout_ms=500` | ScriptTimeout | 385 MB |
| array push | `max_operations=5M` | TooManyOperations | 13.5 MB |
| map index-assign | size limits only | **OOM-KILLED** | **>2 GB** |

The reason is straightforward once seen: `s += s` doubles a buffer in a couple
of operations, so a five-million-operation budget authorises an astronomical
number of doublings. **Operation counts are a proxy for time, not for bytes.**
A wall-clock timeout is better but still weak — 500 ms of doubling reached
577 MB, and the number that matters is bytes-per-millisecond on the host's
allocator, not a figure you can reason about in advance.

Only the explicit size limits bound memory, and they do it in about a
millisecond. The final row is the two findings compounding: with size limits
set but no operations budget, the `max_map_size` gap lets a map bomb through to
an OOM kill.

### Finding 3: a size limit makes container building quadratic

In the table above, the array-push bomb under size limits took **22 seconds**
to be stopped — two orders of magnitude longer than the same bomb under an
operations budget. That is backwards, and worth chasing.

The cause is in `src/eval/data_check.rs`: `check_data_size` calls
`calc_data_sizes`, which **recursively walks the entire container** to total up
its size. Once any data-size limit is set, every checked operation walks
everything, so building a container of N elements costs O(N²) element visits.

[`quadratic_probe.py`](pelicanscript/examples/quadratic_probe.py) confirms it —
each doubling of N roughly quadruples the time
([`quadratic_output.txt`](quadratic_output.txt)):

| N | No limits | With `max_array_size` | Slowdown | Growth per doubling |
|---|---|---|---|---|
| 2,000 | 0.9 ms | 3.1 ms | 3.3× | — |
| 4,000 | 1.6 ms | 10.6 ms | 6.7× | 3.48× |
| 8,000 | 3.2 ms | 40.4 ms | 12.5× | 3.81× |
| 16,000 | 6.1 ms | 151.5 ms | 25.0× | 3.75× |
| 32,000 | 13.3 ms | 597.6 ms | 44.9× | 3.94× |

Object maps show the same shape (4.6× slowdown at N=8,000). Note the limit
itself is never hit here — the array cap is 1,000,000 and N never exceeds
32,000. Merely *enabling* the limit imposes the cost.

The security consequence is that the RAM defence is a CPU amplifier: a script
staying comfortably inside `max_array_size` burns CPU ~45× faster than the same
script would unsandboxed, and the multiplier grows with N. It also means the
limits are not free for legitimate workloads — a script assembling a 32,000-row
result pays 600 ms instead of 13 ms.

---

## Recommended configuration

Putting the three findings together, a defensible sandbox needs limits from all
three families, because each covers a gap the others leave open:

```python
engine = ps.Engine(
    max_operations=5_000_000,   # bounds CPU; also the real backstop for the
                                # max_map_size index-assignment gap
    timeout_ms=1_000,           # bounds wall clock, incl. slow host callbacks
    max_string_size=100_000,    # bounds RAM — operations/time do NOT
    max_array_size=50_000,      # ditto
    max_map_size=10_000,        # ditto, but leaky: needs max_operations too
    max_call_levels=32,         # bounds native stack
    max_expr_depth=64,          # bounds the parser, before execution
)
```

The reasoning in one line each:

- Size limits are the **only** thing that bounds memory. Never omit them.
- `max_operations` is the only thing that plugs the `max_map_size` hole.
- A wall-clock timeout catches what operation counting misses, including time
  spent inside slow host functions, which do not tick the operation counter.
- Because size limits make container work quadratic, the operations budget and
  timeout are doing double duty as the defence against the amplification the
  size limits introduce.

Set together they cost about 5 ms on a legitimate 1,000-element workload, and
they contained every hostile script tried here.

---

## Overall assessment

Rhai is a genuinely pleasant embedding target. The Rust API is clean —
registering a struct with methods, getters, setters and a custom infix operator
took about forty lines — the language itself is expressive enough to be worth
exposing to users, and wrapping the whole thing for Python was a couple of
hundred lines of PyO3 that compiled on the first try.

The sandbox is real and it works, but the mental model the API invites is
wrong. The knobs are presented as an à la carte list where each covers one
resource, and the natural reading is that `max_operations` is the CPU limit
and the size limits are for data hygiene. In fact operation and time budgets
say nothing about memory, the size limits are what bound memory, `max_map_size`
has a hole on the only code path that can grow a map with dynamic keys, and
enabling the size limits makes the thing they protect quadratically expensive.
None of that is documented at the point of use. The knobs are interdependent,
and you have to set essentially all of them to be safe.

## Repository layout

```
pelican-rhai/                    Rust host programs (cargo run --release --bin ...)
  scripts/pelican_tour.rhai      the 16-section language tour
  src/main.rs                    host: custom type, operator, module, call_fn
  src/bin/safety.rs              one hostile script per limit
  src/bin/mapgap.rs              isolates the max_map_size gap
  src/bin/mapdiag.rs             timing/scaling diagnostics for the map path
pelicanscript/                   the Python extension module
  src/lib.rs                     PyO3 bindings: conversion, limits, callbacks
  examples/demo.py               eight-scenario walkthrough
  examples/test_sandbox.py       20 assertive tests
  examples/ram_probe.py          RAM-per-limit matrix (subprocess isolated)
  examples/quadratic_probe.py    O(N^2) measurement
wheels/                          prebuilt CPython 3.11 manylinux x86-64 wheel
*_output.txt                     captured output from every program above
notes.md                         working notes kept during the investigation
```

Both crates depend on a Rhai checkout at `/tmp/rhai`:

```console
$ git clone https://github.com/rhaiscript/rhai /tmp/rhai
```

No files in the Rhai clone were modified, so there is no diff to include.
