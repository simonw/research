//! `=` / `!=` matrix (§6.3), the `<` family (§6.4), effective boolean value (§6.4) and
//! term identity on ids.
//!
//! All functions return `Ok(None)` for a SPARQL error and `Err` only for fatal errors
//! (resolver I/O).
use std::cmp::Ordering;

use oxsdatatypes::Duration;
use sq_core::{Result, Tag, TermId};

use crate::EvalCtx;
use crate::numeric;
use crate::value::{Val, Value, id_of_val, inline_value, term_of_id, value_of, value_of_val};

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

/// True if a TEMP id and a dictionary id are compared: a slot that the planner allowed to stay
/// non-canonical can hold a TEMP id for a term that also has a dictionary row.
#[inline]
pub(crate) fn ambiguous_identity(a: TermId, b: TermId) -> bool {
    (a.is_temp() && b.is_dict()) || (b.is_temp() && a.is_dict())
}

/// RDF term equality on ids (sameTerm). Ids are compared directly except for the
/// TEMP-vs-dictionary case, which compares the decoded terms.
pub fn same_term_ids(a: TermId, b: TermId, cx: &mut EvalCtx) -> Result<bool> {
    if a == b {
        return Ok(true);
    }
    if ambiguous_identity(a, b) && effective_tag(a) == effective_tag(b) {
        return Ok(*term_of_id(a, cx)? == *term_of_id(b, cx)?);
    }
    Ok(false)
}

/// sameTerm on lazy values (computed values are compared through their canonical ids).
pub fn same_term(a: Val, b: Val, cx: &mut EvalCtx) -> Result<bool> {
    let a = id_of_val(a, cx, false)?;
    let b = id_of_val(b, cx, false)?;
    same_term_ids(a, b, cx)
}

/// The `=` operator on lazy values (§6.3). Ok(None) = type error.
pub fn equals(a: &Val, b: &Val, cx: &mut EvalCtx) -> Result<Option<bool>> {
    if let (Val::Id(x), Val::Id(y)) = (a, b) {
        let (x, y) = (*x, *y);
        if x == y {
            // Rule 1, with the NaN exception (only TYPED ids can be NaN).
            if may_be_nan(x) && value_of(x, cx)?.as_num().is_some_and(numeric::Num::is_nan) {
                return Ok(Some(false));
            }
            return Ok(Some(true));
        }
        if let Some(r) = equals_ids_fast(x, y) {
            return Ok(Some(r));
        }
        // Otherwise (including a TEMP id vs a dictionary id of the same term) compare values:
        // the matrix is reflexive on identical terms except for NaN.
    }
    let va = value_of_val(a.clone(), cx)?;
    let vb = value_of_val(b.clone(), cx)?;
    Ok(equals_values(&va, &vb))
}

/// Decides `=` for two distinct ids from their tags alone, when possible.
fn equals_ids_fast(x: TermId, y: TermId) -> Option<bool> {
    if x.is_temp() || y.is_temp() {
        // Distinct TEMP/dictionary ids may still denote the same term (see same_term_ids).
        return None;
    }
    match (x.tag(), y.tag()) {
        // Rule 2: an IRI or blank node is only equal to the identical term.
        (Tag::Iri | Tag::BNode, _) | (_, Tag::Iri | Tag::BNode) => Some(false),
        // Canonical inline encodings: distinct ids are distinct values.
        (Tag::Int, Tag::Int) | (Tag::Bool, Tag::Bool) => Some(false),
        // Simple literals: distinct ids are distinct strings.
        (Tag::ShortStr | Tag::Str, Tag::ShortStr | Tag::Str) => Some(false),
        // Different known classes (string / numeric / boolean / dateTime / date).
        (Tag::ShortStr | Tag::Str, Tag::Int | Tag::Bool | Tag::Decimal | Tag::DateTime | Tag::Date)
        | (Tag::Int | Tag::Bool | Tag::Decimal | Tag::DateTime | Tag::Date, Tag::ShortStr | Tag::Str) => Some(false),
        // Language tags are lower-cased on load: distinct ids differ in lexical form or tag.
        (Tag::Lang, Tag::Lang) => Some(false),
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

/// The `<` family on lazy values (§6.4). Ok(None) = type error.
pub fn compare(a: &Val, b: &Val, cx: &mut EvalCtx) -> Result<Option<Order>> {
    if let (Val::Id(x), Val::Id(y)) = (a, b) {
        // Inline integers order by payload (2 compares, no decode).
        if let (Some(x), Some(y)) = (x.as_int(), y.as_int()) {
            return Ok(Some(x.cmp(&y).into()));
        }
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
