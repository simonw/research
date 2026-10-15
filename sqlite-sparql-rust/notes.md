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
