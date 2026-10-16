//! sq-expr: SPARQL value model, expression compiler/evaluator, builtins, casts, `=` matrix,
//! EBV, ORDER BY sort keys and aggregate accumulators. See docs/SPEC.md §6.
//!
//! * [`compile`] turns an `sq_core::ir::Expr` into a [`CompiledExpr`] (constants resolved,
//!   regexes compiled once, fast paths selected); [`CompiledExpr::eval`] / [`CompiledExpr::ebv`]
//!   evaluate it on a row of term ids. SPARQL errors are `Ok(None)`; `Err` is fatal.
//! * Values are decoded lazily ([`Val`]): inline ids decode from their payload with no I/O,
//!   other ids through the [`ValueCache`] and the host's `TermResolver`.
//! * Computed results stay lazy ([`Val::V`], [`Val::Typed`]) and are interned only when an id
//!   is required ([`CompiledExpr::eval`], [`id_of_val`]), so FILTER / ORDER BY intermediates
//!   never grow the execution's TempDict. They get canonical lexical forms and inline ids when
//!   encodable, so sameTerm ⇔ id equality holds for computed terms too.
//! * Query constants are canonical ids (a TEMP constant is absent from the store), so the
//!   constant fast paths decode only a TEMP row id against a dictionary constant.
//! * [`sort_key`] gives the total ORDER BY / MIN / MAX order (§6.5), [`accumulator`] the
//!   aggregates (§6.7).
#![forbid(unsafe_code)]

mod aggregate;
mod bnode;
mod cast;
mod compare;
mod compile;
mod eval;
mod numeric;
mod regex;
mod sortkey;
mod strings;
mod value;
mod xsd;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

#[cfg(test)]
mod tests;

use sq_core::eval_api::{ExistsEval, TermResolver};

pub use aggregate::{Accumulator, accumulator};
pub use compare::{Order, compare, compare_values, ebv, ebv_value, equals, equals_values, same_term, same_term_ids};
pub use compile::{CompiledExpr, compile};
pub use numeric::Num;
pub use sortkey::{SortKey, sort_key, sort_key_val, sort_key_value};
pub use value::{
    Val, Value, ValueCache, id_of_val, id_of_value, inline_id_of_val, inline_id_of_value, inline_value, is_present,
    kind_of_val, term_of_id, term_of_value, value_of, value_of_val,
};
pub use xsd::{XsdType, well_known_iri_id};

/// Everything an evaluation needs from its host (one per execution, or per batch).
pub struct EvalCtx<'a> {
    /// Decodes ids and interns computed terms (store dictionary + caches + TempDict).
    pub resolver: &'a mut dyn TermResolver,
    /// EXISTS / NOT EXISTS callback.
    pub exists: &'a mut dyn ExistsEval,
    /// Per-execution decoded-value cache.
    pub values: &'a mut ValueCache,
    /// NOW() (fixed for the whole query).
    pub now: oxsdatatypes::DateTime,
    /// Base IRI for IRI()/URI().
    pub base_iri: Option<&'a str>,
    /// RAND(), UUID(), STRUUID().
    pub rng: &'a mut dyn rand::RngCore,
}
