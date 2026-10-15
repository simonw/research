//! `=` / `!=` matrix (§6.3), the `<` family (§6.4), effective boolean value (§6.4) and
//! term identity on ids.
//!
//! All functions return `Ok(None)` for a SPARQL error and `Err` only for fatal errors
//! (resolver I/O).
use std::cmp::Ordering;

use oxsdatatypes::Duration;
use sq_core::{Result, Tag, Term, TermId};

use crate::EvalCtx;
use crate::numeric;
use crate::value::{Val, Value, inline_id_of_val, inline_value, term_of_id, value_of, value_of_val};

/// Result of an ordering comparison that is not a type error.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Order {
    Less,
    Equal,
    Greater,
    /// A NaN operand: every ordering operator is false (but `!=` is true).
    Unordered,
}

impl From<Ordering> for Order {
    fn from(o: Ordering) -> Order {
        match o {
            Ordering::Less => Order::Less,
            Ordering::Equal => Order::Equal,
            Ordering::Greater => Order::Greater,
        }
    }
}

impl From<Option<Ordering>> for Order {
    fn from(o: Option<Ordering>) -> Order {
        match o {
            Some(Ordering::Less) => Order::Less,
            Some(Ordering::Equal) => Order::Equal,
            Some(Ordering::Greater) => Order::Greater,
            None => Order::Unordered,
        }
    }
}

/// Dictionary tag an id would have (TEMP ids report their would-be tag).
#[inline]
pub(crate) fn effective_tag(id: TermId) -> Tag {
    if id.is_temp() { id.temp_tag() } else { id.tag() }
}

/// Could this id hold a float/double NaN? Only TYPED literals (dictionary or TEMP) can.
#[inline]
fn may_be_nan(id: TermId) -> bool {
    effective_tag(id) == Tag::Typed
}

/// True if two distinct row ids may still denote the same term: a slot that the planner
/// allowed to stay non-canonical can hold a TEMP id for a term that also has a dictionary row.
/// Any other pair of distinct ids denotes distinct terms (inline ids are canonical, and the
/// TempDict gives equal terms equal ids).
#[inline]
pub(crate) fn ambiguous_identity(a: TermId, b: TermId) -> bool {
    (a.is_temp() && b.is_dict()) || (b.is_temp() && a.is_dict())
}

/// [`ambiguous_identity`] for a row id `x` against a query constant `c`. Constants are
/// canonical (IR contract: a TEMP constant is absent from the store), so only a TEMP row id
/// against a dictionary constant needs the decoded comparison; a dictionary or inline row id
/// never denotes a TEMP constant's term.
#[inline]
pub(crate) fn may_alias_const(x: TermId, c: TermId) -> bool {
    x.is_temp() && c.is_dict()
}

/// RDF term equality on ids (sameTerm). Ids are compared directly except for the
/// TEMP-vs-dictionary case, which compares the decoded terms.
pub fn same_term_ids(a: TermId, b: TermId, cx: &mut EvalCtx) -> Result<bool> {
    if a == b {
        return Ok(true);
    }
    if ambiguous_identity(a, b) {
        return decoded_same_term(a, b, cx);
    }
    Ok(false)
}

/// Compares the decoded terms of two ids that may alias (same would-be tag required).
fn decoded_same_term(a: TermId, b: TermId, cx: &mut EvalCtx) -> Result<bool> {
    if effective_tag(a) != effective_tag(b) {
        return Ok(false);
    }
    Ok(*term_of_id(a, cx)? == *term_of_id(b, cx)?)
}

/// sameTerm on lazy values. Computed values are compared with the other operand's term
/// without being interned.
pub fn same_term(a: Val, b: Val, cx: &mut EvalCtx) -> Result<bool> {
    match (&a, &b) {
        (Val::Id(x), Val::Id(y)) => same_term_ids(*x, *y, cx),
        (Val::Id(x), v) | (v, Val::Id(x)) => same_term_computed(v, *x, cx),
        _ => Ok(a.computed_term() == b.computed_term()),
    }
}

/// sameTerm of a row value and a query constant `c` (see [`may_alias_const`]).
pub(crate) fn same_term_const(v: &Val, c: TermId, cx: &mut EvalCtx) -> Result<bool> {
    match v {
        Val::Id(x) if *x == c => Ok(true),
        Val::Id(x) if may_alias_const(*x, c) => decoded_same_term(*x, c, cx),
        Val::Id(_) => Ok(false),
        v => same_term_computed(v, c, cx),
    }
}

/// sameTerm of a computed value and an id, without interning the value.
fn same_term_computed(v: &Val, id: TermId, cx: &mut EvalCtx) -> Result<bool> {
    // An inline-encodable term only ever has its inline id.
    if let Some(inline) = inline_id_of_val(v) {
        return Ok(inline == id);
    }
    if id.is_inline() || effective_tag(id) != computed_tag(v) {
        return Ok(false);
    }
    Ok(computed_matches_term(v, &*term_of_id(id, cx)?))
}

/// The tag a non-inline computed value's id would have.
fn computed_tag(v: &Val) -> Tag {
    match v {
        Val::Id(id) => effective_tag(*id),
        Val::V(Value::Iri(_)) => Tag::Iri,
        Val::V(Value::BNode(_)) => Tag::BNode,
        Val::V(Value::Str(_)) => Tag::Str,
        Val::V(Value::LangStr { .. }) => Tag::Lang,
        Val::V(_) | Val::Typed { .. } => Tag::Typed,
    }
}

/// True if `t` is the term of the computed value `v` (`v.computed_term() == t`, without
/// building the term).
fn computed_matches_term(v: &Val, t: &Term) -> bool {
    match (v, t) {
        (Val::V(Value::Iri(a)), Term::Iri(b)) | (Val::V(Value::BNode(a)), Term::BNode(b)) => a == b,
        (Val::V(Value::Str(s)), Term::Literal(l)) => l.is_simple() && l.lex == *s,
        (Val::V(Value::LangStr { lex, lang }), Term::Literal(l)) => l.lang.as_ref() == Some(lang) && l.lex == *lex,
        (Val::V(v), Term::Literal(l)) => {
            l.lang.is_none() && v.datatype_iri() == Some(&*l.datatype) && *l.lex == *v.lexical()
        }
        (Val::Typed { value, datatype, lex }, Term::Literal(l)) => {
            l.lang.is_none() && *l.datatype == *datatype.iri() && *l.lex == *Val::typed_lexical(value, lex)
        }
        _ => false,
    }
}

/// The `=` operator on lazy values (§6.3). Ok(None) = type error.
pub fn equals(a: &Val, b: &Val, cx: &mut EvalCtx) -> Result<Option<bool>> {
    equals_with(a, b, ambiguous_identity, cx)
}

/// `=` of a row value and a query constant `c` (see [`may_alias_const`]).
pub(crate) fn equals_const(a: &Val, c: TermId, cx: &mut EvalCtx) -> Result<Option<bool>> {
    equals_with(a, &Val::Id(c), may_alias_const, cx)
}

fn equals_with(a: &Val, b: &Val, may_alias: fn(TermId, TermId) -> bool, cx: &mut EvalCtx) -> Result<Option<bool>> {
    if let (Val::Id(x), Val::Id(y)) = (a, b) {
        let (x, y) = (*x, *y);
        if x == y {
            // Rule 1, with the NaN exception (only TYPED ids can be NaN).
            if may_be_nan(x) && value_of(x, cx)?.as_num().is_some_and(numeric::Num::is_nan) {
                return Ok(Some(false));
            }
            return Ok(Some(true));
        }
        if !may_alias(x, y)
            && let Some(r) = equals_distinct_terms(x, y)
        {
            return Ok(Some(r));
        }
        // Otherwise (including a TEMP id vs a dictionary id of the same term) compare values:
        // the matrix is reflexive on identical terms except for NaN.
    }
    let va = value_of_val(a.clone(), cx)?;
    let vb = value_of_val(b.clone(), cx)?;
    Ok(equals_values(&va, &vb))
}

/// Decides `=` from the (would-be) tags of two ids known to denote distinct terms, when
/// possible.
fn equals_distinct_terms(x: TermId, y: TermId) -> Option<bool> {
    match (effective_tag(x), effective_tag(y)) {
        // Rule 2: an IRI or blank node is only equal to the identical term.
        (Tag::Iri | Tag::BNode, _) | (_, Tag::Iri | Tag::BNode) => Some(false),
        // Canonical inline encodings: distinct ids are distinct values.
        (Tag::Int, Tag::Int) | (Tag::Bool, Tag::Bool) => Some(false),
        // Simple literals: distinct terms are distinct strings.
        (Tag::ShortStr | Tag::Str, Tag::ShortStr | Tag::Str) => Some(false),
        // Different known classes (string / numeric / boolean / dateTime / date).
        (Tag::ShortStr | Tag::Str, Tag::Int | Tag::Bool | Tag::Decimal | Tag::DateTime | Tag::Date)
        | (Tag::Int | Tag::Bool | Tag::Decimal | Tag::DateTime | Tag::Date, Tag::ShortStr | Tag::Str) => Some(false),
        // Language tags are lower-cased on load: distinct dictionary ids differ in lexical
        // form or tag. (A TEMP language literal is compared by value.)
        (Tag::Lang, Tag::Lang) if !x.is_temp() && !y.is_temp() => Some(false),
        // Lang vs a known class: false (rule 4).
        (Tag::Lang, Tag::ShortStr | Tag::Str | Tag::Int | Tag::Bool | Tag::Decimal | Tag::DateTime | Tag::Date)
        | (Tag::ShortStr | Tag::Str | Tag::Int | Tag::Bool | Tag::Decimal | Tag::DateTime | Tag::Date, Tag::Lang) => {
            Some(false)
        }
        _ => None,
    }
}

/// Equality class of a known value (rule 5/6). None for IRIs, blank nodes, lang strings,
/// Other and IllTyped (handled by rules 2-4).
fn class(v: &Value) -> Option<u8> {
    Some(match v {
        Value::Integer(_) | Value::Decimal(_) | Value::Float(_) | Value::Double(_) => 0,
        Value::Str(_) => 1,
        Value::Boolean(_) => 2,
        Value::DateTime(_) => 3,
        Value::Date(_) => 4,
        Value::Time(_) => 5,
        Value::GYear(_) => 6,
        Value::GYearMonth(_) => 7,
        Value::GMonth(_) => 8,
        Value::GMonthDay(_) => 9,
        Value::GDay(_) => 10,
        Value::Duration(_) | Value::YearMonthDuration(_) | Value::DayTimeDuration(_) => 11,
        _ => return None,
    })
}

/// Any member of the duration family as a general `xsd:duration`.
pub(crate) fn as_duration(v: &Value) -> Option<Duration> {
    match v {
        Value::Duration(d) => Some(*d),
        Value::YearMonthDuration(d) => Some((*d).into()),
        Value::DayTimeDuration(d) => Some((*d).into()),
        _ => None,
    }
}

/// Date/time equality: indeterminate comparisons (one side without timezone, within ±14 h)
/// are errors.
fn date_eq<T: PartialOrd>(a: &T, b: &T) -> Option<bool> {
    a.partial_cmp(b).map(|o| o == Ordering::Equal)
}

/// The `=` matrix on decoded values. None = type error.
pub fn equals_values(a: &Value, b: &Value) -> Option<bool> {
    use Value::*;
    match (a, b) {
        (Iri(x), Iri(y)) | (BNode(x), BNode(y)) => Some(x == y),
        // Rule 2.
        (Iri(_) | BNode(_), _) | (_, Iri(_) | BNode(_)) => Some(false),
        // Rule 3: unknown datatypes and ill-typed literals.
        (Other { .. } | IllTyped { .. }, LangStr { .. }) | (LangStr { .. }, Other { .. } | IllTyped { .. }) => Some(false),
        (Other { lex: l1, datatype: d1 }, Other { lex: l2, datatype: d2 })
        | (IllTyped { lex: l1, datatype: d1 }, IllTyped { lex: l2, datatype: d2 })
            if l1 == l2 && d1 == d2 =>
        {
            Some(true)
        }
        (Other { .. } | IllTyped { .. }, _) | (_, Other { .. } | IllTyped { .. }) => None,
        // Rule 4.
        (LangStr { lex: l1, lang: g1 }, LangStr { lex: l2, lang: g2 }) => Some(l1 == l2 && g1.eq_ignore_ascii_case(g2)),
        (LangStr { .. }, _) | (_, LangStr { .. }) => Some(false),
        // Rules 5 and 6.
        _ => {
            let (ca, cb) = (class(a)?, class(b)?);
            if ca != cb {
                return Some(false);
            }
            match (a, b) {
                (Str(x), Str(y)) => Some(x == y),
                (Boolean(x), Boolean(y)) => Some(x == y),
                (DateTime(x), DateTime(y)) => date_eq(x, y),
                (Date(x), Date(y)) => date_eq(x, y),
                (Time(x), Time(y)) => date_eq(x, y),
                (GYear(x), GYear(y)) => date_eq(x, y),
                (GYearMonth(x), GYearMonth(y)) => date_eq(x, y),
                (GMonth(x), GMonth(y)) => date_eq(x, y),
                (GMonthDay(x), GMonthDay(y)) => date_eq(x, y),
                (GDay(x), GDay(y)) => date_eq(x, y),
                _ if ca == 0 => Some(numeric::compare(a.as_num()?, b.as_num()?) == Some(Ordering::Equal)),
                _ => Some(as_duration(a)? == as_duration(b)?),
            }
        }
    }
}

/// Ordering classes (§6.4) as bits: two operands can only be ordered if they share one.
mod ord_class {
    pub const NUMERIC: u8 = 1;
    pub const STRING: u8 = 1 << 1;
    pub const BOOLEAN: u8 = 1 << 2;
    pub const DATE_TIME: u8 = 1 << 3;
    pub const DATE: u8 = 1 << 4;
    /// time, the g* types, yearMonthDuration and dayTimeDuration (told apart after decoding).
    pub const OTHER: u8 = 1 << 5;
    /// Any typed literal: a TYPED id may hold any typed value except a string (xsd:string
    /// literals are STR / SHORTSTR).
    pub const TYPED: u8 = NUMERIC | BOOLEAN | DATE_TIME | DATE | OTHER;
}

/// The ordering classes a (would-be) tag allows, without decoding. IRIs, blank nodes and
/// language literals are never ordered.
#[inline]
pub(crate) fn tag_order_classes(tag: Tag) -> u8 {
    match tag {
        Tag::Int | Tag::Decimal => ord_class::NUMERIC,
        Tag::ShortStr | Tag::Str => ord_class::STRING,
        Tag::Bool => ord_class::BOOLEAN,
        Tag::DateTime => ord_class::DATE_TIME,
        Tag::Date => ord_class::DATE,
        Tag::Typed => ord_class::TYPED,
        _ => 0,
    }
}

/// The ordering class of a decoded value (0 = never ordered).
fn value_order_classes(v: &Value) -> u8 {
    match v {
        Value::Integer(_) | Value::Decimal(_) | Value::Float(_) | Value::Double(_) => ord_class::NUMERIC,
        Value::Str(_) => ord_class::STRING,
        Value::Boolean(_) => ord_class::BOOLEAN,
        Value::DateTime(_) => ord_class::DATE_TIME,
        Value::Date(_) => ord_class::DATE,
        Value::Time(_)
        | Value::GYear(_)
        | Value::GYearMonth(_)
        | Value::GMonth(_)
        | Value::GMonthDay(_)
        | Value::GDay(_)
        | Value::YearMonthDuration(_)
        | Value::DayTimeDuration(_) => ord_class::OTHER,
        _ => 0,
    }
}

fn order_classes(v: &Val) -> u8 {
    match v {
        Val::Id(id) => tag_order_classes(effective_tag(*id)),
        Val::V(v) | Val::Typed { value: v, .. } => value_order_classes(v),
    }
}

/// The `<` family on lazy values (§6.4). Ok(None) = type error. Operands whose tags (or
/// computed values) share no ordering class are a type error decided without decoding.
pub fn compare(a: &Val, b: &Val, cx: &mut EvalCtx) -> Result<Option<Order>> {
    if let (Val::Id(x), Val::Id(y)) = (a, b) {
        // Inline integers order by payload (2 compares, no decode).
        if let (Some(x), Some(y)) = (x.as_int(), y.as_int()) {
            return Ok(Some(x.cmp(&y).into()));
        }
    }
    if order_classes(a) & order_classes(b) == 0 {
        return Ok(None);
    }
    let va = value_of_val(a.clone(), cx)?;
    let vb = value_of_val(b.clone(), cx)?;
    Ok(compare_values(&va, &vb))
}

fn date_cmp<T: PartialOrd>(a: &T, b: &T) -> Option<Order> {
    // None from partial_cmp is an indeterminate comparison: a type error, not NaN.
    a.partial_cmp(b).map(Order::from)
}

/// The `<` family on decoded values: numeric (with promotion), simple/xsd:string (code
/// points), boolean, date/time types (XSD partial order) and yearMonth/dayTime durations.
pub fn compare_values(a: &Value, b: &Value) -> Option<Order> {
    use Value::*;
    match (a, b) {
        (Str(x), Str(y)) => Some(x.as_bytes().cmp(y.as_bytes()).into()),
        (Boolean(x), Boolean(y)) => Some(x.cmp(y).into()),
        (DateTime(x), DateTime(y)) => date_cmp(x, y),
        (Date(x), Date(y)) => date_cmp(x, y),
        (Time(x), Time(y)) => date_cmp(x, y),
        (GYear(x), GYear(y)) => date_cmp(x, y),
        (GYearMonth(x), GYearMonth(y)) => date_cmp(x, y),
        (GMonth(x), GMonth(y)) => date_cmp(x, y),
        (GMonthDay(x), GMonthDay(y)) => date_cmp(x, y),
        (GDay(x), GDay(y)) => date_cmp(x, y),
        (YearMonthDuration(x), YearMonthDuration(y)) => Some(x.cmp(y).into()),
        (DayTimeDuration(x), DayTimeDuration(y)) => Some(x.cmp(y).into()),
        _ => Some(numeric::compare(a.as_num()?, b.as_num()?).into()),
    }
}

/// Effective boolean value of a lazy value (§6.4). Inline ids and dictionary simple literals
/// (always longer than 7 bytes, hence non-empty) need no decode.
pub fn ebv(v: &Val, cx: &mut EvalCtx) -> Result<Option<bool>> {
    if let Some(value) = v.computed_value() {
        return Ok(ebv_value(value));
    }
    if let Val::Id(id) = v {
        let id = *id;
        match effective_tag(id) {
            Tag::Bool | Tag::Int | Tag::Decimal => {
                let value = inline_value(id).expect("inline id");
                return Ok(ebv_value(&value));
            }
            Tag::ShortStr => return Ok(Some((id.0 as u64 >> 4) & 0xF != 0)),
            Tag::Str if !id.is_temp() => return Ok(Some(true)),
            Tag::Iri | Tag::BNode | Tag::Lang | Tag::DateTime | Tag::Date => return Ok(None),
            Tag::Unbound | Tag::Error => return Ok(None),
            _ => {}
        }
    }
    let value = value_of_val(v.clone(), cx)?;
    Ok(ebv_value(&value))
}

/// EBV of a decoded value: boolean → itself; ill-typed boolean or numeric → false; string →
/// non-empty; numeric → non-zero and not NaN; everything else → error.
pub fn ebv_value(v: &Value) -> Option<bool> {
    match v {
        Value::Boolean(b) => Some(*b),
        Value::Str(s) => Some(!s.is_empty()),
        Value::IllTyped { datatype, .. } => match crate::xsd::XsdType::from_iri(datatype) {
            Some(t) if t == crate::xsd::XsdType::Boolean
                || t.is_integer_family()
                || matches!(t, crate::xsd::XsdType::Decimal | crate::xsd::XsdType::Float | crate::xsd::XsdType::Double) =>
            {
                Some(false)
            }
            _ => None,
        },
        v => v.as_num().map(|n| !n.is_zero_or_nan()),
    }
}
