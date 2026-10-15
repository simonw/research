# sparqlite: final architecture specification (v1.0)

sparqlite is a SQLite loadable extension written in Rust. It implements SPARQL 1.1 Query and Update over RDF stored in ordinary SQLite tables. The query engine runs inside the SQLite process and uses its own planner and join operators. It drives prepared, index-pinned covering range scans directly, with `sqlite3_step` + `sqlite3_column_int64` over integer-encoded quad B-trees. Phase 2 adds a direct B-tree page reader behind the same cursor trait.

The design is fixed up front so that several agents can build it in parallel. Read it in this order: §0, §1 (build), §10 (the frozen `sq-core` source), §11 (per-crate APIs), §15 (work packages).

---

## 0. Decisions at a glance

| Area | Decision | Source |
|---|---|---|
| Storage | One `rdf_quad(g,s,p,o)` WITHOUT ROWID table. GSPO is the PK; GPSO, GPOS and GOSP are full covering indexes, so every `?s :p ?o` has both an s-sorted and an o-sorted stream. There are 4 g-last **partial** indexes `WHERE g<>0` (named graphs only) and 1 tiny partial index `rdf_gpox ... WHERE (o & 15) IN (5,8)` that makes integer range pushdown exact without trusting statistics. | speed + new |
| Ids | 64-bit, 4-bit tag in the **low** bits. Inline values: INT, BOOL, DECIMAL, DATETIME, DATE and SHORTSTR, only under the strict rule `format(decode(x)) == lexical`. Blank nodes are counter ids with no dictionary row. TEMP ids are execution-local. The ERROR sentinel exists only as a group key. | speed + correctness |
| Cursor | `QuadCursor { width, fill, seek(&[..]), rebind }`. It selects only the free suffix columns and uses seekable lower-bound and row-value templates. Cursors are `'static`, each owning one leased pooled statement. | speed |
| Semantics | Every bind-join, filter pushdown and EXISTS rewrite is gated by a `certain/maybe/safe_inject` analysis. GRAPH ?g is pushed into patterns only when that is provably equivalent; otherwise `GraphLoop` is used. Zero-length paths follow the `has_node` rule. The `=` matrix keeps Other and IllTyped separate. The ORDER BY order is built from precomputed `SortKey`s and is total by construction. | correctness + semantic judge |
| Planner | DP over join groups with interesting orders. Operators: Merge (galloping, factorized COUNT), StarMerge (only when cheaper), BindJoin (sorted, deduplicated probes, adaptive switch to hash), Hash, Sort enforcer, and the `SqlJoin` hybrid, which hands selective index-nested-loop chains to SQLite's VDBE. Cardinality comes from characteristic sets, heavy hitters and bounded counts (cap 1000, at most 32 per query). | speed + correctness + engineering |
| Build | The root workspace links the **system** libsqlite3 3.45.1 (no `bundled`). `ext/` is a separate workspace that adds `loadable_extension`. A compile-time version guard and a `cargo tree` gate are in place. Only C APIs present in libsqlite3-sys 0.38.2 `bindgen_3.34.1_ext.rs` may be used (no `total_changes64`, `is_interrupted`, `vtab_in` or `vtab_rhs_value`). | engineering, verified |
| SQL surface | Scalar functions. The `sparql_rows(query[,opts])` table-valued function has fixed columns `n, v0..v15, t0..t15, id0..id15, vars, json` and uses `col_used` so unread columns are never decoded. `CREATE VIRTUAL TABLE … USING sparql_view('…')` gives typed columns. `sparql_status` is both a status table and the **close hook**. | speed + engineering |
| Caches | Connection caches are keyed by `(dict_gen, schema_version)`, not by `total_changes`. They survive writes, including writes to unrelated tables. Only dictionary rows that are known to be committed (`row ≤ committed_max_row`) are cached, which makes rollback safe. Only parsed queries are cached; plans never are. | new (fixes all three) |
| Phase 2 | `sq-btree` is a direct page reader behind `QuadCursor`, with a `reader=sql` kill switch. It is only used for rollback-journal file databases with a held read lock and no write transaction. | speed, deferred |

Measured ground truth for this machine (SQLite 3.45.1, 4 vCPU, ~1–1.4M triples, warm cache), from the design and judge experiments in the scratchpad:

- Pinned scan: 105–140 ns/row for 2–3 columns, 77–105 ns/row for 1 column. Each extra column costs 15–20%.
- `count(*)` inside the VDBE: 37–49 ns/row.
- Point probe: about 1.0–1.4 µs inside a held read transaction, 7–8 µs without one. Sorted probes: 0.66–0.84 µs.
- SQLite's own index-nested-loop join: about 0.55 µs per probe.
- Dictionary decode miss: about 1 µs; cache hit 46–75 ns.
- chain-2 (1.8M rows): merge join 144–172 ms; scan+sort+merge 167–281 ms; hash join 387–611 ms; SQLite 360–650 ms.
- star-4: SQLite 39 ms, our index nested loop (INL) 50 ms, our merge 86–91 ms.
- Hash build for 600K random keys: about 0.33–0.65 µs/row including the scan.

---

## 1. Workspace, build and the `bundled` vs `loadable_extension` resolution

### 1.1 Repository conventions (from `/home/user/research/AGENTS.md`)
- All work lives in **`/home/user/research/sqlite-sparql-rust/`**. Keep `notes.md` (an append-only log) and finish with a `README.md` report.
- Commit only files we wrote. Never commit copies of fetched code: rdf-tests and crate sources stay outside the folder (rdf-tests is already at `/tmp/claude-0/-home-user-research/82148577-c93b-5a93-8757-e7fe894ecb5e/scratchpad/w3c/rdf-tests`).
- Binaries over 2 MB and `target/` are not committed. HTML demos, if any, are linked through `https://simonw.github.io/research/sqlite-sparql-rust/…`.
- A verified copy of the `sq-core` crate (§10) and a prototype of the ext/close-hook/`col_used` plumbing are in `/tmp/claude-0/-home-user-research/82148577-c93b-5a93-8757-e7fe894ecb5e/scratchpad/synth/ws/`. Copy `crates/sq-core` from there verbatim.

### 1.2 Layout
```
sqlite-sparql-rust/        (project/product name: sparqlite)
  Cargo.toml               [workspace] members = ["crates/*", "tools/*"], exclude = ["ext"]
  rust-toolchain.toml      channel = "1.97"
  crates/
    sq-core/      FROZEN contract (§10): ids, codec, terms, batch, store traits, stats, IR, plan, eval traits, testing
    sq-memstore/  in-memory QuadStore + QuadStoreWriter (oracle store / test double for everybody)
    sq-sqlite/    DDL, RawDb, StmtPool, SqlCursor, dictionary + caches, read/write scopes, epoch, bulk loader, analyze
    sq-expr/      Value model, expression compiler/evaluator, builtins, casts, SortKey, aggregate accumulators
    sq-plan/      parse, lower (spargebra -> Logical), props, rewrites, cardinality, DP, physical selection, validate, explain
    sq-exec/      operators, paths, EXISTS, GraphLoop, CONSTRUCT/DESCRIBE, Execution (implements SolutionSource)
    sq-oracle/    naive spec-transcribed evaluator over Logical IR (differential-testing oracle)
    sq-results/   SPARQL JSON/XML/CSV/TSV (sparesults), N-Triples/N-Quads/Turtle (oxrdfio), native SQL mapping, row JSON
    sq-sqlgen/    baseline SPARQL->SQL translator over LoweredQuery
    sq-engine/    facade: query/count/explain/update/load over Rc<dyn QuadStore>; Update executor; generic loader
    sq-api/       register(&Connection): functions, vtabs, ConnState, guard(); used by the cdylib AND native tests
    sq-btree/     PHASE 2: direct B-tree page reader (QuadCursor impl)
  tools/
    sq-w3c/       W3C manifest runner (bin)
    sq-bench/     data generators + Rust micro-benchmarks (bin)
  ext/                     SEPARATE workspace: cdylib "sparqlite" -> libsparqlite.so -> sqlite3_sparqlite_init
  py/  tests/ (pytest), bench.py, diff_oxigraph.py, gen_social.py, gen_bsbm.py
  scripts/check.sh
  notes.md  README.md
```

### 1.3 Crate dependency graph (acyclic)
```
sq-core <- every crate
sq-expr      <- sq-exec, sq-oracle, sq-engine
sq-exec      <- sq-engine                      (sq-exec depends only on sq-core + sq-expr)
sq-plan      <- sq-engine
sq-results   <- sq-engine, sq-api, tools       (depends only on sq-core + sparesults + oxrdfio)
sq-sqlgen    <- sq-api                         (depends only on sq-core)
sq-sqlite    <- sq-api, tools                  (depends on sq-core + rusqlite + oxrdfio)
sq-engine    <- sq-api, tools
sq-memstore  <- dev-dependency of sq-plan, sq-exec, sq-oracle, sq-engine, tools
sq-api       <- ext, tools/sq-w3c, tools/sq-bench
```

### 1.4 Root `Cargo.toml`
```toml
[workspace]
resolver = "3"
members = ["crates/*", "tools/*"]
exclude = ["ext"]

[workspace.package]
edition = "2024"
version = "0.1.0"
rust-version = "1.85"

[workspace.dependencies]
oxrdf = "=0.3.4"
spargebra = "=0.4.7"          # default features only: NO sparql-12, sep-0002, sep-0006
oxttl = "=0.2.4"
oxrdfio = "=0.2.6"
sparesults = "=0.3.4"
oxsdatatypes = "=0.2.3"       # XSD datatype library (not an evaluator); allowed
oxiri = "0.2.11"
thiserror = "2"
rustc-hash = "2"
smallvec = "1"
xxhash-rust = { version = "0.8", features = ["xxh3"] }
regex = "1"
md-5 = "0.10"
sha1 = "0.10"
sha2 = "0.10"
rand = "0.9"
serde_json = "1"
# Links the SYSTEM libsqlite3 (3.45.1 = the Python host). NEVER add "bundled" or "modern_sqlite".
rusqlite = { version = "=0.40.2", default-features = false, features = ["functions", "vtab"] }
sq-core = { path = "crates/sq-core" }
sq-memstore = { path = "crates/sq-memstore" }
sq-sqlite = { path = "crates/sq-sqlite" }
sq-expr = { path = "crates/sq-expr" }
sq-plan = { path = "crates/sq-plan" }
sq-exec = { path = "crates/sq-exec" }
sq-oracle = { path = "crates/sq-oracle" }
sq-results = { path = "crates/sq-results" }
sq-sqlgen = { path = "crates/sq-sqlgen" }
sq-engine = { path = "crates/sq-engine" }
sq-api = { path = "crates/sq-api" }

[profile.release]
debug = 1
panic = "unwind"              # we catch_unwind at every FFI boundary
lto = "thin"
```
Per-crate dependencies:

| Crate | Dependencies |
|---|---|
| sq-memstore | sq-core |
| sq-sqlite | sq-core, rusqlite, oxrdf, oxrdfio, rustc-hash |
| sq-expr | sq-core, oxsdatatypes, oxiri, regex, md-5, sha1, sha2, rand, rustc-hash |
| sq-plan | sq-core, spargebra, rustc-hash, smallvec (dev: sq-memstore) |
| sq-exec | sq-core, sq-expr, rustc-hash, smallvec, rand (dev: sq-memstore) |
| sq-oracle | sq-core, sq-expr (dev: sq-memstore) |
| sq-results | sq-core, sparesults, oxrdfio, oxrdf |
| sq-sqlgen | sq-core |
| sq-engine | sq-core, sq-plan, sq-exec, sq-expr, sq-results, oxrdfio (dev: sq-memstore, sq-oracle) |
| sq-api | sq-core, sq-engine, sq-sqlite, sq-sqlgen, sq-results, rusqlite, serde_json |
| tools/sq-w3c | sq-api, sq-sqlite, sq-engine, sq-memstore, sq-oracle, sq-results, rusqlite, oxttl, oxrdfio, sparesults |
| tools/sq-bench | sq-api, rusqlite, oxrdfio |

`sq-core` has no SQLite dependency.

### 1.5 `ext/` (separate workspace, so features never unify with the test binaries)
`ext/Cargo.toml`:
```toml
[package]
name = "sparqlite-ext"
version = "0.1.0"
edition = "2024"

[lib]
name = "sparqlite"                # -> libsparqlite.so -> SQLite derives entry point sqlite3_sparqlite_init
crate-type = ["cdylib"]

[workspace]                       # standalone on purpose (feature unification!)

[dependencies]
sq-api = { path = "../crates/sq-api" }
rusqlite = { version = "=0.40.2", default-features = false, features = ["loadable_extension", "functions", "vtab"] }

[profile.release]
lto = "fat"
codegen-units = 1
panic = "unwind"
```
`ext/src/lib.rs`, which is complete (prototype verified: it builds, loads into Python's SQLite 3.45.1, and `cargo tree` shows no `bundled`):
```rust
use rusqlite::{ffi, Connection};
use std::os::raw::{c_char, c_int};

// libsqlite3-sys 0.38.2 uses its bindgen_3.34.1_ext bindings for loadable_extension (3034001).
// If "bundled"/"modern_sqlite" leaks in, the bindings become 3.53.x and extension_init2 aborts the host.
const _: () = assert!(ffi::SQLITE_VERSION_NUMBER <= 3_045_000,
    "rusqlite 'bundled'/'modern_sqlite' leaked into the loadable extension build");

fn init(conn: Connection) -> rusqlite::Result<bool> {
    sq_api::register(&conn)?;
    Ok(true) // SQLITE_OK_LOAD_PERMANENTLY: never dlclose a Rust cdylib
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_sparqlite_init(
    db: *mut ffi::sqlite3, pz_err_msg: *mut *mut c_char, p_api: *mut ffi::sqlite3_api_routines,
) -> c_int {
    std::panic::catch_unwind(|| unsafe { Connection::extension_init2(db, pz_err_msg, p_api, init) })
        .unwrap_or(ffi::SQLITE_ERROR)
}
```

### 1.6 SQLite C API allow-list
All library crates call SQLite only through `rusqlite::ffi::*` and `rusqlite::Connection`, so the same source compiles in linked mode and in loadable-extension mode.

**Allowed** (all present in `bindgen_3.34.1_ext.rs`):
- `sqlite3_prepare_v3` with `SQLITE_PREPARE_PERSISTENT`
- `step`, `reset`, `clear_bindings`, `finalize`, `bind_int64`, `bind_text`, `column_*`
- `sqlite3_txn_state` (`SQLITE_TXN_NONE/READ/WRITE`)
- `sqlite3_get_autocommit`, `sqlite3_total_changes` (32-bit; diagnostics only), `sqlite3_changes`
- `sqlite3_db_filename`, `sqlite3_interrupt`
- `sqlite3_file_control` with `SQLITE_FCNTL_FILE_POINTER` and `SQLITE_FCNTL_DATA_VERSION` (phase 2)
- `sqlite3_vtab_config`, `create_module_v2`, `create_function_v2`

**Forbidden** (absent in those bindings; they fail with E0425 in both modes):
- `sqlite3_total_changes64`, `sqlite3_changes64`, `sqlite3_is_interrupted`, `sqlite3_vtab_in*`, `sqlite3_vtab_rhs_value`, `sqlite3_vtab_distinct`
- rusqlite methods gated on `modern_sqlite`: `Connection::is_interrupted`, `transaction_state`, `db_name`, `IndexInfo::rhs_value`, `in_values`, and similar.

Interrupt detection steps a pooled `SELECT 1`, which returns `SQLITE_INTERRUPT` after `sqlite3_interrupt` (about 0.3 µs). Do not install a progress handler: that slot belongs to the user.

### 1.7 `scripts/check.sh` (the one command CI runs)
```bash
set -euo pipefail
VENV=/tmp/claude-0/-home-user-research/82148577-c93b-5a93-8757-e7fe894ecb5e/scratchpad/venv
W3C=/tmp/claude-0/-home-user-research/82148577-c93b-5a93-8757-e7fe894ecb5e/scratchpad/w3c/rdf-tests/sparql
cargo test --workspace --release                                   # native, system libsqlite3 3.45.1
cargo build --release --manifest-path ext/Cargo.toml               # ext/target/release/libsparqlite.so
! cargo tree --manifest-path ext/Cargo.toml -e features -i libsqlite3-sys | grep -E 'bundled|modern_sqlite'
! grep -rnE 'total_changes64|is_interrupted|changes64|vtab_in|rhs_value' crates/ ext/src
$VENV/bin/python -m pip install -q pytest
$VENV/bin/python -m pytest py/tests -q
cargo run --release -p sq-w3c -- --suite $W3C --backend sqlite --known-failures tools/sq-w3c/known_failures.txt
cargo run --release -p sq-w3c -- --suite $W3C --backend mem    --known-failures tools/sq-w3c/known_failures.txt
$VENV/bin/python py/diff_oxigraph.py --ext ext/target/release/libsparqlite --cases 2000 --seed 1
```
A native test asserts `sqlite3_libversion_number() == 3045001`, so Rust tests run against exactly the host engine.

---

## 2. Storage schema

### 2.1 Exact DDL (schema_version 1, encoding_version 1)
`sparql_init()` runs the following in one write scope (§8.6). It is idempotent. All generated SQL is schema-qualified as `main.` so that a user's `temp.rdf_quad` can never shadow ours. Init never changes PRAGMAs itself. The README recommends `PRAGMA page_size=8192` before init on large stores; the benchmark uses 8192 for every SQLite engine.

```sql
CREATE TABLE IF NOT EXISTS main.rdf_meta(key TEXT PRIMARY KEY, value ANY) WITHOUT ROWID;
INSERT OR IGNORE INTO main.rdf_meta(key, value) VALUES
  ('schema_version', 1), ('encoding_version', 1),
  ('next_bnode', 1),            -- blank-node counter (ids (n<<4)|2), allocated under the write lock
  ('write_gen', 0),             -- bumped first thing in every write scope (takes the write lock early)
  ('dict_gen', 0),              -- bumped only when dictionary rows are deleted (sparql_vacuum_terms)
  ('stats_gen', 0),             -- bumped by sparql_analyze
  ('analyzed_quads', 0),
  ('created_by', 'sparqlite 0.1.0');

-- Term dictionary. Row r: TermId = (r << 4) | tag. Rows 1..=63 hold codec::WELL_KNOWN IRIs (fixed);
-- user rows start at 1024. Append-only except sparql_vacuum_terms() (which bumps dict_gen).
CREATE TABLE IF NOT EXISTS main.rdf_term(
  id   INTEGER PRIMARY KEY,                       -- rowid alias: decode = 1 B-tree seek
  tag  INTEGER NOT NULL CHECK (tag IN (1,3,4,5)), -- 1 IRI | 3 STR | 4 LANG | 5 TYPED
  lex  TEXT    NOT NULL,                          -- IRI / exact lexical form (never canonicalised)
  dt   INTEGER,                                   -- TYPED: TermId of the datatype IRI
  lang TEXT,                                      -- LANG: lower-case tag
  h    INTEGER NOT NULL                           -- codec::term_hash(tag, lex, extra)
);
CREATE INDEX IF NOT EXISTS main.rdf_term_h ON rdf_term(h);   -- not UNIQUE: collisions resolved by exact compare

CREATE TABLE IF NOT EXISTS main.rdf_quad(
  g INTEGER NOT NULL,          -- 0 = default graph (record serial type 8: zero payload bytes)
  s INTEGER NOT NULL, p INTEGER NOT NULL, o INTEGER NOT NULL,
  PRIMARY KEY (g, s, p, o)
) WITHOUT ROWID;                                                      -- GSPO
CREATE INDEX IF NOT EXISTS main.rdf_gpso ON rdf_quad(g, p, s, o);
CREATE INDEX IF NOT EXISTS main.rdf_gpos ON rdf_quad(g, p, o, s);
CREATE INDEX IF NOT EXISTS main.rdf_gosp ON rdf_quad(g, o, s, p);
CREATE INDEX IF NOT EXISTS main.rdf_spog ON rdf_quad(s, p, o, g) WHERE g<>0;   -- named graphs only
CREATE INDEX IF NOT EXISTS main.rdf_psog ON rdf_quad(p, s, o, g) WHERE g<>0;
CREATE INDEX IF NOT EXISTS main.rdf_posg ON rdf_quad(p, o, s, g) WHERE g<>0;
CREATE INDEX IF NOT EXISTS main.rdf_ospg ON rdf_quad(o, s, p, g) WHERE g<>0;
CREATE INDEX IF NOT EXISTS main.rdf_gpox ON rdf_quad(g, p, o, s) WHERE (o & 15) IN (5,8);  -- numeric exceptions

CREATE TABLE IF NOT EXISTS main.rdf_graph(g INTEGER PRIMARY KEY);   -- named-graph registry (incl. empty CREATE GRAPH)

-- Statistics (hints only; written by sparql_analyze). scope 0 = default graph, 1 = union of named graphs.
CREATE TABLE IF NOT EXISTS main.rdf_stat_global(scope INTEGER PRIMARY KEY, n INTEGER NOT NULL,
  ds INTEGER NOT NULL, dp INTEGER NOT NULL, dobj INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS main.rdf_stat_pred(scope INTEGER NOT NULL, p INTEGER NOT NULL,
  n INTEGER NOT NULL, ds INTEGER NOT NULL, dobj INTEGER NOT NULL, imin INTEGER, imax INTEGER,
  PRIMARY KEY (scope, p)) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS main.rdf_stat_po(scope INTEGER NOT NULL, p INTEGER NOT NULL,
  o INTEGER NOT NULL, n INTEGER NOT NULL, PRIMARY KEY (scope, p, o)) WITHOUT ROWID;   -- heavy hitters
CREATE TABLE IF NOT EXISTS main.rdf_stat_cs(scope INTEGER NOT NULL, cs INTEGER NOT NULL,
  subjects INTEGER NOT NULL, preds BLOB NOT NULL, occ BLOB NOT NULL,           -- LE i64 arrays, same order
  PRIMARY KEY (scope, cs)) WITHOUT ROWID;                                      -- characteristic sets
```
Init also inserts the `WELL_KNOWN` rows with `INSERT OR IGNORE INTO main.rdf_term(id,tag,lex,dt,lang,h) VALUES (?,1,?,NULL,NULL,?)`, with `h` computed by `term_hash`. If `rdf_meta.encoding_version` or `schema_version` differ from the compiled constants, every API call fails with `Error::Schema`.

### 2.2 Why this layout
- **Bound positions to order.** Every bound/unbound combination is an index prefix (`IndexOrder::candidates`, verified by a test):

  | Bound besides g | Order | Output sorted by |
  |---|---|---|
  | — | GSPO | s,p,o |
  | s / s,p / s,p,o | GSPO | p,o / o / – |
  | p | **GPSO** (s-sorted) or GPOS (o-sorted) — the planner picks by interesting order | |
  | p,o | GPOS | s |
  | o / o,s | GOSP | s,p / p |

- **Why four g-first orders.** SPO/POS/OSP alone cover every prefix. GPSO is the fourth, and it is the order that makes subject stars and `?a :p ?b . ?b :q ?c` chains mergeable with no sort: GPOS(p) is b-sorted and GPSO(q) is b-sorted. Measured: merge 144–172 ms vs SQLite 360–650 ms vs hash 387–611 ms on chain-2.
- **WITHOUT ROWID.** A secondary index on a WITHOUT ROWID table stores its columns plus the missing PK columns, so every index is covering and there are exactly as many B-trees as orders. `EXPLAIN QUERY PLAN` shows `SEARCH rdf_quad USING COVERING INDEX …` for every template (verified, §5.2).
- **Named graphs.** They live in the same table. `GRAPH <g>`, `FROM` and default-graph scans use g-first prefixes. `GRAPH ?g` uses the partial g-last orders. Every statement on them contains the literal term `g<>0`, which is required for SQLite to use a partial index (verified). With g last, rows with equal (s,p,o) from different graphs are adjacent, so union-default and `FROM a FROM b` merges dedupe in O(1) per row. Default-graph-only data pays nothing for the partial indexes. This replaces correctness's "full7" layout (3 extra full B-trees) and engineering's split table.
- **`rdf_gpox`.** It indexes only objects with tag TYPED (5) or DECIMAL (8). That is small on typical data and is maintained by SQLite itself, so it stays exact under plain-SQL writes. An integer range scan on GPOS plus a scan of `rdf_gpox` for the same `(g,p)` sees every object that can satisfy a numeric comparison (INT inline values in range, plus all dictionary-typed and decimal values, which the kept residual FILTER decides). Correctness never depends on statistics or on counters that plain-SQL writes could bypass.
- **Size.** About 62 bytes per triple including the dictionary (86 MB for 1.4M triples, measured with low-tag ids, versus 189 MB with a high-tag layout).

### 2.3 Term de-duplication on load (sq-sqlite bulk loader)
1. **Write scope.** Run `SAVEPOINT sq_w{depth}`, then `UPDATE main.rdf_meta SET value=value+1 WHERE key='write_gen'` to take the write lock first. Then read `next_row = max(SELECT max(id) FROM main.rdf_term)+1, FIRST_USER_ROW)` and `next_bnode`.
2. **Parse** with `oxrdfio::RdfParser::from_format(fmt)`, using the base IRI and an optional target graph. Blank-node labels go through a load-local `FxHashMap<label, TermId>` that allocates `TermId::bnode(next_bnode++)`, so every load call gets fresh blank nodes. N-Triples and N-Quads inputs of 4 MB or more may use `split_slice_for_parallel_parsing(4)`; encoding stays single-threaded.
3. **Encode** each term in this order:
   1. `encode_inline` gives the id with no I/O.
   2. Otherwise look it up in the load-local `FxHashMap<TermKey, TermId>`, capped at 8M entries (cleared when full).
   3. Unless the dictionary had only well-known rows at load start, collect misses per 100K-quad chunk, sort them by hash, and run `SELECT id, tag, lex, dt, lang FROM main.rdf_term INDEXED BY rdf_term_h WHERE h=?1` with an exact compare.
   4. A true miss gets `row = next_row++`. A TYPED literal's datatype IRI is encoded first.
   5. New rows are appended in row order with `INSERT INTO main.rdf_term(id,tag,lex,dt,lang,h) VALUES (?,?,?,?,?,?)`.
4. **Quads.** Collect `Vec<[TermId;4]>` (`[g,s,p,o]`), sort, `dedup`, and insert with `INSERT OR IGNORE INTO main.rdf_quad(g,s,p,o) VALUES (?,?,?,?)`. `sqlite3_changes()` counts new rows. New `g≠0` values go into `rdf_graph` with `INSERT OR IGNORE`.
5. **Bulk mode** applies when the load has at least 100K quads and is more than 0.3× the current `rdf_quad` size (this includes an empty store):
   - `DROP INDEX` the 8 secondary indexes inside the savepoint.
   - Insert in PK order.
   - Run `PRAGMA threads=4`, then the 8 `CREATE INDEX` statements (DDL is transactional), then restore `threads`.
   - Measured: 3.98 → 3.05 s for 2 indexes per 1M rows with 4 threads.
   - Pooled statements that use these indexes re-prepare automatically afterwards; the loader uses none of them during the window.
   - Expected for 1M N-Triples: parse 0.5 s + encode 0.5 s + terms 0.3 s + sorted insert 0.9 s + 3 full index builds about 3 s + 5 partial builds about 1 s, so **6–8 s**. pyoxigraph measured 9.0 s for 1.4M.
6. **Finish.** If this is the first load or the store grew by more than 25% since `analyzed_quads`, run `analyze()` (§2.4). Then `RELEASE`. On any error, `ROLLBACK TO` followed by `RELEASE`.

The generic loader in sq-engine (for MemStore and small inserts) uses `QuadStoreWriter::intern` and `insert` with the same blank-node rule.

### 2.4 Statistics (`sparql_analyze`)
For each scope (g=0 through the g-first orders; named graphs through PSOG/POSG, with the dataset union treated as one scope), analyze makes two passes. That is about 0.3 s per 1M quads.

- **Pass A (GSPO, s-major).**
  - Per predicate: `n` and `ds`, where `ds` is incremented when p is first seen for a new s.
  - Characteristic sets: per subject, the sorted predicate set and the occurrence counts. These are aggregated into a `FxHashMap<Vec<p>, (subjects, occ)>`. Keep the top 10K by subjects.
  - `rdf_stat_global`.
- **Pass B (GPOS).**
  - Per p: `dobj` (counted by adjacency), and `imin`/`imax` over INT-tagged objects.
  - Heavy hitters: `(p,o)` with `n ≥ max(100, n(p)/1000)`, capped at 100K entries. This covers `rdf:type` classes.

Analyze then writes the tables and bumps `stats_gen`. It does **not** run SQLite `ANALYZE`; the benchmark harness runs `ANALYZE main` explicitly so the baseline gets `sqlite_stat1`.

---

## 3. Term identity and encoding

The complete code is in §10 (`id.rs`, `codec.rs`). The 64-bit `TermId` has a 4-bit tag in the **low** bits and is compared as **signed i64**, which is SQLite's integer order.

| Tag | Name | Payload / layout | Inline only if |
|---|---|---|---|
| 0 | UNBOUND | 0 only. Also `g=0` means the default graph. | – |
| 1 | IRI | dictionary row | – |
| 2 | BNODE | counter from `rdf_meta.next_bnode`; **no dictionary row**; label `b<hex>` | – |
| 3 | STR | dictionary row (simple literal > 7 UTF-8 bytes) | – |
| 4 | LANG | dictionary row | – |
| 5 | TYPED | dictionary row (all other literals, including non-canonical numerics, doubles, floats, ill-typed values and unknown datatypes) | – |
| 6 | INT | `v<<4`, i60 | `xsd:integer`, `-?(0\|[1-9][0-9]*)`, not `-0`, \|v\| < 2^59 |
| 7 | BOOL | 0/1 | lexical form exactly `true`/`false` |
| 8 | DECIMAL | `(m<<9)\|(dotzero<<8)\|(scale<<4)`, m: i55 | `0`, `-12` (no point); `5.0` (".0" form); `0.5`, `-12.25` (no trailing zero, scale ≤ 15); no `+`, no leading zeros, no `-0`/`-0.0` |
| 9 | DATETIME | `(ms<<16)\|(tz<<4)`, wall-clock ms of the lexical fields, tz 0=none, 1=`Z`, 2..=1682 = offset+842 | `YYYY-MM-DDThh:mm:ss[.f{1,3}][tz]`, year 0001–6000, no trailing fraction zero, no `24:00`, no `-00:00` |
| 10 | DATE | `(days<<16)\|(tz<<4)` | `YYYY-MM-DD[tz]`, same rules |
| 11 | SHORTSTR | `(be7<<8)\|(len<<4)` | simple literal of ≤ 7 UTF-8 bytes |
| 12, 13 | reserved | | |
| 14 | TEMP | `(idx<<7)\|(would_be_tag<<4)` | execution-local; never stored |
| 15 | ERROR | `TermId::ERROR` only | internal: error-valued GROUP BY key, output as UNBOUND |

**Invariant (sameTerm ⇔ id equality, within one execution).**
- `encode_inline(t)` is a pure function of `(lexical, datatype, lang)`, and its rules are pinned by `encoding_version`.
  - If it returns `Some(x)`, the canonical id is x, and t never gets a dictionary row.
  - Otherwise, IRIs and literals have exactly one `rdf_term` row (hash probe plus exact compare under the write lock).
- Blank nodes are counter ids.
- Computed terms and query constants that are absent from the store get one TEMP id per distinct term from the execution's `TempDict`. When the value can meet stored ids, `TermResolver::intern(t, canonical=true)` probes the dictionary first (§4.3).
- Therefore BGP matching, join compatibility, DISTINCT, GROUP BY, COUNT(DISTINCT), MINUS, VALUES and `sameTerm` are all raw i64 comparisons.
- Literal `=` is **value** equality, implemented in sq-expr, and is never reduced to id equality except for IRIs, blank nodes and simple literals (§6.3).
- Non-canonical lexical forms keep their identity: `distinct-num` returns `"1"`, `"01"` and `"+1"^^xsd:integer` as three distinct terms.

INT ids are monotonic in value within tag 6. `TermId::int_raw_range(lo,hi)` gives the raw GPOS range; other tags interleave inside it, and the scan drops them with a tag check that costs about 1 ns/row.

---

## 4. Solutions, value flow and caches

### 4.1 Rows and batches
- Every variable gets a `Slot` (u16) in **one global layout** for the whole query, including EXISTS patterns. That covers user variables, blank-node pattern variables, spargebra-generated variables, alpha-renamed sub-select variables and aggregate outputs.
- A row is `[TermId; width]` with 0 meaning unbound, so operators never remap columns.
- `Batch` is row-major with 1024 rows (`sq_core::batch`).
- A join copies the left row and writes the new slots (`push_extended`).
- Compatibility merging is `merge_compatible(dst, src, check)`, where `check` holds the slots that may be bound on both sides.
- More than 256 slots is `Error::Unsupported`.

### 4.2 Late materialization
Ids flow through every operator. Strings and values are created in only four places:
1. Expression evaluation that needs a value. Inline values decode with no I/O.
2. Sort keys for non-inline values.
3. GROUP_CONCAT and string functions.
4. Final output, and only for projected slots of emitted rows. In `sparql_rows`, only the columns in `col_used` are decoded.

### 4.3 Decode and lookup paths
`TermResolver` (in sq-exec) decodes an id as follows:
1. UNBOUND gives None.
2. TEMP looks up the execution `TempDict`.
3. Inline and BNODE ids use `codec::decode_inline` and `bnode_label`, with no I/O.
4. Otherwise check the per-execution `FxHashMap<TermId, Arc<Term>>`.
5. Then the connection `DecodeCache`: `FxHashMap<TermId, Arc<Term>>` with CLOCK eviction and a `decode_cache_mb` budget (default 64 MB).
6. Then `Dictionary::decode` on the sorted, deduplicated misses of the whole batch, using pooled `SELECT tag, lex, dt, lang FROM main.rdf_term WHERE id=?1` (sorted lookups cost about 0.7 µs vs 1.1 µs random). When a batch needs more than about 5% of `committed_max_row` rows, it switches to a merge scan, `SELECT id, tag, lex, dt, lang FROM main.rdf_term WHERE id >= ?1 ORDER BY id`, reseeking on gaps of more than 16 rows.

`intern(t, canonical)` works as follows:
1. Inline gives the inline id.
2. Otherwise, if `canonical`, look the term up through the connection `LookupCache` (term→id LRU, 64K entries) and then the dictionary probe (`h` index).
3. Otherwise `TempDict::intern`.

Pass-through expressions (`BIND(?x AS ?y)`, `COALESCE(?x, …)`, `IF(c, ?x, ?y)`) return the existing id with no interning.

### 4.4 Cache validity (fixes the "flush on any write" flaw in all three designs)
Each API entry steps one pooled statement inside the read scope. It costs about 2–3 µs.
```sql
SELECT (SELECT value FROM main.rdf_meta WHERE key='dict_gen'),
       (SELECT value FROM main.rdf_meta WHERE key='stats_gen'),
       (SELECT schema_version FROM pragma_schema_version),
       (SELECT max(id) FROM main.rdf_term)
```
- **DecodeCache / LookupCache** are flushed only when `dict_gen` or `schema_version` changes. Dictionary rows at or below the committed maximum are immutable: the dictionary is append-only, and only `sparql_vacuum_terms` deletes rows, which bumps `dict_gen`. Writes by this connection, writes by other connections, and writes to unrelated tables therefore never flush these caches.
- **Rollback safety.** `committed_max_row` is refreshed from the 4th column **only when `sqlite3_txn_state(db,"main") != SQLITE_TXN_WRITE`** at entry. Without a write transaction, everything visible is committed. Only dictionary ids with `row ≤ committed_max_row` are admitted to the connection caches. Ids allocated by an open (possibly rolled-back) transaction live only in per-execution caches. Negative lookups ("term absent") are never cached beyond one execution.
- **Stats** are reloaded when `stats_gen` changes. Named graphs are resolved per execution.
- **Parsed queries:** an LRU of 128 entries keyed by `(text, base)` holding `Arc<spargebra::Query>`, which saves 20–50 µs. **Plans and lowered trees are never cached**, because constants resolve against the current dictionary.
- **Schema.** A `schema_version` change re-detects the schema state (Absent / Ready / Incompatible). SQLite re-prepares pooled statements itself.
- **Unsupported.** Direct SQL `UPDATE`/`DELETE` on `rdf_term` is unsupported and documented (`sparql_check()` detects the damage). Direct SQL writes to `rdf_quad` are fully supported: no cache depends on quads, and stats are only hints.
- The phase-2 page cache is keyed separately by `SQLITE_FCNTL_DATA_VERSION` (§13).

---

## 5. Physical operators and execution model

### 5.1 Model
The executor is pull-based Volcano over 1024-row batches, single-threaded per query. Operators are `Box<dyn Operator>`, and virtual calls happen per batch. The trait is internal to sq-exec:
```rust
pub(crate) trait Operator {
    /// (Re)start. `seed` = full-width row of values bound by the caller (BindJoin: certain
    /// left slots, SeedMode::Join; EXISTS: every bound slot of the outer row, SeedMode::Substitute).
    fn open(&mut self, cx: &mut ExecCtx, seed: Option<(&[TermId], SeedMode)>) -> Result<()>;
    /// Append rows to `out` until full or exhausted; Ok(false) = exhausted.
    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool>;
    /// COUNT fast path (Scan -> store.count in VDBE; MergeJoin -> sum of run products). None = iterate.
    fn fast_count(&mut self, _cx: &mut ExecCtx) -> Option<Result<u64>> { None }
    fn close(&mut self, cx: &mut ExecCtx);
}
```
`ExecCtx` holds:
- `store: Rc<dyn QuadStore>`, the `TempDict`, the per-execution decode map, `ValueCache` and `LookupCache` handles;
- the graph register stack (`Vec<TermId>`, for `GraphSel::Active`), the resolved `DatasetSpec` and `named_graphs`;
- the EXISTS plans and their memo (keyed by the values of the correlated slots);
- `now` (fixed per query/update request), the RNG, the deadline, the memory budget (bytes charged by hash tables, sorts, DISTINCT sets and closures), the `ExecProfile`, and a row counter for interrupt polling.

CPU loops call `store.check_interrupt()` and check the deadline every 4096 rows.

### 5.2 SqlCursor templates (sq-sqlite)
`StmtKey = { order, prefix_len: u8, lower: Ge|Gt, upper: None|Le|Lt, seek_cols: u8 (0 or 2..=width), kind: Scan|Count|CountCapped }`. Templates are generated from `ScanSpec`, prepared lazily with `SQLITE_PREPARE_PERSISTENT`, and pooled.
```
SELECT <free cols, index order> FROM <order.sql_from()>
 WHERE <c0=?1 AND … c(k-1)=?k> [AND <order.sql_partial_term()>]
       AND c(k) {>=|>} ?L                                   -- always present (lower = i64::MIN when unbounded)
       [AND c(k) {<=|<} ?U]
       [AND (c(k),…,c(k+m-1)) >= (?,…,?)]                   -- seek_cols = m >= 2 (row-value resume)
 ORDER BY <all 4 cols in index order>                       -- satisfied by the index: never a sorter
```
Verified plans (`EXPLAIN QUERY PLAN` on SQLite 3.45.1):
```
SELECT s,o FROM main.rdf_quad INDEXED BY rdf_gpso WHERE g=?1 AND p=?2 AND s>=?3 ORDER BY g,p,s,o
   -> SEARCH rdf_quad USING COVERING INDEX rdf_gpso (g=? AND p=? AND s>?)
SELECT o FROM … rdf_gpso WHERE g=?1 AND p=?2 AND (s,o)>=(?3,?4) …  -> (g=? AND p=? AND (s,o)>(?,?))
SELECT p,o FROM main.rdf_quad INDEXED BY sqlite_autoindex_rdf_quad_1 WHERE g=?1 AND s=?2 AND p>=?3 ORDER BY g,s,p,o -> USING PRIMARY KEY   (NOT INDEXED does not pin the PK of a WITHOUT ROWID table on 3.45; changed during WP2)
SELECT s,o,g FROM … rdf_psog WHERE p=?1 AND g<>0 AND s>=?2 ORDER BY p,s,o,g -> COVERING INDEX rdf_psog (p=? AND s>?)
SELECT o,s FROM … rdf_gpox WHERE g=?1 AND p=?2 AND (o & 15) IN (5,8) … -> COVERING INDEX rdf_gpox (g=? AND p=?)
SELECT count(*) FROM (SELECT 1 FROM … rdf_gpos WHERE g=?1 AND p=?2 AND o=?3 LIMIT ?4)   -- capped count
```
Cursor behavior:
- **fill.** `fill(out, max)` steps up to `max` rows and appends `column_int64` for each free column. It selects **only** the free suffix columns; each extra column costs 15–20% per row.
- **seek(key).**
  - `key.len()==1`: reset, bind `?L = max(lower, key[0])` (keeping Excluded if `key[0] ≤ excluded lower`), step.
  - `key.len()≥2`: lease the `seek_cols=m` variant of the same key.
  - A lease that is already positioned and seeks forward within 6 rows (SQL path) just steps instead of reseeking. This is the galloping threshold G, which comes from `seek_sorted/scan_row`.
- **rebind(prefix, lower, upper).** Reset, clear bindings, bind, with no allocation.
- **Drop.** Reset, clear bindings, return the statement to the pool.
- **Bounded count.** `SELECT count(*) FROM (SELECT 1 … LIMIT ?cap+1)`. Exact count: `SELECT count(*) FROM … WHERE …`, which takes about 40 ns per row in the VDBE.
- A test runs `EXPLAIN QUERY PLAN` on **every** reachable `StmtKey` and asserts the expected index, the expected `(col=? …)` prefix, and the absence of `USE TEMP B-TREE`.

**StmtPool.** `RefCell<FxHashMap<StmtKey|SqlText, Vec<RawStmt>>>`, with at most 8 idle statements per key; SqlJoin statements are keyed by SQL text in an LRU of 256. Borrows last only for check-out and check-in. Nested scans of the same shape hold separate leases, so two `sparql_rows` cursors in one SQL statement are independent. The pool is **armed** only while the `sparql_status` close hook is connected (§8.7); when disarmed, leases finalize on drop.

### 5.3 Join and other operators
These are the `sq_core::plan::Op` variants (§10).

- **Scan.**
  - At `open`, compute the bound positions: constants, seeded slots, and graph register or graph slot.
  - If the planned order's prefix is satisfied, use `rebind`. Otherwise (Substitute-mode seeds vary per row), pick `IndexOrder::candidates(bound)[0]` and re-lease.
  - Map free columns to slots.
  - Post-check repeated variables (`?x :p ?x`), INT tags when `range` is set, and the named-graph `among` set.
  - A constant that is a TEMP id returns 0 rows without touching SQLite.
  - The **IntRange** variant streams the INT range on GPOS, then `rdf_gpox` for the same `(g,p)`. Its output is unsorted.
  - **Default graph resolution.**
    - `DefaultGraph::StoreDefault`: g=0.
    - `Graphs([g1..gk])`, k > 1: a k-way merge of k cursors with the same order and an adjacent dedupe of suffix columns (RDF merge is a set).
    - `UnionAll`: a 2-way merge of the g=0 g-first cursor and the partner g-last partial cursor (GSPO↔SPOG, GPSO↔PSOG, GPOS↔POSG, GOSP↔OSPG; same suffix order), with g projected away and an adjacent dedupe.
    - `GraphSel::Slot` with `NamedGraphs::All`: a partial g-last cursor.
    - With `Only(list)` and |list| ≤ 8: k g-first cursors, each tagging g. Otherwise the partial cursor plus membership in the list.
- **BindJoin** (index nested loop, sideways passing).
  1. Pull a left batch.
  2. If `sort_left`, sort it by the seed tuple (about 20 µs per 1024 rows).
  3. For each **distinct** seed tuple, re-open `right` with SeedMode::Join. For a Scan this is a `rebind`. Runs of equal keys reuse the previous probe's buffered result (3.4× on repeated keys).
  4. Merge results into each matching left row.
  5. For `kind=Left{filter}`, evaluate the filter on the merged row and emit the left row alone if nothing passes. Semi and Anti are supported.
  6. If `adaptive` and actual left rows exceed both 8× `est_rows` and 50,000, buffer the rest of the left input and switch to a HashJoin against one full evaluation of `right`.
- **HashJoin.**
  - Build into a flat arena plus `FxHashMap<key, u32 chain-head>`. Single-slot keys hash the raw i64.
  - Keys are the certainly-bound shared slots. Maybe-bound shared slots are checked on probe through a `CompatTable`: rows are grouped by the bitmask of bound shared slots, and per-(group, mask) indexes are built lazily.
  - With no certain keys, it falls back to NestedLoop with compatibility checks.
  - Kinds: Inner, Left{filter} (build right), Semi, Anti.
- **MergeJoin.**
  - Both inputs are sorted ascending (signed) by `key`, which is certainly bound on both sides. The right-hand run of equal keys is buffered for many-to-many joins.
  - When the right input is a Scan, it seeks to the left key after `gallop` consecutive misses: 6 on the SQL path, 10 with the direct B-tree reader (DBR).
  - `fast_count` returns Σ over runs of (left run × right run), unless `check` is non-empty.
- **StarMerge.** Leapfrog over k ≥ 3 s-sorted inputs. The planner chooses it only when its cost beats chained MergeJoins; a badly used leapfrog measured 4.3 µs per output row.
- **NestedLoop.** Cross products, or no certain shared slot.
- **Minus.** The right side is evaluated independently. A left row is removed iff some right row is compatible **and** shares at least one bound slot with it. Domains come from the actual bound slots of the rows, and in Substitute mode seeded slots are excluded from the intersection. If the static shared-variable set is empty, the planner emits just `left`.
- **Union**, **Filter**, **Extend** (error → slot left unbound; canonical flag per §6.6), **Values** (filter-compatible-and-merge with a seed), **Project** (resets non-projected slots), **Distinct** (FxHashSet specialised for 1, 2 and n slots), **Reduced** (adjacent dedupe), **Slice** (early stop; ASK = limit 1).
- **Sort** (stable, SortKeys precomputed once per row; when every key value is an inline INT it compares raw payloads), **TopK** (bounded heap with k = offset + limit), **SortBySlot** (enforcer: sort by raw i64 of one slot).
- **Group.**
  - Hash aggregation keyed by `SmallVec<[TermId; 2]>`, or streaming when the input is sorted by the single key.
  - An erroring key expression evaluates to `TermId::ERROR`, which forms its own group and is output as UNBOUND.
  - DISTINCT aggregates keep a per-group `FxHashSet<TermId>`.
- **Count.** Calls `input.fast_count()` and otherwise iterates.
- **GraphNames.** One row per named graph of the dataset.
- **GraphLoop.** For each named graph g of the dataset (in Substitute mode with the slot seeded: only that value, if it is a named graph):
  1. Push g on the register stack.
  2. Re-open `input`.
  3. Drop rows whose `slot` is bound to a different value, then set `slot = g`.
  4. Pop g.
- **SqlJoin.** `store.sql_rows(sql, params, out.len())`, then maps columns to slots. It uses no seeds; it only appears as a leaf.

### 5.4 Algebra to operators
| spargebra 0.4.7 node | Logical (lowering) | Physical |
|---|---|---|
| `Bgp{patterns}` | `Bgp(QuadPat…)`; bnodes become hidden slots; constants become ids/TEMP | a join group: Scan / BindJoin / MergeJoin / StarMerge / HashJoin / SqlJoin (§7) |
| `Path{subject,path,object}` | `Path` (top-level `a/b` was already turned into a BGP by spargebra) | `Path` operator (§5.5) |
| `Join` | `Join` | flattened into the parent join group |
| `LeftJoin{expression}` | `LeftJoin{filter}` | BindJoin{Left} if seeds ⊆ safe_inject(right), else HashJoin{Left} |
| `Filter` | `Filter` | pushed down (§7.3); remainder as Filter; `[NOT] EXISTS` → Semi/Anti or correlated |
| `Union` | `Union` | Union |
| `Graph{name}` | `Graph{name, input}`, inner patterns use `GraphSel::Active` | constant: membership check (absent → Empty) and inner Active → Const; variable: push or GraphLoop (§7.2) |
| `Extend` | `Extend` | Extend |
| `Minus` | `Minus` | Minus (no static shared variables → left) |
| `Values` | `Values` (UNDEF = 0) | Values |
| `OrderBy` | `OrderBy` | Sort, or TopK under Slice (through Project, **never** through Distinct) |
| `Project` | `Project`; a non-root Project opens a sub-select scope: unprojected inner variables are alpha-renamed to fresh slots | Project |
| `Distinct`/`Reduced` | same | Distinct / Reduced |
| `Slice` | same | Slice |
| `Group{variables,aggregates}` | `Group{keys, aggs}` (spargebra lowers GROUP BY expressions to Extend) | Group / Count |
| `Service{silent}` | `Service{silent}` | Error::Unsupported; SILENT → Unit |
| `Lateral` | absent (sep-0006 off) | – |
| `Expression::Exists` | `Expr::Exists(id)`, `exists[id]` lowered in the same scope | correlated ExistsEval (memoized) or decorrelated Semi/Anti |
| Dataset FROM / FROM NAMED | `DatasetSpec` (§5.6) | Scan graph resolution (§5.3) |
| ASK | `QueryForm::Ask` | root under Slice{limit 1} |
| CONSTRUCT | `QueryForm::Construct(template)` | §5.7 |
| DESCRIBE | `QueryForm::Describe(slots)` (spargebra binds IRIs via Extend) | §5.7 |

### 5.5 Property paths (`Op::Path`)
`PathPlan` is evaluated by `step(frontier, path, dir) -> pairs`:
- `Pred(p)`: a batched probe of the sorted, deduplicated frontier. Forward uses GSPO `(g,s,p)`; reverse uses GOSP `(g,o)` filtered on p, or GPOS `(g,p,o)`.
- `Inv` flips direction. `Seq` is composition. `Alt` is union, with bag semantics at the top level.
- `Nps(set)` scans `(g,s)` and drops `p ∈ set`.

For `*`, `+` and `?`, evaluation is a BFS by level with a per-start `FxHashSet` of visited nodes, so it terminates on cycles and gives set semantics per (start, end).
- **Expansion strategy per level.** Use sorted-frontier probes while `|frontier| × seek_sorted < |edges(p)| × scan_row`; otherwise do one GPSO(p) scan semi-joined with the frontier hash set.
- **Direction.**
  - Bound subject: forward.
  - Bound object: backward over the inverse path.
  - Both bound: forward with early exit.
  - Both free: one BFS per distinct start (the subjects of the first step).
- **Zero-length** (for `*` and `?`).
  - A **constant** endpoint matches itself even when it is absent from the graph (`zero_or_one_set_start` expects `:o`).
  - An endpoint bound by a **Join-mode seed** must satisfy `has_node(active graphs, x)`: a capped count of GSPO `(g,s=x)` or GOSP `(g,o=x)` is non-zero (`values_and_path` expects no rows).
  - In **Substitute mode** the seeded value counts as a constant.
  - With both endpoints free, every node of the active graph matches: distinct subjects through GSPO and distinct objects through GOSP, found by loose index scans.
  - `?x p* ?x` binds every node; `?x p+ ?x` requires a cycle.
- **Graphs.** Paths never cross graphs. With `GraphSel::Slot` the operator loops over the named graphs; with `Active` it uses the register; with a multi-graph default graph, edges come from the merged scan.

### 5.6 Datasets
- **No dataset clause.** The default graph is `StoreDefault`, or `UnionAll` when `union_default_graph=1`. Named graphs are `All` (registry ∪ distinct g≠0).
- **FROM only.** `default = Graphs(FROM)`, `named = Only([])`.
- **FROM NAMED only.** `default = Graphs([])` (an empty graph), `named = Only(list)`.
- IRIs that are not stored become TEMP ids, so they are empty graphs; the engine never fetches URLs.
- `GRAPH ?g` never binds the default graph.
- Update `USING` / `USING NAMED` / `WITH` arrive as `QueryDataset` (spargebra already pushes `WITH` into the templates and sets `using = {default: [with], named: None}`, where None means All) and map to the same `DatasetSpec`.

### 5.7 CONSTRUCT, DESCRIBE and Update (sq-exec / sq-engine)
- **CONSTRUCT.**
  - For each solution, instantiate the template.
  - Template blank nodes become fresh TEMP bnodes per (solution, label).
  - Skip triples that have an unbound position, a literal subject, or a predicate that is not an IRI.
  - Output is a set, kept in first-seen order with an `FxHashSet<[TermId;3]>`. Slots used in the template are canonical.
- **DESCRIBE.** The concise bounded description in the dataset's default graph: all triples with the resource as subject, recursing through blank-node objects.
- **Update** (`sq_engine::update`). The whole request runs in **one** write scope, created by the caller in sq-api. Operations run in order, and each one sees the previous effects.
  - `InsertData`: bnode labels map to `fresh_bnode()` per operation; terms are interned; quads are inserted; graphs are registered.
  - `DeleteData`: lookup only; quads with unknown terms are skipped.
  - `DeleteInsert`:
    1. Plan WHERE with the USING/WITH dataset (`sq_plan::lower_pattern`).
    2. Run it to completion and **materialize** all solutions first. This avoids the Halloween problem, which was reproduced in the experiments.
    3. Instantiate DELETE templates, lookup only, and skip unbound or unknown positions.
    4. Instantiate INSERT templates with fresh bnodes per (solution, label). TEMP values are resolved to terms and then `intern`ed. A graph variable must be bound to an IRI.
    5. `remove(deletes)`, then `insert(inserts)`.
  - `Load`: only `file://` IRIs and only when `allow_files=1`; otherwise an error. SILENT suppresses errors.
  - `Clear`, `Create` and `Drop` follow the spec, including the SILENT rules. A graph "exists" iff it is in `named_graphs()`. `DROP DEFAULT` = `CLEAR DEFAULT`. `DROP ALL` clears everything and empties the registry.
  - ADD, MOVE and COPY arrive already lowered by spargebra.
- `NOW()` is captured once per query or update request.

---

## 6. Expression evaluation and value model (sq-expr)

### 6.1 API (stubbed on day 0)
```rust
pub enum Value {                               // decoded value (cached per TermId in ValueCache)
    Iri(Arc<str>), BNode(Arc<str>),
    Str(Arc<str>), LangStr { lex: Arc<str>, lang: Arc<str> },
    Boolean(bool), Integer(i64), Decimal(oxsdatatypes::Decimal), Float(oxsdatatypes::Float), Double(oxsdatatypes::Double),
    DateTime(oxsdatatypes::DateTime), Date(oxsdatatypes::Date), Time(oxsdatatypes::Time),
    GYear(oxsdatatypes::GYear), GYearMonth(oxsdatatypes::GYearMonth), GMonth(oxsdatatypes::GMonth),
    GMonthDay(oxsdatatypes::GMonthDay), GDay(oxsdatatypes::GDay),
    Duration(oxsdatatypes::Duration), YearMonthDuration(oxsdatatypes::YearMonthDuration),
    DayTimeDuration(oxsdatatypes::DayTimeDuration),
    Other { lex: Arc<str>, datatype: Arc<str> },      // unknown datatype
    IllTyped { lex: Arc<str>, datatype: Arc<str> },   // known XSD datatype, invalid lexical (incl. facets, e.g. "300"^^xsd:byte)
}
pub enum Val { Id(TermId), V(Value) }          // lazy: Id keeps lexical form for STR/DATATYPE/sameTerm
pub struct ValueCache { /* FxHashMap<TermId, Value>, per execution */ }
pub struct EvalCtx<'a> {
    pub resolver: &'a mut dyn sq_core::eval_api::TermResolver,
    pub exists: &'a mut dyn sq_core::eval_api::ExistsEval,
    pub values: &'a mut ValueCache,
    pub now: oxsdatatypes::DateTime,
    pub base_iri: Option<&'a str>,
    pub rng: &'a mut dyn rand::RngCore,
}
pub struct CompiledExpr { /* tree with pre-encoded/pre-parsed constants, compiled regexes, fast paths */ }
pub fn compile(e: &sq_core::ir::Expr, temps: &sq_core::eval_api::TempDict) -> sq_core::Result<CompiledExpr>;
impl CompiledExpr {
    /// Ok(None) = SPARQL error or unbound; Err only for fatal errors (I/O, interrupt, memory).
    pub fn eval(&self, row: &[TermId], cx: &mut EvalCtx, canonical: bool) -> sq_core::Result<Option<TermId>>;
    pub fn ebv(&self, row: &[TermId], cx: &mut EvalCtx) -> sq_core::Result<Option<bool>>;
}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)] pub struct SortKey(/* see 6.5 */);
pub fn sort_key(id: TermId, cx: &mut EvalCtx) -> sq_core::Result<SortKey>;   // UNBOUND/ERROR -> lowest
pub trait Accumulator { fn add(&mut self, v: Option<TermId>, cx: &mut EvalCtx) -> sq_core::Result<()>;
                        fn finish(&mut self, cx: &mut EvalCtx) -> sq_core::Result<Option<TermId>>; }
pub fn accumulator(spec: &sq_core::ir::AggSpec) -> Box<dyn Accumulator>;
```
**Fast paths** are compiled from constants:
- inline INT against an inline constant: 2 compares;
- `IdEq(slot, id)` for IRI and simple-literal constants;
- `TagIs` for isIRI, isBlank and isLiteral (TEMP ids use `temp_tag`);
- `Bound`;
- `LangEq` (reads only the cached lang);
- REGEX with a constant pattern is compiled once, and a constant literal prefix is checked before the regex runs.

### 6.2 Numerics and errors
- Derived integer types (`int`, `long`, `short`, `byte`, `(non)negative`/`positiveInteger`, `unsigned*`) are Integer, with facet checks; an out-of-range lexical form is IllTyped.
- Promotion order is integer → decimal → float → double, and the result takes the promoted primitive type.
  - `integer / integer` gives a decimal.
  - Integer and decimal overflow, and division by zero, are errors. Float and double give ±INF or NaN.
  - `xsd:integer` beyond i64 is IllTyped (documented divergence).
- `ABS`, `CEIL`, `FLOOR` and `ROUND` keep the type. `ROUND` rounds half toward +∞.
- **Errors** are `None` and propagate, with these exceptions:
  - `||` and `&&` use three-valued logic: E‖T=T, E‖F=E, E&&F=F, E&&T=E.
  - `COALESCE` returns the first non-error value.
  - `IF` is an error if the condition is an error.
  - `BOUND` and `EXISTS` never error.
  - `IN` / `NOT IN` are an OR / AND chain of `=` / `!=`, giving an error only if nothing matched and some comparison errored.
  - Effect: in FILTER an error is false; in BIND, SELECT expressions and aggregate arguments the slot is unbound; in ORDER BY the key counts as unbound.

### 6.3 `=` / `!=` matrix (checked against open-eq-07/08: 12 rows for `=`, 42 for `!=`)
1. **Same id → true.** Exception: a TYPED double or float whose value is NaN gives `=` false and `!=` true. NaN can only be TYPED, so other tags skip the decode.
2. Either side is an IRI or blank node → false.
3. `Other` or `IllTyped` vs a language literal → false. vs anything else (including each other) → **error**.
4. Lang vs lang: equal lexical forms **and** equal lower-case tags. Lang vs any other known class → false.
5. Same known class (numeric, string, boolean, dateTime, date, time, g*, duration family) → value equality. Date/time with indeterminate timezone (±14 h) → error.
6. Different known classes → false.

`!=` is the negation of `=` with errors preserved. A filter `?x = c` may be rewritten to sameTerm/seed only when c is an IRI or a simple literal (§7.3).

### 6.4 EBV and ordering operators
- **EBV.** boolean → itself; IllTyped boolean or numeric → **false**; simple or xsd:string → non-empty; numeric → non-zero and not NaN. Lang literals, IRIs, blank nodes, Other and dates → error.
- **`<` and friends** are defined for numeric×numeric (with promotion), simple/xsd:string × simple/xsd:string (code-point order), boolean, dateTime, date, time, g* (XSD partial order; indeterminate → error), and yearMonthDuration/dayTimeDuration. Everything else is an error.

### 6.5 ORDER BY / MIN / MAX: precomputed `SortKey`, total by construction
`SortKey` is a lexicographic tuple, so `Ord` is total and Rust 1.81+ `sort_by` cannot panic:
```
(class, primary, lex, datatype, lang)
class:  0 unbound/error < 1 bnode < 2 IRI < 3 literal
literal family (inside primary, first component):
  0 numeric  primary = (nan_flag(NaN first), f64 via total_cmp, rank{integer|decimal:0, float:1, double:2}, exact i128 decimal (0 for float/double))
  1 boolean  primary = bool
  2 simple / xsd:string  primary = code points (= UTF-8 bytes order)
  3 lang string          primary = (lex, lang)
  4 dateTime  5 date  6 time  7 gYearMonth 8 gYear 9 gMonthDay 10 gMonth 11 gDay
             primary = UTC instant (no timezone keyed as UTC), Decimal seconds
  12 duration family     primary = (months, seconds)
  13 Other / IllTyped    primary = none (falls through to (lex, datatype))
```
Within a family the order agrees with `<` wherever `<` is defined. Numeric ties are broken by rank, then lexical form, then datatype, so `1`, `01` and `1.0` tie on value and keep a deterministic order. DESC reverses the order. The sort is stable. MIN and MAX use the same order and never error on mixed types.

### 6.6 Strings, language tags, dates, other builtins
- **String functions with language tags.**
  - `STRLEN`, `SUBSTR` (fn:substring rounding, 1-based) and `CONTAINS`/`STRSTARTS`/`STRENDS` work on Unicode code points.
  - Two-argument functions accept (simple|xsd:string, simple|xsd:string), (lang L, lang L) and (lang, simple); anything else is an error.
  - `UCASE`, `LCASE`, `SUBSTR` and `REPLACE` keep arg1's tag.
  - **`STRBEFORE`/`STRAFTER` keep arg1's tag on a match**, including a match at position 0 or with an empty pattern (`STRBEFORE("abc"@en,"")` gives `""@en`). They return a tagless `""` only when there is no match.
  - `CONCAT` keeps a tag only if every argument has the same tag.
  - `LANGMATCHES` follows RFC 4647 basic filtering, with `*` matching any non-empty tag.
- **REGEX / REPLACE** use the `regex` crate.
  - Flags map as `i`→case-insensitive, `s`→dot-all, `m`→multi-line, `x`→ignore whitespace, `q`→literal.
  - XPath-only constructs (`\i`, `\c`, class subtraction) are errors.
  - A REPLACE pattern that matches the empty string is an error; `$1` becomes `${1}`.
  - Compiled regexes are cached per (pattern, flags).
- **Dates.** `YEAR`, `MONTH`, `DAY`, `HOURS`, `MINUTES`, `SECONDS` (a decimal), `TIMEZONE` (a dayTimeDuration; error if there is none), and `TZ` (`""`, `"Z"` or `±hh:mm`).
- **Other builtins.**
  - `STR` errors on blank nodes.
  - `LANG` returns `""` for non-lang literals.
  - `DATATYPE` returns rdf:langString for lang literals and a constant id for inline values (`inline_datatype`).
  - `IRI`/`URI` resolve against the base IRI with oxiri.
  - `BNODE()` is fresh per call; `BNODE(str)` is stable within one solution row.
  - `STRDT`/`STRLANG` require a simple-literal lexical form.
  - `UUID` returns `urn:uuid:` + v4; `STRUUID`; `RAND` returns a double.
  - `MD5` and `SHA*` accept simple/xsd:string only (`md-5`, `sha1`, `sha2`).
  - `ENCODE_FOR_URI` follows RFC 3986.
  - `isNumeric` decodes only TYPED ids.
  - Casts arrive as `Function::Custom(xsd:*)` and follow the XPath casting table (string → number trims whitespace; boolean → number gives 1/0; number → string uses the canonical form; dateTime ↔ date). Any other Custom function is an evaluation error.
- **Computed lexical forms** follow oxsdatatypes `Display`, which matches pyoxigraph: a decimal 2 prints as `"2"`, a double as Rust f64 formatting. The W3C runner's lenient-numeric pass absorbs the expected-file inconsistencies.

### 6.7 Aggregates
- `COUNT(*)` counts rows. `COUNT(expr)` counts rows where expr is bound and not an error.
- `SUM`, `AVG`, `MIN` and `MAX`: any error in the group makes **that aggregate** unbound, but the group row is still emitted (`agg-err-01`).
- `SUM` follows promotion. **`AVG` = SUM / COUNT with promotion**: integers and decimals give a decimal division; any float or double gives a double. For example, `AVG(1.5e0, 2.5e0)` is `"2"^^xsd:double`. An empty group gives `0` (integer).
- `MIN` and `MAX` use the SortKey order. `SAMPLE` returns the first non-error value.
- `GROUP_CONCAT` joins `STR()` values with the separator (default `" "`) and returns a **simple literal**.
- DISTINCT dedupes by canonical id, so argument slots are canonical.
- **No GROUP BY and no rows:** exactly one row (COUNT 0, SUM/AVG 0, GROUP_CONCAT `""`, MIN/MAX/SAMPLE unbound). **GROUP BY with no rows:** no rows. HAVING is a Filter above the Group.

---

## 7. Planner and cost model (sq-plan)

### 7.1 Pipeline
1. `parse_query(text, base)`: `SparqlParser::new().with_base_iri(..)?.parse_query(text)`, with the parsed-query LRU in sq-api.
2. `lower_query(&Query, &dyn QuadStore, &QueryOptions) -> LoweredQuery`:
   - assign slots; open sub-select scopes and alpha-rename;
   - convert pattern blank nodes into hidden slots (one per label per query scope);
   - resolve constants with `Dictionary::lookup`; a miss becomes a `TempDict` TEMP id;
   - map `GRAPH` to `Logical::Graph` with inner `GraphSel::Active`;
   - build the `DatasetSpec` (§5.6), `PathPlan` and `exists[]`.
3. `props()`: compute `vars`, `certain`, `maybe` and `safe_inject` per node (§7.2).
4. Rewrites (§7.3): GRAPH push or loop, filter placement, sameTerm/IRI substitution, EXISTS decorrelation, TopK, Count, IntRange, canonical flags, projection pushdown (needed slots).
5. Join-group optimization by DP (§7.5), then physical selection and post-passes (§7.6).
6. `validate()`: every slot is bound below its use, sort orders are satisfied, seeds ⊆ safe_inject, and no filter sits in a LeftJoin's right side or a Minus's right side. It runs in debug builds and in all tests.

### 7.2 Static properties
`vars(P)` is every slot mentioned. `evars(e)` = slots(e) ∪ for each `Exists(id)` in e: `vars(exists[id]) ∩ maybe(context)`, where the context is the input of the Filter/Extend/LeftJoin that holds e.

**certain / maybe**

| Node | certain | maybe |
|---|---|---|
| Bgp, Path | all var slots (incl. graph slot) | same |
| Values | slots bound in every row | all slots |
| Join(A,B) | cA ∪ cB | mA ∪ mB |
| LeftJoin(A,B) | cA | mA ∪ mB |
| Minus(A,B) | cA | mA |
| Union(A,B) | cA ∩ cB | mA ∪ mB |
| Filter, OrderBy, Distinct, Reduced, Slice | input | input |
| Extend(P,v,e) | cP (∪ {v} iff e is a constant) | mP ∪ {v} |
| Graph(?g,P) | cP ∪ {g} | mP ∪ {g} |
| Group | COUNT outputs | keys ∪ aggregate slots |
| Project(S) | c ∩ S | m ∩ S |
| Unit, Empty, Service | ∅ | ∅ |

**safe_inject** is the set of slots v such that evaluating P with a Join-mode seed on v equals `{μ' ∈ eval(P) | μ' compatible with seed} ⋈ seed`. A slot not in vars(P) is always safe (P ignores it and the rows are merged).

| Node | safe_inject |
|---|---|
| Bgp, Values | all vars |
| Path | endpoint slots + graph slot (exact because of the `has_node` rule) |
| Union(A,B) | v safe in every branch that mentions v |
| Join(A,B) | v safe in every child that mentions v |
| **LeftJoin(L,R,f)** | **safe(L) ∩ (certain(L) ∪ ¬(vars(R) ∪ evars(f)))**. The seed goes to L only. (This fixes correctness's ∪/∩ typo.) |
| Minus(L,R) | safe(L) ∩ (certain(L) ∪ ¬vars(R)) |
| Filter(P,e) | safe(P) ∩ (certain(P) ∪ ¬evars(e)) |
| Extend(P,v,e) | (safe(P) ∩ (certain(P) ∪ ¬evars(e))) ∖ {v} |
| Project(S) | S ∩ safe(inner) |
| Distinct, Reduced | safe(P) ∩ certain(P) |
| OrderBy | safe(inner) |
| Group(keys) | keys ∩ safe(inner) ∩ certain(inner) |
| Graph (GraphLoop), Slice, Service | ∅ |

**GRAPH ?g push rule.** `Graph(?g, P)` is rewritten to P′, where every `Active` in P's immediate scope becomes `GraphSel::Slot(g)`, **iff** all of the following hold:
- P contains only Bgp, Path, Join, LeftJoin, Union, Filter, Extend, Values and nested Graph nodes;
- no Filter, Extend or LeftJoin expression mentions ?g or contains EXISTS;
- no Extend targets ?g;
- **?g ∈ certain(P′)**.

Otherwise the result is `GraphLoop{g, P}` with `Active` kept. Consequences:
- `GRAPH ?g { VALUES … }`, `GRAPH ?g { BIND … }` and `GRAPH ?g {}` (→ GraphNames) bind ?g correctly (graph-no-var, graph-from-named-1/2, bindings/graph).
- MINUS, GROUP BY, LIMIT and sub-select inside GRAPH ?g are evaluated per graph (graph-minus, agg-empty-group-count-graph, sq15, sq16).

**GRAPH <c>.** If c ∉ the dataset's named graphs, the result is Empty, even when there are no patterns inside (graph-from-named-3). Otherwise inner `Active` becomes `Const(c)`.

### 7.3 Rewrites
1. Flatten Join/Bgp into n-ary join groups. The units are triple patterns, paths, Values and opaque subtrees (LeftJoin, Union, Minus, sub-selects, GraphLoop, Filter-wrapped groups). **Filters are never hoisted up or sideways out of their group** (filter-scope tests).
2. **Filter placement.** Split conjunctions, then push each conjunct c to the lowest node whose `certain ⊇ evars(c)`:
   - through Join (to the child, or into the DP as soon as the binding set is complete);
   - into **every** Union branch (always legal);
   - through Extend when its slot ∉ evars(c);
   - through Project when evars(c) ⊆ projected slots;
   - through Distinct, Reduced and OrderBy;
   - into Graph when c doesn't mention ?g;
   - through Group when evars(c) ⊆ keys.

   Filters are **never** pushed into the right side of a LeftJoin (except its own expression) or of a Minus, never across Slice, and never to a point where any variable is only maybe-bound. `FILTER(!bound(?x))` therefore stays above its OPTIONAL. LeftJoin filter conjuncts with `evars ⊆ certain(R)` move into R.
3. `FILTER(sameTerm(?x, c))`, and `FILTER(?x = c)` where c is an IRI or a simple literal, with ?x ∈ certain(P), become a seed constant: `Join(Values{?x:[c]}, P)`.
4. `FILTER [NOT] EXISTS {Q}` as a whole conjunct becomes a Semi (Anti) HashJoin on the shared slots S = vars(Q) ∩ maybe(input), **iff** S ⊆ certain(input), S ⊆ safe_inject(Q), and Q has no zero-length-capable path endpoint in S. Otherwise it stays correlated (`Expr::Exists`), memoized per S-tuple.
5. `Slice(OrderBy(X))` and `Slice(Project(OrderBy(X)))` become TopK(k = offset+limit), then Slice(offset). **Never across Distinct.**
6. `Group{keys:[], aggs:[(o, CountAll, distinct=false)]}` becomes `Count{input, out:o}`. Scan inputs are eligible for VDBE counting when there are no repeated variables, no range, and the graph is single (StoreDefault, Const, or Slot with `NamedGraphs::All`).
7. **IntRange.**
   - Applies to a conjunct `?o op c` or a conjunction of two such (op ∈ <, ≤, >, ≥; c numeric) where ?o is the object slot of one pattern with constant g and p in the same group.
   - Convert the bounds to an integer range: `> 90.5` gives lo=91; `> 90` gives 91; `≥ 90` gives 90.
   - Use `Scan{order: Gpos, range}`, which reads the INT range plus `rdf_gpox(g,p)`. The original FILTER stays (so the plan is exact). The cost model decides whether to take it.
8. **Canonical flags.** `Extend.canonical = true` unless the slot provably flows only to final projection or into expressions. Consumers that force canonical: join keys/check/seeds, Distinct/Reduced, Group keys, aggregate DISTINCT arguments, Minus, Values joins, SameTerm, EXISTS correlation, CONSTRUCT/DESCRIBE templates, and Scans that reference the slot.
9. **Projection pushdown.** The needed-slot set shrinks scan column use and decode work.

### 7.4 Cardinality
Statistics are hints only.
1. **Runtime bounded counts.** A pattern with at least one constant besides the predicate (or with g constant and no stats) gets `store.count(spec, Some(cap))`, with cap = `probe_count_cap` (1000) and at most `max_probe_counts` (32) per query. That is at most 56 µs each. The result is exact if ≤ cap; otherwise stats take over with a floor of cap+1. With no stats at all, use cap 10,000 and extrapolate 10× when exceeded.
2. **Heavy hitters.** `rdf_stat_po(p,o)` gives an exact count for `?x :p :o` (for example, `?x a :C3` → 20,000).
3. **Per-predicate stats.** `(?s p ?o)` → n(p); `(?s p c)` → n(p)/dobj(p); `(c p ?o)` → n(p)/ds(p); unbound p → global totals.
4. **Characteristic sets** for subject stars with constant predicates P: `est = Σ_{cs ⊇ P} subjects(cs) × Π_{p∈P} occ(cs,p)/subjects(cs)`. A bound object scales this by n(p,o)/n(p). Mass outside the stored CSs falls back to (3).
5. **Joins.** `|L|·|R| / max(dv_L(v), dv_R(v))` per shared slot, with dv from ds/dobj by position, capped by the inputs. Filters: `=` const → 1/dv; int range → uniform over imin..imax; otherwise 1/3; regex/contains → 0.25; bound → 0.9; EXISTS / NOT EXISTS → 0.5. OPTIONAL: `max(|L|, |L⋈R|)`. Union: sum. Group: `min(|in|, Π dv(keys))`. Values: its row count.

### 7.5 Costs and join ordering
`CostConstants` (§10, `stats.rs`) are selected by `store.costs()`: SQL cursor, B-tree reader, or memory. Per (left plan L, right unit R):
- **BindJoin**: `distinct_keys(L) × (sorted ? seek_sorted : seek_random) + out × (scan_row + free_cols × scan_col) + |L| × row_copy`. `sort_left` is chosen when |L| > 64 and L's order isn't needed above. Requires seed ⊆ safe_inject(R). If L∪R is a pure chain of quad patterns ("sqlable": default or constant graph, no Values/paths/filters inside) and `caps().sql`, use `vdbe_probe` (550 ns) per probe instead of `seek_*`. Such chains become `SqlJoin` (below).
- **MergeJoin** (both sides sorted by v, each side either free or via a SortBySlot enforcer at `n·log2(n)·sort_row_log`): `inputs + min(|R| × scan_row, distinct(L) × seek_sorted + matched × scan_row) + out × row_copy`.
- **HashJoin** (build the smaller side): `|B| × hash_build(|B|) + |P| × hash_probe(|B|) + inputs`, where build and probe costs use the `_small` constants below 32K entries and `_large` above. This corrects engineering's 3–5× underestimate.
- **StarMerge** (k ≥ 3 inputs s-sorted on v): `Σ_i min(|R_i| × scan_row, |driver| × seek_sorted) + steps × 300 ns`. Taken only if cheaper than chained MergeJoins.
- **Opaque R** (LeftJoin subtree, Union, sub-select, Path, GraphLoop): BindJoin if seeds ⊆ safe_inject(R) and `distinct_keys(L) × cost(R | seeded) < cost(R) + hash costs`; otherwise HashJoin. No certain shared slot → NestedLoop.

**DP.** DPsub over connected subsets for up to 12 units, keeping the best plan **per interesting order** (`sorted_by ∈ {None} ∪ certain slots`). Above 12 units, greedy (cheapest connected next unit), with cross products last. `JoinPolicy` and `reorder=false` force shapes for differential testing.

Worked example, star-4 on the SQL path (`?x a :C3` with 20K rows, then `:age`, `:city`, `:name` at 200K each):
- INL: 3 × 20K × 700 ns = 42 ms.
- Merge: 3 × min(200K × 95, 20K × 700) = 42 ms.
- SqlJoin with the same order: 20K × (3 × 550 ns) + row costs ≈ 35 ms. **SqlJoin is chosen**, matching SQLite's 39 ms instead of losing at 50 ms.

Chain-2: MergeJoin with GPOS(knows) b-sorted and GPSO(knows) b-sorted is about 150 ms, versus SqlJoin about 360 ms and hash about 400 ms+. **Merge is chosen.**

### 7.6 Physical post-passes, SqlJoin rendering, explain
- **SqlJoin rendering.** A maximal left-deep chain of BindJoins over sqlable Scans becomes:
  ```sql
  SELECT t0.s, t1.o, t2.o FROM main.rdf_quad AS t0 INDEXED BY rdf_gpos
    CROSS JOIN main.rdf_quad AS t1 INDEXED BY rdf_gpso
    CROSS JOIN main.rdf_quad AS t2 INDEXED BY rdf_gpso
   WHERE t0.g=0 AND t0.p=?1 AND t0.o=?2 AND t1.g=0 AND t1.p=?3 AND t1.s=t0.s AND t2.g=0 AND t2.p=?4 AND t2.s=t0.s
  ```
  `CROSS JOIN` fixes **our** order and `INDEXED BY` fixes our access paths (verified: three SEARCH lines in this order). Constants are bound as parameters, and repeated variables become extra equalities. Only parameter-free SQL text is cached (LRU 256).
- **Interrupt and memory** checks are inserted by the executor, not the plan.
- **`explain(plan, profile, mode)`** prints pre-order lines with plan id, op, details, `est=` and `cost=`. `analyze` mode adds `rows=`, `opens=`, `seeks=` and `ms=` from `ExecProfile`. For example:
  ```
  #0 Project [?x ?n]                                    est=2.0e4
  #1  SqlJoin t0:Gpos(p=rdf:type,o=:C3) t1:Gpso(p=:age) t2:Gpso(p=:city) t3:Gpso(p=:name) est=2.0e4 cost=35ms
  #2 Count -> ?c  fast=merge-factorized                 est=1
  #3  MergeJoin key=?b gallop=6                         est=1.8e6
  #4   Scan Gpos p=:knows -> ?a ?b (sorted ?b)          est=6.0e5
  #5   Scan Gpso p=:knows -> ?b ?c (sorted ?b)          est=6.0e5
  ```

---

## 8. SQLite-facing API (sq-api)

### 8.1 Loading
```python
conn.enable_load_extension(True); conn.load_extension("ext/target/release/libsparqlite")  # -> sqlite3_sparqlite_init
```
`register(&Connection)` runs per connection; every pooled connection must load the extension. It creates one `Arc<ConnState>` and registers the functions, the 3 modules, and the close hook.

### 8.2 Scalar functions (exact)
All functions use `SQLITE_UTF8`. **None is DETERMINISTIC**, because they depend on the dictionary or the database. Writers and filesystem access are `SQLITE_DIRECTONLY`, so they cannot run from views, triggers or schema. Each nArg variant is registered separately.

| Function | Returns | Flags | Notes |
|---|---|---|---|
| `sparql_version()` | TEXT | | crate version + `sqlite_version()` |
| `sparql_init()` | INTEGER schema_version | D | idempotent. Loads and updates auto-init. Queries on an absent schema see an empty dataset. |
| `sparql_load(data TEXT\|BLOB [, format TEXT='turtle' [, base TEXT [, graph TEXT]]])` | INTEGER new quads | D | formats: `turtle`/`ttl`, `ntriples`/`nt`, `nquads`/`nq`, `trig`, `rdfxml`/`xml`, `n3`, `jsonld`. `graph` = target graph IRI (NULL means default). |
| `sparql_load_file(path [, format [, base [, graph]]])` | INTEGER | D | requires `sparql_config('allow_files', 1)`; format guessed from the extension |
| `sparql_query(query [, format TEXT='json'])` | TEXT | | SELECT/ASK: `json`, `xml`, `csv`, `tsv`. CONSTRUCT/DESCRIBE: `ntriples` (default), `nquads`, `turtle`. Engine per `sparql_config('engine')`. |
| `sparql_ask(query)` | INTEGER 0/1 | | |
| `sparql_count(query)` | INTEGER | | runs fully and counts solutions (or triples) with **no decode and no serialization** (benchmark primitive) |
| `sparql_value(query)` | native | | first projected variable of the first row (§8.4), NULL if none |
| `sparql_update(update)` | INTEGER inserted+deleted | D | one write scope per request |
| `sparql_explain(query [, mode TEXT='plan'])` | TEXT | | modes: `algebra` (spargebra SSE), `logical`, `plan`, `analyze` (executes), `sql` (baseline SQL), `sqlplan` (its EXPLAIN QUERY PLAN) |
| `sparql_analyze()` | TEXT JSON summary | D | |
| `sparql_config(key)` / `sparql_config(key, value)` | TEXT current value | 2-arg: D | §8.5 |
| `sparql_query_sql(query [, format])` | TEXT | | baseline translator (§9) |
| `sparql_count_sql(query)` | INTEGER | | `SELECT count(*) FROM (<sql>)` |
| `sparql_to_sql(query)` | TEXT | | the generated SQL |
| `sparql_term(id)` | TEXT N-Triples / NULL | | inline, bnode and dictionary ids; NULL for 0 and TEMP |
| `sparql_term_value(id)` | native | | §8.4 mapping |
| `sparql_term_id(nt TEXT)` | INTEGER / NULL | | parses one N-Triples term; lookup only, never allocates |
| `sparql_term_kind(id)`, `sparql_term_lex(id)`, `sparql_term_lang(id)`, `sparql_term_datatype(id)` | TEXT | | kind ∈ `iri`/`bnode`/`literal` |
| `sparql_num(id)`, `sparql_str(id)`, `sparql_ebv(id)`, `sparql_sortkey(id)` | INTEGER/REAL/NULL, TEXT/NULL, 0/1/NULL, BLOB | | baseline helpers. `sortkey` is a memcmp-ordered encoding of SortKey. |
| `sparql_vacuum_terms()` | INTEGER rows deleted | D | deletes unreferenced dictionary rows, bumps `dict_gen` |
| `sparql_check()` | TEXT | | invariants: no inline-eligible rows, no duplicate keys, no dangling ids, index/table count parity, registry ⊇ used graphs |

### 8.3 Virtual tables
**(a) `sparql_rows`** is an eponymous-only table-valued function (`Module::eponymous_only_module()`, `Aux = Arc<ConnState>`). Prototype verified.
```sql
CREATE TABLE x(n INTEGER,                                    -- col 0: solution ordinal (1-based)
  v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,v10,v11,v12,v13,v14,v15,     -- cols 1..16: native value of k-th projected var
  t0 TEXT, …, t15 TEXT,                                      -- cols 17..32: N-Triples text
  id0 INTEGER, …, id15 INTEGER,                              -- cols 33..48: TermId (NULL for unbound / TEMP)
  vars TEXT,                                                 -- col 49: JSON array of variable names
  json TEXT,                                                 -- col 50: SPARQL-JSON bindings object for the row
  query HIDDEN, opts HIDDEN)                                 -- cols 51, 52
```
- **Usage:** `SELECT v0, v1 FROM sparql_rows('SELECT ?s ?n WHERE {…}')`, `SELECT count(*) FROM sparql_rows(?1)`, and correlated `FROM jobs j, sparql_rows(j.q)`.
- **xBestIndex** requires a usable EQ constraint on col 51 (argv 1, omit). Without one it returns `Ok(false)`, so SQLite reports "no query solution". `opts` (col 52) is optional (argv 2). It records `info.col_used()` as hex in `idx_str`; verified: `count(*)` gives only bits 51 and 49 when `vars` is read. Estimates: 1000 rows, cost 1000.
- **xFilter** (inside the outer statement's read transaction, `txn_state == READ`, verified):
  1. Check the epoch.
  2. Parse (LRU) and plan.
  3. Create the `Execution` in streaming mode and increment `open_cursors`.
- **xNext** pulls the next 1024-row batch when needed and decodes **only** the columns set in col_used: `v_i`/`t_i` need the term, `id_i` needs nothing, `json` needs all projected terms. It writes them into `RefCell` row buffers, because `VTabCursor::column(&self, …)` is `&self`. **xClose** decrements `open_cursors`.
- **Query forms.** ASK gives one row with `v0` = 0/1. CONSTRUCT/DESCRIBE give one row per triple with `v0..v2` = s,p,o (and `t*`/`id*`). With more than 16 variables, use `json` or `sparql_view`.
- **Options** are a JSON object that overrides the connection config for this call: `engine`, `union_default_graph`, `timeout_ms`, `max_memory_mb`, `join_policy`, `reorder`.

**(b) `sparql_view`** is a `CreateVTab` (`read_only_module`, `KIND = Default`).
```sql
CREATE VIRTUAL TABLE temp.people USING sparql_view(
  'SELECT ?person ?name ?age WHERE { ?person <http://ex/name> ?name OPTIONAL { ?person <http://ex/age> ?age } }',
  engine=native, timeout_ms=2000);
-- declared: CREATE TABLE x("person","name","age","person__term" HIDDEN,"name__term" HIDDEN,"age__term" HIDDEN,
--                          "person__id" HIDDEN,"name__id" HIDDEN,"age__id" HIDDEN, opts HIDDEN)
SELECT name, age FROM people WHERE person__term = '<http://ex/alice>';
```
- **xCreate/xConnect** only parse:
  - Arg 0 (after the module, database and table names) is the query, dequoted with `''` → `'`.
  - The rest are `key=value` parsed with `rusqlite::vtab::parameter`.
  - Columns are the projected variables, double-quote escaped. `SELECT *` gives spargebra's in-scope order.
- **xBestIndex** consumes usable EQ constraints on `__term`/`__id` columns (argv, omit = true) and estimates 10 rows when one is present, 1000 otherwise.
- **xFilter** turns each consumed value into a **top-level `FILTER(sameTerm(?v, c))`** that the planner may turn into a seed only if ?v is certain. This keeps SQL semantics for unbound or NULL columns. A term absent from the store gives zero rows.
- Native-value columns are never pushed down, because TEXT `'x'` is ambiguous between `<x>`, `"x"` and `"x"@en`.
- The definition persists in the schema: use `temp.` for session views. Without the extension loaded, only that table fails.

**(c) `sparql_status`** is eponymous-only and read-only: `(key TEXT, value)`. Keys:
- `version`, `sqlite_version`, `schema_state`, `dict_gen`, `stats_gen`, `committed_max_row`;
- `decode_cache_entries`, `decode_cache_bytes`, `pool_statements`, `open_cursors`, `armed`;
- `config.*`.

It is also the **close hook** (§8.7).

A writable `rdf_quads` vtab is **out of scope for v1** (feasibility judge).

### 8.4 Native SQL value mapping (`v*` columns, `sparql_value`, `sparql_term_value`, `sparql_view`)
| Term | SQL value |
|---|---|
| IRI | TEXT without `<>` |
| blank node | `'_:label'` |
| simple, xsd:string, lang literal | TEXT (lexical; the tag is in `t*`/`__term`) |
| integer family | INTEGER, or TEXT if outside i64 |
| decimal, float, double | REAL |
| boolean | 0/1 |
| date/time types | TEXT (lexical ISO 8601, so SQLite date functions work) |
| other typed | TEXT (lexical) |
| unbound | NULL |

### 8.5 Per-connection configuration (`sparql_config`)
| key | default | values |
|---|---|---|
| `engine` | `native` | `native` \| `sql` |
| `timeout_ms` | 0 (none) | integer |
| `max_memory_mb` | 1024 | integer |
| `union_default_graph` | 0 | 0/1 |
| `decode_cache_mb` | 64 | integer |
| `join_policy` | `auto` | `auto` \| `hash` \| `bind` \| `merge` \| `nosql` |
| `reorder` | 1 | 0/1 |
| `probe_counts` / `probe_cap` | 32 / 1000 | integers |
| `allow_files` | 0 | 0/1. Only the DIRECTONLY 2-arg form can set it. |
| `reader` | `auto` | `auto` \| `sql` \| `btree` (phase 2; `auto` = sql in v1) |

### 8.6 Per-connection state, transactions, re-entrancy
```rust
pub struct ConnState {
    db: sq_sqlite::RawDb,                         // the connection's *mut sqlite3 (never closed by us)
    storage: Rc<sq_sqlite::StorageShared>,        // StmtPool, DecodeCache, LookupCache, stats, epoch, committed_max_row
    config: RefCell<ConnConfig>,
    parsed: RefCell<ParsedLru>,                   // (text, base) -> Arc<spargebra::Query>, 128 entries
    open_cursors: Cell<u32>,
    depth: Cell<u32>,                             // nesting depth -> savepoint names sq_r{d} / sq_w{d}
    last_report: RefCell<ChangeReport>,
}
// SAFETY: one ConnState per sqlite3*. It is only touched inside SQLite callbacks for that connection,
// which SQLite serializes (THREADSAFE=1 holds db->mutex; multi-thread mode forbids sharing a connection).
// Needed because rusqlite requires closures: Send + 'static and vtab Aux: Send + Sync.
unsafe impl Send for ConnState {}
unsafe impl Sync for ConnState {}
pub fn register(conn: &rusqlite::Connection) -> rusqlite::Result<()>;
```
- **ReadScope** (every scalar entry). If `sqlite3_txn_state(db, "main") == SQLITE_TXN_NONE` (a bare `SELECT f()`, verified), run `SAVEPOINT sq_r{depth}` and `RELEASE` it on drop, including on error. In `xFilter` the outer statement already holds a read transaction (verified), so nothing is opened. This turns 7–8 µs probes into about 1 µs probes and gives every internal statement the same snapshot.
- **WriteScope** (load, update, init, analyze, vacuum).
  1. Refuse with `Error::WriteWhileReading` if `open_cursors > 0`.
  2. `SAVEPOINT sq_w{depth}`, then `UPDATE main.rdf_meta SET value=value+1 WHERE key='write_gen'` to take the write lock first (busy_timeout applies).
  3. `RELEASE` on success; on error `ROLLBACK TO sq_w{depth}` + `RELEASE`, then return the error.

  This nests inside a user `BEGIN … COMMIT/ROLLBACK`, and it works from inside `SELECT sparql_update(...)` (verified in all three designs).
- **Re-entrancy rules** (enforced by review and tests):
  - **R1.** Never hold a `RefCell` borrow of ConnState or StorageShared across a call that steps a statement or returns to SQLite. Borrows are O(1) get/put. A violation surfaces as `try_borrow_mut` → `Error::Reentrant`, never as a deadlock. Use no `Mutex` anywhere: the baseline's generated SQL calls our helper UDFs while we step it.
  - **R2.** Our internal SQL touches only `rdf_*` tables, `sqlite_schema` and `pragma_*`. User SQL runs only as the baseline's generated SQL.
  - **R3.** Savepoint names include the depth.
  - **R4.** Every `extern "C"` path is panic-safe:
    - rusqlite catches panics in scalar functions;
    - **every vtab method body** (connect, best_index, open, filter, next, eof, column, rowid) is wrapped in `guard(|| …)` = `catch_unwind` → `SQLITE_ERROR`, because rusqlite's vtab trampolines do not catch, and a panic in xFilter aborted Python with exit 134;
    - `Drop` impls of vtabs catch too;
    - init is wrapped.
  - **R5.** Leases are reset on drop. Only a live streaming cursor keeps statements mid-step, until xClose.
  - **R6.** Interrupts: our statements return `SQLITE_INTERRUPT` and become `Error::Interrupted`. CPU loops poll the pooled `SELECT 1` every 4096 rows. `timeout_ms` is checked at the same points.
  - **R7.** Debug builds record the thread id on entry and assert there is no concurrent entry.
- **Halloween.** Our updates materialize WHERE first. Plain-SQL writes to `rdf_quad` during a `sparql_rows` iteration are documented as undefined.

### 8.7 Close hook (prevents zombie connections; verified)
`register()` creates the `sparql_status` module with `Aux = Arc<ConnState>`. It then runs `conn.prepare("SELECT 1 FROM sparql_status")` and drops the statement; this connects the eponymous instance, and xConnect **arms** the pool. `sqlite3_close_v2` calls xDisconnect on eponymous vtabs *before* its busy check. Our vtab `Drop` (wrapped in catch_unwind) disarms the pool and finalizes every idle statement, so the connection closes fully: the `-wal` file disappeared in the prototype.

The instance stays connected across DDL, other connections' DDL and VACUUM (verified). At each entry, if `armed == false`, re-run the prepare-and-drop to re-arm.

---

## 9. Baseline SPARQL→SQL translator (sq-sqlgen)

`translate(&LoweredQuery) -> Result<SqlQuery { sql, params: Vec<i64>, columns: Vec<String> }>`. It uses the same storage, ids, dictionary, read scope and output path (a `SolutionSource` over the rows), so the benchmark measures **evaluation strategy only**. There is no `INDEXED BY`; SQLite plans with `sqlite_stat1` from `ANALYZE` (no STAT4 in the host). The supported subset is:

- **BGP:** self-joins of `main.rdf_quad` aliases with `tN.g=0` (or the graph constant), constants as parameters, repeated variables as equalities.
- **FILTER:**
  - comparisons with an inline-int fast path, so the baseline is not a straw man: `((tN.o & 15) = 6 AND (tN.o >> 4) > 90 OR (tN.o & 15) <> 6 AND sparql_num(tN.o) > 90)`;
  - other numerics via `sparql_num`;
  - equality with an IRI or simple literal constant via ids;
  - `regex`, `contains` and `str*` via `sparql_str`;
  - `bound` via `IS NOT NULL`; logical operators.
- **OPTIONAL** → `LEFT JOIN (…) ON <compat> AND <filter>`, where compatibility on nullable columns is `(a.x IS NULL OR b.x IS NULL OR a.x=b.x)` and outputs use `COALESCE`.
- **UNION** → `UNION ALL` with NULL padding.
- **MINUS** → `NOT EXISTS`, with the shared-domain condition.
- **[NOT] EXISTS** → correlated `[NOT] EXISTS`.
- **VALUES** → CTE.
- **BIND** → helper-UDF expressions (subset).
- **DISTINCT, LIMIT/OFFSET.** ORDER BY uses `sparql_sortkey(id)`.
- **GROUP BY** with COUNT, SUM, AVG, MIN, MAX and SAMPLE via `sparql_num` / `sparql_sortkey`.
- **`p+` / `p*` with a constant p** → `WITH RECURSIVE … UNION` (set semantics).
- **GRAPH <c> / FROM** via `g` predicates.

Anything else returns `Error::Unsupported("baseline: …")`. The benchmark uses only queries that both engines support, and it cross-checks the results.

---

## 10. `sq-core`: complete source (FROZEN on day 0)

Verified on this machine: `cargo test -p sq-core` gives 6 passed, and `cargo clippy --all-targets` gives 0 warnings. The verified copy is at `/tmp/claude-0/-home-user-research/82148577-c93b-5a93-8757-e7fe894ecb5e/scratchpad/synth/ws/crates/sq-core`. After day 0, changes are **additive only** and need lead review.

### `crates/sq-core/Cargo.toml`
```toml
[package]
name = "sq-core"
edition.workspace = true
version.workspace = true

[features]
default = []
testing = []   # exposes sq_core::testing (cursor conformance suite, random data)

[dependencies]
oxrdf = { workspace = true }
spargebra = { workspace = true }
thiserror = { workspace = true }
rustc-hash = { workspace = true }
xxhash-rust = { workspace = true }
```

### `src/lib.rs`
```rust
//! sq-core: the frozen contract shared by every sparqlite crate.
//! No SQLite dependency. Changes after day 0 are additive-only and need lead review.
#![forbid(unsafe_code)]
#![allow(rustdoc::broken_intra_doc_links, rustdoc::invalid_html_tags)]

pub mod batch;
pub mod codec;
pub mod error;
pub mod eval_api;
pub mod id;
pub mod ir;
pub mod plan;
pub mod stats;
pub mod store;
pub mod term;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use batch::{Batch, Slot, SlotSet, BATCH_ROWS};
pub use error::{Error, Result};
pub use id::{Kind, Tag, TermId};
pub use term::{Literal, Term};

pub use oxrdf;
pub use spargebra;
pub use rustc_hash::{FxHashMap, FxHashSet};

#[cfg(test)]
mod tests;
```

### `src/error.rs`
```rust
/// Every fallible API in the workspace returns this error. Expression-level SPARQL
/// "type errors" are NOT `Error`s: they are `None` inside sq-expr and never abort a query.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("sparql: syntax error: {0}")]
    Syntax(String),
    #[error("sparql: unsupported: {0}")]
    Unsupported(String),
    #[error("sparql: SQLite error {code}: {msg}")]
    Sqlite { code: i32, msg: String },
    #[error("sparql: interrupted")]
    Interrupted,
    #[error("sparql: query timeout")]
    Timeout,
    #[error("sparql: memory budget exceeded ({0} MB)")]
    MemoryBudget(u64),
    #[error("sparql: RDF store schema missing or incompatible: {0}")]
    Schema(String),
    #[error("sparql: invalid argument: {0}")]
    Invalid(String),
    #[error("sparql: RDF parse error: {0}")]
    Parse(String),
    #[error("sparql: re-entrant use of connection state")]
    Reentrant,
    #[error("sparql: cannot modify the RDF store while a sparql_rows cursor is open")]
    WriteWhileReading,
    #[error("sparql: internal error: {0}")]
    Internal(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub fn internal(msg: impl Into<String>) -> Self {
        Error::Internal(msg.into())
    }
}
```

### `src/id.rs`
```rust
//! 64-bit term identifiers.
//!
//! ```text
//!  63                                                 4 3    0
//! +----------------------------------------------------+------+
//! |                 payload (60 bits)                  | tag  |
//! +----------------------------------------------------+------+
//! ```
//! The tag sits in the LOW bits so dictionary ids stay small SQLite varints
//! (row 500K -> id 8,000,017 -> 3 bytes on disk).
//! Ids are stored and compared as SIGNED i64, which is SQLite's integer order.
//! Merge joins, seeks and range scans must compare `i64`, never `u64`.
//!
//! INVARIANT (sameTerm <=> id equality, within one execution):
//!   * inline-encodable terms (`codec::encode_inline` is `Some`) have exactly that id and never
//!     get a dictionary row;
//!   * every other IRI / literal has exactly one `rdf_term` row r and id `(r << 4) | tag`;
//!   * blank nodes have counter ids `(n << 4) | 2` and no dictionary row;
//!   * a computed / query-constant term that is neither inline nor in the dictionary gets ONE
//!     TEMP id from the execution's `TempDict` (which probes the dictionary first when the
//!     value must be canonical).

/// Low 4-bit tag.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum Tag {
    /// Only `TermId(0)`: unbound slot; also `g = 0` = default graph in `rdf_quad`.
    Unbound = 0,
    /// Dictionary IRI.
    Iri = 1,
    /// Blank node: payload = counter (`rdf_meta.next_bnode`), no dictionary row.
    BNode = 2,
    /// Dictionary simple literal (xsd:string) that is not SHORTSTR-inlinable (> 7 UTF-8 bytes).
    Str = 3,
    /// Dictionary language-tagged literal (tag lower-cased by oxrdf).
    Lang = 4,
    /// Dictionary literal with any other datatype (incl. non-canonical numerics, doubles, floats,
    /// ill-typed lexical forms, unknown datatypes).
    Typed = 5,
    /// Inline canonical xsd:integer, payload = i60 value.
    Int = 6,
    /// Inline xsd:boolean "true"/"false".
    Bool = 7,
    /// Inline canonical xsd:decimal (see codec).
    Decimal = 8,
    /// Inline canonical xsd:dateTime (see codec).
    DateTime = 9,
    /// Inline canonical xsd:date (see codec).
    Date = 10,
    /// Inline simple literal of <= 7 UTF-8 bytes.
    ShortStr = 11,
    Reserved12 = 12,
    Reserved13 = 13,
    /// Execution-local term: payload = (index << 3) | would-be dictionary tag (1..=5).
    Temp = 14,
    /// Internal sentinel `TermId::ERROR` (error-valued GROUP BY key). Never stored or output.
    Error = 15,
}

const TAGS: [Tag; 16] = [
    Tag::Unbound,
    Tag::Iri,
    Tag::BNode,
    Tag::Str,
    Tag::Lang,
    Tag::Typed,
    Tag::Int,
    Tag::Bool,
    Tag::Decimal,
    Tag::DateTime,
    Tag::Date,
    Tag::ShortStr,
    Tag::Reserved12,
    Tag::Reserved13,
    Tag::Temp,
    Tag::Error,
];

/// Coarse RDF term kind, available without any lookup.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Kind {
    Unbound,
    Iri,
    BNode,
    Literal,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Debug)]
#[repr(transparent)]
pub struct TermId(pub i64);

impl TermId {
    pub const UNBOUND: TermId = TermId(0);
    pub const DEFAULT_GRAPH: TermId = TermId(0);
    /// Error-valued group key; mapped to UNBOUND on output.
    pub const ERROR: TermId = TermId(Tag::Error as i64);
    pub const INT_MIN: i64 = -(1 << 59);
    pub const INT_MAX: i64 = (1 << 59) - 1;

    #[inline]
    pub const fn raw(self) -> i64 {
        self.0
    }
    #[inline]
    pub const fn is_bound(self) -> bool {
        self.0 != 0
    }
    #[inline]
    pub const fn tag_bits(self) -> u8 {
        (self.0 & 0xF) as u8
    }
    #[inline]
    pub const fn tag(self) -> Tag {
        TAGS[(self.0 & 0xF) as usize]
    }
    /// Has an `rdf_term` row (tags 1, 3, 4, 5).
    #[inline]
    pub const fn is_dict(self) -> bool {
        matches!(self.0 & 0xF, 1 | 3 | 4 | 5) && self.0 > 0
    }
    /// Value is fully contained in the id (tags 6..=11).
    #[inline]
    pub const fn is_inline(self) -> bool {
        matches!(self.0 & 0xF, 6..=11)
    }
    #[inline]
    pub const fn is_temp(self) -> bool {
        self.0 & 0xF == Tag::Temp as i64
    }
    #[inline]
    pub const fn is_bnode(self) -> bool {
        self.0 & 0xF == Tag::BNode as i64
            || (self.is_temp() && (self.0 >> 4) & 0x7 == Tag::BNode as i64)
    }
    /// May this id be written to `rdf_quad`? (Not UNBOUND/TEMP/ERROR/reserved.)
    #[inline]
    pub const fn is_storable(self) -> bool {
        matches!(self.0 & 0xF, 1..=11) && self.0 != 0
    }
    /// `rdf_term.id` (row number) of a dictionary id.
    #[inline]
    pub const fn dict_row(self) -> i64 {
        self.0 >> 4
    }
    #[inline]
    pub const fn from_dict(row: i64, tag: Tag) -> TermId {
        TermId((row << 4) | tag as i64)
    }
    #[inline]
    pub const fn bnode(n: i64) -> TermId {
        TermId((n << 4) | Tag::BNode as i64)
    }
    #[inline]
    pub const fn bnode_counter(self) -> i64 {
        self.0 >> 4
    }
    #[inline]
    pub const fn from_int(v: i64) -> Option<TermId> {
        if v >= Self::INT_MIN && v <= Self::INT_MAX {
            Some(TermId((v << 4) | Tag::Int as i64))
        } else {
            None
        }
    }
    #[inline]
    pub const fn as_int(self) -> Option<i64> {
        if self.0 & 0xF == Tag::Int as i64 {
            Some(self.0 >> 4)
        } else {
            None
        }
    }
    #[inline]
    pub const fn from_bool(b: bool) -> TermId {
        TermId(((b as i64) << 4) | Tag::Bool as i64)
    }
    #[inline]
    pub const fn as_bool(self) -> Option<bool> {
        if self.0 & 0xF == Tag::Bool as i64 {
            Some(self.0 >> 4 != 0)
        } else {
            None
        }
    }
    /// TEMP id; `tag` is the dictionary tag the term would have (Iri/BNode/Str/Lang/Typed),
    /// so kind checks need no arena lookup.
    #[inline]
    pub const fn temp(index: u32, tag: Tag) -> TermId {
        TermId(((index as i64) << 7) | ((tag as i64) << 4) | Tag::Temp as i64)
    }
    #[inline]
    pub const fn temp_index(self) -> Option<u32> {
        if self.is_temp() {
            Some((self.0 >> 7) as u32)
        } else {
            None
        }
    }
    /// For TEMP ids: the dictionary tag the term would have.
    #[inline]
    pub const fn temp_tag(self) -> Tag {
        TAGS[((self.0 >> 4) & 0x7) as usize]
    }
    /// Kind without any lookup.
    #[inline]
    pub const fn kind(self) -> Kind {
        let t = if self.is_temp() { ((self.0 >> 4) & 0x7) as u8 } else { (self.0 & 0xF) as u8 };
        match t {
            0 => Kind::Unbound,
            1 => Kind::Iri,
            2 => Kind::BNode,
            _ => Kind::Literal,
        }
    }
    /// Raw id range covering inline ints in [lo, hi] (inclusive). Other tags interleave inside
    /// the range: callers re-check `tag() == Tag::Int`.
    #[inline]
    pub const fn int_raw_range(lo: i64, hi: i64) -> (i64, i64) {
        let lo = if lo < Self::INT_MIN { Self::INT_MIN } else { lo };
        let hi = if hi > Self::INT_MAX { Self::INT_MAX } else { hi };
        ((lo << 4) | Tag::Int as i64, (hi << 4) | Tag::Int as i64)
    }
}

impl From<i64> for TermId {
    fn from(v: i64) -> Self {
        TermId(v)
    }
}
```

### `src/term.rs`
```rust
//! Owned, cheaply clonable decoded terms (the dictionary / TempDict / output currency).
use std::sync::Arc;

use crate::codec::{XSD_STRING_IRI, RDF_LANG_STRING_IRI};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Term {
    Iri(Arc<str>),
    /// Label without "_:". Store blank nodes are "b<hex counter>", TEMP ones "t<index>".
    BNode(Arc<str>),
    Literal(Literal),
}

/// `datatype` is the full IRI. Simple literals have datatype xsd:string and `lang == None`;
/// language-tagged literals have datatype rdf:langString and a lower-case `lang`.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Literal {
    pub lex: Arc<str>,
    pub datatype: Arc<str>,
    pub lang: Option<Arc<str>>,
}

impl Literal {
    pub fn simple(lex: &str) -> Self {
        Literal { lex: lex.into(), datatype: XSD_STRING_IRI.into(), lang: None }
    }
    pub fn lang(lex: &str, lang: &str) -> Self {
        Literal { lex: lex.into(), datatype: RDF_LANG_STRING_IRI.into(), lang: Some(lang.to_ascii_lowercase().into()) }
    }
    pub fn typed(lex: &str, datatype: &str) -> Self {
        Literal { lex: lex.into(), datatype: datatype.into(), lang: None }
    }
    pub fn is_simple(&self) -> bool {
        self.lang.is_none() && &*self.datatype == XSD_STRING_IRI
    }
}

impl Term {
    pub fn from_oxrdf(t: oxrdf::TermRef<'_>) -> Term {
        #[allow(unreachable_patterns)]
        match t {
            oxrdf::TermRef::NamedNode(n) => Term::Iri(n.as_str().into()),
            oxrdf::TermRef::BlankNode(b) => Term::BNode(b.as_str().into()),
            oxrdf::TermRef::Literal(l) => Term::Literal(Literal {
                lex: l.value().into(),
                datatype: l.datatype().as_str().into(),
                lang: l.language().map(Into::into),
            }),
            _ => panic!("RDF 1.2 triple terms are not supported (oxrdf rdf-12 must stay disabled)"),
        }
    }

    pub fn to_oxrdf(&self) -> oxrdf::Term {
        match self {
            Term::Iri(i) => oxrdf::NamedNode::new_unchecked(i.to_string()).into(),
            Term::BNode(b) => oxrdf::BlankNode::new_unchecked(b.to_string()).into(),
            Term::Literal(l) => match &l.lang {
                Some(lang) => oxrdf::Literal::new_language_tagged_literal_unchecked(l.lex.to_string(), lang.to_string()).into(),
                None if &*l.datatype == XSD_STRING_IRI => oxrdf::Literal::new_simple_literal(l.lex.to_string()).into(),
                None => oxrdf::Literal::new_typed_literal(
                    l.lex.to_string(),
                    oxrdf::NamedNode::new_unchecked(l.datatype.to_string()),
                )
                .into(),
            },
        }
    }

    /// N-Triples serialization (the `t0..t15` columns, `sparql_term()`).
    pub fn to_ntriples(&self) -> String {
        self.to_oxrdf().to_string()
    }

    pub fn kind(&self) -> crate::Kind {
        match self {
            Term::Iri(_) => crate::Kind::Iri,
            Term::BNode(_) => crate::Kind::BNode,
            Term::Literal(_) => crate::Kind::Literal,
        }
    }
}
```

### `src/codec.rs`
```rust
//! Inline value codec, dictionary keys, stable term hash, well-known dictionary rows.
//!
//! The rules here are part of the ON-DISK FORMAT (`rdf_meta.encoding_version = 1`).
//! `encode_inline` is a pure function of (lexical form, datatype, language): a term is inlined
//! iff our formatter reproduces its lexical form EXACTLY (`format(decode(x)) == lexical`).
//! Non-canonical forms ("01", "+1", "1.50", "2020-01-01T00:00:00.000Z", "-00:00") keep their
//! own identity as dictionary TYPED rows, so sameTerm stays exact and lexical forms round-trip.
use std::sync::Arc;

use crate::id::{Tag, TermId};
use crate::term::{Literal, Term};

pub const ENCODING_VERSION: i64 = 1;
pub const SCHEMA_VERSION: i64 = 1;

pub const XSD: &str = "http://www.w3.org/2001/XMLSchema#";
pub const XSD_STRING_IRI: &str = "http://www.w3.org/2001/XMLSchema#string";
pub const XSD_INTEGER_IRI: &str = "http://www.w3.org/2001/XMLSchema#integer";
pub const XSD_DECIMAL_IRI: &str = "http://www.w3.org/2001/XMLSchema#decimal";
pub const XSD_BOOLEAN_IRI: &str = "http://www.w3.org/2001/XMLSchema#boolean";
pub const XSD_DATETIME_IRI: &str = "http://www.w3.org/2001/XMLSchema#dateTime";
pub const XSD_DATE_IRI: &str = "http://www.w3.org/2001/XMLSchema#date";
pub const RDF_LANG_STRING_IRI: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString";

/// Dictionary rows 1..=63 are reserved for these IRIs; `sparql_init()` (and MemStore::new)
/// insert them at exactly these rows. User terms start at `FIRST_USER_ROW`.
pub const WELL_KNOWN: &[(i64, &str)] = &[
    (1, "http://www.w3.org/2001/XMLSchema#string"),
    (2, "http://www.w3.org/2001/XMLSchema#integer"),
    (3, "http://www.w3.org/2001/XMLSchema#decimal"),
    (4, "http://www.w3.org/2001/XMLSchema#double"),
    (5, "http://www.w3.org/2001/XMLSchema#float"),
    (6, "http://www.w3.org/2001/XMLSchema#boolean"),
    (7, "http://www.w3.org/2001/XMLSchema#dateTime"),
    (8, "http://www.w3.org/2001/XMLSchema#date"),
    (9, "http://www.w3.org/2001/XMLSchema#time"),
    (10, "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString"),
    (11, "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
    (12, "http://www.w3.org/1999/02/22-rdf-syntax-ns#first"),
    (13, "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest"),
    (14, "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil"),
    (15, "http://www.w3.org/2001/XMLSchema#duration"),
    (16, "http://www.w3.org/2001/XMLSchema#dayTimeDuration"),
    (17, "http://www.w3.org/2001/XMLSchema#yearMonthDuration"),
    (18, "http://www.w3.org/2001/XMLSchema#long"),
    (19, "http://www.w3.org/2001/XMLSchema#int"),
    (20, "http://www.w3.org/2001/XMLSchema#short"),
    (21, "http://www.w3.org/2001/XMLSchema#byte"),
    (22, "http://www.w3.org/2001/XMLSchema#nonNegativeInteger"),
    (23, "http://www.w3.org/2001/XMLSchema#positiveInteger"),
    (24, "http://www.w3.org/2001/XMLSchema#nonPositiveInteger"),
    (25, "http://www.w3.org/2001/XMLSchema#negativeInteger"),
    (26, "http://www.w3.org/2001/XMLSchema#unsignedLong"),
    (27, "http://www.w3.org/2001/XMLSchema#unsignedInt"),
    (28, "http://www.w3.org/2001/XMLSchema#unsignedShort"),
    (29, "http://www.w3.org/2001/XMLSchema#unsignedByte"),
    (30, "http://www.w3.org/2001/XMLSchema#gYear"),
    (31, "http://www.w3.org/2001/XMLSchema#gYearMonth"),
    (32, "http://www.w3.org/2001/XMLSchema#gMonth"),
    (33, "http://www.w3.org/2001/XMLSchema#gMonthDay"),
    (34, "http://www.w3.org/2001/XMLSchema#gDay"),
    (35, "http://www.w3.org/2000/01/rdf-schema#subClassOf"),
];
pub const FIRST_USER_ROW: i64 = 1024;

pub const XSD_STRING: TermId = TermId::from_dict(1, Tag::Iri);
pub const XSD_INTEGER: TermId = TermId::from_dict(2, Tag::Iri);
pub const XSD_DECIMAL: TermId = TermId::from_dict(3, Tag::Iri);
pub const XSD_DOUBLE: TermId = TermId::from_dict(4, Tag::Iri);
pub const XSD_FLOAT: TermId = TermId::from_dict(5, Tag::Iri);
pub const XSD_BOOLEAN: TermId = TermId::from_dict(6, Tag::Iri);
pub const XSD_DATETIME: TermId = TermId::from_dict(7, Tag::Iri);
pub const XSD_DATE: TermId = TermId::from_dict(8, Tag::Iri);
pub const RDF_LANG_STRING: TermId = TermId::from_dict(10, Tag::Iri);
pub const RDF_TYPE: TermId = TermId::from_dict(11, Tag::Iri);

/// Datatype IRI id of an inline literal (constant, no lookup).
pub const fn inline_datatype(id: TermId) -> Option<TermId> {
    match id.tag() {
        Tag::Int => Some(XSD_INTEGER),
        Tag::Bool => Some(XSD_BOOLEAN),
        Tag::Decimal => Some(XSD_DECIMAL),
        Tag::DateTime => Some(XSD_DATETIME),
        Tag::Date => Some(XSD_DATE),
        Tag::ShortStr => Some(XSD_STRING),
        _ => None,
    }
}

// ---------------------------------------------------------------- inline encoding

/// Canonical id of an inline-encodable term, or None (term needs the dictionary / is a bnode).
pub fn encode_inline(t: oxrdf::TermRef<'_>) -> Option<TermId> {
    match t {
        oxrdf::TermRef::Literal(l) => encode_inline_literal(l.value(), l.datatype().as_str(), l.language()),
        _ => None,
    }
}

pub fn encode_inline_literal(lex: &str, datatype: &str, lang: Option<&str>) -> Option<TermId> {
    if lang.is_some() {
        return None;
    }
    let local = datatype.strip_prefix(XSD)?;
    match local {
        "integer" => TermId::from_int(parse_canonical_integer(lex)?),
        "boolean" => match lex {
            "true" => Some(TermId::from_bool(true)),
            "false" => Some(TermId::from_bool(false)),
            _ => None,
        },
        "decimal" => {
            let (m, scale, dotzero) = parse_canonical_decimal(lex)?;
            Some(encode_decimal(m, scale, dotzero))
        }
        "dateTime" => {
            let (ms, tz) = parse_canonical_datetime(lex)?;
            Some(TermId(((ms << 16) as u64 | ((tz as u64) << 4) | Tag::DateTime as u64) as i64))
        }
        "date" => {
            let (days, tz) = parse_canonical_date(lex)?;
            Some(TermId((((days as i64) << 16) as u64 | ((tz as u64) << 4) | Tag::Date as u64) as i64))
        }
        "string" => encode_short_str(lex),
        _ => None,
    }
}

/// Inverse of `encode_inline` for tags 6..=11.
pub fn decode_inline(id: TermId) -> Option<Term> {
    let (lex, dt): (String, &str) = match id.tag() {
        Tag::Int => ((id.0 >> 4).to_string(), XSD_INTEGER_IRI),
        Tag::Bool => ((if id.0 >> 4 != 0 { "true" } else { "false" }).to_string(), XSD_BOOLEAN_IRI),
        Tag::Decimal => {
            let (m, scale, dotzero) = decimal_parts(id);
            (format_decimal(m, scale, dotzero), XSD_DECIMAL_IRI)
        }
        Tag::DateTime => {
            let (ms, tz) = datetime_parts(id);
            (format_datetime(ms, tz), XSD_DATETIME_IRI)
        }
        Tag::Date => {
            let (days, tz) = date_parts(id);
            (format_date(days, tz), XSD_DATE_IRI)
        }
        Tag::ShortStr => (decode_short_str(id)?, XSD_STRING_IRI),
        _ => return None,
    };
    Some(Term::Literal(Literal { lex: Arc::from(lex), datatype: Arc::from(dt), lang: None }))
}

/// `-?(0|[1-9][0-9]*)`, not "-0", fits i60.
pub fn parse_canonical_integer(lex: &str) -> Option<i64> {
    let digits = lex.strip_prefix('-').unwrap_or(lex);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if digits.len() > 1 && digits.starts_with('0') {
        return None;
    }
    if lex == "-0" {
        return None;
    }
    let v: i64 = lex.parse().ok()?;
    (TermId::INT_MIN..=TermId::INT_MAX).contains(&v).then_some(v)
}

// DECIMAL: id = (m << 9) | (dotzero << 8) | (scale << 4) | 8,  m: i55, scale: 0..=15.
// Accepted lexical forms (and nothing else):
//   integral without point   "0", "-12"            scale 0, dotzero 0   (not "-0")
//   integral with ".0"       "0.0", "5.0", "-5.0"  scale 0, dotzero 1   (not "-0.0")
//   fractional               "0.5", "-12.25"       scale = #fraction digits (1..=15),
//                                                  last fraction digit != '0'
// Integer part never has a leading zero (except "0") and there is never a '+'.
const DEC_M_MIN: i64 = -(1 << 54);
const DEC_M_MAX: i64 = (1 << 54) - 1;

pub fn parse_canonical_decimal(lex: &str) -> Option<(i64, u8, bool)> {
    let (neg, body) = match lex.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, lex),
    };
    let (int_part, frac) = match body.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (body, None),
    };
    if int_part.is_empty() || !int_part.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if int_part.len() > 1 && int_part.starts_with('0') {
        return None;
    }
    let ip: i64 = int_part.parse().ok()?;
    let (mut m, scale, dotzero) = match frac {
        None => (ip, 0u8, false),
        Some("0") => (ip, 0u8, true),
        Some(f) => {
            if f.is_empty() || f.len() > 15 || !f.bytes().all(|b| b.is_ascii_digit()) || f.ends_with('0') {
                return None;
            }
            let fv: i64 = f.parse().ok()?;
            let m = ip.checked_mul(10i64.checked_pow(f.len() as u32)?)?.checked_add(fv)?;
            (m, f.len() as u8, false)
        }
    };
    if neg {
        if m == 0 {
            return None; // "-0", "-0.0"
        }
        m = -m;
    }
    (DEC_M_MIN..=DEC_M_MAX).contains(&m).then_some((m, scale, dotzero))
}

pub fn encode_decimal(m: i64, scale: u8, dotzero: bool) -> TermId {
    debug_assert!((DEC_M_MIN..=DEC_M_MAX).contains(&m) && scale <= 15 && !(dotzero && scale != 0));
    TermId((m << 9) | ((dotzero as i64) << 8) | ((scale as i64) << 4) | Tag::Decimal as i64)
}

pub fn decimal_parts(id: TermId) -> (i64, u8, bool) {
    (id.0 >> 9, ((id.0 >> 4) & 0xF) as u8, (id.0 >> 8) & 1 == 1)
}

pub fn format_decimal(m: i64, scale: u8, dotzero: bool) -> String {
    if scale == 0 {
        return if dotzero { format!("{m}.0") } else { m.to_string() };
    }
    let s = scale as usize;
    let mut digits = m.unsigned_abs().to_string();
    while digits.len() < s + 1 {
        digits.insert(0, '0');
    }
    digits.insert(digits.len() - s, '.');
    if m < 0 {
        digits.insert(0, '-');
    }
    digits
}

// DATETIME: id = (ms << 16) | (tz << 4) | 9, ms: i48 = wall-clock milliseconds since
// 1970-01-01T00:00:00 of the LEXICAL fields (NOT shifted by the offset), tz code (12 bits):
//   0 = no timezone, 1 = "Z", 2..=1682 = explicit offset (minutes + 842), e.g. "+00:00" = 842.
// Canonical lexical: YYYY-MM-DDThh:mm:ss[.f{1,3} without trailing zero][Z|(+|-)hh:mm]
//   year 0001..=6000, hour 00..=23, no leap seconds, |offset| <= 14:00, "-00:00" not inlined.
// DATE: id = (days << 16) | (tz << 4) | 10, days since 1970-01-01, lexical YYYY-MM-DD[tz].

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    // Howard Hinnant's algorithm.
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        _ => 28,
    }
}

fn num(s: &str) -> Option<u32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Parses "YYYY-MM-DD" (exactly 10 bytes) -> days.
fn parse_ymd(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let y = num(&s[0..4])? as i64;
    let m = num(&s[5..7])?;
    let d = num(&s[8..10])?;
    if !(1..=6000).contains(&y) || !(1..=12).contains(&m) || d == 0 || d > days_in_month(y, m) {
        return None;
    }
    Some(days_from_civil(y, m, d))
}

/// Parses an optional timezone suffix -> tz code.
fn parse_tz(s: &str) -> Option<u16> {
    match s {
        "" => Some(0),
        "Z" => Some(1),
        "-00:00" => None,
        _ => {
            let b = s.as_bytes();
            if b.len() != 6 || (b[0] != b'+' && b[0] != b'-') || b[3] != b':' {
                return None;
            }
            let h = num(&s[1..3])? as i32;
            let mi = num(&s[4..6])? as i32;
            if h > 14 || mi > 59 || (h == 14 && mi != 0) {
                return None;
            }
            let off = (h * 60 + mi) * if b[0] == b'-' { -1 } else { 1 };
            Some((off + 842) as u16)
        }
    }
}

fn format_tz(tz: u16) -> String {
    match tz {
        0 => String::new(),
        1 => "Z".to_string(),
        c => {
            let off = c as i32 - 842;
            let sign = if off < 0 { '-' } else { '+' };
            format!("{sign}{:02}:{:02}", off.abs() / 60, off.abs() % 60)
        }
    }
}

fn split_tz(s: &str) -> (&str, &str) {
    if let Some(body) = s.strip_suffix('Z') {
        return (body, "Z");
    }
    if s.len() >= 6 {
        let cut = s.len() - 6;
        let b = s.as_bytes()[cut];
        if (b == b'+' || b == b'-') && s.as_bytes()[s.len() - 3] == b':' {
            return (&s[..cut], &s[cut..]);
        }
    }
    (s, "")
}

const MS_MIN: i64 = -(1 << 47);
const MS_MAX: i64 = (1 << 47) - 1;

pub fn parse_canonical_datetime(lex: &str) -> Option<(i64, u16)> {
    let (body, tzs) = split_tz(lex);
    let tz = parse_tz(tzs)?;
    let (date, time) = body.split_once('T')?;
    let days = parse_ymd(date)?;
    let (hms, frac) = match time.split_once('.') {
        Some((h, f)) => (h, Some(f)),
        None => (time, None),
    };
    let b = hms.as_bytes();
    if b.len() != 8 || b[2] != b':' || b[5] != b':' {
        return None;
    }
    let (h, mi, s) = (num(&hms[0..2])?, num(&hms[3..5])?, num(&hms[6..8])?);
    if h > 23 || mi > 59 || s > 59 {
        return None;
    }
    let ms_frac = match frac {
        None => 0,
        Some(f) => {
            if f.is_empty() || f.len() > 3 || f.ends_with('0') {
                return None;
            }
            num(f)? * 10u32.pow(3 - f.len() as u32)
        }
    };
    let ms = days * 86_400_000 + (h as i64 * 3600 + mi as i64 * 60 + s as i64) * 1000 + ms_frac as i64;
    let out = (ms, tz);
    ((MS_MIN..=MS_MAX).contains(&ms) && format_datetime(ms, tz) == lex).then_some(out)
}

pub fn datetime_parts(id: TermId) -> (i64, u16) {
    (id.0 >> 16, ((id.0 >> 4) & 0xFFF) as u16)
}

pub fn format_datetime(ms: i64, tz: u16) -> String {
    let days = ms.div_euclid(86_400_000);
    let rem = ms.rem_euclid(86_400_000);
    let (y, m, d) = civil_from_days(days);
    let (secs, milli) = (rem / 1000, rem % 1000);
    let mut s = format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}", secs / 3600, (secs / 60) % 60, secs % 60);
    if milli != 0 {
        let f = format!("{milli:03}");
        s.push('.');
        s.push_str(f.trim_end_matches('0'));
    }
    s.push_str(&format_tz(tz));
    s
}

pub fn parse_canonical_date(lex: &str) -> Option<(i32, u16)> {
    let (body, tzs) = split_tz(lex);
    let tz = parse_tz(tzs)?;
    let days = parse_ymd(body)?;
    (format_date(days as i32, tz) == lex).then_some((days as i32, tz))
}

pub fn date_parts(id: TermId) -> (i32, u16) {
    ((id.0 >> 16) as i32, ((id.0 >> 4) & 0xFFF) as u16)
}

pub fn format_date(days: i32, tz: u16) -> String {
    let (y, m, d) = civil_from_days(days as i64);
    format!("{y:04}-{m:02}-{d:02}{}", format_tz(tz))
}

// SHORTSTR: id = (be7 << 8) | (len << 4) | 11 computed in u64, where be7 packs the (<= 7)
// UTF-8 bytes big-endian into 56 bits, zero padded; len = 0..=7.
pub fn encode_short_str(s: &str) -> Option<TermId> {
    let b = s.as_bytes();
    if b.len() > 7 {
        return None;
    }
    let mut be7: u64 = 0;
    for i in 0..7 {
        be7 = (be7 << 8) | *b.get(i).unwrap_or(&0) as u64;
    }
    Some(TermId(((be7 << 8) | ((b.len() as u64) << 4) | Tag::ShortStr as u64) as i64))
}

pub fn decode_short_str(id: TermId) -> Option<String> {
    let u = id.0 as u64;
    let len = ((u >> 4) & 0xF) as usize;
    if len > 7 {
        return None;
    }
    let be7 = u >> 8;
    let bytes: Vec<u8> = (0..len).map(|i| ((be7 >> (8 * (6 - i))) & 0xFF) as u8).collect();
    String::from_utf8(bytes).ok()
}

// ---------------------------------------------------------------- dictionary keys & hash

/// Dictionary tag of a non-inline term (None for blank nodes: they never get rows).
pub fn dict_tag(t: oxrdf::TermRef<'_>) -> Option<Tag> {
    match t {
        oxrdf::TermRef::NamedNode(_) => Some(Tag::Iri),
        oxrdf::TermRef::Literal(l) => Some(if l.language().is_some() {
            Tag::Lang
        } else if l.datatype().as_str() == XSD_STRING_IRI {
            Tag::Str
        } else {
            Tag::Typed
        }),
        _ => None,
    }
}

/// Exact dictionary identity: (tag, lex, extra) with extra = datatype IRI (Typed),
/// lower-case language (Lang), "" otherwise.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct TermKey {
    pub tag: Tag,
    pub lex: Box<str>,
    pub extra: Box<str>,
}

impl TermKey {
    /// None for blank nodes and inline-encodable terms.
    pub fn of(t: oxrdf::TermRef<'_>) -> Option<TermKey> {
        if encode_inline(t).is_some() {
            return None;
        }
        let tag = dict_tag(t)?;
        Some(match t {
            oxrdf::TermRef::NamedNode(n) => TermKey { tag, lex: n.as_str().into(), extra: "".into() },
            oxrdf::TermRef::Literal(l) => TermKey {
                tag,
                lex: l.value().into(),
                extra: match tag {
                    Tag::Lang => l.language().unwrap_or("").to_ascii_lowercase().into(),
                    Tag::Typed => l.datatype().as_str().into(),
                    _ => "".into(),
                },
            },
            _ => return None,
        })
    }
    pub fn hash64(&self) -> i64 {
        term_hash(self.tag, &self.lex, &self.extra)
    }
}

pub const TERM_HASH_SEED: u64 = 0x5350_4152_514C;

/// FROZEN (on-disk `rdf_term.h`): xxh3_64_with_seed([tag] ++ lex ++ [0x00] ++ extra, SEED) as i64.
pub fn term_hash(tag: Tag, lex: &str, extra: &str) -> i64 {
    let mut buf = Vec::with_capacity(lex.len() + extra.len() + 2);
    buf.push(tag as u8);
    buf.extend_from_slice(lex.as_bytes());
    buf.push(0);
    buf.extend_from_slice(extra.as_bytes());
    xxhash_rust::xxh3::xxh3_64_with_seed(&buf, TERM_HASH_SEED) as i64
}

/// Blank-node label of a store bnode id: "b<lower-hex counter>".
pub fn bnode_label(id: TermId) -> String {
    format!("b{:x}", id.bnode_counter())
}

/// Inverse of `bnode_label` (used by `sparql_term_id('_:b1f')`).
pub fn parse_bnode_label(label: &str) -> Option<TermId> {
    let hex = label.strip_prefix('b')?;
    let n = i64::from_str_radix(hex, 16).ok()?;
    let id = TermId::bnode(n);
    (n > 0 && n < (1 << 59) && bnode_label(id) == label).then_some(id)
}
```

### `src/batch.rs`
```rust
//! Solution rows: one global slot layout per query; a row is `[TermId; width]`, 0 = unbound.
use crate::id::TermId;

/// Index of a variable (user, blank-node, path-middle, renamed sub-select, aggregate) in the
/// query-wide slot layout.
pub type Slot = u16;
pub const BATCH_ROWS: usize = 1024;

/// Row-major batch of solutions.
#[derive(Clone, Debug, Default)]
pub struct Batch {
    width: usize,
    len: usize,
    data: Vec<TermId>,
}

impl Batch {
    pub fn new(width: usize) -> Self {
        Self::with_capacity(width, BATCH_ROWS)
    }
    pub fn with_capacity(width: usize, rows: usize) -> Self {
        Batch { width, len: 0, data: Vec::with_capacity(width.max(1) * rows) }
    }
    #[inline]
    pub fn width(&self) -> usize {
        self.width
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    #[inline]
    pub fn is_full(&self) -> bool {
        self.len >= BATCH_ROWS
    }
    #[inline]
    pub fn row(&self, i: usize) -> &[TermId] {
        &self.data[i * self.width..(i + 1) * self.width]
    }
    #[inline]
    pub fn row_mut(&mut self, i: usize) -> &mut [TermId] {
        &mut self.data[i * self.width..(i + 1) * self.width]
    }
    #[inline]
    pub fn get(&self, i: usize, s: Slot) -> TermId {
        self.data[i * self.width + s as usize]
    }
    #[inline]
    pub fn push_row(&mut self, r: &[TermId]) {
        debug_assert_eq!(r.len(), self.width);
        self.data.extend_from_slice(r);
        self.len += 1;
    }
    /// Append an all-unbound row and return it for filling.
    #[inline]
    pub fn push_unbound(&mut self) -> &mut [TermId] {
        let start = self.data.len();
        self.data.resize(start + self.width, TermId::UNBOUND);
        self.len += 1;
        &mut self.data[start..]
    }
    /// Copy `base`, then overwrite (slot, value) pairs: the INL / merge-join emit primitive.
    #[inline]
    pub fn push_extended(&mut self, base: &[TermId], sets: &[(Slot, TermId)]) {
        let start = self.data.len();
        self.data.extend_from_slice(base);
        for &(s, v) in sets {
            self.data[start + s as usize] = v;
        }
        self.len += 1;
    }
    pub fn clear(&mut self) {
        self.data.clear();
        self.len = 0;
    }
    pub fn truncate(&mut self, rows: usize) {
        if rows < self.len {
            self.data.truncate(rows * self.width);
            self.len = rows;
        }
    }
    pub fn rows(&self) -> std::slice::ChunksExact<'_, TermId> {
        self.data[..self.len * self.width].chunks_exact(self.width.max(1))
    }
    pub fn retain(&mut self, mut keep: impl FnMut(&[TermId]) -> bool) {
        let w = self.width;
        let mut out = 0;
        for i in 0..self.len {
            if keep(&self.data[i * w..(i + 1) * w]) {
                if out != i {
                    self.data.copy_within(i * w..(i + 1) * w, out * w);
                }
                out += 1;
            }
        }
        self.truncate(out);
    }
}

/// SPARQL compatibility merge: for every slot in `check` (slots possibly bound on both sides)
/// both values must be equal when both are bound; then every bound slot of `src` is copied
/// into `dst`. Returns false (dst unspecified) if incompatible.
#[inline]
pub fn merge_compatible(dst: &mut [TermId], src: &[TermId], check: &[Slot]) -> bool {
    for &s in check {
        let (a, b) = (dst[s as usize], src[s as usize]);
        if a.is_bound() && b.is_bound() && a != b {
            return false;
        }
    }
    for (d, s) in dst.iter_mut().zip(src) {
        if s.is_bound() {
            *d = *s;
        }
    }
    true
}

/// Fixed-size set of slots (queries have at most 256 slots; more is Error::Unsupported).
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Debug)]
pub struct SlotSet(pub [u64; 4]);

impl SlotSet {
    pub const EMPTY: SlotSet = SlotSet([0; 4]);
    pub const MAX_SLOTS: usize = 256;
    #[inline]
    pub fn insert(&mut self, s: Slot) {
        self.0[(s >> 6) as usize] |= 1 << (s & 63);
    }
    #[inline]
    pub fn remove(&mut self, s: Slot) {
        self.0[(s >> 6) as usize] &= !(1 << (s & 63));
    }
    #[inline]
    pub fn contains(&self, s: Slot) -> bool {
        self.0[(s >> 6) as usize] >> (s & 63) & 1 == 1
    }
    pub fn union(self, o: SlotSet) -> SlotSet {
        SlotSet([self.0[0] | o.0[0], self.0[1] | o.0[1], self.0[2] | o.0[2], self.0[3] | o.0[3]])
    }
    pub fn intersect(self, o: SlotSet) -> SlotSet {
        SlotSet([self.0[0] & o.0[0], self.0[1] & o.0[1], self.0[2] & o.0[2], self.0[3] & o.0[3]])
    }
    pub fn minus(self, o: SlotSet) -> SlotSet {
        SlotSet([self.0[0] & !o.0[0], self.0[1] & !o.0[1], self.0[2] & !o.0[2], self.0[3] & !o.0[3]])
    }
    pub fn is_empty(&self) -> bool {
        self.0 == [0; 4]
    }
    pub fn is_subset(&self, o: &SlotSet) -> bool {
        self.minus(*o).is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = Slot> + '_ {
        (0..256u16).filter(|s| self.contains(*s))
    }
    pub fn from_slots(slots: impl IntoIterator<Item = Slot>) -> SlotSet {
        let mut s = SlotSet::EMPTY;
        for x in slots {
            s.insert(x);
        }
        s
    }
}
```

### `src/store.rs`
```rust
//! Storage-access contract used by the planner and executor. Implemented by
//! `sq-memstore::MemStore` (oracle/test double), `sq-sqlite::SqliteStore` (SQL cursors) and,
//! in phase 2, `sq-btree` (direct page reader) behind the same `QuadCursor`.
use std::ops::Bound;
use std::sync::Arc;

use crate::error::Result;
use crate::id::TermId;
use crate::stats::{CostConstants, Stats};
use crate::term::Term;

/// Quad column. Quads are always written `[g, s, p, o]` (the primary-key order).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Col {
    G = 0,
    S = 1,
    P = 2,
    O = 3,
}

/// The nine physical orders. Every one is a COVERING B-tree of `rdf_quad` (WITHOUT ROWID).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum IndexOrder {
    /// Primary key (g,s,p,o).
    Gspo,
    /// rdf_gpso (g,p,s,o): s-sorted stream for every `?s :p ?o`.
    Gpso,
    /// rdf_gpos (g,p,o,s): o-sorted stream for `?s :p ?o`, s-sorted for `?s :p :o`.
    Gpos,
    /// rdf_gosp (g,o,s,p).
    Gosp,
    /// Partial `WHERE g<>0` (named graphs only), g last: GRAPH ?g / union-default merges.
    Spog,
    Psog,
    Posg,
    Ospg,
    /// rdf_gpox (g,p,o,s) partial `WHERE (o & 15) IN (5,8)`: the objects of p that are
    /// dictionary-typed literals or inline decimals ("numeric exceptions" of int range scans).
    Gpox,
}

impl IndexOrder {
    pub const ALL: [IndexOrder; 9] = [
        IndexOrder::Gspo,
        IndexOrder::Gpso,
        IndexOrder::Gpos,
        IndexOrder::Gosp,
        IndexOrder::Spog,
        IndexOrder::Psog,
        IndexOrder::Posg,
        IndexOrder::Ospg,
        IndexOrder::Gpox,
    ];
    pub const fn cols(self) -> [Col; 4] {
        use Col::*;
        match self {
            IndexOrder::Gspo => [G, S, P, O],
            IndexOrder::Gpso => [G, P, S, O],
            IndexOrder::Gpos | IndexOrder::Gpox => [G, P, O, S],
            IndexOrder::Gosp => [G, O, S, P],
            IndexOrder::Spog => [S, P, O, G],
            IndexOrder::Psog => [P, S, O, G],
            IndexOrder::Posg => [P, O, S, G],
            IndexOrder::Ospg => [O, S, P, G],
        }
    }
    /// Rows only from named graphs (g <> 0); g is the LAST column.
    pub const fn named_only(self) -> bool {
        matches!(self, IndexOrder::Spog | IndexOrder::Psog | IndexOrder::Posg | IndexOrder::Ospg)
    }
    /// Rows only where tag(o) is 5 (Typed) or 8 (Decimal).
    pub const fn numeric_exceptions(self) -> bool {
        matches!(self, IndexOrder::Gpox)
    }
    /// FROM clause used by the SQL cursor (all SQL is schema-qualified `main.`).
    pub const fn sql_from(self) -> &'static str {
        match self {
            IndexOrder::Gspo => "main.rdf_quad INDEXED BY sqlite_autoindex_rdf_quad_1",
            IndexOrder::Gpso => "main.rdf_quad INDEXED BY rdf_gpso",
            IndexOrder::Gpos => "main.rdf_quad INDEXED BY rdf_gpos",
            IndexOrder::Gosp => "main.rdf_quad INDEXED BY rdf_gosp",
            IndexOrder::Spog => "main.rdf_quad INDEXED BY rdf_spog",
            IndexOrder::Psog => "main.rdf_quad INDEXED BY rdf_psog",
            IndexOrder::Posg => "main.rdf_quad INDEXED BY rdf_posg",
            IndexOrder::Ospg => "main.rdf_quad INDEXED BY rdf_ospg",
            IndexOrder::Gpox => "main.rdf_quad INDEXED BY rdf_gpox",
        }
    }
    /// Extra WHERE term that makes a partial index usable (must appear verbatim).
    pub const fn sql_partial_term(self) -> &'static str {
        if self.named_only() {
            "g<>0"
        } else if self.numeric_exceptions() {
            "(o & 15) IN (5,8)"
        } else {
            ""
        }
    }
    /// Orders (excluding Gpox) whose first `bound.len()` columns are exactly the bound
    /// columns. `bound[c]` = column c (G,S,P,O) is bound. If G is unbound, only named-only
    /// orders qualify (GRAPH ?g); if G is bound, only graph-first orders qualify.
    pub fn candidates(bound: [bool; 4]) -> Vec<(IndexOrder, u8)> {
        let k = bound.iter().filter(|b| **b).count();
        IndexOrder::ALL
            .iter()
            .copied()
            .filter(|o| !o.numeric_exceptions() && o.named_only() != bound[0])
            .filter(|o| o.cols()[..k].iter().all(|c| bound[*c as usize]))
            .map(|o| (o, k as u8))
            .collect()
    }
}

/// A range scan: fixed key prefix (in index order) + optional bounds on the next column.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct ScanSpec {
    pub order: IndexOrder,
    /// Values of the first `prefix_len` columns of `order.cols()`.
    pub prefix: [TermId; 4],
    pub prefix_len: u8,
    /// Bounds on column `order.cols()[prefix_len]` (ignored when prefix_len == 4).
    pub lower: Bound<i64>,
    pub upper: Bound<i64>,
}

impl ScanSpec {
    pub fn new(order: IndexOrder, prefix: &[TermId]) -> Self {
        let mut p = [TermId::UNBOUND; 4];
        p[..prefix.len()].copy_from_slice(prefix);
        ScanSpec { order, prefix: p, prefix_len: prefix.len() as u8, lower: Bound::Unbounded, upper: Bound::Unbounded }
    }
    pub fn with_range(mut self, lower: Bound<i64>, upper: Bound<i64>) -> Self {
        self.lower = lower;
        self.upper = upper;
        self
    }
    /// Free (output) columns, in index order.
    pub fn free_cols(&self) -> &'static [Col] {
        let cols: &'static [Col; 4] = match self.order {
            IndexOrder::Gspo => &[Col::G, Col::S, Col::P, Col::O],
            IndexOrder::Gpso => &[Col::G, Col::P, Col::S, Col::O],
            IndexOrder::Gpos | IndexOrder::Gpox => &[Col::G, Col::P, Col::O, Col::S],
            IndexOrder::Gosp => &[Col::G, Col::O, Col::S, Col::P],
            IndexOrder::Spog => &[Col::S, Col::P, Col::O, Col::G],
            IndexOrder::Psog => &[Col::P, Col::S, Col::O, Col::G],
            IndexOrder::Posg => &[Col::P, Col::O, Col::S, Col::G],
            IndexOrder::Ospg => &[Col::O, Col::S, Col::P, Col::G],
        };
        &cols[self.prefix_len as usize..]
    }
    /// Does a full quad [g,s,p,o] match this spec (used by MemStore and the conformance suite)?
    pub fn matches(&self, q: &[TermId; 4]) -> bool {
        let cols = self.order.cols();
        for i in 0..self.prefix_len as usize {
            if q[cols[i] as usize] != self.prefix[i] {
                return false;
            }
        }
        if self.order.named_only() && q[0] == TermId::DEFAULT_GRAPH {
            return false;
        }
        if self.order.numeric_exceptions() && !matches!(q[3].0 & 15, 5 | 8) {
            return false;
        }
        if (self.prefix_len as usize) < 4 {
            let v = q[cols[self.prefix_len as usize] as usize].0;
            let lo_ok = match self.lower {
                Bound::Included(l) => v >= l,
                Bound::Excluded(l) => v > l,
                Bound::Unbounded => true,
            };
            let hi_ok = match self.upper {
                Bound::Included(h) => v <= h,
                Bound::Excluded(h) => v < h,
                Bound::Unbounded => true,
            };
            return lo_ok && hi_ok;
        }
        true
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum ReaderKind {
    #[default]
    Memory,
    SqlCursor,
    Btree,
}

#[derive(Copy, Clone, Default, Debug)]
pub struct CursorCounters {
    pub rows: u64,
    pub seeks: u64,
    pub fills: u64,
    pub reader: ReaderKind,
}

/// A positioned range scan over one index.
///
/// Rows are the FREE SUFFIX columns of the spec (`width() == 4 - prefix_len`), appended
/// row-major, in ascending lexicographic SIGNED-i64 order, each row exactly once.
pub trait QuadCursor {
    fn width(&self) -> usize;
    /// Append up to `max_rows` rows to `out`. Returns the number appended; 0 = exhausted
    /// (a later `seek`/`rebind` restarts it).
    fn fill(&mut self, out: &mut Vec<TermId>, max_rows: usize) -> Result<usize>;
    /// Reposition so that the next `fill` returns exactly the rows r with
    /// `suffix(r) >= key` (lexicographic over the first key.len() free columns,
    /// 1 <= key.len() <= width) that also satisfy the spec's bounds. Forward seeks are the
    /// fast path (finger/galloping); backward seeks are legal but may restart from the root.
    fn seek(&mut self, key: &[TermId]) -> Result<()>;
    /// Cheap retarget for index-nested-loop probes: same order and prefix_len, new prefix
    /// values and bounds; positions at the start. Must not allocate per call.
    fn rebind(&mut self, prefix: &[TermId], lower: Bound<i64>, upper: Bound<i64>) -> Result<()>;
    fn counters(&self) -> CursorCounters;
}

/// Term dictionary access (read side).
pub trait Dictionary {
    /// Canonical id of an EXISTING term: inline-encodable terms are always Some (no I/O);
    /// store blank-node labels "b<hex>" map to their id; never inserts. None = not in store.
    fn lookup(&self, t: oxrdf::TermRef<'_>) -> Result<Option<TermId>>;
    /// Decode storable ids (dictionary, bnode, inline); appends one entry per input id, in
    /// order. TEMP/UNBOUND/ERROR ids are `Error::Internal`. Implementations sort+dedupe misses
    /// and use the connection cache.
    fn decode(&self, ids: &[TermId], out: &mut Vec<Arc<Term>>) -> Result<()>;
    fn decode_one(&self, id: TermId) -> Result<Arc<Term>> {
        let mut v = Vec::with_capacity(1);
        self.decode(&[id], &mut v)?;
        v.pop().ok_or_else(|| crate::Error::internal("decode returned nothing"))
    }
}

#[derive(Copy, Clone, Debug, Default)]
pub struct StoreCaps {
    /// `sql_rows` is implemented (SqlJoin hybrid / CountScan in VDBE).
    pub sql: bool,
    pub reader: ReaderKind,
}

/// Object-safe. Executions hold `Rc<dyn QuadStore>`; cursors are `'static` (they own their
/// leased statement / snapshot), so a streaming execution can own its operator tree.
pub trait QuadStore {
    fn cursor(&self, spec: &ScanSpec) -> Result<Box<dyn QuadCursor>>;
    /// Number of rows of `spec`: exact when `cap` is None; otherwise `min(count, cap + 1)`.
    fn count(&self, spec: &ScanSpec, cap: Option<u64>) -> Result<u64>;
    fn dict(&self) -> &dyn Dictionary;
    /// All named graphs: registry (`rdf_graph`) UNION distinct g <> 0 in rdf_quad; sorted.
    fn named_graphs(&self) -> Result<Vec<TermId>>;
    /// None = never analyzed. Statistics are HINTS: correctness never depends on them.
    fn stats(&self) -> Option<Arc<Stats>>;
    fn costs(&self) -> &CostConstants;
    fn caps(&self) -> StoreCaps;
    /// Run a generated read-only SQL statement whose result columns are `width` INTEGER
    /// TermIds (SqlJoin hybrid). Only when `caps().sql`. seek/rebind are unsupported on it.
    fn sql_rows(&self, _sql: &str, _params: &[i64], _width: usize) -> Result<Box<dyn QuadCursor>> {
        Err(crate::Error::Unsupported("sql_rows".into()))
    }
    /// Polled by CPU-bound loops every 4096 rows; Err(Interrupted) after sqlite3_interrupt.
    fn check_interrupt(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum GraphTarget {
    Default,
    Named(TermId),
    AllNamed,
    All,
}

/// Write side. Atomicity is provided by the CALLER (sq-api wraps every request in a
/// SAVEPOINT write scope); implementations never commit.
pub trait QuadStoreWriter: QuadStore {
    /// Canonical id, inserting a dictionary row if needed. Blank nodes are `Error::Invalid`
    /// (callers map labels to `fresh_bnode()` per load / per update operation).
    fn intern(&self, t: oxrdf::TermRef<'_>) -> Result<TermId>;
    fn fresh_bnode(&self) -> Result<TermId>;
    /// Quads are [g,s,p,o]; sorts + dedups in place; rejects non-storable ids
    /// (`Error::Internal`); registers new named graphs in `rdf_graph`. Returns #new quads.
    fn insert(&self, quads: &mut Vec<[TermId; 4]>) -> Result<u64>;
    /// Returns #removed quads.
    fn remove(&self, quads: &mut Vec<[TermId; 4]>) -> Result<u64>;
    fn clear(&self, target: GraphTarget) -> Result<u64>;
    /// Registers an (empty) named graph; Ok(false) if it already existed.
    fn create_graph(&self, g: TermId) -> Result<bool>;
    /// Removes a graph from the registry (quads must already be cleared); Ok(false) if absent.
    fn drop_graph(&self, g: TermId) -> Result<bool>;
}
```

### `src/stats.rs`
```rust
//! Planner statistics (hints only) and cost constants.
use crate::id::TermId;
use rustc_hash::FxHashMap;

#[derive(Clone, Debug, Default)]
pub struct PredStats {
    /// Triples with this predicate.
    pub n: u64,
    /// Distinct subjects / objects.
    pub ds: u64,
    pub dobj: u64,
    /// Min / max inline xsd:integer object (selectivity of int ranges only).
    pub imin: Option<i64>,
    pub imax: Option<i64>,
}

/// One characteristic set: the subjects whose predicate set is exactly `preds`.
#[derive(Clone, Debug, Default)]
pub struct CharSet {
    pub subjects: u64,
    /// (predicate, number of triples with that predicate over these subjects), sorted by id.
    pub preds: Vec<(TermId, u64)>,
}

#[derive(Clone, Debug, Default)]
pub struct ScopeStats {
    pub n: u64,
    pub ds: u64,
    pub dp: u64,
    pub dobj: u64,
    pub pred: FxHashMap<TermId, PredStats>,
    /// Heavy hitters: (p, o) pairs with n >= max(100, n(p)/1000); covers rdf:type classes.
    pub po: FxHashMap<(TermId, TermId), u64>,
    /// Top characteristic sets by #subjects (at most 10_000).
    pub cs: Vec<CharSet>,
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    /// [0] = default graph (g = 0), [1] = union of all named graphs.
    pub scope: [ScopeStats; 2],
    /// Total quads when analyzed (planner scales nothing by it; diagnostic only).
    pub analyzed_quads: u64,
}

/// Cost model constants in nanoseconds, selected by the store's reader kind.
/// Calibrated in WP-14 from `sq-bench micro`; these are the starting values.
#[derive(Clone, Debug)]
pub struct CostConstants {
    /// Sequential scan cost per row (step + column_int64 per free column).
    pub scan_row: f64,
    /// Extra per selected column.
    pub scan_col: f64,
    /// Open/reposition a range scan at a random key (inside a held read txn).
    pub seek_random: f64,
    /// Reposition at a key > previous key (sorted probes, galloping).
    pub seek_sorted: f64,
    /// Hash build per row while the table fits in L2 (< 32K entries) / beyond.
    pub hash_build_small: f64,
    pub hash_build_large: f64,
    pub hash_probe_small: f64,
    pub hash_probe_large: f64,
    /// Sort per row per log2(n).
    pub sort_row_log: f64,
    pub row_copy_per_slot: f64,
    /// Dictionary decode: miss / cache hit.
    pub decode_miss: f64,
    pub decode_hit: f64,
    /// SQLite VDBE nested-loop probe per outer row (SqlJoin hybrid) and per output column.
    pub vdbe_probe: f64,
    pub vdbe_row: f64,
    /// count(*) evaluated inside the VDBE, per counted row.
    pub vdbe_count_row: f64,
}

pub const SQL_CURSOR_COSTS: CostConstants = CostConstants {
    scan_row: 80.0,
    scan_col: 15.0,
    seek_random: 1100.0,
    seek_sorted: 700.0,
    hash_build_small: 60.0,
    hash_build_large: 250.0,
    hash_probe_small: 25.0,
    hash_probe_large: 120.0,
    sort_row_log: 3.0,
    row_copy_per_slot: 1.0,
    decode_miss: 1000.0,
    decode_hit: 50.0,
    vdbe_probe: 550.0,
    vdbe_row: 60.0,
    vdbe_count_row: 40.0,
};

/// Phase-2 direct B-tree reader (realistic pread + page-cache numbers, not the fs::read prototype).
pub const BTREE_COSTS: CostConstants = CostConstants {
    scan_row: 30.0,
    scan_col: 2.0,
    seek_random: 1300.0,
    seek_sorted: 300.0,
    ..SQL_CURSOR_COSTS
};

pub const MEMORY_COSTS: CostConstants = CostConstants {
    scan_row: 20.0,
    scan_col: 2.0,
    seek_random: 300.0,
    seek_sorted: 150.0,
    ..SQL_CURSOR_COSTS
};
```

### `src/ir.rs`
```rust
//! Logical IR: output of `sq-plan::lower_query`, input of the optimizer AND of the naive
//! `sq-oracle` evaluator. It is spargebra's algebra with variables resolved to slots,
//! constants resolved to canonical ids (TEMP ids for constants absent from the store),
//! sub-select scopes alpha-renamed, and the dataset made explicit.
use std::sync::Arc;

use crate::batch::Slot;
use crate::eval_api::TempDict;
use crate::id::TermId;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum PatTerm {
    Slot(Slot),
    Const(TermId),
}

/// Which graph a quad pattern / path reads.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum GraphSel {
    /// The dataset's default graph (`DatasetSpec::default`).
    Default,
    /// Logical IR: the active graph of the innermost enclosing `Logical::Graph`.
    /// Physical plan: the innermost enclosing `Op::GraphLoop` register.
    Active,
    /// One named graph (membership in the dataset already checked by the planner).
    Const(TermId),
    /// Physical plan only: GRAPH ?g pushed into the pattern; ranges over the dataset's named
    /// graphs and binds the slot (never binds the default graph).
    Slot(Slot),
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct QuadPat {
    pub g: GraphSel,
    pub s: PatTerm,
    pub p: PatTerm,
    pub o: PatTerm,
}

/// Property path with constants resolved. spargebra already rewrites top-level `a/b`
/// into a BGP with a fresh blank-node middle and `!(a|^b)` into Alt(Nps, Inv(Nps)).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum PathPlan {
    Pred(TermId),
    Inv(Box<PathPlan>),
    Seq(Box<PathPlan>, Box<PathPlan>),
    Alt(Box<PathPlan>, Box<PathPlan>),
    ZeroOrMore(Box<PathPlan>),
    OneOrMore(Box<PathPlan>),
    ZeroOrOne(Box<PathPlan>),
    /// Negated property set (forward direction only; inverse sets arrive wrapped in Inv).
    Nps(Vec<TermId>),
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct ExistsId(pub u32);

/// Scalar expression. Constants are canonical ids (TEMP ids live in the query's TempDict).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Expr {
    Const(TermId),
    Slot(Slot),
    Or(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Equal(Box<Expr>, Box<Expr>),
    SameTerm(Box<Expr>, Box<Expr>),
    Greater(Box<Expr>, Box<Expr>),
    GreaterOrEqual(Box<Expr>, Box<Expr>),
    Less(Box<Expr>, Box<Expr>),
    LessOrEqual(Box<Expr>, Box<Expr>),
    In(Box<Expr>, Vec<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Subtract(Box<Expr>, Box<Expr>),
    Multiply(Box<Expr>, Box<Expr>),
    Divide(Box<Expr>, Box<Expr>),
    UnaryPlus(Box<Expr>),
    UnaryMinus(Box<Expr>),
    Bound(Slot),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    Coalesce(Vec<Expr>),
    /// `exists[id]` evaluated by substitution with the current row (spec 18.6).
    Exists(ExistsId),
    /// Builtins 1:1 with spargebra (sparql-12 / sep features OFF). `Custom(xsd:*)` = casts;
    /// any other Custom is an evaluation error (unknown function).
    Call(spargebra::algebra::Function, Vec<Expr>),
}

impl Expr {
    /// Slots read by this expression, NOT including slots inside EXISTS patterns.
    pub fn slots(&self, out: &mut Vec<Slot>) {
        match self {
            Expr::Const(_) | Expr::Exists(_) => {}
            Expr::Slot(s) | Expr::Bound(s) => out.push(*s),
            Expr::Not(a) | Expr::UnaryPlus(a) | Expr::UnaryMinus(a) => a.slots(out),
            Expr::Or(a, b)
            | Expr::And(a, b)
            | Expr::Equal(a, b)
            | Expr::SameTerm(a, b)
            | Expr::Greater(a, b)
            | Expr::GreaterOrEqual(a, b)
            | Expr::Less(a, b)
            | Expr::LessOrEqual(a, b)
            | Expr::Add(a, b)
            | Expr::Subtract(a, b)
            | Expr::Multiply(a, b)
            | Expr::Divide(a, b) => {
                a.slots(out);
                b.slots(out);
            }
            Expr::In(a, l) => {
                a.slots(out);
                l.iter().for_each(|e| e.slots(out));
            }
            Expr::If(a, b, c) => {
                a.slots(out);
                b.slots(out);
                c.slots(out);
            }
            Expr::Coalesce(l) | Expr::Call(_, l) => l.iter().for_each(|e| e.slots(out)),
        }
    }
    pub fn contains_exists(&self) -> bool {
        match self {
            Expr::Exists(_) => true,
            Expr::Const(_) | Expr::Slot(_) | Expr::Bound(_) => false,
            Expr::Not(a) | Expr::UnaryPlus(a) | Expr::UnaryMinus(a) => a.contains_exists(),
            Expr::Or(a, b)
            | Expr::And(a, b)
            | Expr::Equal(a, b)
            | Expr::SameTerm(a, b)
            | Expr::Greater(a, b)
            | Expr::GreaterOrEqual(a, b)
            | Expr::Less(a, b)
            | Expr::LessOrEqual(a, b)
            | Expr::Add(a, b)
            | Expr::Subtract(a, b)
            | Expr::Multiply(a, b)
            | Expr::Divide(a, b) => a.contains_exists() || b.contains_exists(),
            Expr::In(a, l) => a.contains_exists() || l.iter().any(Expr::contains_exists),
            Expr::If(a, b, c) => a.contains_exists() || b.contains_exists() || c.contains_exists(),
            Expr::Coalesce(l) | Expr::Call(_, l) => l.iter().any(Expr::contains_exists),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum AggFunc {
    /// COUNT(*) (with `distinct`: COUNT(DISTINCT *) over all in-scope slots of the input).
    CountAll,
    Count,
    Sum,
    Avg,
    Min,
    Max,
    Sample,
    GroupConcat { separator: String },
    /// Unknown custom aggregate: evaluation error -> unbound.
    Custom(String),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct AggSpec {
    pub func: AggFunc,
    pub distinct: bool,
    pub arg: Option<Expr>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct OrderKey {
    pub expr: Expr,
    pub desc: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Logical {
    /// One empty solution (empty group pattern `{}`).
    Unit,
    /// No solutions.
    Empty,
    Bgp(Vec<QuadPat>),
    Path { g: GraphSel, s: PatTerm, path: PathPlan, o: PatTerm },
    Join(Box<Logical>, Box<Logical>),
    LeftJoin { left: Box<Logical>, right: Box<Logical>, filter: Option<Expr> },
    Minus { left: Box<Logical>, right: Box<Logical> },
    Union(Box<Logical>, Box<Logical>),
    Filter { input: Box<Logical>, expr: Expr },
    Extend { input: Box<Logical>, slot: Slot, expr: Expr },
    /// Rows use TermId::UNBOUND for UNDEF.
    Values { slots: Vec<Slot>, rows: Vec<Vec<TermId>> },
    /// GRAPH name { input }: name is Const (IRI) or Slot (variable).
    Graph { name: PatTerm, input: Box<Logical> },
    Group { input: Box<Logical>, keys: Vec<Slot>, aggs: Vec<(Slot, AggSpec)> },
    OrderBy { input: Box<Logical>, keys: Vec<OrderKey> },
    /// Keeps `slots`, resets every other slot to UNBOUND (sub-select scoping; slots that are
    /// not projected were alpha-renamed to fresh slots during lowering).
    Project { input: Box<Logical>, slots: Vec<Slot> },
    Distinct(Box<Logical>),
    Reduced(Box<Logical>),
    Slice { input: Box<Logical>, offset: u64, limit: Option<u64> },
    /// SERVICE: always Error::Unsupported unless silent (then Unit).
    Service { silent: bool },
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum VarOrigin {
    User,
    /// Blank node in a pattern (one hidden slot per label per BGP scope).
    BlankNode,
    /// spargebra-generated (path middles, aggregate outputs, DESCRIBE constants).
    Generated,
    /// Alpha-renamed sub-select variable.
    Renamed,
}

#[derive(Clone, Debug, Default)]
pub struct VarTable {
    /// Index = Slot.
    pub vars: Vec<(String, VarOrigin)>,
}

impl VarTable {
    pub fn width(&self) -> usize {
        self.vars.len()
    }
    pub fn name(&self, s: Slot) -> &str {
        &self.vars[s as usize].0
    }
}

/// What the default graph of the query dataset is.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DefaultGraph {
    /// No FROM clause: the store's default graph (g = 0).
    StoreDefault,
    /// No FROM clause and `union_default_graph = 1`: RDF merge of g=0 and every named graph.
    UnionAll,
    /// FROM g1..gk (possibly empty: FROM NAMED only => empty default graph). Ids of graphs not
    /// in the store are TEMP ids (empty graphs). Multi-graph default graphs dedupe (s,p,o).
    Graphs(Arc<[TermId]>),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum NamedGraphs {
    /// No dataset clause: every named graph of the store.
    All,
    /// FROM NAMED list (FROM-only queries => empty list).
    Only(Arc<[TermId]>),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DatasetSpec {
    pub default: DefaultGraph,
    pub named: NamedGraphs,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum TemplateTerm {
    Const(TermId),
    Slot(Slot),
    /// Template blank node (fresh per solution).
    BNode(u32),
}

#[derive(Clone, PartialEq, Debug)]
pub enum QueryForm {
    /// Output slots in projection order (names in VarTable).
    Select(Vec<Slot>),
    Ask,
    Construct(Vec<[TemplateTerm; 3]>),
    /// Resources to describe (projected slots; spargebra binds IRIs via Extend).
    Describe(Vec<Slot>),
}

#[derive(Clone, Debug)]
pub struct LoweredQuery {
    pub form: QueryForm,
    pub root: Logical,
    pub vars: VarTable,
    pub dataset: DatasetSpec,
    /// Patterns of EXISTS expressions, indexed by ExistsId. They share the global slot layout.
    pub exists: Vec<Logical>,
    /// TEMP ids created during lowering (constants absent from the store).
    pub temps: TempDict,
    pub base_iri: Option<String>,
}
```

### `src/eval_api.rs`
```rust
//! Contracts between the expression evaluator (sq-expr) and its hosts (sq-exec, sq-oracle).
use std::sync::Arc;

use rustc_hash::FxHashMap;

use crate::error::Result;
use crate::id::{Tag, TermId};
use crate::ir::ExistsId;
use crate::term::Term;

/// Execution-local terms: computed values and query constants absent from the store.
/// Never persisted (writers reject TEMP ids). Equal terms get equal TEMP ids.
#[derive(Clone, Debug, Default)]
pub struct TempDict {
    terms: Vec<Arc<Term>>,
    index: FxHashMap<Arc<Term>, TermId>,
}

impl TempDict {
    pub fn new() -> Self {
        Self::default()
    }
    /// Caller guarantees: `t` is not inline-encodable and (if canonical ids are required) not
    /// in the store. Blank nodes passed here are treated as named (label-keyed) TEMP bnodes.
    pub fn intern(&mut self, t: Term) -> TermId {
        if let Some(id) = self.index.get(&t) {
            return *id;
        }
        let tag = match &t {
            Term::Iri(_) => Tag::Iri,
            Term::BNode(_) => Tag::BNode,
            Term::Literal(l) if l.lang.is_some() => Tag::Lang,
            Term::Literal(l) if l.is_simple() => Tag::Str,
            Term::Literal(_) => Tag::Typed,
        };
        let id = TermId::temp(self.terms.len() as u32, tag);
        let t = Arc::new(t);
        self.terms.push(t.clone());
        self.index.insert(t, id);
        id
    }
    /// A brand-new blank node (BNODE(), CONSTRUCT template bnodes): label "t<index>".
    pub fn fresh_bnode(&mut self) -> TermId {
        let idx = self.terms.len() as u32;
        let t = Arc::new(Term::BNode(format!("t{idx}").into()));
        let id = TermId::temp(idx, Tag::BNode);
        self.terms.push(t.clone());
        self.index.insert(t, id);
        id
    }
    pub fn get(&self, id: TermId) -> Option<&Arc<Term>> {
        self.terms.get(id.temp_index()? as usize)
    }
    pub fn len(&self) -> usize {
        self.terms.len()
    }
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }
}

/// Implemented by sq-exec (store dictionary + DecodeCache + TempDict) and by sq-oracle.
pub trait TermResolver {
    /// Decode any bound id: dictionary, bnode, inline or TEMP. UNBOUND/ERROR = Error::Internal.
    fn term(&mut self, id: TermId) -> Result<Arc<Term>>;
    /// Id of a computed term. `canonical = true`: inline id, else dictionary id if the term is
    /// stored, else TEMP id (required whenever the value can meet stored ids: joins, DISTINCT,
    /// GROUP keys, aggregate DISTINCT, MINUS, VALUES, sameTerm, hash keys, EXISTS seeds,
    /// CONSTRUCT dedupe). `canonical = false`: inline id, else TEMP id (output-only values).
    fn intern(&mut self, t: &Term, canonical: bool) -> Result<TermId>;
    /// Fresh TEMP blank node.
    fn fresh_bnode(&mut self) -> TermId;
}

/// EXISTS / NOT EXISTS callback: evaluate `exists[id]` with every bound slot of `row`
/// substituted (SeedMode::Substitute). Hosts memoize per tuple of correlated slots.
pub trait ExistsEval {
    fn exists(&mut self, id: ExistsId, row: &[TermId]) -> Result<bool>;
}

/// A stream of SELECT solutions in projection order (native executor or baseline SQL).
/// Result writers (sq-results) and the SQL surface (sq-api) consume only this trait.
pub trait SolutionSource {
    fn variables(&self) -> &[String];
    /// Append up to `max_rows` rows of `variables().len()` ids each (UNBOUND = 0).
    /// Returns rows appended; 0 = exhausted.
    fn next_rows(&mut self, out: &mut Vec<TermId>, max_rows: usize) -> Result<usize>;
    /// Decode ids produced by this source (dictionary, bnode, inline, TEMP); UNBOUND -> None.
    /// Implementations batch, sort and cache dictionary misses.
    fn decode(&mut self, ids: &[TermId], out: &mut Vec<Option<Arc<Term>>>) -> Result<()>;
}

/// CONSTRUCT / DESCRIBE output: a SET of triples (already deduplicated), first-seen order.
pub trait TripleSource {
    fn next_triples(&mut self, out: &mut Vec<[TermId; 3]>, max: usize) -> Result<usize>;
    fn decode(&mut self, ids: &[TermId], out: &mut Vec<Option<Arc<Term>>>) -> Result<()>;
}
```

### `src/plan.rs`
```rust
//! Physical plan: produced by sq-plan, consumed by sq-exec, printed by `sparql_explain`.
use std::ops::Bound;

use crate::batch::Slot;
use crate::eval_api::TempDict;
use crate::id::TermId;
use crate::ir::{AggSpec, DatasetSpec, Expr, GraphSel, OrderKey, PatTerm, PathPlan, QuadPat, QueryForm, VarTable};
use crate::store::IndexOrder;

/// How a re-opened subtree treats seed values.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum SeedMode {
    /// Sideways information passing (BindJoin): only legal for slots in safe_inject(subtree).
    /// Seeded slots behave as bound values that are merged into every output row.
    Join,
    /// EXISTS (spec 18.6 substitute): seeded slots behave as constants everywhere in the
    /// subtree, including OPTIONAL/MINUS/FILTER; Minus excludes seeded slots from its
    /// domain-intersection test; zero-length paths treat seeded endpoints as constants.
    Substitute,
}

#[derive(Clone, PartialEq, Debug)]
pub enum JoinKind {
    Inner,
    /// OPTIONAL: filter evaluated on the MERGED row; left row emitted alone iff no compatible
    /// right row passes the filter (errors count as false).
    Left { filter: Option<Expr> },
    /// Keep left rows with >= 1 compatible right row (decorrelated EXISTS).
    Semi,
    /// Keep left rows with no compatible right row (decorrelated NOT EXISTS).
    Anti,
}

/// Integer-range pushdown on the object of a Scan whose order has o as the first free column
/// (Gpos with g,p bound). Exact by construction: the Scan ALSO reads `rdf_gpox` for (g,p)
/// (dictionary-typed and decimal objects) and the original FILTER stays above the Scan.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct IntRange {
    pub lo: i64,
    pub hi: i64,
}

#[derive(Clone, PartialEq, Debug)]
pub struct ScanOp {
    pub pat: QuadPat,
    /// Chosen index; the prefix is the constant/seeded positions (in index order).
    pub order: IndexOrder,
    pub range: Option<IntRange>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Plan {
    /// Pre-order id, unique in the QueryPlan (EXPLAIN ANALYZE counters are keyed by it).
    pub id: u32,
    pub est_rows: f64,
    pub est_cost_ns: f64,
    /// Output sorted ascending (signed i64) by this slot, if guaranteed.
    pub sorted_by: Option<Slot>,
    pub op: Op,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Op {
    Unit,
    Empty,
    Scan(ScanOp),
    Values { slots: Vec<Slot>, rows: Vec<Vec<TermId>> },
    /// Index nested loop / sideways passing: for each distinct tuple of `seed` slots of the
    /// left input (optionally sorted for locality), re-open `right` with SeedMode::Join and
    /// merge. Runs of equal keys reuse the previous probe's result. `adaptive`: switch to a
    /// hash join when actual left rows > 8x est_rows and > 50_000.
    BindJoin { left: Box<Plan>, right: Box<Plan>, seed: Vec<Slot>, kind: JoinKind, sort_left: bool, adaptive: bool },
    /// keys = certainly-bound shared slots; check = maybe-bound shared slots.
    HashJoin { build: Box<Plan>, probe: Box<Plan>, keys: Vec<Slot>, check: Vec<Slot>, kind: JoinKind, build_is_left: bool },
    /// Both inputs sorted by `key` (certainly bound on both sides). Galloping seek after
    /// `gallop` consecutive misses when the right input is a Scan.
    MergeJoin { left: Box<Plan>, right: Box<Plan>, key: Slot, check: Vec<Slot>, gallop: u32 },
    /// Leapfrog intersection on one slot over k inputs all sorted by it (subject stars).
    StarMerge { key: Slot, inputs: Vec<Plan>, check: Vec<Slot> },
    NestedLoop { left: Box<Plan>, right: Box<Plan>, check: Vec<Slot>, kind: JoinKind },
    /// SPARQL MINUS: remove a left row iff some right row is compatible AND shares at least
    /// one bound slot with it. Right evaluated independently. keys/check as in HashJoin.
    Minus { left: Box<Plan>, right: Box<Plan>, keys: Vec<Slot>, check: Vec<Slot> },
    Union(Vec<Plan>),
    Filter { input: Box<Plan>, expr: Expr },
    Extend { input: Box<Plan>, slot: Slot, expr: Expr, canonical: bool },
    Path { g: GraphSel, s: PatTerm, path: PathPlan, o: PatTerm },
    /// GRAPH ?g that cannot be pushed into patterns: for each named graph of the dataset (sorted),
    /// push it on the graph register stack, re-open `input`, drop rows whose `slot` is bound
    /// to a different graph, set `slot`.
    GraphLoop { slot: Slot, input: Box<Plan> },
    /// GRAPH ?g {} : one row per named graph of the dataset.
    GraphNames { slot: Slot },
    /// Hash (or streaming, when input sorted by keys[0] and keys.len()==1) aggregation.
    /// Group keys that evaluate to an error use TermId::ERROR (own group; output UNBOUND).
    Group { input: Box<Plan>, keys: Vec<Slot>, aggs: Vec<(Slot, AggSpec)>, streaming: bool },
    /// COUNT(*) without DISTINCT/GROUP BY: one row binding `out`. Fast paths: input Scan ->
    /// store.count() in the VDBE; input MergeJoin -> factorized sum of run products.
    Count { input: Box<Plan>, out: Slot },
    /// Full SPARQL ORDER BY (stable; precomputed SortKeys).
    Sort { input: Box<Plan>, keys: Vec<OrderKey> },
    /// Enforcer for merge joins: stable sort by raw i64 of `key`.
    SortBySlot { input: Box<Plan>, key: Slot },
    /// ORDER BY + LIMIT: bounded heap of k = offset + limit; never placed across Distinct.
    TopK { input: Box<Plan>, keys: Vec<OrderKey>, k: u64 },
    Distinct { input: Box<Plan>, slots: Vec<Slot> },
    Reduced { input: Box<Plan>, slots: Vec<Slot> },
    Slice { input: Box<Plan>, offset: u64, limit: Option<u64> },
    /// Keeps `slots`, resets all others to UNBOUND.
    Project { input: Box<Plan>, slots: Vec<Slot> },
    /// Hybrid: a left-deep join pipeline rendered as ONE SQL statement (CROSS JOIN + INDEXED BY,
    /// order fixed by our planner) and stepped by SQLite's VDBE; result columns -> `out` slots.
    SqlJoin { sql: String, params: Vec<i64>, out: Vec<Slot> },
}

impl Plan {
    pub fn children(&self) -> Vec<&Plan> {
        match &self.op {
            Op::Unit | Op::Empty | Op::Scan(_) | Op::Values { .. } | Op::Path { .. } | Op::GraphNames { .. } | Op::SqlJoin { .. } => vec![],
            Op::BindJoin { left, right, .. }
            | Op::MergeJoin { left, right, .. }
            | Op::NestedLoop { left, right, .. }
            | Op::Minus { left, right, .. } => vec![left, right],
            Op::HashJoin { build, probe, .. } => vec![build, probe],
            Op::StarMerge { inputs, .. } | Op::Union(inputs) => inputs.iter().collect(),
            Op::Filter { input, .. }
            | Op::Extend { input, .. }
            | Op::GraphLoop { input, .. }
            | Op::Group { input, .. }
            | Op::Count { input, .. }
            | Op::Sort { input, .. }
            | Op::SortBySlot { input, .. }
            | Op::TopK { input, .. }
            | Op::Distinct { input, .. }
            | Op::Reduced { input, .. }
            | Op::Slice { input, .. }
            | Op::Project { input, .. } => vec![input],
        }
    }
}

#[derive(Clone, Debug)]
pub struct QueryPlan {
    pub form: QueryForm,
    pub root: Plan,
    pub vars: VarTable,
    pub dataset: DatasetSpec,
    /// Named graphs of the dataset, resolved at plan time (sorted).
    pub named_graphs: Vec<TermId>,
    /// EXISTS sub-plans indexed by ExistsId (opened with SeedMode::Substitute).
    pub exists: Vec<Plan>,
    pub temps: TempDict,
    pub base_iri: Option<String>,
}

impl QueryPlan {
    pub fn width(&self) -> usize {
        self.vars.width()
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum EngineKind {
    #[default]
    Native,
    /// Baseline SPARQL->SQL translation (sq-sqlgen).
    Sql,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum JoinPolicy {
    #[default]
    Auto,
    PreferHash,
    PreferBind,
    PreferMerge,
    /// Never use SqlJoin (differential tests / memstore).
    NoSql,
}

#[derive(Clone, Debug)]
pub struct QueryOptions {
    pub engine: EngineKind,
    pub timeout_ms: Option<u64>,
    pub max_memory_mb: u64,
    pub union_default_graph: bool,
    pub join_policy: JoinPolicy,
    /// false = keep syntactic join order (differential testing).
    pub reorder: bool,
    pub base_iri: Option<String>,
    /// Max runtime bounded counts per query and their cap.
    pub max_probe_counts: u32,
    pub probe_count_cap: u64,
}

impl Default for QueryOptions {
    fn default() -> Self {
        QueryOptions {
            engine: EngineKind::Native,
            timeout_ms: None,
            max_memory_mb: 1024,
            union_default_graph: false,
            join_policy: JoinPolicy::Auto,
            reorder: true,
            base_iri: None,
            max_probe_counts: 32,
            probe_count_cap: 1000,
        }
    }
}

/// Per-plan-node runtime counters (EXPLAIN ANALYZE), indexed by Plan::id.
#[derive(Clone, Debug, Default)]
pub struct NodeProfile {
    pub rows_out: u64,
    pub opens: u64,
    pub seeks: u64,
    pub time_ns: u64,
}

#[derive(Clone, Debug, Default)]
pub struct ExecProfile {
    pub nodes: Vec<NodeProfile>,
    pub decode_misses: u64,
    pub decode_hits: u64,
    pub plan_ns: u64,
    pub exec_ns: u64,
}

/// Helper for templates/tests: an unbounded range.
pub const UNBOUNDED: (Bound<i64>, Bound<i64>) = (Bound::Unbounded, Bound::Unbounded);
```

### `src/testing.rs` (feature `testing`)
```rust
//! Shared test utilities (feature "testing"): deterministic random quads and the QuadCursor
//! conformance suite run against MemStore, SqliteStore (SQL cursors) and the phase-2 reader.
use std::ops::Bound;
use std::sync::Arc;

use crate::codec::encode_decimal;
use crate::error::{Error, Result};
use crate::id::{Tag, TermId};
use crate::stats::{CostConstants, Stats, MEMORY_COSTS};
use crate::store::*;
use crate::term::Term;

/// xorshift64* — deterministic, dependency-free.
pub struct Rng(pub u64);
impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n.max(1)
    }
}

/// A small-domain random term id covering every storable tag (incl. negative ints, so the
/// signed order matters, and tags 5/8 so rdf_gpox is exercised).
pub fn random_id(r: &mut Rng, domain: i64) -> TermId {
    let v = (r.below(domain as u64) as i64) + 1;
    match r.below(8) {
        0..=2 => TermId::from_dict(1024 + v, Tag::Iri),
        3 => TermId::from_int(v - domain / 2).unwrap(),
        4 => TermId::from_dict(1024 + v, Tag::Typed),
        5 => encode_decimal(v - domain / 2, 1, false),
        6 => TermId::bnode(v),
        _ => TermId::from_dict(1024 + v, Tag::Str),
    }
}

/// Random quads: g in {0, a few named graphs}, s/p/o from small domains (many duplicates
/// prefixes, so prefix scans return several rows). Sorted, deduplicated, [g,s,p,o].
pub fn random_quads(seed: u64, n: usize, named: bool) -> Vec<[TermId; 4]> {
    let mut r = Rng(seed | 1);
    let mut v: Vec<[TermId; 4]> = (0..n)
        .map(|_| {
            let g = if named && r.below(3) == 0 { TermId::from_dict(2000 + r.below(3) as i64, Tag::Iri) } else { TermId::DEFAULT_GRAPH };
            let s = if r.below(4) == 0 { TermId::bnode(1 + r.below(20) as i64) } else { TermId::from_dict(1024 + r.below(40) as i64, Tag::Iri) };
            let p = TermId::from_dict(1100 + r.below(6) as i64, Tag::Iri);
            let o = random_id(&mut r, 50);
            [g, s, p, o]
        })
        .collect();
    v.sort();
    v.dedup();
    v
}

fn project(q: &[TermId; 4], spec: &ScanSpec) -> Vec<TermId> {
    spec.free_cols().iter().map(|c| q[*c as usize]).collect()
}

/// Brute-force expected rows for a spec.
pub fn expected_rows(quads: &[[TermId; 4]], spec: &ScanSpec) -> Vec<Vec<TermId>> {
    let mut rows: Vec<Vec<TermId>> = quads.iter().filter(|q| spec.matches(q)).map(|q| project(q, spec)).collect();
    rows.sort();
    rows.dedup();
    rows
}

fn drain(c: &mut dyn QuadCursor, max: usize) -> Result<Vec<Vec<TermId>>> {
    let w = c.width();
    let mut buf = Vec::new();
    let mut rows = 0;
    loop {
        let n = c.fill(&mut buf, max)?;
        if n == 0 {
            break;
        }
        rows += n;
    }
    assert_eq!(buf.len(), rows * w, "fill must append width() values per row");
    Ok((0..rows).map(|i| buf[i * w..(i + 1) * w].to_vec()).collect())
}

fn random_bound(r: &mut Rng, rows: &[Vec<TermId>]) -> Bound<i64> {
    let v = if rows.is_empty() || r.below(4) == 0 { (r.next_u64() as i64) >> 4 } else { rows[r.below(rows.len() as u64) as usize][0].0 };
    match r.below(3) {
        0 => Bound::Unbounded,
        1 => Bound::Included(v),
        _ => Bound::Excluded(v),
    }
}

/// The conformance suite. `store` must contain exactly `quads` (plus nothing else).
/// Checks, for every order x prefix_len (0..=4) x random prefixes (present and absent) x
/// random ranges: full drains with several fill sizes, forward/backward seeks with 1..=width
/// key columns, and rebind. Panics with a descriptive message on the first mismatch.
pub fn cursor_conformance(store: &dyn QuadStore, quads: &[[TermId; 4]], seed: u64, iterations: usize) {
    let mut r = Rng(seed | 1);
    for order in IndexOrder::ALL {
        for prefix_len in 0..=4u8 {
            for _ in 0..iterations {
                // prefix taken from a real quad (or perturbed to be absent)
                let mut prefix = [TermId::UNBOUND; 4];
                if !quads.is_empty() {
                    let q = quads[r.below(quads.len() as u64) as usize];
                    for i in 0..prefix_len as usize {
                        prefix[i] = q[order.cols()[i] as usize];
                    }
                    if r.below(5) == 0 && prefix_len > 0 {
                        prefix[prefix_len as usize - 1] = TermId(prefix[prefix_len as usize - 1].0 + 16);
                    }
                }
                let mut spec = ScanSpec::new(order, &prefix[..prefix_len as usize]);
                if prefix_len < 4 && r.below(2) == 0 {
                    let all = expected_rows(quads, &spec);
                    spec.lower = random_bound(&mut r, &all);
                    spec.upper = random_bound(&mut r, &all);
                }
                let want = expected_rows(quads, &spec);
                let mut cur = store.cursor(&spec).unwrap_or_else(|e| panic!("cursor({spec:?}): {e}"));
                assert_eq!(cur.width(), 4 - prefix_len as usize, "width {spec:?}");
                let max = [1usize, 3, 1024][r.below(3) as usize];
                let got = drain(cur.as_mut(), max).unwrap();
                assert_eq!(got, want, "drain mismatch for {spec:?} (fill max {max})");
                let cnt = store.count(&spec, None).unwrap();
                assert_eq!(cnt as usize, want.len(), "count mismatch for {spec:?}");
                let cap = r.below(5);
                assert_eq!(store.count(&spec, Some(cap)).unwrap(), (want.len() as u64).min(cap + 1), "capped count {spec:?}");
                // seeks (forward and backward), 1..=width key columns
                if prefix_len < 4 {
                    for _ in 0..4 {
                        let klen = 1 + r.below((4 - prefix_len) as u64) as usize;
                        let key: Vec<TermId> = if !want.is_empty() && r.below(3) != 0 {
                            want[r.below(want.len() as u64) as usize][..klen].to_vec()
                        } else {
                            (0..klen).map(|_| random_id(&mut r, 60)).collect()
                        };
                        cur.seek(&key).unwrap();
                        let got = drain(cur.as_mut(), 7).unwrap();
                        let exp: Vec<Vec<TermId>> = want.iter().filter(|row| row[..klen] >= key[..]).cloned().collect();
                        assert_eq!(got, exp, "seek({key:?}) mismatch for {spec:?}");
                    }
                }
                // rebind to another prefix of the same length
                if !quads.is_empty() {
                    let q = quads[r.below(quads.len() as u64) as usize];
                    let mut p2 = [TermId::UNBOUND; 4];
                    for i in 0..prefix_len as usize {
                        p2[i] = q[order.cols()[i] as usize];
                    }
                    let spec2 = ScanSpec { prefix: p2, ..spec };
                    cur.rebind(&p2[..prefix_len as usize], spec.lower, spec.upper).unwrap();
                    let got = drain(cur.as_mut(), 1024).unwrap();
                    assert_eq!(got, expected_rows(quads, &spec2), "rebind mismatch for {spec2:?}");
                }
            }
        }
    }
}

/// Reference store used to self-test the suite (sorted Vec + brute force). Dictionary only
/// knows inline/bnode ids.
pub struct BruteStore {
    pub quads: Arc<Vec<[TermId; 4]>>,
}

struct BruteCursor {
    quads: Arc<Vec<[TermId; 4]>>,
    spec: ScanSpec,
    rows: Vec<Vec<TermId>>,
    pos: usize,
    seeks: u64,
}

impl QuadCursor for BruteCursor {
    fn width(&self) -> usize {
        4 - self.spec.prefix_len as usize
    }
    fn fill(&mut self, out: &mut Vec<TermId>, max_rows: usize) -> Result<usize> {
        let end = (self.pos + max_rows).min(self.rows.len());
        for r in &self.rows[self.pos..end] {
            out.extend_from_slice(r);
        }
        let n = end - self.pos;
        self.pos = end;
        Ok(n)
    }
    fn seek(&mut self, key: &[TermId]) -> Result<()> {
        self.seeks += 1;
        self.pos = self.rows.partition_point(|r| r[..key.len()] < *key);
        Ok(())
    }
    fn rebind(&mut self, prefix: &[TermId], lower: Bound<i64>, upper: Bound<i64>) -> Result<()> {
        let mut p = [TermId::UNBOUND; 4];
        p[..prefix.len()].copy_from_slice(prefix);
        self.spec = ScanSpec { prefix: p, lower, upper, ..self.spec };
        self.rows = expected_rows(&self.quads, &self.spec);
        self.pos = 0;
        Ok(())
    }
    fn counters(&self) -> CursorCounters {
        CursorCounters { rows: self.pos as u64, seeks: self.seeks, fills: 0, reader: ReaderKind::Memory }
    }
}

struct NoDict;
impl Dictionary for NoDict {
    fn lookup(&self, t: oxrdf::TermRef<'_>) -> Result<Option<TermId>> {
        Ok(crate::codec::encode_inline(t))
    }
    fn decode(&self, ids: &[TermId], out: &mut Vec<Arc<Term>>) -> Result<()> {
        for id in ids {
            out.push(Arc::new(crate::codec::decode_inline(*id).ok_or_else(|| Error::internal("BruteStore cannot decode"))?));
        }
        Ok(())
    }
}

impl QuadStore for BruteStore {
    fn cursor(&self, spec: &ScanSpec) -> Result<Box<dyn QuadCursor>> {
        Ok(Box::new(BruteCursor { quads: self.quads.clone(), spec: *spec, rows: expected_rows(&self.quads, spec), pos: 0, seeks: 0 }))
    }
    fn count(&self, spec: &ScanSpec, cap: Option<u64>) -> Result<u64> {
        let n = expected_rows(&self.quads, spec).len() as u64;
        Ok(cap.map_or(n, |c| n.min(c + 1)))
    }
    fn dict(&self) -> &dyn Dictionary {
        &NoDict
    }
    fn named_graphs(&self) -> Result<Vec<TermId>> {
        let mut g: Vec<TermId> = self.quads.iter().map(|q| q[0]).filter(|g| g.is_bound()).collect();
        g.sort();
        g.dedup();
        Ok(g)
    }
    fn stats(&self) -> Option<Arc<Stats>> {
        None
    }
    fn costs(&self) -> &CostConstants {
        &MEMORY_COSTS
    }
    fn caps(&self) -> StoreCaps {
        StoreCaps::default()
    }
}
```

### `src/tests.rs`
```rust
use crate::codec::*;
use crate::id::{Kind, Tag, TermId};
use crate::store::{IndexOrder, QuadStore};
use crate::testing::{cursor_conformance, random_quads, BruteStore, Rng};
use oxrdf::{Literal, NamedNode, TermRef};

fn lit(lex: &str, dt: &str) -> oxrdf::Term {
    Literal::new_typed_literal(lex, NamedNode::new(format!("{XSD}{dt}")).unwrap()).into()
}

#[test]
fn inline_roundtrip_and_canonicality() {
    let inline = [
        ("0", "integer"), ("-5", "integer"), ("576460752303423487", "integer"), ("-576460752303423488", "integer"),
        ("true", "boolean"), ("false", "boolean"),
        ("0", "decimal"), ("-12", "decimal"), ("5.0", "decimal"), ("0.0", "decimal"), ("-0.5", "decimal"),
        ("12.25", "decimal"), ("0.000000000000001", "decimal"),
        ("2020-01-31T23:59:59Z", "dateTime"), ("1999-12-31T00:00:00.5+05:30", "dateTime"),
        ("0001-01-01T00:00:00", "dateTime"), ("2024-02-29T12:00:00.125-14:00", "dateTime"),
        ("2020-01-01T00:00:00+00:00", "dateTime"),
        ("2020-02-29", "date"), ("1970-01-01Z", "date"), ("0001-01-01-01:30", "date"),
        ("", "string"), ("abc", "string"), ("1234567", "string"), ("é€", "string"),
    ];
    for (lex, dt) in inline {
        let t = lit(lex, dt);
        let id = encode_inline(t.as_ref()).unwrap_or_else(|| panic!("{lex}^^{dt} should inline"));
        assert!(id.is_inline(), "{lex}");
        let back = decode_inline(id).unwrap();
        assert_eq!(back.to_oxrdf(), t, "{lex}^^{dt} round trip");
        assert_eq!(id.kind(), Kind::Literal);
    }
    let not_inline = [
        ("01", "integer"), ("+1", "integer"), ("-0", "integer"), ("576460752303423488", "integer"), ("1.0", "integer"),
        ("1", "boolean"), ("TRUE", "boolean"),
        ("-0", "decimal"), ("-0.0", "decimal"), ("1.50", "decimal"), ("+1.5", "decimal"), (".5", "decimal"),
        ("5.", "decimal"), ("01.5", "decimal"), ("0.0000000000000001", "decimal"),
        ("2020-01-01T00:00:00.000Z", "dateTime"), ("2020-01-01T24:00:00Z", "dateTime"), ("2020-02-30T00:00:00", "dateTime"),
        ("2020-01-01T00:00:00-00:00", "dateTime"), ("2020-01-01T00:00:00.1234Z", "dateTime"),
        ("2020-01-01T00:00:00+14:30", "dateTime"), ("6001-01-01T00:00:00", "dateTime"), ("-0001-01-01T00:00:00", "dateTime"),
        ("2021-02-29", "date"), ("20201-01-01", "date"),
        ("12345678", "string"),
        ("1.0E0", "double"), ("x", "anyURI"),
    ];
    for (lex, dt) in not_inline {
        assert!(encode_inline(lit(lex, dt).as_ref()).is_none(), "{lex}^^{dt} must NOT inline");
    }
    // language-tagged literals never inline
    let l: oxrdf::Term = Literal::new_language_tagged_literal("ab", "en").unwrap().into();
    assert!(encode_inline(l.as_ref()).is_none());
}

#[test]
fn int_order_is_signed_monotonic() {
    let mut r = Rng(7);
    let mut vals: Vec<i64> = (0..1000).map(|_| (r.next_u64() as i64) >> 5).collect();
    vals.extend([TermId::INT_MIN, TermId::INT_MAX, -1, 0, 1]);
    vals.sort();
    let ids: Vec<TermId> = vals.iter().map(|v| TermId::from_int(*v).unwrap()).collect();
    assert!(ids.windows(2).all(|w| w[0] <= w[1]));
    let (lo, hi) = TermId::int_raw_range(-3, 3);
    assert_eq!(TermId(lo).as_int(), Some(-3));
    assert_eq!(TermId(hi).as_int(), Some(3));
}

#[test]
fn tags_are_disjoint() {
    assert_eq!(TermId::temp(5, Tag::Lang).temp_index(), Some(5));
    assert_eq!(TermId::temp(5, Tag::Lang).temp_tag(), Tag::Lang);
    assert_eq!(TermId::temp(9, Tag::BNode).kind(), Kind::BNode);
    assert!(TermId::temp(9, Tag::BNode).is_bnode());
    assert!(!TermId::temp(1, Tag::Iri).is_storable());
    assert!(!TermId::ERROR.is_storable());
    assert!(TermId::bnode(3).is_storable() && !TermId::bnode(3).is_dict());
    assert_eq!(XSD_INTEGER.dict_row(), 2);
    assert_eq!(parse_bnode_label(&bnode_label(TermId::bnode(0x1f))), Some(TermId::bnode(0x1f)));
    assert_eq!(parse_bnode_label("b01"), None);
}

#[test]
fn golden_term_hashes() {
    let h1 = term_hash(Tag::Iri, "http://example.org/a", "");
    let h2 = term_hash(Tag::Str, "a", "");
    let h3 = term_hash(Tag::Lang, "a", "en");
    let h4 = term_hash(Tag::Typed, "1.0E0", "http://www.w3.org/2001/XMLSchema#double");
    println!("GOLDEN {h1} {h2} {h3} {h4}");
    assert_eq!([h1, h2, h3, h4], GOLDEN);
    let k = TermKey::of(TermRef::from(NamedNode::new("http://example.org/a").unwrap().as_ref())).unwrap();
    assert_eq!(k.hash64(), h1);
}
const GOLDEN: [i64; 4] = [-5228397991068580721, 9093457548028347866, 227138330714864330, 614094328781894807];

#[test]
fn candidates_cover_every_shape() {
    for mask in 0..16u8 {
        let bound = [mask & 1 != 0, mask & 2 != 0, mask & 4 != 0, mask & 8 != 0];
        let c = IndexOrder::candidates(bound);
        assert!(!c.is_empty(), "no order for {bound:?}");
    }
    assert!(IndexOrder::candidates([true, false, true, false]).iter().any(|(o, _)| *o == IndexOrder::Gpso));
}

#[test]
fn conformance_suite_self_test() {
    let quads = random_quads(42, 400, true);
    let store = BruteStore { quads: std::sync::Arc::new(quads.clone()) };
    cursor_conformance(&store, &quads, 9, 6);
    assert!(!store.named_graphs().unwrap().is_empty());
}
```

---

## 11. Per-crate public APIs (committed as `todo!()` stubs on day 0 so the workspace builds)

```rust
// ---------------- sq-memstore
pub struct MemStore { /* Rc<RefCell<Inner>>: 9 BTreeSet<[TermId;4]> keyed in index order (Gpox filtered),
                         dictionary Vec<Term> + FxHashMap<TermKey,TermId> pre-filled with WELL_KNOWN,
                         next_bnode, graph registry, Option<Arc<Stats>> */ }
impl MemStore {
    pub fn new() -> Self;
    pub fn quads(&self) -> Vec<[TermId; 4]>;
    pub fn analyze(&self) -> Arc<Stats>;            // same algorithm as sq-sqlite (shared semantics, hints only)
    pub fn set_costs(&self, c: CostConstants);
}
impl QuadStore for MemStore {}      // cursors snapshot the matching range (Arc<Vec>) at create/seek/rebind
impl QuadStoreWriter for MemStore {}

// ---------------- sq-sqlite
pub struct RawDb(std::ptr::NonNull<rusqlite::ffi::sqlite3>);         // non-owning, Copy
impl RawDb { pub unsafe fn from_ptr(p: *mut rusqlite::ffi::sqlite3) -> Self;
             pub fn txn_state(&self) -> i32; pub fn autocommit(&self) -> bool;
             pub fn exec(&self, sql: &str) -> sq_core::Result<()>; }
pub struct StorageConfig { pub decode_cache_mb: u64, pub lookup_cache_entries: usize }
pub enum SchemaState { Absent, Ready { schema_version: i64 }, Incompatible(String) }
pub struct StorageShared { /* StmtPool, DecodeCache, LookupCache, Option<Arc<Stats>>, Epoch, committed_max_row, armed */ }
impl StorageShared {
    pub fn new(db: RawDb, cfg: StorageConfig) -> Rc<Self>;
    pub fn validate_epoch(&self) -> sq_core::Result<SchemaState>;   // §4.4, once per API entry
    pub fn arm_pool(&self); pub fn is_armed(&self) -> bool; pub fn disarm_and_finalize(&self);
    pub fn status(&self) -> Vec<(String, String)>;
}
#[derive(Clone)] pub struct SqliteStore(Rc<StorageShared>);
impl SqliteStore {
    pub fn new(shared: Rc<StorageShared>) -> Self;
    pub fn ensure_schema(&self) -> sq_core::Result<i64>;                     // sparql_init
    pub fn read_scope(&self, depth: u32) -> sq_core::Result<ReadScope>;      // RAII, §8.6
    pub fn write_scope(&self, depth: u32) -> sq_core::Result<WriteScope>;    // RAII; commit() / drop = rollback
    pub fn bulk_load(&self, src: LoadSource<'_>, opts: &LoadOptions) -> sq_core::Result<LoadReport>;   // §2.3
    pub fn analyze(&self) -> sq_core::Result<Arc<Stats>>;
    pub fn vacuum_terms(&self) -> sq_core::Result<u64>;
    pub fn check(&self) -> sq_core::Result<String>;
    pub fn explain_query_plan(&self, sql: &str) -> sq_core::Result<String>;
}
pub enum LoadSource<'a> { Bytes(&'a [u8]), File(&'a std::path::Path) }
pub struct LoadOptions { pub format: oxrdfio::RdfFormat, pub base_iri: Option<String>,
                         pub graph: Option<String>, pub bulk: BulkMode }
pub enum BulkMode { Auto, Never, Always }
pub struct LoadReport { pub quads_read: u64, pub quads_new: u64, pub terms_new: u64, pub bulk: bool, pub ms: u64 }
impl QuadStore for SqliteStore {}  impl QuadStoreWriter for SqliteStore {}

// ---------------- sq-expr: see §6.1

// ---------------- sq-plan
pub fn parse_query(text: &str, base: Option<&str>) -> sq_core::Result<spargebra::Query>;
pub fn parse_update(text: &str, base: Option<&str>) -> sq_core::Result<spargebra::Update>;
pub fn lower_query(q: &spargebra::Query, store: &dyn QuadStore, opts: &QueryOptions) -> sq_core::Result<LoweredQuery>;
/// Update WHERE: returns the lowered pattern (form = Select(all in-scope slots)) and the variable map.
pub fn lower_pattern(p: &spargebra::algebra::GraphPattern, using: Option<&spargebra::algebra::QueryDataset>,
                     base: Option<&str>, store: &dyn QuadStore, opts: &QueryOptions)
    -> sq_core::Result<(LoweredQuery, FxHashMap<spargebra::term::Variable, Slot>)>;
pub struct Props { pub vars: SlotSet, pub certain: SlotSet, pub maybe: SlotSet, pub safe_inject: SlotSet }
pub fn props(l: &Logical, exists: &[Logical]) -> Props;
pub fn optimize(q: LoweredQuery, store: &dyn QuadStore, opts: &QueryOptions) -> sq_core::Result<QueryPlan>;
pub fn plan_query(q: &spargebra::Query, store: &dyn QuadStore, opts: &QueryOptions) -> sq_core::Result<QueryPlan>;
pub fn validate(p: &QueryPlan) -> sq_core::Result<()>;
pub enum ExplainMode { Algebra, Logical, Plan, Analyze }
pub fn explain_logical(q: &LoweredQuery) -> String;
pub fn explain(p: &QueryPlan, profile: Option<&ExecProfile>) -> String;

// ---------------- sq-exec
pub struct ExecCaches { /* handles to connection DecodeCache/LookupCache; Default = none (MemStore) */ }
pub struct Execution { /* owns operator tree, Rc<dyn QuadStore>, ExecCtx, form, projection */ }
impl Execution {
    pub fn new(store: Rc<dyn QuadStore>, plan: QueryPlan, opts: &QueryOptions, caches: ExecCaches) -> sq_core::Result<Self>;
    pub fn form(&self) -> &QueryForm;
    pub fn ask(&mut self) -> sq_core::Result<bool>;
    pub fn count(&mut self) -> sq_core::Result<u64>;            // solutions (or triples): no decode
    pub fn profile(&self) -> ExecProfile;
}
impl SolutionSource for Execution {}   // SELECT (and ASK as 0 vars / 1 row)
impl TripleSource for Execution {}     // CONSTRUCT / DESCRIBE

// ---------------- sq-oracle
pub enum OracleOutput { Solutions { vars: Vec<String>, rows: Vec<Vec<Option<Arc<Term>>>> }, Boolean(bool), Graph(Vec<[Arc<Term>; 3]>) }
/// Deliberately naive, spec-transcribed (§18 of SPARQL 1.1): bottom-up bags, nested loops, explicit
/// substitution for EXISTS, per-graph evaluation of GRAPH, ALP for paths. Uses sq-expr for expressions.
pub fn evaluate(q: &LoweredQuery, store: &dyn QuadStore) -> sq_core::Result<OracleOutput>;

// ---------------- sq-results
pub enum ResultFormat { Json, Xml, Csv, Tsv, NTriples, NQuads, Turtle }
impl ResultFormat { pub fn parse(s: &str) -> sq_core::Result<Self>; }
pub fn write_solutions(src: &mut dyn SolutionSource, fmt: ResultFormat, w: &mut dyn std::io::Write) -> sq_core::Result<()>;
pub fn write_boolean(b: bool, fmt: ResultFormat, w: &mut dyn std::io::Write) -> sq_core::Result<()>;
pub fn write_triples(src: &mut dyn TripleSource, fmt: ResultFormat, w: &mut dyn std::io::Write) -> sq_core::Result<()>;
pub enum SqlNative { Null, Int(i64), Real(f64), Text(String) }
pub fn sql_native(t: Option<&Term>) -> SqlNative;                               // §8.4
pub fn solution_json(vars: &[String], row: &[Option<Arc<Term>>]) -> String;     // SPARQL-JSON bindings object

// ---------------- sq-sqlgen
pub struct SqlQuery { pub sql: String, pub params: Vec<i64>, pub columns: Vec<String> }
pub fn translate(q: &LoweredQuery) -> sq_core::Result<SqlQuery>;

// ---------------- sq-engine
pub fn prepare(store: Rc<dyn QuadStore>, query: &spargebra::Query, opts: &QueryOptions, caches: ExecCaches)
    -> sq_core::Result<Execution>;                                   // lower + optimize + validate(debug) + Execution::new
pub fn query_to_string(store: Rc<dyn QuadStore>, query: &str, fmt: ResultFormat, opts: &QueryOptions) -> sq_core::Result<String>;
pub fn explain(store: Rc<dyn QuadStore>, query: &str, mode: ExplainMode, opts: &QueryOptions) -> sq_core::Result<String>;
pub struct UpdateReport { pub inserted: u64, pub deleted: u64 }
/// Caller provides atomicity (write scope). Rc<dyn QuadStoreWriter> upcasts to Rc<dyn QuadStore> (Rust >= 1.86).
pub fn update(store: Rc<dyn QuadStoreWriter>, update: &spargebra::Update, opts: &QueryOptions) -> sq_core::Result<UpdateReport>;
pub fn load(store: &dyn QuadStoreWriter, data: &[u8], format: oxrdfio::RdfFormat, base: Option<&str>, graph: Option<&str>)
    -> sq_core::Result<u64>;                                         // generic loader (MemStore, small inserts)

// ---------------- sq-api
pub fn register(conn: &rusqlite::Connection) -> rusqlite::Result<()>;
```

---

## 12. Testing, the W3C runner and the benchmark

### 12.1 Per-crate independent tests
| Crate | Independent tests (no other crate's implementation needed) |
|---|---|
| sq-core | codec round trip and non-canonical rejection, signed INT monotonicity, tag disjointness, golden hashes, `candidates` coverage, conformance self-test (§10, passing) |
| sq-memstore | `cursor_conformance` on random quads (named graphs, all tags), plus deletes and re-inserts; dictionary lookup, decode and WELL_KNOWN rows |
| sq-sqlite | conformance suite on `:memory:` **and** a file DB (100K quads, named graphs, deletes and re-inserts); `EXPLAIN QUERY PLAN` assertions for every StmtKey; loader round trip for every format; bulk vs incremental give identical tables; epoch, rollback and cache-admission scenarios (BEGIN; load; query; ROLLBACK; load other; query); probe < 1.5 µs and 2-column scan < 150 ns/row in a held read txn; 1M-triple bulk load < 10 s; `sqlite3_libversion_number()==3045001` |
| sq-expr | table tests from XPath F&O and SPARQL §17 (expected terms copied from W3C `functions/`, `cast/`, `aggregates/`, `open-world/` results); the `=` matrix against open-eq-07/08; NaN; SortKey totality (random mixed values: `sort_by` never panics, and the order is transitive on sampled triples); `py/value_diff.py` against pyoxigraph on random arithmetic and casts (numerics normalized) |
| sq-plan | lower every `.rq` in sparql10/11 without panics; insta-style IR snapshots; property tests (`safe_inject ⊆ vars`, `certain ⊆ maybe`); golden `explain` on MemStore with fake stats (star → SqlJoin or merge, chain → MergeJoin, unselective → Hash); `validate()` on every plan; named regression shapes: graph-minus → GraphLoop, `GRAPH ?g {VALUES}` → GraphLoop, `GRAPH <absent> {}` → Empty |
| sq-exec | hand-built `Plan`s over MemStore (written on day 1, needing no planner); every operator × kind × seed mode; MINUS domain rule; GraphLoop register; paths (has_node, cycles, `p*` both free); factorized count equals iterated count |
| sq-oracle | W3C query evaluation on MemStore (the oracle should pass ≥ 95% on its own) |
| sq-results | golden SPARQL JSON/XML/CSV/TSV; native mapping table |
| sq-sqlgen | golden SQL; on small graphs, results equal sq-engine's |
| sq-engine | **differential fuzzer**: random graphs (all tags, non-canonical numerics, lang strings, named graphs, bnodes) and random queries (BGP, OPTIONAL±filter, UNION, MINUS, [NOT] EXISTS, BIND, VALUES, GROUP/HAVING, ORDER BY+LIMIT, paths, GRAPH, sub-select). The oracle must equal native under `{Auto, PreferHash, PreferBind, PreferMerge, NoSql} × {reorder on, off}` on MemStore **and** on SqliteStore (bag equality modulo blank-node bijection). |
| sq-api | native tests via `register(&Connection)` on system SQLite: every function and vtab; `col_used`; open-cursor write refusal; re-entrancy (baseline UDFs during stepping); savepoint rollback on error; interrupt from another thread; `temp.` view |
| py/tests | load the cdylib into Python 3.45.1; close without a zombie (WAL file removed); panic safety (a debug-only `sparql_config('debug_panic',…)` makes xFilter panic → SQL error, the process survives); transactions; triggers rejecting DIRECTONLY functions |

### 12.2 W3C runner (`tools/sq-w3c`)
- **Manifests.** Walk `sparql11/manifest-all.ttl` and `sparql10/manifest-evaluation.ttl` with oxttl, following `mf:include` and `mf:entries`. sparql11 has 331 QueryEvaluationTests and 94 UpdateEvaluationTests.
- **Backends.** `--backend sqlite` uses a fresh `:memory:` rusqlite connection with `sq_api::register`, the full stack minus the cdylib. `--backend mem` uses MemStore with sq-engine. `--backend oracle` uses sq-oracle.
- **Data and names.** Local files are named `https://w3c.github.io/rdf-tests/sparql/<path>`, which is both the base IRI and the graph name.
  - `qt:data` goes into the default graph and `qt:graphData` into a named graph.
  - If the query has FROM/FROM NAMED and the test has no `qt:data`, the referenced files are preloaded as named graphs (dataset-09b, dup-bnode).
  - Updates use `ut:data` and `ut:graphData` with `ut:graph`.
- **Comparison.**
  - SELECT: parse `.srx`, `.srj`, `.tsv`, `.csv`, and `.ttl`/`.rdf` `rs:ResultSet` with sparesults/oxrdfio; check the variable sets.
  - Ground rows are compared as a multiset.
  - Blank-node rows use a **result-set-wide** bnode bijection (backtracking, pruned by occurrence profiles).
  - `mf:LaxCardinality` uses set comparison.
  - If the top algebra under Project/Distinct/Slice is OrderBy: when the ORDER BY keys are projected, runs of equal keys are compared in order; otherwise the exact sequence is compared under the bijection.
  - ASK compares the boolean. CONSTRUCT, DESCRIBE and update results compare `oxrdf::Dataset`s after `canonicalize`.
  - CSV/TSV format tests compare text after bnode relabeling.
  - **Pass 1** is exact. **Pass 2** (numerics only: same datatype and equal value) is reported separately as `pass-lenient`, because expected numerics are not canonical.
- **Skipped (with reasons):** entailment, service, protocol, graph-store-protocol, http-rdf-update, service-description, and http(s) LOAD.
- Output goes to `w3c-report.json` plus a markdown summary. CI fails on any test not in `known_failures.txt`.
- **pyoxigraph is not authoritative.** It fails graph-minus, graph-no-var and agg-empty-group-count-graph, canonicalizes numerics on load, and treats ill-typed EBV as an error. Disagreements are triaged against the spec and the oracle.
- **Target:** 100% of the non-skipped sparql11 and sparql10 evaluation tests.
- **Named regression tests:** open-eq-*, distinct-num, values_and_path, zero_or_*_set_*, graph-minus, graph-no-var, graph-from-named-1/2/3, bindings/graph, agg-empty-group-count-graph, sq15, sq16, exists03, exists-graph-variable, agg-err-01/02, agg-empty-group-*, strbefore02, pp37, the NPS inverse tests, bnode01, now01, the sort-* tests, dataset-09b.

### 12.3 Differential testing against pyoxigraph (`py/diff_oxigraph.py`)
Seeded random small graphs use only canonical numerals (pyoxigraph canonicalizes). Random queries come from a grammar covering BGP, OPTIONAL±filter, UNION, MINUS, [NOT] EXISTS, BIND, VALUES, GROUP/HAVING, ORDER BY over all projected variables + LIMIT, paths and GRAPH. Each query runs on the cdylib (`sparql_query(q,'json')`) and on `pyoxigraph.Store`, and the results are compared as canonicalized multisets under bnode isomorphism. Known pyoxigraph divergences (above) are filtered by query-shape tags.

### 12.4 Benchmark (`py/bench.py`, `py/gen_social.py`, `py/gen_bsbm.py`)
- **Datasets** (deterministic, seeded):
  - *social*: N persons × {rdf:type `:C{i%10}`, `:name "Person i"`, `:age` int, 3×`:knows`, `:city :cityJ`} ≈ 8 triples/person, at 1.6M and 5M.
  - *bsbm-lite*: products with an `rdfs:subClassOf` hierarchy, producers, offers with decimal and integer prices and dateTimes, reviews, at about 1M.
- **Engines.** All SQLite engines run on the same file (`page_size=8192`, `cache_size=-524288`, `ANALYZE` run, journal DELETE):
  - `native` (`sparql_count`, then `SELECT * FROM sparql_rows(?)` fetched);
  - `baseline` (`sparql_count_sql`, `sparql_query_sql`);
  - `pyoxigraph` (in-memory `Store`, bulk-loaded; `len(list(q))` and serialized);
  - `rdflib` (≤ 100K triples only).
  - Phase 2 adds `native+btree`.
- **Queries:**

  | # | Query |
  |---|---|
  | Q1 | star-4 `?x a :C3; :age ?a; :city ?c; :name ?n` |
  | Q2 | BSBM product star-6 |
  | Q3 | chain-2 `?a :knows ?b . ?b :knows ?c` (COUNT) |
  | Q4 | selective chain-3 from a constant |
  | Q5 | `?x a :C3 OPTIONAL { ?x :knows ?y . ?y a :C4 }` |
  | Q6 | `FILTER(?a > 90)` |
  | Q7 | REGEX on names |
  | Q8 | `GROUP BY ?c COUNT ORDER BY DESC LIMIT 10` |
  | Q9 | `ORDER BY DESC(?age) ?x LIMIT 10` |
  | Q10 | DISTINCT over a join |
  | Q11 | UNION |
  | Q12 | MINUS / NOT EXISTS |
  | Q13 | `:p1 :knows+ ?y` (COUNT) |
  | Q14 | `?x rdf:type/rdfs:subClassOf* :Product` |
  | Q15 | point lookup `:p123 ?p ?o` |
  | Q16 | `COUNT(*) {?s ?p ?o}` |
  | Q17 | BSBM offers with a price range + date filter + ORDER BY price LIMIT 10 |
- **Protocol.** 1 warm-up, then the median of 5. Row counts and order-insensitive checksums must agree across engines, or the run fails. Load times are reported separately. The output is a markdown table in `README.md`.
- **Acceptance.**
  - Native is at least 1.5× faster than baseline on Q3, Q5, Q6, Q8, Q9, Q10, Q13, Q14 and Q16, and never more than 10% slower than baseline on any query. SqlJoin covers Q1/Q2/Q4/Q15.
  - Native is within 2× of pyoxigraph on every query and faster on most.

---

## 13. Phase 2: direct B-tree reader (`sq-btree`)
- **Behind `QuadCursor`.** `SqliteStore::cursor` returns `enum SqliteCursor { Sql(SqlCursor), Btree(BtreeCursor) }`. `costs()` returns `BTREE_COSTS` when it is active. Kill switch: `sparql_config('reader','sql')`.
- **Enabled per execution only if all of the following hold:**
  1. `main` is a file DB (`sqlite3_db_filename` is non-empty) and the header bytes 18/19 are 1 (rollback journal, not WAL).
  2. We hold a read transaction (`txn_state == SQLITE_TXN_READ`, via the ReadScope or the outer statement).
  3. There is no write transaction on this connection.
  4. The page cache's key `SQLITE_FCNTL_DATA_VERSION` is unchanged.

  Conditions 3 and 4 are re-checked on every leaf refill. On a violation, the cursor re-opens as a SqlCursor with the row-value resume template at the last returned key.
- **Pages** are read with `(*file).pMethods->xRead` on the `sqlite3_file*` from `SQLITE_FCNTL_FILE_POINTER`: no second file descriptor, VFS shims respected. Index B-tree pages: 0x02 is interior, 0x0A is leaf. Interior cells are real entries (an in-order walk). Records use serial types 0–6, 8 and 9. Root pages come from `sqlite_schema` and are reloaded when the schema cookie (offset 40) changes.
- **The cursor holds its current leaf** as `Arc<[u8]>` rather than looking it up per row. Forward seeks stay within the leaf when possible (finger seek).
- **Page cache:** `FxHashMap<u32, Arc<[u8]>>` with CLOCK eviction, `page_cache_mb` default 128.
- **Realistic budget:** 25–35 ns/row on warm scans (the 18–23 ns figure came from an `fs::read` prototype), about 1.1–1.5 µs per cold 4K-page pread, and 0.24–0.3 µs per sorted finger seek.
- **Tests:** the conformance suite against SqlCursor under page splits, merges, freelist reuse, VACUUM and 4/8/16K pages; WAL detection; own-write fallback; an interleaved writer forcing a mid-scan fallback; the full W3C suite with `reader=btree` on file DBs.

---

## 14. Risks and the top performance tricks

**Risks and mitigations**
1. **Probe-heavy plans lose to SQLite's 0.55 µs NLJ.** Mitigated by SqlJoin (SQLite runs our order), merge/hash for large inputs, sorted and deduplicated probes, calibrated constants, and adaptive bind-to-hash switching.
2. **Semantic corner cases** (GRAPH scoping, OPTIONAL filters, MINUS domains, EXISTS substitution, aggregate errors, ordering). Mitigated by props-gated rewrites, the spec-transcribed oracle with plan-invariance fuzzing, the W3C suite, and pyoxigraph diffs.
3. **Canonical-id bugs** silently break joins. Mitigated by the frozen codec with golden tests, `encoding_version`, `sparql_check()`, and canonical-flag analysis with `canonical=true` as the default.
4. **FFI panics or lifetime bugs abort the host.** Mitigated by `guard()` on every vtab method, catch_unwind in init and Drop, the close hook, and the R1–R7 rules.
5. **Build drift** (`bundled`/`modern_sqlite` creeping in). Mitigated by the compile-time assert, the `cargo tree` gate and the grep gate.
6. **Memory blow-ups** (hash tables, closures, DISTINCT). Mitigated by `max_memory_mb`, `timeout_ms` and interrupts. There is no spilling in v1.
7. **Cardinality errors.** Mitigated by characteristic sets, heavy hitters, bounded counts and the adaptive switch.
8. **Phase-2 DBR correctness.** Strict preconditions, fallback, kill switch, and it is not on the critical path.

**Top 5 performance tricks (expected effect)**
1. **Held read transaction + pooled persistent statements.** Probes drop from 7–8 µs to about 1 µs. A 500K-probe join went from 815 to 155 ms (5.3×). This also saves 10–30 µs of prepare per statement on short queries.
2. **Four g-first orders → free sorted streams → merge joins.** Galloping (G = 6), factorized COUNT, and a Sort enforcer when only one side is sorted. Chain-2: 144–172 ms vs SQLite 360–650 ms vs hash 387–611 ms. Factorized COUNT removes per-row output cost entirely.
3. **Free-column-only seekable cursors with sorted, deduplicated probe batches, plus SqlJoin for selective INL chains.** Each column saved is 15–20%. Sorted probes take 0.72 µs vs 1.03 µs, repeated keys give 3.4×, and SqlJoin's 0.55 µs/probe brings star-4 back from 50 ms to about 35–39 ms.
4. **Compact low-tag ids, rich inline literals and late materialization.**
   - The file is 2.2× smaller (86 vs 189 MB).
   - Numeric and date FILTERs, aggregates and sort keys cost about 2–5 ns per value with no 1 µs decode.
   - Exact `rdf_gpox`-backed range pushdown: `FILTER(?a > 90)` in about 2–3 ms vs a 20 ms scan vs pyoxigraph's 101 ms.
   - TopK on inline ints: about 25 ms vs pyoxigraph's 1.4 s.
   - Only output columns that are actually read get decoded (`col_used`).
5. **Better cardinality and join order than SQLite's self-join planner** (which has no STAT4 and sees N identical aliases): characteristic sets, heavy hitters, ≤ 32 bounded counts at ≤ 56 µs each, DP with interesting orders, and the adaptive switch. Expect order-of-magnitude wins on queries whose selective pattern comes syntactically last, plus `count(*)` in the VDBE at 40 ns/row.

Phase 2 adds about 3–4× on scan-heavy queries (25–35 ns/row vs 105–140 ns/row) for rollback-journal databases.

---

## 15. Work packages
WP0 runs on day 0; afterwards ≥ 6 agents work in parallel. "Dep" lists hard dependencies; everything else is built against `sq-core` + MemStore + stubs.

| WP | Owner | Scope | Dep | Acceptance tests |
|---|---|---|---|---|
| **WP0** | lead | Workspace skeleton (§1), `sq-core` verbatim (§10), `todo!()` stubs with the exact §11 signatures for every crate, `ext/`, `scripts/check.sh`, `notes.md` | – | `cargo test -p sq-core` passes (6 tests); `cargo build --workspace` passes; ext builds; the `cargo tree` and grep gates pass; Python loads the .so and `SELECT sparql_version()` works |
| **WP1** | A | sq-memstore | WP0 | `cursor_conformance` (random 2K quads × 3 seeds, with named graphs) passes; insert/remove/clear/registry/fresh_bnode semantics; `analyze()` exact on synthetic data |
| **WP2** | B | sq-sqlite core: DDL, RawDb, StmtPool, SqlCursor templates, Dictionary (decode/lookup + caches + admission rule), count, named_graphs, Read/WriteScope, `validate_epoch`, `sql_rows` | WP0 | Conformance suite on memory and file DBs; EXPLAIN assertions for every StmtKey; rollback/cache scenario tests; micro budgets (probe < 1.5 µs, 2-column scan < 150 ns/row) |
| **WP3** | B | sq-sqlite writer + bulk loader + analyze + vacuum/check | WP2 | Round trip for all formats; bulk ≡ incremental tables; 1M N-Triples load < 10 s; INV checks via `check()`; stats equal MemStore's `analyze()` |
| **WP4** | C | sq-expr: Value, compile/eval, all builtins, casts, `=` matrix, EBV, SortKey, accumulators | WP0 | Table tests per function (W3C expected values); open-eq matrix; SortKey totality property test; AVG/agg-err semantics; value_diff vs pyoxigraph ≥ 99% agreement after normalization |
| **WP5** | D | sq-plan lowering + props + validate + explain_logical | WP0 | All sparql10/11 `.rq` files lower; snapshot tests; props property tests; GRAPH push/loop decision tests |
| **WP6** | E | sq-exec operators (all §5.3), ExecCtx, interrupt/memory/deadline, paths, GraphLoop, EXISTS memo, CONSTRUCT/DESCRIBE, Execution sources | WP0 (WP4 for real expressions; a stub evaluator before then) | Hand-built plan tests on MemStore per operator × kind × seed mode; factorized count ≡ iterated count; MINUS domain; has_node rule |
| **WP7** | F | sq-oracle | WP0, WP4 | ≥ 95% of W3C query evaluation tests on MemStore through `sq-w3c --backend oracle` |
| **WP8** | D | sq-plan optimizer: rewrites, cardinality, costs, DP, physical selection, SqlJoin rendering, TopK/Count/IntRange, canonical flags, explain | WP5 | Golden explains (star, chain, OPTIONAL, range, top-k); `validate()` everywhere; plan-invariance fuzz (WP10) green |
| **WP9** | G | sq-results + sq-sqlgen | WP0 | Golden serializations; baseline golden SQL; baseline ≡ engine results on small graphs |
| **WP10** | F | sq-engine facade, update executor, generic loader, differential fuzzer (oracle vs native × policies × stores) | WP6, WP8 (WP7 for the fuzzer) | Fuzzer: 10K cases × policies with zero diffs; W3C update tests on MemStore |
| **WP11** | G | sq-api + ext + py/tests: register, all functions and vtabs (§8), ConnState, guard, close hook, configs, parsed-query LRU | WP2, WP10 | Native API tests; pytest suite (zombie, panic, transactions, interrupt, `col_used`, open-cursor refusal); `sparql_rows` count(*) decodes nothing (counter in `sparql_status`) |
| **WP12** | F | tools/sq-w3c full runner (bijection, ordered runs, lenient pass, reports, known_failures) | WP10, WP11 | Runs all three backends; target 100% of non-skipped tests on `--backend sqlite` |
| **WP13** | H | tools/sq-bench + py bench/generators + `py/diff_oxigraph.py` | WP11 | Reproducible markdown table; cross-engine checksums equal |
| **WP14** | lead + D | Calibration and performance: fit CostConstants from `sq-bench micro`, tune gallop G, adaptive thresholds, decode batching, SqlJoin crossover | WP12, WP13 | The §12.4 acceptance numbers; no W3C regressions |
| **WP15** | B (phase 2) | sq-btree direct reader | WP2, WP14 | §13 tests; W3C green with `reader=btree`; ≥ 2.5× scan speed-up on Q3/Q16 for rollback-journal DBs |
| **WP16** | lead | README.md report (results, benchmark table, deviations), final commit hygiene per §1.1 | all | README includes the W3C pass rate, benchmark table and known divergences |

**Critical path:** WP0 → (WP2, WP4, WP5) → WP8 → WP10 → WP11 → WP12/WP13 → WP14.

**Interface change protocol:** sq-core changes are additive-only and reviewed by the lead. Every crate keeps compiling against the stubs at all times (`cargo build --workspace` in CI).
