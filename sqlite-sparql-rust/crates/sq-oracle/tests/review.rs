//! Adversarial review tests (WP7), end to end: SPARQL text -> sq-plan lowering -> sq-oracle.
use std::str::FromStr;

use sq_core::plan::QueryOptions;
use sq_core::store::QuadStoreWriter;
use sq_core::{Term, TermId};
use sq_memstore::MemStore;
use sq_oracle::{OracleOptions, OracleOutput};

const BASE: &str = "http://ex/";

fn iri(store: &MemStore, local: &str) -> TermId {
    store.intern(oxrdf::NamedNodeRef::new_unchecked(&format!("{BASE}{local}")).into()).unwrap()
}

/// Sorted `?y` values (unbound as `-`).
fn ys(store: &MemStore, query: &str) -> Vec<String> {
    let text = format!("PREFIX : <{BASE}>\n{query}");
    let parsed = sq_plan::parse_query(&text, Some(BASE)).expect("query parses");
    let options = QueryOptions { base_iri: Some(BASE.to_owned()), ..QueryOptions::default() };
    let lowered = sq_plan::lower_query(&parsed, store, &options).expect("query lowers");
    let now = oxsdatatypes::DateTime::from_str("2026-10-15T12:00:00Z").unwrap();
    let out = sq_oracle::evaluate_with(&lowered, store, &OracleOptions { now: Some(now), rng_seed: Some(1) })
        .expect("query evaluates");
    let OracleOutput::Solutions { rows, .. } = out else { panic!("solutions expected") };
    let mut v: Vec<String> = rows
        .iter()
        .map(|r| match r[0].as_deref() {
            Some(Term::Iri(i)) => format!(":{}", &i[BASE.len()..]),
            Some(t) => t.to_ntriples(),
            None => "-".into(),
        })
        .collect();
    v.sort();
    v
}

/// `X (P)|:never Y` must equal `X P Y` when :never has no triples (Alternative = Union, §18.4).
/// spargebra rewrites a top-level `P/Q` into two path patterns joined on a fresh blank node
/// (the spec's own translation), but keeps `seq` inside `|`, where the oracle fixes the middle
/// instead of treating it as the fresh variable of §18.4's sequence definition. pyoxigraph
/// returns no solution for all three queries.
#[test]
fn review_nested_sequence_agrees_with_top_level_sequence() {
    let store = MemStore::new();
    let (a, b, p) = (iri(&store, "a"), iri(&store, "b"), iri(&store, "p"));
    store.insert(&mut vec![[TermId::DEFAULT_GRAPH, a, p, b]]).unwrap();

    let top_level = ys(&store, "SELECT ?y { :zz :p?/:q? ?y }");
    assert!(top_level.is_empty(), "top-level sequence: {top_level:?}");
    let nested = ys(&store, "SELECT ?y { :zz (:p?/:q?)|:never ?y }");
    assert_eq!(nested, top_level, "the same sequence nested in an alternative");
    let closure = ys(&store, "SELECT ?y { :zz (:p?/:q?)+ ?y }");
    assert_eq!(closure, top_level, "(:p?/:q?)+ from a constant that is not a node");
}

/// Same defect with both endpoints constant. By §18.4, `:zz :p?/(:q?/:r?) :zz` joins
/// Path(:zz, :p?, ?V1) = {?V1 -> :zz} with Path(?V1, :q?/:r?, :zz), whose inner middle ?V2 is
/// bound to :zz by `:r? :zz` but Path(?V1, :q?, ?V2) has two free ends and so only matches
/// nodes(G): no solution, ASK is false. The top-level form (rewritten into three joined path
/// patterns by spargebra) is false in the oracle; the nested form must be too.
#[test]
fn review_nested_sequence_with_constant_endpoints() {
    let store = MemStore::new();
    let (a, b, p) = (iri(&store, "a"), iri(&store, "b"), iri(&store, "p"));
    store.insert(&mut vec![[TermId::DEFAULT_GRAPH, a, p, b]]).unwrap();
    let ask = |q: &str| {
        let text = format!("PREFIX : <{BASE}>\n{q}");
        let parsed = sq_plan::parse_query(&text, Some(BASE)).expect("query parses");
        let lowered = sq_plan::lower_query(&parsed, &store, &QueryOptions::default()).expect("query lowers");
        sq_oracle::evaluate(&lowered, &store).expect("query evaluates")
    };
    assert_eq!(ask("ASK { :zz :p?/(:q?/:r?) :zz }"), OracleOutput::Boolean(false), "top-level");
    assert_eq!(ask("ASK { :zz (:p?/(:q?/:r?))|:never :zz }"), OracleOutput::Boolean(false), "nested in |");
}
