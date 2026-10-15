//! Graph pattern operators: BGP, Join, LeftJoin, Minus, Union, Filter, Extend, Values,
//! OrderBy, Project, Distinct, Reduced, Slice, Service.
use sq_core::ir::{Logical, QueryForm};
use sq_core::spargebra::algebra::Function;
use sq_core::{Error, TermId};

use super::*;

#[test]
fn bgp_matches_constants_variables_and_repeated_variables() {
    let mut fx = Fx::new();
    let (a, b, p, q) = (fx.iri("a"), fx.iri("b"), fx.iri("p"), fx.iri("q"));
    fx.add(a, p, b);
    fx.add(b, p, b);
    fx.add(a, q, fx.lit("x"));
    let (x, y) = (fx.var("x"), fx.var("y"));

    assert_eq!(fx.bag(&[x, y], bgp(vec![tp(v(x), c(p), v(y))])), bag(&[&[":a", ":b"], &[":b", ":b"]]));
    // ?x :p ?x
    assert_eq!(fx.select(&[x], bgp(vec![tp(v(x), c(p), v(x))])), rows(&[&[":b"]]));
    // Variable predicate, then a join through the shared ?x.
    assert_eq!(
        fx.bag(&[x, y], bgp(vec![tp(v(x), v(y), c(b)), tp(v(x), c(q), c(fx.lit("x")))])),
        bag(&[&[":a", ":p"]])
    );
    // Ground patterns: present gives one empty solution, absent none, TEMP never matches.
    assert_eq!(fx.select(&[], bgp(vec![tp(c(a), c(p), c(b))])), rows(&[&[]]));
    assert_eq!(fx.select(&[], bgp(vec![tp(c(b), c(p), c(a))])), Vec::<Vec<String>>::new());
    let zz = fx.absent("zz");
    assert!(fx.select(&[x], bgp(vec![tp(v(x), c(p), c(zz))])).is_empty());
}

#[test]
fn blank_node_variables_are_hidden_but_keep_multiplicity() {
    let mut fx = Fx::new();
    let (a, p) = (fx.iri("a"), fx.iri("p"));
    fx.add(a, p, fx.bnode());
    fx.add(a, p, fx.bnode());
    let (s, b) = (fx.var("s"), fx.hidden("b"));
    // SELECT ?s { ?s :p [] }: one solution per blank-node mapping.
    let root = project(bgp(vec![tp(v(s), c(p), v(b))]), &[s]);
    assert_eq!(fx.select(&[s], root), rows(&[&[":a"], &[":a"]]));
}

#[test]
fn unit_and_empty() {
    let fx = Fx::new();
    assert_eq!(fx.select(&[], Logical::Unit), rows(&[&[]]));
    assert!(fx.select(&[], Logical::Empty).is_empty());
    assert!(fx.ask(Logical::Unit));
    assert!(!fx.ask(Logical::Empty));
}

#[test]
fn join_merges_compatible_solutions_and_treats_unbound_as_wildcard() {
    let mut fx = Fx::new();
    let (x, y) = (fx.var("x"), fx.var("y"));
    let (one, two, three) = (fx.int(1), fx.int(2), fx.int(3));
    let left = values(&[x], &[&[one], &[TermId::UNBOUND], &[three]]);
    let right = values(&[x, y], &[&[one, two], &[two, two]]);
    assert_eq!(
        fx.bag(&[x, y], join(left, right)),
        bag(&[&[&int(1), &int(2)], &[&int(1), &int(2)], &[&int(2), &int(2)]])
    );
}

#[test]
fn optional_filter_sees_the_merged_solution() {
    let mut fx = Fx::new();
    let (a, b, name, age) = (fx.iri("a"), fx.iri("b"), fx.iri("name"), fx.iri("age"));
    fx.add(a, name, fx.lit("A"));
    fx.add(a, age, fx.int(30));
    fx.add(a, age, fx.int(5));
    fx.add(b, name, fx.lit("B"));
    fx.add(b, age, fx.int(10));
    let (s, n, g) = (fx.var("s"), fx.var("n"), fx.var("age"));
    let root = Logical::LeftJoin {
        left: Box::new(bgp(vec![tp(v(s), c(name), v(n))])),
        right: Box::new(bgp(vec![tp(v(s), c(age), v(g))])),
        filter: Some(gt(es(g), ec(fx.int(20)))),
    };
    // :a keeps only the passing age; :b has a compatible row that fails, so it stays alone.
    assert_eq!(fx.bag(&[s, n, g], root), bag(&[&[":a", "\"A\"", &int(30)], &[":b", "\"B\"", "-"]]));
}

#[test]
fn optional_filter_on_a_variable_bound_by_neither_side_is_false() {
    let mut fx = Fx::new();
    let (a, p, q) = (fx.iri("a"), fx.iri("p"), fx.iri("q"));
    fx.add(a, p, fx.int(1));
    fx.add(a, q, fx.int(2));
    let (s, o, o2, z) = (fx.var("s"), fx.var("o"), fx.var("o2"), fx.var("z"));
    // OPTIONAL { ?s :q ?o2 FILTER(?z = 2) }: ?z is unbound in the merged row -> error -> false.
    let root = Logical::LeftJoin {
        left: Box::new(bgp(vec![tp(v(s), c(p), v(o))])),
        right: Box::new(bgp(vec![tp(v(s), c(q), v(o2))])),
        filter: Some(eq(es(z), ec(fx.int(2)))),
    };
    assert_eq!(fx.select(&[s, o2], root), rows(&[&[":a", "-"]]));
}

#[test]
fn optional_keeps_left_multiplicity_and_right_matches() {
    let mut fx = Fx::new();
    let (a, b, p, q) = (fx.iri("a"), fx.iri("b"), fx.iri("p"), fx.iri("q"));
    fx.add(a, p, fx.int(1));
    fx.add(b, p, fx.int(1));
    fx.add(a, q, fx.int(7));
    fx.add(a, q, fx.int(8));
    let (s, o, r) = (fx.var("s"), fx.var("o"), fx.var("r"));
    let root = Logical::LeftJoin {
        left: Box::new(bgp(vec![tp(v(s), c(p), v(o))])),
        right: Box::new(bgp(vec![tp(v(s), c(q), v(r))])),
        filter: None,
    };
    assert_eq!(fx.bag(&[s, r], root), bag(&[&[":a", &int(7)], &[":a", &int(8)], &[":b", "-"]]));
}

#[test]
fn minus_removes_only_compatible_solutions_sharing_a_variable() {
    let mut fx = Fx::new();
    let (a, b, p, q) = (fx.iri("a"), fx.iri("b"), fx.iri("p"), fx.iri("q"));
    fx.add(a, p, fx.int(1));
    fx.add(b, p, fx.int(2));
    fx.add(a, q, fx.int(3));
    let (s, o, x, y) = (fx.var("s"), fx.var("o"), fx.var("x"), fx.var("y"));
    let left = || bgp(vec![tp(v(s), c(p), v(o))]);
    // Shared ?s: :a is removed.
    let shared = Logical::Minus { left: Box::new(left()), right: Box::new(bgp(vec![tp(v(s), c(q), v(y))])) };
    assert_eq!(fx.select(&[s], shared), rows(&[&[":b"]]));
    // Disjoint domains: nothing is removed even though every pair is compatible.
    let disjoint = Logical::Minus { left: Box::new(left()), right: Box::new(bgp(vec![tp(v(x), c(q), v(y))])) };
    assert_eq!(fx.bag(&[s], disjoint), bag(&[&[":a"], &[":b"]]));
    // Shared but incompatible values: nothing removed.
    let incompatible = Logical::Minus {
        left: Box::new(left()),
        right: Box::new(values(&[o], &[&[fx.int(9)]])),
    };
    assert_eq!(fx.select(&[s], incompatible).len(), 2);
    // A right solution leaving the shared variable unbound shares no variable.
    let unbound = Logical::Minus {
        left: Box::new(left()),
        right: Box::new(values(&[s, x], &[&[TermId::UNBOUND, a]])),
    };
    assert_eq!(fx.select(&[s], unbound).len(), 2);
}

#[test]
fn union_keeps_duplicates_and_filter_errors_are_false() {
    let mut fx = Fx::new();
    let (a, p) = (fx.iri("a"), fx.iri("p"));
    fx.add(a, p, fx.int(1));
    fx.add(a, p, a);
    let (s, o) = (fx.var("s"), fx.var("o"));
    let pat = || bgp(vec![tp(v(s), c(p), v(o))]);
    let union = Logical::Union(Box::new(pat()), Box::new(pat()));
    assert_eq!(fx.select(&[s], union).len(), 4);
    // ?o > 0: the IRI object is a type error, hence filtered out.
    assert_eq!(fx.select(&[o], filter(pat(), gt(es(o), ec(fx.int(0))))), rows(&[&[&int(1)]]));
    // A filter on an unbound variable is an error too.
    let z = fx.var("z");
    assert!(fx.select(&[o], filter(pat(), call(Function::IsIri, vec![es(z)]))).is_empty());
    assert_eq!(fx.select(&[o], filter(pat(), not(Expr::Bound(z)))).len(), 2);
}

#[test]
fn extend_binds_the_value_or_leaves_the_slot_unbound_on_error() {
    let mut fx = Fx::new();
    let (a, p) = (fx.iri("a"), fx.iri("p"));
    fx.add(a, p, fx.int(41));
    fx.add(a, p, a);
    let (o, y) = (fx.var("o"), fx.var("y"));
    let root = extend(bgp(vec![tp(c(a), c(p), v(o))]), y, add(es(o), ec(fx.int(1))));
    assert_eq!(fx.bag(&[o, y], root), bag(&[&[&int(41), &int(42)], &[":a", "-"]]));
}

#[test]
fn computed_values_are_canonical_ids_that_join_with_stored_terms() {
    let mut fx = Fx::new();
    let (a, label) = (fx.iri("a"), fx.iri("label"));
    fx.add(a, label, fx.lit("hello world!"));
    let (l, s) = (fx.var("l"), fx.var("s"));
    // BIND(CONCAT("hello ", "world!") AS ?l) . ?s :label ?l
    let concat = call(Function::Concat, vec![ec(fx.absent_lit("hello ")), ec(fx.absent_lit("world!"))]);
    let root = join(extend(Logical::Unit, l, concat), bgp(vec![tp(v(s), c(label), v(l))]));
    assert_eq!(fx.select(&[s, l], root), rows(&[&[":a", "\"hello world!\""]]));
    // A computed term that is not stored joins with nothing.
    let other = call(Function::Concat, vec![ec(fx.absent_lit("hello ")), ec(fx.absent_lit("there!"))]);
    let root = join(extend(Logical::Unit, l, other), bgp(vec![tp(v(s), c(label), v(l))]));
    assert!(fx.select(&[s], root).is_empty());
}

#[test]
fn values_rows_with_undef() {
    let mut fx = Fx::new();
    let (x, y) = (fx.var("x"), fx.var("y"));
    let a = fx.iri("a");
    let root = values(&[x, y], &[&[a, TermId::UNBOUND], &[TermId::UNBOUND, TermId::UNBOUND], &[a, a]]);
    assert_eq!(fx.select(&[x, y], root), rows(&[&[":a", "-"], &["-", "-"], &[":a", ":a"]]));
}

#[test]
fn order_by_is_stable_desc_reverses_and_unbound_sorts_first() {
    let mut fx = Fx::new();
    let (k, t) = (fx.var("k"), fx.var("t"));
    let (one, two) = (fx.int(1), fx.int(2));
    let (a, b, cc, d) = (fx.iri("a"), fx.iri("b"), fx.iri("c"), fx.iri("d"));
    let data = values(&[k, t], &[&[two, a], &[one, b], &[TermId::UNBOUND, cc], &[two, d]]);
    let asc = order_by(data.clone(), &[(es(k), false)]);
    assert_eq!(fx.select(&[t], asc), rows(&[&[":c"], &[":b"], &[":a"], &[":d"]]));
    let desc = order_by(data.clone(), &[(es(k), true)]);
    assert_eq!(fx.select(&[t], desc), rows(&[&[":a"], &[":d"], &[":b"], &[":c"]]));
    // Second key breaks ties.
    let two_keys = order_by(data, &[(es(k), true), (es(t), true)]);
    assert_eq!(fx.select(&[t], two_keys), rows(&[&[":d"], &[":a"], &[":b"], &[":c"]]));
}

#[test]
fn project_distinct_reduced_and_slice() {
    let mut fx = Fx::new();
    let (x, y) = (fx.var("x"), fx.var("y"));
    let (a, b) = (fx.iri("a"), fx.iri("b"));
    let data = || values(&[x, y], &[&[a, a], &[a, b], &[b, b], &[a, a]]);
    // Project resets the other slots, so DISTINCT compares projected values only.
    let distinct = Logical::Distinct(Box::new(project(data(), &[x])));
    assert_eq!(fx.select(&[x, y], distinct), rows(&[&[":a", "-"], &[":b", "-"]]));
    let reduced = Logical::Reduced(Box::new(project(data(), &[x])));
    assert_eq!(fx.select(&[x], reduced).len(), 4, "REDUCED may keep every duplicate");
    let slice = Logical::Slice { input: Box::new(data()), offset: 1, limit: Some(2) };
    assert_eq!(fx.select(&[x, y], slice), rows(&[&[":a", ":b"], &[":b", ":b"]]));
    let tail = Logical::Slice { input: Box::new(data()), offset: 3, limit: None };
    assert_eq!(fx.select(&[x, y], tail), rows(&[&[":a", ":a"]]));
    let past = Logical::Slice { input: Box::new(data()), offset: 10, limit: Some(u64::MAX) };
    assert!(fx.select(&[x], past).is_empty());
}

#[test]
fn service_is_unit_when_silent_and_unsupported_otherwise() {
    let fx = Fx::new();
    assert_eq!(fx.select(&[], Logical::Service { silent: true }), rows(&[&[]]));
    let err = fx.try_run(QueryForm::Select(vec![]), Logical::Service { silent: false }).unwrap_err();
    assert!(matches!(err, Error::Unsupported(_)), "{err}");
}
