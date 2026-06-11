"""
Experiments: can psycopg + PostgreSQL enforce the same protections Datasette
relies on for SQLite when running untrusted queries?

  1. statement_timeout  -> analogue of Datasette's progress-handler time limit
  2. read-only enforcement -> analogue of mode=ro / PRAGMA query_only=1
  3. row / memory limits -> analogue of max_returned_rows
  4. blocking allocation / temp file abuse -> temp_file_limit, work_mem

Run against the local server started on the unix socket in /tmp/pgrun.
"""
import time
import psycopg

HOST = "/tmp/pgrun"
SUPERUSER = dict(host=HOST, user="postgres", dbname="testdb")
UNTRUSTED = dict(host=HOST, user="untrusted", dbname="testdb")


def section(title):
    print("\n" + "=" * 70)
    print(title)
    print("=" * 70)


def run(label, fn):
    t0 = time.perf_counter()
    try:
        result = fn()
        dt = (time.perf_counter() - t0) * 1000
        print(f"[OK ]  {label}: {result}  ({dt:.0f}ms)")
    except Exception as e:
        dt = (time.perf_counter() - t0) * 1000
        print(f"[ERR]  {label}: {type(e).__name__}: {e}  ({dt:.0f}ms)")


# ---------------------------------------------------------------------------
section("1. statement_timeout aborts a runaway query")
# The untrusted role has statement_timeout=1000ms set via ALTER ROLE.

def runaway_recursive_cte():
    # PostgreSQL analogue of an infinite recursive CTE / CPU spin.
    with psycopg.connect(**UNTRUSTED) as conn:
        with conn.cursor() as cur:
            cur.execute("""
                WITH RECURSIVE t(n) AS (
                    SELECT 1
                    UNION ALL
                    SELECT n + 1 FROM t
                )
                SELECT count(*) FROM t
            """)
            return cur.fetchone()

run("infinite recursive CTE (expect timeout ~1s)", runaway_recursive_cte)


def pg_sleep_long():
    with psycopg.connect(**UNTRUSTED) as conn:
        with conn.cursor() as cur:
            cur.execute("SELECT pg_sleep(10)")
            return cur.fetchone()

run("pg_sleep(10) (expect timeout ~1s)", pg_sleep_long)


def confirm_role_timeout():
    with psycopg.connect(**UNTRUSTED) as conn:
        with conn.cursor() as cur:
            cur.execute("SHOW statement_timeout")
            return cur.fetchone()[0]

run("statement_timeout setting on untrusted role", confirm_role_timeout)


# A per-connection / per-query override is also possible without relying on
# the role default - this is the closest analogue to Datasette's per-query
# custom_time_limit.
def per_query_timeout():
    with psycopg.connect(**SUPERUSER) as conn:
        with conn.cursor() as cur:
            cur.execute("SET statement_timeout = '300ms'")
            t0 = time.perf_counter()
            try:
                cur.execute("SELECT pg_sleep(5)")
            except psycopg.errors.QueryCanceled:
                return f"cancelled after {(time.perf_counter()-t0)*1000:.0f}ms"

run("per-connection SET statement_timeout=300ms on superuser", per_query_timeout)


# ---------------------------------------------------------------------------
section("2. Read-only enforcement")

def write_blocked_by_readonly_role():
    # untrusted role has default_transaction_read_only = on
    with psycopg.connect(**UNTRUSTED) as conn:
        with conn.cursor() as cur:
            cur.execute("INSERT INTO items(name) VALUES ('hacked')")
            return "INSERT SUCCEEDED - BAD"

run("INSERT as untrusted role (expect read-only error)", write_blocked_by_readonly_role)


def update_blocked():
    with psycopg.connect(**UNTRUSTED) as conn:
        with conn.cursor() as cur:
            cur.execute("UPDATE items SET name='x' WHERE id=1")
            return "UPDATE SUCCEEDED - BAD"

run("UPDATE as untrusted role (expect read-only error)", update_blocked)


def ddl_blocked():
    with psycopg.connect(**UNTRUSTED) as conn:
        with conn.cursor() as cur:
            cur.execute("CREATE TABLE evil(x int)")
            return "CREATE SUCCEEDED - BAD"

run("CREATE TABLE as untrusted role (expect error)", ddl_blocked)


def select_allowed():
    with psycopg.connect(**UNTRUSTED) as conn:
        with conn.cursor() as cur:
            cur.execute("SELECT count(*) FROM items")
            return f"count={cur.fetchone()[0]}"

run("SELECT as untrusted role (expect success)", select_allowed)


def readonly_session_attr():
    # psycopg can also force the *session* to read-only regardless of role.
    with psycopg.connect(**SUPERUSER) as conn:
        conn.read_only = True
        with conn.cursor() as cur:
            try:
                cur.execute("INSERT INTO items(name) VALUES ('via-superuser')")
                return "INSERT SUCCEEDED - BAD (read_only attr ignored)"
            except psycopg.errors.ReadOnlySqlTransaction:
                return "blocked by conn.read_only=True even as superuser"

run("conn.read_only=True blocks write even for superuser", readonly_session_attr)


# Can the untrusted role escape read-only by issuing its own SET?
def escape_readonly_attempt():
    with psycopg.connect(**UNTRUSTED) as conn:
        with conn.cursor() as cur:
            cur.execute("SET default_transaction_read_only = off")
            try:
                cur.execute("INSERT INTO items(name) VALUES ('escaped')")
                return "ESCAPED read-only - BAD"
            except Exception as e:
                return f"still blocked: {type(e).__name__}"

run("untrusted tries SET read_only=off then INSERT", escape_readonly_attempt)


# ---------------------------------------------------------------------------
section("3. Resource limits: rows, memory, temp files")

def row_limit_via_fetchmany():
    # Datasette uses fetchmany(max+1). psycopg server-side cursor streams.
    with psycopg.connect(**UNTRUSTED) as conn:
        with conn.cursor(name="srv") as cur:  # named => server-side cursor
            cur.execute("SELECT * FROM generate_series(1, 10000000)")
            rows = cur.fetchmany(1001)
            return f"fetched {len(rows)} rows then stopped (no full materialise)"

run("server-side cursor + fetchmany caps rows streamed", row_limit_via_fetchmany)


def temp_file_limit():
    # A big sort can spill to disk; temp_file_limit caps that.
    with psycopg.connect(**SUPERUSER) as conn:
        with conn.cursor() as cur:
            cur.execute("SET temp_file_limit = '10MB'")
            cur.execute("SET work_mem = '64kB'")
            cur.execute("SET statement_timeout = 0")
            try:
                cur.execute(
                    "SELECT count(*) FROM (SELECT * FROM generate_series(1, 50000000) "
                    "ORDER BY random()) s"
                )
                return "completed without hitting temp_file_limit"
            except psycopg.errors.ConfigurationLimitExceeded as e:
                return f"blocked by temp_file_limit: {str(e)[:60]}"
            except Exception as e:
                return f"{type(e).__name__}: {str(e)[:60]}"

run("temp_file_limit=10MB caps disk spill of large sort", temp_file_limit)


# ---------------------------------------------------------------------------
section("4. Read-only is soft; GRANTs are the hard barrier")


def escape_via_fresh_txn():
    # In autocommit each statement is its own txn, so SET read_only=off DOES
    # take effect for the subsequent INSERT. The write is then stopped only by
    # the privilege system (untrusted has SELECT but not INSERT on items).
    with psycopg.connect(**UNTRUSTED, autocommit=True) as conn:
        with conn.cursor() as cur:
            cur.execute("SET default_transaction_read_only = off")
            try:
                cur.execute("INSERT INTO items(name) VALUES ('escaped')")
                return "INSERT SUCCEEDED - role had write privilege"
            except psycopg.errors.InsufficientPrivilege:
                return "read_only flipped off, but blocked by GRANTs (the real barrier)"

run("untrusted flips read_only=off in fresh txn", escape_via_fresh_txn)


def server_side_cursor_caps_memory():
    with psycopg.connect(**SUPERUSER) as conn:  # autocommit off -> txn block
        with conn.cursor() as c0:
            c0.execute("SET statement_timeout = 0")
        with conn.cursor(name="srv") as cur:
            cur.itersize = 1000
            cur.execute(
                "SELECT g, repeat('x',100) FROM generate_series(1,5000000) g"
            )
            rows = cur.fetchmany(1001)
            conn.rollback()
            return f"pulled {len(rows)} of 5M wide rows; client memory bounded"

run("server-side cursor + fetchmany bounds client memory", server_side_cursor_caps_memory)


if __name__ == "__main__":
    print("psycopg version:", psycopg.__version__)
