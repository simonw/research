# Can DuckDB run untrusted SQL as safely as Datasette runs SQLite?

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

Datasette can expose a SQL query box to the public internet because it leans
on two SQLite features to run untrusted `SELECT`s safely: an engine-level
**read-only** connection and an opcode-granularity **time limit**. This
investigation documents exactly how Datasette does that, then determines
experimentally whether the latest `duckdb` Python package (1.5.3) can be
constrained the same way — and finishes by getting a branch of Datasette
actually serving a DuckDB database.

**Short answer:** Yes, DuckDB can match (and on memory, exceed) SQLite's
safety guarantees — but `read_only=True` *alone is dangerously incomplete*.
You also need `enable_external_access=false` plus `lock_configuration=true`,
and the query time limit has to be built from a watchdog thread calling
`connection.interrupt()` because DuckDB has no progress-handler timeout.

## 1. How Datasette sandboxes SQLite

### Read-only at the engine level
`Database.connect()` (`datasette/database.py`) opens every read connection
through a SQLite URI filename:

- mutable files: `file:{path}?mode=ro`
- immutable files (`-i`): `file:{path}?immutable=1`
- shared in-memory dbs: opened then `PRAGMA query_only=1`

Writes go to a *separate* connection on a single dedicated write thread.
Because read-only is enforced by the SQLite engine, even if the SQL
validation layer were bypassed an `INSERT` raises "attempt to write a
readonly database". SQLite has no built-in way to reach the filesystem or
network from SQL, so `mode=ro` is a complete sandbox.

### Time limit via the progress handler
`sqlite_timelimit()` (`datasette/utils/__init__.py`) installs a progress
handler that SQLite calls every ~1000 virtual-machine opcodes; once the
deadline passes the handler returns non-zero and SQLite aborts the statement
with `OperationalError: interrupted`, which Datasette re-raises as
`QueryInterrupted`. Default limit: `sql_time_limit_ms = 1000`. Because the
check is per-opcode, even a single pathological query (cartesian join,
recursive CTE) is stopped without any cooperation.

### Defense in depth
A regex allowlist (`validate_sql_select`) requires statements to start with
`select`/`with`/`explain`, PRAGMAs are restricted to an allowlist, and
`max_returned_rows` (default 1000) truncates large result sets via
`fetchmany(N+1)`.

## 2. Constraining DuckDB — experimental findings (duckdb 1.5.3)

### `read_only=True` is NOT enough (`exp1_readonly.py`)
A read-only DuckDB connection blocks writes *to the database*, but still lets
untrusted SQL escape to the host:

| Operation | `read_only=True` |
|---|---|
| `INSERT` / `CREATE TABLE` | blocked |
| `COPY t TO '/path'` | **allowed — writes to the filesystem** |
| `read_csv('/etc/passwd')` | **allowed — reads arbitrary files** |
| `INSTALL httpfs` / network | allowed |
| `SET memory_limit=…` | allowed |

Unlike SQLite, DuckDB's SQL dialect can touch the filesystem and network, so
read-only protects only the database file.

### Read-only + lockdown config = safe (`exp2_lockdown.py`)
Adding these settings closes every hole:

```python
config = {
    "enable_external_access": "false",      # blocks COPY, read_csv(file), ATTACH paths, http
    "allow_community_extensions": "false",
    "autoinstall_known_extensions": "false",
    "autoload_known_extensions": "false",
    "allow_unsigned_extensions": "false",
    "lock_configuration": "true",           # applied last; blocks SET undoing the above
}
duckdb.connect(path, read_only=True, config=config)
```

With this, `COPY TO`, `read_csv` of local files, `INSTALL`/`LOAD`, http
reads, and `ATTACH` of arbitrary paths are all blocked — and crucially
`SET enable_external_access=true` is rejected because the configuration is
locked. `lock_configuration=true` is what makes the lockdown tamper-proof.

### Time limits via `interrupt()` (`exp3_timeout.py`)
DuckDB has **no** progress-handler or `statement_timeout`. The working
analogue is a watchdog thread that calls `connection.interrupt()`:

- DuckDB releases the GIL during `execute()`/`fetchall()`, so the watchdog
  thread runs while the main thread is blocked.
- A 1.0s watchdog interrupted a heavy cross-join at 1.001s, raising
  `duckdb.InterruptException`. `safe_duckdb.py` wraps this in a
  `duckdb_timelimit(con, ms)` context manager mirroring `sqlite_timelimit`.

### Resource limits
`memory_limit` is a hard cap: with `max_temp_directory_size='0B'` (to forbid
disk spill), an oversized aggregate raised `OutOfMemoryException` instead of
OOM-killing the process. `threads` caps CPU parallelism. SQLite has no
equivalent memory cap, so DuckDB is actually *stronger* here.

### Parity summary

| Protection | SQLite (Datasette) | DuckDB equivalent |
|---|---|---|
| Read-only database | `mode=ro` / `immutable=1` | `read_only=True` |
| No filesystem/network escape | (none needed) | `enable_external_access=false` + extension knobs |
| Lockdown can't be undone | n/a | `lock_configuration=true` |
| Query time limit | progress handler returns 1 | watchdog thread → `con.interrupt()` |
| Row cap | `fetchmany(N+1)` | `fetchmany(N+1)` (identical) |
| Memory cap | none | `memory_limit` + `max_temp_directory_size=0B` |

## 3. Bonus: Datasette running on DuckDB

`datasette_duckdb.py` is an adapter that makes Datasette serve a `.duckdb`
file, preserving both safety properties. It needs **no fork** — it
monkeypatches `sqlite_timelimit` to dispatch on connection type and adds a
`DuckDBDatabase(Database)` whose `connect()` returns a hardened, read-only
DuckDB connection wrapped in a small `sqlite3`-compatible shim. DuckDB
already understands `sqlite_master` and `PRAGMA table_info`, so most of
Datasette's introspection runs unchanged; the adapter translates the
remaining SQLite-isms (`[ident]` quoting → `"ident"`, `:name` → `$name`,
`table_xinfo` → `table_info`, unsupported PRAGMAs, and DuckDB exceptions →
`sqlite3.OperationalError`).

`serve_demo.py` drives the real Datasette ASGI app over httpx against a
DuckDB database (full output in `demo_output.txt`):

1. `/demo.json` lists the tables — introspection works
2. `/demo/planets.json` browses rows
3. an ad-hoc `JOIN` query returns correct results
4. an aggregate query works
5. `INSERT` → blocked
6. `COPY … TO` → blocked, no file written
7. `read_csv('/etc/hostname')` (a valid `SELECT`, so it passes Datasette's
   regex) → blocked by the DuckDB engine: *"file system operations are
   disabled"* — the proof that engine-level lockdown, not just the regex,
   guards the filesystem
8. a heavy cross-join → interrupted at 0.51s (the configured 500ms limit)

Remaining gaps before this is production-ready: foreign-key/index
introspection, full-text search, custom SQL functions, and mapping DuckDB's
richer type system in the JSON renderer.

## Files

| File | What it is |
|---|---|
| `notes.md` | Running investigation log |
| `exp1_readonly.py` | Shows `read_only=True` alone leaks the filesystem/network |
| `exp2_lockdown.py` | Shows the hardened config closes every hole |
| `exp3_timeout.py` | Watchdog-`interrupt()` time limit + ATTACH lockdown |
| `safe_duckdb.py` | Reusable hardened-connection + `duckdb_timelimit` helper |
| `datasette_duckdb.py` | The Datasette ↔ DuckDB adapter |
| `serve_demo.py` | End-to-end demo through Datasette's ASGI app |
| `demo_output.txt` | Captured output of every script above |

## Reproducing

```bash
pip install duckdb datasette httpx
python3 exp1_readonly.py      # read_only leaks
python3 exp2_lockdown.py      # hardened config is safe
python3 exp3_timeout.py       # time limit works
python3 safe_duckdb.py        # helper self-test
python3 serve_demo.py         # Datasette serving DuckDB
```
