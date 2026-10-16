"""pyoxigraph adapter: an in-memory ``pyoxigraph.Store``, bulk-loaded.

``count`` mode runs :attr:`BenchQuery.count_text`, so pyoxigraph counts the solutions
itself, as ``sparql_count`` does.  Iterating ``Store.query(q)`` instead (§12.4's
``len(list(q))``, ``--reference-count iterate``) also decodes every term and builds a
``QuerySolution`` per row: at 200K persons that took 1.95x as long on Q11 (128K rows),
1.55x on Q1 (20K rows), and made no measurable difference on queries with few rows.
"""

from __future__ import annotations

import gc
import time
from typing import Any

from ..checksum import ResultSet
from ..datasets import Dataset
from ..queries import BenchQuery
from .base import (Engine, EngineUnavailable, LoadInfo, result_set_from_nt_rows,
                   result_set_from_ntriples_doc, result_set_from_sparql_json,
                   serialised_row_count)


def _import():
    try:
        import pyoxigraph
    except ImportError as e:  # pragma: no cover - depends on the environment
        raise EngineUnavailable(f"pyoxigraph is not installed ({e})") from e
    return pyoxigraph


class PyoxigraphEngine(Engine):
    name = "pyoxigraph"
    short = "pyoxigraph"

    def __init__(self, settings) -> None:
        super().__init__(settings)
        self._ox: Any = None
        self._store: Any = None

    def probe(self) -> str:
        self._ox = _import()
        return f"pyoxigraph {self._ox.__version__}"

    def open(self, dataset: Dataset) -> LoadInfo:
        ox = self._ox or _import()
        self._ox = ox
        store = ox.Store()
        start = time.perf_counter()
        store.bulk_load(path=dataset.path, format=ox.RdfFormat.N_TRIPLES)
        seconds = time.perf_counter() - start
        self._store = store
        return LoadInfo(seconds=seconds, triples=len(store), details={"bulk_load_s": seconds})

    def close(self) -> None:
        self._store = None
        gc.collect()

    def execute(self, query: BenchQuery, mode: str) -> Any:
        if mode == "count":
            return self._count(query)
        results = self._store.query(query.text)
        if mode == "full":
            if query.form == "ask":
                return bool(results)
            if query.form == "select":
                vars_ = [v.value for v in results.variables]
                return vars_, [[None if t is None else str(t) for t in sol] for sol in results]
            return None, [[str(t.subject), str(t.predicate), str(t.object)] for t in results]
        if mode == "json":
            if query.form in ("select", "ask"):
                return results.serialize(format=self._ox.QueryResultsFormat.JSON)
            return results.serialize(format=self._ox.RdfFormat.N_TRIPLES)
        raise ValueError(f"unknown mode {mode!r}")

    def _count(self, query: BenchQuery) -> int:
        counted = query.count_text if self.settings.reference_count == "aggregate" else None
        if counted is not None:
            (solution,) = self._store.query(counted)
            return int(solution[0].value)
        results = self._store.query(query.text)
        if query.form == "ask":
            return 1
        return sum(1 for _ in results)

    def row_count(self, query: BenchQuery, mode: str, raw: Any) -> int:
        if mode == "count":
            return raw
        if mode == "full":
            return 1 if query.form == "ask" else len(raw[1])
        return serialised_row_count(query.form, raw)

    def decode(self, query: BenchQuery, mode: str, raw: Any) -> int | ResultSet:
        if mode == "count":
            return raw
        if mode == "full":
            if query.form == "ask":
                return ResultSet(form="ask", boolean=raw)
            vars_, rows = raw
            return result_set_from_nt_rows(query.form, vars_, rows)
        if query.form in ("select", "ask"):
            return result_set_from_sparql_json(query.form, raw)
        return result_set_from_ntriples_doc(raw)
