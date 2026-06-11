# Notes: Datasette SQLite safety model vs DuckDB

## Goal

1. Understand how Datasette safely runs untrusted SQL against SQLite
   (read-only mode + time limits).
2. Experiment with the latest `duckdb` Python package to see whether the
   same protections (strict read-only + query time limits) can be achieved.
3. Bonus: get a branch of Datasette working against DuckDB.

## Step 1: Datasette's SQLite usage (cloned simonw/datasette @ 6eaa9e3 to /tmp/datasette)

### Read-only enforcement

`datasette/database.py` `Database.connect()` (~line 137):

- File databases open with SQLite URI filenames:
  - mutable databases: `file:{path}?mode=ro` (plus `&nolock=1` if `--nolock`)
  - immutable databases (`-i`): `file:{path}?immutable=1` — SQLite assumes the
    file cannot change, skips locking entirely
  - opened with `sqlite3.connect(uri, uri=True, check_same_thread=False)`
- Shared in-memory named databases: `file:{name}?mode=memory&cache=shared`,
  then `PRAGMA query_only=1` on read connections (mode=ro doesn't work for
  shared-cache memory dbs).
- Writes go through a *separate* dedicated write connection on its own
  single write thread (`isolation_level="IMMEDIATE"`); read connections are
  per-thread in a thread pool executor.

So read-only is enforced at the *connection* level by the SQLite engine
itself — even if the SQL validation layer is bypassed, an INSERT raises
"attempt to write a readonly database".

### Time limit enforcement

`datasette/utils/__init__.py` `sqlite_timelimit()` (~line 245):

```python
@contextmanager
def sqlite_timelimit(conn, ms):
    deadline = time.perf_counter() + (ms / 1000)
    n = 1000  # VM opcodes between checks (~0.08ms per 1000 ops)
    if ms <= 20:
        n = 1
    def handler():
        if time.perf_counter() >= deadline:
            return 1  # non-zero return interrupts the query
    conn.set_progress_handler(handler, n)
    try:
        yield
    finally:
        conn.set_progress_handler(None, n)
```

- Uses `sqlite3_progress_handler`: callback every N virtual machine opcodes,
  in-band on the same thread. Returning non-zero aborts the statement with
  `sqlite3.OperationalError: interrupted`, caught in `Database.execute()`
  and re-raised as `QueryInterrupted`.
- Default limit: `sql_time_limit_ms` setting = 1000ms. Facets use shorter
  custom limits (200ms / 50ms) via `custom_time_limit`.
- Because the check is per-opcode, even a single monster query (cartesian
  joins, recursive CTEs) is interrupted — no cooperation from the query
  needed, no watchdog thread needed.

### Other defenses (defense in depth)

- `validate_sql_select()` (utils/__init__.py ~321): regex allowlist — SQL must
  start with `select`/`with`/`explain select`/`explain query plan` (comments
  allowed first); PRAGMA only via an allowlist of `pragma_*()` table-valued
  functions.
- `max_returned_rows` (default 1000): `cursor.fetchmany(max_returned_rows+1)`
  so unbounded result sets can't exhaust memory.
- `set_authorizer` used in `utils/sql_analysis.py` for analyzing write
  queries (not on the main read path).
- Extensions only loaded if explicitly configured (`--load-extension`).

Key insight: SQLite's safety story for Datasette = engine-level read-only
(URI mode=ro/immutable) + opcode-granularity interruption (progress handler)
+ regex allowlist + row truncation.

## Step 2: DuckDB questions to answer experimentally

Installed duckdb 1.5.3 (latest on PyPI as of 2026-06-10).

- Does `duckdb.connect(path, read_only=True)` block all writes? What about:
  - `COPY ... TO 'file'` (writes to filesystem, not the database!)
  - `ATTACH` another writable database
  - `INSTALL`/`LOAD` extensions
  - `read_csv()` / `read_parquet()` reading arbitrary local files
  - `SET` commands changing configuration
- Is there a built-in statement timeout? (Believed: no — need
  `conn.interrupt()` from a watchdog thread. Does Python release the GIL
  during execute so a watchdog can run?)
- Resource limits: `memory_limit`, `threads`, `temp_directory`,
  `max_temp_directory_size`.
- Lockdown settings: `enable_external_access=false`,
  `disabled_filesystems='LocalFileSystem'`, `lock_configuration=true`
  (the "Securing DuckDB" recipe) — verify they work and can't be undone
  by untrusted SQL.

(experiments below)

## Step 3: Experimental results (duckdb 1.5.3)

### exp1_readonly.py — `read_only=True` ALONE is NOT enough

| Operation | read_only=True |
|---|---|
| SELECT | allowed |
| INSERT / CREATE TABLE | **blocked** ("attached in read-only mode") |
| CREATE TEMP TABLE | allowed (temp only) |
| `COPY t TO 'file'` | **ALLOWED — writes to filesystem!** |
| `read_csv('/secret')` | **ALLOWED — reads arbitrary local files!** |
| `INSTALL httpfs` | allowed |
| `SET memory_limit=...` | allowed |

So unlike SQLite `mode=ro` (which fully sandboxes), DuckDB `read_only`
only protects the *database file*. The filesystem and network are still
reachable through COPY/read_csv/read_parquet/INSTALL. Untrusted SQL could
exfiltrate files or fill the disk.

### exp2_lockdown.py — read_only + hardened config = safe

Adding `enable_external_access=false` (plus the extension knobs and
`lock_configuration=true`) blocks every escape:

| Operation | hardened |
|---|---|
| SELECT | allowed |
| INSERT | blocked (read-only) |
| COPY TO file | **blocked** (file system operations are disabled) |
| read_csv arbitrary file | **blocked** |
| INSTALL / LOAD httpfs | **blocked** |
| read_parquet over http | **blocked** |
| `SET enable_external_access=true` (undo) | **blocked** (configuration locked) |
| ATTACH arbitrary path | **blocked** (external access) |

`lock_configuration=true` is essential — without it untrusted SQL could
`SET enable_external_access=true` and re-open the filesystem.

### exp3_timeout.py — time limit via watchdog + interrupt()

DuckDB has **no** progress-handler / statement_timeout equivalent. But:

- `connection.interrupt()` aborts the running query, raising
  `duckdb.InterruptException`.
- DuckDB **releases the GIL** during `execute()/fetchall()`, so a watchdog
  thread can call `interrupt()` while the main thread is blocked.
- A 1.0s watchdog interrupted a heavy cross-join at 1.001s — clean and
  prompt. This is the DuckDB analogue of `sqlite_timelimit`.

### safe_duckdb.py — reusable helper

`duckdb_timelimit(con, ms)` context manager mirrors Datasette's
`sqlite_timelimit`. `connect_readonly()` applies the hardened config.
A 250ms limit interrupts the heavy query at 0.251s; row cap via
`fetchmany(max_rows+1)` works just like Datasette.

### Memory / resource limits

- `memory_limit` is a **hard cap**. With `max_temp_directory_size='0B'`
  (to forbid disk spill), an oversized single-group `list()` aggregate over
  50M rows raised `OutOfMemoryException` instead of OOM-killing the process.
- Streamable queries (count/group-by) run fine under a small limit because
  DuckDB streams them — the cap only bites genuinely memory-hungry plans.
- `threads` caps CPU parallelism (set to 1 for predictable single-query cost).

## Conclusion on parity

| Protection | SQLite (Datasette) | DuckDB equivalent |
|---|---|---|
| Read-only DB | `file:..?mode=ro` / `immutable=1` | `read_only=True` |
| No FS/network escape | (SQLite has none by default) | `enable_external_access=false` + extension knobs |
| Can't undo lockdown | n/a | `lock_configuration=true` |
| Query time limit | progress handler returns 1 | watchdog thread → `con.interrupt()` |
| Row cap | `fetchmany(N+1)` | `fetchmany(N+1)` (identical) |
| Memory cap | (none built in) | `memory_limit` + `max_temp_directory_size=0B` |

DuckDB can match — and on memory actually exceed — Datasette's SQLite
safety, but it requires an explicit hardened config; `read_only=True` by
itself is dangerously incomplete.

## Step 4: Bonus — Datasette running on DuckDB (datasette_duckdb.py + serve_demo.py)

Built an adapter (no fork of Datasette needed — pure runtime adaptation; the
cloned repo is left unmodified) that lets Datasette serve a `.duckdb` file:

- `DuckDBConnection` / `_Cursor`: a duck-typed stand-in for the subset of
  `sqlite3.Connection`/`Cursor` Datasette uses, backed by a hardened
  read-only DuckDB connection.
- `Row(tuple)`: tuple that also supports `row["col"]` and `dict(row)`, so
  Datasette's `sqlite3.Row` assumptions keep working.
- `duckdb_timelimit`: watchdog-thread + `interrupt()` replacement for
  `sqlite_timelimit`, wired in by `install()` which monkeypatches
  `datasette.database.sqlite_timelimit` to dispatch on connection type.
- `DuckDBDatabase(Database)`: `connect()` returns a hardened DuckDB
  connection (read_only + the lockdown config + memory/threads/temp caps).

Dialect translations the adapter performs:
- `[ident]` (SQLite bracket quoting) -> `"ident"` (DuckDB), but `'ident'`
  inside PRAGMA args.
- `:name` params -> `$name`, and unused named params are dropped (SQLite
  ignores them, DuckDB errors on extras).
- `PRAGMA table_xinfo` -> `table_info` (+ synthesised `hidden=0` column).
- `PRAGMA schema_version` -> `SELECT 0`; `foreign_key_list`/`index_list`
  -> empty results; `cache_size` etc. ignored.
- DuckDB exceptions re-raised as `sqlite3.OperationalError` (interrupts as
  `"interrupted"`) so Datasette's existing error handling produces clean
  400s and `QueryInterrupted`.
- SQLite-only introspection (`hidden_table_names`, `get_all_foreign_keys`,
  `fts_table`) overridden on the Database subclass.

`serve_demo.py` runs the real Datasette ASGI app over httpx against a DuckDB
file. Results (see demo_output.txt):

1. `/demo.json` lists tables `moons`, `planets` (introspection works)
2. `/demo/planets.json` browses rows
3. ad-hoc JOIN query returns correct rows
4. aggregate query works
5. INSERT -> blocked (Datasette regex: "must be a SELECT")
6. COPY TO file -> blocked + no file created
7. `read_csv('/etc/hostname')` (a valid SELECT, so it passes the regex) ->
   blocked by DuckDB engine: "file system operations are disabled" — this is
   the key proof that the engine-level lockdown, not just the regex, protects
   the filesystem
8. heavy cross-join -> interrupted at 0.51s (the configured 500ms limit)

So a Datasette-on-DuckDB branch is feasible and the safety model transfers,
provided the hardened config is applied. Remaining gaps for production:
foreign-key/index introspection, FTS, custom SQL functions, and DuckDB's
richer type system in the JSON renderer.

### Files in this folder
- notes.md / README.md — this writeup
- exp1_readonly.py, exp2_lockdown.py, exp3_timeout.py — safety experiments
- safe_duckdb.py — reusable hardened-connection + time-limit helper
- datasette_duckdb.py — the Datasette<->DuckDB adapter
- serve_demo.py — end-to-end demo through Datasette's ASGI app
- demo_output.txt — captured output of all of the above
