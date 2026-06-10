"""
Experiment 2: Hardened DuckDB config for untrusted SQL.

The "Securing DuckDB" recipe: combine read_only with
  - enable_external_access=false   (blocks COPY TO/FROM, read_csv of files, ATTACH, etc.)
  - allow_community_extensions / autoinstall / autoload knobs
  - lock_configuration=true         (prevents SET undoing the above)
and verify untrusted SQL cannot escape.
"""

import os
import tempfile
import duckdb

print("duckdb", duckdb.__version__)

workdir = tempfile.mkdtemp(prefix="duckdb_lock_")
dbpath = os.path.join(workdir, "data.duckdb")
con = duckdb.connect(dbpath)
con.execute("CREATE TABLE t (id INTEGER, name TEXT)")
con.execute("INSERT INTO t VALUES (1, 'alice'), (2, 'bob')")
con.close()

secret_path = os.path.join(workdir, "secret.txt")
with open(secret_path, "w") as f:
    f.write("TOP SECRET\n")
outfile = os.path.join(workdir, "exfil.csv")


def attempt(con, label, sql):
    try:
        result = con.execute(sql).fetchall()
        print(f"  [ALLOWED] {label}: {result!r:.100}")
        return True
    except Exception as e:
        print(f"  [blocked] {label}: {type(e).__name__}: {str(e).splitlines()[0][:110]}")
        return False


config = {
    "enable_external_access": "false",
    "allow_community_extensions": "false",
    "autoinstall_known_extensions": "false",
    "autoload_known_extensions": "false",
    "lock_configuration": "true",   # must be set LAST conceptually; test it
}

print("\n=== read_only=True + hardened config ===")
con = duckdb.connect(dbpath, read_only=True, config=config)
attempt(con, "SELECT", "SELECT count(*) FROM t")
attempt(con, "INSERT", "INSERT INTO t VALUES (3, 'eve')")
attempt(con, "COPY TO file", f"COPY t TO '{outfile}' (HEADER, FORMAT CSV)")
print("    exfil.csv exists:", os.path.exists(outfile))
attempt(con, "read_csv arbitrary file", f"SELECT * FROM read_csv('{secret_path}', columns={{'l':'VARCHAR'}}, header=false)")
attempt(con, "ATTACH", f"ATTACH '{dbpath}' AS more (READ_ONLY)")
attempt(con, "INSTALL httpfs", "INSTALL httpfs")
attempt(con, "LOAD httpfs", "LOAD httpfs")
attempt(con, "read parquet over http", "SELECT * FROM read_parquet('https://example.com/x.parquet')")
# Try to undo the lockdown:
attempt(con, "SET enable_external_access=true (undo)", "SET enable_external_access=true")
attempt(con, "COPY TO after undo attempt", f"COPY t TO '{outfile}' (HEADER, FORMAT CSV)")
print("    exfil.csv exists after undo attempt:", os.path.exists(outfile))
con.close()
print("\nworkdir:", workdir)
