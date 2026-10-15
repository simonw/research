//! The helper SQL functions that the baseline translator's SQL calls
//! (`sq_sqlgen::HELPER_FUNCTIONS`, whose `semantics` texts are the contract implemented
//! here) and their evaluation context.
//!
//! The functions evaluate with sq-expr over the store's dictionary. A *term argument* is an
//! INTEGER `TermId` or NULL (unbound). While this connection steps a baseline statement, the
//! statement's [`HelperCtx`] is active ([`Baselines`]): TEMP ids are decoded through, and
//! computed terms interned into, that query's `TempDict`, so ids flow between SQL and the
//! helpers exactly as they do inside the native executor. Called directly from user SQL, a
//! helper runs as a complete read entry with a fresh context; TEMP ids then mean nothing,
//! so TEMP arguments are treated as unbound and TEMP results are returned as NULL.
use std::cell::RefCell;
use std::panic::AssertUnwindSafe;
use std::rc::Rc;
use std::sync::Arc;

use rand::SeedableRng;
use rand::rngs::StdRng;
use rusqlite::functions::{Aggregate, Context};
use rusqlite::types::{Value, ValueRef};
use sq_core::codec;
use sq_core::eval_api::{ExistsEval, TempDict, TermResolver};
use sq_core::ir::{AggFunc, AggSpec, ExistsId};
use sq_core::store::Dictionary;
use sq_core::{Error, Result, Tag, Term, TermId};
use sq_expr::{Accumulator, EvalCtx, Num, Val, ValueCache};
use sq_sqlite::SqliteStore;

use crate::error::sql_error;
use crate::state::ConnState;

// ---------------------------------------------------------------- evaluation context

/// Everything a helper evaluation needs besides the store: the executing query's TempDict
/// and per-query caches.
pub(crate) struct HelperCtx {
    pub(crate) temps: TempDict,
    values: ValueCache,
    rng: StdRng,
    now: oxsdatatypes::DateTime,
    base_iri: Option<String>,
}

impl HelperCtx {
    pub(crate) fn new(temps: TempDict, base_iri: Option<String>) -> Self {
        HelperCtx {
            temps,
            values: ValueCache::new(),
            // The baseline rejects RAND / UUID; the generator only satisfies EvalCtx.
            rng: StdRng::seed_from_u64(0x5350_4152_514C),
            now: oxsdatatypes::DateTime::now(),
            base_iri,
        }
    }

    /// Forgets everything query-specific (direct calls reuse one context).
    pub(crate) fn reset(&mut self) {
        if !self.temps.is_empty() {
            self.temps = TempDict::new();
        }
        self.values.clear();
    }

    /// Runs `f` with an sq-expr evaluation context over `store` and this context.
    pub(crate) fn eval<R>(&mut self, store: &SqliteStore, f: impl FnOnce(&mut EvalCtx<'_>) -> Result<R>) -> Result<R> {
        let mut resolver = HelperResolver { store, temps: &mut self.temps };
        let mut exists = NoExists;
        let mut cx = EvalCtx {
            resolver: &mut resolver,
            exists: &mut exists,
            values: &mut self.values,
            now: self.now,
            base_iri: self.base_iri.as_deref(),
            rng: &mut self.rng,
        };
        f(&mut cx)
    }
}

/// `TermResolver` of the helpers: the store's dictionary plus the query's TempDict (§4.3).
struct HelperResolver<'a> {
    store: &'a SqliteStore,
    temps: &'a mut TempDict,
}

impl TermResolver for HelperResolver<'_> {
    fn term(&mut self, id: TermId) -> Result<Arc<Term>> {
        match id.tag() {
            Tag::Temp => self.temps.get(id).cloned().ok_or_else(|| Error::internal(format!("unknown TEMP id {id:?}"))),
            Tag::Unbound | Tag::Error => Err(Error::internal(format!("cannot decode {id:?}"))),
            _ => self.store.decode_one(id),
        }
    }

    fn intern(&mut self, t: &Term, canonical: bool) -> Result<TermId> {
        let r = term_ref(t);
        if let Some(id) = codec::encode_inline(r) {
            return Ok(id);
        }
        if canonical && let Some(id) = self.store.lookup(r)? {
            return Ok(id);
        }
        Ok(self.temps.intern(t.clone()))
    }

    fn fresh_bnode(&mut self) -> TermId {
        self.temps.fresh_bnode()
    }
}

/// The helpers never evaluate EXISTS (the baseline translates it to SQL).
struct NoExists;

impl ExistsEval for NoExists {
    fn exists(&mut self, _id: ExistsId, _row: &[TermId]) -> Result<bool> {
        Err(Error::Unsupported("EXISTS inside a helper function".into()))
    }
}

/// A borrowed oxrdf view of a term (no allocation).
pub(crate) fn term_ref(t: &Term) -> oxrdf::TermRef<'_> {
    match t {
        Term::Iri(i) => oxrdf::NamedNodeRef::new_unchecked(i).into(),
        Term::BNode(b) => oxrdf::BlankNodeRef::new_unchecked(b).into(),
        Term::Literal(l) => match &l.lang {
            Some(lang) => oxrdf::LiteralRef::new_language_tagged_literal_unchecked(&l.lex, lang).into(),
            None => {
                oxrdf::LiteralRef::new_typed_literal(&l.lex, oxrdf::NamedNodeRef::new_unchecked(&l.datatype)).into()
            }
        },
    }
}

// ---------------------------------------------------------------- active baseline statements

/// The helper contexts of the baseline statements this connection is stepping, innermost
/// last. A baseline run activates its context only around the calls that step its
/// statement, so the innermost entry is always the statement whose helpers are running.
#[derive(Default)]
pub(crate) struct Baselines(RefCell<Vec<Rc<RefCell<HelperCtx>>>>);

impl Baselines {
    pub(crate) fn current(&self) -> Result<Option<Rc<RefCell<HelperCtx>>>> {
        Ok(self.0.try_borrow().map_err(|_| Error::Reentrant)?.last().cloned())
    }

    /// Makes `ctx` the active context until the returned guard is dropped.
    pub(crate) fn activate(self: &Rc<Self>, ctx: &Rc<RefCell<HelperCtx>>) -> Result<Activation> {
        self.0.try_borrow_mut().map_err(|_| Error::Reentrant)?.push(ctx.clone());
        Ok(Activation(self.clone()))
    }
}

pub(crate) struct Activation(Rc<Baselines>);

impl Drop for Activation {
    fn drop(&mut self) {
        if let Ok(mut v) = self.0.0.try_borrow_mut() {
            v.pop();
        }
    }
}

// ---------------------------------------------------------------- arguments and results

/// A term argument: an INTEGER id or NULL (unbound; id 0 is unbound too).
pub(crate) fn term_arg(ctx: &Context<'_>, i: usize) -> Result<Option<TermId>> {
    match ctx.get_raw(i) {
        ValueRef::Null => Ok(None),
        ValueRef::Integer(v) => Ok(Some(TermId(v)).filter(|id| id.is_bound())),
        other => {
            Err(Error::Invalid(format!("argument {} must be an INTEGER term id, got {}", i + 1, other.data_type())))
        }
    }
}

/// Whether `id` denotes a term in this context (TEMP ids only inside a baseline statement).
fn usable(id: TermId, in_baseline: bool) -> bool {
    match id.tag() {
        Tag::Temp => in_baseline,
        Tag::Unbound | Tag::Error | Tag::Reserved12 | Tag::Reserved13 => false,
        _ => true,
    }
}

/// Whether decoding `id` may read the dictionary (direct calls then need a read entry).
fn reads_store(ids: &[Option<TermId>]) -> bool {
    ids.iter().flatten().any(|id| id.is_dict())
}

/// A term result: the id, or NULL. Outside a baseline statement TEMP ids are NULL.
fn id_result(id: Option<TermId>, in_baseline: bool) -> Value {
    match id {
        Some(id) if in_baseline && id.is_bound() && id != TermId::ERROR => Value::Integer(id.raw()),
        Some(id) => crate::value::public_id(id).map_or(Value::Null, Value::Integer),
        None => Value::Null,
    }
}

fn bool_result(b: Option<bool>) -> Value {
    b.map_or(Value::Null, |b| Value::Integer(i64::from(b)))
}

// ---------------------------------------------------------------- scalar helpers

/// `sparql_num(id)`: integer family → INTEGER, decimal / float / double → REAL (NaN → NULL).
pub(crate) fn num(st: &ConnState, ctx: &Context<'_>) -> Result<Value> {
    let Some(id) = term_arg(ctx, 0)? else { return Ok(Value::Null) };
    st.with_helper(id.is_dict(), |h, store, in_baseline| {
        if !usable(id, in_baseline) {
            return Ok(Value::Null);
        }
        h.eval(store, |cx| {
            Ok(match sq_expr::value_of(id, cx)?.as_num() {
                Some(Num::Integer(i)) => Value::Integer(i),
                Some(n) => {
                    let f = n.to_f64();
                    if f.is_nan() { Value::Null } else { Value::Real(f) }
                }
                None => Value::Null,
            })
        })
    })
}

/// `sparql_str(id)`: lexical form of a simple, xsd:string or language-tagged literal.
pub(crate) fn str(st: &ConnState, ctx: &Context<'_>) -> Result<Value> {
    let Some(id) = term_arg(ctx, 0)? else { return Ok(Value::Null) };
    if id.tag() == Tag::ShortStr {
        return Ok(codec::decode_short_str(id).map_or(Value::Null, Value::Text));
    }
    if !matches!(id.kind(), sq_core::Kind::Literal) || id.is_inline() {
        // IRIs, blank nodes and every inline typed value are not strings.
        return Ok(Value::Null);
    }
    st.with_helper(id.is_dict(), |h, store, in_baseline| {
        if !usable(id, in_baseline) {
            return Ok(Value::Null);
        }
        h.eval(store, |cx| {
            Ok(match sq_expr::value_of(id, cx)? {
                sq_expr::Value::Str(s) => Value::Text(s.to_string()),
                sq_expr::Value::LangStr { lex, .. } => Value::Text(lex.to_string()),
                _ => Value::Null,
            })
        })
    })
}

/// `sparql_ebv(id)`: effective boolean value, NULL on error.
pub(crate) fn ebv(st: &ConnState, ctx: &Context<'_>) -> Result<Value> {
    let Some(id) = term_arg(ctx, 0)? else { return Ok(Value::Null) };
    st.with_helper(id.is_dict(), |h, store, in_baseline| {
        if !usable(id, in_baseline) {
            return Ok(Value::Null);
        }
        h.eval(store, |cx| sq_expr::ebv(&Val::Id(id), cx).map(bool_result))
    })
}

/// `sparql_sortkey(id)`: memcmp-ordered ORDER BY key (BLOB).
pub(crate) fn sortkey(st: &ConnState, ctx: &Context<'_>) -> Result<Value> {
    let Some(id) = term_arg(ctx, 0)? else { return Ok(Value::Null) };
    st.with_helper(id.is_dict(), |h, store, in_baseline| {
        if !usable(id, in_baseline) {
            return Ok(Value::Null);
        }
        h.eval(store, |cx| Ok(Value::Blob(sq_expr::sort_key(id, cx)?.into_bytes())))
    })
}

/// Arguments `first..` as a row (slot i = argument first + i; unusable ids are unbound).
fn row_of(ctx: &Context<'_>, first: usize) -> Result<Vec<Option<TermId>>> {
    (first..ctx.len()).map(|i| term_arg(ctx, i)).collect()
}

fn eval_row(args: &[Option<TermId>], in_baseline: bool) -> Vec<TermId> {
    args.iter().map(|a| a.filter(|id| usable(*id, in_baseline)).unwrap_or(TermId::UNBOUND)).collect()
}

/// `sparql_regex(text, pattern[, flags])`: REGEX on term arguments, 1 / 0 / NULL.
pub(crate) fn regex(st: &ConnState, ctx: &Context<'_>) -> Result<Value> {
    let args = row_of(ctx, 0)?;
    let compiled = st.template("REGEX", args.len())?;
    st.with_helper(reads_store(&args), |h, store, in_baseline| {
        let row = eval_row(&args, in_baseline);
        h.eval(store, |cx| compiled.ebv(&row, cx).map(bool_result))
    })
}

/// `sparql_eval(op, a1, …, an)`: the operator or builtin `op` on term arguments (canonical
/// result id, NULL on error).
pub(crate) fn eval(st: &ConnState, ctx: &Context<'_>) -> Result<Value> {
    if ctx.is_empty() {
        return Err(Error::Invalid("sparql_eval needs an operator name".into()));
    }
    let op = match ctx.get_raw(0) {
        ValueRef::Text(t) => {
            std::str::from_utf8(t).map_err(|_| Error::Invalid("sparql_eval: operator is not UTF-8".into()))?
        }
        other => {
            return Err(Error::Invalid(format!("sparql_eval: the operator must be TEXT, got {}", other.data_type())));
        }
    };
    let args = row_of(ctx, 1)?;
    let compiled = st.template(op, args.len())?;
    st.with_helper(true, |h, store, in_baseline| {
        let row = eval_row(&args, in_baseline);
        let id = h.eval(store, |cx| compiled.eval(&row, cx, true))?;
        Ok(id_result(id, in_baseline))
    })
}

// ---------------------------------------------------------------- aggregate helpers

/// An aggregate helper (`sparql_sum`, `sparql_avg`, `sparql_min`, `sparql_max`,
/// `sparql_sample`, `sparql_group_concat`): an sq-expr accumulator per group, fed
/// `add(Some(id))` per non-NULL row and `add(None)` per NULL row. SQL applies DISTINCT
/// itself, so the accumulators never dedupe.
pub(crate) struct AggHelper {
    st: Arc<ConnState>,
    func: AggFunc,
}

/// Group state. Accumulators are not `UnwindSafe`; a panic poisons nothing here because
/// rusqlite drops the group state after reporting it.
pub(crate) type AggState = AssertUnwindSafe<Box<dyn Accumulator>>;

impl AggHelper {
    pub(crate) fn new(st: Arc<ConnState>, func: AggFunc) -> Self {
        AggHelper { st, func }
    }

    fn accumulator(func: AggFunc) -> AggState {
        AssertUnwindSafe(sq_expr::accumulator(&AggSpec { func, distinct: false, arg: None }))
    }

    /// The separator argument of the two-argument `sparql_group_concat`.
    fn separator(&self, ctx: &Context<'_>) -> Result<String> {
        let Some(id) = term_arg(ctx, 1)? else { return Ok(" ".into()) };
        self.st.with_helper(id.is_dict(), |h, store, in_baseline| {
            if !usable(id, in_baseline) {
                return Err(Error::Invalid("sparql_group_concat: unknown separator id".into()));
            }
            h.eval(store, |cx| match sq_expr::value_of(id, cx)? {
                sq_expr::Value::Str(s) => Ok(s.to_string()),
                _ => Err(Error::Invalid("sparql_group_concat: the separator must be a simple literal".into())),
            })
        })
    }
}

impl Aggregate<AggState, Value> for AggHelper {
    fn init(&self, ctx: &mut Context<'_>) -> rusqlite::Result<AggState> {
        let func = match &self.func {
            AggFunc::GroupConcat { .. } if ctx.len() == 2 => {
                AggFunc::GroupConcat { separator: self.separator(ctx).map_err(sql_error)? }
            }
            f => f.clone(),
        };
        Ok(Self::accumulator(func))
    }

    fn step(&self, ctx: &mut Context<'_>, acc: &mut AggState) -> rusqlite::Result<()> {
        let id = term_arg(ctx, 0).map_err(sql_error)?;
        self.st
            .with_helper(id.is_some_and(TermId::is_dict), |h, store, in_baseline| {
                h.eval(store, |cx| acc.add(id.filter(|id| usable(*id, in_baseline)), cx))
            })
            .map_err(sql_error)
    }

    fn finalize(&self, _ctx: &mut Context<'_>, acc: Option<AggState>) -> rusqlite::Result<Value> {
        // An empty group: a fresh accumulator's result (SUM 0, AVG 0, GROUP_CONCAT "", …).
        let mut acc = acc.unwrap_or_else(|| Self::accumulator(self.func.clone()));
        self.st
            .with_helper(true, |h, store, in_baseline| {
                let id = h.eval(store, |cx| acc.finish(cx))?;
                Ok(id_result(id, in_baseline))
            })
            .map_err(sql_error)
    }
}

/// Every aggregate helper, with its argument counts.
pub(crate) fn aggregates() -> [(&'static str, &'static [i32], AggFunc); 6] {
    let gc = AggFunc::GroupConcat { separator: " ".into() };
    [
        ("sparql_sum", &[1], AggFunc::Sum),
        ("sparql_avg", &[1], AggFunc::Avg),
        ("sparql_min", &[1], AggFunc::Min),
        ("sparql_max", &[1], AggFunc::Max),
        ("sparql_sample", &[1], AggFunc::Sample),
        ("sparql_group_concat", &[1, 2], gc),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use sq_core::Literal;

    #[test]
    fn term_refs_mirror_the_terms() {
        for t in [
            Term::Iri("http://ex/a".into()),
            Term::BNode("b7".into()),
            Term::Literal(Literal::simple("long enough string")),
            Term::Literal(Literal::lang("chat", "fr")),
            Term::Literal(Literal::typed("1e3", "http://www.w3.org/2001/XMLSchema#double")),
        ] {
            assert_eq!(Term::from_oxrdf(term_ref(&t)), t);
        }
    }

    #[test]
    fn usable_ids() {
        assert!(usable(TermId::from_int(3).unwrap(), false));
        assert!(!usable(TermId::temp(1, Tag::Str), false));
        assert!(usable(TermId::temp(1, Tag::Str), true));
        assert!(!usable(TermId::ERROR, true));
        assert!(!usable(TermId(12), true));
        assert_eq!(id_result(Some(TermId::temp(1, Tag::Str)), false), Value::Null);
        assert_eq!(id_result(Some(TermId::temp(1, Tag::Str)), true), Value::Integer(TermId::temp(1, Tag::Str).raw()));
        assert_eq!(id_result(Some(TermId::ERROR), true), Value::Null);
        assert_eq!(id_result(None, true), Value::Null);
    }
}
