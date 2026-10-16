# sparqlite: SPARQL 1.1 inside SQLite, written in Rust

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

**sparqlite** is a SQLite loadable extension that stores RDF in ordinary SQLite tables and answers
SPARQL 1.1 queries and updates with its own query engine. The engine runs inside the SQLite process
and reads the data rows itself:

- through pooled, persistent prepared statements that range-scan covering indexes of integer-encoded quads, or
- by reading the index **B-tree pages directly**: it decodes SQLite's on-disk format with the VFS `xRead`
  method and stops paying for the virtual machine on every row.

It never translates a whole SPARQL query into one SQL string for SQLite's planner. A translator of
that kind is included as a baseline to measure against. The native engine beats it on 16 of the 17
benchmark queries when it only counts solutions, and on 14 of 17 when every result term is returned.

```sql
sqlite> .load ./ext/target/release/libsparqlite
sqlite> SELECT sparql_init();
1
sqlite> SELECT sparql_load('@prefix ex: <http://example.org/> .
   ...>   ex:Rust ex:influencedBy ex:OCaml, ex:Cxx .  ex:Cxx ex:influencedBy ex:C .');
3
sqlite> SELECT v0 FROM sparql_rows('PREFIX ex: <http://example.org/>
   ...>   SELECT ?x WHERE { ex:Rust ex:influencedBy+ ?x } ORDER BY ?x');
http://example.org/C
http://example.org/Cxx
http://example.org/OCaml
```

Start with the **[tutorial](tutorial.md)**. It covers installing the extension, using it from the
`sqlite3` CLI, SPARQL from first principles, and the extension's SQL features. It comes with a
[script](tutorial/make_tutorial_data.py) that builds a database of 94 real computer languages (91
programming and 3 query languages), their designers and influences.

## Results

**Correctness**

- **W3C SPARQL test suite:** 636 of the 639 applicable query and update evaluation tests pass (99.5%)
  when the whole stack is driven through plain SQL (`sparql_load` / `sparql_query` / `sparql_update`).
  - The 3 failures come from the [spargebra](https://crates.io/crates/spargebra) parser, which every
    engine built on it shares.
  - 299 tests are skipped by category: syntax-only tests, entailment regimes, the protocol tests and
    SERVICE.
  - The runner was itself validated against an independent evaluator and mutation-tested, so it
    rejects wrong answers.
- **Differential fuzzing** against `sq-oracle`, a deliberately naive, line-by-line transcription of
  SPARQL 1.1 §18:
  - random data and random queries run under every join policy and three stores (in-memory, SQLite
    through prepared statements, and SQLite through the direct B-tree reader);
  - about 7.7 million native comparisons in the main campaign, plus re-runs after every performance
    change;
  - no unexplained mismatches remain.
- **Tests:** over 1,000 Rust tests and 50 pytest tests against the built `.so`. The tutorial's verifier
  executes 163 of its code blocks in the `sqlite3` CLI and compares the printed output.

**Speed.** Benchmarked on 1.6M "social network" triples plus 1M BSBM-style product and offer triples.
The machine was a 4-vCPU cloud VM with SQLite 3.45.1, measured quietly (median of 5 after a warm-up).
Every engine's answers were cross-checked by checksum.

| Geometric mean speed-up | vs SQL-translation baseline (17 queries) | vs pyoxigraph 0.5.11 (16 queries; it timed out on Q5) |
|---|---:|---:|
| count mode (solutions counted inside the engine) | **3.5x faster** | **10.6x faster** |
| full mode (every result term turned into a Python string) | **2.8x faster** | **6.6x faster** |
| B-tree page reader vs the same engine on SQL cursors | 2.4x (count) / 1.9x (full) | |

Full mode, milliseconds:

| Query | Rows | sparqlite | SQL baseline | pyoxigraph |
|---|---:|---:|---:|---:|
| Q1 star-4 (class, age, city, name) | 20,000 | **82.3** | 103 | 178 |
| Q2 BSBM product star-6 | 1,606 | **9.94** | 11.8 | 24.0 |
| Q3 chain-2 `?a :knows ?b . ?b :knows ?c`, COUNT | 1 | **50.0** | 461 | 1,855 |
| Q4 selective chain-3 from a constant | 27 | 0.175 | **0.132** | 0.163 |
| Q5 OPTIONAL with a type check inside | 20,634 | **47.7** | 111 | timeout (>120 s) |
| Q6 `FILTER(?age > 90)` | 1,393 | **2.10** | 25.1 | 107 |
| Q7 `REGEX` over 200K names | 11,111 | **102** | 168 | 136 |
| Q8 GROUP BY + COUNT + ORDER BY + LIMIT | 10 | **17.0** | 23.1 | 106 |
| Q9 `ORDER BY DESC(?age) ?x LIMIT 10` | 10 | **9.59** | 164 | 1,508 |
| Q10 DISTINCT over a join | 9,999 | **160** | 910 | 2,337 |
| Q11 UNION | 127,794 | **269** | 305 | 497 |
| Q12 MINUS | 14,877 | **45.6** | 79.8 | 168 |
| Q13 `:p1 :knows+ ?y`, COUNT | 1 | **168** | 441 | 298 |
| Q14 `rdf:type/rdfs:subClassOf*` | 7,800 | 13.0 | **8.35** | 130 |
| Q15 point lookup `:p123 ?p ?o` | 8 | 0.123 | 0.106 | **0.082** |
| Q16 `COUNT(*) { ?s ?p ?o }` | 1 | **1.24** | 61.1 | 366 |
| Q17 BSBM offers: price range + date + ORDER BY price LIMIT 10 | 10 | **24.3** | 79.7 | 70.3 |

The full reports, the count-mode table and the `reader=sql` comparison are in
[bench-results/](bench-results/). The harness, data generators and queries are in
[py/](py/README-bench.md). Loading the 1.6M social triples takes 9.1 s (pyoxigraph: 8.8 s), plus
1.4 s for `ANALYZE` and `sparql_analyze()`.

Where it loses:

- Sub-millisecond queries (Q4, Q15) pay a fixed overhead of a few tens of microseconds for planning
  and for validating the read snapshot.
- Q14 in full mode decodes 7,800 result IRIs more slowly than the baseline's output path. In count
  mode Q14 takes 0.73 ms against 1.50 ms.

## How it works

```
 SQL: sparql_query() / sparql_rows() / sparql_update() / CREATE VIRTUAL TABLE ... USING sparql_view(...)
        |
 sq-api      functions, virtual tables, per-connection state, transactions, close hook, panic guards
        |
 sq-engine   facade: parse (spargebra) -> lower -> optimize -> execute; SPARQL Update; loader
   |-- sq-plan    lowering to an IR, static analysis, rewrites, cardinality, join-order DP, physical plan
   |-- sq-exec    batched pull executor: bind / hash / merge / star joins, paths, EXISTS, aggregates, top-k
   |-- sq-expr    SPARQL values, every builtin, casts, =/< matrices, ORDER BY keys, aggregates
        |
 sq-sqlite   storage: schema, dictionary + caches, pooled index-range cursors, DIRECT B-TREE READER, loader
        |
 SQLite file:  rdf_term (dictionary)  +  rdf_quad(g,s,p,o) WITHOUT ROWID with 4 covering orders + partial indexes
```

The ideas that make it fast:

1. **Everything is an integer.**
   - Every term is a 64-bit id with a 4-bit tag in the low bits. Small integers, booleans, decimals,
     dates, date-times and short strings are stored inline in the id, so filters and sorts on them
     never touch the dictionary.
   - Joins, DISTINCT and GROUP BY compare raw integers. Strings are decoded only for the output
     columns SQLite actually reads; `sparql_rows` honours `colUsed`.
2. **Covering B-trees in every useful order.** `rdf_quad` is a WITHOUT ROWID table keyed `(g,s,p,o)`.
   It has secondary orders `(g,p,s,o)`, `(g,p,o,s)` and `(g,o,s,p)`, plus partial graph-last indexes
   for named graphs. Every triple pattern is one range scan, and every `?s :p ?o` is available sorted
   by `s` or by `o`, which is what makes merge joins possible.
3. **Reading the rows ourselves.**
   - Cursors lease persistent prepared range scans that select only the free columns, inside a held
     read transaction.
   - On rollback-journal file databases, the **direct B-tree reader** walks the index pages itself:
     27 ns per row against 116 ns for stepping a statement.
   - It counts ranges by **summing page cell counts** without decoding any rows. `COUNT(*)` over 1.6M
     quads takes 1.2 ms.
   - Dictionary misses are served from `rdf_term`'s table B-tree in the same way.
   - The reader checks its preconditions on every call and falls back to SQL cursors at the exact
     resume key: a WAL database, an open write transaction on the connection, a changed data version,
     shared-cache mode, or any page it does not understand.
4. **A planner that knows the data.**
   - Bounded counts at plan time, per-predicate statistics, heavy hitters and characteristic sets.
   - DP join ordering with *interesting orders*: merge joins over sorted index streams, with
     factorized `COUNT`.
   - Exact integer-range pushdown into the `(g,p,o)` order.
   - Top-k with lazily built sort keys.
   - A hybrid that hands selective index-nested-loop chains to SQLite's own engine in *our* join order
     (`CROSS JOIN` + `INDEXED BY`).
5. **Correctness first.** Every rewrite is gated by a static analysis of which variables are certainly
   bound, maybe bound, or safe to inject sideways. The spec-transcribed oracle checks the optimized
   engine continuously.

Each crate has a README explaining its design, API, tests and numbers:

| Crate | Role |
|---|---|
| [sq-core](crates/sq-core/README.md) | frozen shared contract: ids and codec, terms, batches, storage traits, IR and plan types |
| [sq-sqlite](crates/sq-sqlite/README.md) | SQLite storage, cursors, caches, bulk loader, statistics, **direct B-tree reader** |
| [sq-memstore](crates/sq-memstore/README.md) | in-memory store used as a test double and by the oracle |
| [sq-expr](crates/sq-expr/README.md) | SPARQL expression semantics |
| [sq-plan](crates/sq-plan/README.md) | lowering, analysis, optimizer, explain |
| [sq-exec](crates/sq-exec/README.md) | physical operators and execution |
| [sq-oracle](crates/sq-oracle/README.md) | naive reference evaluator for differential testing |
| [sq-results](crates/sq-results/README.md) | SPARQL JSON/XML/CSV/TSV and RDF output, SQL value mapping |
| [sq-sqlgen](crates/sq-sqlgen/README.md) | the SPARQL-to-SQL baseline translator |
| [sq-engine](crates/sq-engine/README.md) | facade, SPARQL Update, loader, the differential fuzzer |
| [sq-api](crates/sq-api/README.md) | the SQL surface of the extension |

Supporting tools:

- [tools/sq-w3c](tools/sq-w3c/) runs the W3C test suite.
- [py/](py/) holds the benchmark and the pytest suite.
- [repro/deserialize-eponymous](repro/deserialize-eponymous/README.md) reproduces a SQLite bug
  that the final audit ran into (see Limitations).
- [ext/](ext/) is the cdylib. It is a separate cargo workspace so that rusqlite's
  `loadable_extension` feature never mixes with the native test builds.

## Build, test, benchmark

```bash
cargo build --release --manifest-path ext/Cargo.toml   # -> ext/target/release/libsparqlite.so
scripts/check.sh                                       # every gate: Rust tests, build checks, pytest, W3C
python3 tutorial/verify_tutorial.py --ext ext/target/release/libsparqlite.so
```

`scripts/check.sh` expects:

- the `sparql/` directory of a [w3c/rdf-tests](https://github.com/w3c/rdf-tests) checkout, via the
  `W3C` variable (for example `W3C=~/rdf-tests/sparql`). It also passes that directory to the
  crate-level W3C tests as `SQ_W3C_DIR`;
- a Python virtual environment, via the `VENV` variable (it installs pytest there if needed). The
  benchmark in `py/` also needs pyoxigraph and rdflib in it.

Both default to directories under `SCRATCH` (default `${TMPDIR:-/tmp}/sparqlite-scratch`), where the
build targets go too.

It requires SQLite 3.34.1 or later and was tested with 3.45.1. Apple's built-in `sqlite3` cannot load
extensions; the tutorial covers Homebrew and the Windows file name.

## Limitations

- SPARQL 1.1 only: no SPARQL 1.2 / RDF-star, no `SERVICE`. `LOAD` reads only `file:` IRIs, and only
  when `sparql_config('allow_files', 1)` is set.
- Parser quirks inherited from spargebra:
  - `SELECT *` columns come out in alphabetical order;
  - `OPTIONAL { { P FILTER(F) } }` is flattened;
  - `TRUE`/`False` are rejected.
- `xsd:integer` values beyond 64 bits and decimals beyond 18 fractional digits are treated as
  ill-typed.
- The direct B-tree reader only serves rollback-journal file databases on a private-cache connection.
  WAL and in-memory databases use the (still fast) SQL cursors.
- Small fixed per-query overhead of tens of microseconds, from planning plus snapshot validation.
- A SQLite bug affects connections that have been deserialized into (`sqlite3_deserialize`, Python
  `Connection.deserialize`). After the call, preparing a statement can crash the process if it uses
  an eponymous virtual table that was already used on that connection. That covers `json_each`,
  `pragma_*`, `sparql_rows` if you used it before, and `sparql_status` always, because the
  extension connects it when it loads.
  - **Workaround:** deserialize first, then load the extension.
  - Reproduction and analysis are in
    [repro/deserialize-eponymous](repro/deserialize-eponymous/README.md).
  - sparqlite's own catalog reads use only `sqlite_schema` and `PRAGMA` statements, neither of
    which is a virtual table, so its scalar functions are unaffected.
- Only Linux was tested. Windows instructions in the tutorial are untested.

## How this was built

This was an experiment in building a substantial system with many coordinated AI agents. The full log
is in [notes.md](notes.md).

1. **Design panel.** Three architects designed it independently, with different priorities: raw speed,
   SPARQL semantics, and SQLite engineering. Three judges scored the designs, and a synthesizer merged
   them into [docs/SPEC.md](docs/SPEC.md), with the decisions recorded in
   [docs/DECISIONS.md](docs/DECISIONS.md).
2. **Shared contract.** The frozen `sq-core` crate came from the synthesizer's verified prototype, and
   I added `todo!()` stubs for every other crate.
   Each crate was then built by its own agent pipeline: implement, adversarial review with failing
   tests as evidence, fix, repeated.
3. **Integration.** The ground-truth oracle and a differential fuzzer tied the crates together. The W3C
   suite measured compliance throughout.
4. **Performance.** Benchmark-driven passes added the B-tree range counts, direct dictionary reads,
   lazy sort keys and planner cost fixes, each re-measured and correctness-gated by an independent
   reviewer.
5. **Final audit.** Four narrow auditors covered the FFI layer, the B-tree reader on hostile or
   corrupt files, the virtual tables and their guards, and the SQL-surface security model. They
   found:
   - two high-severity bugs: a datatype cycle in a corrupt dictionary overflowed the stack, and
     the caches survived `sqlite3_deserialize` swapping in another database;
   - one medium: the nesting bound did not cover row fetches;
   - a handful of lows.

   All were fixed with regression tests. The audit also surfaced the SQLite deserialize crash
   described under Limitations.

Reviewers caught many real bugs, for example:

- a write scope that could leave a half-applied update in an open transaction after an interrupt;
- the B-tree reader seeing another connection's uncommitted pages under shared cache;
- sequence property paths treating their middle node as a constant;
- a merge join on a 20-value key taking 55 s where a hash join takes 0.4 s;
- a REGEX shortcut that ignored inline `(?i)` flags.
