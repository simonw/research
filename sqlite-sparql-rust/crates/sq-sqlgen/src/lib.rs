//! sq-sqlgen: baseline SPARQL->SQL translator over the LoweredQuery (§9). Used only as the
//! benchmark comparison point: same storage, same ids, SQLite's planner instead of ours.
#![allow(unused_variables, dead_code)]
use sq_core::ir::LoweredQuery;
use sq_core::Result;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqlQuery {
    pub sql: String,
    pub params: Vec<i64>,
    pub columns: Vec<String>,
}

pub fn translate(q: &LoweredQuery) -> Result<SqlQuery> {
    todo!()
}
