//! The `=` / `!=` matrix (§6.3) checked against W3C sparql10/open-world open-eq-07/08, NaN,
//! the `<` family (§6.4) and TEMP-vs-dictionary identity.
use sq_core::{Literal, Term, TermId};

use super::harness::Harness;

/// open-world/data-2.ttl objects of `:p`: x1..x8.
const OPEN_EQ_TERMS: [&str; 8] = [
    "\"xyz\"",
    "\"xyz\"@en",
    "\"xyz\"@EN",
    "\"xyz\"^^xsd:string",
    "\"xyz\"^^xsd:integer",
    "\"xyz\"^^<http://example/unknown>",
    "_:xyz",
    "<http://example/xyz>",
];

#[test]
fn open_eq_07_08_matrix() {
    let mut h = Harness::new();
    let ids: Vec<TermId> = OPEN_EQ_TERMS.iter().map(|t| h.term(t)).collect();
    let mut eq_true = vec![];
    let mut ne_true = vec![];
    let mut errors = vec![];
    for (i, a) in ids.iter().enumerate() {
        for (j, b) in ids.iter().enumerate() {
            let binds = [("v1", *a), ("v2", *b)];
            let eq = h.ebv("?v1 = ?v2", &binds);
            let ne = h.ebv("?v1 != ?v2", &binds);
            match (eq, ne) {
                (Some(true), Some(false)) => eq_true.push((i + 1, j + 1)),
                (Some(false), Some(true)) => ne_true.push((i + 1, j + 1)),
                (None, None) => errors.push((i + 1, j + 1)),
                other => panic!("x{} vs x{}: inconsistent = / != {other:?}", i + 1, j + 1),
            }
        }
    }
    // open-eq-07: 12 rows; open-eq-08: 42 rows (64 pairs - 12 equal - 10 errors).
    assert_eq!(eq_true.len(), 12, "{eq_true:?}");
    assert_eq!(ne_true.len(), 42);
    let expected_eq = [(1, 1), (1, 4), (4, 1), (4, 4), (2, 2), (2, 3), (3, 2), (3, 3), (5, 5), (6, 6), (7, 7), (8, 8)];
    for p in expected_eq {
        assert!(eq_true.contains(&p), "x{} = x{}", p.0, p.1);
    }
    // Errors: the ill-typed x5 and the unknown-datatype x6 against simple literals and each
    // other (rule 3).
    let mut expected_err = vec![(5, 1), (5, 4), (5, 6), (6, 1), (6, 4), (6, 5)];
    expected_err.extend(expected_err.clone().iter().map(|(a, b)| (*b, *a)));
    expected_err.sort();
    expected_err.dedup();
    errors.sort();
    assert_eq!(errors, expected_err);
}

#[test]
fn equality_classes() {
    let mut h = Harness::new();
    let cases: &[(&str, Option<bool>)] = &[
        // Numeric value equality across types and lexical forms.
        ("1 = 1.0", Some(true)),
        ("1 = 1.0e0", Some(true)),
        ("1 = \"01\"^^xsd:integer", Some(true)),
        ("\"1\"^^xsd:int = 1", Some(true)),
        // The decimal 0.1 promotes to the double nearest to 0.1, i.e. 0.1e0.
        ("0.1 = 0.1e0", Some(true)),
        ("1 = 2", Some(false)),
        ("1 = \"1\"", Some(false)),
        ("1 = true", Some(false)),
        ("true = \"true\"^^xsd:boolean", Some(true)),
        ("true = \"1\"^^xsd:boolean", Some(true)),
        ("\"abc\" = \"abc\"^^xsd:string", Some(true)),
        ("\"abc\" = \"abc\"@en", Some(false)),
        ("\"abc\"@en = \"abc\"@EN", Some(true)),
        ("\"abc\"@en = \"abc\"@fr", Some(false)),
        ("<http://example.org/a> = <http://example.org/a>", Some(true)),
        ("<http://example.org/a> = \"http://example.org/a\"", Some(false)),
        ("<http://example.org/a> = \"x\"^^<http://example/unknown>", Some(false)),
        // Other / IllTyped.
        ("\"x\"^^<http://example/t> = \"x\"^^<http://example/t>", Some(true)),
        ("\"x\"^^<http://example/t> = \"y\"^^<http://example/t>", None),
        ("\"x\"^^<http://example/t> = 1", None),
        ("\"x\"^^<http://example/t> = \"x\"@en", Some(false)),
        ("\"abc\"^^xsd:integer = 1", None),
        ("\"abc\"^^xsd:integer = \"abc\"^^xsd:integer", Some(true)),
        // Dates: value equality; indeterminate (±14 h) comparisons are errors.
        ("\"2002-04-02T12:00:00-01:00\"^^xsd:dateTime = \"2002-04-02T17:00:00+04:00\"^^xsd:dateTime", Some(true)),
        ("\"2002-04-02T12:00:00Z\"^^xsd:dateTime = \"2002-04-02T12:00:00\"^^xsd:dateTime", None),
        ("\"2002-04-02T12:00:00Z\"^^xsd:dateTime = \"2002-04-03T12:00:00\"^^xsd:dateTime", Some(false)),
        ("\"2002-04-02\"^^xsd:date = \"2002-04-02T00:00:00\"^^xsd:dateTime", Some(false)),
        ("\"2002-04-02\"^^xsd:date = \"2002-04-02\"^^xsd:date", Some(true)),
        ("\"PT1H\"^^xsd:dayTimeDuration = \"PT60M\"^^xsd:dayTimeDuration", Some(true)),
        ("\"P1Y\"^^xsd:yearMonthDuration = \"P12M\"^^xsd:duration", Some(true)),
        ("\"P0Y\"^^xsd:yearMonthDuration = \"PT0S\"^^xsd:dayTimeDuration", Some(true)),
        // Unbound.
        ("?u = 1", None),
        ("1 = ?u", None),
    ];
    for (e, expect) in cases {
        assert_eq!(h.ebv(e, &[]), *expect, "{e}");
        // `!=` is the negation with errors preserved.
        let ne = e.replacen(" = ", " != ", 1);
        assert_eq!(h.ebv(&ne, &[]), expect.map(|b| !b), "{ne}");
    }
}

#[test]
fn nan_semantics() {
    let mut h = Harness::new();
    let nan = h.term("\"NaN\"^^xsd:double");
    let fnan = h.term("\"NaN\"^^xsd:float");
    let one = h.term("1");
    // Same id, but NaN ≠ NaN (the rule-1 exception).
    assert_eq!(h.ebv("?x = ?x", &[("x", nan)]), Some(false));
    assert_eq!(h.ebv("?x != ?x", &[("x", nan)]), Some(true));
    assert_eq!(h.ebv("?x = ?x", &[("x", fnan)]), Some(false));
    assert_eq!(h.ebv("sameTerm(?x, ?x)", &[("x", nan)]), Some(true));
    for op in ["<", "<=", ">", ">="] {
        assert_eq!(h.ebv(&format!("?x {op} ?y"), &[("x", nan), ("y", one)]), Some(false), "NaN {op} 1");
        assert_eq!(h.ebv(&format!("?y {op} ?x"), &[("x", nan), ("y", one)]), Some(false), "1 {op} NaN");
    }
    assert_eq!(h.ebv("?x = 1", &[("x", nan)]), Some(false));
    assert_eq!(h.ebv("(0.0e0/0) = (0.0e0/0)", &[]), Some(false));
    assert_eq!(h.ebv("?x IN (?x)", &[("x", nan)]), Some(false));
}

#[test]
fn ordering_operators() {
    let mut h = Harness::new();
    let cases: &[(&str, Option<bool>)] = &[
        ("1 < 2", Some(true)),
        ("2 < 1.5", Some(false)),
        ("1 < 1.5e0", Some(true)),
        ("xsd:float(0.5) <= 0.5", Some(true)),
        ("9223372036854775807 > 9223372036854775806", Some(true)),
        ("0.1 < 0.10000000000000001", Some(true)),
        ("\"a\" < \"b\"", Some(true)),
        ("\"B\" < \"a\"", Some(true)),
        ("\"abc\" < \"abd\"^^xsd:string", Some(true)),
        ("\"\u{e9}\" > \"z\"", Some(true)),
        ("\"a\"@en < \"b\"@en", None),
        ("\"a\" < 1", None),
        ("false < true", Some(true)),
        ("true <= true", Some(true)),
        ("<http://a> < <http://b>", None),
        ("\"2002-04-02T12:00:00Z\"^^xsd:dateTime < \"2002-04-02T13:00:00Z\"^^xsd:dateTime", Some(true)),
        ("\"2002-04-02T12:00:00Z\"^^xsd:dateTime < \"2002-04-02T13:00:00\"^^xsd:dateTime", None),
        ("\"2002-04-02T12:00:00Z\"^^xsd:dateTime < \"2002-04-05T13:00:00\"^^xsd:dateTime", Some(true)),
        ("\"2002-04-02\"^^xsd:date < \"2002-04-03\"^^xsd:date", Some(true)),
        ("\"12:00:00\"^^xsd:time < \"13:00:00\"^^xsd:time", Some(true)),
        ("\"2002\"^^xsd:gYear < \"2003\"^^xsd:gYear", Some(true)),
        ("\"P1Y\"^^xsd:yearMonthDuration < \"P13M\"^^xsd:yearMonthDuration", Some(true)),
        ("\"PT1H\"^^xsd:dayTimeDuration > \"PT59M\"^^xsd:dayTimeDuration", Some(true)),
        ("\"P1Y\"^^xsd:duration < \"P2Y\"^^xsd:duration", None),
        ("\"P1Y\"^^xsd:yearMonthDuration < \"PT1H\"^^xsd:dayTimeDuration", None),
        ("\"2002-04-02\"^^xsd:date < \"2002-04-02T00:00:00\"^^xsd:dateTime", None),
        ("\"abc\"^^xsd:integer < 1", None),
        ("?u < 1", None),
    ];
    for (e, expect) in cases {
        assert_eq!(h.ebv(e, &[]), *expect, "{e}");
    }
    // Fast path: ?x op INT with inline and non-inline values.
    let (five, five_dec, five_dbl, s) = (h.term("5"), h.term("5.0"), h.term("\"5e0\"^^xsd:double"), h.term("\"5\""));
    for x in [five, five_dec, five_dbl] {
        assert_eq!(h.ebv("?x < 6", &[("x", x)]), Some(true));
        assert_eq!(h.ebv("6 > ?x", &[("x", x)]), Some(true));
        assert_eq!(h.ebv("?x >= 5", &[("x", x)]), Some(true));
        assert_eq!(h.ebv("?x = 5", &[("x", x)]), Some(true));
        assert_eq!(h.ebv("5 = ?x", &[("x", x)]), Some(true));
        assert_eq!(h.ebv("?x != 5", &[("x", x)]), Some(false));
        assert_eq!(h.ebv("?x > 5", &[("x", x)]), Some(false));
    }
    assert_eq!(h.ebv("?x < 6", &[("x", s)]), None);
    assert_eq!(h.ebv("?x = 5", &[("x", s)]), Some(false));
    assert_eq!(h.ebv("?x < 6", &[]), None);
}

#[test]
fn id_eq_fast_path() {
    let mut h = Harness::new();
    let iri = h.term("<http://example.org/a>");
    let other_iri = h.term("<http://example.org/b>");
    let s = h.term("\"a string longer than seven\"");
    let short = h.term("\"abc\"");
    let lang = h.term("\"abc\"@en");
    let ill = h.term("\"abc\"^^xsd:integer");
    let unknown = h.term("\"abc\"^^<http://example/t>");
    let num = h.term("1");
    let e_iri = "?x = <http://example.org/a>";
    assert_eq!(h.ebv(e_iri, &[("x", iri)]), Some(true));
    for x in [other_iri, s, short, lang, ill, unknown, num] {
        assert_eq!(h.ebv(e_iri, &[("x", x)]), Some(false));
    }
    let e_str = "?x = \"abc\"";
    assert_eq!(h.ebv(e_str, &[("x", short)]), Some(true));
    for x in [iri, s, lang, num] {
        assert_eq!(h.ebv(e_str, &[("x", x)]), Some(false));
    }
    // Ill-typed and unknown-datatype literals against a simple literal: error (rule 3).
    assert_eq!(h.ebv(e_str, &[("x", ill)]), None);
    assert_eq!(h.ebv(e_str, &[("x", unknown)]), None);
    assert_eq!(h.ebv(e_str, &[]), None);
    let e_long = "?x = \"a string longer than seven\"";
    assert_eq!(h.ebv(e_long, &[("x", s)]), Some(true));
    // LANG(?x) = "en" reads only the language tag.
    assert_eq!(h.ebv("LANG(?x) = \"en\"", &[("x", lang)]), Some(true));
    assert_eq!(h.ebv("LANG(?x) = \"\"", &[("x", short)]), Some(true));
    assert_eq!(h.ebv("LANG(?x) = \"en\"", &[("x", short)]), Some(false));
    assert_eq!(h.ebv("LANG(?x) = \"en\"", &[("x", iri)]), None);
    assert_eq!(h.ebv("LANGMATCHES(LANG(?x), \"EN\")", &[("x", lang)]), Some(true));
    assert_eq!(h.ebv("LANGMATCHES(LANG(?x), \"*\")", &[("x", short)]), Some(false));
}

#[test]
fn temp_ids_compare_by_term() {
    // A slot the planner left non-canonical may hold a TEMP id for a stored term.
    let mut h = Harness::new();
    let iri = h.term("<http://example.org/a>");
    let s = h.term("\"a string longer than seven\"");
    let temp_iri = h.env.resolver.temp(Term::Iri("http://example.org/a".into()));
    let temp_s = h.env.resolver.temp(Term::Literal(Literal::simple("a string longer than seven")));
    assert!(temp_iri.is_temp() && temp_s.is_temp());
    assert_eq!(h.ebv("?x = <http://example.org/a>", &[("x", temp_iri)]), Some(true));
    assert_eq!(h.ebv("sameTerm(?x, ?y)", &[("x", temp_iri), ("y", iri)]), Some(true));
    assert_eq!(h.ebv("?x = ?y", &[("x", temp_s), ("y", s)]), Some(true));
    assert_eq!(h.ebv("?x IN (<http://example.org/b>, <http://example.org/a>)", &[("x", temp_iri)]), Some(true));
    // A canonical result is requested: the pass-through TEMP id is re-interned.
    assert_eq!(h.eval("?x", &[("x", temp_iri)]), Some(iri));
}

/// `<` against numeric constants and numeric expressions (the IntCmp and `num` paths, which
/// decide most type errors from tags without decoding), unary plus and arithmetic agree with
/// the value-level definitions for inline, dictionary and TEMP ids of every value class.
#[test]
fn ordering_against_numbers_matches_value_semantics() {
    use std::str::FromStr;

    use crate::{Order, Value, compare_values, value_of};
    use oxsdatatypes::{Decimal, Double};
    use sq_core::eval_api::TermResolver;
    let terms = [
        "1", "\"01\"^^xsd:integer", "1.5", "\"1\"^^xsd:int", "\"1e0\"^^xsd:double", "\"NaN\"^^xsd:float",
        "\"12.25\"^^xsd:float", "true", "\"1\"^^xsd:boolean", "\"abc\"", "\"a long simple literal\"", "\"\"",
        "\"abc\"@en", "\"x\"^^xsd:integer", "\"300\"^^xsd:byte", "\"x\"^^<http://ex/t>", "<http://ex/a>", "_:b",
        "\"2002-04-01T00:00:00Z\"^^xsd:dateTime", "\"2002-04-01\"^^xsd:date", "\"12:00:00\"^^xsd:time",
        "\"P1Y\"^^xsd:yearMonthDuration", "\"99999999999999999999.5\"^^xsd:decimal",
    ];
    let mut h = Harness::new();
    let mut ids: Vec<TermId> = terms.iter().map(|t| h.term(t)).collect();
    let dups: Vec<TermId> = ids
        .iter()
        .filter(|id| id.is_dict())
        .map(|id| {
            let t = h.env.resolver.term(*id).unwrap();
            h.env.resolver.temp((*t).clone())
        })
        .collect();
    ids.extend(dups);
    let constants = [
        ("10", Value::Integer(10)),
        ("10.5", Value::Decimal(Decimal::from_str("10.5").unwrap())),
        ("\"1e1\"^^xsd:double", Value::Double(Double::from(10.0))),
    ];
    let mut bad = vec![];
    for &x in &ids {
        let vx = h.env.with(|cx| value_of(x, cx)).unwrap();
        let shown = h.render(Some(x));
        for (k, vk) in &constants {
            let order = compare_values(&vx, vk);
            let lt = order.map(|o| o == Order::Less);
            let gt = order.map(|o| o == Order::Greater);
            for (expr, want) in [
                (format!("?x < {k}"), lt),
                (format!("{k} > ?x"), lt),
                (format!("?x > {k}"), gt),
                (format!("{k} <= ?x"), order.map(|o| matches!(o, Order::Greater | Order::Equal))),
            ] {
                let got = h.ebv(&expr, &[("x", x)]);
                if got != want {
                    bad.push(format!("{expr} with ?x = {shown}: got {got:?} want {want:?}"));
                }
            }
            // Arithmetic is an error exactly for non-numeric operands (NaN compares false).
            let expr = format!("?x + 0 < {k}");
            if h.ebv(&expr, &[("x", x)]).is_none() == vx.is_numeric() {
                bad.push(format!("{expr} with ?x = {shown}"));
            }
        }
        let plus = h.eval("+?x", &[("x", x)]);
        if plus.is_some() != vx.is_numeric() {
            bad.push(format!("+?x with ?x = {shown}: got {}", h.render(plus)));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
