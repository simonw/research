//! Crate-level tests: table tests per function, the `=` matrix, ORDER BY key properties,
//! aggregates, a W3C-driven check of expression results, and two ignored tools (hot-path
//! micro-benchmarks and the pyoxigraph value diff).
mod harness;

mod aggregates;
mod bench;
mod decoding;
mod equality;
mod functions;
mod interning;
mod sorting;
mod value_diff;
mod w3c;
mod review;
mod review2;
