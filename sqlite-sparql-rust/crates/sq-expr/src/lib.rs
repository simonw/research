//! sq-expr: SPARQL value model, expression compiler/evaluator, builtins, casts, `=` matrix,
//! EBV, ORDER BY sort keys and aggregate accumulators. See docs/SPEC.md §6.
#![allow(unused_variables, dead_code)]
use std::sync::Arc;

use rustc_hash::FxHashMap;
use sq_core::eval_api::{ExistsEval, TempDict, TermResolver};
use sq_core::ir::{AggSpec, Expr};
use sq_core::{Result, TermId};

/// Decoded value (cached per TermId in `ValueCache`).
#[derive(Clone, Debug)]
pub enum Value {
    Iri(Arc<str>),
    BNode(Arc<str>),
    Str(Arc<str>),
    LangStr { lex: Arc<str>, lang: Arc<str> },
    Boolean(bool),
    Integer(i64),
    Decimal(oxsdatatypes::Decimal),
    Float(oxsdatatypes::Float),
    Double(oxsdatatypes::Double),
    DateTime(oxsdatatypes::DateTime),
    Date(oxsdatatypes::Date),
    Time(oxsdatatypes::Time),
    GYear(oxsdatatypes::GYear),
    GYearMonth(oxsdatatypes::GYearMonth),
    GMonth(oxsdatatypes::GMonth),
    GMonthDay(oxsdatatypes::GMonthDay),
    GDay(oxsdatatypes::GDay),
    Duration(oxsdatatypes::Duration),
    YearMonthDuration(oxsdatatypes::YearMonthDuration),
    DayTimeDuration(oxsdatatypes::DayTimeDuration),
    /// Unknown datatype.
    Other { lex: Arc<str>, datatype: Arc<str> },
    /// Known XSD datatype, invalid lexical form (incl. facets, e.g. "300"^^xsd:byte).
    IllTyped { lex: Arc<str>, datatype: Arc<str> },
}

/// Lazy value: `Id` keeps the lexical form for STR/DATATYPE/sameTerm.
#[derive(Clone, Debug)]
pub enum Val {
    Id(TermId),
    V(Value),
}

/// Per-execution decoded-value cache.
#[derive(Default)]
pub struct ValueCache {
    map: FxHashMap<TermId, Value>,
}
impl ValueCache {
    pub fn new() -> Self {
        Self::default()
    }
}

pub struct EvalCtx<'a> {
    pub resolver: &'a mut dyn TermResolver,
    pub exists: &'a mut dyn ExistsEval,
    pub values: &'a mut ValueCache,
    pub now: oxsdatatypes::DateTime,
    pub base_iri: Option<&'a str>,
    pub rng: &'a mut dyn rand::RngCore,
}

/// Tree with pre-encoded/pre-parsed constants, compiled regexes and fast paths.
pub struct CompiledExpr {
    _p: (),
}

pub fn compile(e: &Expr, temps: &TempDict) -> Result<CompiledExpr> {
    todo!()
}

impl CompiledExpr {
    /// Ok(None) = SPARQL error or unbound; Err only for fatal errors (I/O, interrupt, memory).
    pub fn eval(&self, row: &[TermId], cx: &mut EvalCtx, canonical: bool) -> Result<Option<TermId>> {
        todo!()
    }
    pub fn ebv(&self, row: &[TermId], cx: &mut EvalCtx) -> Result<Option<bool>> {
        todo!()
    }
}

/// Lexicographic, total ORDER BY key (§6.5). Internal representation is private to sq-expr.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct SortKey(Vec<u8>);

/// UNBOUND/ERROR -> lowest.
pub fn sort_key(id: TermId, cx: &mut EvalCtx) -> Result<SortKey> {
    todo!()
}

pub trait Accumulator {
    fn add(&mut self, v: Option<TermId>, cx: &mut EvalCtx) -> Result<()>;
    fn finish(&mut self, cx: &mut EvalCtx) -> Result<Option<TermId>>;
}

pub fn accumulator(spec: &AggSpec) -> Box<dyn Accumulator> {
    todo!()
}
