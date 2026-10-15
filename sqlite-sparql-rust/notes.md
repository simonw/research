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
