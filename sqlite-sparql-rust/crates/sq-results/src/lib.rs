//! sq-results: SPARQL JSON/XML/CSV/TSV (sparesults), N-Triples/N-Quads/Turtle (oxrdfio),
//! native SQL value mapping (§8.4) and per-row SPARQL-JSON. Depends only on sq-core.
#![allow(unused_variables, dead_code)]
use std::sync::Arc;

use sq_core::eval_api::{SolutionSource, TripleSource};
use sq_core::term::Term;
use sq_core::Result;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ResultFormat {
    Json,
    Xml,
    Csv,
    Tsv,
    NTriples,
    NQuads,
    Turtle,
}
impl ResultFormat {
    pub fn parse(s: &str) -> Result<Self> {
        todo!()
    }
}

pub fn write_solutions(src: &mut dyn SolutionSource, fmt: ResultFormat, w: &mut dyn std::io::Write) -> Result<()> {
    todo!()
}
pub fn write_boolean(b: bool, fmt: ResultFormat, w: &mut dyn std::io::Write) -> Result<()> {
    todo!()
}
pub fn write_triples(src: &mut dyn TripleSource, fmt: ResultFormat, w: &mut dyn std::io::Write) -> Result<()> {
    todo!()
}

#[derive(Clone, Debug, PartialEq)]
pub enum SqlNative {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
}
/// §8.4
pub fn sql_native(t: Option<&Term>) -> SqlNative {
    todo!()
}
/// SPARQL-JSON bindings object for one row.
pub fn solution_json(vars: &[String], row: &[Option<Arc<Term>>]) -> String {
    todo!()
}
