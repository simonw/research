"""A stand-in for a ``sqlite3`` connection with libsparqlite loaded.

It implements exactly the SQL the adapters issue, with the result shapes of
docs/SPEC.md §8.2/§8.3 (``sparql_rows`` columns ``n, v0..v15, t0..t15, id0..id15,
vars, json``, and its options argument, of which ``engine`` matters here; native
values per §8.4), backed by pyoxigraph.  Databases are kept in a
class-level registry keyed by path so that several connections share one store, like
connections to one SQLite file.  Python's sqlite3 cannot define virtual tables, hence
a fake connection rather than Python-defined SQL functions.
"""

from __future__ import annotations

import json
import re
import sqlite3

import pyoxigraph as ox

XSD = "http://www.w3.org/2001/XMLSchema#"
ALL_FUNCTIONS = frozenset({
    "sparql_version", "sparql_init", "sparql_load", "sparql_load_file", "sparql_config",
    "sparql_analyze", "sparql_count", "sparql_query", "sparql_count_sql", "sparql_query_sql",
    "sparql_rows"})
ROWS_COLUMNS = (["n"] + [f"v{k}" for k in range(16)] + [f"t{k}" for k in range(16)]
                + [f"id{k}" for k in range(16)] + ["vars", "json"])


class FakeCursor:
    def __init__(self, rows: list[tuple], columns: list[str] | None = None) -> None:
        self._rows = rows
        self.description = [(c, None, None, None, None, None, None) for c in (columns or ["x"])]

    def fetchone(self):
        return self._rows[0] if self._rows else None

    def fetchall(self):
        return list(self._rows)


def native_value(term):
    """§8.4: the SQL value of a term."""
    if term is None:
        return None
    if isinstance(term, ox.NamedNode):
        return term.value
    if isinstance(term, ox.BlankNode):
        return f"_:{term.value}"
    dt = term.datatype.value
    if dt == XSD + "integer":
        return int(term.value)
    if dt in (XSD + "decimal", XSD + "double", XSD + "float"):
        return float(term.value)
    if dt == XSD + "boolean":
        return 1 if term.value in ("true", "1") else 0
    return term.value


def json_binding(term) -> dict:
    if isinstance(term, ox.NamedNode):
        return {"type": "uri", "value": term.value}
    if isinstance(term, ox.BlankNode):
        return {"type": "bnode", "value": term.value}
    out = {"type": "literal", "value": term.value}
    if term.language:
        out["xml:lang"] = term.language
    elif term.datatype.value != XSD + "string":
        out["datatype"] = term.datatype.value
    return out


class FakeSparqliteConnection:
    """See the module docstring.  ``functions`` limits what is "registered";
    ``baseline_rejects`` makes the baseline (``*_sql`` functions and ``sparql_rows``
    with ``{"engine":"sql"}``) raise ``unsupported`` for queries containing that text;
    ``rows_error`` makes ``sparql_rows`` fail with that message for queries containing
    ``rows_error_on``."""

    stores: dict[str, ox.Store] = {}
    log: list[str] = []

    def __init__(self, path: str, functions=ALL_FUNCTIONS, baseline_rejects: str = ":knows+",
                 rows_error: str | None = None, rows_error_on: str = "") -> None:
        if path == ":memory:" or path not in self.stores:
            self.stores[path] = ox.Store()
        if path != ":memory:":
            open(path, "a").close()  # like SQLite, opening creates the database file
        self.path = path
        self.functions = set(functions)
        self.baseline_rejects = baseline_rejects
        self.rows_error = rows_error
        self.rows_error_on = rows_error_on
        self.allow_files = False
        self.loaded = False
        self.closed = False
        self.interrupted = 0

    # sqlite3.Connection surface ----------------------------------------------------
    def enable_load_extension(self, flag: bool) -> None:
        pass

    def load_extension(self, path: str) -> None:
        self.loaded = True

    def interrupt(self) -> None:
        self.interrupted += 1

    def close(self) -> None:
        self.closed = True

    @property
    def store(self) -> ox.Store:
        return self.stores[self.path]

    def _need(self, name: str) -> None:
        if not self.loaded or name not in self.functions:
            raise sqlite3.OperationalError(f"no such function: {name}")

    def execute(self, sql: str, params: tuple = ()) -> FakeCursor:
        self.log.append(sql)
        s = " ".join(sql.split())
        if s.startswith("PRAGMA") or s == "ANALYZE main":
            return FakeCursor([])
        if s.startswith("SELECT count(*) FROM pragma_function_list"):
            return FakeCursor([(1 if params[0] in self.functions else 0,)])
        m = re.fullmatch(r"SELECT (sparql_\w+)\((.*)\)", s)
        if m:
            return self._scalar(m.group(1), m.group(2), params)
        m = re.fullmatch(r"SELECT (.+) FROM sparql_rows\(\?(, \?)?\)", s)
        if m:
            options = json.loads(params[1]) if m.group(2) else {}
            return self._rows(m.group(1), params[0], options)
        raise sqlite3.OperationalError(f"fake: unexpected SQL {sql!r}")

    # functions --------------------------------------------------------------------
    def _scalar(self, name: str, args: str, params: tuple) -> FakeCursor:
        self._need(name)
        if name == "sparql_version":
            return FakeCursor([("sparqlite fake (SQLite 3.45.1)",)])
        if name == "sparql_init":
            return FakeCursor([(1,)])
        if name == "sparql_analyze":
            return FakeCursor([(json.dumps({"quads": len(self.store)}),)])
        if name == "sparql_config":
            literal = re.fullmatch(r"'(\w+)'(?:, (\d+))?", args)
            key, value = (literal.group(1), literal.group(2)) if literal else (params + (None,))[:2]
            if key == "allow_files" and value is not None:
                self.allow_files = str(value) == "1"
            return FakeCursor([(str(value),)])
        if name in ("sparql_load", "sparql_load_file"):
            data = params[0]
            if name == "sparql_load_file":
                if not self.allow_files:
                    raise sqlite3.OperationalError("sparql: invalid argument: files not allowed")
                before = len(self.store)
                self.store.bulk_load(path=data, format=ox.RdfFormat.N_TRIPLES)
            else:
                before = len(self.store)
                self.store.bulk_load(data, format=ox.RdfFormat.N_TRIPLES)
            return FakeCursor([(len(self.store) - before,)])
        query = params[0]
        if name.endswith("_sql") and self.baseline_rejects and self.baseline_rejects in query:
            raise sqlite3.OperationalError("sparql: unsupported: baseline: property paths")
        if name in ("sparql_count", "sparql_count_sql"):
            results = self.store.query(query)
            if isinstance(results, ox.QueryBoolean):
                return FakeCursor([(1,)])
            return FakeCursor([(sum(1 for _ in results),)])
        if name in ("sparql_query", "sparql_query_sql"):
            fmt = params[1] if len(params) > 1 else "json"
            results = self.store.query(query)
            if fmt == "json":
                text = results.serialize(format=ox.QueryResultsFormat.JSON).decode()
            else:
                text = results.serialize(format=ox.RdfFormat.N_TRIPLES).decode()
            return FakeCursor([(text,)])
        raise sqlite3.OperationalError(f"fake: unhandled function {name}")

    def _rows(self, columns: str, query: str, options: dict) -> FakeCursor:
        if "sparql_rows" not in self.functions or not self.loaded:
            raise sqlite3.OperationalError("no such table: sparql_rows")
        if self.rows_error and self.rows_error_on in query:
            raise sqlite3.OperationalError(self.rows_error)
        if (options.get("engine") == "sql" and self.baseline_rejects
                and self.baseline_rejects in query):
            raise sqlite3.OperationalError("sparql: unsupported: baseline: property paths")
        results = self.store.query(query)
        if isinstance(results, ox.QueryBoolean):
            vars_, solutions = [], [[ox.Literal("true" if bool(results) else "false",
                                                datatype=ox.NamedNode(XSD + "boolean"))]]
        elif isinstance(results, ox.QuerySolutions):
            vars_ = [v.value for v in results.variables]
            solutions = [list(sol) for sol in results]
        else:
            vars_ = []
            solutions = [[t.subject, t.predicate, t.object] for t in results]
        full = []
        for n, sol in enumerate(solutions, start=1):
            terms = list(sol) + [None] * (16 - len(sol))
            v = [native_value(t) for t in terms[:16]]
            t = [None if x is None else str(x) for x in terms[:16]]
            ids = [None if x is None else (hash(str(x)) & 0xFFFFFFF) << 4 for x in terms[:16]]
            # ASK and graph rows carry no variables, so no bindings object either.
            binding = {name: json_binding(term)
                       for name, term in zip(vars_, sol if vars_ else [], strict=True)
                       if term is not None}
            full.append(tuple([n] + v + t + ids + [json.dumps(vars_), json.dumps(binding)]))
        if columns == "*":
            return FakeCursor(full, list(ROWS_COLUMNS))
        wanted = [c.strip() for c in columns.split(",")]
        idx = [ROWS_COLUMNS.index(c) for c in wanted]
        return FakeCursor([tuple(row[i] for i in idx) for row in full], wanted)


def connect_factory(**kwargs):
    """A ``connect(path)`` callable for :class:`SparqliteLibrary`."""
    def connect(path: str) -> FakeSparqliteConnection:
        return FakeSparqliteConnection(path, **kwargs)
    return connect
