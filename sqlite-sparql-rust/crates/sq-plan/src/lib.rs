//! sq-plan: parse, lower (spargebra -> Logical IR), static properties, rewrites,
//! cardinality, join ordering, physical selection, validate, explain. See docs/SPEC.md §7.
#![allow(unused_variables, dead_code)]
use rustc_hash::FxHashMap;
use sq_core::batch::{Slot, SlotSet};
use sq_core::ir::{Logical, LoweredQuery};
use sq_core::plan::{ExecProfile, QueryOptions, QueryPlan};
use sq_core::store::QuadStore;
use sq_core::Result;

pub fn parse_query(text: &str, base: Option<&str>) -> Result<spargebra::Query> {
    todo!()
}
pub fn parse_update(text: &str, base: Option<&str>) -> Result<spargebra::Update> {
    todo!()
}
pub fn lower_query(q: &spargebra::Query, store: &dyn QuadStore, opts: &QueryOptions) -> Result<LoweredQuery> {
    todo!()
}
/// Update WHERE: returns the lowered pattern (form = Select(all in-scope slots)) and the variable map.
pub fn lower_pattern(
    p: &spargebra::algebra::GraphPattern,
    using: Option<&spargebra::algebra::QueryDataset>,
    base: Option<&str>,
    store: &dyn QuadStore,
    opts: &QueryOptions,
) -> Result<(LoweredQuery, FxHashMap<spargebra::term::Variable, Slot>)> {
    todo!()
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Props {
    pub vars: SlotSet,
    pub certain: SlotSet,
    pub maybe: SlotSet,
    pub safe_inject: SlotSet,
}
pub fn props(l: &Logical, exists: &[Logical]) -> Props {
    todo!()
}
pub fn optimize(q: LoweredQuery, store: &dyn QuadStore, opts: &QueryOptions) -> Result<QueryPlan> {
    todo!()
}
pub fn plan_query(q: &spargebra::Query, store: &dyn QuadStore, opts: &QueryOptions) -> Result<QueryPlan> {
    todo!()
}
pub fn validate(p: &QueryPlan) -> Result<()> {
    todo!()
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ExplainMode {
    Algebra,
    Logical,
    Plan,
    Analyze,
}
pub fn explain_logical(q: &LoweredQuery) -> String {
    todo!()
}
pub fn explain(p: &QueryPlan, profile: Option<&ExecProfile>) -> String {
    todo!()
}
