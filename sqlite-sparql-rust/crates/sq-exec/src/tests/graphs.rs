//! GraphLoop (the graph register), GraphNames and `GraphSel::Active`.
use super::*;
use sq_core::ir::{DefaultGraph, NamedGraphs};

/// g1 {a p b, b p c}; g2 {a p c}; default {a p a}.
fn data() -> (Fx, [TermId; 6]) {
    let fx = Fx::new();
    let [a, b, cc, p, g1, g2] = ["a", "b", "c", "p", "g1", "g2"].map(|n| fx.iri(n));
    fx.add_g(g1, a, p, b);
    fx.add_g(g1, b, p, cc);
    fx.add_g(g2, a, p, cc);
    fx.add(a, p, a);
    (fx, [a, b, cc, p, g1, g2])
}

fn graph_loop(slot: Slot, input: Plan) -> Plan {
    plan(Op::GraphLoop { slot, input: Box::new(input) })
}

fn active(s: PatTerm, p: PatTerm, o: PatTerm) -> Plan {
    scan_bound(GraphSel::Active, s, p, o, &[])
}

#[test]
fn graph_loop_binds_each_named_graph() {
    let (fx, [a, b, cc, p, g1, g2]) = data();
    let rows = Q::new(&fx.store, 3, graph_loop(2, active(v(0), c(p), v(1)))).rows(fx.rc());
    assert_eq!(rows, vec![vec![a, b, g1], vec![b, cc, g1], vec![a, cc, g2]], "graphs in sorted order");
    // An empty pattern inside GRAPH ?g still binds every named graph (via Unit).
    let rows = Q::new(&fx.store, 1, graph_loop(0, plan(Op::Unit))).rows(fx.rc());
    assert_eq!(rows, vec![vec![g1], vec![g2]]);
}

#[test]
fn graph_loop_drops_rows_bound_to_another_graph() {
    let (fx, [_, _, _, _, g1, g2]) = data();
    // GRAPH ?g { VALUES ?g { g2 } }
    let rows = Q::new(&fx.store, 1, graph_loop(0, values(&[0], vec![vec![g2]]))).rows(fx.rc());
    assert_eq!(rows, vec![vec![g2]]);
    let other = fx.iri("other");
    let rows = Q::new(&fx.store, 1, graph_loop(0, values(&[0], vec![vec![other], vec![u()]]))).rows(fx.rc());
    assert_eq!(rows, vec![vec![g1], vec![g2]], "UNDEF takes the graph; another IRI is dropped");
}

#[test]
fn register_is_visible_to_lazily_opened_subtrees() {
    let (fx, [a, b, cc, p, g1, g2]) = data();
    // GRAPH ?g { ?x :p ?y . ?y :p ?z } as a BindJoin: the right side is opened per probe.
    let inner = bind_join(
        active(v(0), c(p), v(1)),
        scan_bound(GraphSel::Active, v(1), c(p), v(2), &[1]),
        &[1],
        JoinKind::Inner,
    );
    let rows = Q::new(&fx.store, 4, graph_loop(3, inner)).rows(fx.rc());
    assert_eq!(rows, vec![vec![a, b, cc, g1]]);
    // A Union inside the loop opens its branches lazily too.
    let u2 = union(vec![active(v(0), c(p), v(1)), active(v(1), c(p), v(0))]);
    let rows = Q::new(&fx.store, 3, graph_loop(2, u2)).rows(fx.rc());
    assert_eq!(rows.len(), 6);
    assert!(rows.iter().all(|r| r[2] == g1 || r[2] == g2));
    // Nested GRAPH loops: the innermost register wins.
    let nested = graph_loop(2, graph_loop(3, active(v(0), c(p), v(1))));
    let rows = Q::new(&fx.store, 4, nested).rows(fx.rc());
    assert_eq!(rows.len(), 2 * 3, "outer x inner graphs");
    assert!(rows.iter().all(|r| (r[3] == g1) == (r[0] == b || r[1] == b)));
}

#[test]
fn graph_loop_evaluates_minus_and_limits_per_graph() {
    let (fx, [a, _, cc, p, g1, _]) = data();
    // GRAPH ?g { ?x :p ?y LIMIT 1 }: the limit applies per graph.
    let rows = Q::new(&fx.store, 3, graph_loop(2, slice(active(v(0), c(p), v(1)), 0, Some(1)))).rows(fx.rc());
    assert_eq!(rows.len(), 2);
    // GRAPH ?g { ?x :p ?y MINUS { ?x :p :c } }: in g1 a has no edge to c, in g2 it has.
    let minus = plan(Op::Minus {
        left: Box::new(active(v(0), c(p), v(1))),
        right: Box::new(active(v(0), c(p), c(cc))),
        keys: vec![0],
        check: vec![],
    });
    let rows = Q::new(&fx.store, 3, graph_loop(2, minus)).sorted(fx.rc());
    let b = fx.lookup("b").unwrap();
    assert_eq!(rows, ms(vec![vec![a, b, g1]]), "b p c in g1 is removed (b p c), a p c in g2 too");
}

#[test]
fn graph_names() {
    let (fx, [_, _, _, _, g1, g2]) = data();
    let names = plan(Op::GraphNames { slot: 0 });
    assert_eq!(Q::new(&fx.store, 1, names.clone()).rows(fx.rc()), vec![vec![g1], vec![g2]]);
    // FROM NAMED with an absent graph: it is a (empty) graph of the dataset.
    let mut temps = TempDict::new();
    let absent = temps.intern(Term::Iri(format!("{EX}absent").into()));
    let mut list = vec![g2, absent];
    list.sort();
    let mut e = Q::new(&fx.store, 1, names.clone())
        .temps(temps)
        .dataset(DefaultGraph::Graphs(Arc::from(vec![])), NamedGraphs::Only(Arc::from(list.clone())), list)
        .exec(fx.rc());
    let rows = collect(&mut e).unwrap();
    assert_eq!(rows.len(), 2);
    let mut shown: Vec<String> = rows.iter().flat_map(|r| show(&mut e, r)).collect();
    shown.sort();
    assert_eq!(shown, vec![format!("<{EX}absent>"), format!("<{EX}g2>")]);
    // Seeded: only a matching named graph.
    let rows = Q::new(&fx.store, 1, bind_join(values(&[0], vec![vec![g2], vec![fx.iri("x")]]), names, &[0], JoinKind::Inner))
        .rows(fx.rc());
    assert_eq!(rows, vec![vec![g2]]);
    let mut e = Q::new(&fx.store, 1, plan(Op::GraphNames { slot: 0 })).exec(fx.rc());
    assert_eq!(e.count().unwrap(), 2);
}

#[test]
fn active_outside_a_graph_loop_is_an_internal_error() {
    let (fx, [_, _, _, p, ..]) = data();
    let mut e = Q::new(&fx.store, 2, active(v(0), c(p), v(1))).exec(fx.rc());
    assert!(matches!(collect(&mut e), Err(Error::Internal(_))));
}
