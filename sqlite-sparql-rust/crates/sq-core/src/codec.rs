//! Inline value codec, dictionary keys, stable term hash, well-known dictionary rows.
//!
//! The rules here are part of the ON-DISK FORMAT (`rdf_meta.encoding_version = 1`).
//! `encode_inline` is a pure function of (lexical form, datatype, language): a term is inlined
//! iff our formatter reproduces its lexical form EXACTLY (`format(decode(x)) == lexical`).
//! Non-canonical forms ("01", "+1", "1.50", "2020-01-01T00:00:00.000Z", "-00:00") keep their
//! own identity as dictionary TYPED rows, so sameTerm stays exact and lexical forms round-trip.
use std::sync::Arc;

use crate::id::{Tag, TermId};
use crate::term::{Literal, Term};

pub const ENCODING_VERSION: i64 = 1;
pub const SCHEMA_VERSION: i64 = 1;

pub const XSD: &str = "http://www.w3.org/2001/XMLSchema#";
pub const XSD_STRING_IRI: &str = "http://www.w3.org/2001/XMLSchema#string";
pub const XSD_INTEGER_IRI: &str = "http://www.w3.org/2001/XMLSchema#integer";
pub const XSD_DECIMAL_IRI: &str = "http://www.w3.org/2001/XMLSchema#decimal";
pub const XSD_BOOLEAN_IRI: &str = "http://www.w3.org/2001/XMLSchema#boolean";
pub const XSD_DATETIME_IRI: &str = "http://www.w3.org/2001/XMLSchema#dateTime";
pub const XSD_DATE_IRI: &str = "http://www.w3.org/2001/XMLSchema#date";
pub const RDF_LANG_STRING_IRI: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString";

/// Dictionary rows 1..=63 are reserved for these IRIs; `sparql_init()` (and MemStore::new)
/// insert them at exactly these rows. User terms start at `FIRST_USER_ROW`.
pub const WELL_KNOWN: &[(i64, &str)] = &[
    (1, "http://www.w3.org/2001/XMLSchema#string"),
    (2, "http://www.w3.org/2001/XMLSchema#integer"),
    (3, "http://www.w3.org/2001/XMLSchema#decimal"),
    (4, "http://www.w3.org/2001/XMLSchema#double"),
    (5, "http://www.w3.org/2001/XMLSchema#float"),
    (6, "http://www.w3.org/2001/XMLSchema#boolean"),
    (7, "http://www.w3.org/2001/XMLSchema#dateTime"),
    (8, "http://www.w3.org/2001/XMLSchema#date"),
    (9, "http://www.w3.org/2001/XMLSchema#time"),
    (10, "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString"),
    (11, "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
    (12, "http://www.w3.org/1999/02/22-rdf-syntax-ns#first"),
    (13, "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest"),
    (14, "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil"),
    (15, "http://www.w3.org/2001/XMLSchema#duration"),
    (16, "http://www.w3.org/2001/XMLSchema#dayTimeDuration"),
    (17, "http://www.w3.org/2001/XMLSchema#yearMonthDuration"),
    (18, "http://www.w3.org/2001/XMLSchema#long"),
    (19, "http://www.w3.org/2001/XMLSchema#int"),
    (20, "http://www.w3.org/2001/XMLSchema#short"),
    (21, "http://www.w3.org/2001/XMLSchema#byte"),
    (22, "http://www.w3.org/2001/XMLSchema#nonNegativeInteger"),
    (23, "http://www.w3.org/2001/XMLSchema#positiveInteger"),
    (24, "http://www.w3.org/2001/XMLSchema#nonPositiveInteger"),
    (25, "http://www.w3.org/2001/XMLSchema#negativeInteger"),
    (26, "http://www.w3.org/2001/XMLSchema#unsignedLong"),
    (27, "http://www.w3.org/2001/XMLSchema#unsignedInt"),
    (28, "http://www.w3.org/2001/XMLSchema#unsignedShort"),
    (29, "http://www.w3.org/2001/XMLSchema#unsignedByte"),
    (30, "http://www.w3.org/2001/XMLSchema#gYear"),
    (31, "http://www.w3.org/2001/XMLSchema#gYearMonth"),
    (32, "http://www.w3.org/2001/XMLSchema#gMonth"),
    (33, "http://www.w3.org/2001/XMLSchema#gMonthDay"),
    (34, "http://www.w3.org/2001/XMLSchema#gDay"),
    (35, "http://www.w3.org/2000/01/rdf-schema#subClassOf"),
];
pub const FIRST_USER_ROW: i64 = 1024;

pub const XSD_STRING: TermId = TermId::from_dict(1, Tag::Iri);
pub const XSD_INTEGER: TermId = TermId::from_dict(2, Tag::Iri);
pub const XSD_DECIMAL: TermId = TermId::from_dict(3, Tag::Iri);
pub const XSD_DOUBLE: TermId = TermId::from_dict(4, Tag::Iri);
pub const XSD_FLOAT: TermId = TermId::from_dict(5, Tag::Iri);
pub const XSD_BOOLEAN: TermId = TermId::from_dict(6, Tag::Iri);
pub const XSD_DATETIME: TermId = TermId::from_dict(7, Tag::Iri);
pub const XSD_DATE: TermId = TermId::from_dict(8, Tag::Iri);
pub const RDF_LANG_STRING: TermId = TermId::from_dict(10, Tag::Iri);
pub const RDF_TYPE: TermId = TermId::from_dict(11, Tag::Iri);

/// Datatype IRI id of an inline literal (constant, no lookup).
pub const fn inline_datatype(id: TermId) -> Option<TermId> {
    match id.tag() {
        Tag::Int => Some(XSD_INTEGER),
        Tag::Bool => Some(XSD_BOOLEAN),
        Tag::Decimal => Some(XSD_DECIMAL),
        Tag::DateTime => Some(XSD_DATETIME),
        Tag::Date => Some(XSD_DATE),
        Tag::ShortStr => Some(XSD_STRING),
        _ => None,
    }
}

// ---------------------------------------------------------------- inline encoding

/// Canonical id of an inline-encodable term, or None (term needs the dictionary / is a bnode).
pub fn encode_inline(t: oxrdf::TermRef<'_>) -> Option<TermId> {
    match t {
        oxrdf::TermRef::Literal(l) => encode_inline_literal(l.value(), l.datatype().as_str(), l.language()),
        _ => None,
    }
}

pub fn encode_inline_literal(lex: &str, datatype: &str, lang: Option<&str>) -> Option<TermId> {
    if lang.is_some() {
        return None;
    }
    let local = datatype.strip_prefix(XSD)?;
    match local {
        "integer" => TermId::from_int(parse_canonical_integer(lex)?),
        "boolean" => match lex {
            "true" => Some(TermId::from_bool(true)),
            "false" => Some(TermId::from_bool(false)),
            _ => None,
        },
        "decimal" => {
            let (m, scale, dotzero) = parse_canonical_decimal(lex)?;
            Some(encode_decimal(m, scale, dotzero))
        }
        "dateTime" => {
            let (ms, tz) = parse_canonical_datetime(lex)?;
            Some(TermId(((ms << 16) as u64 | ((tz as u64) << 4) | Tag::DateTime as u64) as i64))
        }
        "date" => {
            let (days, tz) = parse_canonical_date(lex)?;
            Some(TermId((((days as i64) << 16) as u64 | ((tz as u64) << 4) | Tag::Date as u64) as i64))
        }
        "string" => encode_short_str(lex),
        _ => None,
    }
}

/// Inverse of `encode_inline` for tags 6..=11.
pub fn decode_inline(id: TermId) -> Option<Term> {
    let (lex, dt): (String, &str) = match id.tag() {
        Tag::Int => ((id.0 >> 4).to_string(), XSD_INTEGER_IRI),
        Tag::Bool => ((if id.0 >> 4 != 0 { "true" } else { "false" }).to_string(), XSD_BOOLEAN_IRI),
        Tag::Decimal => {
            let (m, scale, dotzero) = decimal_parts(id);
            (format_decimal(m, scale, dotzero), XSD_DECIMAL_IRI)
        }
        Tag::DateTime => {
            let (ms, tz) = datetime_parts(id);
            (format_datetime(ms, tz), XSD_DATETIME_IRI)
        }
        Tag::Date => {
            let (days, tz) = date_parts(id);
            (format_date(days, tz), XSD_DATE_IRI)
        }
        Tag::ShortStr => (decode_short_str(id)?, XSD_STRING_IRI),
        _ => return None,
    };
    Some(Term::Literal(Literal { lex: Arc::from(lex), datatype: Arc::from(dt), lang: None }))
}

/// `-?(0|[1-9][0-9]*)`, not "-0", fits i60.
pub fn parse_canonical_integer(lex: &str) -> Option<i64> {
    let digits = lex.strip_prefix('-').unwrap_or(lex);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if digits.len() > 1 && digits.starts_with('0') {
        return None;
    }
    if lex == "-0" {
        return None;
    }
    let v: i64 = lex.parse().ok()?;
    (TermId::INT_MIN..=TermId::INT_MAX).contains(&v).then_some(v)
}

// DECIMAL: id = (m << 9) | (dotzero << 8) | (scale << 4) | 8,  m: i55, scale: 0..=15.
// Accepted lexical forms (and nothing else):
//   integral without point   "0", "-12"            scale 0, dotzero 0   (not "-0")
//   integral with ".0"       "0.0", "5.0", "-5.0"  scale 0, dotzero 1   (not "-0.0")
//   fractional               "0.5", "-12.25"       scale = #fraction digits (1..=15),
//                                                  last fraction digit != '0'
// Integer part never has a leading zero (except "0") and there is never a '+'.
const DEC_M_MIN: i64 = -(1 << 54);
const DEC_M_MAX: i64 = (1 << 54) - 1;

pub fn parse_canonical_decimal(lex: &str) -> Option<(i64, u8, bool)> {
    let (neg, body) = match lex.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, lex),
    };
    let (int_part, frac) = match body.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (body, None),
    };
    if int_part.is_empty() || !int_part.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if int_part.len() > 1 && int_part.starts_with('0') {
        return None;
    }
    let ip: i64 = int_part.parse().ok()?;
    let (mut m, scale, dotzero) = match frac {
        None => (ip, 0u8, false),
        Some("0") => (ip, 0u8, true),
        Some(f) => {
            if f.is_empty() || f.len() > 15 || !f.bytes().all(|b| b.is_ascii_digit()) || f.ends_with('0') {
                return None;
            }
            let fv: i64 = f.parse().ok()?;
            let m = ip.checked_mul(10i64.checked_pow(f.len() as u32)?)?.checked_add(fv)?;
            (m, f.len() as u8, false)
        }
    };
    if neg {
        if m == 0 {
            return None; // "-0", "-0.0"
        }
        m = -m;
    }
    (DEC_M_MIN..=DEC_M_MAX).contains(&m).then_some((m, scale, dotzero))
}

pub fn encode_decimal(m: i64, scale: u8, dotzero: bool) -> TermId {
    debug_assert!((DEC_M_MIN..=DEC_M_MAX).contains(&m) && scale <= 15 && !(dotzero && scale != 0));
    TermId((m << 9) | ((dotzero as i64) << 8) | ((scale as i64) << 4) | Tag::Decimal as i64)
}

pub fn decimal_parts(id: TermId) -> (i64, u8, bool) {
    (id.0 >> 9, ((id.0 >> 4) & 0xF) as u8, (id.0 >> 8) & 1 == 1)
}

pub fn format_decimal(m: i64, scale: u8, dotzero: bool) -> String {
    if scale == 0 {
        return if dotzero { format!("{m}.0") } else { m.to_string() };
    }
    let s = scale as usize;
    let mut digits = m.unsigned_abs().to_string();
    while digits.len() < s + 1 {
        digits.insert(0, '0');
    }
    digits.insert(digits.len() - s, '.');
    if m < 0 {
        digits.insert(0, '-');
    }
    digits
}

// DATETIME: id = (ms << 16) | (tz << 4) | 9, ms: i48 = wall-clock milliseconds since
// 1970-01-01T00:00:00 of the LEXICAL fields (NOT shifted by the offset), tz code (12 bits):
//   0 = no timezone, 1 = "Z", 2..=1682 = explicit offset (minutes + 842), e.g. "+00:00" = 842.
// Canonical lexical: YYYY-MM-DDThh:mm:ss[.f{1,3} without trailing zero][Z|(+|-)hh:mm]
//   year 0001..=6000, hour 00..=23, no leap seconds, |offset| <= 14:00, "-00:00" not inlined.
// DATE: id = (days << 16) | (tz << 4) | 10, days since 1970-01-01, lexical YYYY-MM-DD[tz].

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    // Howard Hinnant's algorithm.
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        _ => 28,
    }
}

fn num(s: &str) -> Option<u32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Parses "YYYY-MM-DD" (exactly 10 bytes) -> days.
fn parse_ymd(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let y = num(&s[0..4])? as i64;
    let m = num(&s[5..7])?;
    let d = num(&s[8..10])?;
    if !(1..=6000).contains(&y) || !(1..=12).contains(&m) || d == 0 || d > days_in_month(y, m) {
        return None;
    }
    Some(days_from_civil(y, m, d))
}

/// Parses an optional timezone suffix -> tz code.
fn parse_tz(s: &str) -> Option<u16> {
    match s {
        "" => Some(0),
        "Z" => Some(1),
        "-00:00" => None,
        _ => {
            let b = s.as_bytes();
            if b.len() != 6 || (b[0] != b'+' && b[0] != b'-') || b[3] != b':' {
                return None;
            }
            let h = num(&s[1..3])? as i32;
            let mi = num(&s[4..6])? as i32;
            if h > 14 || mi > 59 || (h == 14 && mi != 0) {
                return None;
            }
            let off = (h * 60 + mi) * if b[0] == b'-' { -1 } else { 1 };
            Some((off + 842) as u16)
        }
    }
}

fn format_tz(tz: u16) -> String {
    match tz {
        0 => String::new(),
        1 => "Z".to_string(),
        c => {
            let off = c as i32 - 842;
            let sign = if off < 0 { '-' } else { '+' };
            format!("{sign}{:02}:{:02}", off.abs() / 60, off.abs() % 60)
        }
    }
}

fn split_tz(s: &str) -> (&str, &str) {
    if let Some(body) = s.strip_suffix('Z') {
        return (body, "Z");
    }
    if s.len() >= 6 {
        let cut = s.len() - 6;
        let b = s.as_bytes()[cut];
        if (b == b'+' || b == b'-') && s.as_bytes()[s.len() - 3] == b':' {
            return (&s[..cut], &s[cut..]);
        }
    }
    (s, "")
}

const MS_MIN: i64 = -(1 << 47);
const MS_MAX: i64 = (1 << 47) - 1;

pub fn parse_canonical_datetime(lex: &str) -> Option<(i64, u16)> {
    let (body, tzs) = split_tz(lex);
    let tz = parse_tz(tzs)?;
    let (date, time) = body.split_once('T')?;
    let days = parse_ymd(date)?;
    let (hms, frac) = match time.split_once('.') {
        Some((h, f)) => (h, Some(f)),
        None => (time, None),
    };
    let b = hms.as_bytes();
    if b.len() != 8 || b[2] != b':' || b[5] != b':' {
        return None;
    }
    let (h, mi, s) = (num(&hms[0..2])?, num(&hms[3..5])?, num(&hms[6..8])?);
    if h > 23 || mi > 59 || s > 59 {
        return None;
    }
    let ms_frac = match frac {
        None => 0,
        Some(f) => {
            if f.is_empty() || f.len() > 3 || f.ends_with('0') {
                return None;
            }
            num(f)? * 10u32.pow(3 - f.len() as u32)
        }
    };
    let ms = days * 86_400_000 + (h as i64 * 3600 + mi as i64 * 60 + s as i64) * 1000 + ms_frac as i64;
    let out = (ms, tz);
    ((MS_MIN..=MS_MAX).contains(&ms) && format_datetime(ms, tz) == lex).then_some(out)
}

pub fn datetime_parts(id: TermId) -> (i64, u16) {
    (id.0 >> 16, ((id.0 >> 4) & 0xFFF) as u16)
}

pub fn format_datetime(ms: i64, tz: u16) -> String {
    let days = ms.div_euclid(86_400_000);
    let rem = ms.rem_euclid(86_400_000);
    let (y, m, d) = civil_from_days(days);
    let (secs, milli) = (rem / 1000, rem % 1000);
    let mut s = format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}", secs / 3600, (secs / 60) % 60, secs % 60);
    if milli != 0 {
        let f = format!("{milli:03}");
        s.push('.');
        s.push_str(f.trim_end_matches('0'));
    }
    s.push_str(&format_tz(tz));
    s
}

pub fn parse_canonical_date(lex: &str) -> Option<(i32, u16)> {
    let (body, tzs) = split_tz(lex);
    let tz = parse_tz(tzs)?;
    let days = parse_ymd(body)?;
    (format_date(days as i32, tz) == lex).then_some((days as i32, tz))
}

pub fn date_parts(id: TermId) -> (i32, u16) {
    ((id.0 >> 16) as i32, ((id.0 >> 4) & 0xFFF) as u16)
}

pub fn format_date(days: i32, tz: u16) -> String {
    let (y, m, d) = civil_from_days(days as i64);
    format!("{y:04}-{m:02}-{d:02}{}", format_tz(tz))
}

// SHORTSTR: id = (be7 << 8) | (len << 4) | 11 computed in u64, where be7 packs the (<= 7)
// UTF-8 bytes big-endian into 56 bits, zero padded; len = 0..=7.
pub fn encode_short_str(s: &str) -> Option<TermId> {
    let b = s.as_bytes();
    if b.len() > 7 {
        return None;
    }
    let mut be7: u64 = 0;
    for i in 0..7 {
        be7 = (be7 << 8) | *b.get(i).unwrap_or(&0) as u64;
    }
    Some(TermId(((be7 << 8) | ((b.len() as u64) << 4) | Tag::ShortStr as u64) as i64))
}

pub fn decode_short_str(id: TermId) -> Option<String> {
    let u = id.0 as u64;
    let len = ((u >> 4) & 0xF) as usize;
    if len > 7 {
        return None;
    }
    let be7 = u >> 8;
    let bytes: Vec<u8> = (0..len).map(|i| ((be7 >> (8 * (6 - i))) & 0xFF) as u8).collect();
    String::from_utf8(bytes).ok()
}

// ---------------------------------------------------------------- dictionary keys & hash

/// Dictionary tag of a non-inline term (None for blank nodes: they never get rows).
pub fn dict_tag(t: oxrdf::TermRef<'_>) -> Option<Tag> {
    match t {
        oxrdf::TermRef::NamedNode(_) => Some(Tag::Iri),
        oxrdf::TermRef::Literal(l) => Some(if l.language().is_some() {
            Tag::Lang
        } else if l.datatype().as_str() == XSD_STRING_IRI {
            Tag::Str
        } else {
            Tag::Typed
        }),
        _ => None,
    }
}

/// Exact dictionary identity: (tag, lex, extra) with extra = datatype IRI (Typed),
/// lower-case language (Lang), "" otherwise.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct TermKey {
    pub tag: Tag,
    pub lex: Box<str>,
    pub extra: Box<str>,
}

impl TermKey {
    /// None for blank nodes and inline-encodable terms.
    pub fn of(t: oxrdf::TermRef<'_>) -> Option<TermKey> {
        if encode_inline(t).is_some() {
            return None;
        }
        let tag = dict_tag(t)?;
        Some(match t {
            oxrdf::TermRef::NamedNode(n) => TermKey { tag, lex: n.as_str().into(), extra: "".into() },
            oxrdf::TermRef::Literal(l) => TermKey {
                tag,
                lex: l.value().into(),
                extra: match tag {
                    Tag::Lang => l.language().unwrap_or("").to_ascii_lowercase().into(),
                    Tag::Typed => l.datatype().as_str().into(),
                    _ => "".into(),
                },
            },
            _ => return None,
        })
    }
    pub fn hash64(&self) -> i64 {
        term_hash(self.tag, &self.lex, &self.extra)
    }
}

pub const TERM_HASH_SEED: u64 = 0x5350_4152_514C;

/// FROZEN (on-disk `rdf_term.h`): xxh3_64_with_seed([tag] ++ lex ++ [0x00] ++ extra, SEED) as i64.
pub fn term_hash(tag: Tag, lex: &str, extra: &str) -> i64 {
    let mut buf = Vec::with_capacity(lex.len() + extra.len() + 2);
    buf.push(tag as u8);
    buf.extend_from_slice(lex.as_bytes());
    buf.push(0);
    buf.extend_from_slice(extra.as_bytes());
    xxhash_rust::xxh3::xxh3_64_with_seed(&buf, TERM_HASH_SEED) as i64
}

/// Blank-node label of a store bnode id: "b<lower-hex counter>".
pub fn bnode_label(id: TermId) -> String {
    format!("b{:x}", id.bnode_counter())
}

/// Inverse of `bnode_label` (used by `sparql_id('_:b1f')`).
pub fn parse_bnode_label(label: &str) -> Option<TermId> {
    let hex = label.strip_prefix('b')?;
    let n = i64::from_str_radix(hex, 16).ok()?;
    let id = TermId::bnode(n);
    (n > 0 && n < (1 << 59) && bnode_label(id) == label).then_some(id)
}
