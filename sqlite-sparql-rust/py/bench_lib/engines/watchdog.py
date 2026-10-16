"""Deadlines for SQLite executions without per-execution threads.

The SQLite adapters stop an execution that passes ``--timeout`` with
``sqlite3_interrupt`` (``Connection.interrupt``), which must be called from another
thread.  Starting a ``threading.Timer`` per execution costs about 0.3 ms (an OS thread
plus a start handshake) *inside* the timed region, more than a whole point lookup, so
a :class:`Watchdog` keeps one long-lived daemon thread instead:

* :meth:`Watchdog.arm` and :meth:`Watchdog.disarm` only swap a small record under an
  uncontended lock (about a microsecond together).  They never start or wake the
  thread, except when it is idle;
* the thread sleeps until the deadline it last saw.  A later deadline (the timeout is
  constant, so every new deadline is later) is picked up when it wakes up, so a run of
  fast executions costs it one wake-up per ``timeout_s`` at most;
* the interrupt is delivered under the lock, and only if the same execution is still
  armed, so it can never hit the execution after the one that overran.
"""

from __future__ import annotations

import threading
import time
from collections.abc import Callable

#: An armed execution: (token, monotonic deadline, interrupt callable).  A plain tuple,
#: because building it is on the timed path.
_Armed = tuple[int, float, Callable[[], object]]


class Watchdog:
    """One daemon thread that calls an execution's ``interrupt`` at its deadline."""

    def __init__(self, timeout_s: float, name: str = "bench-watchdog") -> None:
        if timeout_s <= 0:
            raise ValueError("a watchdog needs a positive timeout")
        self.timeout_s = timeout_s
        self._name = name
        self._lock = threading.Lock()
        self._wake = threading.Condition(self._lock)
        self._armed: _Armed | None = None
        self._token = 0
        self._fired = 0  # token of the last interrupted execution
        self._sleeping_until: float | None = None  # None while idle
        self._closed = False
        self._thread: threading.Thread | None = None

    def start(self) -> None:
        """Starts the thread now (outside any timing); :meth:`arm` would otherwise."""
        with self._lock:
            self._start_locked()

    def _start_locked(self) -> None:
        if self._thread is None and not self._closed:
            self._thread = threading.Thread(target=self._run, name=self._name, daemon=True)
            self._thread.start()

    def arm(self, interrupt: Callable[[], object]) -> int:
        """Arms the deadline ``now + timeout_s`` for one execution; returns its token."""
        with self._lock:
            if self._thread is None:
                if self._closed:
                    raise RuntimeError("the watchdog is closed")
                self._start_locked()
            token = self._token = self._token + 1
            deadline = time.monotonic() + self.timeout_s
            self._armed = (token, deadline, interrupt)
            sleeping_until = self._sleeping_until
            if sleeping_until is None or deadline < sleeping_until:
                self._wake.notify()
            return token

    def disarm(self, token: int) -> bool:
        """Ends execution ``token``; True if it was interrupted at its deadline."""
        with self._lock:
            armed = self._armed
            if armed is not None and armed[0] == token:
                self._armed = None
            return self._fired == token

    def close(self) -> None:
        """Stops the thread (idempotent); arming a closed watchdog raises."""
        with self._lock:
            self._closed = True
            self._armed = None
            self._wake.notify()
            thread, self._thread = self._thread, None
        if thread is not None:
            thread.join()

    @property
    def closed(self) -> bool:
        return self._closed

    def _run(self) -> None:
        with self._lock:
            while not self._closed:
                armed = self._armed
                if armed is None:
                    self._sleeping_until = None
                    self._wake.wait()
                    continue
                token, deadline, interrupt = armed
                remaining = deadline - time.monotonic()
                if remaining > 0:
                    self._sleeping_until = deadline
                    self._wake.wait(remaining)
                    continue
                self._armed = None
                self._fired = token
                try:
                    interrupt()
                except Exception:  # noqa: BLE001 - a closed connection has nothing to stop
                    pass
            self._sleeping_until = None
