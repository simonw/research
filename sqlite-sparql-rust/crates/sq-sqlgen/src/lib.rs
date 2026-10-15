//! sq-sqlgen: the baseline SPARQL→SQL translator (docs/SPEC.md §9).
//!
//! [`translate()`] turns a [`LoweredQuery`] into ONE SQL statement over the same `main.rdf_quad`
//! / `main.rdf_term` storage the native engine reads. Its result columns are the projected
//! variables' `TermId`s (INTEGER, NULL = unbound), so the benchmark compares evaluation
//! strategies only: SQLite's planner and VDBE instead of ours, same ids, same decode path.
//!
//! Translation scheme:
//! * BGP → self-joins of `main.rdf_quad` aliases (`g = 0`, a FROM graph or the GRAPH constant;
//!   constants as parameters; repeated variables as equalities; no `INDEXED BY`);
//! * FILTER → conditions with id fast paths (exact inline-integer / inline-decimal
//!   comparisons, IRI / simple-literal equality) and helper functions for everything that needs
//!   values;
//! * OPTIONAL → `LEFT JOIN … ON <compat> AND <filter>`; UNION → `UNION ALL` with NULL padding;
//!   MINUS → `NOT EXISTS` with the shared-domain condition; `[NOT] EXISTS` → correlated
//!   `[NOT] EXISTS`; VALUES → constants or a `VALUES` derived table; BIND → helper calls;
//! * GROUP BY → SQL `GROUP BY` with COUNT in SQL and the other aggregates as helper
//!   aggregates; DISTINCT, ORDER BY `sparql_sortkey(id)`, LIMIT / OFFSET;
//! * property paths → derived tables, closures as `WITH RECURSIVE … UNION`;
//! * GRAPH / FROM / FROM NAMED → conditions on `g` (an RDF merge of several graphs reads each
//!   triple's copy in its smallest graph).
//!
//! Every accepted statement prepares on the host's SQLite 3.45: joins stay within 64 tables,
//! compound SELECTs within 500 terms, AND / OR lists are balanced, and a final check of the
//! parser-stack depth and the other compile-time limits (`limits`) rejects what remains.
//!
//! Anything outside that subset is `Error::Unsupported("baseline: …")`. The helper functions
//! the SQL calls, and their exact semantics, are listed in [`helpers`].
#![forbid(unsafe_code)]

mod expr;
mod graph;
pub mod helpers;
mod limits;
mod numeric;
mod path;
mod sql;
mod translate;

#[cfg(test)]
mod tests;

pub use helpers::{HELPER_FUNCTIONS, HelperFunction, HelperKind, eval_template};
use sq_core::Result;
use sq_core::ir::LoweredQuery;

/// One SQL statement for a SPARQL query.
///
/// * `sql` is a single `SELECT` (possibly with nested `WITH RECURSIVE` subqueries); placeholder
///   `?N` binds `params[N - 1]`. Every query constant is a parameter; no query text is ever
///   part of `sql`.
/// * SELECT: one row per solution, one INTEGER `TermId` column per entry of `columns` (the
///   projected variable names, in order); NULL = unbound. A query projecting no variable has
///   exactly one placeholder column (always NULL) instead.
/// * ASK: at most one row (one placeholder column); the answer is whether a row exists.
/// * Result ids may be TEMP ids of the query's `LoweredQuery::temps` (constants absent from
///   the store) or TEMP ids the helper functions interned into that same TempDict while the
///   statement ran: decode them through it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqlQuery {
    pub sql: String,
    pub params: Vec<i64>,
    pub columns: Vec<String>,
}

/// Translates a lowered SELECT or ASK query (CONSTRUCT / DESCRIBE and the constructs listed
/// in the crate docs are `Error::Unsupported`).
pub fn translate(q: &LoweredQuery) -> Result<SqlQuery> {
    translate::Translator::new(q).run()
}
