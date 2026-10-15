//! GROUP BY and aggregates: partitioning, empty groups, errors, COUNT(DISTINCT *).
use sq_core::ir::{AggFunc, AggSpec, Expr, Logical};
use sq_core::{Slot, TermId};

use super::*;

fn agg(func: AggFunc, distinct: bool, arg: Option<Expr>) -> AggSpec {
    AggSpec { func, distinct, arg }
}

fn group(input: Logical, keys: &[Slot], aggs: Vec<(Slot, AggSpec)>) -> Logical {
    Logical::Group { input: Box::new(input), keys: keys.to_vec(), aggs }
}

const DEC: &str = "<http://www.w3.org/2001/XMLSchema#decimal>";

/// :a :p 1, 2 ; :b :p 3, :x (an IRI).
fn data() -> (Fx, Slot, Slot, Logical) {
    let mut fx = Fx::new();
    let (a, b, p, x) = (fx.iri("a"), fx.iri("b"), fx.iri("p"), fx.iri("x"));
    fx.add(a, p, fx.int(1));
    fx.add(a, p, fx.int(2));
    fx.add(b, p, fx.int(3));
    fx.add(b, p, x);
    let (s, o) = (fx.var("s"), fx.var("o"));
    let pattern = bgp(vec![tp(v(s), c(p), v(o))]);
    (fx, s, o, pattern)
}

#[test]
fn groups_by_key_with_every_aggregate() {
    let (mut fx, s, o, pattern) = data();
    let outs: Vec<Slot> = (0..7).map(|i| fx.var(&format!("a{i}"))).collect();
    let sep = AggFunc::GroupConcat { separator: "|".into() };
    let aggs = vec![
        (outs[0], agg(AggFunc::CountAll, false, None)),
        (outs[1], agg(AggFunc::Count, false, Some(es(o)))),
        (outs[2], agg(AggFunc::Sum, false, Some(es(o)))),
        (outs[3], agg(AggFunc::Min, false, Some(es(o)))),
        (outs[4], agg(AggFunc::Max, false, Some(es(o)))),
        (outs[5], agg(sep, false, Some(es(o)))),
        (outs[6], agg(AggFunc::Avg, false, Some(es(o)))),
    ];
    let mut slots = vec![s];
    slots.extend(&outs);
    let result = fx.bag(&slots, group(pattern, &[s], aggs));
    // :b has an IRI: SUM and AVG are errors (unbound) but the group row stays; COUNT,
    // MIN/MAX (SortKey order: IRIs before literals) and GROUP_CONCAT (STR of the IRI) work.
    let row_a = [":a", &int(2), &int(2), &int(3), &int(1), &int(2), "\"1|2\"", &format!("\"1.5\"^^{DEC}")];
    let row_b = [":b", &int(2), &int(2), "-", ":x", &int(3), "\"3|http://ex/x\"", "-"];
    assert_eq!(result, bag(&[&row_a.map(|s| s as &str), &row_b.map(|s| s as &str)]));
}

#[test]
fn no_keys_over_no_solutions_gives_one_row() {
    let (mut fx, _, o, _) = data();
    let outs: Vec<Slot> = (0..5).map(|i| fx.var(&format!("e{i}"))).collect();
    let aggs = vec![
        (outs[0], agg(AggFunc::CountAll, false, None)),
        (outs[1], agg(AggFunc::Sum, false, Some(es(o)))),
        (outs[2], agg(AggFunc::Min, false, Some(es(o)))),
        (outs[3], agg(AggFunc::GroupConcat { separator: " ".into() }, false, Some(es(o)))),
        (outs[4], agg(AggFunc::Avg, false, Some(es(o)))),
    ];
    assert_eq!(
        fx.select(&outs, group(Logical::Empty, &[], aggs.clone())),
        rows(&[&[&int(0), &int(0), "-", "\"\"", &int(0)]])
    );
    // With a key there is no group at all.
    let k = fx.var("k");
    assert!(fx.select(&outs, group(Logical::Empty, &[k], aggs)).is_empty());
}

#[test]
fn count_distinct_star_compares_visible_variables_only() {
    let mut fx = Fx::new();
    let (a, p) = (fx.iri("a"), fx.iri("p"));
    fx.add(a, p, fx.bnode());
    fx.add(a, p, fx.bnode());
    let (s, b, n, d) = (fx.var("s"), fx.hidden("b"), fx.var("n"), fx.var("d"));
    // SELECT (COUNT(*) AS ?n) (COUNT(DISTINCT *) AS ?d) { ?s :p [] }
    let aggs = vec![(n, agg(AggFunc::CountAll, false, None)), (d, agg(AggFunc::CountAll, true, None))];
    let root = group(bgp(vec![tp(v(s), c(p), v(b))]), &[], aggs);
    assert_eq!(fx.select(&[n, d], root), rows(&[&[&int(2), &int(1)]]));
}

#[test]
fn distinct_aggregates_dedupe_by_term() {
    let mut fx = Fx::new();
    let x = fx.var("x");
    let (one, two) = (fx.int(1), fx.int(2));
    let (n, sum) = (fx.var("n"), fx.var("sum"));
    let aggs = || {
        vec![
            (n, agg(AggFunc::Count, true, Some(es(x)))),
            (sum, agg(AggFunc::Sum, true, Some(es(x)))),
        ]
    };
    let input = values(&[x], &[&[one], &[one], &[two]]);
    assert_eq!(fx.select(&[n, sum], group(input, &[], aggs())), rows(&[&[&int(2), &int(3)]]));
    // An unbound argument is an evaluation error: COUNT skips it, SUM becomes unbound.
    let input = values(&[x], &[&[one], &[one], &[two], &[TermId::UNBOUND]]);
    assert_eq!(fx.select(&[n, sum], group(input, &[], aggs())), rows(&[&[&int(2), "-"]]));
}

#[test]
fn unbound_keys_form_their_own_group_and_having_filters_groups() {
    let mut fx = Fx::new();
    let (k, x, n) = (fx.var("k"), fx.var("x"), fx.var("n"));
    let a = fx.iri("a");
    let one = fx.int(1);
    let input = values(&[k, x], &[&[a, one], &[TermId::UNBOUND, one], &[a, one], &[TermId::UNBOUND, one]]);
    let grouped = group(input, &[k], vec![(n, agg(AggFunc::CountAll, false, None))]);
    assert_eq!(fx.bag(&[k, n, x], grouped.clone()), bag(&[&[":a", &int(2), "-"], &["-", &int(2), "-"]]));
    // HAVING (BOUND(?k)) is a Filter above the Group.
    assert_eq!(fx.select(&[k, n], filter(grouped, Expr::Bound(k))), rows(&[&[":a", &int(2)]]));
}

/// §18.5.1: ListEval((unbound), μ) = (error), so a GROUP BY expression that is an error and
/// one over an unbound variable give the same key, (error), and fall into one group.
#[test]
fn error_and_unbound_group_keys_share_a_group() {
    let mut fx = Fx::new();
    let (x, k, n) = (fx.var("x"), fx.var("k"), fx.var("n"));
    let (zero, one) = (fx.int(0), fx.int(1));
    // GROUP BY (1/?x) with ?x unbound, ?x = 0 (division by zero), ?x = 1.
    let input = values(&[x], &[&[TermId::UNBOUND], &[zero], &[one]]);
    let keyed = extend(input, k, Expr::Divide(Box::new(ec(one)), Box::new(es(x))));
    let grouped = group(keyed, &[k], vec![(n, agg(AggFunc::CountAll, false, None))]);
    let one_dec = format!("\"1\"^^{DEC}");
    assert_eq!(fx.bag(&[k, n], grouped), bag(&[&["-", &int(2)], &[&one_dec, &int(1)]]));
}

#[test]
fn sample_and_custom_aggregates() {
    let mut fx = Fx::new();
    let (x, s, u) = (fx.var("x"), fx.var("s"), fx.var("u"));
    let a = fx.iri("a");
    let input = values(&[x], &[&[TermId::UNBOUND], &[a]]);
    let aggs = vec![
        (s, agg(AggFunc::Sample, false, Some(es(x)))),
        (u, agg(AggFunc::Custom("http://ex/agg".into()), false, Some(es(x)))),
    ];
    assert_eq!(fx.select(&[s, u], group(input, &[], aggs)), rows(&[&[":a", "-"]]));
}
