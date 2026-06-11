# Running untrusted queries: Datasette/SQLite vs psycopg/PostgreSQL

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

Datasette lets anyone type arbitrary SQL into a public box and run it against a
database without fear of data corruption or resource exhaustion. This
investigation documents exactly how it achieves that with SQLite, then
determines whether **psycopg + PostgreSQL** can be locked down the same way.

Short answer: **yes — and in some respects PostgreSQL does it better** (its
time limit also catches sleeping/blocked queries, and it adds disk/memory
knobs SQLite lacks). The one important difference is that PostgreSQL's
read-only flag is a *soft* setting; the hard barrier is the GRANT
privilege system.

## How Datasette protects SQLite

Read from a fresh clone of `simonw/datasette`. Two mechanisms do the work:

### 1. Read-only at the connection level
`Database.connect()` (`datasette/database.py`) opens query connections with a
SQLite URI that makes writes impossible:

- mutable files: `file:{path}?mode=ro`
- immutable files: `file:{path}?immutable=1`
- shared in-memory DBs (where `mode=ro` is unavailable): `PRAGMA query_only=1`

Writes are physically segregated onto a **separate write connection** running on
a single dedicated write thread (`isolation_level="IMMEDIATE"`). Read
connections simply *cannot* write — it's enforced by SQLite at file-open time,
not by inspecting the SQL.

### 2. A per-query time limit inside the SQLite VM
`sqlite_timelimit()` (`datasette/utils/__init__.py`) wraps every read query:

```python
def handler():
    if time.perf_counter() >= deadline:
        return 1            # non-zero aborts the query
conn.set_progress_handler(handler, n)   # checked every n=1000 VM ops
```

A non-zero return from the progress handler makes SQLite abort with
`OperationalError: interrupted`, which Datasette turns into `QueryInterrupted`.
Default limit is `sql_time_limit_ms = 1000`. This stops runaway CPU work such
as an infinite recursive CTE. (Limitation: the progress handler fires on VM
instructions, so a query that *sleeps* wouldn't be caught — not an issue for
SQLite, which has no sleep.)

### 3. Defence in depth
- `validate_sql_select()` — regex allowlist; SQL must start with
  `select`/`with`/`explain`; PRAGMAs blocked except a safe allowlist.
- `max_returned_rows` (default 1000) via `fetchmany(max+1)` — bounds memory.
- Queries run on a thread pool so a slow one never blocks the event loop.

The crucial design point: **read-only and the time limit are enforced by the
engine/connection, not by parsing the SQL.** SQL validation is only an extra
layer.

## The same protections in PostgreSQL + psycopg

Setup (see `notes.md`): PostgreSQL 16, a least-privilege `untrusted` role, and
psycopg 3.3.4. `pg_experiments.py` exercises each protection.

| Datasette / SQLite | PostgreSQL / psycopg | Result |
|---|---|---|
| `mode=ro` / `immutable=1` (hard) | **least-privilege GRANTs** (SELECT only, REVOKE write/TEMP/CREATE) | hard barrier ✓ |
| `PRAGMA query_only=1` | `default_transaction_read_only=on` / `conn.read_only=True` | soft guard ✓ |
| progress-handler time limit | `statement_timeout` (per-role / per-session / per-connection) | ✓, also catches sleeps |
| `max_returned_rows` + `fetchmany` | server-side (named) cursor + `fetchmany` | ✓ |
| *(no equivalent)* | `temp_file_limit`, `work_mem`, idle-txn timeout, per-role conn limits | bonus ✓ |

### What the experiments showed
1. **Time limit works and is stronger.** With `statement_timeout=1000ms`, an
   infinite recursive CTE *and* `pg_sleep(10)` were both cancelled at ~1s with
   `QueryCanceled`. SQLite's progress handler would not have interrupted a
   sleep. Settable per-role (`ALTER ROLE`), per-session (`SET`), or per
   connection (`options='-c statement_timeout=...'`); the per-session form is
   the direct analogue of Datasette's per-query `custom_time_limit`.

2. **Read-only works — with a caveat.** `default_transaction_read_only=on` and
   psycopg's `conn.read_only=True` both reject INSERT/UPDATE/DDL with
   `ReadOnlySqlTransaction` (the latter even for a superuser connection).
   **But** a role that owns its session can `SET default_transaction_read_only
   = off` for a fresh transaction — so the flag is a convenience guard, not a
   security boundary. The *hard* barrier is the **privilege system**: with only
   `SELECT` granted, the escaped INSERT was stopped by `InsufficientPrivilege`.
   This differs from SQLite, where `mode=ro` is enforced at file-open and
   cannot be escaped from inside the connection.

   **Robust recipe:** least-privilege role (SELECT only, no write grants,
   `REVOKE TEMP/CREATE`) **plus** read-only transaction **plus**
   `statement_timeout`.

3. **Memory/disk limits.** A server-side cursor with `fetchmany(N)` bounds
   client memory exactly like Datasette's `fetchmany(max_returned_rows+1)`.
   `temp_file_limit=10MB` aborted a huge disk-spilling sort with
   `ConfigurationLimitExceeded` — a resource knob SQLite has no equivalent for.

## Bonus: a Datasette-style database backed by PostgreSQL

`pg_datasette_poc.py` reimplements Datasette's `Database.execute()` contract —
`Results(rows, truncated, description)`, `QueryInterrupted`, async `execute()`,
`custom_time_limit` — backed by psycopg. Every connection is pinned to
`conn.read_only=True` and a `statement_timeout`. Demo output (`poc_output.txt`):

```
1) ordinary SELECT                       -> rows returned
2) row cap / truncation (10000 -> 1000)  -> truncated: True
3) runaway recursive CTE                 -> QueryInterrupted
4) write attempt                         -> blocked (read-only)
5) custom 200ms limit on pg_sleep(5)     -> QueryInterrupted
```

A *full* fork of Datasette onto PostgreSQL is impractical in this scope: the
view and introspection layers are wired to SQLite specifics (`sqlite3.Row`,
`PRAGMA table_info`/`database_list`, `ATTACH`, file hash/size, the write-thread
model). The `PostgresDatabase` class here is the core storage adapter such a
fork would be built around — it preserves the exact safety contract, which is
the heart of the question. A real port would additionally swap introspection to
`information_schema`/`pg_catalog` and drop the file-immutability paths.

## Conclusion

PostgreSQL + psycopg can run untrusted queries as safely as Datasette runs them
on SQLite. The time limit (`statement_timeout`) is a clean, superior analogue.
The one mental-model shift: in SQLite "read-only" is a property of how the file
is opened; in PostgreSQL the durable read-only guarantee comes from **granting
only SELECT** to the query role, with the read-only transaction flag as a
secondary belt-and-braces layer.

## Files
- `notes.md` — running investigation log
- `pg_experiments.py` — protection experiments (time limit, read-only, limits)
- `pg_datasette_poc.py` — Datasette-style `Database` class on PostgreSQL
- `experiments_output.txt`, `poc_output.txt` — captured runs

## Reproducing
```bash
# PostgreSQL 16 as an unprivileged user (initdb refuses root):
initdb -D /tmp/pgdata -U postgres --auth=trust
pg_ctl -D /tmp/pgdata -o '-k /tmp/pgrun -p 5432' start
# create testdb + least-privilege 'untrusted' role (see notes.md), then:
python -m venv venv && venv/bin/pip install 'psycopg[binary]'
venv/bin/python pg_experiments.py
venv/bin/python pg_datasette_poc.py
```
