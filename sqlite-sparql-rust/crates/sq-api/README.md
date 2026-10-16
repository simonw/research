# sq-api

`sq-api` is the SQL surface of sparqlite. Its one entry point, `register(&Connection)`, installs on
a SQLite connection everything a SQL user sees: the scalar and aggregate functions
(`sparql_query`, `sparql_load`, `sparql_update`, `sparql_term`, the baseline helpers, ...), the
virtual tables `sparql_rows`, `sparql_view` and `sparql_status`, and one per-connection state that
owns the storage layer, the configuration and the parsed-query cache. The crate does not
evaluate SPARQL itself. It routes each call to the native engine (`sq-engine` over `sq-sqlite`) or
to the baseline SPARQL-to-SQL translator (`sq-sqlgen`), and it enforces the rules that make that
safe inside a host process: one snapshot per call, savepoint scopes, no `RefCell` borrow across
a re-entrant call, panic containment at every FFI boundary, a recursion budget on request text,
and a close hook so connections close fully. The same code runs in the loadable extension
(`ext/`) and in native tests over the system libsqlite3.

## Where it fits

```
 sq-core  sq-plan  sq-exec  sq-expr  sq-results  sq-sqlgen  sq-sqlite  sq-engine
    \________\_______\________\__________\___________\__________\__________/
                                   |
                                 sq-api      rusqlite 0.40.2 ("functions", "vtab"; system libsqlite3),
                                   |         spargebra, oxrdf, oxrdfio, oxsdatatypes, serde_json,
                                   |         rustc-hash, rand  (dev: sq-core with feature `testing`)
          +------------------------+
   ext/ (cdylib libsparqlite,   tools/sq-w3c (feature `sqlite`,
   own workspace, rusqlite      default on: `--backend sqlite`)
   "loadable_extension")
```

What it takes from each sibling: `sq_engine::{prepare, explain, update_with, UpdateOptions}`;
`sq_plan::{parse_query, parse_update, lower_query, optimize, ExplainMode}`; `sq_exec::Execution`
(the `sparql_view` pushdown path); `sq_sqlite::{SqliteStore, StorageShared, RawDb, ReaderMode,
LoadOptions}` and its read/write scopes; `sq_sqlgen::{translate, eval_template, HELPER_FUNCTIONS}`;
`sq_results` (serializers, `sql_native`, `append_solution_json`); `sq_expr` (evaluation inside the
helper functions); `sq_core` (ids, terms, errors).

## Key concepts

**1. One `ConnState` per connection** (`src/state.rs`). `register()` creates an `Arc<ConnState>`
holding the `RawDb`, an `Rc<SqliteStore>`, `RefCell<ConnConfig>`, the parsed-query LRU, the
`open_cursors` / `depth` counters, the last write report and API counters. Every registered
closure and module holds a clone. It is single-threaded (`Rc`, `Cell`, `RefCell`) with
`unsafe impl Send + Sync`, justified because SQLite serializes all callbacks of one connection;
debug builds assert at each entry that no second thread entered (rule R7). Borrows are O(1) and
taken with `try_borrow*`; a violation is `Error::Reentrant`, never a deadlock (rule R1).

**2. The entry protocol.** Every function that reads or writes the store, and every
`sparql_rows` / `sparql_view` `xFilter`, goes through one of the entries below (`sparql_version`,
`sparql_config` and `sparql_term_kind` need no entry; helpers running inside a baseline statement
reuse that statement's entry; `sparql_status`'s `xFilter` only copies counters):
- `read_call(f)`: `enter()` (the R8 nesting bound, re-arm the pool, record depth),
  `store.read_scope(depth)`, `validate()`
  (the §4.4 epoch check; an absent schema reads as an empty store, an incompatible one fails),
  `f`, then `scope.close()` with its error reported.
- `write_call(auto_init, f)`: refuses with `Error::WriteWhileReading` while a scan is open,
  runs `ensure_schema()` outside any scope when asked (loads and updates auto-initialize; analyze
  and vacuum do not), opens the `WriteScope` *before* any read inside it (the schema-state check
  and auto-init run before it opens, outside any scope), so `busy_timeout` applies to
  taking the write lock; then validates, runs `f`, commits.
- `writer_call(f)`: for `sparql_init`, whose storage call manages its own scopes.

`xNext` and `xColumn` read the store without an entry (the execution's next batch, decodes, the
dictionary probe behind `id*` / `__id`): the snapshot comes from the host statement's read
transaction, and nothing re-arms the pool or checks the epoch between rows.

Inside a statement that writes (`INSERT INTO t SELECT sparql_update(...)`) SQLite refuses every
`SAVEPOINT`. `write_call` then runs without a scope only if `host_statement_writes()` proves that
the host is the only running statement and it writes (autocommit, `sqlite3_txn_state == WRITE`,
walking `sqlite3_next_stmt`): the host's transaction makes the request atomic. Otherwise the
call is refused with an `Error::Invalid` that names the restriction (raised by
`host_statement_writes` itself, or by `savepoint_refusal` rewording SQLite's misleading
`SQLITE_BUSY` "statements in progress"). While a table stored in the database names a writer,
`direct_guard` closes this path too (see [Security](#security-function-flags-and-untrusted-schemas)).

**3. Close hook** (`src/vtab/status.rs`, §8.7). `register()` prepares
`SELECT 1 FROM main.sparql_status` once. Connecting that eponymous table arms the `sq-sqlite`
statement pool; its `Drop` (xDisconnect, which `sqlite3_close` runs before its busy check)
disarms the pool and finalizes idle statements, so the connection closes fully and the `-wal`
file disappears. Each entry re-arms if the instance was disconnected.

**4. Two engines behind `Run`** (`src/query.rs`). `Run::Native(Box<Execution>)` comes from
`sq_engine::prepare` (or, on the `sparql_view` pushdown path, from `sq_plan::optimize` +
`Execution::new`); `Run::Baseline(Box<BaselineRun>)` steps `sq_sqlgen::translate`'s SQL through
`SqliteStore::sql_rows` on the same connection. That SQL calls this crate's helper functions
(`src/helpers.rs`) while this crate steps it, so the run pushes its `HelperCtx` (TempDict,
value cache) onto `Baselines` around each step: TEMP ids flow between SQL and the helpers as
they do inside the native executor. Both runs feed the same output path (`SolutionSource` /
`TripleSource`, `sq-results` writers, the §8.4 native mapping in `src/value.rs`). For the baseline,
`timeout_ms` is checked between 1024-row batches and enforced inside a step by a `Watchdog`
thread (`src/watchdog.rs`) that calls `sqlite3_interrupt`; no progress handler is installed.

**5. Streaming scans with late materialization** (`src/vtab/mod.rs`). A `Stream` pulls
`BATCH_ROWS` (1024) ids per batch and decodes, in one batched call, only the columns that
SQLite's `colUsed` mask says the statement reads (passed from `xBestIndex` to `xFilter` as hex in
`idx_str`). `count(*)` decodes nothing, and `id*` columns decode nothing for stored ids; an
unannounced column is decoded on demand. `id*` / `__id` hold the term's id in the store: a
computed (TEMP) term that exists in the dictionary reports its stored id, otherwise NULL (each
TEMP cell read costs one decode plus one dictionary probe).

**6. View pushdown** (`src/vtab/view.rs`). `xBestIndex` consumes `=` constraints on `__id`
columns, on `__term` columns under BINARY collation only, and on the hidden `opts` column.
`xFilter` resolves each value to an id and wraps the lowered query's root in a top-level
`FILTER(sameTerm(?v, c) && ...)` before planning (`sq_plan::optimize` for the native engine, the
`sq-sqlgen` translator for `engine=sql`), so the result equals the SQL filter
applied to the view's rows (after LIMIT, unbound never matches). A value that can never equal
the column (wrong type, non-canonical N-Triples spelling, an id naming no term) gives an empty
scan; a well-formed term absent from the store becomes a TEMP constant.

**7. Nesting budget** (`src/nesting.rs`). A stack overflow is not a panic and kills the host. Every
request text is scanned before any parse and refused (`Error::Unsupported`, "nests too deeply")
when its estimated recursion cost exceeds `LIMIT = 1000` (weights are roughly KiB of stack per
level, so the worst case stays near 1 MiB). Flat data (`INSERT DATA`, `VALUES`, templates) costs
nothing. Calls can also nest: sparqlite's own SQL reads `main`'s `rdf_*` tables, and a database
file may make one of them a view, or give it a trigger, that calls sparqlite again (a file whose
`rdf_meta` was such a view used to recurse until the host's stack overflowed). `enter()` refuses
a nested entry once `MAX_ENTRY_DEPTH` (8) entries are active or more than `NESTED_STACK_BUDGET`
(384 KiB) of stack lies between it and the outermost entry (rule R8): `sparql: unsupported:
sparqlite calls nest too deeply ...`. Legitimate nesting is one level deep (a reader in a trigger
on `rdf_quad`, fired by a writer's INSERT) and measures about 5 KiB.

**8. Panic containment and errors** (`src/guard.rs`, `src/error.rs`). rusqlite catches panics in
scalar and aggregate functions but not in virtual-table trampolines, so every vtab method body
runs in `guard()` (`guard_eof` for `xEof`, `guard_drop` for destructors). `sql_error` maps
`sq_core::Error` to a SQL error whose message starts with `sparql: `; vtab methods also return
`SQLITE_INTERRUPT` or the original SQLite code. `xFilter` and `xNext` pass both through
`zErrMsg`; `xColumn` goes through `vtab::column_error`, which sets the message as the column's
result text and hands rusqlite the bare code (rusqlite's own column error path ends in
`sqlite3_result_error`, which resets the code to `SQLITE_ERROR`). A failure of a nested call
reaches the user with its own message: `sql_error` drops the `sparql: SQLite error 1: ` that each
enclosing call would add.

**9. Function flags** (`src/functions/mod.rs`). Writers and file access are `SQLITE_DIRECTONLY`
and also pass `ConnState::direct_guard` before they run, because SQLite does not apply
DIRECTONLY to CHECK constraints; every other function is `SQLITE_INNOCUOUS`, so views and
triggers stored in a database can call it even with `PRAGMA trusted_schema = OFF`. The reasoning
is in [Security: function flags](#security-function-flags-and-untrusted-schemas).

## Public API tour

The Rust API is two functions; everything else is `pub(crate)`.

```rust
pub fn register(conn: &rusqlite::Connection) -> rusqlite::Result<()>;
pub fn version_string(sqlite_version: &str) -> String; // "sparqlite <ver> (SQLite <ver>)"
```

Typical use, as in `tests/common/mod.rs` (calling `register` again replaces the state cleanly):

```rust
let c = rusqlite::Connection::open_in_memory()?;
sq_api::register(&c)?;
let n: i64 = c.query_row("SELECT sparql_load(?1)", [DATA], |r| r.get(0))?; // new quads
```

SQL functions (each arity registered separately; none is DETERMINISTIC). Flags: D =
`SQLITE_DIRECTONLY`: SQLite refuses the function in views, triggers and DEFAULT clauses stored in
`main` or an attached database, whatever `trusted_schema` says (generated columns and index
expressions take no non-deterministic function at all); TEMP views and TEMP triggers are exempt,
because only the connection itself can create them. SQLite does not refuse it in a stored CHECK
constraint, so sparqlite does (`direct_guard`). I = `SQLITE_INNOCUOUS`: stored views and triggers
may call it even with `PRAGMA trusted_schema = OFF` (the sqlite3 shell's default); see
[Security](#security-function-flags-and-untrusted-schemas).

| Function | Arities | Flags | Returns |
|---|---|---|---|
| `sparql_version` | 0 | I | TEXT |
| `sparql_init` | 0 | D | schema version (idempotent) |
| `sparql_load(data [, format [, base [, graph]]])` | 1-4 | D | new quads; data TEXT or BLOB; format (case-insensitive, default `turtle`): `turtle`/`ttl`, `ntriples`/`nt`, `nquads`/`nq`, `trig`, `rdfxml`/`xml`/`rdf`/`owl`, `n3`, `jsonld`/`json` |
| `sparql_load_file(path [, format [, base [, graph]]])` | 1-4 | D | new quads; needs `allow_files`; format guessed from the file extension (same names), base = `file://` URL |
| `sparql_update(update)` | 1 | D | inserted + deleted; `LOAD <file:...>` follows `allow_files` |
| `sparql_analyze()` / `sparql_vacuum_terms()` | 0 | D | JSON summary / rows deleted |
| `sparql_check()` | 0 | I | `ok: ...` or `errors: ...` |
| `sparql_config(key)` / `sparql_config(key, value)` | 1 / 2 | I / D | current / new value as TEXT |
| `sparql_query(q [, format])` | 1-2 | I | json (default)/xml/csv/tsv; graphs: ntriples (default)/nquads/turtle |
| `sparql_ask`, `sparql_count`, `sparql_value` | 1 | I | 0/1; count with no decode (always evaluates the query: an aggregate query counts 1 after its aggregation ran); native value of the first row's first variable (ASK: 0/1; CONSTRUCT/DESCRIBE: the first triple's subject) |
| `sparql_explain(q [, mode])` | 1-2 | I | `algebra`, `logical`, `plan` (default), `analyze`, `sql`, `sqlplan` |
| `sparql_query_sql`, `sparql_count_sql`, `sparql_to_sql` | 1-2, 1, 1 | I | baseline engine regardless of `engine` |
| `sparql_term`, `sparql_term_value`, `sparql_term_id`, `sparql_term_kind/_lex/_lang/_datatype` | 1 | I | term accessors; `sparql_term_id` takes N-Triples (`<iri>`, `"lex"@lang`, `"lex"^^<dt>`, bare numbers, `true`/`false`) and says so when given a prefixed name or a bare IRI |
| `sparql_num`, `sparql_str`, `sparql_ebv`, `sparql_sortkey`, `sparql_regex` (2-3), `sparql_eval` (variadic) | | I | baseline helpers |
| aggregates `sparql_sum/_avg/_min/_max/_sample`, `sparql_group_concat` (1-2) | | I | baseline helpers |

Usage taken from the tests:

```sql
SELECT sparql_load(?1, 'ttl', 'http://base/', 'http://ex/g');            -- into a named graph
SELECT sparql_query('SELECT ?n WHERE { ?s <http://ex/name> ?n } ORDER BY ?n', 'csv');
SELECT n, v0, v1, t0, id0, vars, json
  FROM sparql_rows('SELECT ?s ?a WHERE { ?s <http://ex/age> ?a } ORDER BY ?a');
SELECT count(*) FROM sparql_rows(?1, '{"engine":"sql","union_default_graph":1}');
SELECT j.id, r.n, r.v0 FROM jobs j, sparql_rows(j.q) r ORDER BY j.id, r.v0;  -- correlated
CREATE VIRTUAL TABLE temp.people USING sparql_view(
  'SELECT ?person ?name ?age WHERE { ?person <http://ex/name> ?name
     OPTIONAL { ?person <http://ex/age> ?age } }', engine=native, timeout_ms=2000);
SELECT name, age FROM people WHERE person__term = '<http://ex/alice>';      -- pushed down
SELECT value FROM sparql_status WHERE key = 'terms_decoded';
```

`sparql_rows(query [, opts])` columns: `n` (1-based ordinal), `v0..v15` (native values),
`t0..t15` (N-Triples), `id0..id15`, `vars` (JSON array), `json` (SPARQL-JSON bindings of the row),
hidden `query`, `opts`. ASK gives one row (`v0` = 0/1, `json` = `{"boolean":...}`); CONSTRUCT /
DESCRIBE give one row per triple with `vars` = `["subject","predicate","object"]`; with more than
16 variables read `json`. A NULL query gives no rows (like `json_each(NULL)`); without a query
constraint `xBestIndex` returns `false` and SQLite reports "no query solution". `opts` is a JSON
object of per-call keys; `timeout_ms` and `max_memory_mb` there (and in `sparql_view` arguments)
can only tighten the connection's limits: the stricter value wins, and `timeout_ms = 0` does not
remove a connection timeout. `sparql_view` declares the projected variables as value columns, then
hidden `<v>__term`, `<v>__id` and `opts`; it accepts per-call `key=value` arguments (the same keys
as `sparql_rows` opts; a connection setting such as `allow_files` is refused) after the query and
fails at `CREATE` on non-SELECT queries and column-name clashes. Without `temp.` the definition
persists in the schema; a connection without the extension fails only on that table.

`sparql_config` keys and defaults: `engine=native`, `timeout_ms=0`, `max_memory_mb=1024`,
`union_default_graph=0`, `decode_cache_mb=64`, `join_policy=auto` (`hash`/`bind`/`merge`/`nosql`),
`reorder=1`, `probe_counts=32`, `probe_cap=1000`, `allow_files=0`, `reader=auto` (`sql`/`btree`)
and `page_cache_mb=128`; the last two are owned by `sq-sqlite` and apply at once. Only `engine`,
`timeout_ms`, `max_memory_mb`, `union_default_graph`, `join_policy`, `reorder`, `probe_counts`
and `probe_cap` can be set per call (`Key::per_call`). `sparql_status` adds to the `sq-sqlite`
keys (`schema_state`, `dict_gen`, `decode_*`, `pool_*`, `armed`, `btree_*`, ...): `version`,
`sqlite_version`, `open_cursors`, `parsed_cache_entries/_hits/_misses`, `cursors_opened`,
`rows_emitted`, `terms_decoded`, `last_op/_inserted/_deleted/_quads_read/_terms_new/_bulk/_ms`
and `config.<key>` for the 12 documented keys; integer-looking values are returned as INTEGER.

## Security: function flags and untrusted schemas

A database file carries code: views, triggers, CHECK constraints, DEFAULT clauses, index
expressions and generated columns run with the connection of whoever opens the file. SQLite has
two guards against a hostile schema. `SQLITE_DIRECTONLY` functions are refused in views, triggers
and DEFAULT clauses stored in `main` or an attached database, whatever the settings; objects in
the TEMP schema are exempt, since only the connection itself can create them. (Generated columns,
index expressions and partial-index WHERE clauses accept only deterministic functions, and none
of sparqlite's is.) With `PRAGMA trusted_schema = OFF` (`SQLITE_DBCONFIG_TRUSTED_SCHEMA`; the
sqlite3 shell's default, and SQLite's advice for files of uncertain origin), stored schema
objects may call only functions marked `SQLITE_INNOCUOUS`, and use only virtual tables configured
as innocuous. SQLite's bar for innocuous: the function has no side effects, leaks no information
an attacker could not get otherwise, and cannot be misused to cause harm. Its own `changes()`,
`last_insert_rowid()`, `sqlite_version()` and `random()` pass; `load_extension()` does not.

sparqlite's functions fall into two groups (`src/functions/mod.rs`, tested in `tests/trust.rs`):

- **DIRECTONLY**: `sparql_init`, `sparql_load`, `sparql_load_file`, `sparql_update`,
  `sparql_analyze`, `sparql_vacuum_terms` and the two-argument `sparql_config`. They change the
  store, its statistics or the connection's settings (`allow_files`, `timeout_ms`, ...), or read
  files (`sparql_load_file`, and `LOAD <file:...>` in an update once `allow_files` is on). A
  schema object written by someone else must never do that, so they run only from top-level SQL
  and from TEMP views and triggers.
- **INNOCUOUS**: every other function: `sparql_version`, `sparql_query`, `sparql_ask`,
  `sparql_count`, `sparql_value`, `sparql_explain` (every mode), `sparql_query_sql`,
  `sparql_count_sql`, `sparql_to_sql`, `sparql_check`, the one-argument `sparql_config`,
  `sparql_term` and the `sparql_term_*` accessors, and the baseline helpers (`sparql_num`,
  `sparql_str`, `sparql_ebv`, `sparql_sortkey`, `sparql_regex`, `sparql_eval` and the six
  aggregates). The virtual tables `sparql_rows` and `sparql_status` were already innocuous.

**CHECK constraints: where SQLite's guard does not reach.** SQLite 3.45.1 lets a CHECK constraint
stored in `main` or an attached database call a DIRECTONLY function (its own `readfile()` too),
also with `trusted_schema = OFF`. The CHECK runs inside every INSERT and UPDATE of its table,
including one that a stored trigger issues when the application writes elsewhere, and inside
`PRAGMA integrity_check` and `quick_check`. Before this was handled, a file whose table had
`CHECK (sparql_config('allow_files', 1) ... AND sparql_update('LOAD <file:...>') ...)` turned
`allow_files` on, removed the application's `timeout_ms` and copied a local RDF file into itself
the moment the application inserted a row, and an attached file wiped `main`'s store the same
way. A function cannot tell which schema object called it, so every DIRECTONLY function first
passes `ConnState::direct_guard`:

1. It never runs nested inside another sparqlite call (only schema code fired by sparqlite's own
   SQL, such as a trigger on `rdf_quad`, can call it there): `sparql_update cannot run inside
   another sparqlite call`.
2. It scans the definitions of the tables stored in `main` and every attached database (not
   TEMP) for the name of a DIRECTONLY function. SQLite resolves function names ignoring ASCII
   case only, and an identifier has no other spelling, so a CHECK that calls one contains its
   name. If a table names one, the call must show that it is direct: the connection runs
   exactly one statement, that statement is read-only (no CHECK runs in a read-only statement
   except an integrity check), and its own SQL text (`sqlite3_sql`) calls the function by name
   (an integrity check's text does not). The application's `SELECT sparql_update(?)` keeps
   working; a writer in `INSERT ... SELECT`, in a TEMP trigger or view, or beside another running
   statement is refused while such a table exists, with a message that names the table. Without
   such a table nothing changes.

Why the readers pass SQLite's test:

1. **No side effects.** A reader that touches the store does so as a read entry (`read_call`: a
   read scope that is released before the call returns, or nothing at all inside a running
   statement) and only through the store's read interfaces. `sparql_explain(q, 'analyze')` executes
   the query through `Rc<dyn QuadStore>`, which has no write method, and `'sqlplan'` only runs
   `EXPLAIN QUERY PLAN` on the generated SQL. `readers_never_write` runs every reader, and
   `sparql_rows` on both engines, under `PRAGMA query_only = 1`, where any change to a database
   file fails. What a reader does change is the connection's in-memory bookkeeping: the
   parsed-query cache, the decode and lookup caches and the `sparql_status` counters. Settings
   change only through the DIRECTONLY setter. No file, socket or process is opened (`SERVICE` is
   refused); the only thread is the `timeout_ms` watchdog of a baseline run, joined when the run
   ends.
2. **Nothing the schema's author cannot already read.** A reader sees (a) the RDF store in `main`,
   which consists of ordinary tables (`rdf_quad`, `rdf_term`, `rdf_meta`, ...) that plain SQL in a
   view stored in `main` reads just as well; (b) the connection's sparqlite settings, which are
   tuning knobs (no paths, no secrets) and were already visible through `sparql_status`;
   (c) timings and cache hit counts (`sparql_explain(q, 'analyze')`, `sparql_status`): activity
   information of the same kind as `changes()` and `last_insert_rowid()`.
3. **No unbounded resources beyond an ordinary query.** A SPARQL query can run for a long time,
   as a recursive CTE can. A stored view cannot change the limits the application set: the
   scalar functions take no options, and the per-call options of `sparql_rows` and `sparql_view`
   can only lower `timeout_ms` and `max_memory_mb`, never raise or remove them. So a call obeys
   the connection's `timeout_ms` and `sqlite3_interrupt`, its working memory (computed strings
   included) obeys `max_memory_mb`, request text is checked against the nesting budget before
   parsing, and a serialized result (`sparql_query`, `sparql_query_sql`) stops at the
   connection's `SQLITE_LIMIT_LENGTH` (`sparql: SQLite error 18: string or blob too big: ...`),
   as SQLite's `group_concat` does, instead of being built in full first. Recursion through the
   schema (a view in place of `rdf_meta` that calls a reader, so that every reader's epoch check
   calls it again) is cut off by the nesting bound R8 (key concept 7) after 8 levels or 384 KiB of
   stack, with an error; it used to overflow the host's stack.

What this does not cover:

- **Attached databases.** The readers always read `main`'s store. A view or trigger stored in an
  ATTACHed database can normally reference only its own database's tables ("cannot reference
  objects in database main"), but it can call the readers, and so read `main`'s RDF data. If an
  application attaches a database it does not trust to a connection whose `main` holds RDF data
  that the attached file's author must not see, and the attached file later goes back to that
  author, a trigger in it could copy query results into its own tables. `sparql_rows` and
  `sparql_status` already allowed this (they were innocuous before the functions were), so the
  flags add no new reach. Keep such files on a connection that has not loaded sparqlite.
- **Look-alike `rdf_*` objects.** `sq-sqlite` recognizes the store by the names of its tables, not
  their types, so a view named `rdf_meta` passes as the store's table. The nesting bound keeps
  that from crashing the host, and the readers then fail with an error, but the file is not
  refused up front.

## Loading the extension

`ext/` builds `libsparqlite.so`, whose entry point `sqlite3_sparqlite_init` calls
`sq_api::register`. SQLite derives the entry-point name from the file name (`libsparqlite` →
`sparqlite`), so a renamed file needs the entry point passed explicitly.

```sh
sqlite3 data.db '.load ./libsparqlite' "SELECT sparql_version()"
sqlite3 data.db '.load ./renamed.so sqlite3_sparqlite_init' "SELECT sparql_version()"
```

```python
conn = sqlite3.connect("data.db")
conn.enable_load_extension(True)
conn.load_extension("ext/target/release/libsparqlite")  # the .so suffix is optional
```

- The init returns `SQLITE_OK_LOAD_PERMANENTLY`: the library is never unloaded (a Rust cdylib
  must not be `dlclose`d).
- Registration is per connection: every pooled connection must load the extension (or, from
  Rust, call `register`).
- Never enable rusqlite's `bundled` or `modern_sqlite` features in any crate `ext/` depends on:
  the bindings would no longer be the 3.34.1 loadable-extension ones and `extension_init2` aborts
  the host. `ext/src/lib.rs` const-asserts `SQLITE_VERSION_NUMBER`, and `scripts/check.sh`
  checks `ext/`'s feature tree.

## Testing

```sh
export CARGO_TARGET_DIR=/path/to/target CARGO_INCREMENTAL=0
cargo test -p sq-api              # debug: the debug_panic hook is always armed
cargo test -p sq-api --release    # what scripts/check.sh runs (cargo test --workspace --release)
```

The tests link the system libsqlite3; `runs_on_the_host_sqlite_version` asserts version 3.45.1
(the Python host), so it fails on any other SQLite. There are no `#[ignore]`d tests. In release
builds the panic test re-runs itself in a child process with `SPARQLITE_DEBUG_HOOKS=1`; tests
whose failure mode is a dead process also use child processes with internal switches
(`SQ_API_NESTING_CHILD`, `SQ_API_REVIEW_DEEP_CHILD`, `SQ_API_REVIEW2_WIDE`,
`SQ_API_TRUST_RECURSION_CHILD`, `SQ_API_REVIEW_R2_RECURSION`) that they set themselves.

| Where | Covers |
|---|---|
| `src/**` unit tests (47) | config parsing and defaults, per-call limits that only tighten, error codes (nested messages collapsed), `guard`, LRU eviction, the nesting cost model (14), value mapping, N-Triples parse errors that name the usual mistake (prefixed names with `rdf:`/`rdfs:`/`xsd:`/`owl:` expanded, bare IRIs, prefixed datatypes), watchdog, view schema and `idx_str`, entry re-arming and write refusal, the R8 depth and stack bounds, `calls_by_name`, debug-hook arming, RDF format names and `file://` URLs, helper id rules, result-format checks, `colUsed` bits, the parsed-query cache |
| `tests/functions.rs` (19) | every scalar function, formats, `allow_files`, DIRECTONLY in triggers/views, result codes, the `HELPER_FUNCTIONS` contract, `sparql_count` decodes nothing |
| `tests/trust.rs` (9) | function flags against SQLite's schema-trust rules: every reader in a stored view or trigger with `trusted_schema = OFF`, writers refused in stored views and triggers but allowed in TEMP ones, every reader under `PRAGMA query_only`, results stopped at `SQLITE_LIMIT_LENGTH`; hostile files: a view in place of `rdf_meta` that recurses into sparqlite fails cleanly on a 2 MiB thread (child process), writers in a stored CHECK refused on INSERT, UPDATE and `integrity_check` / `quick_check` (also from an attached database), writers never nested |
| `tests/rows.rs` (14) | `sparql_rows` columns, `colUsed` and decode counters, ASK/CONSTRUCT/20 variables, opts, 10,000-row streaming, correlation, write refusal while open |
| `tests/view.rs` (5) | typed columns, `__term`/`__id` pushdown, LIMIT and computed values, options, persistent views |
| `tests/baseline.rs` (3) | 28 queries: baseline equals native; TEMP ids through helpers; dataset options |
| `tests/transactions.rs` (11) | ROLLBACK/COMMIT, atomic requests, writing host statements, two connections, `SQLITE_BUSY` with `busy_timeout`, one snapshot per temp-view scan |
| `tests/robustness.rs` (15) | interrupts (message and `SQLITE_INTERRUPT` code when a scan's `xFilter`, `xNext` or `xColumn` notices one), `timeout_ms`, `max_memory_mb`, per-call options that cannot lift them, panics in every entry point, nesting limit on a 2 MiB stack, close hook (WAL removed) |
| `tests/review.rs` (10), `tests/review2.rs` (5), `tests/review_r2_api.rs` (5) | regressions for the adversarial review findings (round 2 of the flags work: hostile-schema recursion, writers in a stored CHECK, readers in an `rdf_quad` trigger, readers that mint terms, an `xColumn` interrupt rolling back a transaction) |
| `tests/review_perf_engine.rs` (2), `tests/review_perf_sort_stack.rs` (1) | review of the engine performance pass: `sparql_count` of a COUNT(*) query evaluates it (and times out on an endless pattern); lazily built ORDER BY keys give the baseline engine's row sequences; a 6,000-key ORDER BY list runs on a 2 MiB thread (alone in its file: a stack overflow aborts the process) |

The built extension is tested from Python's `sqlite3` module (Python 3.13, SQLite 3.45.1;
`py/tests`, 41 test functions). `conftest.py` loads the path in `SPARQLITE_EXT` (default
`ext/target/release/libsparqlite`) and sets `SPARQLITE_DEBUG_HOOKS=1` by default. Run from the
workspace root, with `pytest` installed (`scripts/check.sh` installs it into its venv):

```sh
EXT=/path/to/ext-target   # any target directory for the ext/ build
CARGO_TARGET_DIR=$EXT cargo build --release --manifest-path ext/Cargo.toml
SPARQLITE_EXT=$EXT/release/libsparqlite python -m pytest py/tests -q
cargo run --release -p sq-w3c -- --suite <rdf-tests/sparql> --backend sqlite \
    --known-failures tools/sq-w3c/known_failures.txt
```

## Performance

Measured on the project's 4-vCPU dev VM, SQLite 3.45.1, release builds (author notes):
- `count(*)` over a 250K-solution `sparql_rows` scan: 16.7 ms (67 ns/row), release cdylib, file DB
  with the B-tree reader. sq-api's streaming code costs about 80 instructions per row (callgrind).
- Decoding a column costs about 0.5 µs per term even on `DecodeCache` hits; about 1,300
  instructions per term are in `sq-exec`'s per-execution decode map and the store decode, not here.
- A bare call (read scope + epoch check + close) on a file DB costs about 11 µs, almost all of it
  SQLite's read-transaction lock and unlock (`sq-sqlite` notes).
- Planning a 240-pattern join group takes about 160 ms. With `timeout_ms` set, each baseline run
  starts one watchdog thread (tens of µs); with the default 0, none.
- W3C suite through plain SQL (`--backend sqlite`): 636 of 639 pass, the same as `--backend mem`
  (the 3 failures are spargebra parser divergences).

## Limitations and known gaps

- Scalar and aggregate functions always fail with `SQLITE_ERROR` (rusqlite sets the code, then
  `sqlite3_result_error` resets it); the `sparql: ...` message carries the detail, for example
  `sparql: SQLite error 5: database is locked`. Virtual-table methods return the real code.
- An interrupted `sparql_rows` / `sparql_view` scan always fails with `SQLITE_INTERRUPT`, but its
  message depends on who notices the interrupt first. sparqlite's code (planning, fetching a
  1024-row batch, decoding a column) reports `sparql: interrupted`; SQLite's own check in the
  statement's loop between two rows reports SQLite's `interrupted`, and no extension callback
  runs there to change it. A scan whose rows arrive quickly is usually stopped by SQLite's
  check; match on the code, or on `interrupted`.
- `timeout_ms` for `sparql_rows` / `sparql_view` counts wall-clock time from `xFilter`, including
  time the consumer spends between rows (`Execution` fixes its deadline at creation).
- Inside a writing statement only `sparql_update` works, only in autocommit mode, and only when
  the host is the sole running statement (conservative: a partly read `SELECT` also blocks it).
  `sparql_load`, `sparql_analyze`, `sparql_vacuum_terms` and `sparql_init` on an absent schema are
  refused there, as are writers in a writing statement inside an explicit transaction. While a
  table stored in `main` or an attached database names a DIRECTONLY function, every writer runs
  only from a top-level read-only statement that calls it by name (`direct_guard`). Any other
  writer call (in a TEMP trigger or view, in `INSERT ... SELECT`, beside another running
  statement) first reads `sqlite_schema` of `main` and every attached database: about 50 µs with
  20 tables on a file DB, and a database locked by another connection's write makes it wait
  (`busy_timeout`) or fail with `SQLITE_BUSY`. A plain `SELECT sparql_update(?)` skips that scan.
- An interrupt (or an I/O or disk-full error) that any virtual-table method reports with its own
  code inside a writing statement in an explicit transaction, for example
  `INSERT INTO t SELECT ... FROM sparql_rows(...)` after `BEGIN`, makes SQLite roll back the whole
  transaction, not only the statement, as `sqlite3_interrupt` documents. `xColumn` reports
  `SQLITE_INTERRUPT` too, so this also holds when the interrupt is noticed while decoding a
  column.
- Nesting limit: roughly 240 UNION/OPTIONAL/FILTER/BIND elements or triple patterns in one join
  group, 190 nested parentheses or function calls, 80 nested groups or EXISTS, 34 nested
  sub-queries, 950-term `+`/`||` chains, 105 nested `[ ]` and about 490 DESCRIBE targets (the
  256-variable limit caps DESCRIBE at 255 anyway). The model errs high in places (first UNION
  branch, GRAPH/SERVICE bodies, dots in prefixed names). These bounds last until `sq-plan` /
  `sq-exec` stop recursing once per join or BIND level.
- A `<` written directly after an operand whose text up to the next `>` looks like an IRI with
  `'`, `#`, `\` or unbalanced brackets is refused as ambiguous ("add a space"). Many unterminated
  `'''` literals make the scan quadratic (spargebra's own parse is similar).
- Baseline engine: the `sq-sqlgen` translator refuses CONSTRUCT / DESCRIBE
  (`sparql: unsupported: baseline: CONSTRUCT queries`), also through `engine=sql`.
- Helpers called directly from user SQL treat TEMP arguments as unbound and return NULL for TEMP
  results (e.g. `sparql_eval('CONCAT', ...)` on long strings, `sparql_group_concat`).
- `__term` matches only the exact canonical N-Triples text (`"x"@EN` does not match `"x"@en`), and
  `BNODE()` results cannot be matched through it. Value columns are never pushed down.
- The `debug_panic` key is compiled into release builds; it is armed only in debug builds or when
  `SPARQLITE_DEBUG_HOOKS=1` is in the environment when `register()` runs for that connection
  (for the extension: when it is loaded into the connection), and is absent from `config.*`.
- Plain-SQL writes to `rdf_quad` during an open scan are undefined; only sparqlite's own writers
  are refused while `open_cursors > 0`.

**Deviations from docs/SPEC.md §8 and §1.6** (the code is current): `xFilter` is a full read
entry and opens a savepoint when `main` has no transaction (a `temp.` view), releasing it before
returning, instead of "nothing is opened" (§8.6); no scope is held between rows. `id*`/`__id`
report the stored id of a computed term instead of NULL for every TEMP id (§8.3a). An absent
`__term` value is a TEMP constant rather than "zero rows" (§8.3b). The watchdog uses a `Mutex`
despite R1's "no Mutex" (it never guards connection state and is never held while a statement
steps). `reader=auto` uses the B-tree reader when its preconditions hold (§8.5 says `sql` in v1),
and `page_cache_mb` is an extra key.
`sparql_rows` opts also accept `probe_counts` / `probe_cap`. `sparql_term` also returns NULL for
the ERROR sentinel, reserved tags and invalid inline ids, and errors on a dictionary id with no row.
`sparql_value` also answers ASK (0/1) and CONSTRUCT / DESCRIBE (the first triple's subject); §8.2
only defines the first projected variable of the first row. Outside the §1.6 SQLite API
allow-list, the crate also calls `sqlite3_next_stmt`, `sqlite3_stmt_busy` and
`sqlite3_stmt_readonly` (`host_statement_writes`, `direct_guard`), `sqlite3_sql`
(`direct_guard`), `sqlite3_vtab_collation` (through `IndexInfo::collation` in `sparql_view`'s
`xBestIndex`) and `sqlite3_limit` (reading `SQLITE_LIMIT_LENGTH`, `ConnState::length_limit`); all
six exist in libsqlite3-sys's `bindgen_3.34.1_ext` bindings. Per-call `timeout_ms` /
`max_memory_mb` can only tighten the connection's values (§8.3a says the options "override" the
connection config). Any new FFI call must exist in those 3.34.1 loadable-extension
bindings too, or the `ext/` build fails; `scripts/check.sh` only greps for the known forbidden
names.

## Further reading

- [docs/SPEC.md](../../docs/SPEC.md) §8 (this crate), §4.4 (cache validity and the epoch check),
  §1.5-§1.6 (`ext/` and the SQLite API allow-list), §9 (baseline translator)
- [docs/DECISIONS.md](../../docs/DECISIONS.md): read/write scopes, close hook, SQL surface
- [sq-sqlite](../sq-sqlite/README.md) (scopes, pool, caches, B-tree reader),
  [sq-engine](../sq-engine/README.md), [sq-sqlgen](../sq-sqlgen/README.md) (helper contract),
  [sq-results](../sq-results/README.md) (formats, native mapping), [sq-exec](../sq-exec/README.md),
  [sq-plan](../sq-plan/README.md), [sq-expr](../sq-expr/README.md), [sq-core](../sq-core/README.md)
