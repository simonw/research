"""
authorizer_lineage.py — Use the stdlib ``sqlite3`` authorizer callback to learn
which ``table.column`` pairs a query depends on. Pure standard library.

``Connection.set_authorizer(cb)`` installs a callback that SQLite invokes once
per action *while preparing* a statement (no execution needed). For every column
access it fires ``SQLITE_READ`` with ``(action, table, column, db, trigger)``.

What this gives you, and what it does NOT
-----------------------------------------
The authorizer yields the *set of columns the query reads* — including columns
used only in WHERE / JOIN / ORDER BY / GROUP BY, and every column produced by a
``*`` expansion. That is exactly right for:

  * column-level data lineage ("what does this query touch?")
  * access control / redaction (return ``SQLITE_DENY`` to block a column)

It is NOT a per-output-column mapping. The reads are not aligned 1:1 with result
columns once there is a WHERE/JOIN/ORDER BY, and an expression column such as
``name || '-suffix'`` shows up simply as a read of ``t1.name`` with no signal
that the output was transformed. For the per-output mapping use
``column_provenance.resolve_columns_for_connection`` (robust) or
``explain_provenance`` (pure-stdlib, best-effort).

The one case where reads line up exactly with outputs is the simplest:
``SELECT col, col, ... FROM single_table`` with no WHERE/ORDER/GROUP and no
expressions — handled by ``naive_output_mapping`` below, which verifies that
precondition before trusting the alignment.
"""

from __future__ import annotations

import sqlite3
from typing import List, Optional, Tuple


def columns_read(conn: sqlite3.Connection, sql: str) -> List[Tuple[str, str]]:
    """Return the ordered list of (table, column) reads SQLite performs while
    preparing ``sql``. Duplicates preserved in firing order. Read-only: the
    statement is prepared but never stepped."""
    reads: List[Tuple[str, str]] = []

    def auth(action, arg1, arg2, db_name, trigger):
        if action == sqlite3.SQLITE_READ and arg1 is not None and arg2 is not None:
            reads.append((arg1, arg2))
        return sqlite3.SQLITE_OK

    conn.set_authorizer(auth)
    try:
        cur = conn.execute(sql)   # prepares; we never fetch, so no rows are read
        cur.close()
    finally:
        conn.set_authorizer(None)
    return reads


def columns_read_set(conn: sqlite3.Connection, sql: str):
    """The deduplicated set of (table, column) the query depends on."""
    return set(columns_read(conn, sql))


def naive_output_mapping(conn: sqlite3.Connection, sql: str
                         ) -> Optional[List[Tuple[str, Tuple[str, str]]]]:
    """If — and only if — the read sequence can be trusted to line up with the
    output columns (number of reads == number of result columns), return a
    best-effort [(output_name, (table, column))] mapping. Otherwise return None
    to signal "authorizer alone can't resolve this query".
    """
    reads = columns_read(conn, sql)
    cur = conn.execute(sql)
    names = [d[0] for d in cur.description]
    cur.close()
    if len(reads) != len(names):
        return None
    return list(zip(names, reads))


if __name__ == "__main__":
    SCHEMA = """
    CREATE TABLE t1 (id INTEGER PRIMARY KEY, name TEXT, extra TEXT);
    CREATE TABLE t2 (id INTEGER PRIMARY KEY, name TEXT, age INTEGER);
    """
    conn = sqlite3.connect(":memory:")
    conn.executescript(SCHEMA)
    for q in [
        "select id, name from t1",
        "select * from t1",
        "select id, name || '-suffix' from t1",
        "select t1.id, t1.name, age from t1 join t2 on t1.name=t2.name "
        "where extra='x' order by age",
    ]:
        print("Q:", q)
        print("   reads :", columns_read(conn, q))
        print("   naive :", naive_output_mapping(conn, q))
        print()
