"""How much RAM can a hostile script allocate under each combination of limits?

Motivation: the obvious sandbox settings (`max_operations`, `timeout_ms`) bound
*how long* a script runs, but a single Rhai operation can allocate an
unbounded amount of memory. A string-doubling bomb reaches gigabytes in well
under a second, so a time/ops budget alone does NOT bound RAM.

Each case runs in a fresh subprocess with RLIMIT_AS set, so a case that would
otherwise exhaust the machine is contained and reported rather than taking the
test runner with it.

    python pelicanscript/examples/ram_probe.py
"""

import json
import os
import resource
import subprocess
import sys
import time

ADDRESS_SPACE_CAP = 2 * 1024**3  # 2 GB per child

BOMBS = {
    "string doubling": 'let s = "SQUAWK"; loop { s += s; }',
    "array doubling": "let a = [1]; loop { a += a; }",
    "array push": 'let a = []; loop { a.push("fish"); }',
    "map index-assign": "let m = #{}; for i in 0..10_000_000 { m[`k_${i}`] = i; } m",
}

CONFIGS = {
    "no limits at all": {},
    "max_operations=5M": dict(max_operations=5_000_000),
    "timeout_ms=500": dict(timeout_ms=500),
    "ops=5M + timeout=500": dict(max_operations=5_000_000, timeout_ms=500),
    "size limits only": dict(
        max_string_size=1_000_000, max_array_size=100_000, max_map_size=100_000
    ),
    "size limits + ops=5M": dict(
        max_string_size=1_000_000,
        max_array_size=100_000,
        max_map_size=100_000,
        max_operations=5_000_000,
    ),
}


def child(bomb_key, config_json):
    """Run one bomb under one config and report peak RSS as JSON on stdout."""
    resource.setrlimit(resource.RLIMIT_AS, (ADDRESS_SPACE_CAP, ADDRESS_SPACE_CAP))
    import pelicanscript as ps  # imported after the cap is in place

    limits = json.loads(config_json)
    engine = ps.Engine(**limits)
    before = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    start = time.perf_counter()
    try:
        engine.eval(BOMBS[bomb_key])
        outcome = "NOT STOPPED"
    except ps.RhaiError as e:
        outcome = type(e).__name__
    elapsed_ms = (time.perf_counter() - start) * 1000
    peak_mb = (resource.getrusage(resource.RUSAGE_SELF).ru_maxrss - before) / 1024
    print(json.dumps({"outcome": outcome, "ms": elapsed_ms, "peak_mb": peak_mb}))


def main():
    print(f"Each case runs in a subprocess capped at {ADDRESS_SPACE_CAP // 1024**3} GB "
          "of address space.\n")
    header = f"{'bomb':18s} {'limits':22s} {'outcome':20s} {'time':>10s} {'peak RAM':>11s}"
    print(header)
    print("-" * len(header))

    for bomb in BOMBS:
        for label, limits in CONFIGS.items():
            proc = subprocess.run(
                [sys.executable, __file__, "--child", bomb, json.dumps(limits)],
                capture_output=True,
                text=True,
                timeout=120,
            )
            if proc.returncode != 0:
                # Killed by the OS / allocator: the sandbox failed to contain it.
                reason = "OOM-KILLED" if proc.returncode < 0 else "ABORTED"
                print(f"{bomb:18s} {label:22s} {reason:20s} {'-':>10s} {'>2 GB':>11s}")
                continue
            r = json.loads(proc.stdout.strip().splitlines()[-1])
            print(
                f"{bomb:18s} {label:22s} {r['outcome']:20s} "
                f"{r['ms']:8.1f}ms {r['peak_mb']:8.1f} MB"
            )
        print()

    print("Takeaway: time and operation budgets do NOT bound memory — a single")
    print("operation can double a buffer. Only the explicit size limits do, and")
    print("max_map_size has a hole (index assignment), so pair it with max_operations.")


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "--child":
        child(sys.argv[2], sys.argv[3])
    else:
        main()
