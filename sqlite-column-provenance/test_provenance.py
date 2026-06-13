"""Exercise the resolver via a pure stdlib sqlite3.Connection (file + memory),
and cross-check every result against apsw's description_full where available.
"""
import os
import sqlite3
import tempfile

from column_provenance import resolve_columns_for_connection, has_column_metadata

SCHEMA = """
CREATE TABLE t1 (id INTEGER PRIMARY KEY, name TEXT, extra TEXT);
CREATE TABLE t2 (id INTEGER PRIMARY KEY, name TEXT, age INTEGER);
INSERT INTO t1 VALUES (1,'alice','x'),(2,'bob','y');
INSERT INTO t2 VALUES (10,'alice',30),(11,'carol',40);
"""

QUERIES = [
    "select id, name from t1",
    "select * from t1",
    # user's "harder still": id only exists in t1 here? no -> use explicit table
    "select t1.id, t1.name, age from t1 join t2 on t1.name = t2.name",
    "select id, name || '-suffix' from t1",
    "select t1.*, t2.age from t1 join t2 on t1.name=t2.name",
    "select id as the_id, name as nm from t1",
    "select count(*), max(age) from t2",
    "select age from (select * from t2) sub",
    "with c as (select id, name from t1) select * from c",
    "select name from t1 union select name from t2",
]


def show(title, conn):
    print(f"\n##### {title}")
    for q in QUERIES:
        print("Q:", q)
        try:
            for c in resolve_columns_for_connection(conn, q):
                print("   ", c)
        except Exception as e:
            print("    ERR", type(e).__name__, e)


def cross_check_with_apsw():
    try:
        import apsw
    except ImportError:
        print("\n(apsw not installed — skipping cross-check)")
        return
    print("\n##### cross-check: stdlib+ctypes vs apsw description_full")
    sconn = sqlite3.connect(":memory:")
    sconn.executescript(SCHEMA)
    aconn = apsw.Connection(":memory:")
    aconn.execute(SCHEMA)
    mism = 0
    for q in QUERIES:
        try:
            mine = [(c.output_name, c.table, c.origin_column)
                    for c in resolve_columns_for_connection(sconn, q)]
        except Exception as e:
            mine = f"ERR:{type(e).__name__}"
        try:
            theirs = [(d[0], d[3], d[4]) for d in aconn.execute(q).description_full]
        except Exception as e:
            theirs = f"ERR:{type(e).__name__}"
        ok = mine == theirs
        if not ok:
            mism += 1
        print(f"  [{'OK ' if ok else 'XX '}] {q}")
        if not ok:
            print("       mine  :", mine)
            print("       apsw  :", theirs)
    print(f"\n  {len(QUERIES)-mism}/{len(QUERIES)} match")


if __name__ == "__main__":
    print("column metadata available:", has_column_metadata())

    # in-memory stdlib connection (serialize/deserialize bridge path)
    mem = sqlite3.connect(":memory:")
    mem.executescript(SCHEMA)
    show("stdlib :memory: connection (serialize bridge)", mem)

    # file-backed stdlib connection (direct file path)
    fd, path = tempfile.mkstemp(suffix=".db")
    os.close(fd)
    try:
        fconn = sqlite3.connect(path)
        fconn.executescript(SCHEMA)
        fconn.commit()
        show("stdlib file connection (direct path)", fconn)
    finally:
        os.remove(path)

    cross_check_with_apsw()
