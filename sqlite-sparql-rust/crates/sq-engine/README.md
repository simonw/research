# sq-engine

`sq-engine` is the facade that turns SPARQL text or a parsed `spargebra` request into work for the
native engine, over any `Rc<dyn QuadStore>`: the SQLite store in production, `MemStore` in tests
and in the W3C `mem` backend. It has four parts. `prepare` / `query_to_string` / `write_results` run
queries: parse (sq-plan), lower, optimize, execute (sq-exec), serialize (sq-results). `explain`
returns the four EXPLAIN views. `update` / `update_with` is the SPARQL 1.1 Update executor. `load` /
`load_quads` is a generic streaming RDF loader over any `QuadStoreWriter`. Joins, operators,
planning and serialization all live in other crates. What `sq-engine` owns is the order of the
steps, the Update semantics and the loader. It opens no transactions: atomicity is the caller's job.
The crate is `#![forbid(unsafe_code)]` and has about 1,500 lines in `src/` (unit tests included).
Its test suite hosts the project's differential fuzzer.

## Where it fits

```
  sq-core   sq-plan (parse, lower, optimize)   sq-exec (Execution)   sq-results (writers)
      \            |                                 |                     /
       +---------- sq-engine ---- dev: sq-memstore, sq-oracle, sq-sqlite, rusqlite, sq-core[testing]
                    |
       +------------+-----------------------------+
   sq-api (sparql_* functions:                 tools/sq-w3c, feature `mem`
   prepare, explain, update_with)             (load, query_to_string, update)
```

Normal dependencies: `sq-core`, `sq-plan`, `sq-exec`, `sq-results`, `sq-expr`, `spargebra`, `oxrdf`,
`oxrdfio`, `oxsdatatypes` (the `NOW()` value) and `rustc-hash`. `sq-expr` is declared in
`Cargo.toml`, but `src/` never references it. *Deviation:* the SPEC §1.3/§1.4 dependency table lists
only sq-core, sq-plan, sq-exec, sq-expr, sq-results and oxrdfio (dev: sq-memstore, sq-oracle). The
code adds the crates above, plus `sq-sqlite` and `rusqlite` as dev-dependencies for the fuzzer's
SQLite stores.

## Key concepts

**1. A thin pipeline, always native.** `prepare` is `sq_plan::lower_query` → `sq_plan::optimize`
(which runs `sq_plan::validate` in debug builds) → `sq_exec::Execution::new`. `query_to_string`
parses with `opts.base_iri`. It rejects a result format that does not fit the query form
(`Error::Invalid`) before it plans anything. The facade ignores `QueryOptions::engine`, so sq-api
routes `EngineKind::Sql` (the sq-sqlgen baseline) itself (`sq-api/src/query.rs`).

**2. The caller provides atomicity and read scopes.** Nothing here begins, commits or rolls back.
sq-api wraps `sparql_update`'s `update_with` call in `ConnState::write_call`, which opens one write
scope per request (unless the host SQL statement is itself writing). On SQLite, the tests open
`write_scope(0)` / `read_scope(0)` themselves. The update executor drops its WHERE `Execution`, and
with it the store cursors, before it writes anything.

**3. Updates: operations in order, each against the current store.** `update::run` loops over
`Update::operations`. Every `DELETE/INSERT ... WHERE` is lowered (`sq_plan::lower_pattern` with the
USING/WITH dataset) and planned when it starts, so a constant that an earlier operation inserted
resolves to its dictionary id. `NOW()` has one value for the whole request: `UpdateOptions::now`,
or `DateTime::now()` taken once. The base IRI is the request's `BASE` if it has one, otherwise
`opts.base_iri`. spargebra has already rewritten `WITH`, `DELETE WHERE` and `ADD`/`COPY`/`MOVE`.

**4. DELETE/INSERT WHERE materializes first** (`update/modify.rs`):
1. Run WHERE to completion and keep only the columns that the templates read. Decode the TEMP ids
   (computed values) while the `Execution` that owns them is still alive, then drop it.
2. DELETE templates are lookup-only. A position that is unbound, unknown to the dictionary, or in an
   illegal role drops that quad.
3. INSERT templates intern their constants lazily, so a template that never fires adds nothing to
   the dictionary. They also intern TEMP values (once per operation) and create one fresh blank
   node per (solution, template label), shared by every template quad of that solution. A quad is
   skipped when a position is unbound, the subject is a literal, the predicate is not an IRI, or
   the graph is not an IRI.
4. `remove(deletes)`, then `insert(inserts)`. A quad that is both deleted and re-inserted stays.

Steps 1-3 are charged against `opts.max_memory_mb`, together with the running execution's own
`memory_used()`. An over-budget operation fails with `Error::MemoryBudget` before it writes.
Blank nodes: a stored blank node in a solution is copied as it is. A blank node minted by `BNODE()`
in WHERE (one TEMP id) becomes one new store blank node for the whole operation. `INSERT DATA`
labels map to fresh blank nodes per operation. `DELETE DATA` is lookup-only: quads with unknown
terms are skipped.

**5. `union_default_graph` is a query-only view.** `update::run` clones the options with
`union_default_graph: false`. Every WHERE clause without `USING` therefore reads the stored default
graph (g = 0), the graph that templates, `CLEAR DEFAULT` and `DROP DEFAULT` write. Without this,
`COPY DEFAULT TO :g` (lowered to an `INSERT .. WHERE { ?s ?p ?o }` with no `USING`) would copy every
named graph's triples. *Deviation:* SPEC §5.6/§5.7 do not say this.

**6. Graph management and SILENT** (`update/graphs.rs`). A named graph exists iff it is in
`store.named_graphs()`.
- `CLEAR` or `DROP` of a missing named graph, and `CREATE` of an existing one, is `Error::Invalid`.
- `CLEAR` keeps the graph registered. `DROP` also unregisters it. `DROP DEFAULT` = `CLEAR DEFAULT`.
- `SILENT` turns only `Invalid`, `Parse` and `Unsupported` into success. Store errors, interrupts,
  timeouts and memory-budget failures are never silenced, because they may have left a partial
  write that only the caller's rollback can undo.

**7. LOAD and the generic loader** (`load.rs`). `LOAD` accepts only local files: `file:///p`,
`file://localhost/p` or `file:/p`, with `%XX` decoded and any query or fragment dropped. It also
requires `UpdateOptions::allow_files`, otherwise it fails with `Error::Unsupported`. The format
comes from the file extension (`RdfFormat::from_extension`), and the base IRI is the document IRI.
The document is streamed in 16,384-quad batches. `LOAD SILENT` first parses the whole file without
writing (`check_file`), so a bad document leaves nothing behind. A plain `LOAD` that fails part-way
leaves its earlier batches for the caller's rollback. `LOAD ... INTO GRAPH` registers the graph even
for an empty document. The public `load` / `load_quads` share the same encoder:
- default-graph triples go to `graph` when one is given;
- blank-node labels map to fresh store blank nodes, with one mapping per call;
- IRIs go through a per-load cache of at most 65,536 entries, which starts over when full;
  literals are interned per occurrence.

sq-sqlite has its own bulk loader for large inputs (SPEC §2.3).

**8. EXPLAIN** (`explain.rs`). The four modes come from `sq_plan::ExplainMode`:
- `Algebra` is spargebra's SSE.
- `Logical` is the lowered IR, with dictionary constants decoded.
- `Plan` is the physical plan with estimates.
- `Analyze` runs the query to completion. It pulls 1,024 rows (or triples) per call and decodes
  nothing. It prints the plan with per-operator `rows`, `opens`, `seeks` and `ms`, and a trailer
  line (example below).

## Public API tour

```rust
pub fn prepare(store: Rc<dyn QuadStore>, query: &spargebra::Query, opts: &QueryOptions, caches: ExecCaches) -> Result<Execution>;
pub fn query_to_string(store: Rc<dyn QuadStore>, query: &str, fmt: ResultFormat, opts: &QueryOptions) -> Result<String>;
pub fn write_results(exec: &mut Execution, fmt: ResultFormat, w: &mut dyn Write) -> Result<()>;
pub fn explain(store: Rc<dyn QuadStore>, query: &str, mode: ExplainMode, opts: &QueryOptions) -> Result<String>;
pub fn update(store: Rc<dyn QuadStoreWriter>, update: &spargebra::Update, opts: &QueryOptions) -> Result<UpdateReport>;
pub fn update_with(store: Rc<dyn QuadStoreWriter>, update: &spargebra::Update, opts: &QueryOptions,
                   uopts: &UpdateOptions) -> Result<UpdateReport>;
pub fn load(store: &dyn QuadStoreWriter, data: &[u8], format: RdfFormat, base: Option<&str>, graph: Option<&str>) -> Result<u64>;
pub fn load_quads(/* same arguments */) -> Result<LoadStats>;

pub struct UpdateOptions { pub allow_files: bool, pub now: Option<oxsdatatypes::DateTime> } // Default: no LOAD, current time
pub struct UpdateReport  { pub inserted: u64, pub deleted: u64 }    // quads actually added / removed
pub struct LoadStats     { pub quads_read: u64, pub quads_new: u64 } // `load` returns quads_new
```

`Result` is `sq_core::Result`. `ExecCaches` is an empty hook struct, so pass `ExecCaches::default()`.
`update` is `update_with` with `UpdateOptions::default()`. The calls below were compiled and run
against this crate. Load, then query to a string:

```rust
use std::rc::Rc;
use oxrdfio::RdfFormat;
use sq_core::plan::QueryOptions;
use sq_memstore::MemStore;
use sq_results::ResultFormat;

let store = MemStore::new();                  // a handle: clones share the data
let ttl = b"@prefix : <http://ex/> . :a :name \"Alice\" ; :knows :b . :b :name \"Bob\" .";
assert_eq!(sq_engine::load(&store, ttl, RdfFormat::Turtle, None, None)?, 3);
let opts = QueryOptions::default();
let csv = sq_engine::query_to_string(Rc::new(store.clone()),
    "SELECT ?n WHERE { ?s <http://ex/name> ?n } ORDER BY ?n", ResultFormat::Csv, &opts)?;
assert_eq!(csv, "n\r\nAlice\r\nBob\r\n");
```

Stream ids with `prepare`. This is the pattern of `tests/common/mod.rs::select`. Ids are only
decodable while the `Execution` is alive, because it owns the TEMP values:

```rust
use sq_core::eval_api::SolutionSource;        // variables / next_rows / decode
use sq_exec::ExecCaches;

let q = sq_plan::parse_query("SELECT ?s ?n WHERE { ?s <http://ex/name> ?n }", None)?;
let mut exec = sq_engine::prepare(Rc::new(store.clone()), &q, &opts, ExecCaches::default())?;
let width = exec.variables().len();
let mut ids = Vec::new();                     // row-major TermIds, UNBOUND = 0
while exec.next_rows(&mut ids, 1024)? > 0 {}
let mut terms = Vec::new();                   // Option<Arc<Term>>, None = unbound
exec.decode(&ids, &mut terms)?;
// Alternatively: exec.count()?, exec.ask()?, or serialize any query form:
// sq_engine::write_results(&mut exec, ResultFormat::Json, &mut out)?;
```

EXPLAIN ANALYZE (actual output for the store above):

```rust
use sq_plan::ExplainMode;

let text = sq_engine::explain(Rc::new(store.clone()),
    "SELECT ?n WHERE { ?s <http://ex/knows> ?o . ?o <http://ex/name> ?n }", ExplainMode::Analyze, &opts)?;
// #0 Project [?n]                                     est=2 cost=524ns rows=1 opens=1 seeks=0 ms=0.005
// #1  BindJoin seed=[?o] adaptive                     est=2 cost=524ns rows=1 opens=1 seeks=0 ms=0.004
// ...
// -- 1 rows; planning 0.063 ms, execution 0.009 ms; decode hits 0, misses 0
```

Update with a fixed `NOW()`, then the same request on SQLite inside a write scope (`tests/update.rs`):

```rust
use std::str::FromStr;
use oxsdatatypes::DateTime;
use sq_engine::{UpdateOptions, UpdateReport};

let u = sq_plan::parse_update("PREFIX : <http://ex/>
    INSERT { ?s :label ?l } WHERE { ?s :name ?n BIND(CONCAT(\"name: \", ?n) AS ?l) } ;
    DELETE DATA { :a :knows :b }", None)?;
let uopts = UpdateOptions { allow_files: false, now: Some(DateTime::from_str("2026-10-15T12:00:00Z").unwrap()) };
assert_eq!(sq_engine::update_with(Rc::new(store.clone()), &u, &opts, &uopts)?,
           UpdateReport { inserted: 2, deleted: 1 });

let scope = sqlite_store.write_scope(0)?;     // sq_sqlite::SqliteStore
let report = sq_engine::update(Rc::new(sqlite_store.clone()), &u, &opts)?;
scope.commit()?;                              // scope.rollback()? on error
```

## Testing

```sh
cargo test -p sq-engine                                                    # default suite
cargo test -p sq-engine --test differential -- --ignored --nocapture       # long fuzz runs
cargo test -p sq-engine --test review_wp10_large -- --ignored              # 90,000-row shapes (+1 known failure)
SQ_REPRO=report.txt SQ_REPRO_PLANS=1 cargo test -p sq-engine --test differential -- --ignored --nocapture replay_report
```

| File | Covers |
|---|---|
| `src/load.rs`, `src/update/graphs.rs` (9 unit tests) | loader stats, target graph and base IRI, per-call blank nodes, batching, file check vs load, bounded IRI cache; `file:` IRI parsing |
| `tests/facade.rs` (8) | every result format and query form, format mismatches, base IRI, `prepare` under all 5 join policies × reorder, the 4 EXPLAIN modes, queries on in-memory SQLite |
| `tests/update.rs` (20), `review_wp10_update.rs` (16), `review_wp10.rs` (7) | the semantics in concepts 3-7: materialization, blank nodes, skip rules, WITH/USING, NOW, SILENT, ADD/COPY/MOVE, the `union_default_graph` view, the memory budget, LOAD, SQLite inside a write scope; CONSTRUCT/DESCRIBE output is a set |
| `tests/differential.rs` | the fuzzer (below) and its self-test: a sabotaged engine that drops a row must be caught and shrunk to ≤ 3 quads |
| `review_wp10_diff.rs`, `review_wp10_shapes.rs` | hand-written native-vs-oracle cases the generator misses: empty registered graphs, query-level VALUES, EXISTS outside FILTER, BNODE() in nested patterns, bulk-loaded vs generic-loaded SQLite |
| `review_wp10_update_oracle.rs` | the DELETE/INSERT WHERE and DELETE WHERE operations of 1,500 random update requests, checked against sq-oracle's WHERE answer plus an independent template instantiation |

**The differential fuzzer** (`tests/fuzz/`, SPEC §12.1) generates small random datasets: 12
subjects, 4 predicates, 2 named graphs, an optional empty registered graph, 32 literals covering
every id tag, and statistics on or off. For each dataset it generates 10 random queries. sq-oracle
answers each query on MemStore, in-memory SQLite (SQL cursors) and a SQLite file with 512-byte
pages (the direct B-tree reader). The three oracle answers must agree, which checks the stores.
Each answer is then compared with the native engine under 5 join policies × reorder on/off.
Answers are compared as bags, sets (REDUCED) or sequences (ORDER BY over every projected
variable), modulo a bijection of minted blank nodes. A mismatch is shrunk, query first and then
data, and printed as a TriG + query repro. *Deviation:* SPEC §12.1 names only MemStore and
SqliteStore; the file store is extra. The update differential runs random requests on all three
stores and requires equal reports, errors and datasets. It also checks the postcondition of the
last operation (COPY copies, DELETE WHERE leaves no match, ...).

Defaults: `differential_short` runs seed 1 with 300 queries, and `update_differential_short` runs
150 requests. The long runs are seed 20261015 with 10,000 queries, and 3,000 requests.
Environment variables:

| Variable | Effect |
|---|---|
| `SQ_FUZZ_SEED` | seed for the query and update runs |
| `SQ_FUZZ_CASES` | number of queries |
| `SQ_FUZZ_UPDATES` | number of update requests |
| `SQ_FUZZ_POLICIES=auto,hash,bind,merge,nosql` | join policies to test |
| `SQ_FUZZ_VERBOSE=1` | print every query |
| `SQ_FUZZ_TRACE=1` | print every update case |
| `SQ_FUZZ_BNODE_ANYWHERE=0` | restrict `BNODE()` to top-level projections (default: anywhere) |
| `SQ_REPRO_UNION`, `SQ_REPRO_NO_PROBES`, `SQ_REPRO_ANALYZE`, `SQ_REPRO_PLANS` | replay options |
| `SQ_REVIEW_UPDATE_CASES`, `SQ_REVIEW_UPDATE_SEED` | size and seed of the update oracle test (default 1,500 cases, seed 11) |

## Numbers

Measured on the project's 4-vCPU dev VM (SQLite 3.45.1, dev profile = `opt-level 1`):
- `cargo test -p sq-engine` passes 100 test runs with 5 ignored. That is 80 distinct tests: the
  harness's 5 self-tests are compiled into each of the 5 binaries that include `mod fuzz`. The tests
  take about 11.5 s after the build, 5.7 s of it in `differential`.
- From the author notes: the long query fuzz run takes about 110 s and the long update run about
  50 s. The doc comment of `review_large_inputs_agree_with_the_oracle` gives about a minute.
- From the project log (`notes.md`): 26 seeds × 10,000 queries gave about 7.7 million native
  comparisons, plus about 12,000 update requests, with zero unexplained mismatches. The W3C
  `mem` backend (MemStore + sq-engine) passes 636/639 tests (the 3 failures are spargebra parser
  issues) and every UpdateEvaluationTest.
- About half the generated queries return no rows, and about 2% are rejected by spargebra's
  scoping rules and skipped (author notes).

## Limitations and known gaps

- `LOAD` reads only local `file:` documents. Remote IRIs always fail with `Unsupported`, which
  `SILENT` suppresses. The format comes only from the file extension. `LOAD SILENT` parses the file
  twice. A failed plain `LOAD` relies on the caller's rollback, and `MemStore` has none.
- These come from spargebra's lowering of `ADD`/`COPY`/`MOVE`, not from SPARQL 1.1 Update:
  `ADD`/`COPY` from a missing source graph raise no error, and `ADD SILENT` has no effect. `MOVE`
  from a missing graph does fail (unless SILENT), through its final `DROP`.
- DELETE/INSERT WHERE holds all solutions of an operation in memory (template columns only). A very
  large update fails with `MemoryBudget` rather than streaming.
- Shared code is outside the differential's reach: the oracle and the native engine share
  sq-plan's lowering and sq-expr, so the fuzzer cannot see bugs there; the W3C suites cover them.
  The generator never emits `NOW`/`RAND`/`UUID`, and emits LIMIT only under a full ORDER BY.
  BindJoin's adaptive switch to hashing needs more than 50,000 left rows, so only
  `review_wp10_large.rs` exercises it. The update differential compares stores with each other.
  Only `review_wp10_update_oracle.rs` checks DELETE/INSERT WHERE against an independent oracle.
- `BNODE()`/`RAND()`/`UUID()`/`STRUUID()` on a BindJoin probe side are handled at run time by
  sq-exec (`build.rs::mints_values`), which hash-joins against a single evaluation. sq-plan still
  costs and EXPLAINs these joins as `BindJoin ... adaptive`. EXISTS patterns are not checked for
  these functions (author notes).
- Ignored test `review_join_on_a_group_by_key_evaluates_the_aggregation_once` documents an sq-plan
  performance defect: `props::group` marks GROUP BY keys only "maybe" bound. A join with an
  aggregating sub-select on its key therefore re-runs the aggregation per left row. Run on the dev
  VM while writing this README, it still fails: `Group ... opens=50000` under a `BindJoin` over
  50,000 left rows.
- The facade has no `count` function, although SPEC §1.2 lists one; use `Execution::count`.

## Further reading

- [docs/SPEC.md](../../docs/SPEC.md): §5.6 (datasets), §5.7 (CONSTRUCT, DESCRIBE, Update), §11
  (the sq-engine API stub; `update_with`, `UpdateOptions`, `write_results`, `load_quads` and
  `LoadStats` are additions), §12.1-12.2 (fuzzer, W3C `mem` backend), §2.3 (bulk vs generic loader).
- [docs/DECISIONS.md](../../docs/DECISIONS.md): the "Update semantics" entry.
- Sibling crates: [sq-plan](../sq-plan/README.md), [sq-exec](../sq-exec/README.md),
  [sq-results](../sq-results/README.md), [sq-oracle](../sq-oracle/README.md),
  [sq-memstore](../sq-memstore/README.md), [sq-sqlite](../sq-sqlite/README.md),
  [sq-core](../sq-core/README.md), [sq-api](../sq-api/README.md).
