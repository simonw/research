use sq_core::ir::{Logical, PathPlan};

use super::lower;
use crate::graph::{GClass, classify, mentions};
use crate::path::zero_length;

/// The body of the first GRAPH in a query, with the query's EXISTS patterns.
fn graph_body(query: &str) -> (Logical, Vec<Logical>, sq_core::Slot) {
    fn find(n: &Logical) -> Option<(&Logical, &sq_core::ir::PatTerm)> {
        match n {
            Logical::Graph { name, input } => Some((input, name)),
            Logical::Join(a, b) | Logical::Union(a, b) | Logical::Minus { left: a, right: b } => {
                find(a).or_else(|| find(b))
            }
            Logical::LeftJoin { left, right, .. } => find(left).or_else(|| find(right)),
            Logical::Filter { input, .. }
            | Logical::Extend { input, .. }
            | Logical::Project { input, .. }
            | Logical::Distinct(input)
            | Logical::Reduced(input)
            | Logical::Slice { input, .. }
            | Logical::OrderBy { input, .. }
            | Logical::Group { input, .. } => find(input),
            _ => None,
        }
    }
    let q = lower(query);
    let (body, name) = find(&q.root).expect("a GRAPH");
    let sq_core::ir::PatTerm::Slot(g) = *name else { panic!("GRAPH ?g expected") };
    (body.clone(), q.exists.clone(), g)
}

fn class(query: &str) -> GClass {
    let (body, exists, _) = graph_body(query);
    classify(&body, &exists)
}

#[test]
fn graph_bodies_are_classified() {
    use GClass::*;
    let cases = [
        ("SELECT * { GRAPH ?g { ?s :p ?o } }", GForm),
        ("SELECT * { GRAPH ?g { ?s :p ?o OPTIONAL { ?o :q ?z } } }", GForm),
        ("SELECT * { GRAPH ?g { ?s :p ?o MINUS { ?s :q ?z } } }", GForm),
        ("SELECT * { GRAPH ?g { ?s :p ?o FILTER EXISTS { ?s :q ?z } } }", GForm),
        ("SELECT * { GRAPH ?g { { ?s :p ?o } UNION { ?s :q ?o } } }", GForm),
        ("SELECT * { GRAPH ?g { ?s :p ?o VALUES ?z { 1 } } }", GForm),
        ("SELECT * { GRAPH ?g { } }", Indep),
        ("SELECT * { GRAPH ?g { VALUES ?z { 1 2 } } }", Indep),
        ("SELECT * { GRAPH ?g { GRAPH :g1 { ?s :p ?o } } }", Indep),
        ("SELECT * { GRAPH ?g { BIND(1 AS ?z) } }", Indep),
        ("SELECT * { GRAPH ?g { { ?s :p ?o } UNION { BIND(1 AS ?s) } } }", Bad),
        ("SELECT * { GRAPH ?g { OPTIONAL { ?s :p ?o } } }", Bad),
        ("SELECT * { GRAPH ?g { BIND(1 AS ?z) FILTER EXISTS { ?s :p ?o } } }", Bad),
        ("SELECT * { GRAPH ?g { { SELECT ?s { ?s :p ?o } LIMIT 1 } } }", Bad),
        ("SELECT * { GRAPH ?g { { SELECT (1 AS ?z) { } } } }", Indep),
        // Paths: one-or-more always reads an edge; a zero-length-capable path pairs a constant
        // endpoint with itself without reading any quad.
        ("SELECT * { GRAPH ?g { ?s :sub+ ?o } }", GForm),
        ("SELECT * { GRAPH ?g { ?s (:sub/:sub*)+ ?o } }", GForm),
        ("SELECT * { GRAPH ?g { :a :sub* ?o } }", Bad),
        ("SELECT * { GRAPH ?g { :a :sub? ?o } }", Bad),
        ("SELECT * { GRAPH ?g { :a (:sub|:p*) ?o } }", Bad),
        ("SELECT * { GRAPH ?g { :a (:sub*)+ ?o } }", Bad),
        ("SELECT * { GRAPH ?g { :a (:sub?)+ ?o } }", Bad),
        ("SELECT * { GRAPH ?g { :a ^(:sub*)+ ?o } }", Bad),
        ("SELECT * { GRAPH ?g { GRAPH :g1 { :a :sub* ?o } } }", Indep),
    ];
    for (q, want) in cases {
        assert_eq!(class(q), want, "{q}");
    }
}

#[test]
fn graph_variable_mentions() {
    let cases = [
        ("SELECT * { GRAPH ?g { ?s :p ?o } }", false),
        ("SELECT * { GRAPH ?g { ?g :p ?o } }", true),
        ("SELECT * { GRAPH ?g { ?s :p ?o FILTER(?o != ?g) } }", true),
        ("SELECT * { GRAPH ?g { ?s :p ?o FILTER EXISTS { ?s :q ?g } } }", true),
        ("SELECT * { GRAPH ?g { ?s :p ?o BIND(?g AS ?h) } }", true),
        ("SELECT * { GRAPH ?g { ?s :p ?o VALUES ?g { :g1 } } }", true),
        ("SELECT * { GRAPH ?g { ?s :p ?o GRAPH ?g { ?s :q ?z } } }", true),
    ];
    for (q, want) in cases {
        let (body, exists, g) = graph_body(q);
        assert_eq!(mentions(&body, g, &exists), want, "{q}");
    }
}

/// The first property path of a query.
fn first_path(query: &str) -> PathPlan {
    fn find(n: &Logical) -> Option<&PathPlan> {
        match n {
            Logical::Path { path, .. } => Some(path),
            Logical::Join(a, b) => find(a).or_else(|| find(b)),
            Logical::Project { input, .. } => find(input),
            _ => None,
        }
    }
    find(&lower(query).root).cloned().unwrap_or_else(|| panic!("no path in {query}"))
}

#[test]
fn zero_length_capable_paths() {
    let cases = [
        ("!(:sub)", false),
        (":sub+", false),
        (":sub/:p", false),
        (":sub*", true),
        (":sub?", true),
        ("^:sub*", true),
        (":sub*/:p?", true),
        (":sub*/:p", false),
        (":sub|:p*", true),
        // x+ starts with a step of x: zero-length capable exactly when x is.
        ("(:sub*)+", true),
        ("(:sub?)+", true),
        ("^(:sub*)+", true),
        ("((:sub?)+)+", true),
        ("(:sub*/:p?)+", true),
        ("(:sub/:sub*)+", false),
        ("(:sub|^:p)+", false),
    ];
    for (path, want) in cases {
        assert_eq!(zero_length(&first_path(&format!("SELECT * {{ :a {path} ?o }}"))), want, "{path}");
    }
}
