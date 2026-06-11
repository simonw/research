# Notes

## Step 1: Datasette's SQLite safety model (cloned to /tmp/datasette)

Key findings from reading the code:

### 1. Read-only connections (datasette/database.py `Database.connect()`, ~line 137)
- File databases opened with SQLite URI: `file:{path}?mode=ro` (mutable files)
  or `file:{path}?immutable=1` (immutable files). `?nolock=1` optionally added.
- Shared in-memory databases (`file:name?mode=memory&cache=shared`) can't use
  mode=ro, so Datasette runs `PRAGMA query_only=1` on read connections instead.
- Writes go through a *separate* dedicated write connection on a single
  write thread (`execute_write_fn` / `_write_queue`), opened without mode=ro
  and with `isolation_level="IMMEDIATE"`.

### 2. Time limits (datasette/utils/__init__.py `sqlite_timelimit`, line 245)
- Context manager wraps every read query (database.py `execute()`, line 532).
- Uses `conn.set_progress_handler(handler, n)` with n=1000 VM instructions
  (n=1 if limit <= 20ms, for the test suite). Comment: ~0.08ms per 1000 ops.
- Handler compares time.perf_counter() against a deadline; returning 1 from
  the progress handler makes SQLite abort the query with
  `sqlite3.OperationalError: interrupted`, which Datasette converts to
  QueryInterrupted (database.py line 546-548).
- Default `sql_time_limit_ms` setting = 1000ms; can be lowered per-query
  via `custom_time_limit` (?_timelimit= param) but never raised above setting.

### 3. Defense in depth
- `validate_sql_select` (utils/__init__.py line 321): regex allowlist - SQL must
  start with select/with/explain select/explain query plan; PRAGMA blocked
  except an allowlist of pragma_*() table-valued functions.
- `max_returned_rows` (default 1000): fetchmany(max+1) truncation so a huge
  result set can't exhaust memory.
- Queries run on a thread pool (num_sql_threads=3 default), one connection per
  thread, so a slow query can't block the event loop.

So: **readonly = enforced by the connection itself (mode=ro/immutable/query_only),
not by SQL parsing; time limit = enforced inside the SQLite VM via progress
handler, so it catches runaway CPU loops like recursive CTEs.**


## Step 2: PostgreSQL + psycopg experiments (pg_experiments.py)

Setup: PostgreSQL 16 in /tmp/pgdata, unix socket /tmp/pgrun, run as the
`postgres` OS user (initdb refuses to run as root). psycopg 3.3.4 in
/tmp/pgvenv. Created an `untrusted` LOGIN role with:
    ALTER ROLE untrusted SET statement_timeout = '1000ms';
    ALTER ROLE untrusted SET default_transaction_read_only = on;
    REVOKE ALL/TEMP ON DATABASE testdb FROM PUBLIC;
    GRANT CONNECT; GRANT USAGE ON SCHEMA; GRANT SELECT ON ALL TABLES.

### Results
1. TIME LIMIT (statement_timeout) — WORKS, and is *better* than SQLite's:
   - Infinite recursive CTE -> QueryCanceled after ~1015ms.
   - pg_sleep(10) -> QueryCanceled after ~1005ms. (SQLite's progress handler
     would NOT interrupt a sleep, only VM instructions — PostgreSQL's timer
     catches sleeping/blocked queries too.)
   - Settable three ways: per-role (ALTER ROLE), per-session (SET), or per
     connection via options='-c statement_timeout=1000'. Per-session SET is
     the direct analogue of Datasette's per-query custom_time_limit.

2. READ-ONLY — WORKS, but with an important distinction:
   - default_transaction_read_only=on blocks INSERT/UPDATE/CREATE with
     ReadOnlySqlTransaction.
   - psycopg conn.read_only=True forces it client-side and blocks writes even
     for a superuser connection (ReadOnlySqlTransaction).
   - CAVEAT (important): default_transaction_read_only is a *soft* guard. A
     role that owns its session can `SET default_transaction_read_only=off`
     for a fresh (autocommit) transaction. It only held in my first test
     because the SET + INSERT shared one already-read-only transaction.
   - The REAL hard barrier is the GRANT privilege model: with only SELECT
     granted, the escaped INSERT was blocked by InsufficientPrivilege. So the
     robust recipe = least-privilege role (SELECT only, no write grants,
     REVOKE TEMP/CREATE) PLUS read-only transaction PLUS statement_timeout.
     This is unlike SQLite where mode=ro is enforced at the file-open level
     and cannot be escaped from within the connection at all.

3. RESOURCE LIMITS:
   - Server-side (named) cursor + fetchmany(N) bounds client memory the same
     way Datasette's fetchmany(max_returned_rows+1) does. Named cursors
     require a transaction block (fail under autocommit).
   - temp_file_limit (e.g. 10MB) aborts a huge disk-spilling sort with
     ConfigurationLimitExceeded — a knob SQLite lacks. work_mem caps in-memory
     sort/hash. These guard against resource exhaustion that a time limit
     alone might not catch quickly.

### Summary mapping
   SQLite (Datasette)              ->  PostgreSQL (psycopg)
   ----------------------------------------------------------
   mode=ro / immutable=1           ->  least-privilege GRANTs (hard) +
   PRAGMA query_only=1             ->    default_transaction_read_only /
                                         conn.read_only (soft)
   progress handler time limit     ->  statement_timeout (also catches sleeps)
   max_returned_rows + fetchmany   ->  server-side cursor + fetchmany
   (no equivalent)                 ->  temp_file_limit, work_mem, idle txn
                                         timeout, per-role connection limits

## Step 3: Datasette-style PoC (pg_datasette_poc.py)

Reimplemented Datasette's Database.execute() contract — Results(rows,
truncated, description), QueryInterrupted, async execute() — backed by psycopg.
Each connection: conn.read_only=True + options='-c statement_timeout=...'.
Per-query custom_time_limit uses SET LOCAL statement_timeout.

Demo output (poc_output.txt): all 5 checks pass — plain SELECT, row-cap
truncation at 1000, recursive-CTE timeout, write rejected, tight 200ms limit.

### Bonus: full Datasette-on-PostgreSQL fork
A complete fork is impractical in scope: Datasette's view/introspection layer
is wired to sqlite3 specifics (sqlite3.Row, PRAGMA table_info/database_list,
ATTACH, the dedicated write-thread model, hash/size of .db files). The
PostgresDatabase class here is the core storage adapter such a fork would be
built around — it preserves the exact safety contract, which was the crux of
the question. A real port would also swap the introspection queries for
information_schema/pg_catalog and drop the file-immutability code paths.
