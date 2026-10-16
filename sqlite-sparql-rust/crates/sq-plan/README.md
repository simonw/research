# sq-plan

`sq-plan` is the query planner of sparqlite. It turns SPARQL text into the
`sq_core::plan::QueryPlan` that `sq-exec` runs. It parses with spargebra and lowers the algebra
to sparqlite's logical IR (`sq_core::ir::LoweredQuery`). It then applies logical rewrites and
orders each join group with a cost-based search, choosing a join operator and an index access
path for every step. It also provides `validate()`, which re-derives every invariant the
executor relies on, and the EXPLAIN printers behind `sparql_explain`. The crate never executes
a plan and does not link SQLite (`rusqlite` is only a dev-dependency). It reads the store only
through a few `QuadStore` calls: dictionary lookups, dictionary decodes (`decode_one`, for the
numeric constants of integer range bounds), bounded counts, statistics, named graphs, cost
constants and capabilities.
The crate exists because sparqlite plans SPARQL itself instead of translating it to SQL. Join
steps become BindJoin, HashJoin, MergeJoin or NestedLoop operators over the integer-encoded quad
indexes. The only SQL text sq-plan generates is a `SqlJoin` for a left-deep chain of plain
pattern lookups, chosen only when the cost model says SQLite's VDBE is cheaper. On the SQLite
store, single-pattern scans still run as sq-sqlite's pooled range-scan statements (or through
its direct B-tree reader), and bounded counts run as `count(*)` statements.

## Where it fits

```
     spargebra 0.4.7 (parser + algebra)   sq-core (ir, plan, store traits, stats, costs)
                         \                 /         oxrdf, rustc-hash, smallvec
                          +---- sq-plan --+
                                   |  QueryPlan (sq_core::plan) ----> sq-exec runs it
          +------------------------+-------------------+-----------------------------+
      sq-engine                 sq-api            tools/sq-w3c                dev-only users:
  (query, UPDATE WHERE,    (parse cache, lower +   (feature `oracle`:        sq-oracle and sq-sqlgen
   EXPLAIN)                 optimize)               lower_query)              tests (parse + lower)
```

`sq-exec` does not depend on this crate. It consumes the `QueryPlan` type from `sq-core` and
implements the seed contract described below; its `seed.rs` cites `sq_plan::physical`. The
planner's own tests use `sq-memstore`, `sq-sqlite`, `sq-exec`, `sq-oracle`, `rusqlite`, `oxttl`
and `oxrdfio` as dev-dependencies to run plans end to end. SPEC §1.3 lists only `sq-engine` as
a dependent, but `sq-api` also calls `parse_query`, `parse_update`, `lower_query` and `optimize`
directly.

## Key concepts

**1. The pipeline.** `plan_query` is `lower_query` followed by `optimize`. `optimize`
(`src/lib.rs`) resolves the dataset's named graphs (sorted and de-duplicated, kept as
`QueryPlan.named_graphs`), runs the logical rewrites, plans physically and, in debug builds
only, calls `validate()`.

| Module | Role |
|---|---|
| `lower` | spargebra algebra to `Logical`, with one global slot layout |
| `props` | `vars` / `certain` / `maybe` / `safe_inject` for logical and physical nodes |
| `rewrite::{graph, empty, filter}` | `GRAPH <c>`, Empty propagation, GRAPH ?g push-or-loop, filter placement, sameTerm seeds |
| `card` | cardinality estimates (hints only) |
| `physical::{join, choice, estimate, bits}` | join groups, DP / greedy ordering, per-step operator candidates and costs |
| `physical::{scan, range}` | index access paths, integer range (IntRange) scans |
| `physical::{sqljoin, star, semi, finish}` | SqlJoin rendering, StarMerge post-pass, EXISTS decorrelation, plan ids and canonical flags |
| `validate`, `explain` | plan checker; EXPLAIN text |
| `expr` | expression helpers: conjunct splitting, seed constants, EXISTS ids, slot sets |
| `logical` | `Logical` tree traversal and rebuilding |

**2. Lowering and the slot layout.** Every variable gets a slot in one layout for the whole
query, EXISTS patterns included. Hidden slots are also allocated for blank nodes (one per label
per scope). These include the blank nodes spargebra creates as the middles of sequence paths
(`?a :p/:q ?b` becomes `?a :p _:b1 . _:b1 :q ?b`), which EXPLAIN shows as `_:bN`. Variables
that spargebra generates (aggregate outputs, `GROUP BY (expr)` keys and the `DESCRIBE <iri>`
helper) also get hidden slots and print as `?_N`. A query that needs more than
256 slots (`SlotSet::MAX_SLOTS`) fails with `Error::Unsupported`. A sub-select opens a scope:
non-projected variables are alpha-renamed to fresh slots. Constants resolve through
`Dictionary::lookup`; a miss becomes a TEMP id in `LoweredQuery.temps`, which matches nothing
(the estimate is 0 rows and the pattern can never join an SQL chain). Plans therefore depend
on the current dictionary, which is why only parsed queries are cached upstream
(docs/DECISIONS.md). `GRAPH` lowers to `Logical::Graph`, and the patterns inside read
`GraphSel::Active`.

**3. Static properties.** `Props { vars, certain, maybe, safe_inject }` follows SPEC §7.2.
`safe_inject` holds the slots on which a Join-mode seed (sideways information passing) gives
exactly the same rows as a join. `safe_hat()` adds every slot outside `vars`, and
`accepts_seed(s)` tests `s ⊆ safe_hat()`. There is one deliberate deviation. For Filter,
Extend and LeftJoin, `safe_inject` uses all variables of referenced EXISTS patterns, not
`vars ∩ maybe(context)`. Otherwise a seed on a slot that only the EXISTS mentions could leak
into the correlated evaluation. Filter placement still uses the spec's `evars`. A GROUP BY
key that is certain in the group's input is certain in its output (every group takes its key
from bound input rows), so a join with an aggregating sub-select on such a key can hash or
merge on it instead of re-running the aggregation per BindJoin probe or falling back to a
NestedLoop.

**4. The seed propagation contract** (`physical/mod.rs`, checked by `validate()`, implemented
by `sq-exec`). A subtree opened with a Join-mode seed S passes S to:
- every input of an inner join;
- every UNION branch;
- the left input only of Left, Semi and Anti joins and of Minus;
- the input of Filter, Extend, Project, Distinct, Reduced, Sort and Group.

Slice, TopK, Count, GraphLoop and GraphNames are never seeded. A BindJoin re-opens its right
input with exactly its `seed` list. The planner emits a BindJoin only when that list holds
every slot both sides may bind, so merging rows needs no compatibility check. Leaves (Unit,
Scan, Values, Path) merge the whole seed into their rows. Project and Group keep it only on
their output slots. `physical::carried` computes which seeded slots a row really holds, and
the planner reads a seeded slot from an input (as a key or a seed) only where that input
carries it. Slots seeded on both inputs of an inner join are neither keys nor checked.
EXISTS plans are opened in `SeedMode::Substitute` instead.

**5. Logical rewrites** (`rewrite::rewrite`, in this order):
1. **`GRAPH <c>`.** If `c` is not a named graph of the dataset, the node becomes `Empty`.
   Otherwise every `Active` in its scope becomes `Const(c)`.
2. **Empty propagation.** The Empty nodes are folded into their ancestors, so no operator
   keeps an Empty input.
3. **`GRAPH ?g`: push or loop.** The graph is pushed into the patterns (`Active` becomes
   `Slot(g)`) only when the whole body classifies as `GForm`; otherwise it stays a `GraphLoop`
   (`GraphNames` for an empty body). This rule is stricter than SPEC §7.2. `rewrite/graph.rs`
   lists the counterexamples: `VALUES ?g` inside the graph; a UNION branch or nested
   `GRAPH <c>` that binds ?g through a subject, predicate or object; and an OPTIONAL whose
   right side reads the graph or binds ?g directly while its left side does not.
4. **Filter placement** (`rewrite/filter.rs`). Conjuncts only move down (a conjunct is copied
   into every UNION branch). They never enter an OPTIONAL or MINUS right side, never cross a
   Slice and never leave their group. The one exception is an OPTIONAL's own filter conjuncts whose `evars`
   the right side binds certainly: they move into that side. `FILTER(sameTerm(?x, c))`, and
   `?x = c` for an IRI or simple literal `c`, become a seed constant
   `Join(Values{?x: [c]}, P)` when `?x` is certainly bound where the conjunct is placed;
   otherwise they stay a Filter.

In debug builds, `filter::check_barriers` verifies that no conjunct moved into an OPTIONAL or
MINUS right side. `validate()` cannot see this on a physical plan.

**6. Join groups and their ordering** (`physical/join.rs`). Join and Bgp nodes are flattened
into units: quad patterns, paths, VALUES and opaque subtrees (OPTIONAL, UNION, MINUS,
sub-selects, GraphLoops, nested filtered groups). The search depends on the group size:
- **Up to 12 units (`MAX_DP_UNITS`).** A left-deep DP over connected subsets runs if its work
  fits `MAX_DP_WORK = 6_000`. It keeps the cheapest prefix per *interesting property*:
  unordered, sorted by a slot a remaining unit binds (for a later merge), or an open SQL
  chain. If keeping every property would exceed the budget, it keeps fewer (`Keep::TIERS`:
  sorted prefixes before SQL chains), down to one prefix per subset.
- **Otherwise:** greedy ordering (cheapest connected next unit, cross products last).
- **`reorder = false`:** a DP over the prefixes of the syntactic order.

The search runs in `Mode::Probe` on estimated `Shape`s. It costs opaque units from one base
plan per node, re-costed for the bound slots (`estimate.rs`). The winning path is then
replayed with real plans in `Mode::Build`. Plans are memoized per (node, context normalized to
the node's slots). A group's filter conjuncts are applied as soon as the joined units bind
their `evars`.

**7. Operators, access paths and policies** (`choice.rs`, `scan.rs`, `range.rs`).

Each step considers these join operators:
- **BindJoin** (index nested loop, emitted `adaptive`). Above 64 left rows that are not already
  sorted by a seed slot it re-sorts batches (`sort_left`); both variants are costed when the
  left order may feed a later merge. Probes in key order cost `seek_sorted` when consecutive
  keys are close in the unit's index range and up to `seek_random` when they are more than
  256 rows apart on average (`ordered_seek_cost`). A unit that mints values (BNODE, RAND, UUID,
  STRUUID anywhere in it) is never a BindJoin right side of an inner or OPTIONAL join: it is
  evaluated once (HashJoin, else NestedLoop), which is what sq-exec runs anyway.
- **SQL step:** the same probe inside a SqlJoin, while the prefix is an open SQL chain of fewer
  than 64 patterns, costed `vdbe_probe` (750 ns) per left row plus `vdbe_row` per produced
  column.
- **HashJoin:** `_small` cost constants below 32,768 build entries, `_large` above.
- **MergeJoin:** gallops after 6 misses (10 with the direct B-tree reader); `SortBySlot`
  enforcers can sort an input by the key. A galloping right side costs, per distinct left key,
  an ordered seek (random when the keys are sparse) plus the rows read past the previous key
  before it (up to the gallop threshold), so 9 keys against a 600K-row range no longer look
  cheaper than 9 probes. The executor buffers the right run of each key and pairs every left
  row of the key with all of it, so a merge on one key that checks the other shared slots
  (`check=[...]`) visits `Σ |L_run| × |R_run| ≈ |L| × rows(R | key)` pairs before the check
  keeps its output. Each pair costs `pair_cost` (a row copy plus one `row_copy_per_slot` per
  checked slot; 5.7 ns measured for 4-slot rows, 5 ns modelled), and every matched right row
  is copied into the run buffer once. A key with few values and checks is therefore costed
  at its run products: the planner merges on the most selective key, or hashes on all the
  keys, instead (see "Joins on several keys" below). A COUNT(*) factorized over a check-free
  merge on top of the group (`fast=merge-factorized`, `fast=star-factorized`) multiplies run
  lengths: it is charged neither the output rows nor the run buffer (`count_free`).
- **NestedLoop:** used when nothing else is legal.
- **Memory.** A SortBySlot enforcer or a hash table whose estimated bytes (rows × (8 × slots
  + 24)) exceed the memory budget (`max_memory_mb`, 1 GB by default) would stop the query
  with "memory budget exceeded". It costs 100 µs more per buffered row (`over_budget`, about
  20,000 row copies), so a plan whose buffers fit wins unless it is far slower; between plans
  that all exceed the budget, the smaller buffers win. It is a cost, not a ban, because the
  row estimates may be too high.

By default (`choose_order`), a quad pattern reads the first `IndexOrder::candidates` order for
its bound columns, except that probes with g, s and p bound (and fully bound probes) read GPSO.
When a merge join or an interesting sort order of the DP wants a different output order, the
planner reads another candidate order that is sorted by that key (`sorted_accesses`). This is
why the golden count below reads `?a :knows ?b` through Gpos. A Scan
claims `sorted_by` its first free column when it is read as one stream. An integer range
filter on an object with a fixed graph and a constant predicate may become
`Scan { order: Gpos, range: IntRange }`. The scan reads the inline integer range plus the
`rdf_gpox` exceptions, and the FILTER stays above it, which keeps the plan exact.

`QueryOptions.join_policy` selects the strategy:

| `JoinPolicy` | Behaviour |
|---|---|
| `Auto` (default) | Cost-based choice of bind, hash and merge (at most one enforcer per merge). SQL steps only when `store.caps().sql`, the group is unseeded and the plan is not an EXISTS plan. StarMerge post-pass. EXISTS decorrelation chosen by cost. |
| `NoSql` | `Auto` without SQL steps. |
| `PreferMerge` | Every inner join with a certain shared key is a binary MergeJoin (enforcers allowed on both sides), on its cheapest key (with several keys, usually the most selective one, the others checked). Other steps are cost-based, without SQL steps. No StarMerge. |
| `PreferHash` | HashJoin wherever keys exist, else NestedLoop. Never merges. Always decorrelates EXISTS when legal. |
| `PreferBind` | BindJoin wherever legal. Never merges. Decorrelated EXISTS becomes a BindJoin. |

EXISTS plans never use merge joins or SQL steps and never claim a sort order.

**8. Other physical decisions.**
- **EXISTS decorrelation.** A whole-conjunct `[NOT] EXISTS {Q}` becomes a Semi or Anti join on
  `S = vars(Q) ∩ maybe(group)` only when all of these hold:
  - `S` is non-empty and certain at that point;
  - `S ⊆ safe_inject(Q)`;
  - `Q` has no zero-length-capable path with an endpoint in `S`;
  - `Q` has no MINUS whose right side mentions a slot of `S`;
  - the EXISTS is not itself inside an EXISTS plan.
- **TopK.** `ORDER BY` + `LIMIT` becomes `TopK{k = offset + limit}`, with a Slice above it
  only when `offset > 0`. This never happens across DISTINCT.
- **Count.** A single non-DISTINCT `COUNT(*)` with no GROUP BY becomes `Count`, with fast
  paths `scan-vdbe`, `merge-factorized` and `star-factorized`.
- **ASK.** The root is wrapped in `Slice{0, Some(1)}`.
- **SERVICE.** `SERVICE SILENT` is planned as `Unit`; any other SERVICE fails with
  `Error::Unsupported`.
- **Post-passes.** SQL chains are rendered as `SqlJoin` when they close (at a filter, a
  non-SQL step or the end of the group, and at most 64 aliases). Merge chains of three or more
  inputs become `StarMerge` where cheaper (`Auto` and `NoSql` only); a StarMerge with checks
  is charged for every combination of its key's runs, as the checked MergeJoins of the chain
  are for their pairs, and both are charged for buffering the runs. Then pre-order plan ids
  are assigned, root first and then the EXISTS plans. Finally `Extend.canonical = false`
  (shown as `(output-only)` in EXPLAIN) is set only for slots that no id-comparing consumer
  reads.

**9. Cardinality and costs** (`card.rs`; statistics only influence plan choice, never
correctness).
- **Bounded counts.** A pattern with a constant besides the predicate, or any pattern when
  there are no statistics, gets a runtime bounded count: `store.count(spec, Some(cap))`. The
  cap is `probe_count_cap` (1,000), with at most `max_probe_counts` (32) counts per query.
  Without statistics the cap is 10,000, and a count that exceeds it is extrapolated 10×.
  IntRange costing spends the same budget on bounded counts of the `rdf_gpox` numeric
  exceptions, and assumes 0 exceptions once the budget is spent. A query that is one quad
  pattern under solution modifiers only (no filter, no EXISTS) has no cost-based choice and
  plans no bounded count (`single_pattern_root`); its EXPLAIN estimate then comes from the
  statistics.
- **Statistics.** Otherwise the estimate comes from heavy hitters and per-predicate statistics,
  then fixed defaults. Subject stars use characteristic sets. One named graph is estimated as
  an even share of the named-graph scope.
- **Store inputs.** `store.costs()` selects the `CostConstants`, `caps().sql` enables SQL
  steps, `caps().reader` sets the gallop threshold, and `stats()` feeds `Card`.
- **Joins** (`choice.rs`, `pattern_join`). Every operator of a join step gets the same row
  estimate. For a quad pattern unit it is the unit's rows in the group's context times the
  combined selectivity of the join keys. Each key's selectivity is `1 / max(dv_L, dv_R)`:
  `dv_R` is the unit's rows over its rows per value of the key, and `dv_L` is the fewest
  distinct values among the prefix's quad patterns that bind the key, capped by the prefix's
  rows (SPEC §7.4.5; a prefix with more values than the unit finds a match for only part of
  its rows). A unit that extends a subject star of the prefix uses its characteristic-set
  ratio as the subject's selectivity. Opaque units are estimated with the keys seeded.
- **Joins on several keys** (`combine_keys`). The most selective key counts in full. Further
  *entity* keys multiply, as SPEC §7.4.5 says (independent keys). Further *attribute* keys,
  whose values the unit repeats on at least 100 rows (`ATTRIBUTE_ROWS`: a city, an age, a
  type, a vendor's country), back off exponentially: the first is square-rooted, the next
  fourth-rooted, and so on. Attributes shared along a join are often correlated with its
  other keys. On the tutorial's 100K-person network, `?a :knows ?b . ?b :knows ?c . ?a
  :livesIn ?city . ?c :livesIn ?city` returns 292,595 rows: independent keys estimate 4.5e4,
  the backoff 2.0e5. On the 1.6M-triple benchmark graph (1,000 cities), friends of friends
  in the same city are 52,949 rows: 1.8e3 independent, 5.7e4 backed off. Backing off an
  attribute costs at most `sqrt(dv)` when the keys are in fact independent, which its few
  values keep small. Entity keys, such as the last edge of a cycle, have as many values as
  the graph has nodes and multiply: the golden triangle `?x :knows ?y . ?y :knows ?z . ?z
  :knows ?x` on 200 random people is estimated at 26.7 rows for its 26 triangles (backing
  off would say 378, and about 500 times too many on the 200K-person benchmark graph).
- **Clamping.** All estimates are finite and clamped to `[0, 1e300]`.

## Public API tour

The whole public surface is re-exported from `src/lib.rs`:
- `parse_query`, `parse_update`, `lower_query`, `lower_pattern`, `optimize`, `plan_query`;
- `validate`;
- `props`, `plan_props`, `Props`;
- `explain`, `explain_with`, `explain_logical`, `explain_logical_with`, `ExplainMode`.

Errors are `sq_core::Error`. The variants are `Syntax` from the parsers, `Unsupported` for more
than 256 slots, RDF 1.2 triple terms and non-silent SERVICE, and `Internal` from `validate`
and the debug-build checks in `optimize`. An invalid base IRI is also a `Syntax` error.
`Internal` can also come from planner invariant failures that are checked in every build (for
example "no legal join operator to replay" in `physical/join.rs`). Store errors from lookups
and counts pass through.

```rust
use sq_core::plan::{JoinPolicy, QueryOptions, QueryPlan};
use sq_core::store::QuadStore;
use sq_core::Result;

fn plan(store: &dyn QuadStore, text: &str, opts: &QueryOptions) -> Result<QueryPlan> {
    let parsed = sq_plan::parse_query(text, opts.base_iri.as_deref())?; // Error::Syntax on bad input
    let lowered = sq_plan::lower_query(&parsed, store, opts)?; // slots, TEMP ids, DatasetSpec
    print!("{}", sq_plan::explain_logical_with(&lowered, Some(store.dict())));
    let plan = sq_plan::optimize(lowered, store, opts)?; // rewrites + physical planning
    sq_plan::validate(&plan)?; // optimize() already ran this in debug builds
    print!("{}", sq_plan::explain_with(&plan, None, Some(store.dict())));
    Ok(plan)
}

// Forced shapes for differential testing: hash joins in syntactic order.
let opts = QueryOptions { join_policy: JoinPolicy::PreferHash, reorder: false, ..Default::default() };
```

`sq-engine` follows the same parse → lower → optimize sequence (`query.rs`). Its EXPLAIN
(`explain.rs`) passes `Some(store.dict())` so that constants print as full IRIs. It never calls
`validate()` itself and relies on the debug-build check inside `optimize`. Without a
dictionary, EXPLAIN prints inline values, TEMP terms, blank nodes and sq-core's well-known
IRIs, and shows other dictionary ids as `<#id>`. IRIs in the rdf, rdfs, xsd and owl namespaces
are abbreviated (`rdf:type`, `xsd:integer`, ...) whenever the IRI is printed and its local name
is a simple name (letters, digits, `_` and `-`). For EXPLAIN ANALYZE, pass the
executor's `ExecProfile` as the second argument; this adds `rows= opens= seeks= ms=` per node.

An UPDATE's WHERE clause is lowered with `lower_pattern`, which also returns the
variable-to-slot map (taken from `tests/lower.rs`):

```rust
let u = sq_plan::parse_update("PREFIX : <http://ex/> WITH :g DELETE { ?s ?p ?o } \
                               INSERT { ?s :q ?o } WHERE { ?s ?p ?o }", None)?;
let spargebra::GraphUpdateOperation::DeleteInsert { using, pattern, .. } = &u.operations[0] else { panic!() };
let (lowered, var_map) = sq_plan::lower_pattern(pattern, using.as_ref(), None, &store, &opts)?;
let plan = sq_plan::optimize(lowered, &store, &opts)?; // form = Select(all in-scope slots)
```

`props` and `plan_props` expose the static properties. `tests/w3c.rs` uses them to check the
subset invariants on every node (`safe_inject ⊆ vars` and `certain ⊆ maybe` on logical and plan
nodes, plus `maybe ⊆ vars` on logical nodes):

```rust
let lp = sq_plan::props(&lowered.root, &lowered.exists);
assert!(lp.certain.is_subset(&lp.maybe) && lp.safe_inject.is_subset(&lp.vars));
let pp = sq_plan::plan_props(&plan.root, &plan.exists);
assert!(pp.certain.is_subset(&pp.maybe) && pp.safe_inject.is_subset(&pp.vars));
```

`accepts_seed` tells whether a node may be opened with a given Join-mode seed (the W3C test does
not call it):

```rust
let seed_ok = pp.accepts_seed(seed); // seed: SlotSet; true when seed ⊆ safe_hat()
```

The golden tests pin plan shapes. This is `COUNT(*)` of a two-hop chain on the 200-person
social graph (MemStore costs) from `tests/golden.rs`: the count is factorized over a merge of
the o-sorted and s-sorted `knows` streams.

```
#0 Project [?n]                                     est=1 cost=29.3us
#1  Extend ?n := ?_3 (output-only)                  est=1 cost=29.3us
#2   Count -> ?_3 fast=merge-factorized             est=1 cost=29.3us
#3    MergeJoin key=?b gallop=6                     est=1.8e3 cost=41.2us
#4     Scan Gpos ?a <http://ex/knows> ?b (sorted ?b) est=598 cost=14.7us
#5     Scan Gpso ?b <http://ex/knows> ?c (sorted ?b) est=598 cost=14.7us
```

On an in-memory SQLite store, the four-pattern star `?x a :C3 ; :age ?a ; :city ?c ; :name ?n`
becomes one statement:
`SqlJoin t0:Gpos(p=rdf:type,o=<http://ex/C3>) t1:Gpso(p=<http://ex/age>,s=t0.s) ... -> [?x ?a ?c ?n]`.
The text depends only on the chain's shape. It uses `CROSS JOIN` plus `INDEXED BY`, and every
constant, including the graph, is a `?N` parameter (`physical/sqljoin.rs`).

## Testing

**Prerequisites.** The crate needs Rust 1.88 or later: it uses edition-2024 let-chains, which
the workspace's `rust-version = "1.85"` understates. It was tested with rustc 1.97. The tests
link the system libsqlite3 through `rusqlite` (the workspace never enables `bundled`), so
install the SQLite development library (`libsqlite3-dev` on Debian/Ubuntu, or equivalent). The
SqlJoin `EXPLAIN QUERY PLAN` assertions in `differential.rs` were validated on SQLite 3.45.1;
another SQLite version may choose a different plan shape.

```sh
cargo test -p sq-plan                       # 64 unit tests in src/ + 83 integration tests
cargo test -p sq-plan --test golden         # one file
SQ_W3C_DIR=/path/to/rdf-tests/sparql cargo test -p sq-plan --test w3c -- --nocapture
REVIEW_SEED=3 REVIEW_N=600 cargo test -p sq-plan --test review3_semantics -- review_physical_plans
REVIEW_SEED=2 REVIEW_CASES=300 REVIEW_SIZE=150 cargo test -p sq-plan --test review_wp8
```

There are no `#[ignore]`d tests. Some tests read environment variables:
- **`tests/w3c.rs`** reads `SQ_W3C_DIR`: the `sparql` directory of a w3c/rdf-tests checkout.
  It must contain both `sparql10/` and `sparql11/`. For example, run
  `git clone https://github.com/w3c/rdf-tests` and point at `rdf-tests/sparql`. The test only checks for
  `sparql11/` before running, so a directory without `sparql10/` fails its `queries > 900`
  assertion. The default is a scratchpad path from the original development session. When the
  suite is absent, the test prints a note and **passes without checking anything**.
- **`tests/review3_semantics.rs`** reads `REVIEW_N`, the case count of both randomized tests
  (defaults 250 for `review_physical_plans_match_naive_evaluation` and 1,500 for
  `graph_var_bodies_match_naive_evaluation`). `REVIEW_SEED` (default 7) only reseeds the
  first; the GRAPH ?g test always uses a fixed seed. Without a test-name filter, the command
  above would also cut the GRAPH ?g test to 600 cases.
- **`tests/review_wp8.rs`** reads `REVIEW_SEED`, `REVIEW_CASES` and `REVIEW_SIZE` (defaults
  11, 150 and 90).

In a dev-profile run on the 4-vCPU dev VM, all 147 tests passed. The slowest files were
`differential` (4.5 s), `optimizer` (3.7 s), `review3_semantics` (2.4 s) and `review_wp8`
(2.2 s).

| File | Covers |
|---|---|
| `lower.rs` | Slot scopes, blank nodes, constants and TEMP ids, datasets, forms, the 256-slot limit, UPDATE WHERE, a logical EXPLAIN snapshot |
| `planner.rs` | GRAPH push / loop, filter scope, OPTIONAL filters, seeds, TopK / Count / ASK, policies, EXISTS plans and decorrelation, canonical flags, SERVICE, no BindJoin into a side that mints values, joins on a GROUP BY key that evaluate the aggregation once |
| `optimizer.rs` | Merge joins and enforcers, order claims, COUNT fast paths, StarMerge, galloping, IntRange choice, SQL chain placement and the 64-alias cap, `validate()` on corrupted plans, many-to-many joins on a key with few values (no merge on the city that checks `?c`, under Auto, NoSql and PreferMerge, at 4K and 40K people; the two-key estimate within 2× of the result), no SortBySlot or hash table over a 1 MB memory budget when a plan that fits exists |
| `golden.rs` | EXPLAIN snapshots: star (and its SqlJoin on SQLite), chain, OPTIONAL, TopK, factorized count, IntRange, cycle. Update them deliberately when costs change. |
| `validate.rs` | The core `validate()` checks (seeds, keys / checks, Extend, Distinct, Active graph, scan order, sortedness, slot ranges, ids), each exercised by mutating a valid plan. The optimizer-specific checks (merge, SortBySlot, IntRange, SqlJoin) are in `optimizer.rs`. |
| `differential.rs` | 1,000 random queries × 8 option sets × {MemStore, SQLite}, executed by sq-exec against sq-oracle; every SqlJoin is checked with `EXPLAIN QUERY PLAN`; an exhaustive IntRange test; 8 many-to-many and several-key queries (same city, cycle, OPTIONAL on two keys) on a 240-person, 3-city network under every option set, with their StarMerge rewrites |
| `w3c.rs` | Every W3C `.rq` file: parse, lower, optimize, validate and props invariants. It plans on an empty store (default options) and on the test's data with and without statistics under 6 option sets. |
| `review*.rs` | Regression tests from adversarial reviews: seeded groups, absent graphs, finite estimates, a naive-evaluator differential of physical plans, planning budget, StarMerge selection, statistics scopes, no merge on an attribute key that checks the person (city, age, join day pairs), cycle estimates within 4× on random graphs |

## Performance

All numbers were measured on the project's 4-vCPU dev VM (SQLite 3.45.1 where SQLite is
involved). The sources are the authors' notes and the test comments, except for the W3C
sweep, which comes from the test run above.

- **W3C planning sweep.** 921 `.rq` files, of which spargebra rejects 101. The sweep built and
  validated 7,892 plans, with 80 planning attempts rejected as `Unsupported` (non-silent
  SERVICE). The whole test took 0.73 s (dev profile).
- **Planning time.** The target is about 2 ms per query (docs/DECISIONS.md).
  - One DP extension costs about 2.0K–2.5K instructions with a forced operator and 3.9K–4.6K
    when bind, hash and merge are compared (`physical/join.rs`). A bounded search therefore
    takes at most about 15M or 28M instructions; the `Auto` worst case took 3–10 ms in release
    on the heavily loaded VM.
  - Planning the 10-pattern 5-clique query under `Auto` costs about 2× the instructions of
    `PreferHash` (measured before the last fix round); `review_wp8` asserts at most 4× in CPU
    time.
- **Plan quality.** When the DP must fall back:
  - in the baseline planner (WP5, before the cost-based optimizer), greedy ordering on an
    11-pattern star with filters and NOT EXISTS gave an estimated cost of 927K against 805K
    under the unbounded DP of that time; this has not been re-measured since;
  - a lower DP tier costs 1.11–1.28× the full search's estimate on the 5-clique.
- **Index choice.** A random (g, s, p) point probe through GPSO takes about 1.4 µs against
  2.1 µs through GSPO.
- **StarMerge.** In release on MemStore, COUNT(*) over 300 subjects × 20 values of three
  predicates takes 2.6 ms as a merge chain (120,000 intermediate rows) and 0.6 ms as a
  StarMerge. Without the count, the StarMerge is slower: 70 ms against 46 ms for 2.4M output
  rows.
- **Joins on several keys** (release, CPU time best of 5). On the tutorial's 100K-person
  network (838K quads, B-tree reader), `?a foaf:knows ?b . ?b foaf:knows ?c . ?a ex:livesIn
  ?city . ?c ex:livesIn ?city` (292,595 rows) took 47 s of CPU time under `Auto` (55 s in
  the tutorial's run) as a `MergeJoin key=?city check=[?c]` over 9.4e9 pairs of rows. It now
  merges on `?c` and checks the city: 0.23 s (0.26 s as a COUNT(*)), against 0.35 s under
  `PreferHash` and 1.2 s under `PreferBind`. On the 1.6M-triple benchmark social graph:
  `?a :knows ?b . ?a :city ?x . ?b :city ?x` went from 17.6 s to 0.12 s; a 4-cycle that also
  checks a city from 52 s to 0.52 s; friends of friends sharing a city, an age or a class
  from over 20 s (timeout) to 0.44–0.50 s; pairs of people sharing a city and a join day
  from over 20 s to 1.2 s. On BSBM, products sharing a feature and a producer went from 7.4 s
  to 1.0 s. Cycles (triangles, mutual `:knows`, 4-cycles) keep their plans and estimates
  (21 triangles: estimated 27; 8 mutual pairs: 9). The 17 benchmark queries (count mode)
  keep their plan shapes, Q3's factorized merge join included, and their times are
  unchanged within noise (0.98–1.03×; Q4, Q7 and Q16 are 0.81×, 0.33× and 0.86× through
  changes in other crates, with the same plans).
- **Memory budget.** `SELECT (COUNT(*) AS ?n) { ?a :city ?x . ?a :age ?g . ?b :city ?x .
  ?b :age ?g }` on the benchmark social graph streams 1.2e9 city pairs in every left-deep
  order (the cities are skewed; estimated 4e7). Without the memory term, `Auto` sorted them
  for a merge and stopped after 1.1 s with "memory budget exceeded (1024 MB)"; it now
  probes a hash table of the 200K `:age` rows and returns the count after 68 s.
- **Cost of the join estimates.** Estimating each step from its keys' distinct values costs
  about 800 instructions per join step considered. Planning the 10-pattern 5-clique on the
  benchmark database went from 2.7 ms to 3.2 ms, a 9-pattern two-subject star from 1.37 ms to
  1.55 ms, and the benchmark queries 0–6% (Q2's 6-pattern star: 275 µs to 292 µs).
- **Correctness evidence.** `differential.rs` ran 16,000 executions with zero differences
  from the oracle. The test is deterministic; its tally covers 380 SqlJoins, 7,737 MergeJoins,
  2,403 enforcers, 1,624 streaming groups, 169 factorized counts and 59 IntRange scans. All
  1,649 forced StarMerge rewrites matched the oracle.

## Limitations and known gaps

- **Planning budget.** Groups with more than 12 units, or with a DP over the 6,000-unit work
  budget (about 11 or more densely connected units), use greedy ordering, which is myopic.
  `Auto` planning of the densest DP-sized groups can exceed the ~2 ms target.
- **Opaque units in the search.** The ordering search estimates opaque units from their base
  plan, keeping its operators and order, and assumes they produce no sort order. A seed that
  would allow a very different inner order is not explored.
- **StarMerge.** StarMerge is only a post-pass over merge chains the DP already chose. Its
  300 ns step cost is conservative (35–50 ns measured), so it is rarely taken outside
  `COUNT(*)`.
- **SqlJoin.** It selects every slot of its patterns, because projection pushdown (SPEC
  §7.3 item 9) is not implemented. Chains close at the first filter.
- **Streaming groups.** A Group streams only when its input happens to be sorted by its
  single key; the planner does not request that order.
- **Cost model.** The cost constants are not calibrated, and Group estimates ignore distinct
  key values. Join estimates assume runs of even size: a merge on a skewed key visits more
  pairs than estimated (the tutorial's 20 cities: 9.4e9 against 4.5e9). Several join keys
  are told apart by a fixed rows-per-value threshold (100): an attribute key that is in fact
  independent of the others is overestimated by up to `sqrt(dv)` (two random people sharing
  a city and an age: 10× with 100 ages), and correlated entity keys are underestimated
  (clustered graphs have more triangles than a random one). `dv_L` is known only from the
  prefix's quad patterns. The memory term is a step at the budget, on estimated rows.
  A merged multi-graph default graph is charged one seek per open. An IntRange scan of the
  GraphLoop register graph assumes 0 numeric exceptions.
- **Graph sort orders.** Order claims are conservative for GRAPH ?g scans over FROM NAMED lists
  of more than 8 graphs.
- **Release builds skip checks.** `validate()` and `check_barriers` run inside `optimize()` only
  in debug builds.
- **EXISTS decorrelation** is conservative about MINUS (it also walks nested EXISTS
  patterns). Empty propagation folds only top-level `[NOT] EXISTS` filter conjuncts.

## Where the code differs from docs/SPEC.md

- **§7.1 / §7.3.** EXISTS decorrelation, TopK, Count, IntRange and canonical flags are
  physical decisions, not logical rewrites. Projection pushdown (§7.3 item 9) does not exist.
  The "no filter in a LeftJoin or Minus right side" check is `check_barriers` (debug only),
  not part of `validate()`.
- **§7.2.** The GRAPH ?g push rule is tighter (`GForm` classification). `safe_inject` uses all
  EXISTS variables for Filter, Extend and LeftJoin.
- **§7.3.4.** Decorrelation has extra conditions (MINUS, non-empty S, not inside EXISTS), is
  cost-based under `Auto`, and can emit HashJoin, NestedLoop or BindJoin Semi/Anti joins.
- **§7.4.** A Group with keys is estimated at `max(1, |in| / 2)` rows (1 without keys), not
  `min(|in|, Π dv(keys))`. A join on several keys multiplies the per-key
  `1 / max(dv_L, dv_R)` of its entity keys only; attribute keys (100 or more rows per value
  in the unit), other than the most selective key, are backed off exponentially. `dv_L`
  comes from the prefix's quad patterns and is capped by its rows.
- **§7.5.** The DP is bounded by work (`MAX_DP_WORK`) with fallback tiers, and keeps open SQL
  chains as an interesting property. StarMerge is costed per distinct key, with an output-row
  term and a COUNT-aware comparison. A MergeJoin or StarMerge with checks also pays for every
  pair (combination) of its key's runs, and both pay a row copy per buffered run row unless a
  COUNT is factorized over them. Enforcers and hash tables estimated over the memory budget
  are penalized. Probes with g, s and p bound read GPSO.
- **§7.6.** In SqlJoin text the graph is a parameter too; chains close at filters and are
  capped at 64 aliases.

## Further reading

- [docs/SPEC.md](../../docs/SPEC.md): §7 (planner and cost model), §5.3–5.6 (operators,
  paths, datasets), §8.2 (`sparql_explain` modes). [docs/DECISIONS.md](../../docs/DECISIONS.md)
  (planning budget, statistics as hints, plan caching).
- The module docs of `src/physical/mod.rs` (seed contract, sortedness rules),
  `src/physical/join.rs` (ordering search) and `src/rewrite/graph.rs` (GRAPH ?g counterexamples).
- [../sq-core/README.md](../sq-core/README.md) (`Logical`, `Plan` / `Op`, `SlotSet`,
  `CostConstants`), [../sq-exec/README.md](../sq-exec/README.md) (executing plans, the seed
  contract), [../sq-sqlite/README.md](../sq-sqlite/README.md) (cursor costs, `sql_rows`,
  statistics), [../sq-memstore/README.md](../sq-memstore/README.md) and
  [../sq-oracle/README.md](../sq-oracle/README.md) (test store and reference evaluator).
- The caller, `crates/sq-engine` (`src/query.rs`, `src/explain.rs`).
