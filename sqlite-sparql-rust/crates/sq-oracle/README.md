# sq-oracle

`sq-oracle` is a deliberately naive SPARQL 1.1 query evaluator. It is transcribed from the
algebra definitions in §18 of the SPARQL 1.1 recommendation and serves as the ground truth for
sparqlite's differential tests. It takes a query that sq-plan has lowered to the Logical IR
(`sq_core::ir::LoweredQuery`) and evaluates it bottom-up:

- every operator turns whole bags of solutions into a new bag;
- joins are nested loops;
- `EXISTS` is evaluated by explicit substitution;
- `GRAPH` evaluates its pattern once per named graph;
- property paths follow the ALP procedure.

Expressions, effective boolean values, ORDER BY keys and aggregates come from sq-expr, so the
oracle and the native executor share one value model. The crate has about 1,860 lines in
`src/` outside `src/tests/`, plus about 1,700 lines of tests. It works on any `QuadStore`, but it
is a test tool: it is only a dev-dependency or an optional tool dependency, so nothing in the
shipped extension (`ext/`, `sq-api`) links it.

## Where it fits

```
   sq-core (Logical IR, TermId, codec, QuadStore)    sq-expr (expressions, aggregates)
   oxrdf  oxsdatatypes  rand  rustc-hash                     |
                         \                                   /
                                     sq-oracle
                                         |
   dev-dependency of:  sq-plan (tests/differential.rs, optimizer.rs, review_wp8*.rs)
                       sq-engine (tests/differential.rs fuzzer, review_wp10_update_oracle.rs)
   optional dependency of tools/sq-w3c (feature `oracle`, on by default): --backend oracle
   own dev-dependencies: sq-memstore (test store), sq-plan + oxttl (end-to-end tests only)
```

In a differential test, both sides start from the same lowered query:

```
SPARQL --sq_plan::parse_query--> spargebra::Query --sq_plan::lower_query--> LoweredQuery
   LoweredQuery --> sq_oracle::evaluate_with                    (expected answer)
   LoweredQuery --> sq-plan optimizer --> sq-exec Execution     (answer under test)
```

*Deviations from the spec:* the SPEC §1.4 table lists only `sq-core`, `sq-expr` and the
dev-dependency `sq-memstore`. The code also depends on `oxrdf`, `oxsdatatypes`, `rand` and
`rustc-hash`, and has the dev-dependencies `sq-plan` and `oxttl`. SPEC §11 lists only
`evaluate`; the code adds `evaluate_with` and `OracleOptions`.

## Key concepts

**1. The input is the Logical IR, not a physical plan.** The oracle sees the same
`LoweredQuery` that the planner optimizes, so it checks the optimizer (sq-plan) and the
executor (sq-exec, the stores). It cannot detect a bug in spargebra's parser or in sq-plan's
lowering, because both sides share that step. Before evaluating, `ir::validate` rejects with
`Error::Invalid` any IR it could not index safely or evaluate in finite time:

- more than `SlotSet::MAX_SLOTS` (256) variables;
- a slot outside the variable table;
- a VALUES row of the wrong length;
- a reference to an EXISTS pattern that does not exist;
- a cycle of EXISTS patterns that reference each other, found by an iterative DFS.

**2. Bags of full-width rows with canonical ids.** A solution is a row of `TermId`s, one per
slot of the query's variable table, with `TermId::UNBOUND` for slots outside its domain
(`bag.rs`). Every id the oracle creates is canonical (`resolver.rs`), so sameTerm is the same
as id equality, and compatibility and merge (§18.3) are plain id comparisons. An id is
canonical when it is:

- an inline id, if the term can be encoded inline;
- the dictionary id, if the term is stored;
- the store id, for a store blank-node label;
- otherwise a TEMP id in a copy of the query's `TempDict`.

`eval.rs` writes each operator as its §18.5 definition. `Join` merges every compatible pair.
`LeftJoin`'s filter sees the merged row, with errors counting as false. `Minus` removes a row
only if a right row is compatible with it **and** shares a bound variable. `Union` appends.
`Extend` leaves the slot unbound on an error. `ORDER BY` is a stable sort on sq-expr
`SortKey`s. `Distinct` keeps the first occurrence of each row, and `Reduced` keeps every row.

A BGP extends each partial solution one triple pattern at a time, in the order the lowered
`Bgp` lists them (no reordering). Each step is a range scan on the first
`IndexOrder::candidates` order whose prefix is exactly the bound columns.

**3. The dataset is explicit** (`dataset.rs`, SPEC §5.6). `Dataset::resolve` turns the
query's `DatasetSpec` into a default graph and a sorted list of named graphs:

- A default graph made of several stored graphs is their RDF merge, a set of triples, so a
  triple stored in two of them matches once.
- A graph name that is not in the store (a TEMP id) is an empty graph.
- `GRAPH <c>` is empty unless `c` is a named graph of the dataset.
- `GRAPH ?g` evaluates its inner pattern once per named graph and never binds the default
  graph.
- A pushed `GraphSel::Slot` in a BGP or path ranges over the named graphs and binds the slot.
- Paths never cross graphs.

**4. EXISTS by substitution** (§18.6). `exists(P)` for a solution μ evaluates `P` with μ as
the *seed* (`Ctx::seed`) instead of rewriting the tree. Every seeded slot is a constant
everywhere:

- in triple patterns and in path endpoints, including zero-length matches;
- in expressions and in `GRAPH ?g`;
- in VALUES, which drops rows that disagree with the seed;
- in `BIND`, where a row survives only if the value is the seeded term or an error.

`MINUS` leaves seeded slots out of its shared-variable test. `Project` and `Group` put the seed
back into their output. With an all-`UNBOUND` seed, which is the case for the query itself,
these rules are the plain algebra. An expression's EXISTS answers are computed for the row
before sq-expr runs, and are handed to it through the `ExistsEval` callback.

**5. Property paths** (`path.rs`, §18.4). `PathEval::pairs` returns the `(x, y)` pairs of
`Path(X, path, Y)`. Each endpoint is either `End::Fixed` (a constant or a seeded slot) or
`End::Free`.

| Path | Result | Notes |
|---|---|---|
| `:p` | bag | the matching triples of the graph |
| `^P` | as `P` | swaps the endpoints |
| `P/Q` | bag | join on a fresh, **free** middle variable, then project |
| `P\|Q` | bag | union, duplicates kept |
| `!(...)` (NPS) | set | one pair however many non-excluded predicates link x and y |
| `P?`, `P*`, `P+` | set | `*` and `+` use ALP: a DFS with a visited set, so cycles terminate |

The zero-length rules are as follows:

- A fixed endpoint matches itself even when it is not a node of the graph, so `:o :p* ?x`
  binds `?x = :o` (W3C `zero_or_one_set_start`).
- With both ends free, every node of the graph matches itself. `nodes(G)` is the subjects
  and objects of the graph; a term used only as a predicate is not a node.
- The middle of a sequence is a variable, so it only takes zero-length steps through graph
  nodes. As a result, `:zz (:p?/:q?) ?y` is empty when `:zz` is not a node, but
  `ASK { :zz :p?/:q? :zz }` is true.

To evaluate a sequence, the operand on the fixed endpoint's side drives the join. The other
operand is evaluated once with its middle free and indexed on the middle. That index is
memoized per pattern in `PathEval`, so a closure over a sequence does not recompute it at
every node.

*Deviations:* SPEC §5.5 says "`Seq` is composition" and that NPS "scans `(g,s)` and drops
`p ∈ set`". The oracle uses the §18.4 join on a free middle variable, and treats NPS as a set
of pairs. sq-exec was changed to match both points (see `notes.md`).

**6. Groups and aggregates** (`group.rs`, SPEC §6.7):

- Groups appear in order of first occurrence.
- Without keys, the whole input is one group, even when the input is empty:
  `SELECT (COUNT(*) AS ?c) {}` over no solutions gives one row with 0. With keys and no input,
  there are no groups.
- An aggregate whose value is an error leaves its slot unbound, but the group row is kept.
- `COUNT(DISTINCT *)` compares the in-scope variables of the group input (`ir::in_scope`),
  except blank-node pattern variables.
- `GROUP BY (expr)` arrives as an `Extend` that leaves the key unbound on an error. An error
  key and an unbound key therefore fall into **one** group: §18.5.1's ListEval gives `(error)`
  for both.

*Deviation:* SPEC §5.3 and `docs/DECISIONS.md` still say that `TermId::ERROR` "forms its own
group". The oracle does not do this, and neither does sq-exec's current `ops/group.rs`, which
puts error and unbound keys in one group.

**7. Deterministic environment.** `NOW()` and the random source behind `RAND()`, `UUID()` and
`STRUUID()` come from `OracleOptions`. Without options they come from the clock and the OS. The
differential tests fix both so that the native engine can be compared with the oracle.

## Public API tour

The whole public API is in `src/lib.rs`:

```rust
pub enum OracleOutput {
    Solutions { vars: Vec<String>, rows: Vec<Vec<Option<Arc<Term>>>> }, // SELECT, solution order
    Boolean(bool),                                                       // ASK
    Graph(Vec<[Arc<Term>; 3]>),           // CONSTRUCT / DESCRIBE: a set, first-seen order
}
pub struct OracleOptions { pub now: Option<oxsdatatypes::DateTime>, pub rng_seed: Option<u64> }
pub fn evaluate(q: &LoweredQuery, store: &dyn QuadStore) -> Result<OracleOutput>;
pub fn evaluate_with(q: &LoweredQuery, store: &dyn QuadStore, opts: &OracleOptions) -> Result<OracleOutput>;
```

The oracle reads the store only through `cursor()`, `named_graphs()`, `dict()` (`lookup` and
`decode_one`) and `check_interrupt()`. It runs unchanged on `MemStore` and `SqliteStore`. The
sq-engine fuzzer runs it on three stores and first checks that the three answers agree.

From SPARQL text, the pattern used by `tests/sparql.rs`, sq-engine's fuzzer and sq-w3c (this
snippet was compiled and run against the current source):

```rust
use std::str::FromStr;
use sq_core::plan::QueryOptions;
use sq_core::store::QuadStoreWriter;
use sq_core::TermId;
use sq_memstore::MemStore;
use sq_oracle::{OracleOptions, OracleOutput};

let store = MemStore::new();
let iri = |s: &str| store.intern(oxrdf::NamedNodeRef::new_unchecked(s).into());
let (a, b, c, knows) = (iri("http://ex/a")?, iri("http://ex/b")?, iri("http://ex/c")?, iri("http://ex/knows")?);
let d = TermId::DEFAULT_GRAPH;
store.insert(&mut vec![[d, a, knows, b], [d, b, knows, c], [d, c, knows, a]])?;

let parsed = sq_plan::parse_query("SELECT ?y { <http://ex/a> <http://ex/knows>+ ?y }", None)?;
let lowered = sq_plan::lower_query(&parsed, &store, &QueryOptions::default())?;
let opts = OracleOptions {
    now: Some(oxsdatatypes::DateTime::from_str("2026-10-15T12:00:00Z").unwrap()),
    rng_seed: Some(1),
};
let OracleOutput::Solutions { vars, rows } = sq_oracle::evaluate_with(&lowered, &store, &opts)? else {
    unreachable!()
};
assert_eq!(vars, ["y"]);
assert_eq!(rows.len(), 3); // :b, :c and :a (around the cycle), each once: `+` is a set
```

The unit tests skip the planner and build the IR by hand. Their `Fx` fixture in
`src/tests/mod.rs` does this; written out in full, it looks like this:

```rust
use sq_core::eval_api::TempDict;
use sq_core::ir::{DatasetSpec, DefaultGraph, GraphSel, Logical, LoweredQuery, NamedGraphs,
                  PatTerm, PathPlan, QueryForm, VarOrigin, VarTable};

let y: sq_core::Slot = 0;
let q = LoweredQuery {
    form: QueryForm::Select(vec![y]),
    root: Logical::Path {
        g: GraphSel::Default,
        s: PatTerm::Const(a),
        path: PathPlan::OneOrMore(Box::new(PathPlan::Pred(knows))),
        o: PatTerm::Slot(y),
    },
    vars: VarTable { vars: vec![("y".to_owned(), VarOrigin::User)] },
    dataset: DatasetSpec { default: DefaultGraph::StoreDefault, named: NamedGraphs::All },
    exists: Vec::new(),          // EXISTS patterns, indexed by ExistsId
    temps: TempDict::new(),      // query constants absent from the store
    base_iri: None,
};
let out = sq_oracle::evaluate(&q, &store)?;   // `a`, `knows`, `store`: previous snippet
```

Output terms are `Arc<sq_core::Term>`. Call `.to_oxrdf()` on them to convert, as
`tools/sq-w3c/src/backend/oracle.rs` does. Unbound values are `None`. Store blank nodes decode
to their `b<hex>` labels. CONSTRUCT template blank nodes are fresh TEMP blank nodes for each
solution. The output forms (`forms.rs`) behave as follows:

- **CONSTRUCT** skips template triples that have an unbound position, a literal subject or a
  non-IRI predicate.
- **DESCRIBE** returns the concise bounded description from the dataset's default graph: the
  resource's subject triples, followed recursively through blank-node objects.

## Testing

```bash
cd /home/user/research/sqlite-sparql-rust
cargo test -p sq-oracle                       # 74 tests run, 1 ignored; ~0.01 s per binary once built
cargo test -p sq-oracle --test review -- --ignored   # the known spargebra deviation (fails by design)
```

The crate's tests read no environment variables. They are:

- **Unit tests (61), on hand-built IR over `MemStore`, in `src/tests/`:**
  - `patterns` (15): BGPs, hidden blank-node variables, Unit/Empty, joins, OPTIONAL filter
    scoping, the MINUS domain rule, UNION multiplicity, Extend errors, canonical computed ids,
    VALUES with UNDEF, stable ORDER BY, Project/Distinct/Reduced/Slice, SERVICE.
  - `exists` (8): substitution, MINUS inside EXISTS, nested EXISTS, the active graph, seeded
    path endpoints, BIND/VALUES on seeded slots, and groups.
  - `graphs` (8): GRAPH semantics, merged default graphs, and named graphs absent from the
    store.
  - `paths` (8): cycles, backward closures, zero-length rules, bag vs set, NPS, and the
    sequence-middle rule.
  - `aggregates` (7); `forms` (7): output forms, `OracleOptions` and `validate` (including
    EXISTS cycles); `review` (3): adversarial review cases.
  - 5 module tests in `bag.rs`, `dataset.rs` and `resolver.rs`.
- **`tests/sparql.rs` (11):** end to end from SPARQL text through sq-plan, over inline TriG.
  Each test is a named W3C regression shape from SPEC §12.2, for example `values_and_path`,
  `exists03`, `graph-minus`, `distinct-num` and `agg-err-01`.
- **`tests/review.rs` (2 + 1 ignored):** nested sequences agree with top-level ones. The
  ignored test, `review_optional_filter_005_not_simplified_end_to_end`, documents that
  spargebra 0.4.7 applies the §18.2.2.8 simplification while parsing. The unit test
  `review_optional_filter_005_preferred_reading` shows that, given the right algebra, the
  oracle produces the expected answer.

The W3C suite (`$W3C` is the `sparql` directory of a w3c/rdf-tests checkout; `scripts/check.sh`
does not run this backend):

```bash
cargo run --release -p sq-w3c -- --suite "$W3C" --backend oracle --known-failures tools/sq-w3c/known_failures.txt
```

Tests elsewhere that use the oracle as ground truth:

```bash
cargo test -p sq-plan --test differential          # plans on MemStore + SQLite vs the oracle
cargo test -p sq-engine --test differential        # short fuzz runs (seed 1, 300 cases)
cargo test -p sq-engine --test differential -- --ignored --nocapture   # long runs: 10,000 queries, 3,000 updates
```

The fuzzer reads `SQ_FUZZ_SEED`, `SQ_FUZZ_CASES`, `SQ_FUZZ_POLICIES`, `SQ_FUZZ_UPDATES`,
`SQ_FUZZ_BNODE_ANYWHERE` and `SQ_FUZZ_VERBOSE`. It skips any case whose oracle answer has more
than 4,000 rows.

## Numbers

All numbers were measured on the project's 4-vCPU dev VM with SQLite 3.45.1:

- **W3C, `--backend oracle`:** 542 of 545 runnable tests pass (533 exact, 9 with numeric
  leniency). 393 tests are skipped, including the 94 update tests (the oracle evaluates
  queries only). The run takes 0.3 s.
  - The 3 failures are the spargebra parser divergences listed in
    `tools/sq-w3c/known_failures.txt`: `dawg-optional-filter-005-not-simplified`, and the
    parser rejecting `values-construct-group-by` and `case-insensitive-booleans`.
  - Of the lenient passes, 8 come from sq-expr's formatting of computed numerics. `tsv03` is
    an error in the expected file (`1.0e6` where the data has `1.0E6`).
- **sq-plan differential test (project log):** 1,000 random queries × 8 option sets ×
  {MemStore, SQLite}, giving 16,000 runs with 0 differences from the oracle.
- **sq-engine fuzzer (project log):** 26 seeds × 10,000 queries, about 7.7 million native
  comparisons, with zero unexplained mismatches.
- **Author's scratch comparison with spareval (outside the repo):** 3,000 random cases gave 57
  divergences, all traced to spareval: zero-length paths from constants that are not graph
  nodes, deduplicated `|`, VALUES/MINUS inside GRAPH, COUNT(*) over an empty input, filters
  that see a sub-select's unprojected variables, MINUS inside EXISTS, and NPS multiplicity.
  pyoxigraph uses the same evaluator, so it is **not authoritative for paths** either: it drops
  zero-length matches for constants that are not graph nodes and uses set semantics for `|`
  and `/`.

## Limitations

- **Queries only.** It has no SPARQL Update support. `review_wp10_update_oracle.rs` in
  sq-engine builds an update check on top of it: it evaluates WHERE with the oracle and
  instantiates the templates itself.
- **SERVICE:** a non-SILENT `SERVICE` returns `Error::Unsupported("SERVICE")`, and
  `SERVICE SILENT` evaluates to one empty solution.
- **Slow by design.** Every bag is materialized. Joins, LEFT JOIN and MINUS are quadratic
  nested loops. EXISTS re-evaluates its pattern for every row, and computes every EXISTS that
  an expression references before the expression runs, even in a branch that `||`, `IF` or
  `COALESCE` would skip. Free-free zero-length paths read the whole graph to find `nodes(G)`.
  Keep inputs small.
- **No resource limits.** `QueryOptions::timeout_ms` and `max_memory_mb` are not consulted.
  The only stop point is the store's `check_interrupt()`, which is polled at the start of
  every triple-pattern read; `MemStore` always returns `Ok`.
- **Shared upstream.** Parser and lowering bugs (spargebra, sq-plan `lower_query`) are
  invisible to differential tests, because the oracle consumes the same `LoweredQuery`.
- **Solution order** is only meaningful under ORDER BY. Compare other results as bags, modulo
  a bijection of blank nodes.

## Further reading

- `docs/SPEC.md`:
  - §5.3-5.6: operators, property paths and datasets, with the deviations noted above.
  - §6.7: aggregates.
  - §11: the sq-oracle API.
  - §12.1-12.2: per-crate tests and the W3C runner.
- `docs/DECISIONS.md`: the decision to make the oracle its own crate, and the outdated ERROR
  group-key note.
- `notes.md`: the sq-oracle, sq-exec and sq-engine fuzzer entries.
- [../sq-core/README.md](../sq-core/README.md): the Logical IR, `TermId` and the store traits.
- [../sq-expr/README.md](../sq-expr/README.md): the expression evaluator the oracle shares with
  sq-exec.
- [../sq-plan/README.md](../sq-plan/README.md): lowering and the planner differential test.
- [../sq-exec/README.md](../sq-exec/README.md): the executor the oracle checks.
- [../sq-memstore/README.md](../sq-memstore/README.md): the store used by the tests.
