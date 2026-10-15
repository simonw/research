//! sq-exec: physical operators (§5.3), paths (§5.5), EXISTS, GraphLoop, CONSTRUCT/DESCRIBE,
//! and `Execution` (a SolutionSource / TripleSource). Depends only on sq-core + sq-expr.
#![allow(unused_variables, dead_code)]
use std::rc::Rc;
use std::sync::Arc;

use sq_core::eval_api::{SolutionSource, TripleSource};
use sq_core::ir::QueryForm;
use sq_core::plan::{ExecProfile, QueryOptions, QueryPlan};
use sq_core::store::QuadStore;
use sq_core::term::Term;
use sq_core::{Result, TermId};

/// Hook for connection-level caches. The store's `Dictionary` already uses the connection
/// DecodeCache/LookupCache, so the default (empty) value is always valid.
#[derive(Clone, Default)]
pub struct ExecCaches {}

/// Owns the operator tree, the store handle, the ExecCtx, the form and the projection.
pub struct Execution {
    _p: (),
}
impl Execution {
    pub fn new(store: Rc<dyn QuadStore>, plan: QueryPlan, opts: &QueryOptions, caches: ExecCaches) -> Result<Self> {
        todo!()
    }
    pub fn form(&self) -> &QueryForm {
        todo!()
    }
    pub fn ask(&mut self) -> Result<bool> {
        todo!()
    }
    /// Solutions (or triples): no decode.
    pub fn count(&mut self) -> Result<u64> {
        todo!()
    }
    pub fn profile(&self) -> ExecProfile {
        todo!()
    }
}
impl SolutionSource for Execution {
    fn variables(&self) -> &[String] {
        todo!()
    }
    fn next_rows(&mut self, out: &mut Vec<TermId>, max_rows: usize) -> Result<usize> {
        todo!()
    }
    fn decode(&mut self, ids: &[TermId], out: &mut Vec<Option<Arc<Term>>>) -> Result<()> {
        todo!()
    }
}
impl TripleSource for Execution {
    fn next_triples(&mut self, out: &mut Vec<[TermId; 3]>, max: usize) -> Result<usize> {
        todo!()
    }
    fn decode(&mut self, ids: &[TermId], out: &mut Vec<Option<Arc<Term>>>) -> Result<()> {
        todo!()
    }
}
