//! sq-engine: facade — query/count/explain/update/load over `Rc<dyn QuadStore>`; the SPARQL
//! Update executor (§5.7) and the generic loader.
#![allow(unused_variables, dead_code)]
use std::rc::Rc;

use sq_core::plan::QueryOptions;
use sq_core::store::{QuadStore, QuadStoreWriter};
use sq_core::Result;
use sq_exec::{ExecCaches, Execution};
use sq_plan::ExplainMode;
use sq_results::ResultFormat;

/// lower + optimize + validate(debug) + Execution::new
pub fn prepare(store: Rc<dyn QuadStore>, query: &spargebra::Query, opts: &QueryOptions, caches: ExecCaches) -> Result<Execution> {
    todo!()
}
pub fn query_to_string(store: Rc<dyn QuadStore>, query: &str, fmt: ResultFormat, opts: &QueryOptions) -> Result<String> {
    todo!()
}
pub fn explain(store: Rc<dyn QuadStore>, query: &str, mode: ExplainMode, opts: &QueryOptions) -> Result<String> {
    todo!()
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UpdateReport {
    pub inserted: u64,
    pub deleted: u64,
}
/// Caller provides atomicity (write scope).
pub fn update(store: Rc<dyn QuadStoreWriter>, update: &spargebra::Update, opts: &QueryOptions) -> Result<UpdateReport> {
    todo!()
}
/// Generic loader (MemStore, small inserts).
pub fn load(store: &dyn QuadStoreWriter, data: &[u8], format: oxrdfio::RdfFormat, base: Option<&str>, graph: Option<&str>) -> Result<u64> {
    todo!()
}
