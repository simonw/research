//! Datasets and GRAPH: the default graph as an RDF merge, named graphs, per-graph evaluation.
use sq_core::ir::{AggFunc, AggSpec, DefaultGraph, GraphSel, Logical, NamedGraphs, PathPlan, QuadPat};
use sq_core::TermId;

use super::*;

/// Default graph :a :p 0; graph :g1 holds :a :p 1 and :b :p 1; graph :g2 holds :a :p 2.
fn three_graphs() -> (Fx, [TermId; 4]) {
    let fx = Fx::new();
    let (g1, g2, a, b, p) = (fx.iri("g1"), fx.iri("g2"), fx.iri("a"), fx.iri("b"), fx.iri("p"));
    fx.add(a, p, fx.int(0));
    fx.add_in(g1, a, p, fx.int(1));
    fx.add_in(g1, b, p, fx.int(1));
    fx.add_in(g2, a, p, fx.int(2));
    (fx, [g1, g2, a, p])
}

fn graph(name: PatTerm, input: Logical) -> Logical {
    Logical::Graph { name, input: Box::new(input) }
}

#[test]
fn graph_variable_ranges_over_the_named_graphs_only() {
    let (mut fx, [_, _, _, p]) = three_graphs();
    let (g, s, o) = (fx.var("g"), fx.var("s"), fx.var("o"));
    let root = graph(v(g), bgp(vec![tpa(v(s), c(p), v(o))]));
    assert_eq!(
        fx.bag(&[g, s, o], root),
        bag(&[&[":g1", ":a", &int(1)], &[":g1", ":b", &int(1)], &[":g2", ":a", &int(2)]])
    );
    // Outside GRAPH the default graph is read.
    assert_eq!(fx.select(&[o], bgp(vec![tp(v(s), c(p), v(o))])), rows(&[&[&int(0)]]));
}

#[test]
fn graph_constant_must_name_a_graph_of_the_dataset() {
    let (mut fx, [g1, g2, a, p]) = three_graphs();
    let o = fx.var("o");
    let inner = || bgp(vec![tpa(c(a), c(p), v(o))]);
    assert_eq!(fx.select(&[o], graph(c(g1), inner())), rows(&[&[&int(1)]]));
    // The default graph and unknown graphs are not named graphs: no solution, not even for {}.
    assert!(fx.select(&[o], graph(c(TermId::DEFAULT_GRAPH), Logical::Unit)).is_empty());
    let nowhere = fx.absent("nowhere");
    assert!(fx.select(&[], graph(c(nowhere), Logical::Unit)).is_empty());
    assert_eq!(fx.select(&[], graph(c(g2), Logical::Unit)), rows(&[&[]]));

    // FROM NAMED <g2> only: g1 is not part of the dataset.
    fx.set_dataset(DefaultGraph::Graphs(arc_dataset(&[])), NamedGraphs::Only(arc_dataset(&[g2])));
    assert!(fx.select(&[o], graph(c(g1), inner())).is_empty());
    assert_eq!(fx.select(&[o], graph(c(g2), inner())), rows(&[&[&int(2)]]));
}

#[test]
fn graph_names_absent_from_the_store_are_empty_named_graphs() {
    let (mut fx, [g1, ..]) = three_graphs();
    let g = fx.var("g");
    let nowhere = fx.absent("nowhere");
    fx.set_dataset(DefaultGraph::Graphs(arc_dataset(&[])), NamedGraphs::Only(arc_dataset(&[g1, nowhere])));
    // GRAPH ?g {} has one solution per named graph of the dataset.
    assert_eq!(fx.bag(&[g], graph(v(g), Logical::Unit)), bag(&[&[":g1"], &[":nowhere"]]));
}

#[test]
fn graph_joins_the_name_with_variables_bound_inside() {
    let mut fx = Fx::new();
    let (g1, g2, p) = (fx.iri("g1"), fx.iri("g2"), fx.iri("p"));
    // In g1 the subject is the graph's own name; in g2 it is not.
    fx.add_in(g1, g1, p, fx.int(1));
    fx.add_in(g2, g1, p, fx.int(2));
    let (g, o) = (fx.var("g"), fx.var("o"));
    let root = graph(v(g), bgp(vec![tpa(v(g), c(p), v(o))]));
    assert_eq!(fx.select(&[g, o], root), rows(&[&[":g1", &int(1)]]));
}

#[test]
fn graph_evaluates_the_whole_inner_pattern_once_per_graph() {
    let (mut fx, [_, _, a, p]) = three_graphs();
    let q = fx.iri("q");
    let (g, s, o, x) = (fx.var("g"), fx.var("s"), fx.var("o"), fx.var("x"));
    // VALUES inside GRAPH ?g: one solution per named graph.
    let vals = graph(v(g), values(&[x], &[&[fx.int(5)]]));
    assert_eq!(fx.bag(&[g, x], vals), bag(&[&[":g1", &int(5)], &[":g2", &int(5)]]));
    // MINUS inside GRAPH reads the same graph on both sides (graph-minus).
    fx.add_in(fx.iri("g2"), a, q, fx.int(9));
    let minus = graph(
        v(g),
        Logical::Minus {
            left: Box::new(bgp(vec![tpa(v(s), c(p), v(o))])),
            right: Box::new(bgp(vec![tpa(v(s), c(q), v(x))])),
        },
    );
    assert_eq!(fx.bag(&[g, s], minus), bag(&[&[":g1", ":a"], &[":g1", ":b"]]));
}

#[test]
fn default_graph_of_several_graphs_is_their_merge() {
    let (mut fx, [g1, g2, a, p]) = three_graphs();
    let (s, o) = (fx.var("s"), fx.var("o"));
    fx.add_in(g2, fx.iri("b"), p, fx.int(1)); // also in g1: merged once
    fx.set_dataset(DefaultGraph::Graphs(arc_dataset(&[g1, g2])), NamedGraphs::Only(arc_dataset(&[])));
    assert_eq!(
        fx.bag(&[s, o], bgp(vec![tp(v(s), c(p), v(o))])),
        bag(&[&[":a", &int(1)], &[":a", &int(2)], &[":b", &int(1)]])
    );
    // FROM NAMED only: the default graph is empty.
    fx.set_dataset(DefaultGraph::Graphs(arc_dataset(&[])), NamedGraphs::Only(arc_dataset(&[g1])));
    assert!(fx.select(&[s], bgp(vec![tp(v(s), c(p), v(o))])).is_empty());
    // union_default_graph: g=0 and every named graph.
    fx.set_dataset(DefaultGraph::UnionAll, NamedGraphs::All);
    assert_eq!(fx.select(&[s], bgp(vec![tp(v(s), c(p), v(o))])).len(), 4);
    assert_eq!(fx.select(&[], bgp(vec![tp(c(a), c(p), c(fx.int(2)))])).len(), 1);
}

#[test]
fn physical_graph_selectors_are_supported() {
    let (mut fx, [g1, _, a, p]) = three_graphs();
    let (g, o, y) = (fx.var("g"), fx.var("o"), fx.var("y"));
    // GraphSel::Const reads one graph; GraphSel::Slot ranges over the named graphs.
    let one = bgp(vec![QuadPat { g: GraphSel::Const(g1), s: c(a), p: c(p), o: v(o) }]);
    assert_eq!(fx.select(&[o], one), rows(&[&[&int(1)]]));
    let each = bgp(vec![QuadPat { g: GraphSel::Slot(g), s: c(a), p: c(p), o: v(o) }]);
    assert_eq!(fx.bag(&[g, o], each), bag(&[&[":g1", &int(1)], &[":g2", &int(2)]]));
    let path = Logical::Path { g: GraphSel::Slot(g), s: c(a), path: PathPlan::Pred(p), o: v(y) };
    assert_eq!(fx.bag(&[g, y], path), bag(&[&[":g1", &int(1)], &[":g2", &int(2)]]));
}

#[test]
fn aggregates_inside_graph_are_computed_per_graph() {
    let (mut fx, [_, _, _, p]) = three_graphs();
    let q = fx.iri("q");
    let (g, s, o, n) = (fx.var("g"), fx.var("s"), fx.var("o"), fx.var("n"));
    let count_in = |pattern: Logical| Logical::Group {
        input: Box::new(pattern),
        keys: vec![],
        aggs: vec![(n, AggSpec { func: AggFunc::CountAll, distinct: false, arg: None })],
    };
    let grouped = count_in(bgp(vec![tpa(v(s), c(p), v(o))]));
    // An empty-group COUNT inside GRAPH ?g is 0 for a graph without matches, and is computed
    // in every named graph (agg-empty-group-count-graph).
    let empty = count_in(bgp(vec![tpa(v(s), c(q), v(o))]));
    assert_eq!(fx.bag(&[g, n], graph(v(g), grouped)), bag(&[&[":g1", &int(2)], &[":g2", &int(1)]]));
    assert_eq!(fx.bag(&[g, n], graph(v(g), empty)), bag(&[&[":g1", &int(0)], &[":g2", &int(0)]]));
}
