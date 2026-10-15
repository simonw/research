//! Aggregate accumulators (§6.7), with expectations from W3C sparql11/aggregates
//! (agg-avg-*, agg-sum-*, agg-min/max-*, agg-err-01/02, agg-groupconcat-*, agg-empty-group-*).
use sq_core::TermId;
use sq_core::ir::{AggFunc, AggSpec};

use super::harness::Harness;
use crate::accumulator;

fn spec(func: AggFunc, distinct: bool) -> AggSpec {
    AggSpec { func, distinct, arg: None }
}

/// Runs an aggregate over stored terms (`None` = an erroring / unbound argument).
fn run(h: &mut Harness, func: AggFunc, distinct: bool, inputs: &[Option<&str>]) -> String {
    let ids: Vec<Option<TermId>> = inputs.iter().map(|t| t.map(|t| h.term(t))).collect();
    let mut acc = accumulator(&spec(func, distinct));
    let out = h.env.with(|cx| {
        for id in &ids {
            acc.add(*id, cx)?;
        }
        acc.finish(cx)
    });
    let out = out.expect("fatal");
    h.render(out)
}

#[test]
fn count() {
    let mut h = Harness::new();
    assert_eq!(run(&mut h, AggFunc::Count, false, &[Some("1"), None, Some("1"), Some("\"a\"")]), r#""3"^^xsd:integer"#);
    assert_eq!(run(&mut h, AggFunc::Count, true, &[Some("1"), None, Some("1"), Some("1.0")]), r#""2"^^xsd:integer"#);
    assert_eq!(run(&mut h, AggFunc::Count, false, &[]), r#""0"^^xsd:integer"#);
    assert_eq!(run(&mut h, AggFunc::CountAll, false, &[None, None]), r#""2"^^xsd:integer"#);
    // COUNT(DISTINCT *) dedupes whole rows.
    let (a, b) = (h.term("<http://example.org/a>"), h.term("<http://example.org/b>"));
    let mut acc = accumulator(&spec(AggFunc::CountAll, true));
    let rows = [[a, b], [a, b], [b, a], [a, TermId::UNBOUND]];
    let out = h.env.with(|cx| {
        for r in &rows {
            acc.add_row(r, cx)?;
        }
        acc.finish(cx)
    });
    assert_eq!(out.unwrap(), TermId::from_int(3));
}

#[test]
fn sum_and_avg() {
    let mut h = Harness::new();
    // agg-sum-01 / agg-avg-01 over agg-numeric.ttl style inputs.
    assert_eq!(run(&mut h, AggFunc::Sum, false, &[Some("1"), Some("2"), Some("3")]), r#""6"^^xsd:integer"#);
    assert_eq!(run(&mut h, AggFunc::Sum, false, &[Some("1"), Some("2.5")]), r#""3.5"^^xsd:decimal"#);
    assert_eq!(run(&mut h, AggFunc::Sum, false, &[Some("1"), Some("\"2.5e0\"^^xsd:double")]), r#""3.5"^^xsd:double"#);
    assert_eq!(run(&mut h, AggFunc::Sum, false, &[]), r#""0"^^xsd:integer"#);
    assert_eq!(run(&mut h, AggFunc::Sum, true, &[Some("1"), Some("1"), Some("2")]), r#""3"^^xsd:integer"#);
    // Any error makes the aggregate unbound (agg-err-01).
    assert_eq!(run(&mut h, AggFunc::Sum, false, &[Some("1"), Some("\"x\"")]), "ERROR");
    assert_eq!(run(&mut h, AggFunc::Sum, false, &[Some("1"), None]), "ERROR");
    assert_eq!(run(&mut h, AggFunc::Sum, false, &[Some("9223372036854775807"), Some("1")]), "ERROR");
    // AVG: decimal division for integers/decimals, double when any float/double, empty → 0.
    assert_eq!(run(&mut h, AggFunc::Avg, false, &[Some("1"), Some("2"), Some("3"), Some("4")]), r#""2.5"^^xsd:decimal"#);
    assert_eq!(run(&mut h, AggFunc::Avg, false, &[Some("1"), Some("2")]), r#""1.5"^^xsd:decimal"#);
    assert_eq!(run(&mut h, AggFunc::Avg, false, &[Some("1"), Some("2"), Some("4")]), r#""2.333333333333333333"^^xsd:decimal"#);
    assert_eq!(run(&mut h, AggFunc::Avg, false, &[Some("1.0"), Some("2.0"), Some("3.0"), Some("4")]), r#""2.5"^^xsd:decimal"#);
    assert_eq!(
        run(&mut h, AggFunc::Avg, false, &[Some("\"1.5e0\"^^xsd:double"), Some("\"2.5e0\"^^xsd:double")]),
        r#""2"^^xsd:double"#
    );
    assert_eq!(run(&mut h, AggFunc::Avg, false, &[Some("1"), Some("\"2\"^^xsd:float")]), r#""1.5"^^xsd:double"#);
    assert_eq!(run(&mut h, AggFunc::Avg, false, &[]), r#""0"^^xsd:integer"#);
    assert_eq!(run(&mut h, AggFunc::Avg, false, &[Some("1"), Some("_:b2"), Some("3")]), "ERROR");
    // agg-avg-distinct: AVG(DISTINCT) over 1, 1, 2 → 1.5.
    assert_eq!(run(&mut h, AggFunc::Avg, true, &[Some("1"), Some("1"), Some("2")]), r#""1.5"^^xsd:decimal"#);
}

#[test]
fn min_max_sample() {
    let mut h = Harness::new();
    assert_eq!(run(&mut h, AggFunc::Min, false, &[Some("3"), Some("1"), Some("2")]), r#""1"^^xsd:integer"#);
    assert_eq!(run(&mut h, AggFunc::Max, false, &[Some("3"), Some("1"), Some("20")]), r#""20"^^xsd:integer"#);
    assert_eq!(run(&mut h, AggFunc::Max, false, &[Some("3"), Some("3.5"), Some("\"2e0\"^^xsd:double")]), r#""3.5"^^xsd:decimal"#);
    // Mixed types never error: the ORDER BY order (blank node < IRI < literal).
    assert_eq!(
        run(&mut h, AggFunc::Min, false, &[Some("1"), Some("<http://example.org/a>"), Some("\"a\"")]),
        "<http://example.org/a>"
    );
    assert_eq!(run(&mut h, AggFunc::Max, false, &[Some("1"), Some("<http://example.org/a>"), Some("\"a\"")]), r#""a""#);
    assert_eq!(run(&mut h, AggFunc::Max, false, &[Some("\"b\""), Some("\"a long string value\"")]), r#""b""#);
    assert_eq!(run(&mut h, AggFunc::Min, false, &[]), "ERROR");
    assert_eq!(run(&mut h, AggFunc::Max, false, &[Some("1"), None, Some("2")]), "ERROR");
    assert_eq!(run(&mut h, AggFunc::Min, true, &[Some("2"), Some("2"), Some("5")]), r#""2"^^xsd:integer"#);
    assert_eq!(run(&mut h, AggFunc::Sample, false, &[None, Some("\"x\""), Some("\"y\"")]), r#""x""#);
    assert_eq!(run(&mut h, AggFunc::Sample, false, &[]), "ERROR");
}

#[test]
fn group_concat() {
    let mut h = Harness::new();
    let sep = |s: &str| AggFunc::GroupConcat { separator: s.to_string() };
    assert_eq!(run(&mut h, sep(" "), false, &[Some("\"1\""), Some("\"22\"")]), r#""1 22""#);
    assert_eq!(run(&mut h, sep("|"), false, &[Some("\"aaa\""), Some("\"bb\""), Some("\"c\"")]), r#""aaa|bb|c""#);
    // A simple literal even when every input shares a language tag (§6.7).
    assert_eq!(run(&mut h, sep(" "), false, &[Some("\"a\"@en"), Some("\"b\"@en")]), r#""a b""#);
    // STR() of numbers and IRIs; stored lexical forms are kept.
    assert_eq!(run(&mut h, sep(","), false, &[Some("1"), Some("\"01\"^^xsd:integer"), Some("<http://x/>")]), r#""1,01,http://x/""#);
    assert_eq!(run(&mut h, sep(" "), true, &[Some("\"a\""), Some("\"a\""), Some("\"b\"")]), r#""a b""#);
    assert_eq!(run(&mut h, sep(" "), false, &[]), r#""""#);
    assert_eq!(run(&mut h, sep(" "), false, &[Some("\"a\""), Some("_:b")]), "ERROR");
    assert_eq!(run(&mut h, sep(" "), false, &[Some("\"a\""), None]), "ERROR");
}

#[test]
fn custom_aggregate_is_unbound() {
    let mut h = Harness::new();
    let f = AggFunc::Custom("http://example.org/agg".into());
    assert_eq!(run(&mut h, f.clone(), false, &[Some("1")]), "ERROR");
    assert_eq!(run(&mut h, f, true, &[]), "ERROR");
}

#[test]
fn computed_results_are_canonical_ids() {
    let mut h = Harness::new();
    let ids: Vec<TermId> = ["1", "2", "3"].iter().map(|t| h.term(t)).collect();
    let mut acc = accumulator(&spec(AggFunc::Sum, false));
    let out = h.env.with(|cx| {
        for id in &ids {
            acc.add(Some(*id), cx)?;
        }
        acc.finish(cx)
    });
    assert_eq!(out.unwrap(), TermId::from_int(6));
    // A stored GROUP_CONCAT result string comes back as the stored dictionary id.
    let stored = h.term("\"alpha beta gamma\"");
    let parts: Vec<TermId> = ["\"alpha\"", "\"beta\"", "\"gamma\""].iter().map(|t| h.term(t)).collect();
    let mut acc = accumulator(&spec(AggFunc::GroupConcat { separator: " ".into() }, false));
    let out = h.env.with(|cx| {
        for id in &parts {
            acc.add(Some(*id), cx)?;
        }
        acc.finish(cx)
    });
    assert_eq!(out.unwrap(), Some(stored));
}
