use sq_core::ir::Logical;

use super::lower;
use crate::graph::{GClass, classify, mentions};

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
