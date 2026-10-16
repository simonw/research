//! sq-results: result serialization for sparqlite (docs/SPEC.md §8.4, §11, §12.1).
//!
//! * SELECT / ASK: SPARQL 1.1 JSON, XML, CSV and TSV via sparesults ([`write_solutions`],
//!   [`write_boolean`]);
//! * CONSTRUCT / DESCRIBE: N-Triples, N-Quads (default graph) and Turtle via oxrdfio
//!   ([`write_triples`]);
//! * the native SQL value mapping of §8.4 ([`sql_native`]);
//! * the per-row SPARQL-JSON bindings object ([`solution_json`]).
//!
//! Writers consume only the [`SolutionSource`](sq_core::eval_api::SolutionSource) /
//! [`TripleSource`](sq_core::eval_api::TripleSource) traits of sq-core, so the native executor
//! and the baseline SQL translator share one output path. They pull rows in batches of
//! [`BATCH_ROWS`](sq_core::BATCH_ROWS), decode each batch with one call and serialize from
//! borrowed terms: nothing is allocated per row.

mod convert;
mod format;
mod json;
mod native;
mod solutions;
mod triples;

#[cfg(test)]
mod tests;

pub use convert::check_ntriples_term;
pub use format::ResultFormat;
pub use json::{append_solution_json, append_term_json, solution_json};
pub use native::{sql_native, SqlNative};
pub use solutions::{write_boolean, write_solutions};
pub use triples::write_triples;

/// Size of the output buffer placed in front of the caller's writer (the sparesults and
/// oxrdfio serializers issue many small unbuffered writes).
const OUTPUT_BUFFER_BYTES: usize = 64 * 1024;

fn io_error(e: std::io::Error) -> sq_core::Error {
    sq_core::Error::Internal(format!("I/O error while writing results: {e}"))
}
