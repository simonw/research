//! Value model (§6.1): decoded values, the lazy `Val`, the per-execution `ValueCache`, and the
//! conversions id → value (inline ids decode with no I/O) and value → term / id (canonical
//! lexical forms; inline-encodable results come out as inline ids so sameTerm ⇔ id equality).
use std::borrow::Cow;
use std::str::FromStr;
use std::sync::Arc;

use oxsdatatypes::{
    Date, DateTime, DayTimeDuration, Decimal, Double, Duration, Float, GDay, GMonth, GMonthDay, GYear, GYearMonth, Time,
    YearMonthDuration,
};
use rustc_hash::FxHashMap;
use sq_core::codec::{self, RDF_LANG_STRING_IRI, XSD_DATETIME_IRI, XSD_DATE_IRI, XSD_STRING_IRI};
use sq_core::{Error, Kind, Literal, Result, Tag, Term, TermId};

use crate::EvalCtx;
use crate::numeric::{Num, dec_raw};
use crate::xsd::{self, XsdType};

/// Decoded value (cached per TermId in `ValueCache`).
#[derive(Clone, Debug)]
pub enum Value {
    Iri(Arc<str>),
    BNode(Arc<str>),
    /// Simple literal or xsd:string.
    Str(Arc<str>),
    /// Language-tagged literal; `lang` is lower case.
    LangStr { lex: Arc<str>, lang: Arc<str> },
    Boolean(bool),
    /// xsd:integer and every derived integer type (facets checked on decode).
    Integer(i64),
    Decimal(Decimal),
    Float(Float),
    Double(Double),
    DateTime(DateTime),
    Date(Date),
    Time(Time),
    GYear(GYear),
    GYearMonth(GYearMonth),
    GMonth(GMonth),
    GMonthDay(GMonthDay),
    GDay(GDay),
    Duration(Duration),
    YearMonthDuration(YearMonthDuration),
    DayTimeDuration(DayTimeDuration),
    /// Unknown datatype.
    Other { lex: Arc<str>, datatype: Arc<str> },
    /// Known XSD datatype, invalid lexical (incl. facets, e.g. "300"^^xsd:byte).
    IllTyped { lex: Arc<str>, datatype: Arc<str> },
}

/// Lazy value: `Id` keeps the lexical form for STR/DATATYPE/sameTerm. Computed results stay
/// unresolved (`V`, `Typed`) until an id is actually required (`id_of_val`), so FILTER and
/// ORDER BY intermediates never grow the execution's TempDict.
#[derive(Clone, Debug)]
pub enum Val {
    Id(TermId),
    /// A computed value; its term is the canonical term of the value (`Value::to_term`).
    V(Value),
    /// A computed typed literal whose term is not the canonical term of its value: a
    /// derived-integer cast (`xsd:int("05")` is `"5"^^xsd:int`, value 5) or STRDT with a
    /// derived datatype or a non-canonical lexical form (`STRDT("05", xsd:integer)`).
    /// `value` is the value of that literal (a valid value of `datatype`).
    Typed {
        value: Value,
        datatype: XsdType,
        /// The lexical form; None = the canonical form of `value`.
        lex: Option<Arc<str>>,
    },
}

impl Val {
    /// The value of a computed `Val` (None for `Id`, which needs decoding).
    pub fn computed_value(&self) -> Option<&Value> {
        match self {
            Val::Id(_) => None,
            Val::V(v) | Val::Typed { value: v, .. } => Some(v),
        }
    }

    /// The RDF term of a computed `Val` (None for `Id`, which needs the resolver).
    pub fn computed_term(&self) -> Option<Term> {
        match self {
            Val::Id(_) => None,
            Val::V(v) => Some(v.to_term()),
            Val::Typed { value, datatype, lex } => Some(Term::Literal(Literal {
                lex: lex.clone().unwrap_or_else(|| value.lexical().into()),
                datatype: datatype.iri().into(),
                lang: None,
            })),
        }
    }

    /// Lexical form of a `Typed` literal.
    pub(crate) fn typed_lexical<'a>(value: &'a Value, lex: &'a Option<Arc<str>>) -> Cow<'a, str> {
        match lex {
            Some(l) => Cow::Borrowed(l),
            None => value.lexical(),
        }
    }
}

impl From<Num> for Value {
    fn from(n: Num) -> Value {
        match n {
            Num::Integer(i) => Value::Integer(i),
            Num::Decimal(d) => Value::Decimal(d),
            Num::Float(f) => Value::Float(f),
            Num::Double(d) => Value::Double(d),
        }
    }
}

impl Value {
    /// Value of a literal given as (lexical form, datatype IRI, language).
    pub fn from_literal(lit: &Literal) -> Value {
        if let Some(lang) = &lit.lang {
            return Value::LangStr { lex: lit.lex.clone(), lang: lang.clone() };
        }
        let Some(t) = XsdType::from_iri(&lit.datatype) else {
            return Value::Other { lex: lit.lex.clone(), datatype: lit.datatype.clone() };
        };
        let lex: &str = &lit.lex;
        let parsed = match t {
            XsdType::String => Some(Value::Str(lit.lex.clone())),
            XsdType::Boolean => xsd::parse_boolean(lex).map(Value::Boolean),
            XsdType::Decimal => xsd::parse_decimal(lex).map(Value::Decimal),
            XsdType::Float => xsd::parse_float(lex).map(|f| Value::Float(f.into())),
            XsdType::Double => xsd::parse_double(lex).map(|d| Value::Double(d.into())),
            XsdType::DateTime => DateTime::from_str(lex).ok().map(Value::DateTime),
            XsdType::Date => Date::from_str(lex).ok().map(Value::Date),
            XsdType::Time => Time::from_str(lex).ok().map(Value::Time),
            XsdType::GYear => GYear::from_str(lex).ok().map(Value::GYear),
            XsdType::GYearMonth => GYearMonth::from_str(lex).ok().map(Value::GYearMonth),
            XsdType::GMonth => GMonth::from_str(lex).ok().map(Value::GMonth),
            XsdType::GMonthDay => GMonthDay::from_str(lex).ok().map(Value::GMonthDay),
            XsdType::GDay => GDay::from_str(lex).ok().map(Value::GDay),
            XsdType::Duration => Duration::from_str(lex).ok().map(Value::Duration),
            XsdType::YearMonthDuration => YearMonthDuration::from_str(lex).ok().map(Value::YearMonthDuration),
            XsdType::DayTimeDuration => DayTimeDuration::from_str(lex).ok().map(Value::DayTimeDuration),
            integer => {
                let (lo, hi) = integer.integer_range().expect("remaining types are integers");
                xsd::parse_integer(lex).filter(|v| (lo..=hi).contains(v)).map(Value::Integer)
            }
        };
        parsed.unwrap_or_else(|| Value::IllTyped { lex: lit.lex.clone(), datatype: lit.datatype.clone() })
    }

    pub fn from_term(t: &Term) -> Value {
        match t {
            Term::Iri(i) => Value::Iri(i.clone()),
            Term::BNode(b) => Value::BNode(b.clone()),
            Term::Literal(l) => Value::from_literal(l),
        }
    }

    /// Coarse kind (IRI, blank node or literal).
    pub fn kind(&self) -> Kind {
        match self {
            Value::Iri(_) => Kind::Iri,
            Value::BNode(_) => Kind::BNode,
            _ => Kind::Literal,
        }
    }

    pub fn as_num(&self) -> Option<Num> {
        Some(match self {
            Value::Integer(i) => Num::Integer(*i),
            Value::Decimal(d) => Num::Decimal(*d),
            Value::Float(f) => Num::Float(*f),
            Value::Double(d) => Num::Double(*d),
            _ => return None,
        })
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self, Value::Integer(_) | Value::Decimal(_) | Value::Float(_) | Value::Double(_))
    }

    /// Lexical form: the IRI / label / string for non-typed terms, the stored form for
    /// Other / IllTyped, and the canonical form (oxsdatatypes `Display`) for typed values.
    pub fn lexical(&self) -> Cow<'_, str> {
        match self {
            Value::Iri(s) | Value::BNode(s) | Value::Str(s) => Cow::Borrowed(s),
            Value::LangStr { lex, .. } | Value::Other { lex, .. } | Value::IllTyped { lex, .. } => Cow::Borrowed(lex),
            Value::Boolean(b) => Cow::Borrowed(if *b { "true" } else { "false" }),
            Value::Integer(i) => Cow::Owned(i.to_string()),
            Value::Decimal(d) => Cow::Owned(d.to_string()),
            Value::Float(f) => Cow::Owned(f.to_string()),
            Value::Double(d) => Cow::Owned(d.to_string()),
            Value::DateTime(v) => Cow::Owned(v.to_string()),
            Value::Date(v) => Cow::Owned(v.to_string()),
            Value::Time(v) => Cow::Owned(v.to_string()),
            Value::GYear(v) => Cow::Owned(v.to_string()),
            Value::GYearMonth(v) => Cow::Owned(v.to_string()),
            Value::GMonth(v) => Cow::Owned(v.to_string()),
            Value::GMonthDay(v) => Cow::Owned(v.to_string()),
            Value::GDay(v) => Cow::Owned(v.to_string()),
            Value::Duration(v) => Cow::Owned(v.to_string()),
            Value::YearMonthDuration(v) => Cow::Owned(v.to_string()),
            Value::DayTimeDuration(v) => Cow::Owned(v.to_string()),
        }
    }

    /// The XSD type of a typed value (None for IRIs, blank nodes, lang strings, Other,
    /// IllTyped; `Integer` reports xsd:integer).
    pub fn xsd_type(&self) -> Option<XsdType> {
        Some(match self {
            Value::Str(_) => XsdType::String,
            Value::Boolean(_) => XsdType::Boolean,
            Value::Integer(_) => XsdType::Integer,
            Value::Decimal(_) => XsdType::Decimal,
            Value::Float(_) => XsdType::Float,
            Value::Double(_) => XsdType::Double,
            Value::DateTime(_) => XsdType::DateTime,
            Value::Date(_) => XsdType::Date,
            Value::Time(_) => XsdType::Time,
            Value::GYear(_) => XsdType::GYear,
            Value::GYearMonth(_) => XsdType::GYearMonth,
            Value::GMonth(_) => XsdType::GMonth,
            Value::GMonthDay(_) => XsdType::GMonthDay,
            Value::GDay(_) => XsdType::GDay,
            Value::Duration(_) => XsdType::Duration,
            Value::YearMonthDuration(_) => XsdType::YearMonthDuration,
            Value::DayTimeDuration(_) => XsdType::DayTimeDuration,
            _ => return None,
        })
    }

    /// Datatype IRI of a literal value (None for IRIs and blank nodes).
    pub fn datatype_iri(&self) -> Option<&str> {
        match self {
            Value::Iri(_) | Value::BNode(_) => None,
            Value::LangStr { .. } => Some(RDF_LANG_STRING_IRI),
            Value::Other { datatype, .. } | Value::IllTyped { datatype, .. } => Some(datatype),
            v => v.xsd_type().map(XsdType::iri),
        }
    }

    /// The RDF term with the canonical lexical form of this value.
    pub fn to_term(&self) -> Term {
        match self {
            Value::Iri(i) => Term::Iri(i.clone()),
            Value::BNode(b) => Term::BNode(b.clone()),
            Value::Str(s) => Term::Literal(Literal { lex: s.clone(), datatype: XSD_STRING_IRI.into(), lang: None }),
            Value::LangStr { lex, lang } => {
                Term::Literal(Literal { lex: lex.clone(), datatype: RDF_LANG_STRING_IRI.into(), lang: Some(lang.clone()) })
            }
            Value::Other { lex, datatype } | Value::IllTyped { lex, datatype } => {
                Term::Literal(Literal { lex: lex.clone(), datatype: datatype.clone(), lang: None })
            }
            v => Term::Literal(Literal {
                lex: v.lexical().into(),
                datatype: v.xsd_type().map(XsdType::iri).unwrap_or(XSD_STRING_IRI).into(),
                lang: None,
            }),
        }
    }
}

// ---------------------------------------------------------------- inline ids → values

/// Seconds from 0001-01-01T00:00:00 (oxsdatatypes' timeline origin) to the Unix epoch.
const UNIX_EPOCH_MS_FROM_YEAR_1: i128 = 719_162 * 86_400_000;
/// Milliseconds → `Decimal` raw units (10^18 per second).
const RAW_PER_MS: i128 = 1_000_000_000_000_000;

/// Timezone offset in minutes of a codec tz code (0 = none, 1 = Z, else offset + 842).
fn tz_offset_minutes(tz: u16) -> Option<i16> {
    match tz {
        0 => None,
        1 => Some(0),
        c => Some(c as i16 - 842),
    }
}

/// oxsdatatypes' 18-byte timestamp layout: the UTC instant on its timeline (Decimal seconds,
/// local time when there is no timezone) and the offset in minutes (0xFFFF = none).
fn timestamp_bytes(local_ms: i64, tz: u16) -> [u8; 18] {
    let off = tz_offset_minutes(tz);
    let utc_ms = local_ms as i128 + UNIX_EPOCH_MS_FROM_YEAR_1 - off.unwrap_or(0) as i128 * 60_000;
    let mut b = [0u8; 18];
    b[..16].copy_from_slice(&(utc_ms * RAW_PER_MS).to_be_bytes());
    b[16..].copy_from_slice(&match off {
        Some(o) => o.to_be_bytes(),
        None => [0xFF, 0xFF],
    });
    b
}

/// Decodes the inline numeric, boolean and date ids directly from their payload, with no
/// I/O and no allocation (SHORTSTR needs an allocation and is cached instead).
pub fn inline_value(id: TermId) -> Option<Value> {
    Some(match id.tag() {
        Tag::Int => Value::Integer(id.0 >> 4),
        Tag::Bool => Value::Boolean(id.0 >> 4 != 0),
        Tag::Decimal => {
            let (m, scale, _) = codec::decimal_parts(id);
            // |m| < 2^54 and scale ≤ 15: always representable.
            Value::Decimal(Decimal::new(m as i128, scale as u32).ok()?)
        }
        Tag::DateTime => {
            let (ms, tz) = codec::datetime_parts(id);
            Value::DateTime(DateTime::from_be_bytes(timestamp_bytes(ms, tz)))
        }
        Tag::Date => {
            let (days, tz) = codec::date_parts(id);
            Value::Date(Date::from_be_bytes(timestamp_bytes(days as i64 * 86_400_000, tz)))
        }
        _ => return None,
    })
}

/// Inline id of a computed value, when the value has one (no resolver needed).
pub fn inline_id_of_value(v: &Value) -> Option<TermId> {
    match v {
        Value::Integer(i) => TermId::from_int(*i),
        Value::Boolean(b) => Some(TermId::from_bool(*b)),
        Value::Decimal(d) => decimal_inline_id(*d),
        Value::Str(s) => codec::encode_short_str(s),
        Value::DateTime(d) => codec::encode_inline_literal(&d.to_string(), XSD_DATETIME_IRI, None),
        Value::Date(d) => codec::encode_inline_literal(&d.to_string(), XSD_DATE_IRI, None),
        _ => None,
    }
}

/// Inline DECIMAL id of the canonical form of `d` (`Decimal`'s `Display`), if encodable:
/// at most 15 fractional digits and a mantissa within i55.
fn decimal_inline_id(d: Decimal) -> Option<TermId> {
    let (mut m, mut scale) = (dec_raw(d), 18u8);
    while scale > 0 && m % 10 == 0 {
        m /= 10;
        scale -= 1;
    }
    if scale > 15 {
        return None;
    }
    let m = i64::try_from(m).ok()?;
    ((-(1i64 << 54))..(1i64 << 54)).contains(&m).then(|| codec::encode_decimal(m, scale, false))
}

// ---------------------------------------------------------------- per-execution cache

/// Default number of decoded values kept before the cache is reset.
const DEFAULT_LIMIT: usize = 1 << 17;
/// Slots of the admission filter (a power of two; 32 KiB of ids).
const SEEN_SLOTS: usize = 1 << 12;

/// Per-execution decoded-value cache (also holds the dynamic regex cache and the
/// `BNODE(str)` scope). Bounded: the decoded values are cleared when they reach the entry
/// limit; the `BNODE(str)` scope is bounded in bytes by two generations.
///
/// A decoded value enters the cache only on the second decode of its id (an admission
/// filter): a FILTER over distinct values (REGEX over every name) decodes each id once and
/// would only pay for inserts and rehashes, while repeated ids are cached from their second
/// use. The filter is a direct-mapped table of recently decoded ids, allocated on first use.
pub struct ValueCache {
    map: FxHashMap<TermId, Value>,
    limit: usize,
    /// Admission filter: ids decoded once, at `hash(id) % SEEN_SLOTS` (UNBOUND = empty).
    seen: Box<[TermId]>,
    pub(crate) regexes: crate::regex::RegexCache,
    pub(crate) bnodes: crate::bnode::BNodeScope,
}

impl Default for ValueCache {
    fn default() -> Self {
        Self::with_limit(DEFAULT_LIMIT)
    }
}

impl ValueCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// A cache holding at most `limit` decoded values (minimum 16).
    pub fn with_limit(limit: usize) -> Self {
        ValueCache {
            map: FxHashMap::default(),
            limit: limit.max(16),
            seen: Box::default(),
            regexes: Default::default(),
            bnodes: Default::default(),
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Empties the decoded values (the admission filter only holds ids, and stays).
    pub fn clear(&mut self) {
        self.map.clear();
    }

    #[inline]
    fn get(&self, id: TermId) -> Option<&Value> {
        if self.map.is_empty() { None } else { self.map.get(&id) }
    }

    /// True if `id` was decoded before (its value is then worth caching); otherwise records
    /// it. Two ids sharing a slot evict each other, which only delays caching.
    #[inline]
    fn admit(&mut self, id: TermId) -> bool {
        if self.seen.is_empty() {
            self.seen = vec![TermId::UNBOUND; SEEN_SLOTS].into_boxed_slice();
        }
        let h = (id.0 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> (64 - SEEN_SLOTS.trailing_zeros());
        let slot = &mut self.seen[h as usize];
        if *slot == id {
            return true;
        }
        *slot = id;
        false
    }

    fn insert(&mut self, id: TermId, v: Value) {
        if self.map.len() >= self.limit {
            self.map.clear();
        }
        self.map.insert(id, v);
    }
}

/// True for ids that carry a value: not UNBOUND and not the ERROR group-key sentinel.
#[inline]
pub fn is_present(id: TermId) -> bool {
    id.0 != 0 && id != TermId::ERROR
}

/// Decodes any bound id to its value: inline ids from the payload, everything else through
/// the `ValueCache` and then `EvalCtx::resolver`.
pub fn value_of(id: TermId, cx: &mut EvalCtx) -> Result<Value> {
    let tag = id.tag();
    match tag {
        Tag::Iri | Tag::BNode | Tag::Str | Tag::Lang | Tag::Typed | Tag::ShortStr | Tag::Temp if is_present(id) => {}
        _ => {
            return inline_value(id)
                .ok_or_else(|| Error::internal(format!("sq-expr: cannot decode term id {}", id.0)));
        }
    }
    if let Some(v) = cx.values.get(id) {
        return Ok(v.clone());
    }
    decode_value(id, tag, cx)
}

/// `value_of` of a non-inline id missing from the `ValueCache`: decodes it (SHORTSTR from
/// its payload, others through the resolver) and caches the value on its second decode.
#[inline(never)]
fn decode_value(id: TermId, tag: Tag, cx: &mut EvalCtx) -> Result<Value> {
    let v = if tag == Tag::ShortStr {
        let s = codec::decode_short_str(id).ok_or_else(|| Error::internal("sq-expr: invalid SHORTSTR id"))?;
        Value::Str(s.into())
    } else {
        let tag = if tag == Tag::Temp { id.temp_tag() } else { tag };
        let t = cx.resolver.term(id)?;
        value_of_term(tag, &t)
    };
    if cx.values.admit(id) {
        cx.values.insert(id, v.clone());
    }
    Ok(v)
}

/// Value of the term of an id with (effective) tag `tag`. The tag decides the value class
/// without a datatype comparison: a STR id is always an xsd:string literal, LANG a
/// language-tagged literal (codec::dict_tag, TempDict::intern). Only TYPED literals are
/// classified by datatype IRI.
#[inline]
fn value_of_term(tag: Tag, t: &Term) -> Value {
    match (tag, t) {
        (Tag::Str, Term::Literal(l)) if l.lang.is_none() => {
            debug_assert_eq!(&*l.datatype, XSD_STRING_IRI);
            Value::Str(l.lex.clone())
        }
        (Tag::Lang, Term::Literal(Literal { lex, lang: Some(lang), .. })) => {
            Value::LangStr { lex: lex.clone(), lang: lang.clone() }
        }
        (Tag::Iri, Term::Iri(i)) => Value::Iri(i.clone()),
        (Tag::BNode, Term::BNode(b)) => Value::BNode(b.clone()),
        _ => Value::from_term(t),
    }
}

/// Decodes a lazy value.
pub fn value_of_val(v: Val, cx: &mut EvalCtx) -> Result<Value> {
    match v {
        Val::Id(id) => value_of(id, cx),
        Val::V(v) | Val::Typed { value: v, .. } => Ok(v),
    }
}

/// The term of any bound id (inline ids decode without the resolver).
pub fn term_of_id(id: TermId, cx: &mut EvalCtx) -> Result<Arc<Term>> {
    if id.is_inline() {
        return codec::decode_inline(id).map(Arc::new).ok_or_else(|| Error::internal("sq-expr: invalid inline id"));
    }
    cx.resolver.term(id)
}

/// Canonical term of a computed value.
pub fn term_of_value(v: &Value) -> Term {
    v.to_term()
}

/// Id of a computed value: inline when encodable, else `TermResolver::intern(term, canonical)`.
pub fn id_of_value(v: &Value, cx: &mut EvalCtx, canonical: bool) -> Result<TermId> {
    if let Some(id) = inline_id_of_value(v) {
        return Ok(id);
    }
    cx.resolver.intern(&v.to_term(), canonical)
}

/// Inline id of a computed value, when its term has one (None for `Id`).
pub fn inline_id_of_val(v: &Val) -> Option<TermId> {
    match v {
        Val::Id(_) => None,
        Val::V(v) => inline_id_of_value(v),
        Val::Typed { value, datatype, lex } => {
            codec::encode_inline_literal(&Val::typed_lexical(value, lex), datatype.iri(), None)
        }
    }
}

/// Id of a lazy value. A pass-through TEMP id is re-interned when a canonical id is required
/// (it may come from a slot the planner allowed to stay non-canonical); TEMP blank nodes are
/// canonical by construction, and so are TEMP query constants (`CompiledExpr::eval` returns
/// those without calling this). Computed values are interned here, and only here.
pub fn id_of_val(v: Val, cx: &mut EvalCtx, canonical: bool) -> Result<TermId> {
    match v {
        Val::Id(id) if canonical && id.is_temp() && id.temp_tag() != Tag::BNode => {
            let t = cx.resolver.term(id)?;
            cx.resolver.intern(&t, true)
        }
        Val::Id(id) => Ok(id),
        // `id_of_value`, with the inline-id test in this function (it is the common case).
        Val::V(v) => match inline_id_of_value(&v) {
            Some(id) => Ok(id),
            None => cx.resolver.intern(&v.to_term(), canonical),
        },
        Val::Typed { value, datatype, lex } => {
            let lex = Val::typed_lexical(&value, &lex);
            match codec::encode_inline_literal(&lex, datatype.iri(), None) {
                Some(id) => Ok(id),
                None => {
                    let t = Term::Literal(Literal { lex: lex.into(), datatype: datatype.iri().into(), lang: None });
                    cx.resolver.intern(&t, canonical)
                }
            }
        }
    }
}

/// Coarse kind of a lazy value without decoding.
pub fn kind_of_val(v: &Val) -> Kind {
    match v {
        Val::Id(id) => id.kind(),
        Val::V(v) => v.kind(),
        Val::Typed { .. } => Kind::Literal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(lex: &str, dt: &str) -> Value {
        Value::from_literal(&Literal::typed(lex, &format!("http://www.w3.org/2001/XMLSchema#{dt}")))
    }

    #[test]
    fn classification() {
        assert!(matches!(lit("12", "integer"), Value::Integer(12)));
        assert!(matches!(lit("+012", "integer"), Value::Integer(12)));
        assert!(matches!(lit("300", "byte"), Value::IllTyped { .. }));
        assert!(matches!(lit("127", "byte"), Value::Integer(127)));
        assert!(matches!(lit("-1", "nonNegativeInteger"), Value::IllTyped { .. }));
        assert!(matches!(lit("0", "positiveInteger"), Value::IllTyped { .. }));
        assert!(matches!(lit("99999999999999999999", "integer"), Value::IllTyped { .. }));
        assert!(matches!(lit("abc", "integer"), Value::IllTyped { .. }));
        assert!(matches!(lit("1", "boolean"), Value::Boolean(true)));
        assert!(matches!(lit("TRUE", "boolean"), Value::IllTyped { .. }));
        assert!(matches!(lit("1.5", "decimal"), Value::Decimal(_)));
        assert!(matches!(lit("1e5", "decimal"), Value::IllTyped { .. }));
        assert!(matches!(lit("-INF", "double"), Value::Double(_)));
        assert!(matches!(lit("inf", "double"), Value::IllTyped { .. }));
        assert!(matches!(lit("2011-01-10T14:45:13.815-05:00", "dateTime"), Value::DateTime(_)));
        assert!(matches!(lit("2011-13-10", "date"), Value::IllTyped { .. }));
        assert!(matches!(lit("P1Y2M", "yearMonthDuration"), Value::YearMonthDuration(_)));
        assert!(matches!(lit("PT1H", "dayTimeDuration"), Value::DayTimeDuration(_)));
        assert!(matches!(lit("x", "string"), Value::Str(_)));
        assert!(matches!(lit("http://x", "anyURI"), Value::Other { .. }));
        let v = Value::from_literal(&Literal::typed("x", "http://example/dt"));
        assert!(matches!(v, Value::Other { .. }));
        let v = Value::from_literal(&Literal::lang("chat", "FR"));
        assert!(matches!(v, Value::LangStr { ref lang, .. } if &**lang == "fr"));
    }

    #[test]
    fn inline_datetime_matches_parser() {
        let samples = [
            "2010-06-21T11:28:01Z",
            "2010-12-21T15:38:02-08:00",
            "2011-02-01T01:02:03",
            "0001-01-01T00:00:00",
            "1969-12-31T23:59:59.999+14:00",
            "6000-12-31T23:59:59.5-14:00",
            "2000-02-29T12:00:00.25+05:30",
            "1600-03-01T00:00:00+00:00",
        ];
        for lex in samples {
            let id = codec::encode_inline_literal(lex, XSD_DATETIME_IRI, None).expect(lex);
            let Some(Value::DateTime(fast)) = inline_value(id) else { panic!("{lex}") };
            let parsed = DateTime::from_str(lex).unwrap();
            assert_eq!(fast.to_be_bytes(), parsed.to_be_bytes(), "{lex}");
        }
        for lex in ["2010-06-21", "0001-01-01Z", "6000-12-31-14:00", "1970-01-01+01:00", "1899-12-31"] {
            let id = codec::encode_inline_literal(lex, XSD_DATE_IRI, None).expect(lex);
            let Some(Value::Date(fast)) = inline_value(id) else { panic!("{lex}") };
            assert_eq!(fast.to_be_bytes(), Date::from_str(lex).unwrap().to_be_bytes(), "{lex}");
        }
    }

    #[test]
    fn inline_datetime_matches_parser_exhaustive_sample() {
        // Pseudo-random walk over the inline range (years 1..=6000, all offsets).
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        for _ in 0..2000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let y = 1 + (x % 6000) as i64;
            let mo = 1 + (x >> 13) % 12;
            let d = 1 + (x >> 17) % 28;
            let (h, mi, s, ms) = ((x >> 22) % 24, (x >> 27) % 60, (x >> 33) % 60, (x >> 39) % 1000);
            let tz = match (x >> 50) % 4 {
                0 => String::new(),
                1 => "Z".into(),
                _ => {
                    let off = ((x >> 52) % 1681) as i64 - 840;
                    format!("{}{:02}:{:02}", if off < 0 { '-' } else { '+' }, off.abs() / 60, off.abs() % 60)
                }
            };
            let frac = if ms == 0 { String::new() } else { format!(".{ms:03}").trim_end_matches('0').to_string() };
            let lex = format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}{frac}{tz}");
            let Some(id) = codec::encode_inline_literal(&lex, XSD_DATETIME_IRI, None) else { continue };
            let Some(Value::DateTime(fast)) = inline_value(id) else { panic!("{lex}") };
            assert_eq!(fast.to_be_bytes(), DateTime::from_str(&lex).unwrap().to_be_bytes(), "{lex}");
        }
    }

    #[test]
    fn inline_decimal_and_int_values() {
        let id = codec::encode_inline_literal("-12.25", sq_core::codec::XSD_DECIMAL_IRI, None).unwrap();
        assert!(matches!(inline_value(id), Some(Value::Decimal(d)) if d.to_string() == "-12.25"));
        let id = codec::encode_inline_literal("5.0", sq_core::codec::XSD_DECIMAL_IRI, None).unwrap();
        assert!(matches!(inline_value(id), Some(Value::Decimal(d)) if d.to_string() == "5"));
        assert!(matches!(inline_value(TermId::from_int(-7).unwrap()), Some(Value::Integer(-7))));
        assert!(matches!(inline_value(TermId::from_bool(true)), Some(Value::Boolean(true))));
        assert!(inline_value(codec::encode_short_str("abc").unwrap()).is_none());
    }

    #[test]
    fn computed_decimals_get_the_codec_id() {
        for s in ["0", "2", "-2", "0.5", "-12.25", "123456789012.123", "0.000000000000001", "0.0000000000000001", "18014398509481983", "18014398509481984", "1.000000000000000001"] {
            let d = Decimal::from_str(s).unwrap();
            let expect = codec::encode_inline_literal(&d.to_string(), sq_core::codec::XSD_DECIMAL_IRI, None);
            assert_eq!(inline_id_of_value(&Value::Decimal(d)), expect, "{s}");
        }
    }

    #[test]
    fn computed_inline_ids() {
        assert_eq!(inline_id_of_value(&Value::Integer(2)), TermId::from_int(2));
        assert_eq!(inline_id_of_value(&Value::Integer(i64::MAX)), None);
        assert_eq!(inline_id_of_value(&Value::Str("1234567".into())), codec::encode_short_str("1234567"));
        assert_eq!(inline_id_of_value(&Value::Str("12345678".into())), None);
        let dt = DateTime::from_str("2010-06-21T11:28:01Z").unwrap();
        assert_eq!(
            inline_id_of_value(&Value::DateTime(dt)),
            codec::encode_inline_literal("2010-06-21T11:28:01Z", XSD_DATETIME_IRI, None)
        );
        assert_eq!(inline_id_of_value(&Value::Double(1.0.into())), None);
    }

    #[test]
    fn canonical_terms() {
        assert_eq!(Value::Double(Double::from(1.5)).to_term(), Term::Literal(Literal::typed("1.5", "http://www.w3.org/2001/XMLSchema#double")));
        assert_eq!(Value::Decimal(Decimal::from(2)).lexical(), "2");
        assert_eq!(Value::Double(Double::from(f64::NEG_INFINITY)).lexical(), "-INF");
        assert_eq!(Value::Str("a".into()).to_term(), Term::Literal(Literal::simple("a")));
        assert_eq!(
            Value::LangStr { lex: "a".into(), lang: "en".into() }.to_term(),
            Term::Literal(Literal::lang("a", "en"))
        );
    }
}
