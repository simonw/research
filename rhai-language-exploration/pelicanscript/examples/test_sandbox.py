"""Assertive tests for pelicanscript — run with `python -m pytest` or directly.

These are the checks behind the claims in the README: every limit raises the
exception it should, values round-trip faithfully, and the hostile scripts
leave the Python process with its memory intact.
"""

import resource
import time

import pelicanscript as ps

FAILURES = []


def check(name, fn):
    try:
        fn()
        print(f"  PASS  {name}")
    except AssertionError as e:
        FAILURES.append((name, str(e)))
        print(f"  FAIL  {name}: {e}")
    except Exception as e:  # noqa: BLE001
        FAILURES.append((name, repr(e)))
        print(f"  ERROR {name}: {type(e).__name__}: {e}")


def peak_rss_mb():
    return resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024


# --------------------------------------------------------------------------
print("\nValue round-tripping")


def test_scalars():
    e = ps.Engine()
    assert e.eval("42") == 42
    assert e.eval("3.5") == 3.5
    assert e.eval("true") is True
    assert e.eval('"squawk"') == "squawk"
    assert e.eval("()") is None
    assert e.eval("[1, 2.5, true, \"pelican\"]") == [1, 2.5, True, "pelican"]
    assert e.eval("#{a: 1, b: [2, 3]}") == {"a": 1, "b": [2, 3]}


def test_python_to_rhai_and_back():
    e = ps.Engine()
    payload = {
        "name": "Percy",
        "wingspan": 2.5,
        "tagged": True,
        "sightings": [1, 2, 3],
        "nest": {"eggs": 3},
        "notes": None,
    }
    e.set("bird", payload)
    assert e.eval("bird") == payload
    assert e.eval("bird.nest.eggs") == 3
    assert e.eval("bird.sightings.len") == 3


def test_unsupported_type_rejected():
    e = ps.Engine()
    try:
        e.set("bad", object())
        raise AssertionError("expected TypeError")
    except TypeError:
        pass


check("scalars, arrays and maps round-trip", test_scalars)
check("nested Python dict/list round-trips", test_python_to_rhai_and_back)
check("unsupported Python type raises TypeError", test_unsupported_type_rejected)

# --------------------------------------------------------------------------
print("\nCallbacks and scope")


def test_callback():
    e = ps.Engine(max_operations=1_000_000)
    e.register("double_fish", lambda n: n * 2, arity=1)
    e.register("join", lambda a, b: f"{a}-{b}", arity=2)
    assert e.eval("double_fish(21)") == 42
    assert e.eval('join("Percy", "Petra")') == "Percy-Petra"


def test_callback_exception_is_typed():
    e = ps.Engine()

    def boom(_):
        raise RuntimeError("nope")

    e.register("boom", boom, arity=1)
    try:
        e.eval("boom(1)")
        raise AssertionError("expected ScriptRuntimeError")
    except ps.ScriptRuntimeError as exc:
        assert "nope" in str(exc), str(exc)


def test_scope_persists():
    e = ps.Engine()
    e.run("let colony = 7;")
    assert e.eval("colony * 2") == 14
    assert e.get("colony") == 7
    assert e.get("no_such_var") is None


def test_output_capture():
    e = ps.Engine()
    e.run('print("a"); print("b");')
    assert e.output == ["a", "b"], e.output
    e.clear_output()
    assert e.output == []


check("Python callbacks callable from Rhai", test_callback)
check("callback exceptions become ScriptRuntimeError", test_callback_exception_is_typed)
check("scope persists across eval/run", test_scope_persists)
check("print output is captured", test_output_capture)

# --------------------------------------------------------------------------
print("\nSandbox limits (each must raise its specific exception)")


def expect(exc_type, script, **limits):
    def run():
        e = ps.Engine(**limits)
        try:
            e.eval(script)
            raise AssertionError(f"script was NOT stopped: {script[:40]}")
        except exc_type:
            pass

    return run


check(
    "max_operations stops an infinite loop",
    expect(ps.TooManyOperations, "let n = 0; loop { n += 1; }", max_operations=500_000),
)
check(
    "timeout_ms stops an infinite loop",
    expect(ps.ScriptTimeout, "let n = 0; loop { n += 1; }", timeout_ms=200),
)
check(
    "max_array_size stops an array bomb",
    expect(
        ps.DataTooLarge,
        'let a = []; loop { a.push("fish"); }',
        max_array_size=5_000,
        max_operations=50_000_000,
    ),
)
check(
    "max_string_size stops a string bomb",
    expect(
        ps.DataTooLarge,
        'let s = "SQUAWK"; loop { s += s; }',
        max_string_size=50_000,
        max_operations=50_000_000,
    ),
)
check(
    "max_call_levels stops infinite recursion",
    expect(ps.StackOverflow, "fn dive(n) { dive(n + 1) } dive(0)", max_call_levels=40),
)
check(
    "max_expr_depth rejects pathological nesting",
    expect(
        ps.ScriptParseError,
        "let x = " + "(1 + " * 200 + "1" + ")" * 200 + ";",
        max_expr_depth=32,
    ),
)
check(
    "every sandbox exception subclasses RhaiError",
    expect(ps.RhaiError, "let n = 0; loop { n += 1; }", max_operations=100_000),
)


def test_timeout_is_per_call_not_per_engine():
    """The deadline is armed per eval(), so a slow first call doesn't poison
    a later one on the same engine."""
    e = ps.Engine(timeout_ms=300)
    try:
        e.eval("let n = 0; loop { n += 1; }")
        raise AssertionError("expected ScriptTimeout")
    except ps.ScriptTimeout:
        pass
    time.sleep(0.4)  # well past the first deadline
    assert e.eval("1 + 1") == 2, "engine was poisoned by the earlier timeout"


check("timeout is per-call, not per-engine", test_timeout_is_per_call_not_per_engine)


def test_legitimate_script_unaffected():
    e = ps.Engine(
        max_operations=5_000_000,
        timeout_ms=5_000,
        max_array_size=50_000,
        max_string_size=100_000,
        max_call_levels=32,
    )
    out = e.eval(
        """
        let flock = [];
        for i in 0..500 { flock.push(#{ id: i, name: `pelican_${i}` }); }
        flock.filter(|p| p.id % 100 == 0).map(|p| p.name)
        """
    )
    assert out == [f"pelican_{i}" for i in range(0, 500, 100)], out


check("a legitimate script runs fine under full limits", test_legitimate_script_unaffected)

# --------------------------------------------------------------------------
print("\nKnown gap: max_map_size on index assignment")


def test_map_size_gap_is_real():
    """Documents upstream behaviour: `m[k] = v` does not re-check map size."""
    e = ps.Engine(max_map_size=100)
    grown = e.eval("let m = #{}; for i in 0..20_000 { m[`k_${i}`] = i; } m")
    assert len(grown) == 20_000, len(grown)


def test_map_size_gap_mitigations():
    # (a) an operations budget bounds it
    e = ps.Engine(max_map_size=100, max_operations=100_000)
    try:
        e.eval("let m = #{}; for i in 0..20_000 { m[`k_${i}`] = i; } m")
        raise AssertionError("expected TooManyOperations")
    except ps.TooManyOperations:
        pass
    # (b) host-side check_size() detects an oversized value
    e = ps.Engine(max_map_size=100)
    try:
        e.check_size({f"k_{i}": i for i in range(500)})
        raise AssertionError("expected DataTooLarge")
    except ps.DataTooLarge:
        pass
    # (c) a property assignment in the loop restores enforcement
    e = ps.Engine(max_map_size=100)
    try:
        e.eval("let m = #{}; for i in 0..20_000 { m[`k_${i}`] = i; m.tick = i; } m")
        raise AssertionError("expected DataTooLarge")
    except ps.DataTooLarge:
        pass


check("map index-assignment bypasses max_map_size (upstream gap)", test_map_size_gap_is_real)
check("mitigations for the map gap all work", test_map_size_gap_mitigations)

# --------------------------------------------------------------------------
print("\nMemory containment")


def test_hostile_scripts_do_not_grow_the_process():
    """NOTE the limits below: `max_string_size` is essential. With only an
    operations budget and a timeout, the string-doubling bomb allocates
    gigabytes before it is stopped — see ram_probe.py. Time budgets bound
    time, not memory."""
    baseline = peak_rss_mb()
    for _ in range(20):
        e = ps.Engine(
            max_array_size=5_000,
            max_string_size=100_000,
            max_map_size=5_000,
            max_operations=5_000_000,
            timeout_ms=1_000,
        )
        for script in (
            'let a = []; loop { a.push("fish"); }',
            'let s = "SQUAWK"; loop { s += s; }',
            "let n = 0; loop { n += 1; }",
        ):
            try:
                e.eval(script)
            except ps.RhaiError:
                pass
    growth = peak_rss_mb() - baseline
    assert growth < 25, f"peak RSS grew {growth:.1f} MB across 60 hostile runs"
    print(f"        (peak RSS grew {growth:.1f} MB over 60 hostile scripts)")


check("60 hostile scripts leave peak RSS flat", test_hostile_scripts_do_not_grow_the_process)

# --------------------------------------------------------------------------
print()
if FAILURES:
    print(f"{len(FAILURES)} FAILURE(S):")
    for name, err in FAILURES:
        print(f"  - {name}: {err}")
    raise SystemExit(1)
print("All tests passed. SQUAWK!")
