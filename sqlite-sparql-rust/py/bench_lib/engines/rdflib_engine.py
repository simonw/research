"""rdflib adapter: an in-memory ``rdflib.Graph`` (small datasets only).

rdflib evaluates ``p+``/``p*`` property paths by recursing once per hop, so Q13
(``:p1 :knows+ ?y``) exceeds Python's default recursion limit of 1000 on any non-trivial
graph.  Opening a dataset raises the limit to :data:`RECURSION_LIMIT`; CPython >= 3.12
keeps Python-to-Python calls off the C stack, and the adapter normally runs in its own
worker process (:mod:`bench_lib.engines.isolated`) anyway.
"""

from __future__ import annotations

import gc
import sys
import time
from typing import Any

from ..checksum import ResultSet
from ..datasets import Dataset
from ..queries import BenchQuery
from ..terms import XSD_STRING, Term, to_nt
from .base import (Engine, EngineUnavailable, LoadInfo, result_set_from_nt_rows,
                   result_set_from_ntriples_doc, result_set_from_sparql_json)

RECURSION_LIMIT = 100_000


def _import():
    try:
        import rdflib
    except ImportError as e:  # pragma: no cover - depends on the environment
        raise EngineUnavailable(f"rdflib is not installed ({e})") from e
    return rdflib


def term_to_nt(term, rdflib) -> str | None:
    """N-Triples text of an rdflib term (``n3()`` is not N-Triples: it uses long
    quotes for multi-line strings)."""
    if term is None:
        return None
    if isinstance(term, rdflib.URIRef):
        return to_nt(Term.iri(str(term)))
    if isinstance(term, rdflib.BNode):
        return to_nt(Term.bnode(str(term)))
    if isinstance(term, rdflib.Literal):
        dt = str(term.datatype) if term.datatype is not None else XSD_STRING
        return to_nt(Term.literal(str(term), dt, term.language))
    raise TypeError(f"unexpected rdflib term {term!r}")


class RdflibEngine(Engine):
    name = "rdflib"
    short = "rdflib"

    def __init__(self, settings) -> None:
        super().__init__(settings)
        self._rdflib = None
        self._graph = None

    def probe(self) -> str:
        self._rdflib = _import()
        return f"rdflib {self._rdflib.__version__}"

    def skip_reason(self, dataset: Dataset) -> str | None:
        limit = self.settings.rdflib_max
        if dataset.triples > limit:
            return f"dataset has {dataset.triples:,} triples > --rdflib-max {limit:,}"
        return None

    def open(self, dataset: Dataset) -> LoadInfo:
        rdflib = self._rdflib or _import()
        self._rdflib = rdflib
        if sys.getrecursionlimit() < RECURSION_LIMIT:
            sys.setrecursionlimit(RECURSION_LIMIT)
        graph = rdflib.Graph()
        start = time.perf_counter()
        graph.parse(dataset.path, format="nt")
        seconds = time.perf_counter() - start
        self._graph = graph
        return LoadInfo(seconds=seconds, triples=len(graph), details={"parse_s": seconds})

    def close(self) -> None:
        self._graph = None
        gc.collect()

    def execute(self, query: BenchQuery, mode: str) -> Any:
        rdflib = self._rdflib
        result = self._graph.query(query.text)
        if mode == "count":
            if query.form == "ask":
                return 1
            return sum(1 for _ in result)
        if mode == "full":
            if query.form == "ask":
                return bool(result.askAnswer)
            if query.form == "select":
                vars_ = [str(v) for v in result.vars]
                return vars_, [[term_to_nt(t, rdflib) for t in row] for row in result]
            return None, [[term_to_nt(t, rdflib) for t in triple] for triple in result]
        if mode == "json":
            if query.form in ("select", "ask"):
                return result.serialize(format="json")
            return result.serialize(format="nt")
        raise ValueError(f"unknown mode {mode!r}")

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
