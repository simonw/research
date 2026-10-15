//! ORDER BY / MIN / MAX keys (§6.5): a lexicographic tuple encoded as memcmp-ordered bytes,
//! so the order is total by construction (`sort_by` can never panic) and the same bytes serve
//! `sparql_sortkey(id)`.
//!
//! ```text
//! (class, primary, lex, datatype, lang)
//! class:  0 unbound/error < 1 bnode < 2 IRI < 3 literal
//! literal primary (first byte = family):
//!   0 numeric   (nan_flag: NaN first, f64 in total order, rank {integer|decimal 0, float 1,
//!                double 2}, exact decimal i128 (×10^18, 0 for float/double))
//!   1 boolean   2 simple/xsd:string (primary = the lex field)   3 lang string (lex, lang)
//!   4 dateTime  5 date  6 time  7 gYearMonth  8 gYear  9 gMonthDay  10 gMonth  11 gDay
//!               (UTC instant as Decimal seconds; no timezone keyed as UTC)
//!   12 duration family (months, seconds)
//!   13 Other / IllTyped (no primary)
//! ```
//! Within a family the order agrees with `<` wherever `<` is defined: integer/decimal → f64
//! conversions are correctly rounded (monotone), float/double comparisons promote exactly as
//! the `<` operator does, and ties on the f64 are broken by the exact decimal value.
//! Strings are escaped (0x00 → 00 FF) and terminated by 00 00; the datatype tie-breaker is one
//! code byte for the XSD types and rdf:langString, else the escaped IRI.
use sq_core::codec::{self, RDF_LANG_STRING_IRI};
use sq_core::{Kind, Result, Tag, TermId};

use crate::EvalCtx;
use crate::numeric::{DEC_SCALE, Num, dec_raw};
use crate::value::{Val, Value, is_present, term_of_id, value_of};
use crate::xsd::XsdType;

/// Lexicographic, total ORDER BY key (§6.5). Bytes compare with memcmp order.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct SortKey(Vec<u8>);

const CLASS_UNBOUND: u8 = 0;
const CLASS_BNODE: u8 = 1;
const CLASS_IRI: u8 = 2;
const CLASS_LITERAL: u8 = 3;

impl SortKey {
    /// The key of UNBOUND and of errors: lower than every term.
    pub fn lowest() -> SortKey {
        SortKey(vec![CLASS_UNBOUND])
    }

    /// The memcmp-ordered encoding (the `sparql_sortkey` BLOB).
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }

    pub fn is_lowest(&self) -> bool {
        self.0 == [CLASS_UNBOUND]
    }
}

/// Appends an escaped, terminated string field.
fn push_str(k: &mut Vec<u8>, s: &[u8]) {
    for &b in s {
        if b == 0 {
            k.extend_from_slice(&[0, 0xFF]);
        } else {
            k.push(b);
        }
    }
    k.extend_from_slice(&[0, 0]);
}

fn push_i128(k: &mut Vec<u8>, x: i128) {
    k.extend_from_slice(&((x as u128) ^ (1 << 127)).to_be_bytes());
}

fn push_i64(k: &mut Vec<u8>, x: i64) {
    k.extend_from_slice(&((x as u64) ^ (1 << 63)).to_be_bytes());
}

/// f64 in `total_cmp` order.
fn push_f64(k: &mut Vec<u8>, x: f64) {
    let bits = x.to_bits();
    let ordered = if bits >> 63 == 1 { !bits } else { bits ^ (1 << 63) };
    k.extend_from_slice(&ordered.to_be_bytes());
}

/// Raw timeline instant of a date/time value (oxsdatatypes' 18-byte layout, first 16 bytes).
fn push_instant(k: &mut Vec<u8>, bytes: [u8; 18]) {
    let mut raw = [0u8; 16];
    raw.copy_from_slice(&bytes[..16]);
    push_i128(k, i128::from_be_bytes(raw));
}

/// Family byte and primary fields of a literal value.
fn push_primary(k: &mut Vec<u8>, v: &Value) {
    match v {
        Value::Integer(_) | Value::Decimal(_) | Value::Float(_) | Value::Double(_) => {
            let n = v.as_num().expect("numeric");
            let f = n.to_f64();
            k.push(0);
            k.push(if f.is_nan() { 0 } else { 1 });
            push_f64(k, f);
            let (rank, exact) = match n {
                Num::Integer(i) => (0, i as i128 * DEC_SCALE),
                Num::Decimal(d) => (0, dec_raw(d)),
                Num::Float(_) => (1, 0),
                Num::Double(_) => (2, 0),
            };
            k.push(rank);
            push_i128(k, exact);
        }
        Value::Boolean(b) => k.extend_from_slice(&[1, *b as u8]),
        Value::Str(_) => k.push(2),
        Value::LangStr { .. } => k.push(3),
        Value::DateTime(x) => {
            k.push(4);
            push_instant(k, x.to_be_bytes());
        }
        Value::Date(x) => {
            k.push(5);
            push_instant(k, x.to_be_bytes());
        }
        Value::Time(x) => {
            k.push(6);
            push_instant(k, x.to_be_bytes());
        }
        Value::GYearMonth(x) => {
            k.push(7);
            push_instant(k, x.to_be_bytes());
        }
        Value::GYear(x) => {
            k.push(8);
            push_instant(k, x.to_be_bytes());
        }
        Value::GMonthDay(x) => {
            k.push(9);
            push_instant(k, x.to_be_bytes());
        }
        Value::GMonth(x) => {
            k.push(10);
            push_instant(k, x.to_be_bytes());
        }
        Value::GDay(x) => {
            k.push(11);
            push_instant(k, x.to_be_bytes());
        }
        Value::Duration(_) | Value::YearMonthDuration(_) | Value::DayTimeDuration(_) => {
            let d = crate::compare::as_duration(v).expect("duration");
            let b = d.to_be_bytes();
            k.push(12);
            push_i64(k, i64::from_be_bytes(b[..8].try_into().expect("8 bytes")));
            push_i128(k, i128::from_be_bytes(b[8..].try_into().expect("16 bytes")));
        }
        Value::Other { .. } | Value::IllTyped { .. } => k.push(13),
        Value::Iri(_) | Value::BNode(_) => unreachable!("not a literal"),
    }
}

/// Datatype tie-breaker: one byte for the XSD types and rdf:langString, else the IRI.
fn push_datatype(k: &mut Vec<u8>, iri: &str) {
    match XsdType::from_iri(iri) {
        Some(t) => k.extend_from_slice(&[1, t as u8]),
        None if iri == RDF_LANG_STRING_IRI => k.extend_from_slice(&[1, 0xFF]),
        None => {
            k.push(2);
            push_str(k, iri.as_bytes());
        }
    }
}

/// Tie-breaker fields: datatype and language.
fn push_datatype_and_lang(k: &mut Vec<u8>, v: &Value) {
    push_datatype(k, v.datatype_iri().unwrap_or(""));
    if let Value::LangStr { lang, .. } = v {
        push_str(k, lang.as_bytes());
    }
}

/// Key of a computed (canonical) value.
pub fn sort_key_value(v: &Value) -> SortKey {
    let mut k = Vec::with_capacity(64);
    match v {
        Value::BNode(label) => {
            k.push(CLASS_BNODE);
            push_str(&mut k, label.as_bytes());
        }
        Value::Iri(iri) => {
            k.push(CLASS_IRI);
            push_str(&mut k, iri.as_bytes());
        }
        _ => {
            k.push(CLASS_LITERAL);
            push_primary(&mut k, v);
            push_str(&mut k, v.lexical().as_bytes());
            push_datatype_and_lang(&mut k, v);
        }
    }
    SortKey(k)
}

/// Decimal digits of an integer (the canonical xsd:integer lexical form), without `fmt`.
fn push_i64_digits(k: &mut Vec<u8>, v: i64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    let mut n = v.unsigned_abs();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    if v < 0 {
        k.push(b'-');
    }
    k.extend_from_slice(&buf[i..]);
}

/// Writes the stored lexical form of an inline id (the codec's canonical form).
fn push_inline_lexical(k: &mut Vec<u8>, id: TermId) {
    match id.tag() {
        Tag::Int => {
            push_i64_digits(k, id.0 >> 4);
            k.extend_from_slice(&[0, 0]);
        }
        Tag::Bool => push_str(k, if id.0 >> 4 != 0 { b"true" } else { b"false" }),
        _ => {
            let lex = match codec::decode_inline(id) {
                Some(sq_core::Term::Literal(l)) => l.lex,
                _ => "".into(),
            };
            push_str(k, lex.as_bytes());
        }
    }
}

/// UNBOUND/ERROR -> lowest.
pub fn sort_key(id: TermId, cx: &mut EvalCtx) -> Result<SortKey> {
    if !is_present(id) {
        return Ok(SortKey::lowest());
    }
    let mut k = Vec::with_capacity(64);
    match id.kind() {
        Kind::Unbound => return Ok(SortKey::lowest()),
        Kind::BNode => {
            let t = term_of_id(id, cx)?;
            k.push(CLASS_BNODE);
            if let sq_core::Term::BNode(label) = &*t {
                push_str(&mut k, label.as_bytes());
            }
        }
        Kind::Iri => {
            let v = value_of(id, cx)?;
            k.push(CLASS_IRI);
            push_str(&mut k, v.lexical().as_bytes());
        }
        Kind::Literal => {
            let v = value_of(id, cx)?;
            k.push(CLASS_LITERAL);
            push_primary(&mut k, &v);
            match &v {
                _ if id.is_inline() => {
                    push_inline_lexical(&mut k, id);
                    push_datatype_and_lang(&mut k, &v);
                }
                Value::Str(lex) | Value::LangStr { lex, .. } | Value::Other { lex, .. } | Value::IllTyped { lex, .. } => {
                    push_str(&mut k, lex.as_bytes());
                    push_datatype_and_lang(&mut k, &v);
                }
                // Typed values keep their stored (possibly non-canonical) lexical form and
                // their exact datatype (e.g. xsd:int, which the value model folds into Integer).
                _ => {
                    if let sq_core::Term::Literal(l) = &*term_of_id(id, cx)? {
                        push_str(&mut k, l.lex.as_bytes());
                        push_datatype(&mut k, &l.datatype);
                    }
                }
            }
        }
    }
    Ok(SortKey(k))
}

/// Key of a lazy value. Computed values are keyed without being interned; a `Typed` literal
/// gets the key of its exact term (value, lexical form, datatype), as its id would.
pub fn sort_key_val(v: &Val, cx: &mut EvalCtx) -> Result<SortKey> {
    match v {
        Val::Id(id) => sort_key(*id, cx),
        Val::V(v) => Ok(sort_key_value(v)),
        Val::Typed { value, datatype, lex } => {
            let mut k = Vec::with_capacity(64);
            k.push(CLASS_LITERAL);
            push_primary(&mut k, value);
            push_str(&mut k, Val::typed_lexical(value, lex).as_bytes());
            push_datatype(&mut k, datatype.iri());
            Ok(SortKey(k))
        }
    }
}
