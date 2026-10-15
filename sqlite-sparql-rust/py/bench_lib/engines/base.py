"""The engine adapter contract used by the runner.

An adapter is opened once per dataset (:meth:`Engine.open` loads the data and returns
:class:`LoadInfo`), then executes queries in the measurement *modes*:

``count``
    run the query fully and count solutions (or triples) without materialising terms;
``full``
    materialise every projected result term as one Python string (N-Triples text or
    the engine's equivalent ``str()``), so every engine does the same work per term;
``json``
    produce the serialised SPARQL-JSON results document (N-Triples for graph forms).

:meth:`Engine.execute` is the timed part and returns an opaque raw result;
:meth:`Engine.decode` turns the raw result of the last repetition into a row count
(``count``) or a canonical :class:`~bench_lib.checksum.ResultSet`, outside the timing.
Adapters signal outcomes with the exceptions below; anything else is an engine error.
"""

from __future__ import annotations

import json
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Any

from ..checksum import GRAPH_VARS, ResultSet
from ..datasets import Dataset
from ..queries import BenchQuery
from ..terms import canonical, canonical_nt, from_sparql_json, parse_nt

MODES = ("count", "full", "json")


class EngineUnavailable(Exception):
    """The engine (or one of its modes) cannot run here: missing library or function."""


class QueryUnsupported(Exception):
    """The engine rejects this query by design (e.g. outside the baseline's subset)."""


class QueryTimeout(Exception):
    """The execution was interrupted at the deadline."""


@dataclass
class Timed:
    """A raw result together with the execution time measured where it ran (an engine
    in a child process times itself, so IPC is not part of the measurement)."""

    elapsed_ms: float
    raw: Any


@dataclass
class LoadInfo:
    """How a dataset got into an engine.  ``seconds`` is the total; ``details`` holds
    the phases (load, ANALYZE, ...) and sizes in seconds/bytes."""

    seconds: float
    triples: int | None
    details: dict[str, Any] = field(default_factory=dict)
    shared_with: str | None = None

    def to_json(self) -> dict:
        return {"seconds": self.seconds, "triples": self.triples, "details": self.details,
                "shared_with": self.shared_with}


@dataclass
class EngineSettings:
    """Options the CLI passes to every adapter (each uses what applies to it).

    They change what the engine columns measure, so the run records them
    (:meth:`to_json`) next to the results.
    """

    work_dir: str
    ext_path: str | None = None
    reuse_db: bool = False
    sqlite_load: str = "auto"
    full_columns: str = "terms"
    native_config: dict[str, str] = field(default_factory=dict)
    rdflib_max: int = 100_000
    timeout_s: float = 600.0
    isolate: bool = True

    def to_json(self) -> dict:
        return {"work_dir": self.work_dir, "ext_path": self.ext_path, "reuse_db": self.reuse_db,
                "sqlite_load": self.sqlite_load, "full_columns": self.full_columns,
                "native_config": dict(self.native_config), "rdflib_max": self.rdflib_max,
                "timeout_s": self.timeout_s, "isolate": self.isolate}


class Engine(ABC):
    """Base class of the engine adapters."""

    #: Stable identifier used in the JSON output and on the command line.
    name: str = ""
    #: Column heading in the markdown tables.
    short: str = ""

    def __init__(self, settings: EngineSettings) -> None:
        self.settings = settings

    def probe(self) -> str:
        """Checks that the engine can run at all; returns its version description.

        Raises :class:`EngineUnavailable` otherwise.
        """
        return "unknown"

    def skip_reason(self, dataset: Dataset) -> str | None:
        """Why this engine sits out ``dataset`` (None = it runs)."""
        return None

    @abstractmethod
    def open(self, dataset: Dataset) -> LoadInfo:
        """Loads ``dataset``; raises :class:`EngineUnavailable` if the engine cannot."""

    @abstractmethod
    def close(self) -> None:
        """Releases the loaded dataset."""

    def mode_unavailable(self, mode: str) -> str | None:
        """Why ``mode`` cannot be measured on this engine (None = it can)."""
        return None if mode in MODES else f"unknown mode {mode!r}"

    @abstractmethod
    def execute(self, query: BenchQuery, mode: str) -> Any:
        """Runs ``query`` once in ``mode`` (the timed part).  May return :class:`Timed`
        when the engine measured the execution itself."""

    @abstractmethod
    def decode(self, query: BenchQuery, mode: str, raw: Any) -> int | ResultSet:
        """Turns ``execute``'s raw result into a count or a canonical result set."""


# --------------------------------------------------------------------------- helpers

def result_set_from_nt_rows(form: str, vars_: list[str], rows: list) -> ResultSet:
    """Builds a ResultSet from rows of N-Triples term strings (``None`` = unbound)."""
    if form in ("construct", "describe"):
        vars_ = GRAPH_VARS
        form = "graph"
    return ResultSet(form=form, vars=list(vars_),
                     rows=[tuple(canonical_nt(c) for c in row) for row in rows])


def result_set_from_sparql_json(form: str, text: str | bytes) -> ResultSet:
    """Parses a SPARQL 1.1 Query Results JSON document."""
    doc = json.loads(text)
    if "boolean" in doc:
        return ResultSet(form="ask", boolean=bool(doc["boolean"]))
    vars_ = list(doc.get("head", {}).get("vars", []))
    rows = []
    for binding in doc.get("results", {}).get("bindings", []):
        rows.append(tuple(canonical(from_sparql_json(binding[v])) if v in binding else canonical(None)
                          for v in vars_))
    if form not in ("select", "ask"):
        raise ValueError(f"SPARQL-JSON results for a {form} query")
    return ResultSet(form="select", vars=vars_, rows=rows)


def result_set_from_ntriples_doc(text: str | bytes) -> ResultSet:
    """Parses an N-Triples document (graph results serialised by an engine)."""
    if isinstance(text, bytes):
        text = text.decode("utf-8")
    rows = []
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        rows.append(tuple(canonical(t) for t in split_ntriples_line(line)))
    return ResultSet(form="graph", vars=list(GRAPH_VARS), rows=rows)


def split_ntriples_line(line: str) -> tuple:
    """Splits ``s p o .`` into three parsed terms."""
    if not line.endswith("."):
        raise ValueError(f"N-Triples statement without final dot: {line!r}")
    body = line[:-1].rstrip()
    terms = []
    i = 0
    for _ in range(2):
        j = body.index(">", i) + 1 if body[i] == "<" else body.index(" ", i)
        terms.append(parse_nt(body[i:j]))
        i = j
        while body[i] == " " or body[i] == "\t":
            i += 1
    terms.append(parse_nt(body[i:]))
    return tuple(terms)
