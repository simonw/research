"""
column_provenance.py — Map SQLite query result columns back to their source
``table.column`` using SQLite's "column metadata" C API, driven from pure
ctypes so it works with only the Python standard library (no apsw required).

The interesting functions are:

    sqlite3_column_origin_name(stmt, i)   -> real column name in the base table
    sqlite3_column_table_name(stmt, i)    -> real table name
    sqlite3_column_database_name(stmt, i) -> attached database name ("main", ...)

They only exist when libsqlite3 was compiled with SQLITE_ENABLE_COLUMN_METADATA
(true for the system libsqlite3.so on most Linux distros, and for apsw).

We never need to *run* the query: preparing the statement is enough for SQLite
to resolve every output column — including ``*`` expansion, joins, aliases, and
even subqueries / CTEs / unions — down to the underlying base-table column.
Expressions (``a || b``, ``upper(x)``, literals, aggregates) legitimately have
no single source column, and the API reports NULL for those.

Usage:
    from column_provenance import resolve_columns
    for col in resolve_columns("mydb.sqlite", "select id, name from t1"):
        print(col.output_name, "<-", col.source)   # e.g. id <- t1.id
"""

from __future__ import annotations

import ctypes
import ctypes.util
from dataclasses import dataclass
from typing import List, Optional


# --- locate and bind libsqlite3 ------------------------------------------------

def _load_libsqlite3() -> ctypes.CDLL:
    candidates = [
        ctypes.util.find_library("sqlite3"),
        "libsqlite3.so.0",
        "libsqlite3.so",
        "/usr/lib/x86_64-linux-gnu/libsqlite3.so",
        "libsqlite3.dylib",
    ]
    last_err = None
    for name in candidates:
        if not name:
            continue
        try:
            return ctypes.CDLL(name)
        except OSError as e:  # pragma: no cover - platform dependent
            last_err = e
    raise OSError(f"could not load libsqlite3: {last_err}")


_lib = _load_libsqlite3()

# Minimal signature declarations.
_lib.sqlite3_open_v2.argtypes = [ctypes.c_char_p, ctypes.POINTER(ctypes.c_void_p),
                                 ctypes.c_int, ctypes.c_char_p]
_lib.sqlite3_open_v2.restype = ctypes.c_int
_lib.sqlite3_close_v2.argtypes = [ctypes.c_void_p]
_lib.sqlite3_close_v2.restype = ctypes.c_int

_lib.sqlite3_prepare_v2.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int,
                                    ctypes.POINTER(ctypes.c_void_p),
                                    ctypes.POINTER(ctypes.c_char_p)]
_lib.sqlite3_prepare_v2.restype = ctypes.c_int
_lib.sqlite3_finalize.argtypes = [ctypes.c_void_p]
_lib.sqlite3_finalize.restype = ctypes.c_int

_lib.sqlite3_column_count.argtypes = [ctypes.c_void_p]
_lib.sqlite3_column_count.restype = ctypes.c_int

for _fn in ("sqlite3_column_name", "sqlite3_column_origin_name",
            "sqlite3_column_table_name", "sqlite3_column_database_name",
            "sqlite3_column_decltype"):
    getattr(_lib, _fn).argtypes = [ctypes.c_void_p, ctypes.c_int]
    getattr(_lib, _fn).restype = ctypes.c_char_p

_lib.sqlite3_errmsg.argtypes = [ctypes.c_void_p]
_lib.sqlite3_errmsg.restype = ctypes.c_char_p

# sqlite3_deserialize lets us load the bytes from a stdlib Connection.serialize()
# into a private ctypes connection, so we can inspect in-memory databases too.
_HAS_DESERIALIZE = hasattr(_lib, "sqlite3_deserialize")
if _HAS_DESERIALIZE:
    _lib.sqlite3_deserialize.argtypes = [
        ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p,
        ctypes.c_int64, ctypes.c_int64, ctypes.c_uint,
    ]
    _lib.sqlite3_deserialize.restype = ctypes.c_int

_HAS_METADATA = hasattr(_lib, "sqlite3_column_origin_name")

_SQLITE_OK = 0
_SQLITE_OPEN_READONLY = 0x00000001
_SQLITE_OPEN_URI = 0x00000040
_SQLITE_OPEN_READWRITE = 0x00000002
_SQLITE_OPEN_CREATE = 0x00000004
_SQLITE_OPEN_MEMORY = 0x00000080
_SQLITE_DESERIALIZE_READONLY = 4


@dataclass
class ResolvedColumn:
    """One output column of a query and where it came from."""
    index: int
    output_name: str          # name/alias as it appears in the result set
    decltype: Optional[str]   # declared type from the schema, if any
    database: Optional[str]   # "main", attached db name, or None for expressions
    table: Optional[str]      # source table, or None for expressions
    origin_column: Optional[str]  # source column, or None for expressions

    @property
    def is_direct_column(self) -> bool:
        """True if this output is a clean reference to a base-table column."""
        return self.table is not None and self.origin_column is not None

    @property
    def source(self) -> Optional[str]:
        """'table.column' for direct references, else None (an expression)."""
        if self.is_direct_column:
            return f"{self.table}.{self.origin_column}"
        return None

    def __str__(self) -> str:
        return f"{self.output_name} <- {self.source or '(expression / no source)'}"


def _decode(p) -> Optional[str]:
    return p.decode() if p is not None else None


def _require_metadata() -> None:
    if not _HAS_METADATA:
        raise RuntimeError(
            "libsqlite3 was built without SQLITE_ENABLE_COLUMN_METADATA; "
            "origin/table-name functions are unavailable."
        )


def _read_metadata(db: ctypes.c_void_p, sql: str) -> List[ResolvedColumn]:
    """Prepare ``sql`` against an open db handle and read column metadata.

    The statement is prepared but never stepped, so no rows are fetched.
    """
    stmt = ctypes.c_void_p()
    sql_bytes = sql.encode()
    rc = _lib.sqlite3_prepare_v2(db, sql_bytes, len(sql_bytes),
                                 ctypes.byref(stmt), None)
    if rc != _SQLITE_OK:
        msg = _decode(_lib.sqlite3_errmsg(db)) or f"rc={rc}"
        raise RuntimeError(f"could not prepare SQL: {msg}")
    try:
        cols: List[ResolvedColumn] = []
        for i in range(_lib.sqlite3_column_count(stmt)):
            cols.append(ResolvedColumn(
                index=i,
                output_name=_decode(_lib.sqlite3_column_name(stmt, i)),
                decltype=_decode(_lib.sqlite3_column_decltype(stmt, i)),
                database=_decode(_lib.sqlite3_column_database_name(stmt, i)),
                table=_decode(_lib.sqlite3_column_table_name(stmt, i)),
                origin_column=_decode(_lib.sqlite3_column_origin_name(stmt, i)),
            ))
        return cols
    finally:
        _lib.sqlite3_finalize(stmt)


def resolve_columns(db_path: str, sql: str) -> List[ResolvedColumn]:
    """Resolve each result column of ``sql`` to its source table.column.

    The query is prepared (not executed) against the database at ``db_path``,
    which is opened read-only. Requires libsqlite3 built with
    SQLITE_ENABLE_COLUMN_METADATA.
    """
    _require_metadata()
    db = ctypes.c_void_p()
    flags = _SQLITE_OPEN_READONLY | _SQLITE_OPEN_URI
    rc = _lib.sqlite3_open_v2(db_path.encode(), ctypes.byref(db), flags, None)
    if rc != _SQLITE_OK:
        msg = _decode(_lib.sqlite3_errmsg(db)) or f"rc={rc}"
        _lib.sqlite3_close_v2(db)
        raise RuntimeError(f"could not open {db_path!r}: {msg}")
    try:
        return _read_metadata(db, sql)
    finally:
        _lib.sqlite3_close_v2(db)


def resolve_columns_for_connection(conn, sql: str) -> List[ResolvedColumn]:
    """Resolve result columns for a query against a **stdlib sqlite3.Connection**.

    This is the "works with the standard library" entry point. Python's stdlib
    ``sqlite3`` does not expose the column-metadata API, so we bridge to it:

      * If the connection's ``main`` database is a real file on disk, we open
        that file read-only with our own ctypes handle and inspect it.
      * Otherwise (``:memory:`` or a temp database) we use
        ``Connection.serialize()`` (Python 3.11+) to snapshot the database and
        ``sqlite3_deserialize`` it into a private in-memory ctypes connection.

    The snapshot path means in-memory schemas — including tables created only in
    this process — resolve correctly without touching disk.
    """
    _require_metadata()

    # Find the file backing the "main" database, if any.
    main_path = None
    for seq, name, fname in conn.execute("PRAGMA database_list").fetchall():
        if name == "main":
            main_path = fname
            break

    if main_path:  # file-backed: cheapest, and sees committed data
        return resolve_columns(main_path, sql)

    # In-memory / temp: snapshot via serialize() and load through ctypes.
    if not hasattr(conn, "serialize"):
        raise RuntimeError(
            "in-memory connection requires Connection.serialize() (Python 3.11+)"
        )
    if not _HAS_DESERIALIZE:
        raise RuntimeError("libsqlite3 lacks sqlite3_deserialize")

    blob = conn.serialize()  # bytes snapshot of the "main" database

    db = ctypes.c_void_p()
    flags = (_SQLITE_OPEN_READWRITE | _SQLITE_OPEN_CREATE |
             _SQLITE_OPEN_MEMORY | _SQLITE_OPEN_URI)
    rc = _lib.sqlite3_open_v2(b":memory:", ctypes.byref(db), flags, None)
    if rc != _SQLITE_OK:
        _lib.sqlite3_close_v2(db)
        raise RuntimeError(f"could not open in-memory db: rc={rc}")
    # Keep the buffer alive for the lifetime of the handle (READONLY => SQLite
    # does not copy or take ownership of it).
    buf = ctypes.create_string_buffer(blob, len(blob))
    try:
        rc = _lib.sqlite3_deserialize(db, b"main", buf, len(blob), len(blob),
                                      _SQLITE_DESERIALIZE_READONLY)
        if rc != _SQLITE_OK:
            msg = _decode(_lib.sqlite3_errmsg(db)) or f"rc={rc}"
            raise RuntimeError(f"sqlite3_deserialize failed: {msg}")
        return _read_metadata(db, sql)
    finally:
        _lib.sqlite3_close_v2(db)
        del buf


def has_column_metadata() -> bool:
    """Whether the loaded libsqlite3 supports the column-metadata API."""
    return _HAS_METADATA


if __name__ == "__main__":
    import sys
    if len(sys.argv) != 3:
        print("usage: python column_provenance.py <db_path> <sql>", file=sys.stderr)
        raise SystemExit(2)
    for col in resolve_columns(sys.argv[1], sys.argv[2]):
        print(f"  [{col.index}] {col.output_name!r:20} -> "
              f"{col.source or '(expression / no source)'}")
