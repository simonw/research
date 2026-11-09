"""Demonstration of the `pelicanscript` Python library.

Shows Python code driving embedded Rhai scripts: passing data in, getting
typed values back, calling Python functions from a script, capturing script
output, and — the main event — surviving deliberately hostile scripts.

    pip install pelicanscript/target/wheels/pelicanscript-*.whl
    python pelicanscript/examples/demo.py
"""

import time

import pelicanscript as ps


def section(title):
    print(f"\n{'=' * 70}\n{title}\n{'=' * 70}")


# ---------------------------------------------------------------------------
section("1. Evaluating a script and getting Python values back")

engine = ps.Engine(max_operations=5_000_000, timeout_ms=2000)

result = engine.eval(
    """
    let flock = ["Percy", "Petra", "Pip", "Gulliver"];
    #{
        colony: "Brackish Bay",
        size: flock.len,
        names: flock,
        avg_name_len: flock.reduce(|a, n| a + n.len, 0).to_float() / flock.len,
        thriving: flock.len > 3,
    }
    """
)
print(f"  returned a {type(result).__name__}: {result}")
print(f"  result['names'] is a {type(result['names']).__name__}: {result['names']}")
print(f"  result['thriving'] is a {type(result['thriving']).__name__}")
print(f"  rhai version: {ps.rhai_version}")

# ---------------------------------------------------------------------------
section("2. Passing Python data into the script scope")

engine = ps.Engine(max_operations=5_000_000)
engine.set(
    "sightings",
    [
        {"species": "Brown", "count": 12, "wingspan": 2.1},
        {"species": "Dalmatian", "count": 3, "wingspan": 3.4},
        {"species": "Great White", "count": 7, "wingspan": 3.0},
    ],
)
engine.set("min_count", 5)

report = engine.eval(
    """
    let notable = sightings.filter(|s| s.count >= min_count);
    let total = sightings.reduce(|acc, s| acc + s.count, 0);
    let widest = "";
    let widest_span = 0.0;
    for s in sightings {
        if s.wingspan > widest_span { widest_span = s.wingspan; widest = s.species; }
    }
    #{ total: total, notable: notable.map(|s| s.species), widest: widest }
    """
)
print(f"  total pelicans seen : {report['total']}")
print(f"  notable species     : {report['notable']}")
print(f"  widest wingspan     : {report['widest']}")

# Scope persists between eval() calls on the same engine:
engine.run("let tagged = notable.len;")
print(f"  read back from scope: tagged = {engine.get('tagged')}")

# ---------------------------------------------------------------------------
section("3. Calling Python functions from inside a Rhai script")

engine = ps.Engine(max_operations=5_000_000)

FISH_DB = {"Brackish Bay": 220, "Mangrove Island": 91, "Salt Marsh": 14}


def fish_stock(location):
    """A 'database lookup' living in Python, exposed to the script."""
    return FISH_DB.get(location, 0)


def alert(level, message):
    print(f"  [python:{level}] {message}")
    return None


engine.register("fish_stock", fish_stock, arity=1)
engine.register("alert", alert, arity=2)

engine.eval(
    """
    let sites = ["Brackish Bay", "Mangrove Island", "Salt Marsh", "Dry Gulch"];
    for site in sites {
        let stock = fish_stock(site);
        if stock == 0 {
            alert("error", `${site}: no data, do not send the flock`);
        } else if stock < 50 {
            alert("warn", `${site}: only ${stock} fish, ration carefully`);
        } else {
            alert("info", `${site}: ${stock} fish, send the pelicans`);
        }
    }
    """
)

# Exceptions raised inside the Python callback surface as ScriptRuntimeError:
def grumpy(_n):
    raise ValueError("this pelican refuses to be counted")


engine.register("grumpy", grumpy, arity=1)
try:
    engine.eval("grumpy(1)")
except ps.ScriptRuntimeError as e:
    print(f"  callback exception propagated: {type(e).__name__}: {e}")

# ---------------------------------------------------------------------------
section("4. Capturing script print() output")

engine = ps.Engine(max_operations=1_000_000)
engine.run(
    """
    for i in 1..=3 { print(`pelican ${i} reporting for duty`); }
    debug("pouch check complete");
    """
)
for line in engine.output:
    print(f"  captured: {line}")
engine.clear_output()
print(f"  after clear_output(): {engine.output}")

# ---------------------------------------------------------------------------
section("5. Rejecting bad syntax before running anything")

engine = ps.Engine()
try:
    engine.check("let x = ;")
except ps.ScriptParseError as e:
    print(f"  check() rejected it: {e}")
engine.check("let x = 1 + 1;")
print("  check() accepted a valid script")

# ---------------------------------------------------------------------------
section("6. THE SANDBOX: hostile scripts cannot hurt the Python process")

HOSTILE = [
    (
        "infinite loop (CPU)",
        "let n = 0; loop { n += 1; }",
        dict(max_operations=2_000_000),
    ),
    (
        "infinite loop (wall clock)",
        "let n = 0; loop { n += 1; }",
        dict(timeout_ms=250),
    ),
    (
        "array bomb (RAM)",
        "let a = []; loop { a.push(\"fish\"); }",
        dict(max_array_size=10_000, max_operations=50_000_000),
    ),
    (
        "string bomb (RAM)",
        'let s = "SQUAWK"; loop { s += s; }',
        dict(max_string_size=100_000, max_operations=50_000_000),
    ),
    (
        "infinite recursion (stack)",
        "fn dive(n) { dive(n + 1) } dive(0)",
        dict(max_call_levels=50),
    ),
    (
        "deeply nested expression (parser)",
        "let x = " + "(1 + " * 300 + "1" + ")" * 300 + ";",
        dict(max_expr_depth=32),
    ),
    (
        "map bomb via index assignment",
        "let m = #{}; for i in 0..5_000_000 { m[`k_${i}`] = i; } m",
        dict(max_map_size=1_000, max_operations=2_000_000),
    ),
]

for label, script, limits in HOSTILE:
    engine = ps.Engine(**limits)
    start = time.perf_counter()
    try:
        engine.eval(script)
        print(f"  {label:34s} NOT STOPPED (!)")
    except ps.RhaiError as e:
        ms = (time.perf_counter() - start) * 1000
        print(f"  {label:34s} {type(e).__name__:18s} after {ms:7.2f} ms")

print("\n  ...and the Python process is still perfectly healthy:")
print(f"  2 + 2 = {2 + 2}")

# ---------------------------------------------------------------------------
section("7. The one limit that needs help: max_map_size")

# Rhai does not re-check the enclosing map's size on `m[key] = value`, so with
# max_map_size alone a script can still build an enormous map. An operations
# budget is the reliable backstop (see the README for the analysis).
engine = ps.Engine(max_map_size=1_000)  # no operations budget
grown = engine.eval("let m = #{}; for i in 0..50_000 { m[`k_${i}`] = i; } m")
print(f"  max_map_size=1000 but the script built a map with {len(grown)} keys")

engine = ps.Engine(max_map_size=1_000)
try:
    engine.check_size(grown)
except ps.DataTooLarge as e:
    print(f"  engine.check_size() catches it after the fact: {e}")

engine = ps.Engine(max_map_size=1_000, max_operations=200_000)
try:
    engine.eval("let m = #{}; for i in 0..50_000 { m[`k_${i}`] = i; } m")
except ps.TooManyOperations as e:
    print(f"  adding max_operations stops it properly: {type(e).__name__}: {e}")

# ---------------------------------------------------------------------------
section("8. A practical use: user-supplied rules, safely")

# Imagine these rules came from untrusted users of a bird-survey app.
USER_RULES = [
    ("sensible", "sightings.filter(|s| s.count > 5).map(|s| s.species)"),
    ("also fine", "sightings.reduce(|a, s| a + s.count, 0) > 20"),
    ("malicious", "loop { }"),
    ("malicious", "let a = []; loop { a.push(1); }"),
    ("broken", "sightings.frobnicate()"),
]

sandbox = ps.Engine(
    max_operations=1_000_000,
    timeout_ms=1000,
    max_array_size=10_000,
    max_string_size=100_000,
    max_call_levels=32,
    max_expr_depth=64,
)
sandbox.set(
    "sightings",
    [
        {"species": "Brown", "count": 12},
        {"species": "Dalmatian", "count": 3},
        {"species": "Great White", "count": 7},
    ],
)

for kind, rule in USER_RULES:
    try:
        value = sandbox.eval(rule)
        print(f"  [{kind:9s}] {rule[:46]:46s} -> {value}")
    except ps.RhaiError as e:
        print(f"  [{kind:9s}] {rule[:46]:46s} -> BLOCKED ({type(e).__name__})")

print("\nAll demos complete. The colony is safe. SQUAWK!")
