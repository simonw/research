//! Numeric leniency for the second comparison pass (`pass-lenient`): two numeric literals
//! of the SAME datatype with equal values compare equal even if their lexical forms differ
//! (`"1.0e6"^^xsd:double` vs `"1.0E6"^^xsd:double`, `"01"^^xsd:integer` vs `"1"^^xsd:integer`).
//! Literals of different datatypes, and ill-typed lexical forms, are never merged.
use oxrdf::{Dataset, Literal, NamedNode, Quad, Term};

const XSD: &str = "http://www.w3.org/2001/XMLSchema#";

const INTEGER_TYPES: [&str; 13] = [
    "integer",
    "long",
    "int",
    "short",
    "byte",
    "nonNegativeInteger",
    "positiveInteger",
    "nonPositiveInteger",
    "negativeInteger",
    "unsignedLong",
    "unsignedInt",
    "unsignedShort",
    "unsignedByte",
];

/// How literals are compared.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Numeric {
    /// RDF term equality.
    Exact,
    /// Numeric literals compare by (datatype, value).
    Lenient,
}

impl Numeric {
    /// The term to compare: unchanged in exact mode, value-canonical in lenient mode.
    pub fn apply(self, t: &Term) -> Term {
        match (self, t) {
            (Numeric::Lenient, Term::Literal(l)) => canonical_literal(l).map_or_else(|| t.clone(), Term::Literal),
            _ => t.clone(),
        }
    }

    pub fn apply_dataset(self, d: &Dataset) -> Dataset {
        match self {
            Numeric::Exact => d.clone(),
            Numeric::Lenient => d
                .iter()
                .map(|q| {
                    let mut q = Quad::from(q);
                    q.object = self.apply(&q.object);
                    q
                })
                .collect(),
        }
    }
}

/// A literal with the canonical lexical form of its value, if it is a valid numeric literal.
fn canonical_literal(l: &Literal) -> Option<Literal> {
    let local = l.datatype().as_str().strip_prefix(XSD)?;
    let lex = l.value();
    let canonical = if INTEGER_TYPES.contains(&local) {
        canonical_integer(lex)?
    } else {
        match local {
            "decimal" => canonical_decimal(lex)?,
            "double" => canonical_floating(lex, false)?,
            "float" => canonical_floating(lex, true)?,
            _ => return None,
        }
    };
    (canonical != lex).then(|| Literal::new_typed_literal(canonical, NamedNode::new_unchecked(l.datatype().as_str())))
}

fn split_sign(s: &str) -> (bool, &str) {
    match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    }
}

fn all_digits(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_digit())
}

/// `[+-]?[0-9]+` without leading zeros or `+`; `-0` is `0`. Works on any length.
fn canonical_integer(lex: &str) -> Option<String> {
    let (negative, digits) = split_sign(lex);
    if digits.is_empty() || !all_digits(digits) {
        return None;
    }
    let digits = digits.trim_start_matches('0');
    Some(match (digits.is_empty(), negative) {
        (true, _) => "0".to_owned(),
        (false, true) => format!("-{digits}"),
        (false, false) => digits.to_owned(),
    })
}

/// `[+-]?(digits)?(.digits)?` as `-?I.F` with no leading zeros in I (except a lone 0) and no
/// trailing zeros in F (except a lone 0).
fn canonical_decimal(lex: &str) -> Option<String> {
    let (negative, body) = split_sign(lex);
    let (int, frac) = body.split_once('.').unwrap_or((body, ""));
    if (int.is_empty() && frac.is_empty()) || !all_digits(int) || !all_digits(frac) {
        return None;
    }
    let int = match int.trim_start_matches('0') {
        "" => "0",
        i => i,
    };
    let frac = match frac.trim_end_matches('0') {
        "" => "0",
        f => f,
    };
    let zero = int == "0" && frac == "0";
    Some(format!("{}{int}.{frac}", if negative && !zero { "-" } else { "" }))
}

/// Value-canonical form of a double (or float, compared at single precision).
fn canonical_floating(lex: &str, single: bool) -> Option<String> {
    let value = match lex {
        "INF" | "+INF" => f64::INFINITY,
        "-INF" => f64::NEG_INFINITY,
        "NaN" => f64::NAN,
        _ => {
            let (_, body) = split_sign(lex);
            let mantissa = body.split(['e', 'E']).next().unwrap_or(body);
            if mantissa.is_empty() || !mantissa.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
                return None;
            }
            if single {
                f64::from(lex.parse::<f32>().ok()?)
            } else {
                lex.parse::<f64>().ok()?
            }
        }
    };
    Some(if value.is_nan() {
        "NaN".to_owned()
    } else if value.is_infinite() {
        if value > 0.0 { "INF" } else { "-INF" }.to_owned()
    } else if value == 0.0 {
        "0E0".to_owned()
    } else if single {
        format!("{:E}", value as f32)
    } else {
        format!("{value:E}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(lex: &str, local: &str) -> Term {
        Literal::new_typed_literal(lex, NamedNode::new_unchecked(format!("{XSD}{local}"))).into()
    }

    fn same(a: &Term, b: &Term) -> bool {
        Numeric::Lenient.apply(a) == Numeric::Lenient.apply(b)
    }

    #[test]
    fn equal_values_of_one_datatype_merge() {
        let pairs = [
            (lit("01", "integer"), lit("1", "integer")),
            (lit("+1", "integer"), lit("1", "integer")),
            (lit("-0", "integer"), lit("0", "integer")),
            (lit("123456789012345678901234567890", "integer"), lit("+0123456789012345678901234567890", "integer")),
            (lit("1.50", "decimal"), lit("1.5", "decimal")),
            (lit("1", "decimal"), lit("1.0", "decimal")),
            (lit(".5", "decimal"), lit("0.5", "decimal")),
            (lit("-0.0", "decimal"), lit("0", "decimal")),
            (lit("1.0e6", "double"), lit("1.0E6", "double")),
            (lit("1000000", "double"), lit("1E6", "double")),
            (lit("-0", "double"), lit("0.0e0", "double")),
            (lit("NaN", "double"), lit("NaN", "double")),
            (lit("+INF", "double"), lit("INF", "double")),
            (lit("0.1", "float"), lit("0.100000001", "float")),
            (lit("007", "byte"), lit("7", "byte")),
        ];
        for (a, b) in &pairs {
            assert!(same(a, b), "{a} vs {b}");
        }
    }

    #[test]
    fn different_values_datatypes_or_ill_typed_stay_apart() {
        let pairs = [
            (lit("1", "integer"), lit("2", "integer")),
            (lit("1", "integer"), lit("1", "decimal")),
            (lit("1", "integer"), lit("1", "int")),
            (lit("1.0", "double"), lit("1.0", "float")),
            (lit("0.1", "double"), lit("0.100000001", "double")),
            (lit("abc", "integer"), lit("abd", "integer")),
            (lit("1e1", "decimal"), lit("10", "decimal")),
            (lit("x", "string"), lit("y", "string")),
        ];
        for (a, b) in &pairs {
            assert!(!same(a, b), "{a} vs {b}");
        }
    }

    #[test]
    fn exact_mode_and_non_literals_are_untouched() {
        let t = lit("01", "integer");
        assert_eq!(Numeric::Exact.apply(&t), t);
        let iri: Term = NamedNode::new_unchecked("http://ex/1").into();
        assert_eq!(Numeric::Lenient.apply(&iri), iri);
        let ill = lit("1x", "integer");
        assert_eq!(Numeric::Lenient.apply(&ill), ill);
    }
}
