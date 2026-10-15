# Notes: sqlite-sparql-rust

Goal: a SQLite loadable extension written in Rust that implements SPARQL, and is
fast because the query engine runs *inside* SQLite and reads the index B-tree rows
directly (prepared index-range scans over integer-encoded quads) instead of
translating SPARQL to SQL text or running as an external client.

## Environment (2026-10-15)
- rustc/cargo 1.97.0, 4 CPUs, 15GB RAM
- No sqlite3 CLI; Python 3.13 sqlite3 module = SQLite 3.45.1, enable_load_extension works
- crates: rusqlite 0.40.2 (has `loadable_extension` feature), spargebra 0.4.7,
  oxttl 0.2.4, oxrdfio 0.2.6, oxrdf 0.3.4, sparesults 0.3.4, spareval 0.2.7
- pyoxigraph 0.5.11 and rdflib 7.6.0 installable from PyPI (for reference/benchmarks)

## Log
- Smoke test OK: cdylib built with rusqlite{loadable_extension,functions,vtab} + spargebra,
  entry point `sqlite3_<name>_init` via `Connection::extension_init2`, loaded from Python sqlite3.
- Interpretation of the request: "hit the data rows directly" = the SPARQL engine lives in the
  SQLite process as an extension and drives index range scans on integer-encoded quad tables
  itself (prepared statements, sqlite3_step/column_int64), rather than (a) generating one big SQL
  query and hoping the SQL planner copes, or (b) an external client issuing SQL per pattern.
- Plan: design panel workflow (3 architects w/ different priorities -> judges -> synthesis), then
  implement as a cargo workspace so agents can build modules in parallel.
- W3C test suite: HTTPS tarball from codeload is blocked by the proxy, but `git clone --sparse` of
  github.com/w3c/rdf-tests works. sparql/sparql11 has 331 QueryEvaluationTests + 94 UpdateEvaluationTests;
  sparql/sparql10 has 289 QueryEvaluationTests. (Kept outside the repo in the scratchpad.)

### Design panel (workflow: 3 architects -> 3 judges -> synthesis)
- Architects: speed / correctness / engineering angles. Judge totals: correctness 21, engineering 19, speed 18.5.
- Synthesized spec is committed at docs/SPEC.md (3,850 lines) with the decision log in docs/DECISIONS.md.
- Key decisions: one `rdf_quad(g,s,p,o)` WITHOUT ROWID table with 4 graph-first covering orders
  (GSPO pk + GPSO/GPOS/GOSP) + 4 partial g-last orders for named graphs + a tiny partial
  `rdf_gpox` index for exact integer range pushdown; 64-bit ids with a 4-bit LOW tag and inline
  int/bool/decimal/dateTime/date/short-string literals; pull-based batched executor over
  `QuadCursor`s backed by pooled persistent prepared statements; planner with bind/hash/merge
  joins and a "SqlJoin" hybrid that hands selective index-nested-loop chains to SQLite's VDBE in
  OUR join order (CROSS JOIN + INDEXED BY).
- Measured by the panel on this machine: pinned scan 105-140 ns/row; point probe ~1 us inside a
  held read txn vs 7-8 us without (=> always hold a read transaction); SQLite's own NLJ ~0.55
  us/probe; chain-2 merge join 144-172 ms vs SQLite's join 360-650 ms.

### WP0 (lead)
- Cargo workspace: crates/{sq-core, sq-memstore, sq-sqlite, sq-expr, sq-plan, sq-exec, sq-oracle,
  sq-results, sq-sqlgen, sq-engine, sq-api}, tools/{sq-w3c, sq-bench}, ext/ (separate workspace so
  rusqlite's `loadable_extension` never unifies with the native test builds that link the system
  libsqlite3 3.45.1 — same version Python loads into).
- sq-core copied from the synthesizer's verified prototype; all other crates are `todo!()` stubs with
  the spec §11 signatures; sq-memstore implemented by me (passes the cursor conformance suite).
- ext builds (400 KB), `cargo tree` gate shows no `bundled`, Python loads it: `sparql_version()` ->
  `sparqlite 0.1.0 (SQLite 3.45.1)`.
- Added spareval as an OPTIONAL dependency of tools/sq-w3c only (`--backend reference`) so the W3C
  harness itself can be validated before our engine exists. It is never linked into the extension.

### Wave 1 results
- sq-results + tools/sq-w3c (pipeline: implement -> adversarial review, no blocking findings): the W3C
  runner finds 938 tests (sparql11: 331 query + 94 update + 3 CSV + others; sparql10: 288 — the 289th is
  deliberately not in mf:entries). 639 runnable, 299 skipped (syntax tests, entailment, service...).
  Validated against spareval as a *reference backend*: 612/639 pass (597 exact + 15 numeric-lenient);
  the 27 failures were triaged as genuine spareval/spargebra divergences (GRAPH scoping, zero-length
  paths, xsd:date, BNODE(str) reuse...) and listed in tools/sq-w3c/reference_divergences.txt.
  The reviewer mutation-tested the comparator (14 kinds of output corruption) — all detected.
- spargebra quirks that will affect us: `SELECT *` columns come out sorted alphabetically;
  `OPTIONAL { { P FILTER(F) } }` is simplified into a LeftJoin condition (dawg-optional-filter-005-not-
  simplified cannot pass); `TRUE`/`False` booleans and VALUES after CONSTRUCT..GROUP BY are rejected.
- sq-expr (implement -> 2 review/fix rounds; reviewers found e.g. a REGEX literal-prefix fast path
  that ignored top-level alternation, TEMP constants re-interned per row, BNODE(str) scoping): 97 tests,
  W3C-driven expression tests over 16 directories all pass (8 only numeric-lenient: "3" vs "3.0").
  20,000 random arithmetic/cast/compare expressions vs pyoxigraph: 97.8% raw agreement, 100% after
  excluding documented pyoxigraph divergences (NaN comparisons error, ROUND half-away-from-zero,
  oxsdatatypes decimal x*0 / 0/2.5 failing, STR canonicalising numerics...). Release micro-benchmarks:
  `?x > 500` on inline ints 10 ns/row, IdEq 8.7 ns, REGEX with literal prefix 87 ns, SUM(int) 12 ns.
- Benchmark harness (py/bench.py + generators + Q1..Q17; 2 review rounds focused on fairness: e.g. a
  timer thread inside the SQLite timed region, count mode not like-for-like (pyoxigraph built terms) —
  fixed by timing engine-side COUNT(*) for the reference engines, every execution's result verified).
  Data: social 200K persons = 1,600,000 triples (3.8 s to generate); bsbm-lite = 1,004,103 triples.
  pyoxigraph 0.5.11 reference timings (count mode, median of 5, ms): Q1 101, Q2 16, Q3 2351,
  Q4 0.2, Q5 >300 s timeout (super-linear OPTIONAL), Q6 148, Q7 121, Q8 173, Q9 1951, Q10 3085,
  Q11 195, Q12 201, Q13 421, Q14 150, Q15 0.1, Q16 486, Q17 78. Load: 8.0 s social, 4.3 s bsbm.
  rdflib on 80K triples: 3.9 ms .. 26 s; pyoxigraph 12-1900x faster than rdflib.
- sq-sqlite (implement -> 2 review/fix rounds; reviewers found real transaction bugs: a write scope
  dropped after an interrupt left a half-applied update in an open transaction that a later COMMIT
  would persist; stale cache admission after another connection's vacuum; savepoints failing inside a
  host statement that writes another schema). Final: 88 tests incl. cursor conformance on memory+file
  DBs, EXPLAIN QUERY PLAN asserted for 570+ statement shapes. Release numbers on ~1M social triples:
  bulk load 6.3-7.1 s (incl. 3 full + 5 partial index builds), point probe 0.9 us sorted / 1.3-2.1 us
  random, 2-column pinned scan 117-131 ns/row, count(*) in VDBE 40-50 ns/row, decode miss ~1.0 us,
  hit ~100 ns, bare API entry (read txn + epoch check) ~11 us.
- Found during WP2: `NOT INDEXED` does NOT pin the primary key of a WITHOUT ROWID table on SQLite
  3.45 — the planner still chose rdf_gpso/rdf_gosp. Fixed in sq-core: GSPO now renders as
  `INDEXED BY sqlite_autoindex_rdf_quad_1`.
- sq-plan (implement -> 2 review/fix rounds): lowering of every spargebra construct, §7.2 props,
  rewrites (GRAPH push/loop, filter placement, sameTerm seeding, Count, TopK, EXISTS decorrelation
  under §7.3.4), bounded-count + stats cardinality, left-deep DP (<= 12 units, bounded at 6000 steps,
  greedy beyond), BindJoin/HashJoin/NestedLoop choice, validate(), explain. All 921 W3C .rq files
  (101 rejected by spargebra) planned & validated: 5,532 plans under 4 option sets. Reviewers found
  plans that validate() rejected (seeded OPTIONAL groups), a non-equivalent EXISTS decorrelation when
  the EXISTS contains a MINUS, a GRAPH ?g push that returned extra rows (`GRAPH ?g {?s :p ?o OPTIONAL
  {?o :q ?g}}`), DP time cliffs. The planner author wrote down a precise "seed propagation contract"
  that the executor must follow; I appended it to the executor's work package mid-flight so its
  reviewers enforce it.
- Tightened vs spec: the GRAPH ?g push rule (spec's `?g ∈ certain(P')` test accepts 3 non-equivalent
  cases: VALUES ?g inside the graph, UNIONs mixing graph-reading and non-reading branches, OPTIONAL
  whose left side does not read the graph).
- sq-oracle (naive §18 transcription; implement -> 2 review/fix rounds): W3C oracle backend
  542/545 (99.45%), the 3 failures are spargebra parser divergences. A reviewer caught that sequence
  paths treated the middle node as a constant (`:zz (:p?/:q?) ?y` matched through zero-length steps).
  The oracle author also showed two places where our own spec was wrong w.r.t. SPARQL 1.1 §18:
  (1) an erroring GROUP BY key and an unbound key form ONE group (ListEval((unbound)) = (error));
  (2) the middle of a path sequence is a join variable. pyoxigraph/spareval are not authoritative on
  paths either: they drop zero-length matches for constants absent from the graph and use set
  semantics for `|` and `/` where §18.4 keeps bags.
- **Direct B-tree page reader (WP15)** in crates/sq-sqlite/src/btree/: obtains the sqlite3_file* via
  SQLITE_FCNTL_FILE_POINTER, reads pages through the VFS xRead, parses index b-tree pages (0x02/0x0A,
  in-order interior cells, varints, serial types), own CLOCK page cache keyed by
  SQLITE_FCNTL_DATA_VERSION. Preconditions re-checked on every fill/seek/rebind (file DB, rollback
  journal not WAL, read txn held, no write txn on this connection, data version unchanged, not
  shared-cache); on violation it falls back to the SQL cursor *at the exact resume key*.
  Release, 1M triples, 8K pages: 2-column scan SQL 116 ns/row -> B-tree 27 ns/row warm / 34 cold
  (4.2x); 3-column 137 -> 30 ns/row (4.5x); sorted probes GSPO 1.16 -> 0.38 us, GPSO 1.02 -> 0.29 us.
  Random probes are memory-bound and at parity. Reviewer found a real issue: under shared-cache
  connections the reader could see another connection's uncommitted spilled pages; shared cache is now
  detected (EXPLAIN looks for OP_TableLock — no allow-listed API reports the mode) and refused.
  Conformance suite passes at page sizes 4K/8K/16K/64K, with reserved bytes, after VACUUM, and on 600
  rounds of randomly corrupted pages (fall back, never panic).
- sq-exec (implement -> 2 review/fix rounds; the round-1 reviewer enforced the planner's seed contract
  and the oracle's §18 path semantics that I appended mid-flight — it caught NPS returning bags and the
  sequence-middle-as-constant bug; round 2 bounded the EXISTS memo and capped path edge counts):
  109 tests incl. a 60-seed property test (BindJoin == HashJoin == NestedLoop == MergeJoin+enforcers),
  28 seeded-subtree equivalence shapes. A scratch W3C sweep through sq-plan + sq-exec on MemStore:
  528 pass + 8 lenient, same under Auto / PreferBind / PreferHash / no-reorder.
- Disk: agents' cargo target dirs reached ~20 GB (the allowance filled up once); cleaned and added a
  one-target-dir + CARGO_INCREMENTAL=0 rule to the pipeline preamble.
- WP8 optimizer (implement -> 2 review/fix rounds): interesting-order DP (best prefix per output order),
  MergeJoin with galloping + SortBySlot enforcers, StarMerge post-pass (cost-gated), SqlJoin hybrid
  (`main.rdf_quad AS tN INDEXED BY idx` + CROSS JOIN, constants as ?N params — every one of ~600
  generated SqlJoins checked with EXPLAIN QUERY PLAN to run in exactly our alias order), exact
  IntRange pushdown (exhaustive test: 4 ops x 17 constants x 4 shapes x 2 stores; off-by-one mutations
  caught), factorized COUNT over merge joins (chain-2 COUNT = Count{MergeJoin(Gpos sorted ?b, Gpso
  sorted ?b)}). Differential test in sq-plan: 1,000 random queries x 8 option sets x {MemStore,
  SQLite} = 16,000 runs vs the oracle, 0 differences; 1,646 forced StarMerge rewrites all match.
  Reviewers caught: SqlJoin chains not capped at SQLite's 64-table join limit; DP budget no longer
  bounding work; union-default-graph cardinality explosions; StarMerge never chosen (cost per row
  instead of per distinct key).
- sq-sqlgen baseline (implement -> 2 review/fix rounds): one SQL statement per query over the same
  storage; inline-int fast paths, LEFT JOIN with nullable compatibility, NOT EXISTS for MINUS,
  correlated EXISTS with substitution semantics, WITH RECURSIVE for p+/p*, window functions for
  DISTINCT+ORDER BY, helper UDFs (sparql_num/str/ebv/sortkey/regex/eval + aggregate helpers) backed by
  sq-expr. W3C: 579 of 613 query-evaluation tests inside its subset, 578 match (the 1 is the shared
  spargebra parser quirk). All 17 benchmark queries supported. A seeded fuzzer vs spareval (150K
  queries exploratory) found real bugs (GROUP BY on never-bound keys, OPTIONAL with constant right side);
  reviewers found zero-length-path / GRAPH membership bugs, float promotion in truth-only comparisons,
  exponential SQL growth when a BIND variable is referenced repeatedly.

### Documentation requirements (added by the user mid-project)
- Every folder in crates/ gets a README.md; tutorial.md teaches install + sqlite3 CLI usage + SPARQL + the
  extension's features; tutorial/make_tutorial_data.py populates a DB with realistic example data.
- Installed the sqlite3 CLI (apt: 3.45.1, same as Python's) so every tutorial command can be verified;
  its built-in readfile() lets the CLI feed Turtle files to sparql_load().
- Plan: README workflow for the 9 finished crates (write -> fact-check against source -> fix); dataset
  workflow (curated real programming-language knowledge graph with influencedBy edges for property
  paths, a paradigm class hierarchy for rdfs:subClassOf*, language-tagged labels, partial homepages for
  OPTIONAL; clearly-fictional survey named graphs + a relational job_postings table for SQL joins;
  optional synthetic social network for the performance section) -> independent fact-check + usability
  review -> fix. Tutorial text and the sq-engine/sq-api READMEs follow once those crates are final, with
  every tutorial command executed in sqlite3 against the final build.
- sq-engine (implement -> 2 review/fix rounds): facade, SPARQL Update executor (WHERE materialized
  before writes, fresh bnodes per solution, SILENT rules, LOAD file:// gated), generic loader, and the
  DIFFERENTIAL FUZZER: random data + random queries (OPTIONAL±filter, MINUS, EXISTS, GRAPH, FROM,
  paths, aggregates, sub-selects, VALUES, ORDER BY...) run on the native engine under 5 join policies x
  reorder on/off x 3 stores (MemStore, in-memory SQLite via SQL cursors, file SQLite with 512-byte pages
  so the direct B-tree reader is exercised) vs the oracle. 26 seeds x 10,000 queries = ~7.7 million
  native comparisons + ~12,000 update requests: zero unexplained mismatches (after fixing what it found:
  an sq-exec bug, and BNODE() on a BindJoin probe side being re-evaluated per seed). Reviewer found
  `union_default_graph` leaking into ADD/COPY/MOVE. W3C mem backend: 636/639 (3 = spargebra parser),
  every UpdateEvaluationTest passes.
- sq-api (implement -> 2 review/fix rounds): every §8.2 function (per-arity registration, flags: nothing
  DETERMINISTIC, writers/file access DIRECTONLY), sparql_rows (col_used-driven decoding, streaming 1024-row
  batches; count(*) decodes nothing), sparql_view (typed columns + __term/__id pushdown as sameTerm),
  sparql_status + close hook (WAL file disappears after close: no zombie connection), guard() around every
  vtab method, baseline helper UDFs over sq-expr. Reviewers found: a temp.sparql_view scan holding a SAVEPOINT
  open between rows (user writes silently swallowed), deeply nested/wide query text overflowing the host
  stack (catch_unwind cannot catch that) -> a nesting/width cost limiter now refuses such requests, a
  baseline-engine timeout watchdog thread (sqlite3_interrupt, no progress handler), COLLATE-aware view
  pushdown. Final: 105 sq-api Rust tests + pytest suite against the real .so; W3C through plain SQL
  (`--backend sqlite`): 636/639 (3 = spargebra parser).
- Tutorial dataset (tutorial/make_tutorial_data.py, stdlib only, deterministic via its own SplitMix64):
  languages.ttl = 2,108 triples: 94 languages, 117 people, 40 organisations, 275 influencedBy edges, a
  13-class paradigm hierarchy, labels/descriptions in en/fr/de/es/ja; survey.trig = FICTIONAL survey
  named graphs; job_postings (FICTIONAL, 300 rows) for SQL joins; optional --people N synthetic network
  (100K people = 836K triples). Fact-checker found backward influence edges (Java<-C#, Rust<-Swift,
  Rust<-Elm), non-source file extensions, a missing death date; the fixer then checked every language and
  person against live Wikipedia infoboxes (fixing e.g. COBOL 1960, SQL 1973, Rust 2012 per infobox) and
  added validation that an influencer is never younger than what it influenced and that influencedBy is
  acyclic. I spot-checked the three 2026 death dates it added against Wikipedia myself.

### Integration verification (lead)
- `scripts/check.sh` (release, all crates, --no-fail-fast): 921 Rust tests passed, 4 failed — all four were
  LOW-severity reviewer findings pinned as failing tests that never got a fix round (W3C comparator ORDER BY
  tie handling x2, CSV column realignment, B-tree stale page cache after a rejected snapshot); a small
  fix workflow is handling them. Also fixed myself: XML results containing characters XML 1.0 cannot
  represent (e.g. U+0001) are now refused with a clear error instead of producing a broken document.
- Release cdylib: 6.6 MB, cargo-tree gate (no bundled SQLite) and API allow-list grep gate pass.
- pytest against the real .so in Python 3.13 / SQLite 3.45.1: 39 passed.
- W3C through plain SQL (`sq-w3c --backend sqlite`, the full stack via sparql_load/sparql_query/
  sparql_update): 636 of 639 runnable tests (99.53%; 627 exact + 9 numeric-lenient) in 2.2 s; the 3 failures
  are spargebra parser divergences; 299 skipped by design (syntax-only, entailment, protocol, SERVICE).

### Benchmark run A (1.6M social + 1M bsbm; native vs SQL baseline vs pyoxigraph; load avg 6-10 during run)
- All 17 queries: row counts + checksums agree across all engines and modes.
- full mode (every result term materialised as a Python string, like-for-like): geomean native vs
  pyoxigraph 3.75x, native vs baseline 1.76x. Big wins: Q3 chain-2 count (factorized merge join) 74.6 ms vs
  baseline 533 vs pyoxigraph 2294; Q6 FILTER(?a > 90) with IntRange pushdown 2.3 ms vs 33 / 150; Q10 DISTINCT
  over a join 207 vs 1353 / 2895; Q5 OPTIONAL 61 ms vs 116, pyoxigraph timeout (>300 s).
  Losses: Q4/Q15 tiny queries (fixed per-query overhead ~0.2 ms), Q7 REGEX (300 vs pyoxigraph 180),
  Q16 COUNT(*) full (84 vs baseline 66).
- count mode: Q3/Q13/Q16 are ARTIFACTS — `sparql_count()` of an aggregate query returns 1 without
  evaluating it (verified: 0.006 s vs 0.13 s via sparql_value). Must be removed/handled before reporting.
  Measured directly: Q16 count over 1.6M quads: B-tree reader 129 ms vs reader=sql (VDBE count) 68 ms ->
  the reader should count by summing leaf cell counts instead of decoding rows.
- tutorial.md (write -> new-user walkthrough + SPARQL-expert review -> fix): ~5,000 lines: install (incl.
  macOS/Windows notes, minimum SQLite 3.34.1 = the loadable-extension bindings' version), first steps in the
  sqlite3 CLI, RDF in 5 minutes, the dataset, 16 SPARQL lessons each with a verified exercise, the SQL interface,
  performance on 100K synthetic people, reference + troubleshooting. tutorial/verify_tutorial.py runs every
  sql/output block pair through the sqlite3 CLI (157 verified, 3 skipped). The walkthrough reviewer
  reproduced every output and all 15 exercise solutions; the expert review fixed several subtle explanations.
  It also surfaced engine issues now queued: a merge join on a ~20-value key with a residual check taking 55 s
  (0.43 s as a hash join), read-only functions not SQLITE_INNOCUOUS (stored views fail with trusted_schema off).
