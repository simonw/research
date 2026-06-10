"""
datasette_duckdb: a proof-of-concept adapter that lets Datasette serve a
DuckDB database file instead of SQLite, while preserving the two safety
properties Datasette relies on for untrusted SQL:

  * strict read-only + filesystem/network lockdown (see safe_duckdb.py)
  * a query time limit (watchdog thread -> connection.interrupt(), DuckDB's
    analogue of SQLite's progress-handler timeout)

It works by giving Datasette a small shim object that mimics the subset of
the sqlite3.Connection / Cursor API that Datasette actually calls, backed by
a hardened DuckDB connection. DuckDB already understands `sqlite_master` and
`PRAGMA table_info`, so most of Datasette's introspection runs unchanged.

Call install() before creating your Datasette instance, then add
DuckDBDatabase instances to it.
"""

import re
import sqlite3
import threading
import time

import duckdb

import datasette.database as ds_database
import datasette.utils as ds_utils
from datasette.database import Database, Results


HARDENED_CONFIG = {
    "enable_external_access": "false",
    "allow_community_extensions": "false",
    "autoinstall_known_extensions": "false",
    "autoload_known_extensions": "false",
    "allow_unsigned_extensions": "false",
    "lock_configuration": "true",  # applied last; blocks SET undoing the above
}


# Datasette quotes identifiers as [name] (SQLite); DuckDB wants "name".
# Restricting to word characters and spaces avoids touching DuckDB list
# literals like [1, 2, 3] or ['a', 'b'].
_BRACKET_IDENT_RE = re.compile(r"\[([\w ]+)\]")


def _rewrite_sql(sql, params):
    """Translate the SQLite dialect quirks Datasette emits into DuckDB."""
    # PRAGMA table_xinfo(x) -> PRAGMA table_info(x); we pad the missing
    # `hidden` column in the cursor.
    rewritten_xinfo = "table_xinfo(" in sql
    if rewritten_xinfo:
        sql = sql.replace("table_xinfo(", "table_info(")
    # [identifier] -> "identifier"; pragma args want '...' not "..." though.
    if sql.strip().lower().startswith("pragma"):
        sql = _BRACKET_IDENT_RE.sub(lambda m: "'" + m.group(1) + "'", sql)
    else:
        sql = _BRACKET_IDENT_RE.sub(lambda m: '"' + m.group(1) + '"', sql)
    # Named params: SQLite uses :name, DuckDB uses $name. Only rewrite (and
    # later bind) keys actually referenced -- SQLite ignores unused named
    # params but DuckDB rejects them.
    used_keys = []
    if isinstance(params, dict):
        for key in params:
            placeholder = f":{key}"
            if placeholder in sql:
                sql = sql.replace(placeholder, f"${key}")
                used_keys.append(key)
    return sql, rewritten_xinfo, used_keys


# PRAGMAs Datasette issues that DuckDB doesn't support -- silently ignore.
_IGNORED_PRAGMA_PREFIXES = ("pragma cache_size", "pragma case_sensitive_like")

# PRAGMAs DuckDB lacks but that we can answer with an equivalent SELECT.
# schema_version is used only for cache invalidation; a constant is fine for
# an immutable database.
_PRAGMA_REWRITES = {
    "pragma schema_version": "SELECT 0 AS schema_version",
}

# PRAGMAs DuckDB has no equivalent for -- answer with an empty result set.
_PRAGMA_EMPTY = ("pragma foreign_key_list", "pragma index_list")


class Row(tuple):
    """A tuple that also supports sqlite3.Row-style access by column name.

    Iterates as values (so zip(columns, row) and list(row) work), but also
    provides keys()/__getitem__[str] so dict(row) yields a name->value map.
    """

    def __new__(cls, values, columns):
        self = super().__new__(cls, values)
        self._columns = columns
        return self

    def keys(self):
        return list(self._columns)

    def __getitem__(self, key):
        if isinstance(key, str):
            return tuple.__getitem__(self, self._columns.index(key))
        return tuple.__getitem__(self, key)


class _Cursor:
    def __init__(self, conn):
        self._conn = conn
        self._rel = None
        self.description = None
        self.rowcount = -1
        self.lastrowid = None
        self._pad_hidden = False
        self._empty = False

    def execute(self, sql, params=None):
        self._empty = False
        lowered = sql.strip().lower().rstrip(";")
        if any(lowered.startswith(p) for p in _IGNORED_PRAGMA_PREFIXES):
            self.description = None
            return self
        if lowered in _PRAGMA_REWRITES:
            self._rel = self._conn._duck.execute(_PRAGMA_REWRITES[lowered])
            self.description = self._rel.description
            self._pad_hidden = False
            return self
        if any(lowered.startswith(p) for p in _PRAGMA_EMPTY):
            self._rel = None
            self.description = None
            self._pad_hidden = False
            self._empty = True
            return self
        sql, self._pad_hidden, used_keys = _rewrite_sql(sql, params)
        try:
            if isinstance(params, dict):
                bind = {k: params[k] for k in used_keys}
                if bind:
                    self._rel = self._conn._duck.execute(sql, bind)
                else:
                    self._rel = self._conn._duck.execute(sql)
            elif params:
                self._rel = self._conn._duck.execute(sql, list(params))
            else:
                self._rel = self._conn._duck.execute(sql)
        except duckdb.InterruptException as e:
            # Map to what Datasette expects so it raises QueryInterrupted.
            raise sqlite3.OperationalError("interrupted") from e
        except duckdb.Error as e:
            # Surface every other DuckDB failure as the exception type
            # Datasette already knows how to turn into a clean 400.
            raise sqlite3.OperationalError(str(e).splitlines()[0]) from e
        self.description = self._rel.description
        return self

    def _columns(self):
        cols = [d[0] for d in self.description] if self.description else []
        if self._pad_hidden:
            cols = cols + ["hidden"]
        return cols

    def _wrap(self, rows):
        cols = self._columns()
        if self._pad_hidden:
            return [Row(tuple(r) + (0,), cols) for r in rows]
        return [Row(r, cols) for r in rows]

    def fetchall(self):
        if self._empty:
            return []
        return self._wrap(self._rel.fetchall()) if self._rel else []

    def fetchmany(self, size):
        if self._empty:
            return []
        return self._wrap(self._rel.fetchmany(size)) if self._rel else []

    def fetchone(self):
        if self._empty or not self._rel:
            return None
        row = self._rel.fetchone()
        if row is None:
            return None
        cols = self._columns()
        if self._pad_hidden:
            row = tuple(row) + (0,)
        return Row(row, cols)

    def __iter__(self):
        return iter(self.fetchall())


class DuckDBConnection:
    """A duck-typed stand-in for sqlite3.Connection over a DuckDB connection."""

    def __init__(self, duck):
        self._duck = duck
        # Datasette sets these; we accept and ignore them.
        self.row_factory = None
        self.text_factory = None

    def cursor(self):
        return _Cursor(self)

    def execute(self, sql, params=None):
        return _Cursor(self).execute(sql, params)

    def set_progress_handler(self, handler, n):
        # No-op: DuckDB has no progress handler. Time limits are enforced by
        # duckdb_timelimit() via interrupt() instead.
        pass

    def create_function(self, *args, **kwargs):
        # Best effort: skip custom SQL functions in this POC.
        pass

    def interrupt(self):
        self._duck.interrupt()

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        return False

    def close(self):
        try:
            self._duck.close()
        except Exception:
            pass


class duckdb_timelimit:
    """interrupt()-based time limit; analogue of utils.sqlite_timelimit."""

    def __init__(self, conn, ms):
        self.conn = conn
        self.ms = ms
        self._cancel = threading.Event()
        self._thread = None

    def __enter__(self):
        deadline = time.perf_counter() + (self.ms / 1000)

        def watchdog():
            remaining = deadline - time.perf_counter()
            if remaining > 0 and self._cancel.wait(remaining):
                return
            if not self._cancel.is_set():
                try:
                    self.conn.interrupt()
                except Exception:
                    pass

        self._thread = threading.Thread(target=watchdog, daemon=True)
        self._thread.start()
        return self

    def __exit__(self, *exc):
        self._cancel.set()
        if self._thread:
            self._thread.join()
        return False


# A sqlite_timelimit replacement that dispatches based on connection type.
_orig_sqlite_timelimit = ds_utils.sqlite_timelimit


def _dispatching_timelimit(conn, ms):
    if isinstance(conn, DuckDBConnection):
        return duckdb_timelimit(conn, ms)
    return _orig_sqlite_timelimit(conn, ms)


class DuckDBDatabase(Database):
    """A Datasette Database backed by a hardened, read-only DuckDB file."""

    def __init__(self, ds, path, memory_limit="256MB", threads=1, **kwargs):
        super().__init__(ds, path=path, is_mutable=False, **kwargs)
        self._duck_memory_limit = memory_limit
        self._duck_threads = threads

    def connect(self, write=False):
        if write:
            raise RuntimeError("DuckDBDatabase is read-only in this POC")
        config = {
            "memory_limit": self._duck_memory_limit,
            "threads": str(self._duck_threads),
            "max_temp_directory_size": "0B",
        }
        config.update(HARDENED_CONFIG)
        duck = duckdb.connect(self.path, read_only=True, config=config)
        conn = DuckDBConnection(duck)
        self._all_file_connections.append(conn)
        return conn

    @property
    def size(self):
        import os

        try:
            return os.path.getsize(self.path)
        except OSError:
            return 0

    async def hidden_table_names(self):
        # "Hidden" tables (FTS shadow tables, SpatiaLite, etc.) are a
        # SQLite-specific concept that DuckDB does not have.
        return []

    async def get_all_foreign_keys(self):
        # The upstream helper relies on SQLite's pragma_foreign_key_list and
        # double-quoted string literals. Report no foreign keys for this POC.
        tables = await self.table_names()
        return {t: {"incoming": [], "outgoing": []} for t in tables}

    async def fts_table(self, table):
        # SQLite full-text-search detection; not applicable to DuckDB.
        return None


def install():
    """Patch Datasette's time-limit helper to understand DuckDB connections."""
    ds_database.sqlite_timelimit = _dispatching_timelimit
    ds_utils.sqlite_timelimit = _dispatching_timelimit
