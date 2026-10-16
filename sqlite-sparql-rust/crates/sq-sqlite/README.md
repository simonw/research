# sq-sqlite

`sq-sqlite` is sparqlite's storage layer. It keeps RDF quads in ordinary SQLite tables inside the
user's database file, and it implements the `sq_core` storage traits (`QuadStore`,
`QuadStoreWriter`, `Dictionary`) over one SQLite connection. Because of this crate, the planner and
executor never generate SQL for basic graph patterns: they ask for index range scans
(`ScanSpec`) and receive integer-encoded rows from a cursor. A cursor is either a pooled,
pre-rendered SQL statement (`SqlCursor`) or, when it is safe, a reader that decodes the index
B-tree pages of the database file itself (`BtreeCursor`). The same reader answers range counts
from page cell counts and reads dictionary rows from the `rdf_term` pages. The crate also
contains the bulk loader, the statistics pass (`analyze`), the dictionary caches and the
savepoint scopes that make every API call see one snapshot.

## Where it fits

```
                sq-core  (TermId, ScanSpec, IndexOrder, QuadStore / QuadStoreWriter / Dictionary, Stats)
                   ^
                   |
  rusqlite::ffi -- sq-sqlite -- oxrdf, oxrdfio (parsing for the loader), rustc-hash
  (system libsqlite3; allow-listed calls only)
                   ^
                   |  normal dependency
                sq-api   (per-connection state; sparql_init / sparql_load / sparql_config / sparql_status ...)

  dev-dependency only: sq-engine (differential fuzzer on an in-memory SQLite store),
                       sq-plan (EXPLAIN QUERY PLAN of SqlJoin statements), sq-sqlgen (test harness)
```

`sq-plan` and `sq-exec` see this crate only through `&dyn QuadStore`. Its own dev-dependencies are
`sq-core` with the `testing` feature (cursor conformance suite, random quads) and `sq-memstore`
(the reference implementation the tests compare against).

## Key concepts

**1. Schema: one quad table, nine B-trees** (`src/schema.rs`, SPEC §2.1). `rdf_quad(g,s,p,o)` is a
`WITHOUT ROWID` table whose primary key is GSPO. It has 8 secondary indexes, so every
bound/unbound combination is an index prefix and every index is covering:

| `IndexOrder` | B-tree | Columns | Rows |
|---|---|---|---|
| `Gspo` | `sqlite_autoindex_rdf_quad_1` (primary key) | g,s,p,o | all |
| `Gpso`, `Gpos`, `Gosp` | `rdf_gpso`, `rdf_gpos`, `rdf_gosp` | g first | all |
| `Spog`, `Psog`, `Posg`, `Ospg` | `rdf_spog` ... `rdf_ospg` | g last | `WHERE g<>0` (named graphs) |
| `Gpox` | `rdf_gpox` | g,p,o,s | `WHERE (o & 15) IN (5,8)` (typed literals, inline decimals) |

Terms are stored in `rdf_term(id, tag, lex, dt, lang, h)`. Dictionary row `r` is `TermId`
`(r << 4) | tag`. Rows 1..=63 are reserved for the fixed `codec::WELL_KNOWN` IRIs, which
currently fill rows 1..=35. Rows up to 1023 are reserved, and user rows start at
`FIRST_USER_ROW` (1024). Integers, booleans, short strings and similar values are inlined in the
id and have no row; blank nodes are counter ids allocated from `rdf_meta.next_bnode`. `rdf_meta`
holds `schema_version`/`encoding_version` (both 1), the generation counters `write_gen`,
`dict_gen` and `stats_gen`, and `db_id`, a random 64-bit store id written once by `sparql_init`
(stores created before it existed have no row and read NULL). Row numbers and blank-node
counters must lie in `1..=2^59-1` (the id payload); a damaged or hostile file holding anything
else (rows, `max(id)`, `next_bnode`) gets `Error::Schema("corrupt ...")` wherever the value would
become an id, instead of ids that alias other terms. `rdf_graph` is the named-graph registry, and the `rdf_stat_*` tables
hold statistics. All internal SQL is qualified with `main.`.

**2. One `StorageShared` per connection.** `StorageShared` (`src/shared.rs`) owns the statement
pool, the B-tree reader state, both dictionary caches, the cached schema state and statistics,
and the epoch counters. `SqliteStore(pub Rc<StorageShared>)` is the cheap, clonable handle that
implements the traits. `RawDb` is a non-owning `Copy` wrapper around the `sqlite3*`: this crate
never opens or closes the connection. Everything is single-threaded (`Rc`, `Cell`, `RefCell`).
`RefCell` borrows are only held for O(1) check-in and check-out, never while a statement steps
(rule R1). A violation shows up as `Error::Reentrant`.

**3. Statement templates and the pool** (`src/template.rs`, `src/pool.rs`, SPEC §5.2).
`StmtKey { order, prefix_len, lower, upper, seek_cols, kind }` renders exactly one SQL text that
selects only the free suffix columns and orders by the index. For example, `Gpso` with 2 bound
columns becomes
`SELECT s,o FROM main.rdf_quad INDEXED BY rdf_gpso WHERE g=?1 AND p=?2 AND s>=?3 ORDER BY g,p,s,o`.
GSPO is pinned with `INDEXED BY sqlite_autoindex_rdf_quad_1`, because `NOT INDEXED` does not pin
a `WITHOUT ROWID` primary key on SQLite 3.45 (`src/tests/eqp.rs` shows this). Statements are
prepared lazily with `SQLITE_PREPARE_PERSISTENT` and leased from `StmtPool`. A dropped lease is
reset and returned to the pool. The pool keeps at most 8 idle statements per key and caches
`sql_rows` texts in an LRU of 256. Savepoint statements at depth 16 or more are not pooled.
**The pool starts disarmed.** While disarmed, released statements are finalized. The host must
call `arm_pool()` only while it can guarantee `disarm_and_finalize()` before the connection
closes; `sqlite3_close` fails with `SQLITE_BUSY` while idle pooled statements exist.

**4. `SqlCursor`** (`src/cursor.rs`). Implements `fill` / `seek` / `rebind`. A 1-column `seek`,
or a `rebind` whose bound shape is unchanged, resets the base statement and rebinds it, with no
allocation. A `rebind` that switches between bounded, unbounded or exclusive bounds gives its
statements back and leases a different pooled template (which may be prepared then). A seek
with `m >= 2` key columns uses the row-value variant (`(s,o)>=(?3,?4)`). A forward seek first steps up to
`GALLOP` = 6 rows before re-seeking. After a failed seek, rebind or step, `fill` returns an
error (`Error::Interrupted` or `Error::Invalid`) until the cursor is repositioned, never a silent
end of the scan. When the B-tree reader cannot serve a count (concept 8), `count()` runs
`count(*)` in SQLite's VDBE, with an optional `LIMIT cap+1` sub-select.

**5. Scopes** (`src/scope.rs`, SPEC §8.6).
- `ReadScope` runs `SAVEPOINT sq_r{depth}` only when `main` has no transaction, as in a bare
  `SELECT f()`. All statements of the call then share one snapshot and one held read lock.
  Inside `xFilter`, or inside a host statement that writes another schema (SQLite refuses the
  savepoint there), nothing is opened, and `is_active()` returns false.
- `WriteScope` runs `SAVEPOINT sq_w{depth}`, then bumps `write_gen` to take the write lock
  first, so `busy_timeout` applies. `commit()` releases the savepoint; `rollback()` or drop undoes
  it.
- A read scope whose savepoint started the transaction commits nested writes when it ends. Use
  `close()` to see whether that commit failed.
- If an interrupt prevents a write scope from ending its savepoint (or a read scope that owns
  the transaction or contains nested writes), the scope steps a harmless "possible writer"
  statement (`DROP TABLE IF EXISTS main.sqlite_sparqlite_abort`). SQLite then rolls back the
  whole transaction, including an enclosing user `BEGIN`, so a later `COMMIT` can never
  persist half an update. A read-only read scope inside a user transaction only returns the
  error and leaves that transaction alone.
- Contributor rule: any DDL run inside a scope must be followed by `shared.refresh_abort()`,
  because `CREATE INDEX` expires every prepared statement.

**6. Dictionary, caches and the epoch check** (`src/dict.rs`, `src/cache.rs`, SPEC §4.3–4.4).
- `lookup`: inline terms, store blank-node labels (`b<hex>`) and well-known IRIs need no I/O.
  Other terms go to the `LookupCache` (CLOCK, 64K entries, keyed by a 64-bit term hash and
  verified against the full key), then to a hash probe of `rdf_term_h` with an exact compare.
  Negative results are never cached.
- `decode`: inline, blank-node and well-known ids need no I/O. Other ids go to the
  `DecodeCache` (CLOCK, `decode_cache_mb` budget of 64 MB by default, about 96 bytes plus the
  lexical size per entry). Sorted, deduplicated misses are read from the `rdf_term` table
  b-tree pages by the direct reader when it can serve them (concept 8). The rows it leaves
  to SQL, or all of them when it cannot serve, are fetched by pooled point lookups, or by one
  merge scan when a batch needs more than 1/20 of the committed rows. `decode_one` returns a
  cache hit without allocating and fetches a miss's one row without the batch bookkeeping.
- `validate_epoch()` must run once per API entry, inside the entry's scope: the read scope for
  reads, the write scope for writes (open the write scope before the first read). It reads the
  schema cookie, `dict_gen`, `stats_gen`, `max(id)`, `db_id` and the version rows. Skipping it
  on a write lets `intern` return ids that another connection's `vacuum_terms` has deleted.
- The caches are flushed only when `dict_gen` changes (bumped by `vacuum_terms`), when the
  schema changes, when `db_id` changes, when the committed `max(id)` shrinks, or when the main
  database file changes (its `sqlite3_file` address or VFS method table, from
  `SQLITE_FCNTL_FILE_POINTER`: `sqlite3_deserialize`, Python's `Connection.deserialize`). A
  new main file is treated like a schema change (re-detection). Ordinary writes do not flush.
- Decode follows a TYPED row's `dt` only when it is an IRI id; any other `dt` (a TYPED row,
  a cycle, a chain) is `Error::Schema("corrupt dictionary: ...")`, so damaged files cannot
  recurse.
- `check()` (`sparql_check`) reports rows whose `lex` or `lang` is not valid UTF-8 instead of
  failing with an internal error.
- Known SQLite bug: an eponymous virtual table (every `pragma_*` table-valued function,
  `json_each`, ...) used both before and after `sqlite3_deserialize` replaces main can crash
  inside SQLite while the next statement is prepared (stale schema pointer). It reproduces from
  plain Python on 3.45.1 and on 3.53.2; see
  [repro/deserialize-eponymous](../../repro/deserialize-eponymous/README.md). Every catalog read
  in this crate is therefore a PRAGMA statement: schema detection (`PRAGMA main.index_info`),
  the epoch's cookie (`PRAGMA main.schema_version`), and the B-tree reader's layout checks
  (`PRAGMA main.table_xinfo` / `index_list` / `index_xinfo`).
- Only rows `<= committed_max_row` enter the caches, and that bound is refreshed only outside
  a write transaction. A rolled-back term therefore never stays cached.

**7. Loader** (`src/loader.rs`, SPEC §2.3).
- `bulk_load` creates the schema if needed, opens one write scope and parses with `oxrdfio`.
  Inputs of 1 MB or more are parsed on a scoped background thread; SQLite is only called from
  the caller's thread.
- Terms are encoded through a load-local map (capped at 8M entries) plus per-chunk hash probes
  (100K quads per chunk). New rows are appended in row order. Blank nodes are fresh for each
  load.
- Every load (bulk or incremental, any `BulkMode`) into a dictionary with no user rows drops
  `rdf_term_h` and rebuilds it at the end of the load, or before the first probe if the 8M-entry
  term map fills up and is cleared.
- **Bulk mode** drops the 8 secondary indexes, inserts in primary-key order and rebuilds the
  indexes with `PRAGMA threads=4` and `cache_size` capped at 16 MB. `LoadOptions.bulk` picks
  the mode: `BulkMode::Auto` (default; at least 100K quads and more than 0.3x the current
  store), `BulkMode::Always` (whenever the load has quads) or `BulkMode::Never`.
- `analyze` runs automatically at the end of a load when the store has never been analyzed
  (`analyzed_quads` = 0) and has grown, or when the net growth counter
  `rdf_meta.growth_since_analyze` exceeds 25% of `analyzed_quads`.

**8. Direct B-tree reader** (`src/btree/`, SPEC §13).
- `SqliteStore::cursor` returns a `BtreeCursor` when the reader is not set to `sql` and all of
  these hold:
  1. `main` is a rollback-journal (not WAL) file database with a private cache.
  2. A read transaction is held.
  3. This connection has no write transaction.
  4. The order's B-tree has exactly the expected layout.

  Shared-cache mode is detected by looking for `OP_TableLock` in an `EXPLAIN`. A changed
  `SQLITE_FCNTL_DATA_VERSION` (or file) does not refuse a new cursor: it starts a fresh
  snapshot, re-validates the header and root pages, and flushes the page cache.
- Pages are read through the connection's own VFS `xRead`, using the `sqlite3_file*` from
  `SQLITE_FCNTL_FILE_POINTER`. They are cached in a CLOCK page cache (`page_cache_mb`, default
  128) that is flushed when the file or the data version changes.
- The cursor pins its leaf and uses finger searches for forward seeks and sorted rebinds. Every
  byte read is untrusted.
- An open cursor re-checks the mode, preconditions 2–3 and an unchanged data version at every
  `fill`/`seek`/`rebind` (`seek`/`rebind` first try a fresh snapshot and re-descend from the
  root). When the check still fails, or a page or record is unsupported (overflow, non-integer
  column) or corrupt, the cursor re-opens as a `SqlCursor` at the exact resume key. Callers
  never see a difference.
- A precondition fallback is transient: the next `rebind` returns the cursor to the B-tree
  reader once the preconditions hold again. Unsupported or corrupt structures keep that cursor
  on SQL. The `btree_fallbacks` and `btree_resumes` status rows count both.
- `ReaderMode::Sql` (`sparql_config('reader','sql')`) is the kill switch, and it also moves open
  cursors onto SQL. `costs()`/`caps()` report `BTREE_COSTS` / `ReaderKind::Btree` only while all
  nine orders are readable directly right now. The answer depends on the current transaction
  state (outside a read transaction, or inside a write transaction, they report the SQL
  cursor), so query them inside the read scope after `validate_epoch()`.
  `StorageShared::btree_serves(order)` answers the same question for one order.
- **Counts** (`btree/count.rs`). Whenever a cursor over the order would be a `BtreeCursor`,
  `count(spec, cap)` walks the order's b-tree instead of running `count(*)`. Every entry of an
  index b-tree is one cell, interior cells included, so a subtree that lies entirely inside the
  range contributes the cell counts of its pages, read from the page headers. Only the pages
  on the two boundary paths are binary-searched. A cap stops the walk once the total exceeds
  it (`min(count, cap + 1)`), so a planner probe with cap 1000 reads only a few pages. The
  separator cells that bound each visited child must increase strictly within the parent's
  interval, and the first and last cell of each visited leaf must lie inside the leaf's interval. A
  damaged b-tree therefore cannot count a page twice. Rows inside a counted leaf are not
  decoded: their order and the partial-index predicate are trusted to the index definition
  that the root check verified, as SQLite's own `OP_Count` trusts them. A page or record the
  reader cannot use abandons the walk (a counted fallback) and the VDBE count runs instead. A
  pending interrupt is noticed every 1024 pages.
- **Dictionary rows** (`btree/terms.rs`). Decode cache misses read `rdf_term`, a rowid table,
  from its table b-tree (interior 0x05, leaf 0x0D pages). The root is used only when the table
  has exactly the §2.1 columns, `id` is the rowid alias (no primary-key index), and the
  database is UTF-8. Lookups run in ascending row order along a root-to-leaf path that climbs
  only as far as the next row needs, and each page is searched by interpolation between its
  first and last rowid, since rowids are dense. The reader keeps the path between calls for
  as long as the snapshot is unchanged (it is dropped with the page cache when the data
  version changes), which mostly helps single-id misses. A row the b-tree does not hold, a payload that spills to overflow pages, or a record
  of any other shape (non-integer tag or datatype, BLOB, invalid UTF-8, missing columns) is
  read by the pooled SQL statement, which returns the same value or the same error. An
  inconsistent page sends the rest of the batch to SQL. The `DecodeCache` admission rule is
  unchanged.
- The `btree_counts` and `btree_term_rows` status rows count the counts and the dictionary
  rows the reader served.
- `analyze` and the maintenance functions always use SQL.

## Public API tour

Re-exports from `lib.rs`: `SqliteStore`, `StorageShared`, `StorageConfig`, `DictCounters`, `RawDb`,
`SchemaState`, `ReadScope`, `WriteScope`, `LoadSource`, `LoadOptions`, `BulkMode`, `LoadReport`,
`PoolCounters`, `SqlCursor`, `SqlRowsCursor`, `BtreeCursor`, `ReaderMode`, `BtreeCounters` and
`DEFAULT_PAGE_CACHE_MB`. Cursors are only created through `QuadStore::cursor` / `sql_rows`.

Opening a store over a `rusqlite` connection (as `src/tests/mod.rs::store_for` does). The `?`s
mix `rusqlite::Error` and `sq_core::Error` (which has no `From<rusqlite::Error>`), so this
snippet assumes a function returning `Result<_, Box<dyn std::error::Error>>` or similar:

```rust
use rusqlite::Connection;
use sq_sqlite::{RawDb, SqliteStore, StorageConfig, StorageShared};

let conn = Connection::open("data.db")?;
// SAFETY: `conn` must outlive the store and every cursor it hands out.
let db = unsafe { RawDb::from_ptr(conn.handle()) };
let shared = StorageShared::new(db, StorageConfig::default()); // decode_cache_mb 64, lookup_cache_entries 65536
shared.arm_pool();               // keep statements; call store.shared().disarm_and_finalize() before closing
let store = SqliteStore::new(shared);
let schema_version = store.ensure_schema()?; // sparql_init(): idempotent DDL
```

**Before `ensure_schema()`.** Only `ensure_schema` and `bulk_load` create the schema. Until then
reads see an empty store (`cursor` yields no rows, `count` is 0, and `lookup` returns `None`
for every dictionary term except the well-known IRIs). `intern`, `fresh_bnode`, `create_graph`
and a non-empty `insert` fail with
`Error::Schema("the RDF store is not initialized (run sparql_init())")`, as do `analyze`,
`vacuum_terms` and `check`. `remove`, `clear` and `drop_graph` do nothing.

One read entry, in a function returning `sq_core::Result` (`sq-api`'s `read_call` has this shape;
the probe loop mirrors `src/tests/perf.rs`). `subjects: &[TermId]` is a sorted, non-empty list of
subject ids:

```rust
use std::ops::Bound;
use oxrdf::NamedNodeRef;
use sq_core::store::{Dictionary, IndexOrder, QuadStore, ScanSpec};
use sq_core::{Error, TermId};
use sq_sqlite::SchemaState;

let scope = store.read_scope(0)?;                       // SAVEPOINT sq_r0 if main has no transaction
if let SchemaState::Incompatible(m) = store.shared().validate_epoch()? {
    return Err(Error::Schema(m));
}
let Some(knows) = store.lookup(NamedNodeRef::new_unchecked("http://ex.org/knows").into())? else {
    return scope.close();                                // unknown IRI: nothing can match
};
// (g = default graph, p = knows) -> (s, o) rows in s order; BtreeCursor or SqlCursor.
let spec = ScanSpec::new(IndexOrder::Gpso, &[TermId::DEFAULT_GRAPH, knows]);
let mut cur = store.cursor(&spec)?;
let mut rows: Vec<TermId> = Vec::new();
while cur.fill(&mut rows, 1024)? > 0 {}                 // width 2: [s0, o0, s1, o1, ...]
let capped = store.count(&spec, Some(1000))?;           // min(count, 1001); page cell counts or count(*)
// Index-nested-loop probes: one cursor, rebound per (sorted) subject.
let mut probe = store.cursor(&ScanSpec::new(IndexOrder::Gpso, &[TermId::DEFAULT_GRAPH, knows, subjects[0]]))?;
let mut objects = Vec::new();
for &s in subjects {
    probe.rebind(&[TermId::DEFAULT_GRAPH, knows, s], Bound::Unbounded, Bound::Unbounded)?;
    probe.fill(&mut objects, 16)?;
}
let mut terms = Vec::new();
store.decode(&rows, &mut terms)?;                       // one Arc<sq_core::term::Term> per id
scope.close()?;
```

Writes (atomicity is the caller's write scope; `insert` sorts and deduplicates its `Vec` in place).
The write scope opens before anything is read, and the epoch check runs inside it:

```rust
use sq_core::store::{GraphTarget, QuadStoreWriter};

let ws = store.write_scope(0)?;
if let SchemaState::Incompatible(m) = store.shared().validate_epoch()? {
    return Err(Error::Schema(m));
}
let s = store.intern(NamedNodeRef::new_unchecked("http://ex.org/alice").into())?;
let p = store.intern(NamedNodeRef::new_unchecked("http://ex.org/knows").into())?;
let o = store.fresh_bnode()?;
let added = store.insert(&mut vec![[TermId::DEFAULT_GRAPH, s, p, o]])?;
let cleared = store.clear(GraphTarget::AllNamed)?;
ws.commit()?; // or ws.rollback()? / drop(ws) to undo
```

Loading, maintenance and configuration:

```rust
use oxrdfio::RdfFormat;
use sq_sqlite::{BulkMode, LoadOptions, LoadSource, ReaderMode, StorageConfig};

let opts = LoadOptions { graph: Some("http://ex.org/g1".into()), ..LoadOptions::new(RdfFormat::Turtle) };
let report = store.bulk_load(LoadSource::File(std::path::Path::new("data.ttl")), &opts)?;
println!("{} new quads, {} new terms, bulk={}, {} ms", report.quads_new, report.terms_new, report.bulk, report.ms);

store.analyze()?;                          // recompute rdf_stat_*, bump stats_gen
let removed = store.vacuum_terms()?;       // delete unreferenced dictionary rows, bump dict_gen
let verdict = store.check()?;              // "ok: N terms, M quads, K named graphs" or "errors: N" + lines
let plan = store.explain_query_plan("SELECT s FROM main.rdf_quad WHERE g=0")?;

let shared = store.shared();
shared.set_reader(ReaderMode::parse("sql")?);   // kill switch for the B-tree reader
shared.set_page_cache_mb(256)?;
shared.set_config(StorageConfig { decode_cache_mb: 128, ..shared.config() })?; // sparql_config('decode_cache_mb', …)
let direct = shared.btree_serves(IndexOrder::Gpso); // would a new GPSO cursor read pages directly now?
let rows: Vec<(String, String)> = shared.status(); // sparql_status rows: caches, pool, reader counters
```

`set_config` applies new `decode_cache_mb` / `lookup_cache_entries` budgets at runtime. The
counters behind `status()` are also available typed: `dict_counters()`, `pool_counters()` and
`btree_counters()`. Other accessors are `config()`, `schema_state()`, `check_interrupt()`,
`reader()`, `page_cache_mb()`, `clear_page_cache()` and `is_armed()`.

## Testing

```sh
cargo test -p sq-sqlite                                        # debug: 152 tests run, 3 ignored
cargo test -p sq-sqlite --release -- --ignored --nocapture     # the 3 perf tests (about 60 s)
cargo test -p sq-sqlite review_rejected_snapshot               # one test by name filter
```

**Prerequisite: the system SQLite library.** `rusqlite` is built without `bundled` and links the
system `libsqlite3`, so install its development package first (for example `libsqlite3-dev` on
Debian/Ubuntu); linking fails without it. Do not enable `bundled` (the root `Cargo.toml` forbids
it). The tests expect SQLite 3.45.1: with any other version `links_the_host_sqlite_version`
fails, and some `EXPLAIN QUERY PLAN` tests may fail too.

No test reads its own environment variables. Temporary database files go to
`std::env::temp_dir()`, which honours `TMPDIR`. The perf tests return early in debug builds.
They hold `tests::perf_lock()`, so they run one at a time without `--test-threads=1`.
`scripts/check.sh` runs the whole workspace with `--release`.

Unit tests live in `btree/format.rs` (including the fast key decoder against the general one),
`btree/cursor.rs`, `btree/terms.rs` (record shapes, interpolation search against
`partition_point`), `cache.rs` and `keys.rs`. The
integration-style tests are in `src/tests/`:

| File | Covers |
|---|---|
| `conformance.rs` | `sq_core::testing::cursor_conformance` on SQL cursors (memory and file DBs, after deletes), galloping seeks vs brute force |
| `btree.rs`, `review_btree.rs` | reader conformance at page sizes 4K–64K, reserved bytes, after VACUUM; WAL / memory / shared-cache / own-write fallbacks; mid-scan fallback at the exact key; kill switch; corrupt pages never panic; journal-mode and root-page changes under a positioned cursor. Counts: `count(spec, cap)` against brute force for every order, prefix length, bound shape and cap below, at and above the count, inside every conformance scenario (page sizes, reserved bytes, partial indexes, deletes, VACUUM); ranges that start or end exactly at every entry, interior cells included; capped counts read only the pages they need; counts fall back where cursors do; interrupts; damaged pages. Dictionary rows: decoded from the `rdf_term` pages at page sizes 512–64K, in any order, in batches and one by one, with overflowing lexical forms left to SQL; after `vacuum_terms` and VACUUM; the same error as SQL for a deleted row; the path kept between calls is dropped when another connection reuses a row number; SQL inside the own write transaction and for an unexpected table layout; damaged pages never panic |
| `review_perf.rs` | review of the count walk and the direct dictionary reads against the VDBE / SQL paths: keys of every integer width and the i64 extremes, and non-integer keys (REAL in and out of the i64 range, TEXT, BLOB) written by plain SQL; counts after the connection's own commits, a rolled-back transaction whose pages spilled to the file, deleting every row, and with a zero page-cache budget; lexical forms straddling the table-leaf local limit (512 / 1024 with reserved bytes / 4096-byte pages); sparse rowids up to 2^59 and odd `rdf_term` records; `decode_one` against `decode` for every kind of id; the kept `rdf_term` path after the connection's own commit reuses a row number |
| `dict.rs` | intern/lookup/decode round trips, point vs merge decode, cache admission and rollback, `dict_gen` / schema invalidation, hash collisions, exact DDL |
| `audit_fixes.rs` (+ `tests/audit_ffi.rs`) | non-IRI / cyclic TYPED datatypes refused; caches follow a main database replaced by `sqlite3_deserialize` (with and without `db_id`); `db_id` random and stable; out-of-range rows, `max(id)` and `next_bnode` refused by lookup, intern, the loader and `fresh_bnode`; `check()` reports non-UTF-8 rows. Unit tests: `FileId` includes the method table, `term_row` refuses huge serial types, the close hook survives a borrowed pool map |
| `scope.rs`, `review.rs` | savepoint nesting, pool limits and disarm-on-close, interrupts, busy commits, writing host statements, `busy_timeout`, re-analysis triggers |
| `writer.rs`, `loader.rs` | random operations vs `MemStore`; every RDF format; bulk, incremental and chunked loads build identical tables; parse errors leave the store untouched |
| `analyze.rs`, `maint.rs`, `eqp.rs` | stats equal `sq_memstore::compute_stats`; `check()` detects each broken invariant; `EXPLAIN QUERY PLAN` of every reachable `StmtKey` uses the expected covering index and never a temp B-tree |
| `perf.rs`, `btree_perf.rs`, `review_btree.rs::review_perf_per_call_overhead` (ignored) | budgets on 1M triples (load < 10 s, sorted probe < 1.5 µs, 2-column scan < 150 ns/row); reader scans at least 2.5x faster than SQL, warm sorted probes faster, counts (exact and capped) at least 10x faster than the VDBE, decode misses read directly faster than through SQL, zero fallbacks; per-call overhead of reader `fill`/`seek` vs a `SqlCursor` |

Pitfalls for test authors: `sq_core::testing::random_quads` seeds with `seed | 1`, so seeds 2k and
2k+1 give identical data. Its blank nodes are not below `next_bnode`, so that data does not
pass `check()`.

## Performance

These are release builds on the project's 4-vCPU dev VM with SQLite 3.45.1. The data is about
1M social-network triples (999,680 triples, 290,696 terms) in a file database with
`page_size=8192` and `cache_size=-524288` (512 MB). The VM is shared, so ranges span several
runs; runs under heavy machine load are excluded (one such bulk load took 8.7 s, and one early
round measured decode cache hits at 40–160 ns/term).

| Measurement | SQL cursors (`reader=sql`) | B-tree reader |
|---|---|---|
| Bulk load, 1M triples (budget 10 s) | 6.3–7.7 s | – |
| 2-column pinned scan, GPSO (g,p) -> s,o | 116–131 ns/row | 21 warm / 29 cold ns/row (5.6x) |
| 3-column scan, GSPO (g) -> s,p,o | 137 ns/row | 22 warm / 30 cold ns/row (6.2x) |
| Sorted point probe (rebind + fill), GSPO / GPSO | 0.88–1.16 / 0.97–1.28 µs | 0.38 / 0.29 µs |
| Random point probe, GSPO / GPSO | 2.1–2.6 / 1.3–1.7 µs | about the same (memory-bound) |
| Exact count, GSPO (g), 999,680 rows | 38.6 ms (`count(*)`, 39 ns/row) | 0.48 warm / 10.1 cold ms (80x) |
| Capped count (cap 1000), GPSO (g,p) | 56 µs | 1 µs (67x) |
| Decode, batched miss (sorted batches of 1024) | 1.0–1.7 µs per term | 1.06 µs warm / 1.14 µs cold page cache |
| Decode, cache hit | 95–144 ns per term (batched) | – |
| Bare API entry (read scope + epoch check) | 10.6–12 µs | – |

- On a quiet machine the load breaks down as: parse + encode 2.7 s, `rdf_term_h` rebuild 0.3 s,
  primary-key insert 0.5 s, 3 full index builds 3.2 s, analyze 0.4 s, commit 0.2 s.
- Most of the entry cost is taking and releasing the read-transaction lock.
- The reader's precondition check costs about 43 ns per cursor call.
- The scan, count and decode-miss rows were re-measured after the count walk, the direct
  dictionary reads and the fast key decoder (release perf test, 2026-10-15, load average 2–5);
  the other rows are older. On the 1.6M-triple benchmark database (8K pages, 402K terms) a
  `SELECT (COUNT(*) AS ?n) WHERE { ?s ?p ?o }` takes about 2–3 ms of CPU instead of about
  68 ms. Decoding the 200K `:name` literals with a cold decode cache takes 0.74 µs/term in
  sorted batches of 1024 (1.43 µs through SQL) and 1.3 µs/term one by one through
  `decode_one` (2.0 µs). A warm cache hit through `decode_one` costs 160–200 ns, mostly cache
  misses on the 200K-entry cache (it was about 380 ns with the generic batch path).
- For `(g,s,p)` probes, GPSO is about 1.7x faster than GSPO with random subjects.

## Limitations and known gaps

- **Lazy cache release.** When the data version changes, the page cache of the superseded snapshot
  is released by the next `BtreeReader::snapshot()` call that sees the new version (the next
  cursor open or reposition), whether the new snapshot is accepted or rejected (for example
  after `PRAGMA journal_mode=WAL`). Until then the dead pages stay allocated; they are never served.
- **Where the B-tree reader runs.** Only rollback-journal file databases with a private cache.
  WAL, `:memory:` and temp databases, shared-cache connections, and the connection's own write
  transactions always use SQL cursors. `ReaderMode::Btree` currently behaves like `Auto`.
- **Plain-SQL writes.** `UPDATE`/`DELETE` on `rdf_term` is unsupported; `check()` detects the
  damage. Plain-SQL writes to `rdf_quad` are supported, but `growth_since_analyze` does not
  count them.
- **Bulk mode and other readers.** Bulk mode cannot drop indexes while another statement of the
  connection is reading (`SELECT sparql_load(x) FROM t`). The load then falls back to incremental
  inserts and reports `bulk = false`.
- **Write scopes and locking.** A `WriteScope` cannot open inside a statement that writes,
  because SQLite refuses the `SAVEPOINT`. Open it before the call's first read: upgrading an
  already held read lock does not run SQLite's busy handler.
- **Interrupts.** An interrupted write scope rolls back the enclosing user transaction, as SQLite
  does for an interrupted `INSERT`.
- **Perf gate sensitivity.** The 2-column scan budget (150 ns/row) depends on machine load; one
  busy run measured 154–174 ns/row.
- **Counts trust the index definition.** A B-tree count does not decode the rows inside
  counted leaves, so it does not re-check their order or the partial-index predicate the way a
  scan does. On a damaged file it can therefore return a different number than a scan (SQLite's
  own `count(*)` has the same property). `check()` and `PRAGMA integrity_check` find such
  damage.
- **UTF-16 databases.** `sparql_init()` fails on a database whose text encoding is UTF-16
  ("schema still absent after sparql_init"), because `RawStmt::col_bytes` reads TEXT through
  `sqlite3_column_blob`, which returns the database encoding. The dictionary reader declines
  such databases too (header byte 56).
- **Cold `decode_one`.** A single-id miss through `decode_one` (the engine's FILTER path
  decodes one id at a time) costs about 1.3 µs: each call re-validates the snapshot and builds
  a one-row batch. Sorted batches cost about 0.75 µs per term.

## Where the code differs from docs/SPEC.md

- **§13.** The reader is the `btree` module of this crate, not a separate `sq-btree` crate.
  It also answers `count()` and reads dictionary rows, which §13 does not describe.
  `cursor()` returns `Box<dyn QuadCursor>`, not an enum. Preconditions are re-checked at every
  entry point, not only on leaf refills, and there is an additional shared-cache precondition.
  When only the data version changed, `seek`/`rebind` re-descend instead of falling back.
- **§5.2.** Row-value seek templates omit `c(k) >= ?L`. The lower bound is folded into the bound
  row value instead, because SQLite 3.45 would otherwise scan from the start of the range.
- **§4.4.** The epoch check is two pooled statements: the schema cookie, then the meta and
  version rows (plus `db_id`), after a main-file identity check that needs no SQL. Well-known
  rows are always admissible and are decoded from a static table.
- **§8.6.** When the scope's savepoint started the transaction, undo uses a plain `ROLLBACK`
  (`RELEASE` after `ROLLBACK TO` could fail with `SQLITE_BUSY` and leak the lock). Interrupted
  write scopes (and read scopes that own the transaction or wrote) force a full rollback.
- **§2.3.**
  - Parsing runs on one background thread for inputs of 1 MB or more; the optional
    `split_slice_for_parallel_parsing` is not used.
  - Bulk mode also caps `cache_size` and uses multi-row `VALUES` inserts.
  - The re-analysis trigger uses the `growth_since_analyze` counter, which is created on first
    use.
- **§2.4.** Per-predicate `imin`/`imax` are computed in pass A. The named-graph scope reads
  SPOG/POSG and removes adjacent duplicates. The scope-level `dobj` (`rdf_stat_global`) comes
  from one `count(DISTINCT o)`; per-predicate `dobj` is still counted by adjacency in pass B.

## Further reading

- [docs/SPEC.md](../../docs/SPEC.md): §1.6 C API allow-list, §2 storage schema / loader /
  statistics, §4.3–4.4 decode and cache validity, §5.2 cursor templates, §8.6–8.7 scopes and the
  close hook, §13 the B-tree reader. [docs/DECISIONS.md](../../docs/DECISIONS.md).
- [../sq-core/README.md](../sq-core/README.md) (`TermId` layout, the storage traits),
  [../sq-memstore/README.md](../sq-memstore/README.md) (the reference store),
  [docs/SPEC.md §8](../../docs/SPEC.md) and [`sq-api/src/state.rs`](../sq-api/src/state.rs)
  (how the SQL functions drive this crate),
  [../sq-plan/README.md](../sq-plan/README.md) (how `costs()`, `caps()` and `stats()` are used).
