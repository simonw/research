"""SQLite crashes when an eponymous virtual table is used before and after sqlite3_deserialize.

Each case runs in a child process so a segfault is reported, not fatal:

    python3 repro.py                                # built-in tables only
    python3 repro.py --ext path/to/libsparqlite.so  # also sparqlite's sparql_rows

A case first runs its query on a fresh in-memory database, then replaces main with
Connection.deserialize() and prepares the same query again. The statement cache is
disabled so the second run really prepares a new statement.
"""
import argparse
import sqlite3
import subprocess
import sys

CASES = [
    ("PRAGMA statement", "PRAGMA index_info('sqlite_autoindex_t_1')"),
    ("pragma_index_info", "SELECT * FROM pragma_index_info('sqlite_autoindex_t_1')"),
    ("pragma_table_xinfo", "SELECT * FROM pragma_table_xinfo('t')"),
    ("pragma_database_list", "SELECT name FROM pragma_database_list"),
    ("pragma_schema_version", "SELECT * FROM pragma_schema_version"),
    ("json_each", "SELECT * FROM json_each('[1]')"),
]
EXT_CASES = [
    ("sparql_query (scalar)", "SELECT sparql_query('SELECT ?s WHERE { ?s ?p ?o }', 'tsv')"),
    ("sparql_rows (eponymous)", "SELECT v0 FROM sparql_rows('SELECT ?s WHERE { ?s ?p ?o }')"),
]


def connect(ext):
    conn = sqlite3.connect(":memory:", cached_statements=0)
    conn.execute("CREATE TABLE t(a PRIMARY KEY, b) WITHOUT ROWID")
    if ext:
        conn.enable_load_extension(True)
        conn.load_extension(ext)
        conn.execute("SELECT sparql_init()")
        conn.execute("SELECT sparql_load('<http://ex/a> <http://ex/p> <http://ex/b> .')")
    return conn


def child(sql, ext):
    conn = connect(ext)
    conn.execute(sql).fetchall()
    conn.deserialize(connect(ext).serialize())
    conn.execute(sql).fetchall()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--ext")
    parser.add_argument("--child")
    args = parser.parse_args()
    if args.child:
        child(args.child, args.ext)
        return
    print(f"SQLite {sqlite3.sqlite_version}")
    cases = CASES + (EXT_CASES if args.ext else [])
    for name, sql in cases:
        cmd = [sys.executable, "-I", __file__, "--child", sql] + (["--ext", args.ext] if args.ext else [])
        rc = subprocess.run(cmd, capture_output=True).returncode
        verdict = "ok" if rc == 0 else ("SEGFAULT" if rc in (-11, 139) else f"exit {rc}")
        print(f"  {name:<26} {verdict}")


if __name__ == "__main__":
    main()
