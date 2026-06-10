"""
Experiment 1: How strict is DuckDB read_only mode?

Tests whether an untrusted SELECT-ish query can still:
  - write to the database (INSERT/CREATE) -- should be blocked
  - write to the filesystem via COPY ... TO
  - read arbitrary local files via read_csv / read_text
  - ATTACH another database read-write
  - INSTALL / LOAD extensions
  - change config via SET / PRAGMA
"""

import os
import tempfile
import duckdb

print("duckdb", duckdb.__version__)

workdir = tempfile.mkdtemp(prefix="duckdb_exp_")
dbpath = os.path.join(workdir, "data.duckdb")

# First create a database with some data using a writable connection.
con = duckdb.connect(dbpath)
con.execute("CREATE TABLE t (id INTEGER, name TEXT)")
con.execute("INSERT INTO t VALUES (1, 'alice'), (2, 'bob')")
con.close()

# A secret file on disk we do NOT want untrusted SQL to read.
secret_path = os.path.join(workdir, "secret.txt")
with open(secret_path, "w") as f:
    f.write("TOP SECRET CONTENTS\n")

outfile = os.path.join(workdir, "exfil.csv")


def attempt(con, label, sql):
    try:
        result = con.execute(sql).fetchall()
        print(f"  [ALLOWED] {label}: {result!r:.120}")
        return True
    except Exception as e:
        print(f"  [blocked] {label}: {type(e).__name__}: {str(e).splitlines()[0][:120]}")
        return False


print("\n=== read_only=True connection ===")
ro = duckdb.connect(dbpath, read_only=True)
attempt(ro, "SELECT", "SELECT * FROM t")
attempt(ro, "INSERT", "INSERT INTO t VALUES (3, 'eve')")
attempt(ro, "CREATE TABLE", "CREATE TABLE evil (x INT)")
attempt(ro, "CREATE TEMP TABLE", "CREATE TEMP TABLE tmp AS SELECT 1")
attempt(ro, "COPY TO file (exfil)", f"COPY t TO '{outfile}' (HEADER, FORMAT CSV)")
print("    exfil.csv exists after COPY TO:", os.path.exists(outfile))
attempt(ro, "read_csv arbitrary file", f"SELECT * FROM read_csv('{secret_path}', columns={{'line':'VARCHAR'}}, header=false)")
attempt(ro, "read_text arbitrary file", f"SELECT * FROM read_text('{secret_path}')")
attempt(ro, "ATTACH rw db", f"ATTACH '{os.path.join(workdir, 'extra.duckdb')}' AS extra")
attempt(ro, "INSTALL httpfs", "INSTALL httpfs")
attempt(ro, "SET memory_limit", "SET memory_limit='100MB'")
attempt(ro, "PRAGMA database_list", "PRAGMA database_list")
attempt(ro, "PRAGMA version", "PRAGMA version")
ro.close()
print("\nworkdir:", workdir)
