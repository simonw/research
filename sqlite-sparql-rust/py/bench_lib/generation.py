"""Helpers shared by the dataset generators (``gen_social.py``, ``gen_bsbm.py``).

Sampling consumes only ``random.Random.random()`` draws (plus ``shuffle`` by callers),
so a seed reproduces the same data on every CPython 3 version.
"""

from __future__ import annotations

import bisect
import os
from collections.abc import Callable, Iterable
from dataclasses import dataclass
from typing import TextIO, TypeVar

T = TypeVar("T")


@dataclass(frozen=True)
class GenStats:
    """What a generator run produced."""

    entities: int
    triples: int
    bytes: int
    seconds: float


def write_atomically(path: str, produce: Callable[[TextIO], T]) -> T:
    """Calls ``produce(fh)`` on a temporary file next to ``path``, then renames it over
    ``path``, so an interrupted run never leaves a truncated dataset behind."""
    tmp = f"{path}.tmp{os.getpid()}"
    try:
        with open(tmp, "w", encoding="utf-8", newline="\n") as fh:
            result = produce(fh)
        os.replace(tmp, path)
    finally:
        if os.path.exists(tmp):
            os.unlink(tmp)
    return result


def cumulative(weights: Iterable[float]) -> list[float]:
    """Running sums of ``weights`` (for :func:`pick`)."""
    total = 0.0
    out = []
    for w in weights:
        if w < 0:
            raise ValueError("weights must be non-negative")
        total += w
        out.append(total)
    if not out or total <= 0.0:
        raise ValueError("weights must have a positive sum")
    return out


def zipf_cumulative(n: int, s: float) -> list[float]:
    """Cumulative Zipf weights ``1/(k+1)^s`` for ranks ``0..n-1``."""
    return cumulative(1.0 / (k + 1) ** s for k in range(n))


def pick(cum: list[float], u: float) -> int:
    """Index drawn from cumulative weights ``cum`` with a uniform ``u`` in [0, 1)."""
    i = bisect.bisect_right(cum, u * cum[-1])
    return i if i < len(cum) else len(cum) - 1


class PowerLawRanks:
    """Samples ranks in ``0..n-1`` with density proportional to ``(rank+1)^-s``.

    Inverse-CDF sampling of the continuous power law on ``[1, n+1)``: O(1) per draw,
    no tables, so it scales to millions of ranks.
    """

    def __init__(self, n: int, s: float) -> None:
        if n < 1:
            raise ValueError("need at least one rank")
        if not 0.0 < s < 1.0:
            raise ValueError("exponent must be in (0, 1)")
        self._n = n
        self._inv = 1.0 / (1.0 - s)
        self._span = (n + 1) ** (1.0 - s) - 1.0

    def sample(self, u: float) -> int:
        rank = int((1.0 + u * self._span) ** self._inv) - 1
        return min(max(rank, 0), self._n - 1)


def distinct_draws(k: int, draw: Callable[[], int], exclude: int = -1) -> list[int]:
    """``k`` distinct values from ``draw()`` (rejection sampling), never ``exclude``.

    The caller guarantees that at least ``k`` admissible values exist.
    """
    picked: list[int] = []
    while len(picked) < k:
        v = draw()
        if v != exclude and v not in picked:
            picked.append(v)
    return picked
