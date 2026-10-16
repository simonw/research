# sq-sqlgen

`sq-sqlgen` is sparqlite's baseline SPARQL-to-SQL translator. It turns a lowered SELECT or ASK
query (`sq_core::ir::LoweredQuery`) into one SQL statement over the same `main.rdf_quad` table
the native engine reads (plus `main.rdf_graph` for GRAPH membership and the named-graph list),
returning the same integer `TermId`s. Storage, dictionary, ids and the decode/output path are
shared, so a benchmark between the two engines measures only the evaluation strategy: SQLite's
planner and VDBE against sparqlite's own. The crate produces SQL text only. It opens no
connection and computes no values. Joins, id comparisons, grouping and ordering run in SQL.
Anything that needs RDF value semantics calls a helper SQL function (`sparql_num`,
`sparql_eval`, …). This crate defines the helpers' contract, and `sq-api` implements them with
`sq-expr`. Size: about 3,400 lines of non-test code, 1,200 lines of unit tests (`src/tests/`)
and 2,800 lines of integration tests (`tests/`).

## Where it fits

```
        sq-core   LoweredQuery, Logical, Expr, TermId / Tag, codec, Error
           |
       sq-sqlgen  translate() -> SqlQuery { sql, params, columns }; HELPER_FUNCTIONS, eval_template()
           |
        sq-api    sparql_to_sql, sparql_query_sql, sparql_count_sql, sparql_explain(q, 'sql' | 'sqlplan'),
                  the `engine` = `sql` setting; registers the helper functions on every connection
```

The only normal dependency is `sq-core`, and `sq-api` is the only dependent. `BaselineRun` in
`crates/sq-api/src/query.rs` translates a lowered query, steps the SQL on the same connection
and decodes the result ids. The dev-dependencies (`sq-core` with its `testing` feature,
`sq-plan`, `sq-memstore`, `sq-sqlite`, `sq-expr`, `rusqlite`, `spareval`, `spargebra`, `oxrdf`,
`oxrdfio`, `oxttl`, `oxsdatatypes`, `sparesults`, `rand`) serve the test harness, which runs the
SQL on a real SQLite store and compares the answers with spareval.

### Using the baseline from SQL

After loading the extension in the `sqlite3` CLI (see [sq-api](../sq-api/README.md)), three
functions always use the baseline. The per-connection `engine` setting, or an `engine` option
on one call, routes `sparql_query`, `sparql_rows` and the other query functions through it too:

```sql
SELECT sparql_to_sql('SELECT ?s WHERE { ?s ?p ?o } LIMIT 3');
-- SELECT t1.s AS v0 FROM main.rdf_quad AS t1 WHERE t1.g = 0 LIMIT ?1
SELECT sparql_explain('SELECT ?s WHERE { ?s ?p ?o } LIMIT 3', 'sqlplan');
-- SEARCH t1 USING COVERING INDEX rdf_gosp (g=?)
SELECT sparql_query_sql('SELECT ?s WHERE { ?s ?p ?o } LIMIT 3');   -- SPARQL-JSON
SELECT sparql_count_sql('SELECT ?s WHERE { ?s ?p ?o }');
SELECT sparql_config('engine', 'sql');      -- sparql_query now runs on the baseline
SELECT count(*) FROM sparql_rows('SELECT ?s WHERE { ?s ?p ?o }', '{"engine":"sql"}');  -- one call
```

A query outside the subset fails with, for example,
`sparql: unsupported: baseline: CONSTRUCT queries`.

## Key concepts

**1. One statement, ids in, ids out.** `SqlQuery::sql` is a single `SELECT`. It may nest
derived tables, correlated `EXISTS` and `WITH RECURSIVE`. For SELECT there is one INTEGER
`TermId` column per entry of `columns` (aliased `v<slot>`; read them by position), and NULL
means unbound. A query that projects nothing has one placeholder column, `NULL AS u`. ASK
yields at most one row (`LIMIT ?n` bound to 1), and the answer is whether a row exists.
CONSTRUCT and DESCRIBE are `Error::Unsupported`.

**2. Every query constant is a parameter.** `?N` binds `params[N - 1]`. `Params` (`src/sql.rs`)
deduplicates values, and `Params::finish` renumbers placeholders by first use and drops values
of fragments the translator discarded. Every value is referenced, so
`sqlite3_bind_parameter_count == params.len()`. LIMIT/OFFSET values and bounds derived from
query constants are parameters too. Fixed encoding constants (tag numbers, `TRUE_ID` 23, the
`.0` bit 256, mantissa and inline-integer ranges) are SQL integer literals. The only string
literals in the SQL are `sparql_eval` operator names and XSD datatype IRIs from static tables.
`g = 0` and `g <> 0` are written literally, so the schema's `WHERE g<>0` partial indexes stay
usable. There is no `INDEXED BY`: SQLite plans with whatever `ANALYZE` left in `sqlite_stat1`.

**3. Blocks and bindings.** A graph pattern becomes a `Block` (`src/sql.rs`): a flat `SELECT`
under construction with FROM items (quad aliases `tN`, derived tables `dN`, `LEFT JOIN`s),
WHERE conditions and one `Col` per bound slot. A BGP is a self-join of `main.rdf_quad`
aliases. Joins concatenate FROM lists, so SQLite plans a whole group as one join. A slot bound
on both sides must be compatible (`compat` in `translate.rs`). If neither side may be unbound,
the condition is `a = b`. If only `b` may be unbound, it is `(b IS NULL OR b = a)` and the
certainly-bound `a` is kept. If both may be unbound, it is `(a IS NULL OR b IS NULL OR a = b)`
and the merged value is `COALESCE(a, b)`. Identical SQL on both sides needs no condition.

| SPARQL | SQL |
|---|---|
| OPTIONAL | `LEFT JOIN <right> ON <right's conditions> AND <compat> AND <filter>`; a right side that is not one plain FROM item is wrapped as a derived table |
| UNION | `UNION ALL` of the branches, NULL-padded to the union of their slots |
| MINUS | `NOT EXISTS (…)` with the shared-domain condition |
| `[NOT] EXISTS` | correlated `[NOT] EXISTS (SELECT 1 …)` |
| VALUES | parameters for one row, otherwise a `VALUES` derived table (UNDEF is NULL) |
| GROUP BY | SQL `GROUP BY` in a derived table; COUNT is `(COUNT(…) << 4) \| 6` (an inline integer id); SUM, AVG, MIN, MAX, SAMPLE, GROUP_CONCAT call aggregate helpers; never-bound keys are omitted, and if no key is ever bound the query groups by `NULL` (so an empty input still has no group) |
| ORDER BY | `sparql_sortkey(id)` per key |
| DISTINCT + ORDER BY a non-projected key | `row_number() OVER (ORDER BY …)`, then `GROUP BY` the projection and `ORDER BY min(rn)` |
| LIMIT / OFFSET | `LIMIT ?n [OFFSET ?m]`; OFFSET without LIMIT binds the LIMIT parameter to -1 |

**4. Expressions: conditions or terms, with id fast paths.** `src/expr.rs` compiles an
expression as a condition (1, 0, or NULL for a SPARQL error), so SQL's three-valued AND/OR/NOT
match `&&`/`||`/`!`, or as a term (an SQL expression yielding a `TermId`). In a FILTER or
OPTIONAL condition (`Ctx::Truth`) false and error may be conflated, so `?n = "Alice"` is plain
id equality. Under NOT, inside IF/COALESCE and for values (`Ctx::Exact`) the error must survive:
simple-literal equality keeps the id test but sends TYPED/TEMP rows to `sparql_eval`, and only
an operand that is not a column or parameter falls back to `sparql_eval` entirely. Most fast
paths test the tag bits `id & 15` (see `sq_core::Tag`). They apply only when the tested operand
is a column or parameter: any other operand appears once in the SQL, so an expression's SQL
stays linear in its size.
- IRI and language-tagged constants compare by id (ids are canonical). Two columns: same id is
  equal unless it may be a NaN (TYPED or TEMP); an IRI or blank node against another id is
  unequal; the rest goes to `sparql_eval`.
- Ordering against an inline integer or decimal constant (`src/numeric.rs`) splits on the row's
  tag. INT (6): the payload `x >> 4` against an integer floor/ceiling of the constant. DECIMAL
  (8): `sparql_num(x)` against the constant, only where the double comparison is provably exact
  (row mantissa up to 2^53 - 1 against an integer, 2^51 - 1 against a decimal, and for a
  decimal constant its own mantissa up to 2^51 - 1). TYPED (5), TEMP (14) and other decimals:
  `sparql_eval`. Any other tag is not numeric, so the result is NULL (error). Equality and
  `IN` use the same split, except that a non-numeric row is 0 (false), not NULL, and inline
  decimals need no helper: they compare by id key as `(x | 256)`, which normalizes the `.0`
  marker bit.
- Inline dateTime/date constants compare by payload when tag and timezone bits match.
- `+ - *` of inline integers stay in SQL while the result fits an inline integer.
- `isIRI`/`isBlank`/`isLiteral` test tag bits. `CONTAINS`/`STRSTARTS`/`STRENDS` with a
  simple-literal constant become `instr`/`substr` over `sparql_str`. `REGEX` is `sparql_regex`.

**5. Computed values are computed once.** A BIND whose value is computed stays in the block's
SELECT list until something reads the variable (FILTER, another BIND, a join, ORDER BY, GROUP
BY, EXISTS). Before that read, `Translator::settle` wraps the block as an opaque derived table
`(SELECT … LIMIT -1) AS dN` and the reader uses its column. `LIMIT -1` keeps SQLite from
flattening it or pushing conditions into it, either of which would paste the expression into
every reference. Sub-selects, UNIONs and OPTIONAL right sides that expose a computed column are
opaque for the same reason. Before this rule, BIND chains made the SQL grow exponentially
(`tests/review_round2.rs`). A `debug_assert!` in `View::read` catches unsettled reads.

**6. EXISTS uses substitution semantics.** An EXISTS pattern is translated in an environment
(`Env::outer`) holding the enclosing row's bindings. Inside the pattern those variables are
correlated references that act as constants (SPARQL 1.1 §18.6). A variable that EXISTS
substitutes is left out of a nested MINUS's domain test, as `docs/DECISIONS.md` specifies
(pinned in `SPEC_ANSWERS` of `tests/equivalence.rs`).

**7. Datasets, GRAPH and property paths.**
- Default graph: `g = 0`. One FROM graph: `g = ?N`. Several FROM graphs are an RDF merge (a
  set): `g IN (…)` plus a `NOT EXISTS` probe that keeps only the copy in the smallest graph.
  `union_default_graph` lists the graphs with a loose index scan (`WITH RECURSIVE gs(g) …`).
- `GRAPH <c>` adds a membership test on `rdf_graph`/`rdf_quad` unless every solution of the
  body reads a quad of `c` (with FROM NAMED, membership is decided from the list).
- `GRAPH ?g` is pushed into the body's quad patterns, which bind `?g` with `g <> 0`. That is
  correct only if every solution reads the graph through a quad, so `src/graph.rs` classifies
  the body: `GForm` is pushed, `Indep` (never reads the active graph) is joined with the list
  of named graphs, mixed (`Bad`) bodies are unsupported.
- A property path (`src/path.rs`) is a derived table of `(s, o)` pairs. `p` and `^p` are quad
  patterns, sequences are joins and alternatives `UNION ALL` (bags), `!(…)` is `p NOT IN (…)`.
  `p+`/`p*` are `WITH RECURSIVE … UNION` (sets), seeded from a constant endpoint when there is
  one. `p*`, `p?`, and `x+` when `x` can match zero-length (`(p*)+`), add zero-length pairs.

**8. Every accepted statement prepares on SQLite 3.45.1.** The host links the system
libsqlite3 3.45.1 with default limits. By construction, joins stay within 64 tables
(`cap_tables` turns a block into one opaque derived table; path sequences split at 63),
compound SELECTs are chunked at 500 terms (`union_all`), and AND/OR lists become balanced runs
of at most 64. `src/limits.rs` then scans the finished text with a conservative model of
3.45.1's fixed 100-entry parser stack (calibrated by measurement, replayed in
`src/tests/limits.rs`) and checks expression depth (1000), function arguments (127), result
columns (2000), parameters (250,000) and an 8 MiB size budget (also checked per pattern while
building). Anything over a limit is `Error::Unsupported("baseline: …")`, for example
`baseline: query nesting too deep for SQLite's parser`.

## Public API

```rust
// crate root
pub struct SqlQuery { pub sql: String, pub params: Vec<i64>, pub columns: Vec<String> }
pub fn translate(q: &LoweredQuery) -> Result<SqlQuery>;
pub use helpers::{HELPER_FUNCTIONS, HelperFunction, HelperKind, eval_template};

// sq_sqlgen::helpers
pub const HELPER_FUNCTIONS: &[HelperFunction];
pub enum HelperKind { Scalar, Aggregate }
pub struct HelperFunction { pub name: &'static str, pub min_args: usize, pub max_args: usize,
                            pub kind: HelperKind, pub semantics: &'static str }  // max_args usize::MAX = variadic
pub fn helper(name: &str) -> Option<&'static HelperFunction>;
pub fn eval_template(op: &str, nargs: usize) -> Option<Expr>;
pub const TRUE_ID: i64 = TermId::from_bool(true).raw();    // 23
pub const FALSE_ID: i64 = TermId::from_bool(false).raw();  // 7
pub const INT_TAG: i64 = Tag::Int as i64;                  // 6
```

Translate after lowering against the store the SQL will run on: a constant absent from its
dictionary becomes a TEMP id, and a BGP with a TEMP constant translates to an empty block.
Adapted from `src/tests/golden.rs` and `src/tests/mod.rs`, where `store` is any `QuadStore`
(the tests use an `sq_memstore::MemStore` with `:age` interned):

```rust
use sq_core::{TermId, plan::QueryOptions};
let parsed = sq_plan::parse_query("PREFIX : <http://ex.org/> SELECT ?x ?w WHERE { ?x :age ?a BIND(?a + 1 AS ?w) }", None)?;
let lowered = sq_plan::lower_query(&parsed, &store, &QueryOptions::default())?;
let q = sq_sqlgen::translate(&lowered)?;
assert_eq!(q.columns, ["x", "w"]);
// The integer payload 1 (inline fast path), then the id of "1"^^xsd:integer (sparql_eval fallback).
assert_eq!(q.params[..2], [1, TermId::from_int(1).unwrap().raw()]);
```

Running it, as `tests/common/mod.rs` does once the helpers are registered on `conn`:

```rust
let mut stmt = conn.prepare(&q.sql)?;
let width = q.columns.len().max(1);                      // 1 = the placeholder column
let mut rows = stmt.query(rusqlite::params_from_iter(q.params.iter()))?;
while let Some(row) = rows.next()? {
    let ids: Vec<Option<i64>> = (0..width).map(|i| row.get(i)).collect::<Result<_, _>>()?;
    // decode through the store's dictionary and this execution's clone of lowered.temps
}
```

`SELECT ?x ?n WHERE { ?x a :Person ; :name ?n ; :age ?a FILTER(?a >= 18) }` becomes (golden
SQL, line breaks added):

```sql
SELECT t1.s AS v0, t2.o AS v1
FROM main.rdf_quad AS t1, main.rdf_quad AS t2, main.rdf_quad AS t3
WHERE t1.g = 0 AND t1.p = ?1 AND t1.o = ?2 AND t2.g = 0 AND t1.s = t2.s AND t2.p = ?3
  AND t3.g = 0 AND t1.s = t3.s AND t3.p = ?4
  AND CASE WHEN (t3.o & 15) = 6 THEN (t3.o >> 4) >= ?5
           WHEN (t3.o & 15) = 8 AND (t3.o >> 9) BETWEEN -9007199254740991 AND 9007199254740991
             THEN sparql_num(t3.o) >= ?5
           WHEN (t3.o & 15) IN (5, 8, 14) THEN sparql_eval('>=', t3.o, ?6) = 23 END
```

### The helper-function contract

`HELPER_FUNCTIONS` is the single source of truth; each `semantics` text is what `sq-api` must
implement, and `sq-api`'s `every_helper_of_the_baseline_contract_is_registered` test iterates
over it.

| Function | Args | Result |
|---|---|---|
| `sparql_num(id)` | 1 | INTEGER for xsd:integer and derived types; REAL for decimal (correctly rounded), float, double (NaN is NULL); else NULL |
| `sparql_str(id)` | 1 | TEXT lexical form of a simple, xsd:string or language-tagged literal; else NULL (an argument check, not `STR()`) |
| `sparql_ebv(id)` | 1 | effective boolean value: 1, 0, or NULL on error |
| `sparql_sortkey(id)` | 1 | `sq_expr::SortKey` bytes as a BLOB; NULL stays NULL (sorts lowest, like unbound) |
| `sparql_regex(x, pattern[, flags])` | 2-3 | REGEX on term ids: 1, 0 or NULL |
| `sparql_eval(op, a1, …)` | 1+ | term id of `eval_template(op, n)` applied to the arguments; booleans are ids 23 / 7 |
| `sparql_sum` / `_avg` / `_min` / `_max` / `_sample` | 1 | an `sq_expr::accumulator`; an empty group returns a fresh accumulator's `finish()` |
| `sparql_group_concat(x[, sep])` | 1-2 | the same; `sep` is a constant simple-literal id |

Shared rules: term arguments are INTEGER ids or NULL. Ids may be TEMP ids of
`LoweredQuery::temps`, and helpers that compute a new term intern it into that same TempDict
with `canonical = true`, so one execution needs one TempDict (cloned from `lowered.temps`)
shared by all helper calls and by result decoding. Helpers evaluate with
`base_iri = LoweredQuery::base_iri` and one `ValueCache` per execution. None may be registered
DETERMINISTIC. `eval_template(op, n)` maps an operator back to an `Expr` with argument i in
slot i: `= < <= > >= + - * /`, `u+`, `u-`, 41 builtins by SPARQL name (`STR`, `LCASE`, `isIRI`,
`REGEX`, …) and casts by full XSD datatype IRI. `tests/common/mod.rs` holds a complete
reference implementation of every helper over `sq-expr` (about 200 lines); its `sparql_eval`:

```rust
let template = sq_sqlgen::eval_template(op, args.len()).ok_or_else(|| user_err(format!("unknown op {op}")))?;
let compiled = sq_expr::compile(&template, &st.temps).map_err(user_err)?;   // cached per (op, nargs)
compiled.eval(args, cx, true).map(|r| r.map(TermId::raw))                     // canonical = true
```

## Testing

The tests link the system libsqlite3 (the workspace's `rusqlite` has no `bundled` feature), so
they need its development library (for example `libsqlite3-dev`; 3.45.1 is the calibrated
version) and the toolchain pinned in `rust-toolchain.toml`. Run them from anywhere in the
workspace. The full suite takes about 10 seconds.

```bash
cargo test -p sq-sqlgen                        # 42 unit + 17 integration tests; none is #[ignore]d
cargo test -p sq-sqlgen -- --nocapture         # also print the W3C, benchmark, limits and fuzz reports
SQ_W3C_DIR=/path/to/rdf-tests/sparql cargo test -p sq-sqlgen --test w3c -- --nocapture
SQ_SQLGEN_FUZZ_SEEDS=50 cargo test -p sq-sqlgen --test fuzz -- --nocapture       # default: 4 seeds
SQ_SQLGEN_PRINT=1 cargo test -p sq-sqlgen --test benchmark -- --nocapture        # print each query's SQL
```

The integration tests share `tests/common/mod.rs`: the same data goes into an in-memory
`SqliteStore` and an `oxrdf::Dataset`; the SQL runs with test helpers registered; the result
is compared with spareval 0.2.7 as a bag up to blank-node renaming (order checked for ORDER BY
variables, REDUCED compared as a set). SPEC §12.1 says the baseline is checked against
sq-engine; the code checks it against spareval. The harness builds the default graph as an RDF
merge (a set) itself, because spareval keeps duplicate triples across FROM graphs.

| Test | Covers |
|---|---|
| `src/tests/golden.rs` | exact SQL for 30 representative queries; parameter numbering; the placeholder column |
| `src/tests/{sql,numeric,graph,helpers,shapes,limits}.rs` | `Params`, `compat`, joins; exactness of the numeric fast paths (randomized); GRAPH classification; every function in generated SQL is a listed helper; statement shapes under SQLite's limits; parser-stack calibration |
| `tests/equivalence.rs` | a corpus over data mixing every term kind and named graphs; `SPEC_ANSWERS` where spareval deviates; queries that must be rejected; `union_default_graph` |
| `tests/fuzz.rs` | seeded random graphs and queries (600 per seed), avoiding known spareval divergences |
| `tests/benchmark.rs` | the 17 queries of `py/queries` on small generated data: all supported and matching |
| `tests/limits.rs` | 21 families that grow one construct until rejected; every accepted size must prepare and match |
| `tests/review.rs`, `tests/review_round2.rs` | adversarial-review regressions: zero-length paths and GRAPH membership, float promotion, SQLite compile-time limits (wide BGP / IN / UNION, deep EXISTS), BIND growth, the parameter limit |
| `tests/w3c.rs` | W3C SPARQL 1.0/1.1 query-evaluation tests whose query is inside the subset |

`tests/w3c.rs` reads `$SQ_W3C_DIR` (no default; `scripts/check.sh` sets it) and passes with
a note when it is unset or the suite is missing; `tests/benchmark.rs` likewise skips without `py/queries`.
Tests that need an entailment regime other than simple entailment are reported, not asserted.

## Measured results

The crate has no timing benchmarks; its speed against the native engine is measured by the
Python harness (`py/README-bench.md`, via `sparql_count_sql`,
`sparql_rows(…, '{"engine":"sql"}')` and `sparql_query_sql`).
Measured on the project's 4-vCPU dev VM with SQLite 3.45.1:
- W3C: 613 tests. 70 require an entailment regime (28 of them still match). Of the 509
  simple-entailment tests in the subset, 508 match the expected results; the one known
  mismatch is `dawg-optional-filter-005-not-simplified`, a spargebra parse the native engine
  shares. 32 are unsupported (CONSTRUCT 9, mixed `GRAPH ?g` bodies 8, non-deterministic
  functions 7, SERVICE 6, `GRAPH ?g` using `?g` 2) and 2 are skipped (spargebra parse errors).
- All 17 benchmark queries are supported and match spareval (none uses BIND).
- The default fuzz run compares 2,392 supported random queries. The author's exploratory run
  of 150 seeds (89,761 queries) found no mismatch, and the `View::read` assertion never fired.
- Deepest nesting accepted by `tests/limits.rs` (the next size it tries is rejected): FILTER
  EXISTS 6, FILTER NOT EXISTS 4, MINUS 6, GROUP BY 6, IF/COALESCE 4, OPTIONAL with filters 10,
  sub-selects 10, `*` closures 3, BIND chain read twice per level 12. Wide shapes pass at every
  size tried: 130-pattern BGP, 700-branch UNION, 1,100-item IN and OR lists, 66-step paths.
  `CONCAT` takes 126 arguments; 127 would exceed SQLite's function-argument limit.

## Limitations

These return `Error::Unsupported("baseline: …")`:
- CONSTRUCT, DESCRIBE, and `SERVICE` without SILENT (`SERVICE SILENT` is one empty solution).
- BNODE, RAND, NOW, UUID, STRUUID: `sparql_eval` only carries builtins that are deterministic
  per argument tuple.
- `GRAPH ?g` over a body that reads the graph in only some solutions or uses `?g` itself;
  property paths other than `p` / `^p` inside a pushed `GRAPH ?g`.
- Zero-length paths whose endpoint EXISTS substitutes in only some rows; closures nested more
  than 6 deep (the parser-stack check stops them earlier, see above).
- `GROUP_CONCAT(DISTINCT …; SEPARATOR s)` with a separator other than a single space;
  separators over 7 bytes (passed as an inline short-string id).
- `COUNT(DISTINCT *)` next to a duplicate-sensitive aggregate, an aggregate over EXISTS, or
  keys/arguments over hidden variables.
- Anything past SQLite 3.45.1's limits (concept 8). The parser model is calibrated for 3.45.1,
  whose stack is fixed; on 3.46+ (growable stack) it may reject queries that would prepare.

Also: custom functions other than XSD casts, and unknown aggregates, translate to NULL
(evaluation error) rather than being rejected; REDUCED is ignored, which the spec permits.
`sq-api` does not map SQLite prepare errors: `BaselineRun::new` (`crates/sq-api/src/query.rs`)
passes them through, so on a host with lowered `sqlite3_limit`s a query this crate accepts can
fail with a plain SQLite error (`parser stack overflow`, `at most 64 tables in a join`, `too many
terms in compound SELECT`, `Expression tree is too large`, `too many arguments on function`,
`too many SQL variables`, `variable number must be between`). Mapping those messages to
`Unsupported` in `sq-api` is a recommended follow-up.

SPEC §9 is behind the code: the inline-integer fast path is the tag-split `CASE` of concept 4,
not `… OR (x & 15) <> 6 AND sparql_num(x) > k` (which ignored float/double promotion); VALUES
is a derived table, not a CTE; aggregates use helper aggregates rather than `sparql_num` /
`sparql_sortkey`; paths go well beyond `p+`/`p*`; `GRAPH ?g`, ASK and `union_default_graph` are
supported.

## Further reading

- [`docs/SPEC.md`](../../docs/SPEC.md): §9 (the baseline; see the deviations above), §2
  (schema and indexes), §3 (term ids and tags), §5.5-§5.6 (path and dataset semantics), §6
  (the value semantics the helpers implement), §8.2 (`sparql_to_sql`, `sparql_query_sql`,
  `sparql_count_sql`), §12 (testing and benchmark).
- [`docs/DECISIONS.md`](../../docs/DECISIONS.md): why the crate depends only on `sq-core`; the
  EXISTS + MINUS substitution rule.
- [`py/README-bench.md`](../../py/README-bench.md): how the baseline is timed.
- Siblings: [sq-core](../sq-core/README.md) (IR, `TermId`, codec), [sq-plan](../sq-plan/README.md)
  (`parse_query`, `lower_query`), [sq-expr](../sq-expr/README.md) (what the helpers evaluate),
  [sq-sqlite](../sq-sqlite/README.md) (the schema the SQL reads), [sq-api](../sq-api/README.md)
  (helper registration, SQL functions), [sq-results](../sq-results/README.md) (shared output).
