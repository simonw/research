//! Statement shapes that keep SQLite's limits and the dataset rules (results are checked
//! against spareval by the integration tests).
use sq_core::Error;
use sq_core::plan::QueryOptions;

use super::{lower, lower_with};
use crate::translate;

fn unsupported(query: &str) -> String {
    match translate(&lower(query)) {
        Err(Error::Unsupported(msg)) => msg,
        other => panic!("{query}: expected Unsupported, got {other:?}"),
    }
}

#[test]
fn wide_joins_are_split_into_opaque_chunks() {
    let pats: Vec<String> = (0..130).map(|i| format!(":a :knows ?x{i} .")).collect();
    let sql = super::sql(&format!("SELECT * WHERE {{ {} }}", pats.join(" "))).sql;
    // Two chunks of 64 tables, each a derived table SQLite does not flatten.
    assert_eq!(sql.matches(" LIMIT -1)").count(), 2, "{sql}");
    let small = super::sql("SELECT * WHERE { :a :knows ?x . ?x :knows ?y }").sql;
    assert!(!small.contains("LIMIT -1"), "{small}");
}

#[test]
fn long_path_sequences_are_split_into_opaque_prefixes() {
    let seq = vec!["!(:p|:q)"; 70].join("/");
    let sql = super::sql(&format!("SELECT * WHERE {{ ?x {seq} ?y }}")).sql;
    assert_eq!(sql.matches(" LIMIT -1").count(), 1, "one prefix of 63 steps");
    let alts = vec![":p"; 700].join("|");
    let sql = super::sql(&format!("SELECT * WHERE {{ :a ({alts})+ ?y }}")).sql;
    assert_eq!(sql.matches("SELECT * FROM (").count(), 4, "two alternations of 700 (seed and step)");
}

#[test]
fn wide_unions_are_grouped_under_the_compound_limit() {
    let branches: Vec<String> = (0..1001).map(|i| format!("{{ ?s :age {i} }}")).collect();
    let sql = super::sql(&format!("SELECT ?s WHERE {{ {} }}", branches.join(" UNION "))).sql;
    assert_eq!(sql.matches("SELECT * FROM (").count(), 3, "three runs of at most 500 terms");
}

#[test]
fn long_or_chains_and_in_lists_stay_shallow() {
    let terms: Vec<String> = (0..2000).map(|i| format!("?a = {i}")).collect();
    let sql = super::sql(&format!("SELECT ?x WHERE {{ ?x :age ?a FILTER({}) }}", terms.join(" || "))).sql;
    assert!(sql.matches('(').count() < 20_000, "no parenthesis per operand pair");
    let items: Vec<String> = (0..2000).map(|i| i.to_string()).collect();
    let sql = super::sql(&format!("SELECT ?x WHERE {{ ?x :age ?a FILTER(?a IN ({})) }}", items.join(", "))).sql;
    assert_eq!(sql.matches("sparql_eval('='").count(), 2000, "one value comparison per item, for TYPED rows");
    assert!(sql.contains("t1.o IN (?"), "{}", &sql[..200]);
}

#[test]
fn nesting_past_the_parser_stack_is_unsupported() {
    let mut p = "?s :knows ?o".to_owned();
    for _ in 0..12 {
        p = format!("?s :knows ?o FILTER EXISTS {{ {p} }}");
    }
    let msg = unsupported(&format!("SELECT * WHERE {{ {p} }}"));
    assert_eq!(msg, "baseline: query nesting too deep for SQLite's parser");
    let closures = format!("SELECT ?x WHERE {{ :a {}:sub{} ?x }}", "(".repeat(7), ")*".repeat(7));
    assert_eq!(unsupported(&closures), "baseline: property paths with closures nested more than 6 deep");
}

#[test]
fn graph_membership_and_named_graphs() {
    // GRAPH <c> over a zero-length path checks that c is a graph; over a BGP it need not.
    let path = super::sql("SELECT ?y WHERE { GRAPH :g1 { :a :sub* ?y } }").sql;
    assert!(path.contains("EXISTS (SELECT 1 FROM main.rdf_graph WHERE g = "), "{path}");
    let bgp = super::sql("SELECT ?y WHERE { GRAPH :g1 { :a :sub ?y } }").sql;
    assert!(!bgp.contains("rdf_graph"), "{bgp}");
    // FROM without FROM NAMED: no named graph, so GRAPH ?g has no solution at all.
    for q in [
        "SELECT ?g ?y FROM :g1 WHERE { GRAPH ?g { :a :sub* ?y } }",
        "SELECT ?g ?y FROM :g1 WHERE { GRAPH ?g { :a :sub? ?y } }",
        "SELECT ?g FROM :g1 WHERE { GRAPH ?g { BIND(1 AS ?x) } }",
    ] {
        assert!(super::sql(q).sql.ends_with(" WHERE 0"), "{q}");
    }
}

#[test]
fn merged_default_graphs_read_each_triple_once_through_indexes() {
    let opts = QueryOptions { union_default_graph: true, ..Default::default() };
    let sql = translate(&lower_with("SELECT * WHERE { ?s :knows ?o }", &opts)).unwrap().sql;
    assert!(sql.contains("t1.g IN (WITH RECURSIVE gs(g) AS (SELECT min(g) FROM main.rdf_quad"), "{sql}");
    assert!(sql.contains("(t1.g = 0 OR NOT EXISTS (SELECT 1 FROM main.rdf_quad AS u"), "{sql}");
    assert!(!sql.contains("DISTINCT"), "no materialized merge: {sql}");
    let from = super::sql("SELECT * WHERE { :a :sub+ ?o }");
    let merged = super::sql("SELECT * FROM :g1 FROM :g2 WHERE { :a :sub+ ?o }").sql;
    assert!(!from.sql.contains("u2"), "a single graph needs no deduplication");
    // The closure's recursive step skips the first-copy test (the closure is a set).
    assert_eq!(merged.matches("NOT EXISTS").count(), 1, "{merged}");
}
