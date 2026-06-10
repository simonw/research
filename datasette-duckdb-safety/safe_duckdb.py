"""
safe_duckdb: a reusable helper that locks a DuckDB connection down to the
same safety guarantees Datasette relies on for SQLite:

  * strict read-only (engine refuses writes to the database)
  * no filesystem / network escape (COPY TO, read_csv of local files,
    ATTACH arbitrary paths, INSTALL/LOAD extensions all blocked)
  * configuration locked so untrusted SQL cannot loosen the above
  * resource caps (memory_limit, threads, temp dir size)
  * a query time limit, implemented with a watchdog thread that calls
    connection.interrupt() -- DuckDB's analogue of SQLite's progress handler.
"""

import threading
import time
from contextlib import contextmanager

import duckdb

# Settings that sandbox a DuckDB connection for untrusted SELECT traffic.
HARDENED_CONFIG = {
    "enable_external_access": "false",     # blocks COPY TO/FROM, read_csv(file), ATTACH new paths, http
    "allow_community_extensions": "false",
    "autoinstall_known_extensions": "false",
    "autoload_known_extensions": "false",
    "allow_unsigned_extensions": "false",
    "lock_configuration": "true",          # must come last; prevents SET undoing the above
}


def connect_readonly(path, memory_limit="256MB", threads=1, extra_config=None):
    """Open a hardened, read-only DuckDB connection to an existing db file."""
    config = {
        "memory_limit": memory_limit,
        "threads": str(threads),
    }
    if extra_config:
        config.update(extra_config)
    # lock_configuration must be applied last, so build the dict in order.
    config.update(HARDENED_CONFIG)
    return duckdb.connect(path, read_only=True, config=config)


class QueryInterrupted(Exception):
    "Raised when a query exceeds its time limit (mirrors Datasette's exception)."


@contextmanager
def duckdb_timelimit(con, ms):
    """
    Interrupt the connection's running query after `ms` milliseconds.

    Analogous to datasette.utils.sqlite_timelimit, but DuckDB has no
    progress-handler timeout, so we use a watchdog thread + con.interrupt().
    DuckDB releases the GIL while a query runs, so the watchdog can fire.
    """
    deadline = time.perf_counter() + (ms / 1000)
    cancel = threading.Event()
    fired = threading.Event()

    def watchdog():
        remaining = deadline - time.perf_counter()
        if remaining > 0 and cancel.wait(remaining):
            return  # query finished in time
        if not cancel.is_set():
            fired.set()
            con.interrupt()

    t = threading.Thread(target=watchdog, daemon=True)
    t.start()
    try:
        yield fired
    finally:
        cancel.set()
        t.join()


def execute(con, sql, params=None, time_limit_ms=1000, max_rows=1000):
    """Run an untrusted SELECT with a time limit and row cap."""
    with duckdb_timelimit(con, time_limit_ms) as fired:
        try:
            rel = con.execute(sql, params or [])
            rows = rel.fetchmany(max_rows + 1)
        except duckdb.InterruptException as e:
            raise QueryInterrupted(str(e)) from e
        except Exception:
            if fired.is_set():
                raise QueryInterrupted("interrupted")
            raise
    truncated = len(rows) > max_rows
    return rows[:max_rows], truncated


if __name__ == "__main__":
    print("duckdb", duckdb.__version__)
    con = duckdb.connect(":memory:", config={"threads": "1"})

    print("\n=== memory_limit enforcement ===")
    con.execute("SET memory_limit='100MB'")
    try:
        # Try to materialize a huge string aggregation that blows the cap.
        con.execute(
            "SELECT count(*) FROM (SELECT repeat('x', 1000000) FROM range(100000))"
        ).fetchall()
        print("  completed (no OOM hit)")
    except Exception as e:
        print(f"  [blocked] {type(e).__name__}: {str(e).splitlines()[0][:90]}")

    print("\n=== time limit via helper (250ms) ===")
    HEAVY = "SELECT count(*) FROM range(1000000) a, range(1000000) b WHERE (a.range*b.range)%7=0"
    start = time.perf_counter()
    try:
        execute(con, HEAVY, time_limit_ms=250)
        print("  completed (unexpected)")
    except QueryInterrupted:
        print(f"  QueryInterrupted after {time.perf_counter()-start:.3f}s")

    print("\n=== fast query + row cap ===")
    rows, truncated = execute(con, "SELECT * FROM range(5000)", time_limit_ms=1000, max_rows=1000)
    print(f"  rows={len(rows)} truncated={truncated}")
    con.close()
