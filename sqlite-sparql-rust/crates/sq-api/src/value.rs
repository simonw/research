//! Term ↔ SQL value conversions shared by the functions and virtual tables.
//!
//! * [`native`]: the §8.4 native value mapping (`v*` columns, `sparql_value`,
//!   `sparql_term_value`, `sparql_view`). It delegates to `sq_results::sql_native` and only
//!   short-cuts the common text cases (IRIs, simple and language-tagged literals) so that they
//!   are handed to SQLite borrowed, without a per-cell allocation.
//! * [`write_ntriples`]: N-Triples text (`t*` / `__term` columns, `sparql_term`) appended to a
//!   reusable buffer; byte-for-byte `Term::to_ntriples`.
//! * [`public_id`]: the `id*` / `__id` column value (NULL for unbound and execution-local ids).
//! * [`parse_term`]: one N-Triples term (`sparql_term_id`, `__term` constraints).
use std::borrow::Cow;
use std::fmt::Write as _;
use std::str::FromStr;

use rusqlite::types::{ToSql, ToSqlOutput, Value, ValueRef};
use sq_core::codec::XSD_STRING_IRI;
use sq_core::{Error, Result, Term, TermId};
use sq_results::SqlNative;

/// A native SQL value that may borrow from the term it maps.
#[derive(Debug, PartialEq)]
pub(crate) enum Native<'a> {
    Null,
    Int(i64),
    Real(f64),
    Text(Cow<'a, str>),
}

impl ToSql for Native<'_> {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(match self {
            Native::Null => ToSqlOutput::Owned(Value::Null),
            Native::Int(i) => ToSqlOutput::Owned(Value::Integer(*i)),
            // SQLite stores a NaN REAL as NULL.
            Native::Real(f) if f.is_nan() => ToSqlOutput::Owned(Value::Null),
            Native::Real(f) => ToSqlOutput::Owned(Value::Real(*f)),
            Native::Text(s) => ToSqlOutput::Borrowed(ValueRef::Text(s.as_bytes())),
        })
    }
}

impl Native<'_> {
    /// The owned SQL value (scalar function results).
    pub(crate) fn into_value(self) -> Value {
        match self {
            Native::Null => Value::Null,
            Native::Int(i) => Value::Integer(i),
            Native::Real(f) if f.is_nan() => Value::Null,
            Native::Real(f) => Value::Real(f),
            Native::Text(s) => Value::Text(s.into_owned()),
        }
    }
}

/// §8.4 mapping of a term (None = unbound).
pub(crate) fn native(t: Option<&Term>) -> Native<'_> {
    match t {
        None => Native::Null,
        Some(Term::Iri(iri)) => Native::Text(Cow::Borrowed(iri)),
        Some(Term::Literal(l)) if l.lang.is_some() || &*l.datatype == XSD_STRING_IRI => {
            Native::Text(Cow::Borrowed(&l.lex))
        }
        Some(t) => match sq_results::sql_native(Some(t)) {
            SqlNative::Null => Native::Null,
            SqlNative::Int(i) => Native::Int(i),
            SqlNative::Real(f) => Native::Real(f),
            SqlNative::Text(s) => Native::Text(Cow::Owned(s)),
        },
    }
}

/// Appends the N-Triples form of `t` to `out` (identical to `Term::to_ntriples`).
pub(crate) fn write_ntriples(t: &Term, out: &mut String) {
    // Writing into a String cannot fail.
    let _ = match t {
        Term::Iri(iri) => write!(out, "{}", oxrdf::NamedNodeRef::new_unchecked(iri)),
        Term::BNode(label) => write!(out, "{}", oxrdf::BlankNodeRef::new_unchecked(label)),
        Term::Literal(l) => {
            let lit = match &l.lang {
                Some(lang) => oxrdf::LiteralRef::new_language_tagged_literal_unchecked(&l.lex, lang),
                None if &*l.datatype == XSD_STRING_IRI => oxrdf::LiteralRef::new_simple_literal(&l.lex),
                None => oxrdf::LiteralRef::new_typed_literal(&l.lex, oxrdf::NamedNodeRef::new_unchecked(&l.datatype)),
            };
            write!(out, "{lit}")
        }
    };
}

/// The value of an `id*` / `__id` column: storable ids only (UNBOUND, TEMP and the ERROR
/// sentinel are execution-local and map to NULL).
#[inline]
pub(crate) fn public_id(id: TermId) -> Option<i64> {
    id.is_storable().then_some(id.raw())
}

/// Parses exactly one N-Triples term (`<iri>`, `_:label`, `"lex"`, `"lex"@lang`,
/// `"lex"^^<datatype>`).
pub(crate) fn parse_term(text: &str) -> Result<oxrdf::Term> {
    oxrdf::Term::from_str(text.trim()).map_err(|e| Error::Invalid(format!("not an N-Triples term: {text:?} ({e})")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sq_core::Literal;

    fn terms() -> Vec<Term> {
        vec![
            Term::Iri("http://ex/a".into()),
            Term::BNode("b1f".into()),
            Term::Literal(Literal::simple("plain \"quoted\"\n\ttab\\ \u{7} é")),
            Term::Literal(Literal::lang("chat", "fr")),
            Term::Literal(Literal::typed("42", "http://www.w3.org/2001/XMLSchema#integer")),
            Term::Literal(Literal::typed("x", "http://ex/dt")),
        ]
    }

    #[test]
    fn ntriples_matches_term_to_ntriples() {
        let mut buf = String::new();
        for t in terms() {
            buf.clear();
            write_ntriples(&t, &mut buf);
            assert_eq!(buf, t.to_ntriples());
            let parsed = parse_term(&buf).unwrap();
            assert_eq!(Term::from_oxrdf(parsed.as_ref()), t);
        }
    }

    #[test]
    fn native_matches_sql_native() {
        for t in terms().into_iter().chain([
            Term::Literal(Literal::typed("2.5", "http://www.w3.org/2001/XMLSchema#decimal")),
            Term::Literal(Literal::typed("true", "http://www.w3.org/2001/XMLSchema#boolean")),
            Term::Literal(Literal::typed("NaN", "http://www.w3.org/2001/XMLSchema#double")),
            Term::Literal(Literal::typed("2020-01-01", "http://www.w3.org/2001/XMLSchema#date")),
            Term::Literal(Literal::typed("abc", "http://www.w3.org/2001/XMLSchema#string")),
        ]) {
            let expected = match sq_results::sql_native(Some(&t)) {
                SqlNative::Null => Native::Null,
                SqlNative::Int(i) => Native::Int(i),
                SqlNative::Real(f) if f.is_nan() => {
                    assert!(matches!(native(Some(&t)), Native::Real(g) if g.is_nan()));
                    continue;
                }
                SqlNative::Real(f) => Native::Real(f),
                SqlNative::Text(s) => Native::Text(Cow::Owned(s)),
            };
            assert_eq!(native(Some(&t)), expected, "{t:?}");
        }
        assert_eq!(native(None), Native::Null);
        assert!(matches!(native(Some(&Term::Iri("x".into()))), Native::Text(Cow::Borrowed("x"))));
    }

    #[test]
    fn public_ids() {
        assert_eq!(public_id(TermId::UNBOUND), None);
        assert_eq!(public_id(TermId::ERROR), None);
        assert_eq!(public_id(TermId::temp(3, sq_core::Tag::Iri)), None);
        assert_eq!(public_id(TermId::from_int(5).unwrap()), Some(TermId::from_int(5).unwrap().raw()));
        assert!(parse_term("not a term").is_err());
        assert!(parse_term("<a> <b>").is_err());
    }
}
