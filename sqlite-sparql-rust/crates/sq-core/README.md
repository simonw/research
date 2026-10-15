# sq-core

`sq-core` is the shared contract of the sparqlite workspace. It defines the 64-bit `TermId`
and the inline value codec, which together are the on-disk term encoding. It also defines the
decoded `Term` type, solution `Batch`es, and the storage traits (`QuadStore`, `QuadCursor`,
`Dictionary`, `QuadStoreWriter`) that the planner and executor use to read quads. The planner
statistics and cost constants live here too, along with the logical IR (`ir::Logical`), the
physical plan (`plan::Op`), and the traits that link the expression evaluator to its hosts
(`TermResolver`, `ExistsEval`, `SolutionSource`, `TripleSource`). The crate contains no SQL
and no SQLite code, and it uses `#![forbid(unsafe_code)]`. It exists so that the other
crates, which were built in parallel, can agree on ids, scans and plans without depending on
each other. `lib.rs` describes it as frozen: after day 0, changes are additive-only and need
lead review.

## Where it fits

```
  oxrdf 0.3.4, spargebra 0.4.7, thiserror, rustc-hash, xxhash-rust (xxh3)
                                  |
                               sq-core   (re-exports oxrdf, spargebra, FxHashMap, FxHashSet)
                                  |
   sq-memstore  sq-sqlite  sq-expr  sq-plan  sq-exec  sq-oracle  sq-results  sq-sqlgen
   sq-engine  sq-api        tools/sq-bench        tools/sq-w3c (optional dependency)
```

Every library crate in `crates/` depends on `sq-core`; `ext/` reaches it only through `sq-api`.
Every dependent in `crates/` except `sq-results` also lists `sq-core` as a dev-dependency with
`features = ["testing"]` to get `sq_core::testing`. The source is a verbatim copy of SPEC §10
apart from two doc comments (in `store.rs` and `codec.rs`).

## Key concepts

**1. `TermId`: a signed i64 with a 4-bit tag in the low bits.** The low tag keeps dictionary
ids small as SQLite varints. Ids are compared as signed `i64`, which is SQLite's integer
order, so merge joins, seeks and range bounds must never compare them as `u64`. Row `r` of
`rdf_term` becomes `(r << 4) | tag`.

| Tag | Name | Meaning |
|---|---|---|
| 0 | `Unbound` | only `TermId(0)`: an unbound slot, and also `g = 0` (the default graph) |
| 1, 3, 4, 5 | `Iri`, `Str`, `Lang`, `Typed` | dictionary terms (`is_dict()`); `Str` = simple literal longer than 7 UTF-8 bytes |
| 2 | `BNode` | blank node `(n << 4) \| 2` from `rdf_meta.next_bnode`; no dictionary row; label `b<hex>` |
| 6..=11 | `Int`, `Bool`, `Decimal`, `DateTime`, `Date`, `ShortStr` | inline values (`is_inline()`), decoded with no I/O |
| 12, 13 | reserved | unused |
| 14 | `Temp` | execution-local: `(index << 7) \| (would_be_tag << 4) \| 14`, never stored |
| 15 | `Error` | only `TermId::ERROR`, the sentinel for an error-valued GROUP BY key; output as unbound |

`TermId::kind()` returns IRI, blank node or literal without any lookup. This also works for
TEMP ids, because they carry the tag the term would have in the dictionary. Use
`is_storable()` to decide whether an id may be written to `rdf_quad`: it is true for tags
1..=11 and false for UNBOUND, TEMP, ERROR and the reserved tags.

**2. Inline only when the lexical form is canonical.** `codec::encode_inline` is a pure
function of (lexical form, datatype, language). It returns `Some(id)` only when formatting the
decoded value reproduces the lexical form exactly. Non-canonical forms such as `"01"`, `"+1"`,
`"1.50"`, `"2020-01-01T00:00:00.000Z"`, `"-00:00"` or `"-0"` stay in the dictionary as `Typed`
rows, so they keep their own identity and round-trip unchanged. The inline ranges are:

- **INT:** `[-2^59, 2^59 - 1]`.
- **DECIMAL:** a signed 55-bit mantissa with up to 15 fraction digits. `"5.0"` is
  distinguished from `"5"` by a `dotzero` bit.
- **DATETIME and DATE:** years 0001–6000 and at most 3 fraction digits. Offsets go up to
  ±14:00, but `-00:00` is not inlined. The id stores the wall-clock fields of the lexical
  form, not a UTC-normalised instant.
- **SHORTSTR:** up to 7 UTF-8 bytes.

Doubles, floats and every other datatype always go through the dictionary. These rules are
part of the on-disk format (`codec::ENCODING_VERSION = 1`). sq-sqlite writes this version to
`rdf_meta` and reports `Error::Schema` for a store whose version differs
(`sq-sqlite/src/schema.rs`).

**3. Invariant: sameTerm ⇔ id equality within one execution.** The `id.rs` header states four
rules:

1. An inline-encodable term has exactly its inline id and never gets a dictionary row.
2. Every other IRI or literal has exactly one `rdf_term` row.
3. A blank node is a counter id.
4. A computed term or query constant that is neither inline nor stored gets one TEMP id from
   the execution's `eval_api::TempDict`. Equal terms get equal TEMP ids.

Because of this invariant, BGP matching, joins, DISTINCT, GROUP BY, MINUS and `sameTerm` are
raw `i64` comparisons. SPARQL `=` on literals is value equality, which sq-expr implements.

**4. Dictionary identity and well-known rows.** `codec::TermKey` (tag, lexical form, extra) is
the exact identity of a dictionary term. `extra` is the datatype IRI for `Typed`, the
lower-case language tag for `Lang`, and empty otherwise. `codec::term_hash` is
`xxh3_64_with_seed(...)` and is stored in `rdf_term.h`. It is frozen, and a golden test pins
it.

`codec::WELL_KNOWN` places 35 XSD/RDF/RDFS IRIs at fixed rows 1..=35, so constants such as
`codec::RDF_TYPE` and `codec::XSD_INTEGER` need no lookup. Rows up to 63 are reserved for this
table, and user terms start at `codec::FIRST_USER_ROW = 1024`.

**5. Nine index orders and one cursor contract.** `store::IndexOrder` lists the B-trees of
`rdf_quad`: the primary key `Gspo`; the full g-first indexes `Gpso`, `Gpos` and `Gosp`; four
g-last partial indexes over named graphs only (`WHERE g<>0`): `Spog`, `Psog`, `Posg`, `Ospg`;
and `Gpox`, a partial index over objects with tag 5 or 8 (`WHERE (o & 15) IN (5,8)`). `Gpox`
holds the dictionary-typed literals and inline decimals, which are the numeric values that an
inline-int range scan would miss.

`IndexOrder::candidates(bound)` returns the orders whose leading columns are exactly the bound
ones. It returns g-first orders when `g` is bound and named-only orders when `g` is free.

A `ScanSpec` is an order plus a fixed key prefix plus optional `Bound<i64>` limits on the next
column. A `QuadCursor` returns only the free suffix columns (`width() == 4 - prefix_len`). It
returns them row-major, in ascending signed order, and each row exactly once. It supports
`seek` (forward seeks are the fast path) and `rebind` (re-targets the cursor for
index-nested-loop probes without allocating).

**6. One global slot layout per query.** Every variable gets a `Slot` (`u16`). That includes
user variables, blank-node variables, generated variables, renamed sub-select variables,
aggregate outputs and EXISTS patterns. A solution is `[TermId; width]` with `0` meaning
unbound, so operators never remap columns. `Batch` holds 1024 rows (`BATCH_ROWS`), row-major.
`SlotSet` is a fixed 256-bit set, and queries with more than `SlotSet::MAX_SLOTS = 256` slots
are rejected as `Error::Unsupported` (`sq-plan/src/lower.rs`).

**7. Plans are plain data.** `ir::LoweredQuery`, the spargebra algebra with slots and resolved
constants, is produced by `sq_plan::lower_query`. It is consumed by `sq_plan::optimize`,
`sq-oracle` and `sq_sqlgen::translate`. `plan::QueryPlan` and its `Op` tree are produced by
`sq_plan::optimize`, run by sq-exec and printed by `sq_plan::explain`. Each EXISTS has its own
sub-pattern, indexed by `ExistsId` and sharing the global slot layout, and is evaluated with
`SeedMode::Substitute`. `plan::QueryOptions` carries the per-query knobs: engine, timeout,
memory budget, union default graph, join policy, reorder, base IRI, and probe-count caps.

## Public API tour

| Module | Contents |
|---|---|
| `id` | `TermId`, `Tag`, `Kind` |
| `codec` | `encode_inline[_literal]`, `decode_inline`, `TermKey`, `term_hash`, `bnode_label`/`parse_bnode_label`, `WELL_KNOWN`, XSD/RDF constants, version constants |
| `term` | `Term` (`Iri`/`BNode`/`Literal`) and `Literal` with `Arc<str>` fields; `from_oxrdf`, `to_oxrdf`, `to_ntriples` |
| `store` | `Col`, `IndexOrder`, `ScanSpec`, `QuadCursor`, `Dictionary`, `QuadStore`, `QuadStoreWriter`, `GraphTarget`, `StoreCaps`, `CursorCounters` |
| `batch` | `Batch`, `Slot`, `SlotSet`, `BATCH_ROWS`, `merge_compatible` |
| `stats` | `Stats`/`ScopeStats`/`PredStats`/`CharSet`, `CostConstants` and the three presets |
| `ir`, `plan` | logical IR, physical plan, `QueryOptions`, `ExecProfile` |
| `eval_api` | `TempDict`, `TermResolver`, `ExistsEval`, `SolutionSource`, `TripleSource` |
| `error` | `Error` (12 variants, all displayed with a `sparql:` prefix) and `Result<T>` |
| `testing` | feature `testing`: `Rng`, `random_id`, `random_quads`, `expected_rows`, `cursor_conformance`, `BruteStore` |

The crate root re-exports `TermId`, `Tag`, `Kind`, `Term`, `Literal`, `Batch`, `Slot`,
`SlotSet`, `BATCH_ROWS`, `Error`, `Result`, `oxrdf`, `spargebra`, `FxHashMap` and
`FxHashSet`. The snippets below compile and run against the current source.

Ids and the codec:

```rust
use sq_core::codec::{self, decode_inline, encode_inline_literal, XSD_DECIMAL_IRI, XSD_INTEGER_IRI};
use sq_core::{Kind, Literal, Tag, Term, TermId};

let five = TermId::from_int(5).unwrap();               // raw (5 << 4) | 6
assert!(five.is_inline() && five.is_storable() && five.as_int() == Some(5));
assert_eq!(codec::inline_datatype(five), Some(codec::XSD_INTEGER));

let iri = TermId::from_dict(1024, Tag::Iri);            // rdf_term row 1024
assert!(iri.is_dict() && iri.dict_row() == 1024 && iri.kind() == Kind::Iri);
assert_eq!(codec::parse_bnode_label("b1f"), Some(TermId::bnode(0x1f)));

assert!(encode_inline_literal("042", XSD_INTEGER_IRI, None).is_none()); // non-canonical
let id = encode_inline_literal("12.25", XSD_DECIMAL_IRI, None).unwrap();
assert_eq!(decode_inline(id), Some(Term::Literal(Literal::typed("12.25", XSD_DECIMAL_IRI))));
assert_eq!(Term::Literal(Literal::lang("chat", "FR")).to_ntriples(), "\"chat\"@fr");
```

A range scan, using the reference `BruteStore` from `sq_core::testing`. The SQLite and memory
stores implement the same traits:

```rust
use std::{ops::Bound, sync::Arc};
use sq_core::TermId;
use sq_core::store::{Col, IndexOrder, QuadStore, ScanSpec};
use sq_core::testing::{random_quads, BruteStore};

let quads = random_quads(42, 400, true);                // sorted, deduplicated [g,s,p,o]
let store = BruteStore { quads: Arc::new(quads.clone()) };
let p = quads[0][2];
// ?s <p> ?o in the default graph, o-sorted, objects restricted to the raw range of ints -10..=10
let (lo, hi) = TermId::int_raw_range(-10, 10);
let spec = ScanSpec::new(IndexOrder::Gpos, &[TermId::DEFAULT_GRAPH, p])
    .with_range(Bound::Included(lo), Bound::Included(hi));
assert_eq!(spec.free_cols(), &[Col::O, Col::S]);
let mut cur = store.cursor(&spec)?;
let mut buf = Vec::new();
while cur.fill(&mut buf, sq_core::BATCH_ROWS)? > 0 {}
// other tags interleave inside an int raw range: re-check the tag
let ints: Vec<i64> = buf.chunks_exact(cur.width()).filter_map(|row| row[0].as_int()).collect();
let n = store.count(&spec, Some(1000))?;                // exact, or 1001 meaning "more than 1000"
```

`IndexOrder::candidates([true, false, true, false])` (g and p bound) returns
`[(Gpso, 2), (Gpos, 2)]`, and `candidates([false, true, false, false])` returns `[(Spog, 1)]`.

TEMP ids and batches (`iri` and `five` come from the first snippet):

```rust
use sq_core::eval_api::TempDict;

let mut temps = TempDict::new();
let x = temps.intern(Term::Iri("http://example.org/absent".into()));  // not inline, not stored
assert_eq!(temps.intern(Term::Iri("http://example.org/absent".into())), x);
assert!(x.is_temp() && !x.is_storable());
let b = temps.fresh_bnode();                            // label "t<index>"

let mut batch = sq_core::Batch::new(3);                 // width 3, capacity 1024 rows
batch.push_row(&[iri, TermId::UNBOUND, five]);
let base = batch.row(0).to_vec();
batch.push_extended(&base, &[(1, b)]);                  // copy base, then set slot 1
```

**Implementing a store.** Implement `QuadStore`, which needs `cursor`, `count`, `dict`,
`named_graphs`, `stats`, `costs` and `caps`. `sql_rows` and `check_interrupt` have default
implementations. A writable store also implements `QuadStoreWriter`:

- `intern` rejects blank nodes with `Error::Invalid`; use `fresh_bnode` instead.
- `insert` and `remove` sort and deduplicate `[g,s,p,o]` in place and reject non-storable ids.
- Implementations never commit. sq-api wraps every request in a SAVEPOINT.

`Dictionary::lookup` never inserts, and `Dictionary::decode` returns `Error::Internal` for
TEMP, UNBOUND and ERROR ids. `testing::BruteStore` (about 80 lines) is the smallest complete
`QuadStore` implementation. The existing implementations are `sq_memstore::MemStore`
(`MEMORY_COSTS`) and `sq_sqlite::SqliteStore`, which returns `BTREE_COSTS` while its
direct page reader is active and `SQL_CURSOR_COSTS` otherwise. A new store should pass the
conformance suite, exactly as sq-memstore's tests do:

```rust
use sq_core::testing::{cursor_conformance, random_quads};

let q = random_quads(seed, 2000, true);   // includes named graphs
let s = store_with(&q);                   // your store, holding exactly q
cursor_conformance(&s, &q, seed, 4);      // panics with the failing ScanSpec on a mismatch
```

## Testing

```bash
cd /home/user/research/sqlite-sparql-rust
cargo test -p sq-core                                        # 6 unit tests, ~0.04 s once built
cargo test -p sq-core golden_term_hashes -- --nocapture      # also prints the GOLDEN hash line
```

There are no `#[ignore]`d tests, no doc tests, and no environment variables. The tests, in
`src/tests.rs`, cover:

- **`inline_roundtrip_and_canonicality`:** 25 canonical forms round-trip, including the INT
  extremes, decimals with `.0`, offset dateTimes and multi-byte short strings. 28
  non-canonical forms and a language-tagged literal are rejected.
- **`int_order_is_signed_monotonic`:** INT ids sort like their values, and `int_raw_range`
  maps back to its endpoints.
- **`tags_are_disjoint`:** TEMP, BNode and ERROR tag predicates, and the bnode label round
  trip, which rejects `b01`.
- **`golden_term_hashes`:** four frozen `term_hash` values. A change here breaks every
  existing database.
- **`candidates_cover_every_shape`:** all 16 bound/free masks have an index order.
- **`conformance_suite_self_test`:** `cursor_conformance` against `BruteStore`.

For each index order, prefix length (0..=4), random present and absent prefixes and random
bounds, `cursor_conformance` checks full drains (fill sizes 1, 3 and 1024), exact and capped
`count`, forward and backward seeks with 1..=width key columns, and `rebind`.

The suite runs against real stores in `sq-memstore/src/tests.rs` (including after deletes and
re-inserts), `sq-sqlite/src/tests/conformance.rs` (SQL cursors on in-memory and file
databases) and `sq-sqlite/src/tests/btree.rs` (the direct page reader at several page sizes).

## Performance numbers

sq-core has no benchmarks of its own. Its `CostConstants` (nanoseconds) are the planner's
model of the measured costs. The "Measured" column comes from SPEC §0 and `notes.md` (4-vCPU
dev VM, SQLite 3.45.1, about 1–1.4M triples, warm cache). The other fields are the same in
all three presets, because `BTREE_COSTS` and `MEMORY_COSTS` inherit them with
`..SQL_CURSOR_COSTS`.

| Field | SQL cursor | B-tree | Memory | Measured |
|---|---|---|---|---|
| `scan_row` + `scan_col` per column | 80 + 15 | 30 + 2 | 20 + 2 | SQL 2-column scan 117–131 ns/row; B-tree 27 (warm) / 34 (cold) ns/row |
| `seek_random` | 1100 | 1300 | 300 | SQL probe in a held read txn 1.0–1.4 µs (§0), 1.3–2.1 µs (notes) |
| `seek_sorted` | 700 | 300 | 150 | SQL 0.66–0.84 µs (§0), ~0.9 µs (notes); B-tree 0.29–0.38 µs |
| `decode_miss` / `decode_hit` | 1000 / 50 | same | same | ~1.0 µs / 46–75 ns (§0), ~100 ns (notes) |
| `vdbe_probe` / `vdbe_count_row` | 550 / 40 | same | same | SQLite nested-loop probe ~0.55 µs; `count(*)` 37–50 ns/row |
| `hash_build_*` / `hash_probe_*` (small, large) | 60, 250 / 25, 120 | same | same | 600K-key hash build 0.33–0.65 µs/row including the scan |

The low-bit tag layout is a measured choice. SPEC §2.2 reports about 62 bytes per triple
including the dictionary: 86 MB for 1.4M triples, against 189 MB with the tag in the high
bits.

## Limitations and known gaps

- **Frozen contract.** Any change must be additive. Codec changes alter the on-disk format and
  need a new `ENCODING_VERSION`. Changing `term_hash` or `WELL_KNOWN` invalidates existing
  databases.
- **No RDF 1.2.** `Term::from_oxrdf` panics on RDF 1.2 triple terms, and oxrdf's `rdf-12`
  feature must stay off. spargebra is built with default features only (no `sparql-12`,
  `sep-0002` or `sep-0006`), so `ir::Expr::Call` covers only the SPARQL 1.1 builtins.
- **At most 256 slots per query** (`SlotSet::MAX_SLOTS`).
- **Limits of inlining.** Values outside the inline ranges in concept 2, all doubles and
  floats, and simple literals longer than 7 bytes go through the dictionary and cost a decode.
  Only INT ids follow value order (`int_raw_range`). An integer range scan therefore also has
  to read `Gpox` (dictionary-typed and decimal objects) to be exact.
- **Unused helpers.** `batch::merge_compatible`, `Batch::push_extended` and
  `Batch::push_unbound` have no callers outside sq-core. sq-exec uses its own
  `rows::compatible`, `merge_into` and `push_merged`.
- **Cost constants not yet calibrated.** They are still the SPEC §10 starting values.
  `stats.rs` says "Calibrated in WP-14", but that pass has not changed them.
- **Stale doc comments.** Several comments describe the spec rather than the current code:
  - `store.rs` mentions a phase-2 `sq-btree` crate. The direct page reader is implemented
    inside sq-sqlite (`crates/sq-sqlite/src/btree/`), and there is no `sq-btree` crate.
  - `codec::parse_bnode_label` says it backs `sparql_id(...)`. The registered SQL function is
    `sparql_term_id`, which reaches it through sq-sqlite's `Dictionary::lookup`.
  - `plan::Op::Group` (and `docs/DECISIONS.md`) say an erroring group key forms its own group.
    sq-exec (`ops/group.rs`, `build.rs`) writes `TermId::ERROR` for both an error and an
    unbound result of a GROUP BY key expression, so the two share one group (SPARQL §18.5.1,
    per the oracle finding in `notes.md`).
- **Small spec deviation.** SPEC §3 says inline INT requires |v| < 2^59. The code also accepts
  -2^59 (`TermId::INT_MIN`), and the round-trip test covers that value.

## Further reading

- [`docs/SPEC.md`](../../docs/SPEC.md): §0 (decisions, measured ground truth), §2 (storage
  schema, why each index exists), §3 (term identity and encoding), §4 (rows, late
  materialization, caches), §5.2 (SQL cursor templates), §7 (planner, cost model), §10 (this
  crate's source), §12.1 (per-crate tests), §13 (direct B-tree reader).
- [`docs/DECISIONS.md`](../../docs/DECISIONS.md) and the project log
  [`notes.md`](../../notes.md).
- Implementations of these traits: [sq-memstore](../sq-memstore/README.md),
  [sq-sqlite](../sq-sqlite/README.md), [sq-expr](../sq-expr/README.md) (uses `TermResolver`),
  [sq-exec](../sq-exec/README.md), [sq-oracle](../sq-oracle/README.md).
- Producers and consumers of the IR and plans: [sq-plan](../sq-plan/README.md),
  [sq-sqlgen](../sq-sqlgen/README.md), [sq-results](../sq-results/README.md),
  [sq-engine](../sq-engine/README.md), [sq-api](../sq-api/README.md).
