//! EXISTS / NOT EXISTS by substitution (§18.6): the current solution's values are constants
//! everywhere in the pattern.
use sq_core::ir::{AggFunc, AggSpec, Logical, PathPlan};
use sq_core::TermId;

use super::*;

#[test]
fn exists_and_not_exists_substitute_the_current_solution() {
    let mut fx = Fx::new();
    let (a, b, p, q) = (fx.iri("a"), fx.iri("b"), fx.iri("p"), fx.iri("q"));
    fx.add(a, p, fx.int(1));
    fx.add(b, p, fx.int(2));
    fx.add(a, q, fx.int(3));
    let (s, o, z) = (fx.var("s"), fx.var("o"), fx.var("z"));
    let has_q = fx.exists(bgp(vec![tp(v(s), c(q), v(z))]));
    let pat = || bgp(vec![tp(v(s), c(p), v(o))]);
    assert_eq!(fx.select(&[s], filter(pat(), has_q.clone())), rows(&[&[":a"]]));
    assert_eq!(fx.select(&[s], filter(pat(), not(has_q))), rows(&[&[":b"]]));
    // Variables of the EXISTS pattern do not leak into the solution.
    let leak = fx.exists(bgp(vec![tp(v(s), c(q), v(z))]));
    assert_eq!(fx.select(&[s, z], filter(pat(), leak)), rows(&[&[":a", "-"]]));
}

#[test]
fn minus_inside_exists_ignores_substituted_variables() {
    // ?x :knows ?y MINUS { ?x :age ?a } with ?x = :a substituted: ?x is a constant on both
    // sides, the domains share nothing, and MINUS removes nothing (docs/SPEC.md, semi joins).
    let mut fx = Fx::new();
    let (a, knows, age, b) = (fx.iri("a"), fx.iri("knows"), fx.iri("age"), fx.iri("b"));
    fx.add(a, knows, b);
    fx.add(a, age, fx.int(3));
    let (x, y, ag) = (fx.var("x"), fx.var("y"), fx.var("ag"));
    let minus = || Logical::Minus {
        left: Box::new(bgp(vec![tp(v(x), c(knows), v(y))])),
        right: Box::new(bgp(vec![tp(v(x), c(age), v(ag))])),
    };
    let ex = fx.exists(minus());
    assert_eq!(fx.select(&[x], filter(values(&[x], &[&[a]]), ex)), rows(&[&[":a"]]));
    // Evaluated on its own, the same MINUS removes :a.
    assert!(fx.select(&[x], minus()).is_empty());
}

#[test]
fn exists_in_an_optional_filter_sees_the_merged_solution() {
    let mut fx = Fx::new();
    let (a, b, p, q, ok) = (fx.iri("a"), fx.iri("b"), fx.iri("p"), fx.iri("q"), fx.iri("ok"));
    fx.add(a, p, fx.int(1));
    fx.add(b, p, fx.int(1));
    fx.add(a, q, fx.int(7));
    fx.add(b, q, fx.int(8));
    fx.add(fx.int(8), ok, fx.int(1));
    let (s, o, r, z) = (fx.var("s"), fx.var("o"), fx.var("r"), fx.var("z"));
    // OPTIONAL { ?s :q ?r FILTER EXISTS { ?r :ok ?z } }: ?r comes from the right side.
    let ex = fx.exists(bgp(vec![tp(v(r), c(ok), v(z))]));
    let root = Logical::LeftJoin {
        left: Box::new(bgp(vec![tp(v(s), c(p), v(o))])),
        right: Box::new(bgp(vec![tp(v(s), c(q), v(r))])),
        filter: Some(ex),
    };
    assert_eq!(fx.bag(&[s, r], root), bag(&[&[":a", "-"], &[":b", &int(8)]]));
}

#[test]
fn exists_reads_the_active_graph() {
    let mut fx = Fx::new();
    let (g1, g2, a, p, q) = (fx.iri("g1"), fx.iri("g2"), fx.iri("a"), fx.iri("p"), fx.iri("q"));
    fx.add_in(g1, a, p, fx.int(1));
    fx.add_in(g2, a, p, fx.int(2));
    fx.add_in(g2, a, q, fx.int(3));
    fx.add(a, q, fx.int(4)); // default graph: invisible inside GRAPH
    let (g, s, o, z) = (fx.var("g"), fx.var("s"), fx.var("o"), fx.var("z"));
    let ex = fx.exists(bgp(vec![tpa(v(s), c(q), v(z))]));
    let root = Logical::Graph { name: v(g), input: Box::new(filter(bgp(vec![tpa(v(s), c(p), v(o))]), ex)) };
    assert_eq!(fx.select(&[g, o], root), rows(&[&[":g2", &int(2)]]));

    // A substituted graph variable selects one graph inside EXISTS.
    let inner = Logical::Graph { name: v(g), input: Box::new(bgp(vec![tpa(c(a), c(q), v(z))])) };
    let ex = fx.exists(inner);
    let root = filter(values(&[g], &[&[g1], &[g2]]), ex);
    assert_eq!(fx.select(&[g], root), rows(&[&[":g2"]]));
}

#[test]
fn nested_exists_sees_the_outer_substitution() {
    let mut fx = Fx::new();
    let (a, b, p, q, r) = (fx.iri("a"), fx.iri("b"), fx.iri("p"), fx.iri("q"), fx.iri("r"));
    fx.add(a, p, b);
    fx.add(b, q, a);
    fx.add(b, r, b);
    let (x, y, z) = (fx.var("x"), fx.var("y"), fx.var("z"));
    // ?x :p ?y FILTER EXISTS { ?y :q ?z FILTER EXISTS { ?y :r ?y . ?z :p ?y } }
    let innermost = fx.exists(bgp(vec![tp(v(y), c(r), v(y)), tp(v(z), c(p), v(y))]));
    let middle = fx.exists(filter(bgp(vec![tp(v(y), c(q), v(z))]), innermost));
    assert_eq!(fx.select(&[x, y], filter(bgp(vec![tp(v(x), c(p), v(y))]), middle)), rows(&[&[":a", ":b"]]));
}

#[test]
fn substituted_path_endpoints_are_constants() {
    // VALUES ?x { :zz } with :zz absent from the graph.
    let mut fx = Fx::new();
    let (a, p) = (fx.iri("a"), fx.iri("p"));
    fx.add(a, p, a);
    let zz = fx.absent("zz");
    let (x, y) = (fx.var("x"), fx.var("y"));
    let path = || Logical::Path { g: sq_core::ir::GraphSel::Default, s: v(x), path: PathPlan::ZeroOrMore(Box::new(PathPlan::Pred(p))), o: v(y) };
    // Joined (values_and_path): ?x :p* ?y with both ends free only matches graph nodes.
    assert!(fx.select(&[x, y], join(values(&[x], &[&[zz]]), path())).is_empty());
    // Substituted: the zero-length path matches the constant :zz itself.
    let ex = fx.exists(path());
    assert_eq!(fx.select(&[x], filter(values(&[x], &[&[zz]]), ex)), rows(&[&[":zz"]]));
}

#[test]
fn bind_and_values_on_a_substituted_variable_constrain_it() {
    let mut fx = Fx::new();
    let (a, b) = (fx.iri("a"), fx.iri("b"));
    let x = fx.var("x");
    // EXISTS { BIND(:a AS ?x) } is true only where ?x = :a (or the value is an error).
    let bind_a = fx.exists(extend(Logical::Unit, x, ec(a)));
    let outer = || values(&[x], &[&[a], &[b]]);
    assert_eq!(fx.select(&[x], filter(outer(), bind_a)), rows(&[&[":a"]]));
    let bind_err = fx.exists(extend(Logical::Unit, x, add(ec(a), ec(fx.int(1)))));
    assert_eq!(fx.select(&[x], filter(outer(), bind_err)).len(), 2);
    // EXISTS { VALUES ?x { :b UNDEF } }: compatible rows only.
    let vals = fx.exists(values(&[x], &[&[b]]));
    assert_eq!(fx.select(&[x], filter(outer(), vals)), rows(&[&[":b"]]));
    let undef = fx.exists(values(&[x], &[&[TermId::UNBOUND]]));
    assert_eq!(fx.select(&[x], filter(outer(), undef)).len(), 2);
}

#[test]
fn substitution_reaches_into_groups_and_their_having() {
    let mut fx = Fx::new();
    let (a, b, p) = (fx.iri("a"), fx.iri("b"), fx.iri("p"));
    fx.add(a, p, fx.int(1));
    fx.add(a, p, fx.int(2));
    fx.add(b, p, fx.int(1));
    let (x, o, n) = (fx.var("x"), fx.var("o"), fx.var("n"));
    let count = || Logical::Group {
        input: Box::new(bgp(vec![tp(v(x), c(p), v(o))])),
        keys: vec![],
        aggs: vec![(n, AggSpec { func: AggFunc::CountAll, distinct: false, arg: None })],
    };
    let outer = || values(&[x], &[&[a], &[b]]);
    // EXISTS { SELECT (COUNT(*) AS ?n) { ?x :p ?o } HAVING (?n > 1) }: counted per ?x.
    let many = fx.exists(filter(count(), gt(es(n), ec(fx.int(1)))));
    assert_eq!(fx.select(&[x], filter(outer(), many)), rows(&[&[":a"]]));
    // The substituted ?x stays a constant above the group.
    let is_b = fx.exists(filter(count(), eq(es(x), ec(b))));
    assert_eq!(fx.select(&[x], filter(outer(), is_b)), rows(&[&[":b"]]));
}
