//! sq-oracle: deliberately naive, spec-transcribed evaluator over the Logical IR
//! (SPARQL 1.1 §18): bottom-up bags, nested loops, explicit substitution for EXISTS,
//! per-graph evaluation of GRAPH, ALP for paths. Uses sq-expr for expressions.
#![allow(unused_variables, dead_code)]
use std::sync::Arc;

use sq_core::ir::LoweredQuery;
use sq_core::store::QuadStore;
use sq_core::term::Term;
use sq_core::Result;

#[derive(Clone, Debug, PartialEq)]
pub enum OracleOutput {
    Solutions { vars: Vec<String>, rows: Vec<Vec<Option<Arc<Term>>>> },
    Boolean(bool),
    Graph(Vec<[Arc<Term>; 3]>),
}

pub fn evaluate(q: &LoweredQuery, store: &dyn QuadStore) -> Result<OracleOutput> {
    todo!()
}
