"""Runs an in-process Python engine (pyoxigraph, rdflib) in a child process.

Python-level engines cannot be interrupted, and some benchmark queries are pathological
for them (pyoxigraph's OPTIONAL on Q5 grows super-linearly with the data).  The child
holds the loaded store and times each execution itself, exactly like the runner does in
process (``perf_counter_ns``, GC paused), so IPC and pickling of the raw result are not
measured.  At the timeout the parent kills the child; the next execution starts a new
child and reloads the dataset (reload time is not reported).  The child also keeps the
engine's memory out of the harness process: closing a dataset frees it for certain.
"""

from __future__ import annotations

import gc
import multiprocessing
import time
from typing import Any

from ..checksum import ResultSet
from ..datasets import Dataset
from ..queries import BenchQuery
from .base import (Engine, EngineSettings, EngineUnavailable, LoadInfo, QueryTimeout,
                   QueryUnsupported, Timed)

_EXCEPTIONS = {cls.__name__: cls for cls in (EngineUnavailable, QueryUnsupported, QueryTimeout)}
_CLOSE_TIMEOUT_S = 60.0


def _inner(engine_name: str, settings: EngineSettings) -> Engine:
    from .oxigraph import PyoxigraphEngine
    from .rdflib_engine import RdflibEngine
    classes = {PyoxigraphEngine.name: PyoxigraphEngine, RdflibEngine.name: RdflibEngine}
    return classes[engine_name](settings)


def _serve(engine_name: str, settings: EngineSettings, conn) -> None:
    """The child's request loop: ``open``, ``execute`` and ``close`` messages."""
    engine = _inner(engine_name, settings)
    while True:
        try:
            request = conn.recv()
        except EOFError:
            return
        kind = request[0]
        try:
            if kind == "probe":
                conn.send(("ok", engine.probe()))
            elif kind == "open":
                engine.probe()
                conn.send(("ok", engine.open(request[1])))
            elif kind == "execute":
                _, query, mode = request
                gc.disable()
                try:
                    start = time.perf_counter_ns()
                    raw = engine.execute(query, mode)
                    elapsed_ms = (time.perf_counter_ns() - start) / 1e6
                finally:
                    gc.enable()
                conn.send(("ok", Timed(elapsed_ms, raw)))
            elif kind == "close":
                engine.close()
                conn.send(("ok", None))
                return
            else:
                conn.send(("error", "RuntimeError", f"unknown request {kind!r}"))
        except Exception as e:  # noqa: BLE001 - reported to the parent, which classifies it
            conn.send(("error", type(e).__name__, str(e)))


class IsolatedEngine(Engine):
    """Wraps a Python engine adapter so that it runs in a killable child process."""

    def __init__(self, engine_name: str, settings: EngineSettings) -> None:
        super().__init__(settings)
        self._local = _inner(engine_name, settings)  # probe/skip/decode need no store
        self.name = self._local.name
        self.short = self._local.short
        self._ctx = multiprocessing.get_context("spawn")
        self._proc = None
        self._conn = None
        self._dataset: Dataset | None = None
        self.restarts = 0

    def probe(self) -> str:
        return self._local.probe()

    def skip_reason(self, dataset: Dataset) -> str | None:
        return self._local.skip_reason(dataset)

    def mode_unavailable(self, mode: str) -> str | None:
        return self._local.mode_unavailable(mode)

    # ------------------------------------------------------------------ child process

    def _start(self) -> None:
        parent, child = self._ctx.Pipe()
        proc = self._ctx.Process(target=_serve, args=(self.name, self.settings, child),
                                 name=f"bench-{self.name}", daemon=True)
        proc.start()
        child.close()
        self._proc, self._conn = proc, parent

    def _kill(self) -> None:
        if self._proc is not None:
            self._proc.kill()
            self._proc.join()
        if self._conn is not None:
            self._conn.close()
        self._proc = self._conn = None

    def _call(self, request: tuple, timeout_s: float | None) -> Any:
        self._conn.send(request)
        if not self._conn.poll(timeout_s):
            self._kill()
            raise QueryTimeout(f"no answer within {timeout_s:g} s (worker process killed)")
        try:
            reply = self._conn.recv()
        except EOFError:
            code = self._proc.exitcode if self._proc else None
            self._kill()
            raise RuntimeError(f"{self.name} worker process died (exit code {code})") from None
        if reply[0] == "ok":
            return reply[1]
        _, kind, message = reply
        if kind in _EXCEPTIONS:
            raise _EXCEPTIONS[kind](message)
        raise RuntimeError(f"{kind}: {message}")

    def _load_timeout(self) -> float | None:
        return max(self.settings.timeout_s, 3600.0) if self.settings.timeout_s > 0 else None

    def open(self, dataset: Dataset) -> LoadInfo:
        self._kill()
        self._start()
        self._dataset = dataset
        return self._call(("open", dataset), self._load_timeout())

    def _ensure_loaded(self) -> None:
        if self._proc is None or not self._proc.is_alive():
            self._kill()
            self._start()
            self.restarts += 1
            self._call(("open", self._dataset), self._load_timeout())

    def close(self) -> None:
        if self._proc is not None and self._proc.is_alive():
            try:
                self._call(("close",), _CLOSE_TIMEOUT_S)
            except (QueryTimeout, RuntimeError, OSError):
                pass
        self._kill()
        self._dataset = None

    def execute(self, query: BenchQuery, mode: str) -> Timed:
        self._ensure_loaded()
        timeout = self.settings.timeout_s if self.settings.timeout_s > 0 else None
        return self._call(("execute", query, mode), timeout)

    def decode(self, query: BenchQuery, mode: str, raw: Any) -> int | ResultSet:
        return self._local.decode(query, mode, raw)
