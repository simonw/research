"""Wall-clock deadline wrapper for `Pattern.search`.

Even though TRE is provably non-exponential, you may want a hard cap on
wall-clock time spent in any one `search()` — for example when matching
against attacker-supplied input on a request hot path.

`search_with_deadline` runs the match on a worker thread and joins with a
timeout. If the worker is still running when the deadline expires, we
raise `MatchTimeout`. The worker thread itself cannot be forcibly killed
(Python doesn't support that), but TRE is well-behaved and typically
returns within milliseconds, so a timeout means something is genuinely
wrong, not stuck in a tight loop forever.

The same trick gives you a clean way to cap memory / CPU at the OS level
on Unix: the caller can spawn a child process and pair this deadline with
`resource.setrlimit(RLIMIT_AS, ...)` for a true sandbox. We don't do that
here because the in-process variant is enough for ReDoS protection.
"""
from __future__ import annotations

import threading
from typing import Optional

from . import Match, Pattern


class MatchTimeout(Exception):
    """Raised when `search_with_deadline` exceeds its budget."""


def search_with_deadline(
    pattern: Pattern, string: str, timeout_s: float
) -> Optional[Match]:
    result: list[Optional[Match]] = [None]
    error: list[BaseException] = []

    def _run():
        try:
            result[0] = pattern.search(string)
        except BaseException as e:  # noqa: BLE001
            error.append(e)

    t = threading.Thread(target=_run, name="tre-deadline", daemon=True)
    t.start()
    t.join(timeout_s)
    if t.is_alive():
        raise MatchTimeout(
            f"tre_py.search exceeded {timeout_s}s deadline"
        )
    if error:
        raise error[0]
    return result[0]
