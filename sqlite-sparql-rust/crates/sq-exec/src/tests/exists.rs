//! Correlated EXISTS: substitution semantics (OPTIONAL, MINUS, FILTER, GRAPH), memoization,
//! nesting, and EXISTS in BIND.
use super::*;
use sq_core::ir::ExistsId;

fn exists(id: u32) -> Expr {
    Expr::Exists(ExistsId(id))
}

fn not(e: Expr) -> Expr {
    Expr::Not(Box::new(e))
}

/// a p b, b p c, c q d; a r 1.
fn data() -> (Fx, [TermId; 7]) {
    let fx = Fx::new();
    let [a, b, cc, d, p, q, r] = ["a", "b", "c", "d", "p", "q", "r"].map(|n| fx.iri(n));
    fx.add(a, p, b);
    fx.add(b, p, cc);
    fx.add(cc, q, d);
    fx.add(a, r, int(1));
    (fx, [a, b, cc, d, p, q, r])
}

#[test]
fn filter_exists_and_not_exists() {
    let (fx, [a, b, cc, d, p, ..]) = data();
    let subjects = values(&[0], vec![vec![a], vec![b], vec![cc], vec![d]]);
    // FILTER EXISTS { ?0 :p ?1 }
    let ex = scan(v(0), c(p), v(1));
    let rows = Q::new(&fx.store, 2, filter(subjects.clone(), exists(0))).exists(vec![ex.clone()]).rows(fx.rc());
    assert_eq!(rows, vec![vec![a, u()], vec![b, u()]]);
    let rows = Q::new(&fx.store, 2, filter(subjects, not(exists(0)))).exists(vec![ex]).rows(fx.rc());
    assert_eq!(rows, vec![vec![cc, u()], vec![d, u()]]);
}

#[test]
fn exists_is_memoized_per_correlated_tuple() {
    let (fx, [a, b, cc, _, p, ..]) = data();
    let rows: Vec<Vec<TermId>> = [a, b, a, cc, a, b].iter().map(|x| vec![*x, int(0)]).collect();
    let p0 = filter(values(&[0, 2], rows), exists(0));
    // The pattern mentions ?0 and ?1 only: ?2 (different per row or not) is not part of the key.
    let mut e = Q::new(&fx.store, 3, p0).exists(vec![scan(v(0), c(p), v(1))]).exec(fx.rc());
    assert_eq!(collect(&mut e).unwrap().len(), 5);
    // Node ids: 0 Filter, 1 Values, 2 the EXISTS scan.
    assert_eq!(e.profile().nodes[2].opens, 3, "one evaluation per distinct ?0");
}

#[test]
fn substitution_reaches_filters_and_optionals() {
    let (fx, [a, b, cc, d, p, q, _]) = data();
    let subjects = || values(&[0], vec![vec![a], vec![b], vec![cc], vec![d]]);
    // EXISTS { FILTER(?0 = :b) }: the outer ?0 is a constant inside.
    let ex = filter(plan(Op::Unit), Expr::SameTerm(Box::new(sv(0)), Box::new(cv(b))));
    let rows = Q::new(&fx.store, 1, filter(subjects(), exists(0))).exists(vec![ex]).rows(fx.rc());
    assert_eq!(rows, vec![vec![b]]);
    // EXISTS { ?1 :p ?2 OPTIONAL { ?2 :q ?0 } FILTER(BOUND(?0)) }: with substitution ?0 is
    // bound everywhere inside; the OPTIONAL's right side is substituted too.
    let opt = hash_join(scan(v(2), c(q), v(0)), scan(v(1), c(p), v(2)), &[2], &[0], left(None), false);
    let ex = filter(opt, Expr::Bound(0));
    let rows = Q::new(&fx.store, 3, filter(subjects(), exists(0))).exists(vec![ex]).rows(fx.rc());
    assert_eq!(rows, vec![vec![a, u(), u()], vec![b, u(), u()], vec![cc, u(), u()], vec![d, u(), u()]]);
    // EXISTS { ?1 :p ?2 . ?2 :q ?0 }: only d (b p c, c q d).
    let ex = bind_join(scan(v(1), c(p), v(2)), scan_bound(GraphSel::Default, v(2), c(q), v(0), &[2]), &[2], JoinKind::Inner);
    let rows = Q::new(&fx.store, 3, filter(subjects(), exists(0))).exists(vec![ex]).rows(fx.rc());
    assert_eq!(rows, vec![vec![d, u(), u()]]);
}

#[test]
fn substituted_slots_are_not_minus_domain() {
    let (fx, [a, b, cc, _, p, _, r]) = data();
    // EXISTS { ?0 :p ?1 MINUS { ?0 :r ?2 } }: ?0 is a constant in both sides, so the MINUS
    // shares no variable and removes nothing (a still qualifies although a r 1).
    let minus = plan(Op::Minus {
        left: Box::new(scan(v(0), c(p), v(1))),
        right: Box::new(scan(v(0), c(r), v(2))),
        keys: vec![0],
        check: vec![],
    });
    let subjects = values(&[0], vec![vec![a], vec![b], vec![cc]]);
    let rows = Q::new(&fx.store, 3, filter(subjects, exists(0))).exists(vec![minus.clone()]).rows(fx.rc());
    assert_eq!(rows, vec![vec![a, u(), u()], vec![b, u(), u()]]);
    // Without substitution the same MINUS removes a.
    let rows = Q::new(&fx.store, 3, minus).sorted(fx.rc());
    assert_eq!(rows, ms(vec![vec![b, cc, u()]]));
}

#[test]
fn exists_inside_graph_loop_uses_the_active_graph() {
    let fx = Fx::new();
    let [a, b, p, g1, g2] = ["a", "b", "p", "g1", "g2"].map(|n| fx.iri(n));
    fx.add_g(g1, a, p, b);
    fx.add_g(g2, b, p, a);
    // GRAPH ?2 { VALUES ?0 {a b} FILTER EXISTS { ?0 :p ?1 } }
    let inner = filter(values(&[0], vec![vec![a], vec![b]]), exists(0));
    let gl = plan(Op::GraphLoop { slot: 2, input: Box::new(inner) });
    let ex = scan_bound(GraphSel::Active, v(0), c(p), v(1), &[]);
    let rows = Q::new(&fx.store, 3, gl).exists(vec![ex]).rows(fx.rc());
    assert_eq!(rows, vec![vec![a, u(), g1], vec![b, u(), g2]], "memo keyed by the active graph too");
    // EXISTS { GRAPH ?2 { ?0 :p ?1 } } with ?2 substituted: only that graph.
    let ex = plan(Op::GraphLoop { slot: 2, input: Box::new(scan_bound(GraphSel::Active, v(0), c(p), v(1), &[])) });
    let outer = values(&[0, 2], vec![vec![a, g1], vec![a, g2], vec![b, g2]]);
    let rows = Q::new(&fx.store, 3, filter(outer, exists(0))).exists(vec![ex]).rows(fx.rc());
    assert_eq!(rows, vec![vec![a, u(), g1], vec![b, u(), g2]]);
}

#[test]
fn nested_exists_and_exists_in_bind() {
    let (fx, [a, b, cc, d, p, q, _]) = data();
    // FILTER EXISTS { ?0 :p ?1 FILTER EXISTS { ?1 :q ?2 } }: b (b p c, c q d).
    let ex0 = filter(scan(v(0), c(p), v(1)), exists(1));
    let ex1 = scan(v(1), c(q), v(2));
    let subjects = values(&[0], vec![vec![a], vec![b], vec![cc], vec![d]]);
    let rows = Q::new(&fx.store, 4, filter(subjects.clone(), exists(0))).exists(vec![ex0, ex1]).rows(fx.rc());
    assert_eq!(rows, vec![vec![b, u(), u(), u()]]);
    // BIND(EXISTS { ?0 :p ?1 } AS ?3)
    let rows = Q::new(&fx.store, 4, extend(subjects, 3, exists(0))).exists(vec![scan(v(0), c(p), v(1))]).rows(fx.rc());
    let flags: Vec<TermId> = rows.iter().map(|r| r[3]).collect();
    let (t, f) = (TermId::from_bool(true), TermId::from_bool(false));
    assert_eq!(flags, vec![t, t, f, f]);
}

#[test]
fn exists_rows_must_agree_with_rebound_substituted_slots() {
    let (fx, [a, b, ..]) = data();
    // EXISTS { BIND(:b AS ?0) }: the pattern rebinds the substituted ?0; only ?0 = :b agrees.
    let ex = extend(plan(Op::Unit), 0, cv(b));
    let rows = Q::new(&fx.store, 1, filter(values(&[0], vec![vec![a], vec![b]]), exists(0))).exists(vec![ex]).rows(fx.rc());
    assert_eq!(rows, vec![vec![b]]);
}
