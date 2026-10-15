//! Ties between ORDER BY keys (SPARQL 1.1 §15.1): two solutions whose keys tie may come out
//! in either order, so the runs that `RowOrder::Runs` compares in sequence are runs of tied
//! keys, not of identical RDF terms.
//!
//! Keys tie when §15.1 puts them in the same place:
//! * unbound and unbound;
//! * any two blank nodes (§15.1 ranks blank nodes as one class and orders nothing inside it);
//! * two literals that are equal under the `<` of the operator table: numerics by value
//!   across datatypes (`"1"^^xsd:integer` and `"1.0"^^xsd:decimal`; see
//!   [`NumericValue::same_value`]), booleans by value, `xsd:dateTime`s that are the same
//!   instant (both with a timezone) or the same local time (both without);
//! * otherwise only identical terms (IRIs compare as strings; simple literals by code point).
//!
//! Pairs whose relative order §15.1 leaves undefined but which are not equal (`"a"@en` and
//! `"b"@en`, a literal of an unknown datatype and a number, a dateTime with a timezone and
//! one without) stay separate runs, so the expected order is required between them. That
//! keeps the tie relation an equivalence (adjacent rows are grouped transitively).
use oxrdf::{Literal, Term};

use super::numeric::NumericValue;

const XSD: &str = "http://www.w3.org/2001/XMLSchema#";

/// Do two ORDER BY keys tie (neither sorts before the other)?
pub(super) fn order_keys_tie(a: Option<&Term>, b: Option<&Term>) -> bool {
    match (a, b) {
        (None, None) | (Some(Term::BlankNode(_)), Some(Term::BlankNode(_))) => true,
        (Some(Term::Literal(x)), Some(Term::Literal(y))) => x == y || same_value(x, y),
        (a, b) => a == b,
    }
}

/// Equal values under SPARQL's `<` / `=` operator table.
fn same_value(x: &Literal, y: &Literal) -> bool {
    if let (Some(p), Some(q)) = (NumericValue::of(x), NumericValue::of(y)) {
        return p.same_value(&q);
    }
    match (xsd_local_name(x), xsd_local_name(y)) {
        (Some("boolean"), Some("boolean")) => {
            boolean(x.value()).is_some_and(|p| boolean(y.value()) == Some(p))
        }
        (Some("dateTime"), Some("dateTime")) => {
            date_time(x.value()).is_some_and(|p| date_time(y.value()).as_ref() == Some(&p))
        }
        _ => false,
    }
}

fn xsd_local_name(l: &Literal) -> Option<&str> {
    l.datatype().as_str().strip_prefix(XSD)
}

fn boolean(lex: &str) -> Option<bool> {
    match lex {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

/// An xsd:dateTime value: (has a timezone, whole seconds since 1970-01-01T00:00:00 — UTC
/// when there is a timezone, local otherwise — fractional-second digits without trailing
/// zeros).
type DateTime = (bool, i128, String);

/// `-?YYYY-MM-DDThh:mm:ss(.s+)?(Z|[+-]hh:mm)?`, including `24:00:00` (the next midnight).
fn date_time(lex: &str) -> Option<DateTime> {
    let two = |s: &str| -> Option<i64> { (s.len() == 2 && digits(s)).then(|| s.parse().ok()).flatten() };
    let (date, time) = lex.split_once('T')?;
    let (negative, date) = date.strip_prefix('-').map_or((false, date), |d| (true, d));
    let mut ymd = date.splitn(3, '-');
    let (year, month, day) = (ymd.next()?, ymd.next()?, ymd.next()?);
    if year.len() < 4 || !digits(year) {
        return None;
    }
    let year: i64 = year.parse().ok()?;
    let year = if negative { -year } else { year };
    let (month, day) = (two(month)?, two(day)?);
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return None;
    }

    let (clock, offset_minutes) = if let Some(clock) = time.strip_suffix('Z') {
        (clock, Some(0))
    } else if time.len() > 6 && matches!(time.as_bytes()[time.len() - 6], b'+' | b'-') {
        let (clock, zone) = time.split_at(time.len() - 6);
        let (h, m) = zone[1..].split_once(':')?;
        let (h, m) = (two(h)?, two(m)?);
        if m > 59 || h * 60 + m > 14 * 60 {
            return None;
        }
        (clock, Some(if zone.starts_with('-') { -(h * 60 + m) } else { h * 60 + m }))
    } else {
        (time, None)
    };
    let (hms, fraction) = clock.split_once('.').map_or((clock, None), |(a, b)| (a, Some(b)));
    if fraction.is_some_and(|f| f.is_empty() || !digits(f)) {
        return None;
    }
    let fraction = fraction.unwrap_or("").trim_end_matches('0');
    let mut parts = hms.splitn(3, ':');
    let (h, m, s) = (two(parts.next()?)?, two(parts.next()?)?, two(parts.next()?)?);
    if h > 24 || m > 59 || s > 59 || (h == 24 && (m, s, fraction) != (0, 0, "")) {
        return None;
    }
    let seconds = i128::from(days_from_civil(year, month, day)) * 86_400 + i128::from(h * 3600 + m * 60 + s)
        - i128::from(offset_minutes.unwrap_or(0) * 60);
    Some((offset_minutes.is_some(), seconds, fraction.to_owned()))
}

fn digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days from 1970-01-01 to the given proleptic Gregorian date (H. Hinnant's algorithm).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxrdf::{BlankNode, NamedNode};

    fn lit(lex: &str, local: &str) -> Term {
        Literal::new_typed_literal(lex, NamedNode::new_unchecked(format!("{XSD}{local}"))).into()
    }
    fn tie(a: &Term, b: &Term) -> bool {
        let forward = order_keys_tie(Some(a), Some(b));
        assert_eq!(forward, order_keys_tie(Some(b), Some(a)), "{a} / {b} is not symmetric");
        forward
    }

    #[test]
    fn unbound_and_blank_nodes() {
        assert!(order_keys_tie(None, None));
        let (p, q): (Term, Term) = (BlankNode::new_unchecked("p").into(), BlankNode::new_unchecked("q").into());
        assert!(tie(&p, &q));
        let iri: Term = NamedNode::new_unchecked("http://ex/a").into();
        assert!(!tie(&p, &iri));
        assert!(!order_keys_tie(None, Some(&p)));
        assert!(!order_keys_tie(Some(&iri), None));
    }

    #[test]
    fn numerics_tie_by_value_across_datatypes() {
        let ties = [
            (lit("1", "integer"), lit("1.0", "decimal")),
            (lit("01", "int"), lit("1", "integer")),
            (lit("1", "integer"), lit("1.0E0", "double")),
            (lit("1", "integer"), lit("1", "float")),
            (lit("-0", "integer"), lit("0.0e0", "double")),
            (lit("0.5", "decimal"), lit("5E-1", "float")),
            // Promotion: decimal 0.1 cast to double is the double 0.1.
            (lit("0.1", "decimal"), lit("0.1", "double")),
            (lit("0.1", "decimal"), lit("0.1", "float")),
            (lit("INF", "double"), lit("+INF", "float")),
            (lit("NaN", "double"), lit("NaN", "float")),
        ];
        for (a, b) in &ties {
            assert!(tie(a, b), "{a} / {b}");
        }
        let apart = [
            (lit("1", "integer"), lit("2", "integer")),
            (lit("1", "integer"), lit("1.000000001", "decimal")),
            // A float promoted to double is not the double 0.1.
            (lit("0.1", "float"), lit("0.1", "double")),
            (lit("NaN", "double"), lit("1", "double")),
            (lit("1", "integer"), lit("1", "string")),
            (lit("1x", "integer"), lit("1", "integer")),
            (lit("1", "integer"), lit("true", "boolean")),
        ];
        for (a, b) in &apart {
            assert!(!tie(a, b), "{a} / {b}");
        }
    }

    #[test]
    fn booleans_date_times_and_other_literals() {
        assert!(tie(&lit("1", "boolean"), &lit("true", "boolean")));
        assert!(!tie(&lit("0", "boolean"), &lit("true", "boolean")));
        let ties = [
            ("2000-01-01T00:00:00Z", "2000-01-01T01:00:00+01:00"),
            ("2000-01-01T00:00:00Z", "1999-12-31T19:00:00-05:00"),
            ("2000-01-01T00:00:00.500", "2000-01-01T00:00:00.5"),
            ("1999-12-31T24:00:00Z", "2000-01-01T00:00:00Z"),
            ("2000-02-29T12:00:00-14:00", "2000-03-01T02:00:00Z"),
            ("-0044-03-15T12:00:00Z", "-0044-03-15T13:00:00+01:00"),
        ];
        for (a, b) in ties {
            assert!(tie(&lit(a, "dateTime"), &lit(b, "dateTime")), "{a} / {b}");
        }
        let apart = [
            ("2000-01-01T00:00:00Z", "2000-01-01T00:00:00"),
            ("2000-01-01T00:00:00Z", "2000-01-01T00:00:01Z"),
            ("2000-01-01T00:00:00.1Z", "2000-01-01T00:00:00Z"),
            ("2001-02-29T00:00:00Z", "2001-03-01T00:00:00Z"),
            ("2000-01-01T24:00:01Z", "2000-01-02T00:00:01Z"),
        ];
        for (a, b) in apart {
            assert!(!tie(&lit(a, "dateTime"), &lit(b, "dateTime")), "{a} / {b}");
        }
        // Undefined but unequal pairs and plain strings keep their expected order.
        let en = |s: &str| -> Term { Literal::new_language_tagged_literal_unchecked(s, "en").into() };
        assert!(!tie(&en("a"), &en("b")));
        assert!(tie(&en("a"), &en("a")));
        let s = |v: &str| -> Term { Literal::new_simple_literal(v).into() };
        assert!(!tie(&s("1"), &s("1.0")));
        assert!(!tie(&s("a"), &en("a")));
        let iri = |v: &str| -> Term { NamedNode::new_unchecked(v).into() };
        assert!(!tie(&iri("http://ex/a"), &iri("http://ex/b")));
    }
}
