# sq-memstore

`sq-memstore` is an in-memory implementation of the sq-core storage traits `Dictionary`,
`QuadStore` and `QuadStoreWriter`. Its one public type, `MemStore`, keeps quads in nine
sorted sets that copy the nine covering indexes of the SQLite schema, and its term
dictionary allocates ids the same way `sq-sqlite` does (for the same write sequence; see key
concept 3 for where they differ). Scans, counts, partial indexes and the
signed-`i64` key order therefore behave the same as on a real database. The crate exists so
that the planner, executor, oracle and engine can be built and tested without SQLite. It is
also the reference for two parts of `sq-sqlite`: its writer semantics and its statistics
algorithm (`compute_stats`). The code is about 480 lines in `src/lib.rs`, with unit tests in
`src/tests.rs`. Nothing in the shipped extension (`ext/`, `sq-api`) links it.

## Where it fits

```
   sq-core (TermId, codec, store traits, stats)     oxrdf 0.3.4     rustc-hash
                         \                              |              /
                                       sq-memstore
                                            |
   dev-dependency of:  sq-plan  sq-exec  sq-oracle  sq-engine  sq-sqlite  sq-sqlgen
   optional dependency of tools/sq-w3c (features `mem` and `oracle`, both on by default)
```

sq-exec runs hand-built plans over it through its `Fx` and `TestStore` fixtures. sq-plan,
sq-oracle and sq-engine use it in their planner, oracle, loader, update and fuzzer tests; the
differential fuzzer runs every case on `MemStore` and on two SQLite stores. sq-sqlite checks
its writer and its `analyze()` against this crate. tools/sq-w3c uses it for the
`--backend mem` run (sq-engine) and the `--backend oracle` run (sq-oracle).

*Deviation from the spec:* SPEC §1.3 lists only sq-plan, sq-exec, sq-oracle, sq-engine and
tools as dependents, and the §1.4 table gives `sq-core` as the only dependency. The code also
depends on `oxrdf` and `rustc-hash`. sq-sqlite and sq-sqlgen also use the crate as a
dev-dependency.

## Key concepts

**1. Nine sorted sets, one per physical order.** `Inner::sets` is
`[BTreeSet<[TermId; 4]>; 9]`, indexed in `IndexOrder::ALL` order. Each set holds every quad it
stores permuted into that order's column order (`IndexOrder::cols()`). `TermId` derives `Ord`
over its `i64`, so keys compare lexicographically as signed integers, which is SQLite's integer
order. `belongs()` copies the SQLite partial-index `WHERE` clauses:

| Orders | Quads stored |
|---|---|
| `Gspo`, `Gpso`, `Gpos`, `Gosp` | all quads |
| `Spog`, `Psog`, `Posg`, `Ospg` (g last) | only named-graph quads (`g != 0`) |
| `Gpox` | only quads whose object tag is 5 (`Typed`) or 8 (`Decimal`) |

The `Gspo` set (`sets[0]`) decides whether a quad is present. The other eight sets are updated
only when a quad is actually added or removed.

**2. Cursors are snapshots.** A new cursor reads its rows once, in `rows_for`:

1. Build the scan range. The prefix columns are fixed, the bounded column takes the bound
   values, and the remaining columns take `i64::MIN` and `i64::MAX`.
2. Range-scan that slice of the set, filtering each key with `ScanSpec::matches`. The filter
   applies exclusive bounds and the partial-order rules.
3. Copy the free suffix columns, row-major, into an `Arc<Vec<TermId>>`.

After that, `fill` copies slices of the snapshot, `seek` binary-searches it (forward and
backward seeks cost the same; a key longer than `width()` is truncated), and `rebind` builds a
new snapshot. Writes made after a cursor is created are invisible to it until `rebind`.
`MemStore` allows writes while cursors are open.

A width-0 cursor (`prefix_len == 4`) yields 0 or 1 rows and appends no values. `counters()`
reports `ReaderKind::Memory` and counts rows, fills (including the final 0-row fill) and seeks,
but not rebinds.

*Deviations:* SPEC §11 says cursors snapshot "at create/seek/rebind", but the code snapshots
only at create and rebind, so `seek` never re-reads the store. The `QuadCursor::rebind`
contract says "must not allocate per call", but this `rebind` allocates a new `Vec` each time.

**3. Dictionary ids are allocated as in sq-sqlite.** The dictionary is
`rows: Vec<Option<Arc<Term>>>` (indexed by row), `index: FxHashMap<TermKey, TermId>` (tag,
lexical form, and the datatype or lower-cased language) and one `next_row` counter shared by
all tags. `MemStore::new()` puts the 35 `codec::WELL_KNOWN` IRIs at their fixed rows, so
`rdf:type` is `codec::RDF_TYPE`. User rows start at `codec::FIRST_USER_ROW` (1024), and the id
of row `r` is `(r << 4) | tag`. `intern` treats terms as follows:

- An inline-encodable term returns its inline id and gets no row.
- A blank node is `Error::Invalid`. Use `fresh_bnode()` instead; its counter starts at 1.
- A `Typed` literal interns its datatype IRI first, as `sq-sqlite` does, so both stores
  allocate rows in the same order.

The ids are only the same when both stores see the same writes and sq-sqlite has not
vacuumed or rolled back. sq-sqlite picks the next row as `max(max(id) + 1, FIRST_USER_ROW)`
(`writer.rs`), so it reuses top rows freed by `vacuum_terms` or by a rolled-back write scope.
MemStore's `next_row` only grows and never frees a row.

`lookup` never inserts. `decode` handles inline ids through the codec, returns blank nodes as
`Term::BNode("b<hex>")` and returns dictionary ids as the shared `Arc` from `rows`. UNBOUND,
ERROR, reserved tags, unknown rows and non-blank-node TEMP ids give `Error::Internal`.

*Deviation:* the `Dictionary::decode` contract says every TEMP id is `Error::Internal`. But
`decode` tests `is_bnode()`, which is also true for a TEMP id whose would-be tag is BNode, so
such an id decodes to a made-up label (`TermId::temp(3, Tag::BNode)` gives `b1a`).
sq-sqlite's `dict.rs` checks `tag() == Tag::BNode && is_storable()` and returns the error.

**4. A graph registry next to the quads.** A `registry: BTreeSet<TermId>` tracks named
graphs:

- `insert` registers every named graph it mentions.
- `create_graph` returns `Ok(false)` if the graph is already listed, and `Error::Invalid` for
  the default graph or an id that cannot be stored. It does not check that the id is an IRI,
  so `create_graph(TermId::from_int(5).unwrap())` returns `Ok(true)`.
- `clear` never touches the registry, so a cleared graph stays listed.
- `drop_graph` only removes the registry entry. The trait says the quads "must already be
  cleared", and MemStore does not check this.
- `named_graphs()` returns the registry plus every distinct non-default `g`, sorted. It finds
  the `g` values with a skip-scan over `Gspo`.

`sq-sqlite/src/writer.rs` says it matches this behaviour "exactly". Its
`random_operations_match_memstore` test compares results, quads and graph lists after each of
200 random operations, for three seeds.

**5. A cloneable single-threaded handle with no transactions.** `MemStore` is
`Rc<RefCell<Inner>>` plus an `Rc<Cell<&'static CostConstants>>`. `Clone` shares both, and the
type is neither `Send` nor `Sync`. Executions take `Rc<dyn QuadStore>`, so pass
`Rc::new(store.clone())` and keep your own handle for inspection.

`QuadStoreWriter` leaves atomicity to the caller, and `MemStore` provides none. If a request
fails part-way, the writes it already made stay. `insert` sorts and deduplicates its argument
in place, and checks every quad before it changes anything. A TEMP, ERROR or reserved-tag id
anywhere, or UNBOUND in s, p or o, rejects the whole batch with `Error::Internal`. By then the
argument has already been sorted and deduplicated. `remove` does not validate.

**6. Statistics and costs are hints only.** `analyze()` runs `compute_stats` over `quads()`
and caches the result. `stats()` is `None` until the first `analyze()`, and later writes do
not refresh it.

`compute_stats` is the reference implementation of SPEC §2.4. Scope 0 is the default graph.
Scope 1 is the RDF merge (distinct s,p,o) of all named graphs. Each scope gets:

- the totals `n`, `ds`, `dp` and `dobj`;
- per predicate: `n`, `ds`, `dobj`, and `imin`/`imax` over inline integers;
- heavy-hitter `(p, o)` pairs with count `>= max(100, n(p)/1000)`, keeping the top 100,000;
- characteristic sets, keeping the top 10,000 by subjects.

`analyzed_quads` is the number of input quads.

`costs()` returns `MEMORY_COSTS` unless `set_costs` replaced it. `caps()` is
`{ sql: false, reader: Memory }`, so the planner never picks the SqlJoin hybrid here.
`sql_rows` and `check_interrupt` keep their trait defaults: `sql_rows` returns `Unsupported`
and `check_interrupt` always returns `Ok`.

## Public API tour

The public items are `MemStore` (`new`, `Default`, `Clone`, `quads()` returning every
`[g,s,p,o]` sorted, `analyze()`, `set_costs()` and the three trait impls), `compute_stats`, and
`simple(lex)`, a `Term::Literal(Literal::simple(lex))` helper that nothing in the workspace
uses.

To use `MemStore` in another workspace crate's tests, add these dev-dependencies (the
pattern of `crates/sq-exec/Cargo.toml`):

```toml
[dev-dependencies]
sq-memstore = { workspace = true }
sq-core = { workspace = true, features = ["testing"] }  # sq_core::testing: random_quads, cursor_conformance
oxrdf = { workspace = true }                            # pinned to =0.3.4
```

Without the `testing` feature, `sq_core::testing` does not exist. Instead of the `oxrdf` line
you can use the re-export `sq_core::oxrdf`.

The fragments below form one program. It was compiled and run against the current source,
inside a function that returns `sq_core::Result<()>`.

```rust
use std::ops::Bound;
use std::rc::Rc;
use sq_core::store::{Dictionary, GraphTarget, IndexOrder, QuadStore, QuadStoreWriter, ScanSpec};
use sq_core::TermId;
use sq_memstore::MemStore;

let store = MemStore::new();
let iri = |v: &str| store.intern(oxrdf::NamedNode::new_unchecked(v).as_ref().into());
let (alice, bob, knows, age) = (iri("http://ex/alice")?, iri("http://ex/bob")?, iri("http://ex/knows")?, iri("http://ex/age")?);
let g1 = iri("http://ex/g1")?;
let d = TermId::DEFAULT_GRAPH;
let mut quads = vec![
    [d, alice, knows, bob],
    [d, alice, age, TermId::from_int(42).unwrap()], // inline xsd:integer: no dictionary row
    [g1, alice, knows, bob],
    [g1, alice, knows, bob],                        // duplicate: sorted + deduped in place
];
assert_eq!(store.insert(&mut quads)?, 3);

// (p, o) of alice in the default graph: GSPO with prefix [g, s]
let spec = ScanSpec::new(IndexOrder::Gspo, &[d, alice]);
let mut cur = store.cursor(&spec)?;
assert_eq!(cur.width(), 2);
let mut buf = Vec::new();
while cur.fill(&mut buf, 1024)? > 0 {}              // row-major [p, o, p, o]
assert_eq!(store.count(&spec, Some(0))?, 1);        // capped: min(n, cap + 1)

// integer range on o: GPOS prefix [g, p] plus bounds on the next column
let (lo, hi) = TermId::int_raw_range(10, 100);
let range = ScanSpec::new(IndexOrder::Gpos, &[d, age]).with_range(Bound::Included(lo), Bound::Included(hi));

// named graphs only, g last: SPOG with prefix [s] yields (p, o, g) = [knows, bob, g1]
let mut named = store.cursor(&ScanSpec::new(IndexOrder::Spog, &[alice]))?;

let mut terms = Vec::new();
store.decode(&[alice], &mut terms)?;
assert_eq!(terms[0].to_ntriples(), "<http://ex/alice>");
```

The raw-id range from `int_raw_range` also contains ids with other tags, so callers re-check
`tag() == Tag::Int` (see [../sq-core/README.md](../sq-core/README.md)). The rest of the
program shows snapshots, statistics and the graph registry:

```rust
let carol = iri("http://ex/carol")?;
let mut cur = store.cursor(&spec)?;                    // snapshot: 2 rows
store.insert(&mut vec![[d, alice, knows, carol]])?;   // allowed while `cur` is open
while cur.fill(&mut buf, 1024)? > 0 {}                 // still 2 rows
cur.rebind(&[d, alice], Bound::Unbounded, Bound::Unbounded)?;  // re-reads: 3 rows

let st = store.analyze();                              // Arc<Stats>, also cached for stats()
assert_eq!(st.scope[0].pred[&age].imax, Some(42));
store.set_costs(sq_core::stats::SQL_CURSOR_COSTS);     // shared by all clones

let g2 = iri("http://ex/g2")?;
assert!(store.create_graph(g2)?);                      // Ok(false) the second time
assert_eq!(store.clear(GraphTarget::Named(g1))?, 1);
assert_eq!(store.named_graphs()?, vec![g1, g2]);       // CLEAR keeps g1 registered
assert!(store.drop_graph(g1)?);
```

To run SPARQL on a `MemStore`, go through the sq-engine facade. The `mem` backend of
`tools/sq-w3c` and the sq-engine tests do the same. This fragment also needs `sq-engine`,
`sq-results`, `oxrdfio` and `spargebra` as dependencies (all `{ workspace = true }`):

```rust
let ttl: &[u8] = b"@prefix : <http://ex/> . :bob :knows :alice .";       // load takes bytes
sq_engine::load(&store, ttl, oxrdfio::RdfFormat::Turtle, None, None)?;   // -> #new quads
let opts = sq_core::plan::QueryOptions::default();
let json = sq_engine::query_to_string(Rc::new(store.clone()), "SELECT * WHERE { ?s ?p ?o }",
    sq_results::ResultFormat::Json, &opts)?;
// update takes an already parsed spargebra::Update (sq_plan::parse_update does the same)
let u = spargebra::SparqlParser::new().parse_update("DELETE WHERE { ?s <http://ex/age> ?o }")
    .map_err(|e| sq_core::Error::Syntax(e.to_string()))?;
let report = sq_engine::update(Rc::new(store.clone()), &u, &opts)?;      // report.deleted == 1
```

## Testing

```bash
cd /home/user/research/sqlite-sparql-rust
cargo test -p sq-memstore          # 6 unit tests, well under a second once built (dev profile is opt-level 1)
```

The crate has no `#[ignore]`d tests, no doc tests, no `tests/` directory and reads no
environment variables. The tests are in `src/tests.rs`:

- **`conformance_default_graph`, `conformance_named_graphs`:** run
  `sq_core::testing::cursor_conformance` over `random_quads(seed, 2000, named)` with 3 seeds
  each. The suite covers all 9 orders × prefix lengths 0..=4 × 4 iterations. Each iteration
  checks drains with several fill sizes, exact and capped `count`, forward and backward seeks,
  and `rebind`.
- **`conformance_after_deletes_and_reinserts`:** removes every third quad and re-inserts half
  of them, checks the returned counts, re-runs the suite and compares `quads()` with the
  expected set.
- **`dictionary_roundtrip_and_well_known`:** checks the tags of IRI, language, typed and
  long-string terms, that `intern` is idempotent, `lookup`, and that `decode` returns the
  language tag in lower case (oxrdf lower-cases it when the literal is built). It also covers the `rdf:type` WELL_KNOWN row, inline integers, the
  blank-node label round trip, `intern` rejecting blank nodes, and an unknown term.
- **`graphs_registry_and_clear`:** registry, `create_graph`, `clear` and `drop_graph`
  semantics, and `insert` rejecting a TEMP id.
- **`analyze_exact_on_synthetic_data`:** exact scope, predicate, heavy-hitter and
  characteristic-set numbers on 1,500 hand-built triples.

Tests in other crates that depend on this crate being correct:

```bash
cargo test -p sq-sqlite random_operations_match_memstore   # SQLite writer == MemStore
cargo test -p sq-sqlite analyze_equals_compute_stats       # SQLite analyze == compute_stats
cargo run --release -p sq-w3c -- --suite "$W3C" --backend mem --known-failures tools/sq-w3c/known_failures.txt
```

`$W3C` is the `sparql` directory of a w3c/rdf-tests checkout. The last command comes from
`scripts/check.sh`. The project log (`notes.md`) records 636 of 639 tests passing on the `mem`
backend; the 3 failures are spargebra parser issues. Every UpdateEvaluationTest passes.

## Performance

The crate has no benchmarks of its own and is not tuned for speed:

- Creating a cursor costs O(log n) plus a copy of every matching row, so a prefix-0 scan
  copies a whole index.
- `count` builds the same copy and throws it away; `cap` only limits the value returned.
- `seek` is O(log rows) over the snapshot, and `rebind` copies a new range.
- `clear` copies every quad before filtering.
- Each quad is a 32-byte key stored in 4 sets (default graph) to 9 sets (named graph with a
  typed or decimal object), plus `BTreeSet` overhead.

`MEMORY_COSTS` are not measurements. `stats.rs` calls them "the starting values" for a
calibration planned in WP-14. They set `scan_row` 20 ns, `scan_col` 2 ns, `seek_random` 300 ns
and `seek_sorted` 150 ns, and take everything else from `SQL_CURSOR_COSTS`.

MemStore timings measured elsewhere (release builds on the project's 4-vCPU dev VM):

- sq-exec: a BindJoin probe on MemStore takes about 1.3 µs according to the author's notes,
  and 1.2-1.8 µs in later re-runs. Most of that time is the range copy in `rebind`, not
  executor work (see [../sq-exec/README.md](../sq-exec/README.md)).
- sq-plan's StarMerge calibration (module doc of `crates/sq-plan/src/physical/star.rs`): on
  300 subjects × 20 values of three predicates under `COUNT(*)`, the MergeJoin chain takes
  2.6 ms and the StarMerge 0.6 ms; without the count, 46 ms against 70 ms.

Do not use MemStore timings to judge the SQLite store.

## Limitations and gotchas

- **Test and oracle store only.** It has no persistence, transactions or rollback.
  `check_interrupt` always returns `Ok`, and the planner never uses SqlJoin on it.
- **Memory grows with scan size.** Every cursor and every `count` copies its whole range.
- **`rebind` allocates on every probe,** although the trait contract says it should not.
- **Statistics go stale.** `stats()` keeps returning the last `analyze()` result until you
  call `analyze()` again.
- **Dictionary rows are never freed.** A term can still be looked up after its last quad is
  removed.
- **The first spelling of a language tag wins.** The dictionary key lower-cases the tag, but
  the stored term keeps the case it was interned with. A literal built with
  `new_language_tagged_literal_unchecked("chat", "FR")` decodes as `"chat"@FR`, and a later
  `"chat"@fr` gets the same id.
- **`drop_graph` does not clear.** On a graph that still has quads it returns `true`, but the
  graph stays in `named_graphs()`. Callers must clear first, as sq-engine's `DROP` does.
  `create_graph` accepts any storable id, not only IRIs.
- **Blank nodes are not checked against the store.** `lookup` of any well-formed `_:b<hex>`
  label returns an id even if no such blank node exists (sq-sqlite's `dict.rs` does the same).
  `fresh_bnode` ignores the quads, so in a store filled with hand-made `TermId::bnode(n)` ids
  (as `random_quads` makes) a fresh id can collide with an existing one.
- **`set_costs` leaks one boxed `CostConstants` per call** (`Box::leak`). Nothing in the
  workspace currently calls it.

## Further reading

- `docs/SPEC.md`: §1.2-1.4 (layout, dependency graph), §2.4 (statistics algorithm), §10
  `src/store.rs` (store-trait contract), §11 (sq-memstore API), §12.1-12.2 (per-crate tests,
  W3C runner backends).
- [../sq-core/README.md](../sq-core/README.md): `TermId`, the codec, the store traits and the
  conformance suite.
- [../sq-sqlite/README.md](../sq-sqlite/README.md): the production store that this crate
  imitates.
- The main test users of `MemStore`: [../sq-exec/README.md](../sq-exec/README.md),
  [../sq-plan/README.md](../sq-plan/README.md), [../sq-oracle/README.md](../sq-oracle/README.md)
  and sq-engine ([../sq-engine/src/lib.rs](../sq-engine/src/lib.rs), the facade with `load`,
  `query_to_string` and `update`).
