"""
explain_provenance.py — A PURE standard-library proof of concept that maps
result columns back to source ``table.column`` by parsing SQLite's ``EXPLAIN``
VDBE bytecode. No ctypes, no apsw — just the stdlib ``sqlite3`` module.

How it works
------------
``EXPLAIN <query>`` returns the virtual-machine program SQLite would run.
We read three things from it:

* ``OpenRead p1=cursor p2=rootpage``  — opens a btree cursor on a table/index.
  We map ``rootpage`` back to a name via ``sqlite_master``.
* ``Column   p1=cursor p2=colidx p3=reg`` — copies table column ``colidx`` of
  ``cursor`` into register ``reg``.
* ``Rowid    p1=cursor p2=reg`` — copies the rowid (== an INTEGER PRIMARY KEY
  alias) into register ``reg``.
* ``ResultRow p1=start p2=count`` — emits registers ``start .. start+count-1``
  as one output row.

So for each output register we find the opcode that last wrote it. If that was
a ``Column``/``Rowid`` straight off a base-table cursor, we have a clean source.
Anything else (``Concat``, ``Function``, ``String8``, ``Count`` ...) means the
column is a computed expression with no single source.

Important caveat
----------------
This is *optimizer-dependent*. The query planner can route a column through an
automatic or covering index (``OpenAutoindex``), an ephemeral table, or a
co-routine for subqueries — and then the bytecode no longer points straight at
a base table. We set ``PRAGMA automatic_index=OFF`` to avoid the most common
case, but EXPLAIN parsing remains a best-effort heuristic. The ctypes /
column-metadata approach in ``column_provenance.py`` is the robust answer; this
module exists to show how much you can squeeze out of the pure stdlib.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Dict, List, Optional, Tuple


@dataclass
class ExplainColumn:
    index: int
    output_name: str
    table: Optional[str]
    origin_column: Optional[str]
    note: str = ""

    @property
    def source(self) -> Optional[str]:
        if self.table and self.origin_column:
            return f"{self.table}.{self.origin_column}"
        return None

    def __str__(self) -> str:
        return f"{self.output_name} <- {self.source or '(expression / no source)'}" \
               + (f"   [{self.note}]" if self.note else "")


def _schema_maps(conn):
    """rootpage -> (kind, name, tbl_name) for tables and indexes."""
    rootmap: Dict[int, Tuple[str, str, str]] = {}
    for typ, name, tbl, rootpage in conn.execute(
        "SELECT type, name, tbl_name, rootpage FROM sqlite_master "
        "WHERE rootpage IS NOT NULL AND rootpage>0"
    ):
        rootmap[rootpage] = (typ, name, tbl)
    return rootmap


def _table_columns(conn, table: str) -> Tuple[List[str], Optional[str]]:
    """Return (columns_by_cid, rowid_alias_column_or_None)."""
    cols, alias = [], None
    for cid, name, ctype, notnull, dflt, pk in conn.execute(
        f'PRAGMA table_info("{table}")'
    ):
        cols.append(name)
        if pk == 1 and (ctype or "").upper() == "INTEGER":
            alias = name
    return cols, alias


def _index_columns(conn, index: str) -> List[str]:
    return [name for _seq, _cid, name in conn.execute(
        f'PRAGMA index_info("{index}")')]


def resolve_columns_via_explain(conn, sql: str) -> List[ExplainColumn]:
    """Best-effort, pure-stdlib mapping of result columns to table.column."""
    rootmap = _schema_maps(conn)

    # Disable automatic indexes so joined columns stay on their base cursors.
    prev = conn.execute("PRAGMA automatic_index").fetchone()[0]
    conn.execute("PRAGMA automatic_index=OFF")
    try:
        program = list(conn.execute("EXPLAIN " + sql))
        # Column names / count come from a normally-prepared statement.
        cur = conn.execute(sql)
        names = [d[0] for d in cur.description]
        cur.close()
    finally:
        conn.execute(f"PRAGMA automatic_index={'ON' if prev else 'OFF'}")

    # cursor -> ("table"|"index", name)
    cursor_root: Dict[int, Tuple[str, str, str]] = {}
    # register -> ("col", cursor, colidx) | ("rowid", cursor) | ("expr", opcode)
    reg_writer: Dict[int, Tuple] = {}
    result_row: Optional[Tuple[int, int]] = None

    for addr, op, p1, p2, p3, p4, p5, comment in program:
        if op == "OpenRead" or op == "OpenWrite":
            # p2 is the rootpage; p3 is the database index.
            info = rootmap.get(p2)
            if info:
                cursor_root[p1] = info
        elif op == "Column":
            reg_writer[p3] = ("col", p1, p2)
        elif op in ("Rowid", "IdxRowid"):
            reg_writer[p2] = ("rowid", p1)
        elif op == "ResultRow" and result_row is None:
            result_row = (p1, p2)

    out: List[ExplainColumn] = []
    if result_row is None:
        # No ResultRow (e.g. a bare aggregate may still have one; otherwise
        # we just cannot tell) — return names with unknown sources.
        return [ExplainColumn(i, n, None, None, "no ResultRow found")
                for i, n in enumerate(names)]

    start, count = result_row
    for i in range(count):
        reg = start + i
        name = names[i] if i < len(names) else f"col{i}"
        w = reg_writer.get(reg)
        if not w:
            out.append(ExplainColumn(i, name, None, None, "register not written by Column/Rowid"))
            continue
        kind = w[0]
        if kind == "col":
            _, cur_no, colidx = w
            info = cursor_root.get(cur_no)
            if not info:
                out.append(ExplainColumn(i, name, None, None,
                           f"cursor {cur_no} not a base table (ephemeral/index)"))
                continue
            typ, obj_name, tbl_name = info
            if typ == "table":
                cols, _ = _table_columns(conn, obj_name)
                col = cols[colidx] if 0 <= colidx < len(cols) else None
                out.append(ExplainColumn(i, name, obj_name, col))
            elif typ == "index":
                idx_cols = _index_columns(conn, obj_name)
                col = idx_cols[colidx] if 0 <= colidx < len(idx_cols) else None
                out.append(ExplainColumn(i, name, tbl_name, col, f"via index {obj_name}"))
            else:
                out.append(ExplainColumn(i, name, None, None, f"cursor on {typ}"))
        elif kind == "rowid":
            _, cur_no = w
            info = cursor_root.get(cur_no)
            if not info:
                out.append(ExplainColumn(i, name, None, None, "rowid of ephemeral cursor"))
                continue
            typ, obj_name, tbl_name = info
            tname = obj_name if typ == "table" else tbl_name
            _, alias = _table_columns(conn, tname)
            out.append(ExplainColumn(i, name, tname, alias or "rowid"))
        else:
            out.append(ExplainColumn(i, name, None, None, "computed expression"))
    return out


if __name__ == "__main__":
    import sqlite3
    import sys
    if len(sys.argv) == 3:
        conn = sqlite3.connect(sys.argv[1])
        for c in resolve_columns_via_explain(conn, sys.argv[2]):
            print("  ", c)
    else:
        print("usage: python explain_provenance.py <db_path> <sql>", file=sys.stderr)
        raise SystemExit(2)
