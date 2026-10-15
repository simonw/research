//! ORDER BY keys (§6.5): totality over random mixed values, agreement with `<` wherever `<`
//! is defined, class order, and identical keys for identical terms.
use std::cmp::Ordering;

use sq_core::TermId;
use sq_core::testing::Rng;

use super::harness::Harness;
use crate::{Order, SortKey, Value, compare_values, sort_key, sort_key_value, value_of};

fn pick<'a>(r: &mut Rng, xs: &[&'a str]) -> &'a str {
    xs[r.below(xs.len() as u64) as usize]
}

/// A random term in SPARQL syntax covering every value class, canonical and non-canonical
/// lexical forms, NaN / ±INF / -0, ill-typed literals and unknown datatypes.
fn random_term(r: &mut Rng) -> String {
    let small = |r: &mut Rng| r.below(21) as i64 - 10;
    match r.below(22) {
        0 => format!("{}", small(r)),
        1 => format!("{}", (r.next_u64() >> r.below(64)) as i64 * if r.below(2) == 0 { 1 } else { -1 }),
        2 => format!("\"{}{:02}\"^^xsd:integer", pick(r, &["+", "", "-"]), r.below(30)),
        3 => format!("{}.{}", small(r), r.below(1000)),
        4 => format!("\"{}.{:019}\"^^xsd:decimal", small(r), r.below(u64::MAX)),
        5 => format!("\"{}\"^^xsd:double", pick(r, &["NaN", "INF", "-INF", "-0", "0", "0.1", "1e300", "-1e-300", "2.5", "1", "-2"])),
        6 => format!("\"{}e{}\"^^xsd:double", small(r), small(r)),
        7 => format!("\"{}\"^^xsd:float", pick(r, &["NaN", "INF", "-INF", "0.1", "1.5", "-7.875", "1", "16777217"])),
        8 => format!("\"{}\"^^xsd:{}", small(r).abs(), pick(r, &["int", "byte", "unsignedShort", "long"])),
        9 => format!("\"{}\"", pick(r, &["", "a", "B", "abc", "abd", "ab\u{0}c", "ab", "é", "a long string value", "食べ物"])),
        10 => format!("\"{}\"@{}", pick(r, &["a", "chat", "abc"]), pick(r, &["en", "fr", "en-gb"])),
        11 => pick(r, &["true", "false", "\"1\"^^xsd:boolean", "\"0\"^^xsd:boolean"]).to_string(),
        12 => format!(
            "\"2002-04-{:02}T{:02}:00:00{}\"^^xsd:dateTime",
            1 + r.below(3),
            r.below(24),
            pick(r, &["", "Z", "+05:00", "-14:00", "+00:00"])
        ),
        13 => format!("\"2002-04-{:02}{}\"^^xsd:date", 1 + r.below(3), pick(r, &["", "Z", "-08:00"])),
        14 => format!("\"{:02}:30:00{}\"^^xsd:time", r.below(24), pick(r, &["", "Z", "+01:00"])),
        15 => format!("\"{}\"^^xsd:gYear", 2000 + r.below(3)),
        16 => format!("\"P{}M\"^^xsd:yearMonthDuration", r.below(30)),
        17 => format!("\"PT{}M\"^^xsd:dayTimeDuration", r.below(200)),
        18 => format!("\"P{}D\"^^xsd:duration", r.below(40)),
        19 => format!("<http://example.org/{}>", pick(r, &["a", "b", "ab", ""])),
        20 => pick(r, &["\"abc\"^^xsd:integer", "\"x\"^^<http://example/t>", "\"y\"^^<http://example/t>", "\"300\"^^xsd:byte"])
            .to_string(),
        _ => "_:b".to_string(),
    }
}

struct Sample {
    ids: Vec<TermId>,
    keys: Vec<SortKey>,
    values: Vec<Option<Value>>,
}

fn sample(h: &mut Harness, seed: u64, n: usize) -> Sample {
    let mut r = Rng(seed);
    let mut ids: Vec<TermId> = (0..n).map(|_| h.term(&random_term(&mut r))).collect();
    ids.push(TermId::UNBOUND);
    ids.push(TermId::ERROR);
    let keys = ids.iter().map(|id| h.env.with(|cx| sort_key(*id, cx)).unwrap()).collect();
    let values = ids
        .iter()
        .map(|id| if crate::is_present(*id) { Some(h.env.with(|cx| value_of(*id, cx)).unwrap()) } else { None })
        .collect();
    Sample { ids, keys, values }
}

#[test]
fn sort_is_total_and_agrees_with_less_than() {
    let mut h = Harness::new();
    for seed in [1, 2, 3] {
        let s = sample(&mut h, seed * 0x9E37_79B9, 400);
        // sort_by over arbitrary mixed values never panics and is deterministic.
        let mut order: Vec<usize> = (0..s.ids.len()).collect();
        order.sort_by(|a, b| s.keys[*a].cmp(&s.keys[*b]));
        let mut again = order.clone();
        again.reverse();
        again.sort_by(|a, b| s.keys[*a].cmp(&s.keys[*b]));
        let ka: Vec<_> = order.iter().map(|i| &s.keys[*i]).collect();
        let kb: Vec<_> = again.iter().map(|i| &s.keys[*i]).collect();
        assert_eq!(ka, kb);
        // Agreement with `<` wherever `<` is defined.
        let mut checked = 0;
        for i in 0..s.ids.len() {
            for j in 0..s.ids.len() {
                let (Some(a), Some(b)) = (&s.values[i], &s.values[j]) else { continue };
                let expect = match compare_values(a, b) {
                    Some(Order::Less) => Ordering::Less,
                    Some(Order::Greater) => Ordering::Greater,
                    _ => continue,
                };
                checked += 1;
                assert_eq!(s.keys[i].cmp(&s.keys[j]), expect, "{a:?} vs {b:?}");
            }
        }
        assert!(checked > 5000, "only {checked} comparable pairs");
        // Identical terms ⇔ identical keys (UNBOUND and ERROR both key as lowest).
        let n = s.ids.len() - 2;
        for i in 0..n {
            for j in 0..n {
                assert_eq!(s.ids[i] == s.ids[j], s.keys[i] == s.keys[j], "{:?} / {:?}", s.values[i], s.values[j]);
            }
        }
    }
}

#[test]
fn sampled_triples_are_transitive() {
    let mut h = Harness::new();
    let s = sample(&mut h, 42, 300);
    let mut r = Rng(7);
    let n = s.keys.len() as u64;
    for _ in 0..20_000 {
        let (a, b, c) = (&s.keys[r.below(n) as usize], &s.keys[r.below(n) as usize], &s.keys[r.below(n) as usize]);
        if a <= b && b <= c {
            assert!(a <= c);
        }
        if a < b && b < c {
            assert!(a < c);
        }
    }
}

#[test]
fn class_order() {
    let mut h = Harness::new();
    let ids = [
        TermId::UNBOUND,
        h.term("_:b"),
        h.term("<http://example.org/z>"),
        h.term("-5"),
        h.term("\"NaN\"^^xsd:double"),
        h.term("\"a\""),
    ];
    let keys: Vec<SortKey> = ids.iter().map(|id| h.env.with(|cx| sort_key(*id, cx)).unwrap()).collect();
    assert!(keys[0].is_lowest());
    assert_eq!(keys[0], h.env.with(|cx| sort_key(TermId::ERROR, cx)).unwrap());
    assert!(keys[0] < keys[1] && keys[1] < keys[2] && keys[2] < keys[3]);
    // NaN sorts first among numerics.
    assert!(keys[4] < keys[3]);
    assert!(keys[3] < keys[5], "numeric family before strings");
}

#[test]
fn numeric_ties_are_deterministic() {
    let mut h = Harness::new();
    // 1, 01, 1.0 and 1e0 tie on value and are ordered by rank, then lexical form.
    let ids = [h.term("1"), h.term("\"01\"^^xsd:integer"), h.term("1.0"), h.term("\"1e0\"^^xsd:double"), h.term("\"1\"^^xsd:int")];
    let keys: Vec<SortKey> = ids.iter().map(|id| h.env.with(|cx| sort_key(*id, cx)).unwrap()).collect();
    for i in 0..keys.len() {
        for j in 0..keys.len() {
            assert_eq!(i == j, keys[i] == keys[j]);
        }
    }
    assert!(keys[3] > keys[0] && keys[3] > keys[1] && keys[3] > keys[2], "double rank after integer/decimal");
    // Large integers beyond 2^53 still order exactly.
    let (a, b) = (h.term("9007199254740993"), h.term("9007199254740992"));
    let (ka, kb) = (h.env.with(|cx| sort_key(a, cx)).unwrap(), h.env.with(|cx| sort_key(b, cx)).unwrap());
    assert!(kb < ka);
}

#[test]
fn computed_values_key_like_stored_terms() {
    let mut h = Harness::new();
    for (expr, stored) in [("1 + 1", "2"), ("CONCAT(\"a long \", \"string\")", "\"a long string\""), ("1.5 * 2", "3.0"), ("STRLANG(\"x\", \"en\")", "\"x\"@en")] {
        let c = h.compile(expr);
        let row = h.row(&[]);
        let computed = h.env.with(|cx| c.sort_key(&row, cx)).unwrap();
        let id = h.term(stored);
        let direct = h.env.with(|cx| sort_key(id, cx)).unwrap();
        let v = h.env.with(|cx| value_of(id, cx)).unwrap();
        if expr == "1.5 * 2" {
            // The computed decimal is the canonical "3", the stored one is "3.0": equal values
            // with different lexical forms tie on the primary and differ on the lexical form.
            assert_ne!(computed, direct);
            assert_eq!(&computed.as_bytes()[..20], &direct.as_bytes()[..20]);
        } else {
            assert_eq!(computed, direct, "{expr}");
            assert_eq!(sort_key_value(&v), direct, "{expr}");
        }
    }
    // Errors key as lowest.
    let c = h.compile("1/0");
    let row = h.row(&[]);
    assert!(h.env.with(|cx| c.sort_key(&row, cx)).unwrap().is_lowest());
}
