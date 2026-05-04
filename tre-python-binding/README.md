# TRE Python binding — ReDoS robustness demo

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

A small Python ctypes binding for the
[TRE regex library](https://github.com/laurikari/tre/), built specifically
to demonstrate that TRE is immune to the catastrophic-backtracking
("ReDoS") class of inputs that hangs Python's `re` module.

## TL;DR

| Pattern | `re` at n=24 | TRE at n=10,000,000 |
|---|---|---|
| `^(a+)+$` | 0.765 s | 0.164 s |
| `^(a*)*$` | 1.127 s | 0.201 s |
| `^([a-zA-Z]+)*$` | 0.827 s | 0.336 s |
| `^(x+x+)+$` | 0.506 s | 0.267 s |

TRE on a **ten-million-character** input finishes faster than `re` on a
twenty-four-character input, on the same evil patterns. And TRE's
runtime grows **linearly** with input length, not exponentially.

## Why TRE is safe and `re` is not

CPython's `re` module is a backtracking matcher. On a pattern with
nested quantifiers (`(a+)+`, `(a*)*`, `([a-zA-Z]+)*`, etc.) and an input
that *almost* matches, the engine can explore an exponential number of
ways to partition the input across the inner and outer quantifier
before giving up. That's where the 12-second runtime on a 28-character
input comes from.

TRE uses a parallel matcher built on Tagged-NFA simulation. The TRE
README says it best:

> The matching algorithm used in TRE uses linear worst-case time in the
> length of the text being searched, and quadratic worst-case time in
> the length of the used regular expression.
>
> A `regexec()` call never allocates memory from the heap. TRE allocates
> all the memory it needs during a `regcomp()` call.

Two consequences:

1. **No exponential explosion.** Total work is bounded by `O(M^2 · N)`
   where M is the regex length, N is the input length. There is no
   pattern shape that can blow this up.
2. **Bounded memory at match time.** Allocation happens once, in
   `regcomp`. The matcher itself runs out of a fixed stack frame.
   Adversarial input cannot drive memory usage up.

(TRE drops back to a back-tracking matcher only if you enable the
`REG_BACKTRACKING_MATCHER` flag, which we don't expose. POSIX backrefs
are also not supported by TRE — the only feature whose matching is
genuinely NP-complete.)

## Layout

```
.
├── src/tre_py/
│   ├── __init__.py     # ctypes binding: compile / Pattern.search / Match
│   └── deadline.py     # search_with_deadline() — wall-clock cap
├── tests/
│   ├── test_basic.py   # 10 tests — API surface
│   └── test_redos.py   # 12 tests — ReDoS robustness, scaling, memory
├── benchmark.py        # head-to-head re vs TRE
├── benchmark.out       # captured benchmark output
├── build_libtre.sh     # rebuild libtre.so from a fresh TRE clone
├── libtre.so           # built shared library (259K, ELF x86-64)
└── pyproject.toml
```

## Reproducing

Requires `uv`, plus autotools to rebuild `libtre.so`:

```sh
sudo apt-get install -y autoconf automake libtool gettext autopoint
./build_libtre.sh                       # writes libtre.so
uv sync                                 # installs pytest
uv run pytest tests/ -v                 # → 22 passed
PYTHONPATH=src uv run python benchmark.py
```

## API

```python
import tre_py

pat = tre_py.compile(r"(a+)(b+)", flags=tre_py.IGNORECASE)
m = pat.search("xxxAAABBBxxx")
m.group(0)   # "AAABBB"
m.group(1)   # "AAA"
m.span(2)    # (6, 9)
```

For belt-and-braces wall-clock protection:

```python
from tre_py.deadline import search_with_deadline, MatchTimeout

try:
    m = search_with_deadline(pat, attacker_input, timeout_s=1.0)
except MatchTimeout:
    ...
```

`search_with_deadline` runs the search on a worker thread and joins
with the timeout — it can't kill the worker (Python doesn't allow
that), but for TRE that's a non-issue: the matcher always returns,
quickly, on its own.

## What the tests prove

`tests/test_redos.py` is the meat of the demo. It contains:

- `test_python_re_explodes[…]` (×4) — sanity check that `re.match` on
  each evil pattern at n=24 takes >0.25s. This is the baseline that
  shows we're not fighting a strawman.
- `test_tre_handles_evil_pattern_within_budget[…]` (×4) — same patterns
  through TRE at n=100,000 must finish under a 2 s budget *and* return
  the correct answer (`None`, since each input is anchored and missing
  the final literal).
- `test_tre_scales_subexponentially` — measures `^(a+)+$` at three
  sizes that grow 10× at a time (1k → 10k → 100k chars). Asserts that
  runtime grows by less than 40× per 10× input growth. In practice it's
  ~10× — i.e. linear.
- `test_tre_match_is_memory_bounded` — sandwiches a single 1M-char
  match between two `getrusage(RUSAGE_SELF).ru_maxrss` reads and
  asserts the delta is under 20 MB. Most of that delta is the input
  string itself in wide-char form; TRE's own working set during match
  is constant.
- `test_match_with_deadline_returns_in_time` — TRE plus deadline
  wrapper on a real evil pattern returns inside its budget with the
  correct answer.
- `test_match_with_deadline_raises_on_timeout` — monkeypatches
  `Pattern.search` to sleep 2s; the deadline wrapper raises
  `MatchTimeout` after 0.2s.

```
$ uv run pytest tests/ -v
…
============================== 22 passed in 3.38s ==============================
```

## Limitations / decisions

- **Subset of TRE.** Only compile + search are exposed. No
  `findall`/`finditer`/`sub`, no approximate matching (`tre_regaexec`),
  no agrep-style cost parameters. Easy to add; orthogonal to the goal.
- **No back-references.** TRE doesn't support them at all — that's a
  design choice that's intrinsically tied to the linear-time guarantee
  (back-reference matching is NP-complete).
- **Wide-char internally.** The binding uses `tre_regwcomp` /
  `tre_regwnexec`, so spans are reported in Python str codepoints, not
  bytes. Works for both ASCII and Unicode inputs.
- **POSIX extended syntax.** `EXTENDED` is forced on by default — that
  matches what most users mean by "regex". POSIX BRE is reachable by
  passing `flags=0` and clearing the bit, but I don't expose that.
- **Threading-based deadline, not signals.** Works on Windows and from
  non-main threads. Cannot interrupt a runaway C call, but TRE never
  runs away — the deadline catches misconfiguration, not algorithmic
  blow-up.

## Files committed

- `notes.md` — running notes
- `README.md` — this file
- `src/tre_py/{__init__,deadline}.py` — binding and deadline wrapper
- `tests/test_{basic,redos}.py` — TDD tests
- `benchmark.py`, `benchmark.out` — benchmark and its captured output
- `build_libtre.sh` — reproducible build of libtre.so
- `libtre.so` — built shared library (259K, well under the 2MB cap)
- `pyproject.toml`, `uv.lock` — project + dependency lock
- `test_output.txt` — pytest output capture

The TRE source itself is *not* checked in — only the built `.so` and
the script that rebuilds it.
