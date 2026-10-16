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
/// `"lex"^^<datatype>`; oxrdf also takes Turtle's bare numbers and `true` / `false`).
pub(crate) fn parse_term(text: &str) -> Result<oxrdf::Term> {
    let t = text.trim();
    oxrdf::Term::from_str(t).map_err(|e| {
        let why = term_mistake(t).unwrap_or_else(|| e.to_string());
        Error::Invalid(format!("not an N-Triples term: {text:?} ({why})"))
    })
}

/// A plain explanation for the usual mistakes: SPARQL / Turtle shorthand that N-Triples does
/// not have. oxrdf reads anything that does not start with `<`, `_` or `"` as a number, so
/// its own message for `ex:Python` is "Double serialization with an invalid exponent".
fn term_mistake(t: &str) -> Option<String> {
    let first = t.chars().next()?;
    if first == '"' {
        // `"5"^^xsd:integer`: the datatype must be a full IRI too.
        let (_, after) = t.rsplit_once('"')?;
        let dt = after.trim_start().strip_prefix("^^")?.trim_start();
        return (!dt.starts_with('<')).then(|| match expand_well_known(dt) {
            Some(iri) => format!(
                "the datatype {dt} must be a full IRI in angle brackets: ^^<{iri}>; prefixed names are not N-Triples"
            ),
            None => format!(
                "the datatype {dt} must be a full IRI in angle brackets, \
                 e.g. ^^<http://www.w3.org/2001/XMLSchema#integer>; prefixed names are not N-Triples"
            ),
        });
    }
    if first == '\'' {
        return Some("N-Triples strings use double quotes: \"…\"".into());
    }
    if !(first.is_alphabetic() || first == ':') {
        return None; // `<…>`, `_:…`, numbers: oxrdf's message is to the point.
    }
    // Classify the first word (`ex:Python .` is still a prefixed name).
    let word = t.split(char::is_whitespace).next().unwrap_or(t);
    let Some((prefix, local)) = word.split_once(':') else {
        return Some(
            "neither a literal nor an IRI: write a string in double quotes (\"…\") and an IRI in angle \
             brackets (<…>)"
                .into(),
        );
    };
    if local.starts_with("//") || ["urn", "mailto", "file", "tag", "data"].contains(&prefix) {
        return Some(format!("an IRI needs angle brackets: <{word}>"));
    }
    if prefix.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.')) {
        let how = match expand_well_known(word) {
            Some(iri) => format!("<{iri}>"),
            // The example must not look like the answer: the namespace is the user's own.
            None => format!(
                "the namespace IRI that {prefix}: stands for, followed by {local} \
                 (if {prefix}: is <http://example.org/>, that is <http://example.org/{local}>)"
            ),
        };
        return Some(format!(
            "{word} is a prefixed name, and N-Triples has none: no PREFIX applies outside a SPARQL query, so \
             write the full IRI in angle brackets: {how}"
        ));
    }
    None
}

/// The full IRI of a prefixed name in one of the standard vocabularies whose prefix everyone
/// uses the same way (`rdf:`, `rdfs:`, `xsd:`, `owl:`).
fn expand_well_known(name: &str) -> Option<String> {
    const PREFIXES: [(&str, &str); 4] = [
        ("rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#"),
        ("rdfs", "http://www.w3.org/2000/01/rdf-schema#"),
        ("xsd", "http://www.w3.org/2001/XMLSchema#"),
        ("owl", "http://www.w3.org/2002/07/owl#"),
    ];
    let (prefix, local) = name.split_once(':')?;
    let ns = PREFIXES.iter().find(|(p, _)| *p == prefix)?.1;
    Some(format!("{ns}{local}"))
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

    fn mistake(text: &str) -> String {
        match parse_term(text) {
            Err(Error::Invalid(m)) => m,
            other => panic!("{text}: {other:?}"),
        }
    }

    #[test]
    fn usual_mistakes_are_explained() {
        let m = mistake("ex:Python");
        assert!(m.contains("ex:Python is a prefixed name"), "{m}");
        assert!(m.contains("<http://example.org/Python>") && !m.contains("exponent"), "{m}");
        assert!(mistake(":x").contains("prefixed name"));
        assert!(mistake("schema:Person").contains("prefixed name"));
        let m = mistake("http://example.org/pl/Go");
        assert!(m.contains("an IRI needs angle brackets: <http://example.org/pl/Go>"), "{m}");
        assert!(mistake("urn:isbn:0451450523").contains("<urn:isbn:0451450523>"));
        let m = mistake("\"5\"^^xsd:integer");
        assert!(m.contains("the datatype xsd:integer must be a full IRI"), "{m}");
        assert!(m.contains("^^<http://www.w3.org/2001/XMLSchema#integer>"), "{m}");
        let m = mistake("\"5\"^^xsd:decimal");
        assert!(m.contains("^^<http://www.w3.org/2001/XMLSchema#decimal>"), "{m}");
        let m = mistake("\"5\"^^ex:dt");
        assert!(m.contains("the datatype ex:dt must be a full IRI") && m.contains("e.g."), "{m}");
        // Standard prefixes expand to their real IRIs; other prefixes get no copyable guess.
        let m = mistake("xsd:integer");
        assert!(m.contains(": <http://www.w3.org/2001/XMLSchema#integer>") && !m.contains("example.org"), "{m}");
        let m = mistake("rdf:type");
        assert!(m.contains("<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>"), "{m}");
        assert!(mistake("rdfs:label").contains("<http://www.w3.org/2000/01/rdf-schema#label>"));
        assert!(mistake("owl:Thing").contains("<http://www.w3.org/2002/07/owl#Thing>"));
        let m = mistake("schema:Person");
        assert!(m.contains("the namespace IRI that schema: stands for, followed by Person"), "{m}");
        assert!(mistake("'single'").contains("double quotes"));
        assert!(mistake("Python").contains("neither a literal nor an IRI"));
        assert!(mistake("ex:Python .").contains("ex:Python is a prefixed name"));
        // Errors inside well-delimited terms keep the parser's message.
        assert!(mistake("<http://ex/a").contains("not an N-Triples term: \"<http://ex/a\" ("));
        assert!(mistake("1.").contains("Decimal serialization"), "numbers keep oxrdf's message");
        // Accepted shorthand is unchanged.
        assert!(parse_term("42").is_ok() && parse_term("true").is_ok() && parse_term("4.2e1").is_ok());
        assert!(parse_term("\"5\"^^<http://www.w3.org/2001/XMLSchema#integer>").is_ok());
    }
}
