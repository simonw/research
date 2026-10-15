//! Property paths: ALP closures on cycles, zero-length rules, bag vs set semantics.
use sq_core::ir::{GraphSel, Logical, PathPlan};

use super::*;

fn pred(p: TermId) -> PathPlan {
    PathPlan::Pred(p)
}

fn star(p: PathPlan) -> PathPlan {
    PathPlan::ZeroOrMore(Box::new(p))
}

fn plus(p: PathPlan) -> PathPlan {
    PathPlan::OneOrMore(Box::new(p))
}

fn opt(p: PathPlan) -> PathPlan {
    PathPlan::ZeroOrOne(Box::new(p))
}

fn seq(a: PathPlan, b: PathPlan) -> PathPlan {
    PathPlan::Seq(Box::new(a), Box::new(b))
}

fn alt(a: PathPlan, b: PathPlan) -> PathPlan {
    PathPlan::Alt(Box::new(a), Box::new(b))
}

fn inv(a: PathPlan) -> PathPlan {
    PathPlan::Inv(Box::new(a))
}

fn path(s: PatTerm, p: PathPlan, o: PatTerm) -> Logical {
    Logical::Path { g: GraphSel::Default, s, path: p, o }
}

/// :a -> :b -> :c -> :a (a cycle on :p) and :c -> :d.
fn cycle() -> (Fx, [TermId; 5]) {
    let fx = Fx::new();
    let (a, b, cc, d, p) = (fx.iri("a"), fx.iri("b"), fx.iri("c"), fx.iri("d"), fx.iri("p"));
    fx.add(a, p, b);
    fx.add(b, p, cc);
    fx.add(cc, p, a);
    fx.add(cc, p, d);
    (fx, [a, b, cc, d, p])
}

#[test]
fn closures_terminate_on_cycles_with_set_semantics() {
    let (mut fx, [a, _, _, d, p]) = cycle();
    let y = fx.var("y");
    assert_eq!(fx.bag(&[y], path(c(a), plus(pred(p)), v(y))), bag(&[&[":a"], &[":b"], &[":c"], &[":d"]]));
    assert_eq!(fx.bag(&[y], path(c(a), star(pred(p)), v(y))), bag(&[&[":a"], &[":b"], &[":c"], &[":d"]]));
    // :d has no outgoing edge: + gives nothing, * gives :d itself.
    assert!(fx.select(&[y], path(c(d), plus(pred(p)), v(y))).is_empty());
    assert_eq!(fx.select(&[y], path(c(d), star(pred(p)), v(y))), rows(&[&[":d"]]));
    // Both endpoints fixed: one empty solution, not one per route.
    assert_eq!(fx.select(&[], path(c(a), plus(pred(p)), c(d))), rows(&[&[]]));
    assert!(fx.select(&[], path(c(d), plus(pred(p)), c(a))).is_empty());
}

#[test]
fn closure_with_a_fixed_object_walks_backwards() {
    let (mut fx, [a, _, _, d, p]) = cycle();
    let x = fx.var("x");
    assert_eq!(fx.bag(&[x], path(v(x), plus(pred(p)), c(d))), bag(&[&[":a"], &[":b"], &[":c"]]));
    assert_eq!(fx.bag(&[x], path(v(x), star(pred(p)), c(d))), bag(&[&[":a"], &[":b"], &[":c"], &[":d"]]));
    assert_eq!(fx.bag(&[x], path(v(x), plus(inv(pred(p))), c(a))), bag(&[&[":a"], &[":b"], &[":c"], &[":d"]]));
}

#[test]
fn zero_length_paths_match_constants_absent_from_the_graph() {
    let (mut fx, [a, _, _, _, p]) = cycle();
    let zz = fx.absent("zz");
    let y = fx.var("y");
    // zero_or_one_set_start: the constant matches itself.
    assert_eq!(fx.select(&[y], path(c(zz), star(pred(p)), v(y))), rows(&[&[":zz"]]));
    assert_eq!(fx.select(&[y], path(v(y), opt(pred(p)), c(zz))), rows(&[&[":zz"]]));
    assert_eq!(fx.select(&[], path(c(zz), opt(pred(p)), c(zz))), rows(&[&[]]));
    assert!(fx.select(&[], path(c(a), star(pred(p)), c(zz))).is_empty());
    // A predicate is not a node: :p* from free endpoints never binds :p.
    let x = fx.var("x");
    let all = fx.bag(&[x, y], path(v(x), star(pred(p)), v(y)));
    assert!(!all.iter().any(|r| r[0] == ":p"));
    // 4 nodes, all mutually reachable from :a, :b, :c (4 each) plus :d -> :d.
    assert_eq!(all.len(), 13);
}

#[test]
fn same_variable_on_both_ends() {
    let (mut fx, [_, _, _, _, p]) = cycle();
    let x = fx.var("x");
    // ?x :p+ ?x: the nodes on a cycle; ?x :p* ?x: every node.
    assert_eq!(fx.bag(&[x], path(v(x), plus(pred(p)), v(x))), bag(&[&[":a"], &[":b"], &[":c"]]));
    assert_eq!(fx.bag(&[x], path(v(x), star(pred(p)), v(x))), bag(&[&[":a"], &[":b"], &[":c"], &[":d"]]));
}

#[test]
fn sequence_and_alternative_keep_duplicates_but_zero_or_one_is_a_set() {
    let mut fx = Fx::new();
    let (a, m1, m2, z, p, q) = (fx.iri("a"), fx.iri("m1"), fx.iri("m2"), fx.iri("z"), fx.iri("p"), fx.iri("q"));
    fx.add(a, p, m1);
    fx.add(a, p, m2);
    fx.add(m1, q, z);
    fx.add(m2, q, z);
    let y = fx.var("y");
    // Two intermediate nodes: two solutions.
    assert_eq!(fx.select(&[y], path(c(a), seq(pred(p), pred(q)), v(y))), rows(&[&[":z"], &[":z"]]));
    assert_eq!(fx.select(&[], path(c(a), seq(pred(p), pred(q)), c(z))).len(), 2);
    // Union of two identical links: each edge twice.
    assert_eq!(fx.select(&[y], path(c(a), alt(pred(p), pred(p)), v(y))).len(), 4);
    // (p/q)? and (p/q)*: sets.
    assert_eq!(fx.bag(&[y], path(c(a), opt(seq(pred(p), pred(q))), v(y))), bag(&[&[":a"], &[":z"]]));
    assert_eq!(fx.bag(&[y], path(c(a), star(seq(pred(p), pred(q))), v(y))), bag(&[&[":a"], &[":z"]]));
}

#[test]
fn inverse_and_negated_property_sets() {
    let mut fx = Fx::new();
    let (a, b, p, q, r) = (fx.iri("a"), fx.iri("b"), fx.iri("p"), fx.iri("q"), fx.iri("r"));
    fx.add(a, p, b);
    fx.add(a, q, b);
    fx.add(b, r, a);
    let (x, y) = (fx.var("x"), fx.var("y"));
    assert_eq!(fx.select(&[x], path(v(x), inv(pred(p)), c(a))), rows(&[&[":b"]]));
    // !(:p): every other predicate, one solution per triple.
    assert_eq!(fx.bag(&[x, y], path(v(x), PathPlan::Nps(vec![p]), v(y))), bag(&[&[":a", ":b"], &[":b", ":a"]]));
    // !(^:r) arrives as Inv(Nps([:r])): reverse edges whose predicate is not :r.
    assert_eq!(
        fx.bag(&[x, y], path(v(x), inv(PathPlan::Nps(vec![r])), v(y))),
        bag(&[&[":b", ":a"], &[":b", ":a"]])
    );
    assert!(fx.select(&[], path(c(a), PathPlan::Nps(vec![p, q]), c(b))).is_empty());
}

#[test]
fn paths_never_cross_graphs() {
    let mut fx = Fx::new();
    let (g1, g2, a, b, cc, p) = (fx.iri("g1"), fx.iri("g2"), fx.iri("a"), fx.iri("b"), fx.iri("c"), fx.iri("p"));
    fx.add_in(g1, a, p, b);
    fx.add_in(g2, b, p, cc);
    let (g, y) = (fx.var("g"), fx.var("y"));
    let inner = Logical::Path { g: GraphSel::Active, s: c(a), path: plus(pred(p)), o: v(y) };
    let root = Logical::Graph { name: v(g), input: Box::new(inner) };
    assert_eq!(fx.select(&[g, y], root), rows(&[&[":g1", ":b"]]));
    // The default graph is empty here; a zero-length path still matches the constant.
    assert_eq!(fx.select(&[y], path(c(a), star(pred(p)), v(y))), rows(&[&[":a"]]));
}
