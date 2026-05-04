# TRE Python Binding - Investigation Notes

## Goal

Build a Python binding for the TRE regex library
(https://github.com/laurikari/tre/) and use it to demonstrate robustness
against ReDoS (Regular Expression Denial of Service) — patterns that
cause exponential blowup with Python's `re` module.

The hypothesis: TRE uses a non-backtracking matching algorithm with linear
time in input length and quadratic in pattern length, so there is no
exponential explosion on catastrophic backtracking patterns. Per the
README:

> The matching algorithm used in TRE uses linear worst-case time in the
> length of the text being searched, and quadratic worst-case time in the
> length of the used regular expression.
> A `regexec()` call never allocates memory from the heap.

## What I built

- `src/tre_py/__init__.py` — small ctypes binding for the wide-char
  TRE API (`tre_regwcomp` / `tre_regwnexec` / `tre_regfree` /
  `tre_regerror`). Exposes `compile`, `Pattern.search`, `Match`, plus
  flags `IGNORECASE`/`EXTENDED`/`NEWLINE`/`NOSUB`.
- `src/tre_py/deadline.py` — `search_with_deadline(pattern, s, t)`:
  thread-based wall-clock cap on a single search, raising `MatchTimeout`
  on overrun. Belt-and-braces protection on top of TRE's algorithmic
  guarantees.
- `tests/test_basic.py` — 10 tests for the API surface.
- `tests/test_redos.py` — 12 tests demonstrating ReDoS robustness:
    - 4 evil patterns shown to be catastrophic in `re` at n=24.
    - The same 4 evil patterns finishing in TRE under 2s at n=100,000.
    - Polynomial scaling: 10x more input ≤ 40x more time.
    - Memory bound: matching 1M chars adds <20MB RSS.
    - Deadline wrapper returns in time and raises on overrun.
- `benchmark.py` — head-to-head numbers, captured in `benchmark.out`.
- `build_libtre.sh` — reproduce `libtre.so` from a fresh TRE clone.
- `libtre.so` — built shared library (259K, ELF x86-64).

## Process

1. **Clone & build TRE.** The repo uses autotools. After
   `apt-get install autopoint gettext`, `./utils/autogen.sh` regenerates
   the configure script and `./configure --enable-shared && make`
   produces `lib/.libs/libtre.so.5.0.0`.

2. **Set up uv project.** `pyproject.toml` with `package = false`,
   `pythonpath = ["src"]`, dev-dep on `pytest`. `uv sync` brings in the
   test runner.

3. **Red phase — basic API.** Wrote `tests/test_basic.py` with 10 tests
   covering compile/search/anchored/groups/alternation/invalid pattern/
   unicode/quantifiers/case-insensitive. Ran pytest; got
   `ModuleNotFoundError: No module named 'tre_py'`.

4. **Green phase — implement.** Wrote a ctypes binding around
   `tre_regwcomp` (wide-char compile) and `tre_regwnexec` (wide-char,
   length-aware search). Used the wide-char variants because Python str
   is unicode and reporting spans in str-indices is what callers expect.
   All 10 tests went green.

5. **ReDoS demo — calibration.** Tried several "evil" patterns. Some
   that look catastrophic in textbooks aren't actually exponential in
   CPython's `re`:
     - `(a|aa)+$` finishes in 9ms at n=24 (CPython probably folds the
       alternation).
     - `(.*a){10}b$` runs in 122ms — closer to linear because `.*` is
       greedy and consumes the whole string up front.
   Patterns confirmed exponential at n=24 (>=400ms each on this host):
     - `^(a+)+$` — 0.72s
     - `^(a*)*$` — 1.05s
     - `^([a-zA-Z]+)*$` — 0.81s
     - `^(x+x+)+$` — 0.51s
   At n=28 these would each take ~10–60s, which is too slow for tests.

6. **Test that TRE handles them.** Same patterns at n=100,000 (≈4000×
   bigger input than what `re` couldn't handle) finished in milliseconds.
   Bumped to 10M chars in the benchmark to see scaling properly.

7. **Memory bound.** `resource.getrusage(RUSAGE_SELF).ru_maxrss` before
   and after a 1M-char match grows by ~4MB (the input string itself in
   wide-char form) — well under the 20MB allowance in the test.

8. **Deadline wrapper.** Threading-based, not signal-based, so it works
   inside non-main threads and on Windows. Can't actually kill the
   worker, but TRE returns within ms in normal operation, so a timeout
   means something is genuinely wrong — not stuck spinning forever.

## Calibration data captured along the way

```
re on (a+)+$       n=20: 0.047s   n=24: 0.728s   n=28: 11.737s
```

Doubling 20→28 gives ~250x slowdown, perfectly consistent with 2^N
exponential blow-up.

## benchmark.out summary

```
(a+)+$       re   n=24       0.765s
(a+)+$       tre  n=10,000,000  0.164s
```

TRE on a 10-million-character input is faster than `re` on a 24-character
input. Across all four evil patterns, TRE scales linearly — 10x more
input gives ≤10x more time.

## Things I deliberately did not do

- I didn't wrap `tre_regaexec` (approximate matching). Useful but
  orthogonal to the ReDoS goal.
- I didn't expose `findall` / `finditer` / `sub`. Single-shot `search`
  is enough to make the point.
- I didn't modify the upstream TRE source. The build is unmodified.
- I didn't use the existing `tre/python/tre-python.c` CPython extension.
  ctypes is lighter, simpler, and one less compile step.

## How to rebuild the binding from scratch

```sh
./build_libtre.sh                   # regenerates libtre.so
uv sync                             # installs pytest
uv run pytest tests/                # 22 passed
PYTHONPATH=src uv run python benchmark.py
```
