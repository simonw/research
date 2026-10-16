//! sq-w3c: the W3C SPARQL 1.0 / 1.1 test-suite runner for sparqlite (docs/SPEC.md §12.2).
//!
//! * [`manifest`] walks the manifests (`mf:include`, `mf:entries`) into [`manifest::TestCase`]s;
//! * [`backend`] puts each engine (reference spareval, MemStore + sq-engine, SQLite + sq-api,
//!   sq-oracle) behind one trait;
//! * [`runner`] loads the test data, runs the query or update in an isolated worker thread
//!   and compares the outcome ([`compare`]) with the expected results ([`results`]);
//! * [`report`] aggregates outcomes into the summary table and the JSON report.
pub mod backend;
pub mod cli;
pub mod compare;
pub mod manifest;
pub mod report;
pub mod results;
pub mod runner;
pub mod suite;
mod vocab;
