# sq-exec

`sq-exec` is sparqlite's native query executor. It takes a physical `sq_core::plan::QueryPlan`
(built by `sq-plan`) and runs it directly against any `sq_core::store::QuadStore`: the SQLite
store in production, `MemStore` in tests. It is a pull-based Volcano executor over 1024-row
batches of integer term ids. Joins, OPTIONAL, MINUS, property paths, correlated EXISTS,
grouping, sorting, CONSTRUCT and DESCRIBE are all implemented here in Rust. The store supplies
index range cursors, (capped) counts and dictionary lookups. The one exception is the hybrid
`SqlJoin` leaf, which asks the store to run a planner-rendered SQL join. The crate also
owns the per-query `TermResolver`, the memory budget, the deadline and interrupt polling. The
crate is `#![forbid(unsafe_code)]`, and its whole public surface is `Execution` and
`ExecCaches`.

## Where it fits

```
  sq-core  (TermId, Batch, QuadStore, plan::QueryPlan, eval_api traits)
     |  \
     |  sq-expr  (compiled expressions, aggregates, SortKey)
     |  /
  sq-exec  ---- dev-deps: sq-memstore, sq-core[testing]
     |
     +-- sq-engine  (queries, EXPLAIN ANALYZE, the WHERE part of DELETE/INSERT)
     +-- sq-api     (the SQLite functions and virtual tables, via sq-engine or directly)
     +-- sq-plan    (dev-dependency only: end-to-end and differential tests)
```

Normal dependencies are `sq-core`, `sq-expr`, `oxsdatatypes` (the `NOW()` value), `rand`
(`RAND()`, `UUID()`), `rustc-hash`, `smallvec` and `oxrdf`. `sq-exec` does not depend on
`sq-plan` or `sq-sqlite`; it only sees the `QueryPlan` type and the `QuadStore` trait.

## Key concepts

**1. One slot layout per query.** Every variable, including hidden ones and the variables of
EXISTS patterns, has a `Slot` in one query-wide layout. A row is `[TermId; width]` with
`TermId::UNBOUND` (0) for "unbound", so operators never remap columns. `Execution::new` rejects a
plan wider than `SlotSet::MAX_SLOTS` (256) with `Error::Unsupported`. `TermId::ERROR` only
appears in GROUP BY key slots and is turned into UNBOUND on output.

**2. The operator protocol** (`ops/mod.rs`, internal). `build.rs` turns each `Plan` node into a
`Box<dyn Operator>`. Expressions are compiled once there with `sq_expr::compile`, and every node
is wrapped in `Instrumented`, which counts opens and rows per `Plan::id` and ticks the
interrupt-polling counter. The methods are:

| Method | Purpose |
|---|---|
| `open(cx, Option<Seed>)` | (re)start; may be called again without `close` |
| `next(cx, &mut Batch)` | append rows until the batch is full (`true`) or the input is exhausted (`false`) |
| `next_limited(cx, out, max)` | row-count hint: honoured by Scan, used by merge joins and row-dropping operators |
| `fast_count(cx, cap)` | count without producing rows (Scan via `store.count`, MergeJoin as a sum of run products) |
| `seek(cx, slot, key)` | galloping support (Scan, SortBySlot) |
| `reset(cx)` | release memory, keep cursors leased so the next `open` can `rebind` them |
| `close(cx)` | release cursors and memory |

*Early stopping.* `next` takes no row limit, so an input stops early only when the batch it
fills has no free space left. Consumers use that. Slice pads its private batch so that only
min(offset + limit left, 1024) rows are free. EXISTS evaluates on a batch that already holds
1023 placeholder rows. The row-dropping operators (Filter, Distinct, Reduced, Minus,
GraphLoop) pull through `rows::Pull`, which asks for the output's free space with
`next_limited` and doubles the request on each refill.

**3. Seeds: Join mode and Substitute mode** (`seed.rs`). A `Seed { row, subst: SlotSet }` is a
full-width row in which exactly the seeded slots are bound. A Join-mode seed comes from a
BindJoin probe (sideways information passing). A Substitute-mode seed comes from EXISTS: the
slot acts as a constant everywhere, including OPTIONAL and MINUS right sides, sub-selects and
inputs under a LIMIT. Both modes can be live at once. Leaves merge the whole seed into their
rows. Join-mode seeds reach every input of an inner join, except a BindJoin's right input. That
input is re-opened with exactly its own seed slots (taken from each left row) plus the
substitution. Join-mode seeds also reach the left input of Left, Semi and Anti joins and of
Minus, every Union branch, and the input of Filter, Extend, Project, Distinct, Reduced, Sort,
SortBySlot and Group. Slice, TopK, Count and GraphLoop pass only the substitution on. Project,
Group and Count write substituted constants back onto their output rows. This is the
planner's seed contract (see [../sq-plan/README.md](../sq-plan/README.md)).

**4. Scans are index range cursors** (`ops/scan.rs`). Each cursor is a `QuadCursor` leased from
the store with `store.cursor(spec)`; in sq-sqlite that is a pooled range statement or the
B-tree reader. At `open` the Scan works out which positions are bound (constants, seeds, the
graph). It keeps the planned `IndexOrder` if that order's prefix is exactly the bound columns
and the order passes the same filters as `candidates`: it is not the numeric-exceptions index,
and it is a g-last index only when the graph is free. Otherwise it takes
`IndexOrder::candidates(mask)[0].0`. If the Scan already holds a cursor of the chosen order and
prefix length, it `rebind`s that cursor, which allocates nothing. Otherwise it leases a new one.
Repeated variables (`?x :p ?x`), the INT tag of an `IntRange` scan and FROM NAMED membership
are checked after the read. An `IntRange` scan reads in two phases: first the INT range on
GPOS, then the numeric exceptions of `(g,p)` on GPOX (dictionary-typed literals and inline
decimals). Its output is therefore unsorted, and the FILTER kept above it decides on the
exceptions. A TEMP or otherwise non-storable bound value gives 0 rows without opening a cursor.
Graph selection works as follows:

- the store default graph, a single FROM graph, a constant graph, the GraphLoop register, or a
  seeded graph slot: one g-first cursor;
- several FROM graphs: a k-way merge with an adjacent dedupe (the RDF merge is a set);
- `UnionAll`: the g=0 cursor merged with the g-last partner index (SPOG, PSOG, POSG or OSPG);
- a free graph slot: with `NamedGraphs::All` the g-last cursor binds it; with a FROM NAMED list,
  one g-first cursor per graph for up to 8 graphs, otherwise the g-last cursor plus a membership
  check.

**5. Joins** (`ops/bind_join.rs`, `hash_join.rs`, `merge_join.rs`, `minus.rs`).
- **BindJoin** re-opens `right` once per run of equal adjacent seed tuples in each left batch.
  The later rows of a run reuse the buffered result of the run's probe. With `sort_left` the
  batch is sorted by seed tuple first, so each distinct tuple is probed once per batch. Without
  it, a tuple that comes back non-adjacently is probed again. The join switches to hashing
  against one unseeded evaluation of `right` in two cases. If `adaptive` is set (and `right`
  does not rely on sort orders that its seeds provide), it switches once the left side passes
  both 50,000 rows and 8x its estimate.
  If the join is Inner or Left and `right` mints values (BNODE, RAND, UUID, STRUUID), it hashes
  immediately, because one solution must not get several values.
- **HashJoin** keeps its build side in a flat row arena with `FxHashMap` chain heads. A build
  row whose key is not fully bound goes to a wildcard list that every probe visits. A probe row
  with an unbound key visits every build row. NestedLoop is a HashJoin with no keys.
- **MergeJoin** needs both inputs sorted by signed id. It buffers the right run and, after
  `gallop` misses, seeks whichever input supports `seek`. `fast_count` sums run products when
  `check` is empty. The join returns an internal error if an input turns out not to be sorted.
  **StarMerge** is a leapfrog over k inputs.
- **Minus** hashes an independent evaluation of the right side. It removes a left row only if
  some right row is compatible **and** shares a bound slot with it; substituted slots are left
  out of that domain test.

**6. Property paths** (`ops/path.rs`) follow SPARQL 1.1 §18.4 literally. `Pred` either probes
per start or scans all p edges once. The choice comes from a count of the p edges capped at the
break-even point `|starts| * seek_sorted / scan_row`. The count is remembered per predicate and
graph for the whole execution, and steps with 64 or fewer starts always probe.

`*` and `+` run a multi-source BFS with a `(start, node)` visited set. `Seq` joins on a fresh
middle variable (bag), `Alt` is a bag union, and `Nps` gives a set of pairs per start.
Zero-length matches have extra rules:

- a constant or substituted endpoint matches itself even when it is absent from the graph;
- any other value bound by a zero-length step (a Join-seeded endpoint, a sequence middle) must
  be a node of the graph (`has_node`), unless it equals a constant at the other end. So
  `:zz :p?/:q? :zz` holds even when `:zz` is not a node;
- with both endpoints free, every node of the graph matches. The nodes are found with loose
  index scans of GSPO and GOSP, and starts are evaluated in chunks of 256.

Paths never cross graphs; for a graph variable the operator loops over the named graphs itself.

**7. Correlated EXISTS** (`exists.rs`). Each `plan.exists[id]` tree is built once. On a memo
miss it is opened in Substitute mode with every bound slot of the outer row that it mentions,
nested patterns included, and `reset` afterwards so its scans can be rebound. It runs on a
private batch that already holds 1023 placeholder rows, so streaming operators stop after one
row. A row only counts if it agrees with the outer row on the substituted slots. Answers are
memoized per (id, active graph, correlated values). The memo holds at most 65,536 entries, is
charged against the budget, and is cleared rather than failing the query when memory is short.

**8. Other operators.**
- **Sort, TopK, SortBySlot** (`ops/sort.rs`). Sort is stable and computes the `SortKey`s once per
  row. TopK keeps a bounded heap of offset + limit rows. SortBySlot is the merge-join enforcer
  and sorts by the raw signed id of one slot. When every key is a bare variable and every value
  is an inline INT, Sort and TopK compare raw ids and build no `SortKey` (the "INT fast path").
- **Group, Count** (`ops/group.rs`). Group uses hash aggregation, with groups in first-seen
  order, or streams when the input is sorted by its single key. DISTINCT aggregates are
  deduplicated in a per-group set charged against the budget. Count is COUNT(*) without
  DISTINCT or GROUP BY: it uses its input's `fast_count` (a store count or a factorized merge
  join) and iterates otherwise.
- **Distinct** keeps the first row of each key in a hash set. **Reduced** drops only adjacent
  duplicates.
- **Union** (`ops/union.rs`) runs its branches one after another (bag semantics) and opens each
  one lazily with the seed.
- **GraphLoop** (`ops/graph.rs`) pushes each named graph onto the graph register around every
  `open` and `next` of its input, because inputs open subtrees lazily (Union branches, BindJoin
  probes, EXISTS evaluations).

**9. Terms, memory and interrupts** (`resolver.rs`, `ctx.rs`).

*Decoding.* The resolver decodes inline and blank-node ids with no I/O and TEMP ids from the
execution's `TempDict`. Dictionary ids go to `store.dict()`, which is where sq-sqlite applies
its connection caches. `decode_batch` sorts and deduplicates the misses. There are two
per-execution maps, both cleared when they fill up: decoded terms (up to 131,072) and canonical
lookups, misses included (up to 65,536).

*Memory.* Every materializing structure holds a `Charge` against `max_memory_mb` MiB, and going
over gives `Error::MemoryBudget`. That covers hash tables, buffered join runs, sort rows, DISTINCT
sets, groups, DISTINCT-aggregate sets, GROUP_CONCAT strings, path pairs, the CONSTRUCT and
DESCRIBE output sets, and new TEMP terms. TEMP terms stay charged until the execution ends.

*Interrupts.* `store.check_interrupt()`, the deadline and the memory check run every 4,096 rows
of work. That counts operator output, scan reads, hash builds, path steps and probe candidates.
Each API call also checks the deadline.

## Public API tour

`Execution::new` validates the layout, builds the operator trees, fixes `NOW()` and starts the
timeout clock. The deadline is `Instant::now() + timeout_ms`, taken in `new`, so time spent
between `new` and the first call counts against it. Rows are produced from the first call on.

Most callers should not chain the planner steps by hand. `sq_engine::prepare(store, &query,
&opts, caches)` returns a ready `Execution`, and `sq_plan::plan_query` is `lower_query` +
`optimize` in one call. Spelled out, this is what sq-engine's `prepare` does
(`sq-engine/src/query.rs`):

```rust
use std::rc::Rc;
use sq_core::eval_api::SolutionSource;
use sq_core::plan::QueryOptions;
use sq_exec::{ExecCaches, Execution};

let opts = QueryOptions::default(); // timeout_ms: None, max_memory_mb: 1024
let query = sq_plan::parse_query("SELECT ?s ?o WHERE { ?s <http://ex/p> ?o }", None)?;
let lowered = sq_plan::lower_query(&query, &*store, &opts)?;
let plan = sq_plan::optimize(lowered, &*store, &opts)?;
let mut exec = Execution::new(Rc::clone(&store), plan, &opts, ExecCaches::default())?;

let width = exec.variables().len();        // number of projected variables
let mut ids = Vec::new();
while exec.next_rows(&mut ids, 1024)? > 0 {} // appends width ids per row, 0 = unbound
let mut terms = Vec::new();
SolutionSource::decode(&mut exec, &ids, &mut terms)?; // Option<Arc<Term>> per id
```

Things to know when you call it:

- Output ids can be TEMP ids that only this `Execution` can decode, so decode them before you
  drop it. `Execution` implements both `SolutionSource` and `TripleSource`, and each has a
  `decode` method. Import only one of the traits, or call it fully qualified as above.
- The form decides which calls work. SELECT uses `next_rows`. ASK uses `ask()`, or `next_rows`,
  which gives one zero-width row iff the answer is true. CONSTRUCT and DESCRIBE use
  `TripleSource::next_triples(&mut out, max)`, which returns a deduplicated set in first-seen
  order. Using the wrong source returns `Error::Invalid`. `form()` returns the plan's
  `QueryForm`; sq-engine uses it to choose the serializer.
- `count()` and `ask()` consume the execution. `count()` returns the number of solutions not
  yet read (triples for CONSTRUCT and DESCRIBE, 0 or 1 for ASK) without decoding anything, then
  closes the tree. If nothing has been read yet, it uses the root's `fast_count`. For example,
  a plain triple pattern turns into one `store.count`. `ask()` closes the tree after the first
  solution.
- `set_now(oxsdatatypes::DateTime)` pins `NOW()`; sq-engine uses it so that a whole update
  request sees one value. `memory_used()` reports the bytes currently charged.
  `enable_timing()` adds per-node wall time to `profile()`, which returns an
  `sq_core::plan::ExecProfile` with `opens`, `rows_out` and `seeks` per `Plan::id`, decode
  hits and misses, and `exec_ns`. `plan_ns` is left at 0 for the caller to fill in.
- The options that sq-exec reads are `timeout_ms`, `max_memory_mb` and `base_iri`, the last
  only when the plan has none. `ExecCaches` is an empty placeholder.
- The errors a caller sees are `Error::Timeout` (the deadline), `Error::Interrupted` (from
  `store.check_interrupt()`), `Error::MemoryBudget(mb)`, `Error::Unsupported` (from `new`, for a
  layout wider than 256 slots), `Error::Invalid` (the wrong source for the form),
  `Error::Internal` for a broken plan invariant such as an unsorted MergeJoin input, and
  whatever the store returns.
- When an API call fails, the operator tree is closed. Dropping an `Execution` returns every
  cursor to the store.

The unit tests build plans by hand with no planner involved, using the helpers in
`src/tests/mod.rs` (`Fx`, `Q`, `scan`, `values`, `filter`, ...). This is
`exists_is_memoized_per_correlated_tuple` from `src/tests/exists.rs`. `data()` and
`exists(id)` (which is `Expr::Exists(ExistsId(id))`) are helpers in that file:

```rust
#[test]
fn exists_is_memoized_per_correlated_tuple() {
    let (fx, [a, b, cc, _, p, ..]) = data();
    let rows: Vec<Vec<TermId>> = [a, b, a, cc, a, b].iter().map(|x| vec![*x, int(0)]).collect();
    let p0 = filter(values(&[0, 2], rows), exists(0));
    // The pattern mentions ?0 and ?1 only: ?2 (different per row or not) is not part of the key.
    let mut e = Q::new(&fx.store, 3, p0).exists(vec![scan(v(0), c(p), v(1))]).exec(fx.rc());
    assert_eq!(collect(&mut e).unwrap().len(), 5);
    // Node ids: 0 Filter, 1 Values, 2 the EXISTS scan.
    assert_eq!(e.profile().nodes[2].opens, 3, "one evaluation per distinct ?0");
}
```

## Adding a plan operator

A new `sq_core::plan::Op` variant needs these changes in this crate:

1. An `Operator` in `ops/<file>.rs`. `open` follows the seed contract above, `next` stops when
   the batch is full, `reset` keeps cursors leased (the default just closes), and every
   materializing structure holds a `Charge`. `fast_count`, `next_limited` and `seek` are
   optional.
2. In `build.rs`: an arm in `build_in` (the `Instrumented` wrapper is added there for every
   node), an arm in `mints_values` (an exhaustive match: does the node evaluate BNODE, RAND,
   UUID or STRUUID?), and a case in `needs_sorted_input` if the node relies on sorted input.
3. An arm in `exists::collect` (an exhaustive match) that lists every slot the node mentions
   and its EXISTS ids. It is used both for the layout check and for EXISTS memo keys.

Outside this crate, the variant and `Plan::children` live in sq-core. sq-plan has per-op
matches in `validate.rs`, `props.rs`, `explain.rs` and `physical/` (`estimate.rs`, `finish.rs`,
`mod.rs`). The compiler flags the exhaustive matches. It does not flag `needs_sorted_input` or
a match with a wildcard arm, such as the one in `physical/estimate.rs`.

## Testing

```sh
cargo test -p sq-exec                                            # 113 passed, 2 ignored
cargo test --release -p sq-exec bench_operators -- --ignored --nocapture --test-threads=1
```

The second command runs the operator micro-benchmark. The filter `bench` alone would also match
`bench_hash_join_only` and run both benchmarks at once, which skews the numbers.

The tests read no environment variables. All tests are unit tests under `src/tests/`, plus a few
in `seed.rs` and `rows.rs`. They run hand-built plans over `MemStore` or the `TestStore`
wrapper. TestStore counts cursors, store counts (uncapped counts separately) and interrupt
polls. It can also inject interrupts and supply canned `SqlJoin` rows.

| Module | Tests | Covers |
|---|---|---|
| `scan.rs` | 11 | bound positions, repeated variables, TEMP constants, IntRange, fast counts, every graph mode |
| `joins.rs` | 17 | every join operator x kind, run reuse, adaptive switch, galloping, factorized counts, StarMerge, Minus, Union |
| `seeds.rs` | 2 | 28 subtree shapes: Join-seeded = unseeded + HashJoin; substituted = semi join |
| `prop.rs` | 2 | 60 random graphs and join trees: BindJoin = HashJoin (both sides) = NestedLoop = MergeJoin; merge `fast_count` = iterated count |
| `paths.rs` | 12 | every path form, zero-length rules, NPS multiplicities, cycles, graphs, brute-force reachability for both expansion strategies, capped edge counts |
| `exists.rs` | 11 | substitution through OPTIONAL/MINUS/FILTER/GRAPH, memoization, nesting, bounded memo, cursor reuse |
| `modifiers.rs` | 15 | Filter, Extend, Group/Count, Sort/TopK/SortBySlot, Distinct, Reduced, Slice, Project |
| `graphs.rs`, `construct.rs` | 6, 5 | GraphLoop register, GraphNames; CONSTRUCT, DESCRIBE, ASK, decoding |
| `limits.rs` | 8 | interrupts, deadlines, the memory budget, polling of probes that find no match |
| `review.rs`, `review_exec.rs` | 8, 13 | reviewer-pinned semantics; inputs that cross batch boundaries; resource handling |

End-to-end coverage lives in other crates. These are the main checks of sq-exec:

- **The differential fuzzer**, `crates/sq-engine/tests/differential.rs` (with `tests/fuzz/`). It
  runs random queries through sq-plan + sq-exec under 5 join policies, with reorder on and off,
  on 3 stores: MemStore, in-memory SQLite, and a SQLite file with 512-byte pages that
  exercises the B-tree reader. It compares every result against sq-oracle. The project log
  records about 7.7 million comparisons in the long runs. They found an sq-exec bug, and
  `BNODE()` being re-evaluated per seed on a BindJoin probe side.
- **The W3C runner**, `tools/sq-w3c`, with its `known_failures.txt`. The project log records
  636 of 639 tests passing on the `mem` backend (MemStore + sq-engine). The 3 failures are
  spargebra parser divergences.

`sq-plan/tests/differential.rs` also runs planner + sq-exec against sq-oracle on MemStore and
SQLite, and `sq-engine` and `sq-api` have their own query tests. An earlier scratch W3C sweep
(sq-plan + sq-exec on MemStore, not committed) gave 528 passes and 8 lenient results under the
Auto, PreferBind and PreferHash join policies and with reorder off.

## Performance

`bench_operators` (`src/tests/bench.rs`) uses 200K-row predicates in a store that replays cached
ranges. It therefore measures executor overhead per input row, with no SQLite I/O; for store
costs see [../sq-sqlite/README.md](../sq-sqlite/README.md). The table gives ns per input row,
release build, on the project's 4-vCPU dev VM. The first column is from the author's notes. The
second is from 2026-10-15, with `bench_operators` run alone (`--test-threads=1`, as above). The
low end of each range is the best of four runs at a load average near 2. The other end is one
run on the idle VM (load 0.03). Each figure comes from a single timed pass after one warm-up
pass, so expect 10-20% variation between runs, and more under load.

| Shape | Author | 2026-10-15 |
|---|---|---|
| scan | 22 | 20-21 |
| FILTER `?o > k` / BIND `?o + 1` | 60 / 82 | 59 / 79-80 |
| merge join / factorized merge COUNT | 66 / 28 | 55-56 / 22 |
| hash join (both inputs counted) | 335-480 | 298-308 |
| sort, INT raw-id fast path / by `-?o` (SortKeys) | 180 / – | 116-122 / 1,200-1,370 |
| TopK 10 / group by (1000 groups) / distinct | 51 / 81 / 93 | 45-47 / 89-96 / 106-109 |

Probe-heavy shapes on MemStore (BindJoin: about 1.3 µs per probe in the author's notes, 1.1-1.4
µs on 2026-10-15) mostly measure the range copy MemStore makes on every `rebind`, not the
executor.

## Where the code differs from docs/SPEC.md

- **Operator trait (§5.1).** `open` takes `Option<Seed>` (row plus substituted `SlotSet`)
  instead of `(&[TermId], SeedMode)`. The trait adds `next_limited`, `seek` and `reset`, and
  `fast_count` takes a `cap`.
- **ExecCaches (§11, §4.3).** It is an empty struct. The connection DecodeCache and LookupCache
  are applied behind `store.dict()`.
- **HashJoin (§5.3).** There is no bitmask-grouped `CompatTable`. Wildcard lists and per-candidate
  `check` comparisons keep the join exact.
- **Group (§5.3).** A GROUP BY expression that errors and one that evaluates to unbound form
  *one* group: both write `TermId::ERROR`, which is output as unbound (SPARQL §18.5.1; the
  oracle work corrected the spec). A plain-variable key that is unbound stays UNBOUND, a group
  separate from ERROR. `review_exec::review_error_and_unbound_group_keys_form_one_group` pins the
  rule. The old "own group" wording survives in the doc comments on `Builder::group_input` and
  `Extend` (`ops/filter.rs`), in sq-core's `Op::Group` doc, and in the name of the test
  `erroring_group_keys_form_their_own_group`, whose assertions follow the new rule.
- **Paths (§5.5).**
  - A sequence middle is a join variable.
  - `Alt` is a bag union at every level.
  - Visited entries are `(start, node)` pairs in one set.
  - The probe-or-scan choice uses a capped count cached per execution, not a per-level comparison
    of exact sizes.
  - A merged default graph (several FROM graphs, or the union default graph) always probes,
    because a merged scan has no `fast_count`.
- **MergeJoin (§5.3).** Either input may seek, not only a right-hand Scan.
- **SqlJoin (§5.3).** Seed values (in practice the EXISTS substitution) are applied to its rows as
  a compatibility filter.
- **EXISTS memo (§5.1).** The memo is bounded and gives way under memory pressure.

## Limitations and known gaps

- **Single-threaded.** State is shared through `Rc` and `RefCell`, so an `Execution` is not
  `Send`.
- **No streaming for some outputs.** CONSTRUCT and DESCRIBE keep every distinct output triple
  for the whole execution, charged at 56 bytes each. DESCRIBE reads all solutions before
  emitting anything. A path materializes all its pairs per graph, or per 256-start chunk. TEMP
  terms stay charged to the end. So a large CONSTRUCT, or a query that mints many strings or
  blank nodes, can fail with `Error::MemoryBudget`.
- **Bag semantics by design.** `Alt` keeps duplicates, so `!(:q|^:q)` counts a self-loop twice.
- **Longer cursor leases.** A subtree that is `reset` instead of closed keeps its cursors leased
  until the root finishes. This applies to EXISTS trees, hash build sides, Minus right sides and
  sort inputs.
- **Edge-count cache assumes a fixed store.** The path cache expects the store not to change
  during an `Execution`. A stale count can only pick the slower strategy, never a wrong answer.
- **Interrupt latency.** Interrupts are seen at most every 4,096 rows of work, not on every row.
- **Gaps in `fast_count`.** IntRange scans, merged-graph scans and scans with post-checks
  iterate instead of counting in the store.
- **Known W3C failures are parser divergences.** `tools/sq-w3c/known_failures.txt` lists three
  tests that spargebra 0.4.7 parses differently. It folds `OPTIONAL {{ P FILTER(F) }}` into the
  LeftJoin condition (`dawg-optional-filter-005-not-simplified`), rejects VALUES after
  CONSTRUCT .. GROUP BY, and rejects `TRUE` / `False`. None of them is an sq-exec issue.

## Further reading

- [`docs/SPEC.md`](../../docs/SPEC.md):
  - §4.1-§4.3: rows, late materialization, decode paths;
  - §5.1, §5.3, §5.4: execution model, operators, algebra mapping;
  - §5.5 (paths), §5.6 (datasets), §5.7 (CONSTRUCT, DESCRIBE, update);
  - §6.7: aggregates;
  - §11: the stub API.
- [`docs/DECISIONS.md`](../../docs/DECISIONS.md): the decoupling of expressions and EXISTS from
  execution, Substitute mode, `safe_inject`.
- [../sq-core/README.md](../sq-core/README.md): `TermId`, TEMP ids, `Batch`, `QuadStore`,
  `plan::Op`.
- [../sq-expr/README.md](../sq-expr/README.md): expression evaluation, aggregates and SortKeys.
- [../sq-plan/README.md](../sq-plan/README.md): how plans, seeds and `sorted_by` claims are
  produced.
- [../sq-sqlite/README.md](../sq-sqlite/README.md) and
  [../sq-memstore/README.md](../sq-memstore/README.md): the stores behind the cursors.
- [`crates/sq-engine/src/lib.rs`](../sq-engine/src/lib.rs) (`prepare`, `query_to_string`,
  `explain`, `update`) and [`crates/sq-api/src`](../sq-api/src): the callers. Neither crate has a
  README yet.
- [../sq-oracle/README.md](../sq-oracle/README.md): the reference evaluator.
- [`tools/sq-w3c`](../../tools/sq-w3c) and its
  [`known_failures.txt`](../../tools/sq-w3c/known_failures.txt): the W3C test runner.
