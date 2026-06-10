"""
Experiment 3: Can DuckDB queries be time-limited like SQLite's progress handler?

DuckDB has no per-opcode progress-handler timeout. Two mechanisms to test:
  (A) Watchdog thread calling con.interrupt() -- requires the GIL to be
      released during execute() so the watchdog thread can run.
  (B) Whether a long-running query actually releases the GIL.

Also test ATTACH of arbitrary path under enable_external_access=false.
"""

import os
import tempfile
import threading
import time
import duckdb

print("duckdb", duckdb.__version__)

config = {
    "enable_external_access": "false",
    "allow_community_extensions": "false",
    "autoinstall_known_extensions": "false",
    "autoload_known_extensions": "false",
}

# In-memory hardened connection
con = duckdb.connect(":memory:", config={**config, "threads": "1"})

# A deliberately expensive query: large cross join aggregation.
HEAVY = """
SELECT count(*) FROM range(1000000) a, range(1000000) b
WHERE (a.range * b.range) % 7 = 0
"""


def run_with_interrupt(con, sql, timeout_s):
    """Run sql, interrupt() from a watchdog thread after timeout_s."""
    done = threading.Event()

    def watchdog():
        if not done.wait(timeout_s):
            con.interrupt()

    t = threading.Thread(target=watchdog, daemon=True)
    start = time.perf_counter()
    t.start()
    try:
        con.execute(sql).fetchall()
        status = "completed"
    except Exception as e:
        status = f"{type(e).__name__}: {str(e).splitlines()[0][:80]}"
    finally:
        done.set()
        t.join()
    elapsed = time.perf_counter() - start
    return status, elapsed


print("\n=== (A) Watchdog interrupt() with 1.0s timeout ===")
status, elapsed = run_with_interrupt(con, HEAVY, 1.0)
print(f"  status={status}  elapsed={elapsed:.3f}s")

print("\n=== (B) Same query, no timeout (baseline, capped query) ===")
SMALL = "SELECT count(*) FROM range(3000) a, range(3000) b WHERE (a.range*b.range)%7=0"
start = time.perf_counter()
n = con.execute(SMALL).fetchall()
print(f"  small query result={n} elapsed={time.perf_counter()-start:.3f}s")

# ATTACH arbitrary path under lockdown?
print("\n=== ATTACH arbitrary path with external access disabled ===")
workdir = tempfile.mkdtemp(prefix="duckdb_attach_")
other = os.path.join(workdir, "other.duckdb")
duckdb.connect(other).close()  # create a real db file
try:
    con.execute(f"ATTACH '{other}' AS other (READ_ONLY)")
    print("  [ALLOWED] attached arbitrary path -- escape!")
except Exception as e:
    print(f"  [blocked] ATTACH: {type(e).__name__}: {str(e).splitlines()[0][:90]}")
con.close()
