//! Native SQL value mapping (§8.4) for the `v*` columns of `sparql_rows`, `sparql_value`,
//! `sparql_term_value` and `sparql_view`.
//!
//! | Term | SQL value |
//! |---|---|
//! | IRI | TEXT without `<>` |
//! | blank node | `'_:label'` |
//! | simple, xsd:string, lang literal | TEXT (lexical) |
//! | integer family | INTEGER, or TEXT if outside i64 |
//! | decimal, float, double | REAL |
//! | boolean | 0/1 |
//! | date/time types | TEXT (lexical ISO 8601) |
//! | other typed | TEXT (lexical) |
//! | unbound | NULL |
//!
//! Ill-typed literals (a lexical form outside the datatype's lexical space, or an integer
//! outside a derived type's range, e.g. `"300"^^xsd:byte`) map to their lexical TEXT, like
//! any other typed literal. Lexical forms are matched exactly (no whitespace collapsing),
//! consistently with the expression evaluator. Floats are converted from their lexical form
//! at double precision, so `"0.1"^^xsd:float` maps to `0.1`. NaN maps to a REAL NaN, which
//! SQLite stores as NULL.
use sq_core::codec::XSD;
use sq_core::term::{Literal, Term};

/// A SQLite storage-class value.
#[derive(Clone, Debug, PartialEq)]
pub enum SqlNative {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
}

/// Maps a term (None = unbound) to its native SQL value (§8.4).
pub fn sql_native(t: Option<&Term>) -> SqlNative {
    match t {
        None => SqlNative::Null,
        Some(Term::Iri(iri)) => SqlNative::Text(iri.to_string()),
        Some(Term::BNode(label)) => SqlNative::Text(format!("_:{label}")),
        Some(Term::Literal(l)) => literal_native(l),
    }
}

fn literal_native(l: &Literal) -> SqlNative {
    let lex = &*l.lex;
    let text = || SqlNative::Text(lex.to_owned());
    if l.lang.is_some() {
        return text();
    }
    let Some(local) = l.datatype.strip_prefix(XSD) else {
        return text();
    };
    let native = if let Some(range) = integer_range(local) {
        lex.parse::<i64>().ok().filter(|v| range.contains(v)).map(SqlNative::Int)
    } else {
        match local {
            "decimal" => is_decimal_lexical(lex).then(|| lex.parse::<f64>().ok()).flatten().map(SqlNative::Real),
            "double" | "float" => parse_floating(lex).map(SqlNative::Real),
            "boolean" => match lex {
                "true" | "1" => Some(SqlNative::Int(1)),
                "false" | "0" => Some(SqlNative::Int(0)),
                _ => None,
            },
            _ => None,
        }
    };
    native.unwrap_or_else(text)
}

/// Value range of xsd:integer and its derived types (local names), clipped to i64: values
/// that are valid but do not fit an i64 (big xsd:integer, xsd:unsignedLong above i64::MAX)
/// fail the `i64` parse and fall back to TEXT.
fn integer_range(local: &str) -> Option<std::ops::RangeInclusive<i64>> {
    Some(match local {
        "integer" | "long" => i64::MIN..=i64::MAX,
        "int" => i32::MIN as i64..=i32::MAX as i64,
        "short" => i16::MIN as i64..=i16::MAX as i64,
        "byte" => i8::MIN as i64..=i8::MAX as i64,
        "nonNegativeInteger" | "unsignedLong" => 0..=i64::MAX,
        "positiveInteger" => 1..=i64::MAX,
        "nonPositiveInteger" => i64::MIN..=0,
        "negativeInteger" => i64::MIN..=-1,
        "unsignedInt" => 0..=u32::MAX as i64,
        "unsignedShort" => 0..=u16::MAX as i64,
        "unsignedByte" => 0..=u8::MAX as i64,
        _ => return None,
    })
}

/// Splits an optional leading sign.
fn strip_sign(s: &str) -> &str {
    s.strip_prefix(['+', '-']).unwrap_or(s)
}

/// `[0-9]+ ('.' [0-9]*)? | '.' [0-9]+` (the unsigned decimal mantissa).
fn is_unsigned_decimal(s: &str) -> bool {
    let (int, frac) = match s.split_once('.') {
        Some((int, frac)) => (int, Some(frac)),
        None => (s, None),
    };
    let digits = |x: &str| x.bytes().all(|b| b.is_ascii_digit());
    match frac {
        None => !int.is_empty() && digits(int),
        Some(frac) => (!int.is_empty() || !frac.is_empty()) && digits(int) && digits(frac),
    }
}

/// xsd:decimal lexical space: `[+-]? ([0-9]+ ('.' [0-9]*)? | '.' [0-9]+)`.
fn is_decimal_lexical(lex: &str) -> bool {
    is_unsigned_decimal(strip_sign(lex))
}

/// xsd:double / xsd:float lexical space (XSD 1.1): a signed decimal mantissa with an optional
/// exponent, or `[+-]?INF`, or `NaN`.
fn parse_floating(lex: &str) -> Option<f64> {
    match lex {
        "INF" | "+INF" => return Some(f64::INFINITY),
        "-INF" => return Some(f64::NEG_INFINITY),
        "NaN" => return Some(f64::NAN),
        _ => {}
    }
    let unsigned = strip_sign(lex);
    let (mantissa, exponent) = match unsigned.find(['e', 'E']) {
        Some(i) => (&unsigned[..i], Some(&unsigned[i + 1..])),
        None => (unsigned, None),
    };
    let exponent_ok = exponent.is_none_or(|e| {
        let digits = strip_sign(e);
        !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
    });
    (is_unsigned_decimal(mantissa) && exponent_ok).then(|| lex.parse::<f64>().ok()).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn typed(lex: &str, local: &str) -> Term {
        Term::Literal(Literal::typed(lex, &format!("{XSD}{local}")))
    }

    fn native(t: Term) -> SqlNative {
        sql_native(Some(&t))
    }

    fn text(s: &str) -> SqlNative {
        SqlNative::Text(s.to_owned())
    }

    #[test]
    fn mapping_table() {
        use SqlNative::*;
        let cases: Vec<(Term, SqlNative)> = vec![
            (Term::Iri("http://ex/a".into()), text("http://ex/a")),
            (Term::BNode("b1a".into()), text("_:b1a")),
            (Term::Literal(Literal::simple("hello")), text("hello")),
            (typed("hello", "string"), text("hello")),
            (Term::Literal(Literal::lang("chat", "fr")), text("chat")),
            (typed("42", "integer"), Int(42)),
            (typed("-7", "int"), Int(-7)),
            (typed("+007", "integer"), Int(7)),
            (typed("9223372036854775807", "long"), Int(i64::MAX)),
            (typed("9223372036854775808", "integer"), text("9223372036854775808")),
            (typed("18446744073709551615", "unsignedLong"), text("18446744073709551615")),
            (typed("1.5", "decimal"), Real(1.5)),
            (typed("-.5", "decimal"), Real(-0.5)),
            (typed("2.", "decimal"), Real(2.0)),
            (typed("1.0E6", "double"), Real(1.0e6)),
            (typed("-1e-3", "double"), Real(-0.001)),
            (typed("0.1", "float"), Real(0.1)),
            (typed("INF", "double"), Real(f64::INFINITY)),
            (typed("+INF", "float"), Real(f64::INFINITY)),
            (typed("-INF", "double"), Real(f64::NEG_INFINITY)),
            (typed("true", "boolean"), Int(1)),
            (typed("0", "boolean"), Int(0)),
            (typed("2024-01-02T03:04:05Z", "dateTime"), text("2024-01-02T03:04:05Z")),
            (typed("2024-01-02", "date"), text("2024-01-02")),
            (typed("12:00:00", "time"), text("12:00:00")),
            (typed("P1D", "dayTimeDuration"), text("P1D")),
            (typed("a7", "hexBinary"), text("a7")),
            (Term::Literal(Literal::typed("5,5", "http://ex/custom")), text("5,5")),
        ];
        for (term, expected) in cases {
            assert_eq!(native(term.clone()), expected, "{term:?}");
        }
        assert_eq!(sql_native(None), Null);
    }

    #[test]
    fn nan_is_real_nan() {
        match native(typed("NaN", "double")) {
            SqlNative::Real(v) => assert!(v.is_nan()),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn ill_typed_literals_are_lexical_text() {
        let cases = [
            ("abc", "integer"),
            ("1.5", "integer"),
            (" 1", "integer"),
            ("", "integer"),
            ("+", "integer"),
            ("300", "byte"),
            ("-1", "nonNegativeInteger"),
            ("0", "positiveInteger"),
            ("1", "negativeInteger"),
            ("70000", "unsignedShort"),
            ("1e5", "decimal"),
            (".", "decimal"),
            ("inf", "double"),
            ("Infinity", "double"),
            ("nan", "float"),
            ("1e", "double"),
            ("e5", "double"),
            ("1.0E+", "double"),
            ("yes", "boolean"),
            ("TRUE", "boolean"),
        ];
        for (lex, local) in cases {
            assert_eq!(native(typed(lex, local)), text(lex), "{lex}^^xsd:{local}");
        }
    }

    #[test]
    fn integer_family_ranges_accept_bounds() {
        let cases = [
            ("-128", "byte", -128),
            ("127", "byte", 127),
            ("65535", "unsignedShort", 65535),
            ("4294967295", "unsignedInt", 4294967295),
            ("-2147483648", "int", -2147483648),
            ("0", "nonPositiveInteger", 0),
            ("-1", "negativeInteger", -1),
            ("1", "positiveInteger", 1),
            ("255", "unsignedByte", 255),
        ];
        for (lex, local, v) in cases {
            assert_eq!(native(typed(lex, local)), SqlNative::Int(v), "{lex}^^xsd:{local}");
        }
    }

    #[test]
    fn accepts_shared_terms() {
        let t = Arc::new(Term::Iri("http://ex/x".into()));
        assert_eq!(sql_native(Some(&t)), text("http://ex/x"));
    }
}
