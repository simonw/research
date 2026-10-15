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
                                   |         spargebra, oxrdf, oxrdfio, oxsdatatypes, serde_json, rand
          +------------------------+---------------------------+
   ext/ (cdylib libsparqlite,   tools/sq-w3c (feature `sqlite`,   tools/sq-bench
   own workspace, rusqlite      default on: `--backend sqlite`)   (declared dependency;
   "loadable_extension")                                          the binary is a stub)
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

**2. The entry protocol.** Every function body and every `xFilter` goes through one of:
- `read_call(f)`: `enter()` (re-arm the pool, record depth), `store.read_scope(depth)`, `validate()`
  (the §4.4 epoch check; an absent schema reads as an empty store, an incompatible one fails),
  `f`, then `scope.close()` with its error reported.
- `write_call(auto_init, f)`: refuses with `Error::WriteWhileReading` while a scan is open,
  runs `ensure_schema()` outside any scope when asked (loads and updates auto-initialize; analyze
  and vacuum do not), opens the `WriteScope` *before* any read
  (so `busy_timeout` applies to taking the write lock), validates, runs `f`, commits.
- `writer_call(f)`: for `sparql_init`, whose storage call manages its own scopes.

Inside a statement that writes (`INSERT INTO t SELECT sparql_update(...)`) SQLite refuses every
`SAVEPOINT`. `write_call` then runs without a scope only if `host_statement_writes()` proves that
the host is the only running statement and it writes (autocommit, `sqlite3_txn_state == WRITE`,
walking `sqlite3_next_stmt`): the host's transaction makes the request atomic. Otherwise the
call is refused with a message that names the restriction (`savepoint_refusal`).

**3. Close hook** (`src/vtab/status.rs`, §8.7). `register()` prepares
`SELECT 1 FROM main.sparql_status` once. Connecting that eponymous table arms the `sq-sqlite`
statement pool; its `Drop` (xDisconnect, which `sqlite3_close` runs before its busy check)
disarms the pool and finalizes idle statements, so the connection closes fully and the `-wal`
file disappears. Each entry re-arms if the instance was disconnected.

**4. Two engines behind `Run`** (`src/query.rs`). `Run::Native(Execution)` comes from
`sq_engine::prepare`; `Run::Baseline(BaselineRun)` steps `sq_sqlgen::translate`'s SQL through
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
`idx_str`). `count(*)` and `id*` columns decode nothing; an unannounced column is decoded on
demand. `id*` / `__id` hold the term's id in the store: a computed (TEMP) term that exists in the
dictionary reports its stored id, otherwise NULL (one decode and one dictionary probe per TEMP
cell read; stored ids cost nothing).

**6. View pushdown** (`src/vtab/view.rs`). `xBestIndex` consumes `=` constraints on `__id`
columns, on `__term` columns under BINARY collation only, and on the hidden `opts` column.
`xFilter` resolves each value to an id and wraps the lowered query's root in a top-level
`FILTER(sameTerm(?v, c) && ...)` before `sq_plan::optimize`, so the result equals the SQL filter
applied to the view's rows (after LIMIT, unbound never matches). A value that can never equal
the column (wrong type, non-canonical N-Triples spelling, an id naming no term) gives an empty
scan; a well-formed term absent from the store becomes a TEMP constant.

**7. Nesting budget** (`src/nesting.rs`). A stack overflow is not a panic and kills the host. Every
request text is scanned before any parse and refused (`Error::Unsupported`, "nests too deeply")
when its estimated recursion cost exceeds `LIMIT = 1000` (weights are roughly KiB of stack per
level, so the worst case stays near 1 MiB). Flat data (`INSERT DATA`, `VALUES`, templates) costs
nothing.

**8. Panic containment and errors** (`src/guard.rs`, `src/error.rs`). rusqlite catches panics in
scalar and aggregate functions but not in virtual-table trampolines, so every vtab method body
runs in `guard()` (`guard_eof` for `xEof`, `guard_drop` for destructors). `sql_error` maps
`sq_core::Error` to a SQL error whose message starts with `sparql: `; vtab methods also return
`SQLITE_INTERRUPT` or the original SQLite code.

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

SQL functions (each arity registered separately; none is DETERMINISTIC; D = `SQLITE_DIRECTONLY`,
so not callable from views, triggers or the schema):

| Function | Arities | Flags | Returns |
|---|---|---|---|
| `sparql_version` | 0 | | TEXT |
| `sparql_init` | 0 | D | schema version (idempotent) |
| `sparql_load(data [, format [, base [, graph]]])` | 1-4 | D | new quads; data TEXT or BLOB; format default `turtle` |
| `sparql_load_file(path [, format [, base [, graph]]])` | 1-4 | D | new quads; needs `allow_files`; format from extension, base = `file://` URL |
| `sparql_update(update)` | 1 | D | inserted + deleted; `LOAD <file:...>` follows `allow_files` |
| `sparql_analyze()` / `sparql_vacuum_terms()` | 0 | D | JSON summary / rows deleted |
| `sparql_check()` | 0 | | `ok: ...` or `errors: ...` |
| `sparql_config(key)` / `sparql_config(key, value)` | 1 / 2 | - / D | current / new value as TEXT |
| `sparql_query(q [, format])` | 1-2 | | json (default)/xml/csv/tsv; graphs: ntriples (default)/nquads/turtle |
| `sparql_ask`, `sparql_count`, `sparql_value` | 1 | | 0/1; count with no decode; native value |
| `sparql_explain(q [, mode])` | 1-2 | | `algebra`, `logical`, `plan` (default), `analyze`, `sql`, `sqlplan` |
| `sparql_query_sql`, `sparql_count_sql`, `sparql_to_sql` | 1-2, 1, 1 | | baseline engine regardless of `engine` |
| `sparql_term`, `sparql_term_value`, `sparql_term_id`, `sparql_term_kind/_lex/_lang/_datatype` | 1 | | term accessors |
| `sparql_num`, `sparql_str`, `sparql_ebv`, `sparql_sortkey`, `sparql_regex` (2-3), `sparql_eval` (variadic) | | | baseline helpers |
| aggregates `sparql_sum/_avg/_min/_max/_sample`, `sparql_group_concat` (1-2) | | | baseline helpers |

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
object of per-call keys. `sparql_view` declares the projected variables as value columns, then
hidden `<v>__term`, `<v>__id` and `opts`; it accepts `key=value` arguments after the query and
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
(`SQ_API_NESTING_CHILD`, `SQ_API_REVIEW_DEEP_CHILD`, `SQ_API_REVIEW2_WIDE`) that they set
themselves.

| Where | Covers |
|---|---|
| `src/**` unit tests (43) | config parsing and defaults, error codes, `guard`, LRU eviction, the nesting cost model (14), value mapping, watchdog, view schema and `idx_str`, entry re-arming and write refusal |
| `tests/functions.rs` (19) | every scalar function, formats, `allow_files`, DIRECTONLY in triggers/views, result codes, the `HELPER_FUNCTIONS` contract, `sparql_count` decodes nothing |
| `tests/rows.rs` (14) | `sparql_rows` columns, `colUsed` and decode counters, ASK/CONSTRUCT/20 variables, opts, 10,000-row streaming, correlation, write refusal while open |
| `tests/view.rs` (5) | typed columns, `__term`/`__id` pushdown, LIMIT and computed values, options, persistent views |
| `tests/baseline.rs` (3) | 28 queries: baseline equals native; TEMP ids through helpers; dataset options |
| `tests/transactions.rs` (11) | ROLLBACK/COMMIT, atomic requests, writing host statements, two connections, `SQLITE_BUSY` with `busy_timeout`, one snapshot per temp-view scan |
| `tests/robustness.rs` (11) | interrupts, `timeout_ms`, `max_memory_mb`, panics in every entry point, nesting limit on a 2 MiB stack, close hook (WAL removed) |
| `tests/review.rs` (10), `tests/review2.rs` (5) | regressions for the adversarial review findings |

The built extension is tested from Python 3.45.1 (`py/tests`, 33 test functions; `conftest.py` reads
`SPARQLITE_EXT` and sets `SPARQLITE_DEBUG_HOOKS=1` by default):

```sh
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
- `timeout_ms` for `sparql_rows` / `sparql_view` counts wall-clock time from `xFilter`, including
  time the consumer spends between rows (`Execution` fixes its deadline at creation).
- Inside a writing statement only `sparql_update` works, only in autocommit mode, and only when
  the host is the sole running statement (conservative: a partly read `SELECT` also blocks it).
  `sparql_load`, `sparql_analyze`, `sparql_vacuum_terms` and `sparql_init` on an absent schema are
  refused there, as are writers in a writing statement inside an explicit transaction.
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
  `SPARQLITE_DEBUG_HOOKS=1` is set before the extension loads, and is absent from `config.*`.
- Plain-SQL writes to `rdf_quad` during an open scan are undefined; only sparqlite's own writers
  are refused while `open_cursors > 0`.

**Deviations from docs/SPEC.md §8** (the code is current): `xFilter` is a full read entry and opens
a savepoint when `main` has no transaction (a `temp.` view), releasing it before returning, instead
of "nothing is opened" (§8.6); no scope is held between rows. `id*`/`__id` report the stored id of a
computed term instead of NULL for every TEMP id (§8.3a). An absent `__term` value is a TEMP constant
rather than "zero rows" (§8.3b). The watchdog uses a `Mutex` despite R1's "no Mutex" (it never
guards connection state and is never held while a statement steps). `reader=auto` uses the B-tree
reader when its preconditions hold (§8.5 says `sql` in v1), and `page_cache_mb` is an extra key.
`sparql_rows` opts also accept `probe_counts` / `probe_cap`. `sparql_term` also returns NULL for
the ERROR sentinel, reserved tags and invalid inline ids, and errors on a dictionary id with no row.

## Further reading

- [docs/SPEC.md](../../docs/SPEC.md) §8 (this crate), §4.4 (cache validity and the epoch check),
  §1.5-§1.6 (`ext/` and the SQLite API allow-list), §9 (baseline translator)
- [docs/DECISIONS.md](../../docs/DECISIONS.md): read/write scopes, close hook, SQL surface
- [sq-sqlite](../sq-sqlite/README.md) (scopes, pool, caches, B-tree reader),
  [sq-engine](../sq-engine/README.md), [sq-sqlgen](../sq-sqlgen/README.md) (helper contract),
  [sq-results](../sq-results/README.md) (formats, native mapping), [sq-exec](../sq-exec/README.md),
  [sq-plan](../sq-plan/README.md), [sq-expr](../sq-expr/README.md), [sq-core](../sq-core/README.md)
