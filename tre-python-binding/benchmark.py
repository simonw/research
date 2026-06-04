"""Quick benchmark: Python `re` vs TRE on classic ReDoS patterns.

Usage:
    uv run python benchmark.py
"""
from __future__ import annotations

import re
import time

import tre_py


PATTERNS = [
    ("(a+)+$",        lambda n: "a" * n + "!"),
    ("(a*)*$",        lambda n: "a" * n + "!"),
    ("([a-zA-Z]+)*$", lambda n: "a" * n + "!"),
    ("(x+x+)+$",      lambda n: "x" * n + "!"),
]

# Sizes for each engine. `re` blows up exponentially so we pick small n;
# TRE is polynomial so we pick a much larger n.
RE_SIZES = [16, 20, 24]
TRE_SIZES = [10_000, 100_000, 1_000_000, 10_000_000]

TIMEOUT_S = 10.0  # cap individual `re` calls so we never hang on n too big.


def _time_one(fn, *args) -> float:
    t = time.perf_counter()
    fn(*args)
    return time.perf_counter() - t


def main() -> None:
    print(f"{'pattern':<18} {'engine':<6} {'n':>10} {'time':>10}")
    print("-" * 48)
    for label, make in PATTERNS:
        for n in RE_SIZES:
            s = make(n)
            t0 = time.perf_counter()
            re.match(f"^{label}", s)
            elapsed = time.perf_counter() - t0
            print(f"{label:<18} {'re':<6} {n:>10} {elapsed:>9.3f}s")
            if elapsed > TIMEOUT_S:
                break
        pat = tre_py.compile(f"^{label}")
        for n in TRE_SIZES:
            s = make(n)
            elapsed = _time_one(pat.search, s)
            print(f"{label:<18} {'tre':<6} {n:>10} {elapsed:>9.3f}s")
        print()


if __name__ == "__main__":
    main()
