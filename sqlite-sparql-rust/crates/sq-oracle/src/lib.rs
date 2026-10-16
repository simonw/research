//! sq-oracle: a deliberately naive, spec-transcribed SPARQL 1.1 evaluator over the Logical IR
//! (`sq_core::ir::LoweredQuery`), used as the ground truth for differential testing of the
//! optimized engine (docs/SPEC.md §11, §12.1).
//!
//! It follows SPARQL 1.1 §18 literally and favours obvious correctness over speed:
//!
//! * bottom-up evaluation: every Logical node evaluates to a whole bag of solutions
//!   (`bag`), operators are the algebra's definitions (Join / LeftJoin / Minus / Filter /
//!   Extend / Group ... by nested loops), in `eval` and `group`;
//! * basic graph patterns match triple pattern by triple pattern against store cursors, on
//!   any index whose prefix is the bound positions (`dataset`);
//! * the dataset is explicit: the default graph is the RDF merge of its graphs (a set of
//!   triples), `GRAPH` evaluates its pattern once per named graph of the dataset;
//! * `EXISTS` evaluates its pattern with the current solution substituted (§18.6);
//! * property paths use the ALP procedure, with the zero-length rules of §18.4; a sequence
//!   joins its operands on a fresh middle variable (`path`);
//! * expressions, effective boolean values, ORDER BY keys and aggregates come from sq-expr,
//!   always producing canonical ids, so solution compatibility is id equality.
//!
//! Output terms are decoded to `Arc<Term>` (`forms`).
use std::sync::Arc;

use rand::SeedableRng;
use rand::rngs::StdRng;
use sq_core::ir::LoweredQuery;
use sq_core::store::QuadStore;
use sq_core::term::Term;
use sq_core::{Result, TermId};

mod bag;
mod dataset;
mod eval;
mod forms;
mod group;
mod ir;
mod path;
mod resolver;

#[cfg(test)]
mod tests;

use dataset::ActiveGraph;
use eval::{Ctx, Evaluator};

/// The result of a query.
#[derive(Clone, Debug, PartialEq)]
pub enum OracleOutput {
    /// SELECT: variable names and rows in projection order (None = unbound), in solution order.
    Solutions { vars: Vec<String>, rows: Vec<Vec<Option<Arc<Term>>>> },
    /// ASK.
    Boolean(bool),
    /// CONSTRUCT / DESCRIBE: a set of triples in first-seen order.
    Graph(Vec<[Arc<Term>; 3]>),
}

/// Evaluation settings that are otherwise taken from the environment.
#[derive(Clone, Debug, Default)]
pub struct OracleOptions {
    /// The value of `NOW()` (default: the current time).
    pub now: Option<oxsdatatypes::DateTime>,
    /// Seed for `RAND()`, `UUID()` and `STRUUID()` (default: from the OS).
    pub rng_seed: Option<u64>,
}

/// Evaluates a lowered query against `store` (bottom-up, nested loops, explicit substitution
/// for EXISTS, per-graph evaluation of GRAPH, ALP for paths).
pub fn evaluate(q: &LoweredQuery, store: &dyn QuadStore) -> Result<OracleOutput> {
    evaluate_with(q, store, &OracleOptions::default())
}

/// [`evaluate`] with a fixed `NOW()` and random seed.
pub fn evaluate_with(q: &LoweredQuery, store: &dyn QuadStore, opts: &OracleOptions) -> Result<OracleOutput> {
    ir::validate(q)?;
    let now = opts.now.unwrap_or_else(oxsdatatypes::DateTime::now);
    let rng = match opts.rng_seed {
        Some(seed) => StdRng::seed_from_u64(seed),
        None => StdRng::from_os_rng(),
    };
    let mut ev = Evaluator::new(q, store, now, rng)?;
    let unbound = vec![TermId::UNBOUND; ev.width];
    let solutions = ev.eval(&q.root, Ctx { seed: &unbound, active: ActiveGraph::Default })?;
    ev.output(&solutions)
}
