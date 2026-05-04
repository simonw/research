"""ReDoS robustness tests for tre_py.

Each test does two things:

1. Verifies that Python's `re` module **does** suffer catastrophic
   backtracking on the well-known pathological pattern (or, when running
   `re` would just hang the test suite, asserts the slowdown via small
   inputs and a tight cutoff).
2. Verifies that the same pattern run through TRE finishes well within a
   fixed time budget on a much larger input.

The TRE inputs are deliberately many orders of magnitude larger than the
`re` ones — that is the point. The matcher runs in O(M^2 * N) worst case
in the *length of the input*, never exponential in it.
"""
from __future__ import annotations

import re
import resource
import time

import pytest

import tre_py


# A generous-but-finite per-call time budget for TRE matches in this file.
# On a typical CI box every TRE call below finishes in < 100 ms; we allow
# 2 s to leave headroom for slow shared runners.
TRE_BUDGET_S = 2.0


def _measure(fn, *a, **kw) -> tuple[float, object]:
    t0 = time.perf_counter()
    result = fn(*a, **kw)
    return time.perf_counter() - t0, result


# ---------------------------------------------------------------------------
# Catastrophic-backtracking patterns
# ---------------------------------------------------------------------------

# Classic "evil regex" examples. Each tuple is
#   (pattern, attack_input_factory, label)
# where attack_input_factory(n) builds an input of size n that maximizes
# the work the backtracking matcher has to do.
# All four entries below are confirmed exponential in CPython's `re` on
# this host: at n=24 each takes >= 0.4s. At n=28 each takes 5–60s.
EVIL = [
    # The canonical nested-quantifier example.
    (r"^(a+)+$",          lambda n: "a" * n + "!", "(a+)+$"),
    # Same shape with `*` instead of `+`.
    (r"^(a*)*$",          lambda n: "a" * n + "!", "(a*)*$"),
    # "Email-validator from hell"-style: char-class instead of `a`.
    (r"^([a-zA-Z]+)*$",   lambda n: "a" * n + "!", "([a-zA-Z]+)*$"),
    # Two overlapping `+` groups inside a `+`. Classic ReDoS lecture material.
    (r"^(x+x+)+$",        lambda n: "x" * n + "!", "(x+x+)+$"),
]


@pytest.mark.parametrize("pattern,make_input,label", EVIL, ids=[e[2] for e in EVIL])
def test_python_re_explodes(pattern, make_input, label):
    """Sanity-check: stock `re` is in fact catastrophic on this input.

    We pick an n small enough that we *can* let `re` finish, but the
    runtime grows so fast that even modest n is already much slower than
    TRE on a huge input. We assert that >= a quarter second is spent —
    this is a deliberately conservative threshold that any backtracker
    will blow past.
    """
    # Calibration on this host: at n=24 the cheapest evil pattern
    # already costs ~0.7s. At n=28 it would cost ~12s — too slow to run
    # in a test suite. n=24 is the sweet spot: large enough to be
    # unambiguously exponential, small enough to not hang.
    n = 24
    elapsed, _ = _measure(re.match, pattern, make_input(n))
    assert elapsed > 0.25, (
        f"expected re to be catastrophic on {label!r} at n={n}, "
        f"but it finished in {elapsed:.3f}s"
    )


@pytest.mark.parametrize("pattern,make_input,label", EVIL, ids=[e[2] for e in EVIL])
def test_tre_handles_evil_pattern_within_budget(pattern, make_input, label):
    """TRE finishes the same family of patterns on huge inputs in bounded time."""
    n = 100_000  # ~3500x larger than what we throw at `re`
    s = make_input(n)
    pat = tre_py.compile(pattern)
    elapsed, m = _measure(pat.search, s)
    assert elapsed < TRE_BUDGET_S, (
        f"TRE took {elapsed:.3f}s on {label!r} with n={n}, "
        f"budget was {TRE_BUDGET_S}s"
    )
    # All four "evil" patterns above are anchored and the input ends with
    # `!` (or lacks the trailing literal `b`), so a *correct* matcher
    # returns None. This is what we want: we proved the matcher is fast
    # *and* that it returns the right answer.
    # All four patterns above are anchored with a trailing literal that
    # the input does not satisfy, so a correct matcher returns None.
    assert m is None


# ---------------------------------------------------------------------------
# Linear scaling: doubling input does not blow up runtime exponentially
# ---------------------------------------------------------------------------

def test_tre_scales_subexponentially():
    """For an evil pattern, TRE's runtime should grow polynomially.

    We measure at three sizes that each differ by 10x. A backtracker would
    show wildly superlinear (effectively exponential) growth here. TRE's
    parallel matcher, with M and the alphabet fixed, is linear in N, so
    we expect roughly a 10x increase in runtime per 10x in input length.
    Use a generous bound (40x) to allow for noise, JIT warmup and so on.
    """
    pat = tre_py.compile(r"^(a+)+$")
    sizes = [1_000, 10_000, 100_000]
    times = []
    for n in sizes:
        s = "a" * n + "!"
        # Run a few times and take the min to reduce noise.
        best = min(_measure(pat.search, s)[0] for _ in range(3))
        times.append(best)

    # Each 10x growth in n should grow runtime by less than 40x.
    for prev, cur in zip(times, times[1:]):
        # Avoid div-by-zero if a measurement is too small.
        if prev < 1e-5:
            continue
        ratio = cur / prev
        assert ratio < 40, (
            f"TRE runtime grew by {ratio:.1f}x for 10x more input "
            f"(times={times}); not subexponential."
        )


# ---------------------------------------------------------------------------
# Memory: TRE does not allocate from the heap during matching
# ---------------------------------------------------------------------------

def test_tre_match_is_memory_bounded():
    """The TRE README claims `regexec` never allocates from the heap.

    We test the practical consequence: matching a much larger string does
    not balloon RSS. We compile the pattern once, do a warmup pass, then
    measure the delta in maximum RSS across an 8x larger input.
    """
    pat = tre_py.compile(r"^(a+)+$")
    # Warmup so any one-off allocations (Python str interning, ctypes
    # caches, etc.) happen before the measurement.
    pat.search("a" * 1_000 + "!")

    rss_before = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    pat.search("a" * 1_000_000 + "!")
    rss_after = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss

    # Allow the input string itself to land in RSS (it's ~4MB at 1Mchars
    # for Python 3 wide strings) plus ~5MB of working slack.
    delta_kb = rss_after - rss_before
    assert delta_kb < 20_000, f"RSS grew by {delta_kb} KB during match"


# ---------------------------------------------------------------------------
# A wall-clock-bounded match wrapper, in case you still want a hard cap
# ---------------------------------------------------------------------------

def test_match_with_deadline_returns_in_time():
    """Even if TRE were misconfigured, a thread-based deadline gives a
    hard ceiling on time spent in `search()`.
    """
    from tre_py.deadline import search_with_deadline

    pat = tre_py.compile(r"^(a+)+$")
    s = "a" * 200_000 + "!"
    elapsed, m = _measure(search_with_deadline, pat, s, 1.0)
    assert m is None  # correct answer
    assert elapsed < 1.5  # within the deadline + a small margin


def test_match_with_deadline_raises_on_timeout(monkeypatch):
    """If the search overruns, the deadline wrapper raises Timeout.

    To trigger it deterministically we mock `Pattern.search` to sleep.
    """
    import time as _time

    from tre_py.deadline import search_with_deadline, MatchTimeout

    pat = tre_py.compile(r".")

    def slow_search(_self, _s):
        _time.sleep(2.0)
        return None

    monkeypatch.setattr(tre_py.Pattern, "search", slow_search)
    with pytest.raises(MatchTimeout):
        search_with_deadline(pat, "x", 0.2)
