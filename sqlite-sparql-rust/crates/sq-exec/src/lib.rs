//! sq-exec: the native executor (spec §4.1–§4.3, §5.1–§5.7).
//!
//! A pull-based Volcano executor over 1024-row [`sq_core::Batch`]es in one query-wide slot
//! layout (rows are `[TermId; width]`, 0 = unbound). It runs a [`sq_core::plan::QueryPlan`]
//! directly against a [`sq_core::store::QuadStore`]: physical operators (§5.3) in [`ops`],
//! property paths (§5.5), EXISTS by memoized substitution, GraphLoop's graph register,
//! CONSTRUCT / DESCRIBE (§5.7), the `TermResolver` (§4.3), deadlines, the memory budget and
//! interrupt polling. Depends only on sq-core and sq-expr.
//!
//! [`Execution`] is the entry point: a `SolutionSource` for SELECT / ASK and a
//! `TripleSource` for CONSTRUCT / DESCRIBE.
#![forbid(unsafe_code)]

mod build;
mod construct;
mod ctx;
mod execution;
mod exists;
mod ops;
mod resolver;
mod rows;
mod seed;

#[cfg(test)]
mod tests;

pub use execution::Execution;

/// Hook for connection-level caches. The store's `Dictionary` already uses the connection
/// DecodeCache/LookupCache, so the default (empty) value is always valid.
#[derive(Clone, Debug, Default)]
pub struct ExecCaches {}
