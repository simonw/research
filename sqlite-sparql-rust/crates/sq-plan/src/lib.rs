//! sq-plan: parse, lower (spargebra -> Logical IR), static properties, rewrites,
//! cardinality, join ordering, physical selection, validate, explain. See docs/SPEC.md §7.
//!
//! Pipeline (`plan_query`): [`parse_query`] -> [`lower_query`] -> [`optimize`] ->
//! [`validate()`] (debug builds). `optimize` runs, in order:
//! * logical rewrites (`rewrite`): `GRAPH <c>` membership, GRAPH ?g push-or-loop, filter
//!   placement and sameTerm seed constants;
//! * physical planning (`physical`): join groups ordered by DP over connected subsets (greedy
//!   beyond 12 units), BindJoin / HashJoin / NestedLoop per `JoinPolicy` and cost, OPTIONAL,
//!   MINUS, EXISTS (correlated or decorrelated into Semi / Anti joins), TopK, Count, GraphLoop
//!   / GraphNames, then pre-order ids and canonical flags.
//!
//! Module map: `lower` (IR construction), `props` (certain / maybe / safe_inject),
//! `rewrite::{graph, filter}`, `card` (cardinality), `physical::{join, scan, semi, finish}`,
//! `validate`, `explain`.
mod card;
mod explain;
mod expr;
mod logical;
mod lower;
mod physical;
mod props;
mod rewrite;
mod validate;

use rustc_hash::FxHashMap;
use sq_core::batch::Slot;
use sq_core::ir::LoweredQuery;
use sq_core::plan::{QueryOptions, QueryPlan};
use sq_core::store::QuadStore;
use sq_core::{Error, Result};

pub use explain::{explain, explain_logical, explain_logical_with, explain_with, ExplainMode};
pub use props::{plan_props, props, Props};
pub use validate::validate;

/// Parse a SPARQL query (`Error::Syntax` on failure).
pub fn parse_query(text: &str, base: Option<&str>) -> Result<spargebra::Query> {
    parser(base)?.parse_query(text).map_err(|e| Error::Syntax(e.to_string()))
}

/// Parse a SPARQL update (`Error::Syntax` on failure).
pub fn parse_update(text: &str, base: Option<&str>) -> Result<spargebra::Update> {
    parser(base)?.parse_update(text).map_err(|e| Error::Syntax(e.to_string()))
}

fn parser(base: Option<&str>) -> Result<spargebra::SparqlParser> {
    let p = spargebra::SparqlParser::new();
    match base {
        Some(b) => p.with_base_iri(b).map_err(|e| Error::Syntax(format!("invalid base IRI <{b}>: {e}"))),
        None => Ok(p),
    }
}

/// Lower a parsed query (spec §7.1 step 2). Constants resolve against `store`'s dictionary.
pub fn lower_query(q: &spargebra::Query, store: &dyn QuadStore, opts: &QueryOptions) -> Result<LoweredQuery> {
    lower::lower_query(q, store, opts)
}

/// Update WHERE: returns the lowered pattern (form = Select(all in-scope slots)) and the variable map.
pub fn lower_pattern(
    p: &spargebra::algebra::GraphPattern,
    using: Option<&spargebra::algebra::QueryDataset>,
    base: Option<&str>,
    store: &dyn QuadStore,
    opts: &QueryOptions,
) -> Result<(LoweredQuery, FxHashMap<spargebra::term::Variable, Slot>)> {
    lower::lower_pattern(p, using, base, store, opts)
}

/// Rewrite and plan a lowered query (spec §7.1 steps 3-5); validated in debug builds.
pub fn optimize(q: LoweredQuery, store: &dyn QuadStore, opts: &QueryOptions) -> Result<QueryPlan> {
    let named = physical::named_graphs(&q, store)?;
    let rewritten = rewrite::rewrite(q.root.clone(), q.exists.clone(), &named)?;
    let plan = physical::plan_query(&q, rewritten, named, store, opts)?;
    if cfg!(debug_assertions) {
        validate(&plan)?;
    }
    Ok(plan)
}

/// `lower_query` + `optimize`.
pub fn plan_query(q: &spargebra::Query, store: &dyn QuadStore, opts: &QueryOptions) -> Result<QueryPlan> {
    optimize(lower_query(q, store, opts)?, store, opts)
}
