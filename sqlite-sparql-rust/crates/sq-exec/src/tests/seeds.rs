//! Seed propagation through every operator shape: a Join-mode seeded evaluation (BindJoin)
//! must equal the unseeded evaluation joined afterwards (HashJoin), and a Substitute-mode
//! evaluation (EXISTS) must equal a semi join, for subtrees where ?0 is certainly bound.
use sq_core::ir::{AggFunc, AggSpec, ExistsId, PathPlan};

use super::*;

struct D {
    fx: Fx,
    subjects: Vec<TermId>,
    p: [TermId; 3],
}

fn data() -> D {
    let fx = Fx::new();
    let subjects: Vec<TermId> = (0..6).map(|i| fx.iri(&format!("s{i}"))).collect();
    let p = ["p0", "p1", "p2"].map(|n| fx.iri(n));
    let edges = [(0, 0, 1), (1, 0, 2), (2, 0, 0), (3, 0, 3), (0, 1, 4), (2, 1, 5), (4, 1, 1), (5, 2, 0), (1, 2, 1)];
    for (s, pp, o) in edges {
        fx.add(subjects[s], p[pp], subjects[o]);
    }
    fx.add(subjects[3], p[1], int(7));
    D { fx, subjects, p }
}

/// Subtrees in which ?0 is certainly bound (and safe to inject).
fn shapes(d: &D) -> Vec<(&'static str, Plan)> {
    let [p0, p1, p2] = d.p;
    let s = |a: Slot, p: TermId, b: Slot| scan(v(a), c(p), v(b));
    let count = AggSpec { func: AggFunc::CountAll, distinct: false, arg: None };
    vec![
        ("scan", s(0, p0, 1)),
        ("scan-repeated", scan(v(0), c(p0), v(0))),
        ("union", union(vec![s(0, p0, 1), s(0, p1, 2)])),
        ("filter", filter(s(0, p0, 1), Expr::Not(Box::new(Expr::SameTerm(Box::new(sv(1)), Box::new(cv(d.subjects[2]))))))),
        ("extend", extend(s(0, p0, 1), 3, sv(1))),
        ("project", project(s(0, p0, 1), &[0])),
        ("distinct", plan(Op::Distinct { input: Box::new(project(s(0, p0, 1), &[0])), slots: vec![0] })),
        ("reduced", plan(Op::Reduced { input: Box::new(s(0, p0, 1)), slots: vec![0, 1] })),
        ("sort", plan(Op::Sort { input: Box::new(s(0, p0, 1)), keys: vec![order_key(sv(1), true)] })),
        ("sort-by-slot", sort_by(s(0, p0, 1), 1)),
        ("group", plan(Op::Group { input: Box::new(s(0, p0, 1)), keys: vec![0], aggs: vec![(3, count)], streaming: false })),
        ("hash-inner", hash_join(s(1, p1, 2), s(0, p0, 1), &[1], &[], JoinKind::Inner, false)),
        ("hash-inner-build-left", hash_join(s(0, p0, 1), s(1, p1, 2), &[1], &[], JoinKind::Inner, true)),
        ("hash-left", hash_join(s(1, p1, 2), s(0, p0, 1), &[1], &[], left(None), false)),
        ("hash-left-build-left", hash_join(s(0, p0, 1), s(1, p1, 2), &[1], &[], left(None), true)),
        ("hash-semi", hash_join(s(1, p2, 2), s(0, p0, 1), &[1], &[], JoinKind::Semi, false)),
        ("hash-anti-build-left", hash_join(s(0, p0, 1), s(1, p2, 2), &[1], &[], JoinKind::Anti, true)),
        ("nested", nested(s(0, p0, 1), s(1, p1, 2), &[1], JoinKind::Inner)),
        ("nested-left", nested(s(0, p0, 1), s(1, p1, 2), &[1], left(None))),
        ("bind", bind_join(s(0, p0, 1), scan_bound(GraphSel::Default, v(1), c(p1), v(2), &[1]), &[1], JoinKind::Inner)),
        ("bind-left", bind_join(s(0, p0, 1), scan_bound(GraphSel::Default, v(1), c(p1), v(2), &[1]), &[1], left(None))),
        ("bind-anti", bind_join(s(0, p0, 1), scan_bound(GraphSel::Default, v(1), c(p2), v(2), &[1]), &[1], JoinKind::Anti)),
        ("merge", merge_join(sort_by(s(0, p0, 1), 1), sort_by(s(1, p1, 2), 1), 1, &[], 6)),
        ("star", plan(Op::StarMerge { key: 0, inputs: vec![s(0, p0, 1), s(0, p1, 2)], check: vec![] })),
        (
            "minus",
            plan(Op::Minus { left: Box::new(s(0, p0, 1)), right: Box::new(s(1, p1, 2)), keys: vec![1], check: vec![] }),
        ),
        ("path-forward", plan(Op::Path { g: GraphSel::Default, s: v(0), path: PathPlan::OneOrMore(Box::new(PathPlan::Pred(p0))), o: v(1) })),
        ("path-backward", plan(Op::Path { g: GraphSel::Default, s: v(1), path: PathPlan::ZeroOrMore(Box::new(PathPlan::Pred(p1))), o: v(0) })),
        ("values", values(&[0, 1], vec![vec![d.subjects[1], int(1)], vec![d.subjects[4], u()], vec![d.subjects[1], int(2)]])),
    ]
}

/// Every subject plus a literal and an IRI that occurs nowhere.
fn outer(d: &D) -> Plan {
    let mut rows: Vec<Vec<TermId>> = d.subjects.iter().map(|s| vec![*s]).collect();
    rows.push(vec![int(7)]);
    rows.push(vec![d.fx.iri("nowhere")]);
    values(&[0], rows)
}

#[test]
fn join_seeds_equal_unseeded_hash_join() {
    let d = data();
    let mut nonempty = 0;
    for (name, p) in shapes(&d) {
        let seeded = Q::new(&d.fx.store, 4, bind_join(outer(&d), p.clone(), &[0], JoinKind::Inner)).sorted(d.fx.rc());
        let joined = Q::new(&d.fx.store, 4, hash_join(p, outer(&d), &[0], &[], JoinKind::Inner, false)).sorted(d.fx.rc());
        assert_eq!(seeded, joined, "{name}");
        nonempty += usize::from(!joined.is_empty());
    }
    assert_eq!(nonempty, shapes(&d).len(), "every shape produces rows");
}

#[test]
fn substitution_equals_semi_join() {
    let d = data();
    for (name, p) in shapes(&d) {
        let ex = filter(outer(&d), Expr::Exists(ExistsId(0)));
        let substituted = Q::new(&d.fx.store, 4, ex).exists(vec![p.clone()]).sorted(d.fx.rc());
        let mut semi = Q::new(&d.fx.store, 4, hash_join(p, outer(&d), &[0], &[], JoinKind::Semi, false)).sorted(d.fx.rc());
        if name == "path-backward" {
            // A substituted endpoint is a constant: `?1 :p1* <nowhere>` matches <nowhere>
            // itself, although <nowhere> is not a node of the graph (unlike a join).
            semi.push(vec![d.fx.lookup("nowhere").unwrap(), u(), u(), u()]);
            semi.sort();
        }
        assert_eq!(substituted, semi, "{name}");
    }
}
