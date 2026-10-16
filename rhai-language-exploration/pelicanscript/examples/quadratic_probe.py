"""Does enabling a size limit make container building quadratic?

Suspicion raised by ram_probe.py: an array-push bomb under `max_array_size`
took 22 seconds to be stopped, far longer than the same bomb under an
operations budget. Rhai's `check_data_size` (src/eval/data_check.rs) calls
`calc_data_sizes`, which walks the *entire* container on every checked
operation — so pushing N items costs O(N^2) element visits once any data-size
limit is set.

If that's right, the safety feature itself is a CPU denial-of-service vector:
turning on `max_array_size` makes legitimate list-building dramatically slower.

    python pelicanscript/examples/quadratic_probe.py
"""

import time

import pelicanscript as ps


def build_time(n, **limits):
    engine = ps.Engine(max_operations=500_000_000, **limits)
    script = f"let a = []; for i in 0..{n} {{ a.push(i); }} a.len"
    start = time.perf_counter()
    assert engine.eval(script) == n
    return time.perf_counter() - start


print("Building an array of N integers, with and without a size limit set.\n")
header = f"{'N':>8s} {'no limits':>12s} {'max_array_size':>16s} {'slowdown':>10s} {'ratio vs N/2':>13s}"
print(header)
print("-" * len(header))

prev_limited = None
for n in (2_000, 4_000, 8_000, 16_000, 32_000):
    plain = build_time(n)
    limited = build_time(n, max_array_size=1_000_000)
    growth = f"{limited / prev_limited:.2f}x" if prev_limited else "-"
    prev_limited = limited
    print(
        f"{n:8d} {plain * 1000:10.1f}ms {limited * 1000:14.1f}ms "
        f"{limited / plain:9.1f}x {growth:>13s}"
    )

print(
    "\nA doubling of N that roughly quadruples the limited time (ratio ~4x) "
    "confirms O(N^2)."
)

print("\nSame question for object maps (property assignment triggers the check):\n")
header = f"{'N':>8s} {'no limits':>12s} {'max_map_size':>16s} {'slowdown':>10s}"
print(header)
print("-" * len(header))
for n in (1_000, 2_000, 4_000, 8_000):
    script = f"let m = #{{}}; for i in 0..{n} {{ m[`k_${{i}}`] = i; m.tick = i; }} m.len()"

    e = ps.Engine(max_operations=500_000_000)
    start = time.perf_counter()
    e.eval(script)
    plain = time.perf_counter() - start

    e = ps.Engine(max_operations=500_000_000, max_map_size=1_000_000)
    start = time.perf_counter()
    e.eval(script)
    limited = time.perf_counter() - start

    print(f"{n:8d} {plain * 1000:10.1f}ms {limited * 1000:14.1f}ms {limited / plain:9.1f}x")

print(
    "\nPractical consequence: a size limit is necessary to bound RAM, but it "
    "makes\ncontainer building quadratic — so a script can burn CPU cheaply "
    "while staying\nwell inside the size limit. Always pair size limits with "
    "max_operations AND a\nwall-clock timeout."
)
