"""
Proof-of-concept: a Datasette-style Database class backed by PostgreSQL.

Datasette's datasette/database.py Database.execute() runs untrusted SELECTs
against SQLite with three guarantees:
  * the connection is read-only (mode=ro / PRAGMA query_only=1)
  * a per-query time limit (progress handler) aborts runaway queries
  * results are truncated to max_returned_rows so memory can't be exhausted

This module reproduces that exact contract against PostgreSQL via psycopg,
so a Datasette-like app could swap the storage engine and keep the same
safety properties. It mirrors the public surface Datasette relies on:
Results(rows, truncated, description), QueryInterrupted, and an async execute().
"""
import asyncio
import psycopg
from psycopg import sql as pgsql


class QueryInterrupted(Exception):
    "Raised when a query exceeds its time limit (mirrors datasette.database)."


class Results:
    def __init__(self, rows, truncated, description):
        self.rows = rows
        self.truncated = truncated
        self.description = description

    def __iter__(self):
        return iter(self.rows)

    def __len__(self):
        return len(self.rows)


class PostgresDatabase:
    """Mirror of datasette.database.Database, backed by PostgreSQL.

    The untrusted role is expected to already be least-privileged (SELECT
    only, no write GRANTs, REVOKE TEMP/CREATE) - that is the hard read-only
    barrier. We additionally pin every connection to a read-only transaction
    and a statement_timeout as defence in depth.
    """

    def __init__(self, conninfo, sql_time_limit_ms=1000, max_returned_rows=1000):
        self.conninfo = conninfo
        self.sql_time_limit_ms = sql_time_limit_ms
        self.max_returned_rows = max_returned_rows

    def _connect(self):
        # read_only=True forces a read-only transaction (analogue of mode=ro);
        # options sets statement_timeout for the whole session (analogue of the
        # SQLite progress-handler time limit).
        conn = psycopg.connect(
            self.conninfo,
            autocommit=False,
            options=f"-c statement_timeout={self.sql_time_limit_ms}",
        )
        conn.read_only = True
        return conn

    async def execute(self, sql, params=None, custom_time_limit=None, truncate=True):
        """Run a read-only query with a time limit and row cap, off the event loop."""
        return await asyncio.to_thread(
            self._execute_sync, sql, params, custom_time_limit, truncate
        )

    def _execute_sync(self, sql, params, custom_time_limit, truncate):
        time_limit_ms = self.sql_time_limit_ms
        if custom_time_limit and custom_time_limit < time_limit_ms:
            time_limit_ms = custom_time_limit
        conn = self._connect()
        try:
            with conn.cursor() as cur:
                if custom_time_limit:
                    cur.execute(
                        pgsql.SQL("SET LOCAL statement_timeout = {}").format(
                            pgsql.Literal(time_limit_ms)
                        )
                    )
                try:
                    cur.execute(sql, params)
                except psycopg.errors.QueryCanceled as e:
                    raise QueryInterrupted(str(e)) from e
                except (
                    psycopg.errors.InsufficientPrivilege,
                    psycopg.errors.ReadOnlySqlTransaction,
                ) as e:
                    # write attempt or unauthorised table - report, don't crash
                    raise QueryInterrupted(f"not permitted: {e}") from e

                max_rows = self.max_returned_rows
                if truncate:
                    rows = cur.fetchmany(max_rows + 1)
                    truncated = len(rows) > max_rows
                    rows = rows[:max_rows]
                else:
                    rows = cur.fetchall()
                    truncated = False
                description = cur.description
            conn.rollback()
            return Results(rows, truncated, description)
        finally:
            conn.close()


# --------------------------------------------------------------------------
async def _demo():
    db = PostgresDatabase(
        "host=/tmp/pgrun user=untrusted dbname=testdb",
        sql_time_limit_ms=1000,
        max_returned_rows=1000,
    )

    print("1) ordinary SELECT")
    r = await db.execute("SELECT id, name FROM items ORDER BY id LIMIT 3")
    print("   rows:", r.rows, "truncated:", r.truncated)

    print("2) row cap / truncation (10000 rows, cap 1000)")
    r = await db.execute("SELECT * FROM generate_series(1, 10000)")
    print("   returned:", len(r.rows), "truncated:", r.truncated)

    print("3) time limit on runaway recursive CTE")
    try:
        await db.execute(
            "WITH RECURSIVE t(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM t) "
            "SELECT count(*) FROM t"
        )
        print("   NO TIMEOUT - BAD")
    except QueryInterrupted as e:
        print("   QueryInterrupted:", str(e)[:50])

    print("4) write attempt is rejected (read-only + least privilege)")
    try:
        await db.execute("INSERT INTO items(name) VALUES ('x')")
        print("   WRITE SUCCEEDED - BAD")
    except QueryInterrupted as e:
        print("   blocked:", str(e)[:60])

    print("5) custom (tighter) per-query time limit, 200ms")
    try:
        await db.execute("SELECT pg_sleep(5)", custom_time_limit=200)
        print("   NO TIMEOUT - BAD")
    except QueryInterrupted as e:
        print("   QueryInterrupted (tight limit):", str(e)[:40])


if __name__ == "__main__":
    asyncio.run(_demo())
