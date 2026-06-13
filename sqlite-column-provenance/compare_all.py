"""
compare_all.py — Run every technique side by side on one battery of queries so
you can see exactly where each one succeeds and fails.

Techniques compared (all driven from a stdlib sqlite3 in-memory connection):
  1. ctypes column-metadata bridge   (column_provenance.resolve_columns_for_connection)
  2. EXPLAIN bytecode parsing         (explain_provenance.resolve_columns_via_explain)
  3. authorizer read-set             (authorizer_lineage.columns_read_set)
  4. apsw description_full           (reference oracle, if apsw is installed)
"""
import sqlite3

from column_provenance import resolve_columns_for_connection, has_column_metadata
from explain_provenance import resolve_columns_via_explain
from authorizer_lineage import columns_read_set

SCHEMA = """
CREATE TABLE t1 (id INTEGER PRIMARY KEY, name TEXT, extra TEXT);
CREATE TABLE t2 (id INTEGER PRIMARY KEY, name TEXT, age INTEGER);
INSERT INTO t1 VALUES (1,'alice','x'),(2,'bob','y');
INSERT INTO t2 VALUES (10,'alice',30),(11,'carol',40);
"""

QUERIES = [
    "select id, name from t1",
    "select * from t1",
    "select t1.id, t1.name, age from t1 join t2 on t1.name = t2.name",
    "select id, name || '-suffix' from t1",
    "select t1.*, t2.age from t1 join t2 on t1.name=t2.name",
    "select id as the_id, name as nm from t1",
    "select count(*), max(age) from t2",
    "select age from (select * from t2) sub",
    "with c as (select id, name from t1) select * from c",
    "select name from t1 union select name from t2",
]


def fmt(cols):
    return ", ".join(f"{c.output_name}->{c.source or 'EXPR'}" for c in cols)


def main():
    conn = sqlite3.connect(":memory:")
    conn.executescript(SCHEMA)

    try:
        import apsw
        aconn = apsw.Connection(":memory:")
        aconn.execute(SCHEMA)
    except ImportError:
        aconn = None

    print("column metadata available:", has_column_metadata(), "| apsw:", aconn is not None)
    for q in QUERIES:
        print("\nQ:", q)
        # 1. ctypes metadata bridge
        try:
            print("  metadata :", fmt(resolve_columns_for_connection(conn, q)))
        except Exception as e:
            print("  metadata : ERR", type(e).__name__, e)
        # 2. EXPLAIN
        try:
            print("  explain  :", fmt(resolve_columns_via_explain(conn, q)))
        except Exception as e:
            print("  explain  : ERR", type(e).__name__, e)
        # 3. authorizer dependency set
        try:
            deps = sorted(f"{t}.{c}" for t, c in columns_read_set(conn, q))
            print("  auth-set :", "{" + ", ".join(deps) + "}")
        except Exception as e:
            print("  auth-set : ERR", type(e).__name__, e)
        # 4. apsw oracle
        if aconn is not None:
            try:
                d = aconn.execute(q).description_full
                s = ", ".join(f"{x[0]}->{(x[3]+'.'+x[4]) if x[3] else 'EXPR'}" for x in d)
                print("  apsw     :", s)
            except Exception as e:
                print("  apsw     : ERR", type(e).__name__, e)


if __name__ == "__main__":
    main()
