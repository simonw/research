"""The measurement protocol (docs/SPEC.md §12.4).

For every dataset the runner opens each engine once (load times are recorded
separately), then for every query of that dataset and every (engine, mode) cell:

1. ``warmup`` executions that do not count (default 1);
2. ``repeat`` timed executions (default 5), reduced so that one cell stays within
   ``cell_budget_s`` when the warm-up shows the query is slow (at least one run); a
   warm-up that alone exceeds the budget is taken as the single sample;
3. the median wall-clock time is reported.

Every execution's result, warm-up included, is checked outside the timing.  The first
(cold-cache) and the last execution are decoded into a row count, an order-insensitive
multiset checksum and, for totally ordered queries, a sequence checksum; every other
execution's rows are counted (:meth:`Engine.row_count`).  If the executions of one cell
disagree, the engine answered the same query differently on cold and warm caches (or
from run to run), and the cell is an *error* (:class:`InconsistentResults`).  The
previous execution's result is freed before the next timed call, never inside it.

The Python garbage collector is paused during each timed execution (as ``timeit``
does), so collection pauses from earlier work do not land in a measurement.  An
execution that exceeds ``timeout_s`` (interrupted by the SQLite engines, killed with
its worker process for the Python engines) marks the cell *timeout*, and the engine's
remaining modes for that query are skipped as timeouts too.

After each query the results of all (engine, mode) cells are cross-checked: row counts
must agree everywhere (``count`` mode included), checksums across all
result-producing modes, and sequence checksums for ``# ordered: true`` queries.

An engine that is *unavailable* (not installed or built) sits out quietly.  An engine
that is present but fails, in its probe or while loading a dataset, is an *error*:
the record says so and every cell it could not run is an error cell, so the run fails
(:meth:`RunResult.setup_errors`, :meth:`RunResult.errors`).
"""

from __future__ import annotations

import gc
import statistics
import sys
import time
from collections.abc import Callable
from dataclasses import dataclass, field
from typing import Any

from .checksum import ResultSet, multiset_checksum, sequence_checksum
from .datasets import Dataset
from .engines import Engine, EngineUnavailable, LoadInfo, QueryTimeout, QueryUnsupported, Timed
from .queries import BenchQuery

OK = "ok"
UNSUPPORTED = "unsupported"
UNAVAILABLE = "unavailable"
SKIPPED = "skipped"
ERROR = "error"
TIMEOUT = "timeout"

#: Message prefix of the error cells of an engine that failed its probe or its load.
NOT_RUN = "not run: "


class InconsistentResults(Exception):
    """The executions of one cell did not all return the same result."""


@dataclass
class RunConfig:
    """The protocol, plus what the report must record about how the engines were set
    up (``engine_settings``: :meth:`EngineSettings.to_json`) and invoked (``argv``)."""

    modes: list[str]
    warmup: int = 1
    repeat: int = 5
    timeout_s: float = 600.0
    cell_budget_s: float = 120.0
    engine_settings: dict = field(default_factory=dict)
    argv: list[str] = field(default_factory=list)

    def to_json(self) -> dict:
        return {"modes": self.modes, "warmup": self.warmup, "repeat": self.repeat,
                "timeout_s": self.timeout_s, "cell_budget_s": self.cell_budget_s,
                "engine_settings": self.engine_settings, "argv": self.argv}


@dataclass
class Measurement:
    """One (dataset, query, engine, mode) cell."""

    dataset: str
    query: str
    engine: str
    mode: str
    status: str
    times_ms: list[float] = field(default_factory=list)
    warmup_ms: list[float] = field(default_factory=list)
    rows: int | None = None
    checksum: str | None = None
    sequence: str | None = None
    message: str | None = None

    @property
    def median_ms(self) -> float | None:
        return statistics.median(self.times_ms) if self.status == OK and self.times_ms else None

    @property
    def where(self) -> str:
        return f"{self.engine}/{self.mode}"

    def to_json(self) -> dict:
        return {"dataset": self.dataset, "query": self.query, "engine": self.engine,
                "mode": self.mode, "status": self.status, "median_ms": self.median_ms,
                "times_ms": self.times_ms, "warmup_ms": self.warmup_ms, "rows": self.rows,
                "checksum": self.checksum, "sequence": self.sequence, "message": self.message}


@dataclass
class Check:
    """Cross-engine agreement for one query."""

    dataset: str
    query: str
    status: str  # ok | mismatch | single | none
    rows: int | None = None
    checksum: str | None = None
    participants: list[str] = field(default_factory=list)
    details: list[str] = field(default_factory=list)

    def to_json(self) -> dict:
        return {"dataset": self.dataset, "query": self.query, "status": self.status,
                "rows": self.rows, "checksum": self.checksum,
                "participants": self.participants, "details": self.details}


@dataclass
class EngineRecord:
    name: str
    short: str
    status: str
    version: str | None = None
    message: str | None = None

    def to_json(self) -> dict:
        return {"name": self.name, "short": self.short, "status": self.status,
                "version": self.version, "message": self.message}


@dataclass
class LoadRecord:
    dataset: str
    engine: str
    status: str
    info: LoadInfo | None = None
    message: str | None = None

    def to_json(self) -> dict:
        return {"dataset": self.dataset, "engine": self.engine, "status": self.status,
                "info": self.info.to_json() if self.info else None, "message": self.message}


@dataclass
class RunResult:
    config: RunConfig
    environment: dict = field(default_factory=dict)
    engines: list[EngineRecord] = field(default_factory=list)
    datasets: list[Dataset] = field(default_factory=list)
    queries: list[BenchQuery] = field(default_factory=list)
    loads: list[LoadRecord] = field(default_factory=list)
    measurements: list[Measurement] = field(default_factory=list)
    checks: list[Check] = field(default_factory=list)
    started: float = field(default_factory=time.time)
    finished: float | None = None

    def mismatches(self) -> list[Check]:
        return [c for c in self.checks if c.status == "mismatch"]

    def errors(self) -> list[Measurement]:
        return [m for m in self.measurements if m.status == ERROR]

    def setup_errors(self) -> list[str]:
        """Engines that failed their probe or failed to load a dataset."""
        out = [f"{e.name}: {e.message}" for e in self.engines if e.status == ERROR]
        out += [f"{ld.engine} on {ld.dataset}: {ld.message}" for ld in self.loads
                if ld.status == ERROR]
        return out

    def measurement(self, query: str, engine: str, mode: str) -> Measurement | None:
        for m in self.measurements:
            if m.query == query and m.engine == engine and m.mode == mode:
                return m
        return None

    def to_json(self) -> dict:
        return {
            "started": self.started, "finished": self.finished,
            "environment": self.environment, "config": self.config.to_json(),
            "engines": [e.to_json() for e in self.engines],
            "datasets": [d.to_json() for d in self.datasets],
            "queries": [{"id": q.id, "title": q.title, "dataset": q.dataset,
                         "ordered": q.ordered, "path": q.path, "sha256": q.sha256,
                         "text": q.text} for q in self.queries],
            "loads": [ld.to_json() for ld in self.loads],
            "measurements": [m.to_json() for m in self.measurements],
            "checks": [c.to_json() for c in self.checks],
        }


def _log(msg: str) -> None:
    print(msg, file=sys.stderr, flush=True)


@dataclass(frozen=True)
class _Outcome:
    """What one decoded execution is compared on."""

    rows: int
    checksum: str | None = None
    sequence: str | None = None


def _outcome(query: BenchQuery, value: int | ResultSet) -> _Outcome:
    if isinstance(value, int):
        return _Outcome(value)
    return _Outcome(len(value), multiset_checksum(value),
                    sequence_checksum(value) if query.ordered else None)


class _Executions:
    """The results of one cell's executions, checked outside the timing as they come
    in: the first one is decoded completely, the others are counted, and the last one
    is decoded completely by :meth:`finish`.  No raw or decoded result is kept."""

    def __init__(self, engine: Engine, query: BenchQuery, mode: str) -> None:
        self.engine, self.query, self.mode = engine, query, mode
        self.rows: list[int] = []
        self.first: _Outcome | None = None

    def add(self, raw: Any) -> None:
        if self.first is None:
            self.first = _outcome(self.query, self.engine.decode(self.query, self.mode, raw))
            self.rows.append(self.first.rows)
        else:
            self.rows.append(self.engine.row_count(self.query, self.mode, raw))

    def finish(self, last_raw: Any) -> _Outcome:
        """The checked outcome of the cell, given the last execution's raw result.

        Raises :class:`InconsistentResults` when the executions disagree."""
        if self.first is None:
            raise ValueError("no execution was recorded")
        if len(self.rows) == 1:
            return self.first
        last = _outcome(self.query, self.engine.decode(self.query, self.mode, last_raw))
        if last.rows != self.rows[-1]:  # a bug in the adapter, not in the engine
            raise ValueError(f"{type(self.engine).__name__}.row_count found {self.rows[-1]} "
                             f"rows where decode found {last.rows}")
        if len(set(self.rows)) > 1:
            raise InconsistentResults(
                f"results differ between executions: rows per execution {self.rows} "
                "(warm-up first)")
        if last != self.first:
            raise InconsistentResults(
                f"results differ between executions: the first returned "
                f"{self.first.checksum} (sequence {self.first.sequence}), the last "
                f"{last.checksum} (sequence {last.sequence})")
        return last


class Runner:
    def __init__(self, engines: list[Engine], config: RunConfig,
                 progress: Callable[[str], None] | None = _log,
                 on_update: Callable[[RunResult], None] | None = None) -> None:
        self.engines = engines
        self.config = config
        self.progress = progress or (lambda _msg: None)
        self.on_update = on_update or (lambda _result: None)

    # ------------------------------------------------------------------ top level

    def run(self, datasets: list[Dataset], queries: list[BenchQuery],
            environment: dict | None = None) -> RunResult:
        result = RunResult(config=self.config, environment=environment or {},
                           datasets=list(datasets), queries=list(queries))
        usable = self._probe_engines(result)
        for dataset in datasets:
            dataset_queries = [q for q in queries if q.dataset == dataset.name]
            if dataset_queries:
                self._run_dataset(result, dataset, dataset_queries, usable)
        result.finished = time.time()
        self.on_update(result)
        return result

    def _probe_engines(self, result: RunResult) -> list[Engine]:
        usable = []
        for engine in self.engines:
            try:
                version = engine.probe()
            except EngineUnavailable as e:
                result.engines.append(EngineRecord(engine.name, engine.short, UNAVAILABLE,
                                                   message=str(e)))
                self.progress(f"{engine.name}: unavailable ({e})")
                continue
            except Exception as e:  # noqa: BLE001 - a broken engine must not stop the run
                result.engines.append(EngineRecord(engine.name, engine.short, ERROR,
                                                   message=f"{type(e).__name__}: {e}"))
                self.progress(f"{engine.name}: ERROR ({e})")
                continue
            result.engines.append(EngineRecord(engine.name, engine.short, OK, version=version))
            unavailable = {m: engine.mode_unavailable(m) for m in self.config.modes}
            notes = "; ".join(f"{m} unavailable: {why}" for m, why in unavailable.items() if why)
            self.progress(f"{engine.name}: {version}" + (f" ({notes})" if notes else ""))
            usable.append(engine)
        return usable

    def _run_dataset(self, result: RunResult, dataset: Dataset, queries: list[BenchQuery],
                     usable: list[Engine]) -> None:
        opened: list[Engine] = []
        try:
            for engine in usable:
                record = self._open(engine, dataset)
                result.loads.append(record)
                if record.status == OK:
                    opened.append(engine)
            self.on_update(result)
            for query in queries:
                cells = []
                for engine in self.engines:
                    timed_out: str | None = None
                    for mode in self.config.modes:
                        if engine not in opened:
                            m = self._absent(engine, dataset, query, mode, result)
                        elif timed_out:
                            m = Measurement(dataset.name, query.id, engine.name, mode, TIMEOUT,
                                            message=f"skipped: timed out in {timed_out} mode")
                        else:
                            m = self._measure(engine, dataset, query, mode)
                            if m.status == TIMEOUT:
                                timed_out = mode
                        cells.append(m)
                        result.measurements.append(m)
                check = cross_check(dataset.name, query, cells)
                result.checks.append(check)
                if check.status == "mismatch":
                    self.progress(f"!!! MISMATCH {query.id}: " + "; ".join(check.details))
                self.on_update(result)
        finally:
            for engine in opened:
                engine.close()

    def _open(self, engine: Engine, dataset: Dataset) -> LoadRecord:
        reason = engine.skip_reason(dataset)
        if reason:
            self.progress(f"[{dataset.name}] {engine.name}: skipped ({reason})")
            return LoadRecord(dataset.name, engine.name, SKIPPED, message=reason)
        self.progress(f"[{dataset.name}] {engine.name}: loading {dataset.triples:,} triples ...")
        try:
            info = engine.open(dataset)
        except EngineUnavailable as e:
            self.progress(f"[{dataset.name}] {engine.name}: unavailable ({e})")
            return LoadRecord(dataset.name, engine.name, UNAVAILABLE, message=str(e))
        except Exception as e:  # noqa: BLE001 - a broken engine must not stop the run
            self.progress(f"[{dataset.name}] {engine.name}: load failed ({e})")
            return LoadRecord(dataset.name, engine.name, ERROR, message=f"{type(e).__name__}: {e}")
        if info.shared_with:
            self.progress(f"[{dataset.name}] {engine.name}: using the database built for "
                          f"{info.shared_with}")
        else:
            self.progress(f"[{dataset.name}] {engine.name}: loaded {info.triples} triples "
                          f"in {info.seconds:.2f} s")
        return LoadRecord(dataset.name, engine.name, OK, info=info)

    def _absent(self, engine: Engine, dataset: Dataset, query: BenchQuery, mode: str,
                result: RunResult) -> Measurement:
        """The cell of an engine that is not open for this dataset: it inherits the
        status of the engine's probe or load record (an error stays an error)."""
        status, message = UNAVAILABLE, None
        for record in result.engines:
            if record.name == engine.name and record.status != OK:
                status, message = record.status, record.message
        for load in result.loads:
            if load.dataset == dataset.name and load.engine == engine.name:
                status, message = load.status, load.message
        if status == ERROR:
            message = f"{NOT_RUN}{message}"
        return Measurement(dataset.name, query.id, engine.name, mode, status, message=message)

    # ------------------------------------------------------------------ one cell

    def _timed(self, engine: Engine, query: BenchQuery, mode: str) -> tuple[float, Any]:
        gc_was_enabled = gc.isenabled()
        gc.disable()
        try:
            start = time.perf_counter_ns()
            raw = engine.execute(query, mode)
            elapsed_ms = (time.perf_counter_ns() - start) / 1e6
        finally:
            if gc_was_enabled:
                gc.enable()
        if isinstance(raw, Timed):
            elapsed_ms, raw = raw.elapsed_ms, raw.raw
        if self.config.timeout_s > 0 and elapsed_ms > self.config.timeout_s * 1000.0:
            raise QueryTimeout(f"took {elapsed_ms / 1000:.1f} s > --timeout "
                               f"{self.config.timeout_s:g} s (not interruptible)")
        return elapsed_ms, raw

    def _over_budget(self, elapsed_ms: float) -> bool:
        return 0 < self.config.cell_budget_s * 1000.0 <= elapsed_ms

    def _repetitions(self, reference_ms: float) -> int:
        budget_ms = self.config.cell_budget_s * 1000.0
        if budget_ms <= 0 or reference_ms <= 0:
            return self.config.repeat
        return max(1, min(self.config.repeat, int(budget_ms // reference_ms)))

    def _measure(self, engine: Engine, dataset: Dataset, query: BenchQuery,
                 mode: str) -> Measurement:
        m = Measurement(dataset.name, query.id, engine.name, mode, OK)
        why = engine.mode_unavailable(mode)
        if why:
            m.status, m.message = UNAVAILABLE, why
            return m
        raw: Any = None
        executions = _Executions(engine, query, mode)
        try:
            for _ in range(self.config.warmup):
                raw = None  # free the previous result before, not inside, the timing
                elapsed, raw = self._timed(engine, query, mode)
                m.warmup_ms.append(elapsed)
                executions.add(raw)
            reps = self._repetitions(m.warmup_ms[-1]) if m.warmup_ms else self.config.repeat
            if m.warmup_ms and self._over_budget(m.warmup_ms[-1]):
                # Repeating a query slower than the whole cell budget tells nothing new.
                m.times_ms.append(m.warmup_ms.pop())
                reps = 0
            i = 0
            while i < reps:
                raw = None
                elapsed, raw = self._timed(engine, query, mode)
                m.times_ms.append(elapsed)
                executions.add(raw)
                if i == 0 and not m.warmup_ms:
                    reps = self._repetitions(elapsed)
                i += 1
            outcome = executions.finish(raw)
        except QueryUnsupported as e:
            m.status, m.message = UNSUPPORTED, str(e)
        except EngineUnavailable as e:
            m.status, m.message = UNAVAILABLE, str(e)
        except QueryTimeout as e:
            m.status, m.message = TIMEOUT, str(e) or f"interrupted after {self.config.timeout_s:g} s"
        except InconsistentResults as e:
            m.status, m.message = ERROR, str(e)
        except Exception as e:  # noqa: BLE001 - engine failures are results, not crashes
            m.status, m.message = ERROR, f"{type(e).__name__}: {e}"
        else:
            m.rows, m.checksum, m.sequence = outcome.rows, outcome.checksum, outcome.sequence
        self.progress(_describe(dataset.name, query, m))
        return m


def _describe(dataset: str, query: BenchQuery, m: Measurement) -> str:
    head = f"[{dataset}] {query.id:>3} {m.engine:<18} {m.mode:<5}"
    if m.status != OK:
        return f"{head} {m.status}" + (f": {m.message}" if m.message else "")
    return f"{head} {m.median_ms:10.2f} ms  ({len(m.times_ms)} runs, {m.rows:,} rows)"


def cross_check(dataset: str, query: BenchQuery, cells: list[Measurement]) -> Check:
    """Do all successful cells of one query agree?"""
    ok = [m for m in cells if m.status == OK]
    check = Check(dataset, query.id, "none", participants=[m.where for m in ok])
    if not ok:
        return check
    groups = {
        "rows": _group(ok, lambda m: m.rows),
        "checksum": _group([m for m in ok if m.checksum is not None], lambda m: m.checksum),
        "order": _group([m for m in ok if m.sequence is not None], lambda m: m.sequence),
    }
    check.rows = ok[0].rows
    checksums = groups["checksum"]
    check.checksum = next(iter(checksums)) if len(checksums) == 1 else None
    for what, group in groups.items():
        if len(group) > 1:
            check.details.append(f"{what} differ: " + ", ".join(
                f"{value} [{' '.join(where)}]" for value, where in group.items()))
    if check.details:
        check.status = "mismatch"
    elif len({m.engine for m in ok}) < 2:
        check.status = "single"
    else:
        check.status = "ok"
    return check


def _group(cells: list[Measurement], key: Callable[[Measurement], Any]) -> dict[Any, list[str]]:
    out: dict[Any, list[str]] = {}
    for m in cells:
        out.setdefault(key(m), []).append(m.where)
    return out
