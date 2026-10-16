"""sparqlite adapters (docs/SPEC.md §8, §9, §12.4).

Both SQLite engines run on the same file database per dataset, built once by
:class:`SparqliteDatabase`:

* ``PRAGMA page_size=8192``, ``journal_mode=DELETE`` and ``cache_size=-524288`` are set
  before ``SELECT sparql_init()``;
* the data is loaded with ``sparql_load_file(path, 'ntriples')`` (after
  ``sparql_config('allow_files', 1)``) or ``sparql_load(blob, 'ntriples')``;
* then ``ANALYZE main`` (so the baseline's SQL gets ``sqlite_stat1``) and
  ``SELECT sparql_analyze()``.

What each mode measures, per engine:

========  ===================================  ==============================================
mode      sparqlite-native                     sparqlite-baseline
========  ===================================  ==============================================
count     ``sparql_count(q)``                  ``sparql_count_sql(q)``
full      ``SELECT t0..t{n-1} FROM             ``SELECT t0..t{n-1} FROM
          sparql_rows(q)`` fetched             sparql_rows(q, '{"engine":"sql"}')`` fetched
json      ``sparql_query(q, 'json')``          ``sparql_query_sql(q, 'json')``
========  ===================================  ==============================================

Full mode fetches one N-Triples string per projected term, which is what pyoxigraph's
``str()`` per term and rdflib's N-Triples text per term produce, so the three columns
of that table do the same work.  The baseline goes through the same ``sparql_rows``
output path with the §8.3 ``engine`` option (§9: same storage, ids, dictionary and
output path, so only the evaluation strategy differs).  ``--full-columns star`` fetches
``SELECT *`` instead (all 51 columns decoded per row, the literal wording of §12.4),
for both SQLite engines.  Connections run in autocommit mode, so every call gets the
extension's own read scope (§8.6).

Outcomes are classified strictly (:func:`classify_error`): only a library that cannot
be loaded or a function, table or module that does not exist is *unavailable*; only
the baseline may answer *unsupported* (§9 subset); an interrupt is a *timeout* only
when the watchdog delivered it.  Every other failure, at build time, in the probe or
per query, is an *error* and fails the run.
"""

from __future__ import annotations

import hashlib
import json
import os
import sqlite3
import time
from collections.abc import Callable, Sequence
from typing import Any

from ..checksum import GRAPH_VARS, ResultSet
from ..datasets import Dataset
from ..queries import BenchQuery
from ..terms import UNBOUND, canonical, canonical_nt, from_sparql_json
from .base import (MODES, Engine, EngineSettings, EngineUnavailable, LoadInfo, QueryTimeout,
                   QueryUnsupported, result_set_from_ntriples_doc, result_set_from_sparql_json,
                   serialised_row_count)
from .watchdog import Watchdog

PAGE_SIZE = 8192
CACHE_SIZE_KIB = 524288
MAX_COLUMNS = 16  # sparql_rows exposes v0..v15 / t0..t15 / id0..id15
FULL_COLUMNS = ("terms", "star")

#: §8.3 options of ``sparql_rows`` that run a query through the baseline translator.
BASELINE_ROWS_OPTIONS = '{"engine":"sql"}'

PROBE_QUERY = "SELECT * WHERE { ?s ?p ?o }"
PROBE_TRIPLE = "<http://example.org/s> <http://example.org/p> <http://example.org/o> .\n"

#: sq-core's rendering of ``Error::Unsupported`` (§10 error.rs).
UNSUPPORTED_PREFIX = "sparql: unsupported:"
#: sq-core's rendering of ``Error::Timeout`` (the extension's own ``timeout_ms``).
TIMEOUT_PREFIX = "sparql: query timeout"

ConnectFn = Callable[[str], Any]


class ExtensionError(RuntimeError):
    """The extension is present but failed: a bug to report, never a reason to skip."""


def default_connect(path: str) -> sqlite3.Connection:
    return sqlite3.connect(path, isolation_level=None)


def extension_candidates(repo_root: str) -> list[str]:
    """Where to look for ``libsparqlite`` when neither ``--ext`` nor
    ``$SPARQLITE_EXT`` is given."""
    return [os.path.join(repo_root, "ext", "target", "release", "libsparqlite")]


def _library_file(path: str) -> str | None:
    for candidate in (path, path + ".so", path + ".dylib", path + ".dll"):
        if os.path.isfile(candidate):
            return candidate
    return None


def is_missing(message: str) -> bool:
    """Does a SQLite error say that a function, table-valued function or module is not
    registered (as opposed to failing while it ran)?"""
    low = message.strip().lower()
    return (low.startswith("no such function") or low.startswith("no such table: sparql_")
            or low.startswith("no such module"))


def classify_error(err: sqlite3.Error, *, fired: bool = False,
                   unsupported_ok: bool = False) -> Exception:
    """Maps a SQLite error raised by an extension call to the adapter outcome.

    ``fired``: the watchdog interrupted this execution at the ``--timeout`` deadline.
    ``unsupported_ok``: the caller is the baseline, whose translator rejects queries
    outside its subset with ``sparql: unsupported: ...`` (§9); for the native engine,
    which implements all of SPARQL 1.1, that is an error like any other.
    """
    msg = str(err).strip()
    if is_missing(msg):
        return EngineUnavailable(msg)
    if fired:
        return QueryTimeout(f"interrupted at the --timeout deadline ({msg})")
    if msg.lower().startswith(TIMEOUT_PREFIX):
        return QueryTimeout(msg)  # sparql_config('timeout_ms', N) via --native-config
    if unsupported_ok and msg.startswith(UNSUPPORTED_PREFIX):
        return QueryUnsupported(msg)
    return RuntimeError(msg)


class SparqliteLibrary:
    """The extension shared library: loading it into connections and probing it once."""

    #: (probe name, SQL, parameters) of every call the adapters rely on, in probe order.
    PROBE_CALLS: tuple[tuple[str, str, tuple], ...] = (
        ("sparql_config", "SELECT sparql_config('allow_files')", ()),
        ("sparql_load", "SELECT sparql_load(?, 'ntriples')", (PROBE_TRIPLE,)),
        ("sparql_analyze", "SELECT sparql_analyze()", ()),
        ("sparql_count", "SELECT sparql_count(?)", (PROBE_QUERY,)),
        ("sparql_rows", "SELECT * FROM sparql_rows(?)", (PROBE_QUERY,)),
        ("sparql_rows_sql", "SELECT * FROM sparql_rows(?, ?)", (PROBE_QUERY, BASELINE_ROWS_OPTIONS)),
        ("sparql_query", "SELECT sparql_query(?, 'json')", (PROBE_QUERY,)),
        ("sparql_count_sql", "SELECT sparql_count_sql(?)", (PROBE_QUERY,)),
        ("sparql_query_sql", "SELECT sparql_query_sql(?, 'json')", (PROBE_QUERY,)),
    )

    def __init__(self, path: str | None, connect: ConnectFn = default_connect) -> None:
        self.requested = path
        self._connect = connect
        self._probe: dict[str, Any] | None = None

    @property
    def file(self) -> str | None:
        return _library_file(self.requested) if self.requested else None

    def connect(self, db_path: str) -> Any:
        """Opens ``db_path`` with the extension loaded (autocommit mode)."""
        lib = self.file
        if lib is None:
            raise EngineUnavailable(
                f"sparqlite extension not found at {self.requested!r} "
                "(build ext/ and pass --ext or set SPARQLITE_EXT)" if self.requested else
                "no sparqlite extension given (--ext or SPARQLITE_EXT)")
        conn = self._connect(db_path)
        try:
            conn.enable_load_extension(True)
            conn.load_extension(lib)
            conn.enable_load_extension(False)
        except (sqlite3.Error, AttributeError) as e:
            conn.close()
            raise EngineUnavailable(f"cannot load {lib}: {e}") from e
        return conn

    def fingerprint(self) -> str:
        lib = self.file
        if lib is None:
            return "missing"
        h = hashlib.sha256()
        with open(lib, "rb") as fh:
            while chunk := fh.read(1 << 20):
                h.update(chunk)
        return h.hexdigest()

    def probe(self) -> dict[str, Any]:
        """Loads the extension into ``:memory:`` and calls everything once.

        Result keys: ``version``; ``error`` (the library cannot be used at all: not
        loadable, or ``sparql_init`` not registered); ``init_error`` (``sparql_init()``
        is registered but failed); ``calls``: name -> None if the call worked, else its
        error message; ``missing``: the names whose function or table does not exist.
        Only ``missing`` makes a mode unavailable: a registered call that fails here
        will fail on the benchmark queries too, where it is reported as an error.
        """
        if self._probe is None:
            self._probe = self._run_probe()
        return self._probe

    def _run_probe(self) -> dict[str, Any]:
        out: dict[str, Any] = {"version": None, "error": None, "init_error": None,
                               "calls": {}, "missing": []}
        try:
            conn = self.connect(":memory:")
        except EngineUnavailable as e:
            out["error"] = str(e)
            return out
        calls: dict[str, str | None] = out["calls"]
        missing: list[str] = out["missing"]

        def attempt(name: str, sql: str, params: tuple = ()) -> list | None:
            try:
                rows = conn.execute(sql, params).fetchall()
            except sqlite3.Error as e:
                calls[name] = str(e)
                if is_missing(str(e)):
                    missing.append(name)
                return None
            calls[name] = None
            return rows

        try:
            rows = attempt("sparql_version", "SELECT sparql_version()")
            out["version"] = rows[0][0] if rows else None
            attempt("sparql_init", "SELECT sparql_init()")
            if "sparql_init" in missing:
                out["error"] = f"sparql_init() is not registered: {calls['sparql_init']}"
                return out
            if calls["sparql_init"] is not None:
                out["init_error"] = f"sparql_init() failed: {calls['sparql_init']}"
                return out
            if self.has_function(conn, "sparql_load_file"):
                calls["sparql_load_file"] = None
            else:
                calls["sparql_load_file"] = "not registered"
                missing.append("sparql_load_file")
            for name, sql, params in self.PROBE_CALLS:
                attempt(name, sql, params)
            return out
        finally:
            conn.close()

    def has_function(self, conn: Any, name: str) -> bool:
        try:
            row = conn.execute("SELECT count(*) FROM pragma_function_list WHERE name = ?",
                               (name,)).fetchone()
            return bool(row and row[0])
        except sqlite3.Error:
            return False


def _set_config(conn: Any, key: str, value: str) -> None:
    v: Any = int(value) if value.lstrip("-").isdigit() else value
    conn.execute("SELECT sparql_config(?, ?)", (key, v)).fetchone()


def _remove_db(path: str) -> None:
    for suffix in ("", "-journal", "-wal", "-shm", ".json"):
        try:
            os.unlink(path + suffix)
        except FileNotFoundError:
            pass


def _setup_error(what: str, err: sqlite3.Error) -> Exception:
    """A failure while building or configuring: unavailable only if something is not
    registered, otherwise an :class:`ExtensionError`."""
    message = f"{what} failed: {err}"
    return EngineUnavailable(message) if is_missing(str(err)) else ExtensionError(message)


class SparqliteDatabase:
    """Builds (or reuses) the file database of each dataset, once per run."""

    def __init__(self, library: SparqliteLibrary, settings: EngineSettings) -> None:
        self.library = library
        self.settings = settings
        self._built: dict[str, tuple[str, LoadInfo, str]] = {}
        self._failed: dict[str, Exception] = {}

    def path_for(self, dataset: Dataset) -> str:
        return os.path.join(self.settings.work_dir, f"{dataset.name}-{dataset.fingerprint[:12]}.db")

    def ensure(self, dataset: Dataset, requester: str) -> tuple[str, LoadInfo]:
        """The database for ``dataset``; the first requester builds it, later ones get
        a LoadInfo marked ``shared_with`` the builder.  A failed build is not retried:
        later requesters get the same outcome (unavailable or error)."""
        if dataset.fingerprint in self._built:
            path, built, builder = self._built[dataset.fingerprint]
            return path, LoadInfo(seconds=0.0, triples=built.triples, details={},
                                  shared_with=builder)
        if dataset.fingerprint in self._failed:
            first = self._failed[dataset.fingerprint]
            kind = EngineUnavailable if isinstance(first, EngineUnavailable) else ExtensionError
            raise kind(str(first))
        path = self.path_for(dataset)
        info = self._reuse(path, dataset) if self.settings.reuse_db else None
        if info is None:
            try:
                info = self._build(path, dataset)
            except Exception as e:
                self._failed[dataset.fingerprint] = e
                _remove_db(path)
                raise
        self._built[dataset.fingerprint] = (path, info, requester)
        return path, info

    def _sidecar_key(self, dataset: Dataset) -> dict:
        return {"dataset": dataset.fingerprint, "extension": self.library.fingerprint(),
                "page_size": PAGE_SIZE}

    def _reuse(self, path: str, dataset: Dataset) -> LoadInfo | None:
        try:
            with open(path + ".json", encoding="utf-8") as fh:
                meta = json.load(fh)
        except (OSError, ValueError):
            return None
        if meta.get("key") != self._sidecar_key(dataset) or not os.path.isfile(path):
            return None
        load = meta["load"]
        details = dict(load.get("details", {}), reused=True)
        return LoadInfo(seconds=load["seconds"], triples=load.get("triples"), details=details)

    def _build(self, path: str, dataset: Dataset) -> LoadInfo:
        os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
        _remove_db(path)
        conn = self.library.connect(path)
        try:
            try:
                conn.execute(f"PRAGMA page_size={PAGE_SIZE}")
                conn.execute("PRAGMA journal_mode=DELETE").fetchone()
                conn.execute(f"PRAGMA cache_size=-{CACHE_SIZE_KIB}")
                conn.execute("SELECT sparql_init()").fetchone()
                mode = self._load_mode(conn)
                details: dict[str, Any] = {"load_mode": mode}
                if mode == "file":
                    conn.execute("SELECT sparql_config('allow_files', 1)").fetchone()
                    start = time.perf_counter()
                    loaded = conn.execute("SELECT sparql_load_file(?, 'ntriples')",
                                          (dataset.path,)).fetchone()[0]
                else:
                    with open(dataset.path, "rb") as fh:
                        data = fh.read()
                    start = time.perf_counter()
                    loaded = conn.execute("SELECT sparql_load(?, 'ntriples')", (data,)).fetchone()[0]
                    del data
                details["load_s"] = time.perf_counter() - start
                start = time.perf_counter()
                conn.execute("ANALYZE main")
                details["analyze_sql_s"] = time.perf_counter() - start
                start = time.perf_counter()
                summary = conn.execute("SELECT sparql_analyze()").fetchone()[0]
                details["sparql_analyze_s"] = time.perf_counter() - start
                details["sparql_analyze"] = summary
            except sqlite3.Error as e:
                raise _setup_error(f"building the {dataset.name} database", e) from e
        finally:
            conn.close()
        details["db_bytes"] = os.path.getsize(path)
        seconds = details["load_s"] + details["analyze_sql_s"] + details["sparql_analyze_s"]
        info = LoadInfo(seconds=seconds, triples=int(loaded), details=details)
        with open(path + ".json", "w", encoding="utf-8") as fh:
            json.dump({"key": self._sidecar_key(dataset), "load": info.to_json()}, fh, indent=1)
        return info

    def _load_mode(self, conn: Any) -> str:
        wanted = self.settings.sqlite_load
        if wanted in ("file", "blob"):
            return wanted
        has_file = self.library.has_function(conn, "sparql_load_file")
        return "file" if has_file and self.library.has_function(conn, "sparql_config") else "blob"


def rows_columns(query: BenchQuery, full_columns: str) -> list[str] | None:
    """The ``sparql_rows`` columns full mode fetches for ``query`` (None = ``*``).

    ``terms``: one N-Triples column per projected variable (``t0..t{n-1}``, §8.3), so
    ``col_used`` makes the extension decode exactly those terms and Python builds one
    string per term, like the other engines.  The projection comes from the query text
    (:attr:`BenchQuery.projection`); when it is unknown (``SELECT *``) the names come
    from the ``vars`` column, and past 16 variables only ``json`` has every binding.
    """
    if full_columns == "star":
        return None
    if query.form == "ask":
        return ["v0"]
    if query.form in ("construct", "describe"):
        return ["t0", "t1", "t2"]
    projection = query.projection
    if projection is None:
        return ["vars"] + [f"t{k}" for k in range(MAX_COLUMNS)]
    if len(projection) > MAX_COLUMNS:
        return ["vars", "json"]
    return [f"t{k}" for k in range(len(projection))]


class _SparqliteEngine(Engine):
    """What the native and baseline adapters share: probing, connection, deadlines."""

    #: mode -> probe call that must be registered for the mode to be measured
    mode_calls: dict[str, str] = {}
    #: whether ``sparql: unsupported:`` is a legitimate answer of this engine (§9)
    unsupported_ok = False
    #: ``sparql_rows`` arguments after the query (the §8.3 options)
    rows_options: tuple[str, ...] = ()

    def __init__(self, settings: EngineSettings, database: SparqliteDatabase) -> None:
        super().__init__(settings)
        if settings.full_columns not in FULL_COLUMNS:
            raise ValueError(f"full_columns must be one of {FULL_COLUMNS}")
        self.database = database
        self._conn: Any = None
        self._watchdog: Watchdog | None = None
        self._rows_sql: dict[str, tuple[str, list[str] | None]] = {}

    @property
    def library(self) -> SparqliteLibrary:
        return self.database.library

    def probe(self) -> str:
        info = self.library.probe()
        if info["error"]:
            raise EngineUnavailable(info["error"])
        if info["init_error"]:
            raise ExtensionError(info["init_error"])
        if {"sparql_load", "sparql_load_file"} <= set(info["missing"]):
            raise EngineUnavailable("neither sparql_load nor sparql_load_file is registered")
        if all(self.mode_unavailable(m) for m in MODES):
            reasons = {m: self.mode_unavailable(m) for m in MODES}
            raise EngineUnavailable(f"no measurable mode: {reasons}")
        return str(info["version"])

    def mode_unavailable(self, mode: str) -> str | None:
        call = self.mode_calls.get(mode)
        if call is None:
            return f"unknown mode {mode!r}"
        info = self.library.probe()
        if call in info["missing"]:
            return f"{call}: {info['calls'][call]}"
        if call not in info["calls"]:
            return f"{call}: not probed ({info['error'] or info['init_error']})"
        return None

    def open(self, dataset: Dataset) -> LoadInfo:
        path, info = self.database.ensure(dataset, self.name)
        conn = self.library.connect(path)
        try:
            conn.execute(f"PRAGMA cache_size=-{CACHE_SIZE_KIB}")
            self._configure(conn)
        except sqlite3.Error as e:
            conn.close()
            raise _setup_error("configuring the connection", e) from e
        self._conn = conn
        if self.settings.timeout_s > 0:
            self._watchdog = Watchdog(self.settings.timeout_s, name=f"{self.name}-watchdog")
            self._watchdog.start()
        return info

    def _configure(self, conn: Any) -> None:
        """Per-engine connection settings (after the extension is loaded)."""

    def close(self) -> None:
        if self._watchdog is not None:
            self._watchdog.close()
            self._watchdog = None
        if self._conn is not None:
            self._conn.close()
            self._conn = None

    # ------------------------------------------------------------- timed executions

    def _guarded(self, run: Callable[..., Any], *args: Any) -> Any:
        """Calls ``run(*args)`` under the ``--timeout`` deadline and classifies SQLite
        errors.  This is inside the timed region: arming and disarming the watchdog
        costs about a microsecond and never starts a thread (see watchdog.py)."""
        watchdog = self._watchdog
        if watchdog is None and self.settings.timeout_s > 0:
            watchdog = self._watchdog = Watchdog(self.settings.timeout_s,
                                                 name=f"{self.name}-watchdog")
        token = watchdog.arm(self._conn.interrupt) if watchdog is not None else 0
        try:
            return run(*args)
        except sqlite3.Error as e:
            error = e
        finally:
            fired = watchdog.disarm(token) if watchdog is not None else False
        raise classify_error(error, fired=fired, unsupported_ok=self.unsupported_ok) from error

    def _fetch_scalar(self, sql: str, params: Sequence[Any]) -> Any:
        return self._conn.execute(sql, params).fetchone()[0]

    def _fetch_rows(self, sql: str, params: Sequence[Any]) -> tuple[Any, list]:
        cur = self._conn.execute(sql, params)
        return cur, cur.fetchall()

    def _scalar(self, sql: str, *params: Any) -> Any:
        return self._guarded(self._fetch_scalar, sql, params)

    def _rows(self, query: BenchQuery) -> tuple[list[str], list]:
        """Full mode: the ``sparql_rows`` fetch, completely materialised."""
        cached = self._rows_sql.get(query.text)
        if cached is None:
            cols = rows_columns(query, self.settings.full_columns)
            select = "*" if cols is None else ", ".join(cols)
            args = ", ".join("?" for _ in range(1 + len(self.rows_options)))
            cached = self._rows_sql[query.text] = (f"SELECT {select} FROM sparql_rows({args})", cols)
        sql, cols = cached
        cur, rows = self._guarded(self._fetch_rows, sql, (query.text, *self.rows_options))
        return (cols if cols is not None else [d[0] for d in cur.description]), rows

    def _decode_rows(self, query: BenchQuery, raw: Any) -> ResultSet:
        names, rows = raw
        return decode_sparql_rows(query.form, names, rows, query.projection)

    def row_count(self, query: BenchQuery, mode: str, raw: Any) -> int:
        if mode == "count":
            return int(raw)
        if mode == "full":
            return len(raw[1])  # ASK: its one sparql_rows row
        return serialised_row_count(query.form, raw)


class SparqliteNative(_SparqliteEngine):
    name = "sparqlite-native"
    short = "native"
    mode_calls = {"count": "sparql_count", "full": "sparql_rows", "json": "sparql_query"}

    def __init__(self, settings: EngineSettings, database: SparqliteDatabase) -> None:
        super().__init__(settings, database)
        if settings.native_config:
            # A non-default configuration is part of what the column measures.
            self.short = "native[" + ",".join(f"{k}={v}" for k, v in settings.native_config.items()) + "]"

    def _configure(self, conn: Any) -> None:
        for key, value in self.settings.native_config.items():
            _set_config(conn, key, value)

    def execute(self, query: BenchQuery, mode: str) -> Any:
        if mode == "count":
            return self._scalar("SELECT sparql_count(?)", query.text)
        if mode == "full":
            return self._rows(query)
        if mode == "json":
            fmt = "json" if query.form in ("select", "ask") else "ntriples"
            return self._scalar("SELECT sparql_query(?, ?)", query.text, fmt)
        raise ValueError(f"unknown mode {mode!r}")

    def decode(self, query: BenchQuery, mode: str, raw: Any) -> int | ResultSet:
        if mode == "count":
            return int(raw)
        if mode == "json":
            if query.form in ("select", "ask"):
                return result_set_from_sparql_json(query.form, raw)
            return result_set_from_ntriples_doc(raw)
        return self._decode_rows(query, raw)


class SparqliteBaseline(_SparqliteEngine):
    name = "sparqlite-baseline"
    short = "baseline"
    mode_calls = {"count": "sparql_count_sql", "full": "sparql_rows_sql", "json": "sparql_query_sql"}
    unsupported_ok = True
    rows_options = (BASELINE_ROWS_OPTIONS,)

    def probe(self) -> str:
        """As for native, plus: the probe query is one triple pattern, inside every
        translator subset, so ``unsupported`` there means a broken baseline, which
        must not turn every cell into a quiet "unsupported"."""
        version = super().probe()
        calls = self.library.probe()["calls"]
        for call in self.mode_calls.values():
            problem = calls.get(call)
            if problem is not None and problem.startswith(UNSUPPORTED_PREFIX):
                raise ExtensionError(f"the baseline rejects the probe query in {call}: {problem}")
        return version

    def execute(self, query: BenchQuery, mode: str) -> Any:
        if mode == "count":
            return self._scalar("SELECT sparql_count_sql(?)", query.text)
        if mode == "full":
            return self._rows(query)
        if mode == "json":
            fmt = "json" if query.form in ("select", "ask") else "ntriples"
            return self._scalar("SELECT sparql_query_sql(?, ?)", query.text, fmt)
        raise ValueError(f"unknown mode {mode!r}")

    def decode(self, query: BenchQuery, mode: str, raw: Any) -> int | ResultSet:
        if mode == "count":
            return int(raw)
        if mode == "full":
            return self._decode_rows(query, raw)
        if query.form in ("select", "ask"):
            return result_set_from_sparql_json(query.form, raw)
        return result_set_from_ntriples_doc(raw)


def decode_sparql_rows(form: str, names: list[str], rows: list[tuple],
                       projection: Sequence[str] | None = None) -> ResultSet:
    """Canonicalises ``sparql_rows`` output (columns per §8.3: ``n, v0..v15, t0..t15,
    id0..id15, vars, json``; any subset that includes what the form needs).

    The variable names come from the ``vars`` column when it was fetched, else from
    ``projection`` (the query's explicit projection, in column order).
    """
    idx = {name: i for i, name in enumerate(names)}
    if form == "ask":
        if len(rows) != 1:
            raise ValueError(f"ASK via sparql_rows returned {len(rows)} rows")
        return ResultSet(form="ask", boolean=bool(rows[0][idx["v0"]]))
    if form in ("construct", "describe"):
        cols = [idx[f"t{k}"] for k in range(3)]
        return ResultSet(form="graph", vars=list(GRAPH_VARS),
                         rows=[tuple(canonical_nt(row[c]) for c in cols) for row in rows])
    if not rows:
        return ResultSet(form="select", vars=list(projection or []))
    if "vars" in idx:
        vars_ = json.loads(rows[0][idx["vars"]])
    elif projection is not None:
        vars_ = list(projection)
    else:
        raise ValueError("neither the vars column nor the query's projection is known")
    if all(f"t{k}" in idx for k in range(len(vars_))):
        tcols = [idx[f"t{k}"] for k in range(len(vars_))]
        out = [tuple(canonical_nt(row[c]) for c in tcols) for row in rows]
    elif "json" in idx:
        jc = idx["json"]
        out = []
        for row in rows:
            binding = json.loads(row[jc])
            out.append(tuple(canonical(from_sparql_json(binding[v])) if v in binding else UNBOUND
                             for v in vars_))
    else:
        raise ValueError(f"{len(vars_)} variables but neither t-columns nor json were fetched")
    return ResultSet(form="select", vars=vars_, rows=out)
