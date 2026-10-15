//! End-to-end: SPARQL text -> sq-plan lowering -> sq-oracle on MemStore, over inline TriG
//! data. Each case is a named regression shape of docs/SPEC.md §12.2 (or the W3C test it
//! transcribes); the full W3C suite runs through `sq-w3c --backend oracle`.
use std::collections::HashMap;
use std::str::FromStr;

use sq_core::plan::QueryOptions;
use sq_core::store::QuadStoreWriter;
use sq_core::{Term, TermId};
use sq_memstore::MemStore;
use sq_oracle::{OracleOptions, OracleOutput};

const BASE: &str = "http://ex/";

/// Loads TriG (default prefix `:` = <http://ex/>) into a fresh store.
fn store(trig: &str) -> MemStore {
    let store = MemStore::new();
    let text = format!("@prefix : <{BASE}> .\n@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .\n{trig}");
    let mut bnodes: HashMap<String, TermId> = HashMap::new();
    let mut id = |t: oxrdf::TermRef<'_>| match t {
        oxrdf::TermRef::BlankNode(b) => {
            *bnodes.entry(b.as_str().to_owned()).or_insert_with(|| store.fresh_bnode().unwrap())
        }
        t => store.intern(t).unwrap(),
    };
    let mut quads = Vec::new();
    for q in oxttl::TriGParser::new().with_base_iri(BASE).unwrap().for_slice(text.as_bytes()) {
        let q = q.expect("valid TriG");
        let g = match q.graph_name.as_ref() {
            oxrdf::GraphNameRef::DefaultGraph => TermId::DEFAULT_GRAPH,
            oxrdf::GraphNameRef::NamedNode(n) => id(n.into()),
            oxrdf::GraphNameRef::BlankNode(b) => id(b.into()),
        };
        quads.push([g, id(q.subject.as_ref().into()), id(q.predicate.as_ref().into()), id(q.object.as_ref())]);
    }
    store.insert(&mut quads).unwrap();
    store
}

fn run(store: &MemStore, query: &str) -> OracleOutput {
    let text = format!("PREFIX : <{BASE}>\nPREFIX xsd: <http://www.w3.org/2001/XMLSchema#>\n{query}");
    let parsed = sq_plan::parse_query(&text, Some(BASE)).expect("query parses");
    let options = QueryOptions { base_iri: Some(BASE.to_owned()), ..QueryOptions::default() };
    let lowered = sq_plan::lower_query(&parsed, store, &options).expect("query lowers");
    let now = oxsdatatypes::DateTime::from_str("2026-10-15T12:00:00Z").unwrap();
    let opts = OracleOptions { now: Some(now), rng_seed: Some(1) };
    sq_oracle::evaluate_with(&lowered, store, &opts).expect("query evaluates")
}

/// `:local` for the test namespace, the bare lexical form for xsd:integer, else N-Triples.
fn show(t: &Term) -> String {
    match t {
        Term::Iri(i) if i.starts_with(BASE) => format!(":{}", &i[BASE.len()..]),
        Term::Literal(l) if &*l.datatype == sq_core::codec::XSD_INTEGER_IRI => l.lex.to_string(),
        t => t.to_ntriples(),
    }
}

/// Solutions as `?var=term` lines (unbound variables omitted), in solution order.
fn solutions(out: OracleOutput) -> Vec<String> {
    let OracleOutput::Solutions { vars, rows } = out else { panic!("expected solutions, got {out:?}") };
    rows.iter()
        .map(|row| {
            vars.iter()
                .zip(row)
                .filter_map(|(v, t)| t.as_ref().map(|t| format!("?{v}={}", show(t))))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

fn sorted(mut rows: Vec<String>) -> Vec<String> {
    rows.sort();
    rows
}

fn select(data: &str, query: &str) -> Vec<String> {
    sorted(solutions(run(&store(data), query)))
}

fn lines(expected: &[&str]) -> Vec<String> {
    sorted(expected.iter().map(|s| s.to_string()).collect())
}

#[test]
fn values_and_path_only_match_graph_nodes() {
    // property-path/values_and_path: the VALUES term is not a node of the (empty) graph.
    assert!(select("", "SELECT * { VALUES ?v { 1 } ?v :p? ?v }").is_empty());
    // property-path/zero_or_one_set_start: a constant endpoint matches itself.
    assert_eq!(select("", "SELECT ?s { ?s :p? :o }"), lines(&["?s=:o"]));
    assert_eq!(select("", "SELECT ?o { :s :p* ?o }"), lines(&["?o=:s"]));
}

#[test]
fn negated_property_sets_with_inverse_members() {
    let data = ":sd :pd :od . :sr :pr :or .";
    assert_eq!(select(data, "SELECT ?s ?o { ?s !(:pd|^:pr) ?o }"), lines(&["?s=:od ?o=:sd", "?s=:sr ?o=:or"]));
}

#[test]
fn optional_filter_scoping() {
    let data = ":a :n 1 ; :v 10 . :b :n 2 ; :v 20 .";
    // The OPTIONAL's filter sees ?n from the left side.
    assert_eq!(
        select(data, "SELECT ?s ?v { ?s :n ?n OPTIONAL { ?s :v ?v FILTER(?n = 2) } }"),
        lines(&["?s=:a", "?s=:b ?v=20"])
    );
    // A filter in a nested group cannot see ?n.
    assert_eq!(
        select(data, "SELECT ?s ?v { ?s :n ?n OPTIONAL { ?s :v ?v { FILTER(?n = 2) } } }"),
        lines(&["?s=:a", "?s=:b"])
    );
}

#[test]
fn exists_inside_graph_and_with_a_graph_variable() {
    let data = ":g1 { :s :p :o1 , :o2 . :t :p :o1 } :g2 { :t :p :o2 } :s :link :g1 . :t :link :g3 .";
    // exists03: the EXISTS pattern reads the active graph.
    assert_eq!(
        select(data, "SELECT ?s { GRAPH :g1 { ?s :p :o1 FILTER EXISTS { ?s :p :o2 } } }"),
        lines(&["?s=:s"])
    );
    // exists-graph-variable: GRAPH ?g with ?g substituted from the outer solution.
    assert_eq!(
        select(data, "SELECT ?s { ?s :link ?g FILTER EXISTS { GRAPH ?g { ?x :p ?y } } }"),
        lines(&["?s=:s"])
    );
}

#[test]
fn graph_minus_and_graph_values_are_evaluated_per_graph() {
    let data = ":g1 { :a :p 1 . :b :p 2 } :g2 { :a :p 3 . :a :q 4 }";
    assert_eq!(
        select(data, "SELECT ?g ?s { GRAPH ?g { ?s :p ?o MINUS { ?s :q ?z } } }"),
        lines(&["?g=:g1 ?s=:a", "?g=:g1 ?s=:b"])
    );
    assert_eq!(select(data, "SELECT ?g ?x { GRAPH ?g { VALUES ?x { 7 } } }").len(), 2);
    assert_eq!(select(data, "SELECT ?g { GRAPH ?g { } }"), lines(&["?g=:g1", "?g=:g2"]));
}

#[test]
fn datasets_from_and_from_named() {
    let data = ":g1 { :a :p 1 } :g2 { :a :p 1 . :b :p 2 } :c :p 3 .";
    assert_eq!(
        select(data, "SELECT ?s ?o FROM :g1 FROM :g2 { ?s :p ?o }"),
        lines(&["?s=:a ?o=1", "?s=:b ?o=2"])
    );
    assert!(select(data, "SELECT ?s FROM NAMED :g1 { ?s :p ?o }").is_empty());
    assert_eq!(
        select(data, "SELECT ?g FROM NAMED :g2 FROM NAMED :nowhere { GRAPH ?g { } }"),
        lines(&["?g=:g2", "?g=:nowhere"])
    );
}

#[test]
fn distinct_keeps_distinct_lexical_forms() {
    // distinct-num: "1", "01" and "+1" are three different terms.
    let data = r#":a :p 1, "01"^^xsd:integer, "+1"^^xsd:integer . :b :p 1 ."#;
    assert_eq!(select(data, "SELECT DISTINCT ?o { ?s :p ?o }").len(), 3);
    // ... but equal values for `=`.
    assert_eq!(select(data, "SELECT ?o { :a :p ?o FILTER(?o = 1) }").len(), 3);
}

#[test]
fn sub_select_variables_are_scoped() {
    let data = ":a :p :b . :b :q :c . :a :q :z .";
    // The inner ?y is not projected, so it does not join with the outer ?y.
    assert_eq!(
        select(data, "SELECT ?x ?y { ?x :p ?y { SELECT ?x { ?x :q ?y } } }"),
        lines(&["?x=:a ?y=:b"])
    );
    assert_eq!(
        select(data, "SELECT ?x (COUNT(*) AS ?n) { ?x ?p ?o } GROUP BY ?x HAVING (COUNT(*) > 1) ORDER BY ?x"),
        lines(&["?x=:a ?n=2"])
    );
}

#[test]
fn aggregates_errors_and_empty_groups() {
    let data = ":a :p 1, 2 . :b :p 3, :x .";
    // agg-err-01 shape: the error makes AVG unbound but keeps the group.
    assert_eq!(
        select(data, "SELECT ?s (AVG(?o) AS ?avg) { ?s :p ?o } GROUP BY ?s"),
        lines(&["?s=:a ?avg=\"1.5\"^^<http://www.w3.org/2001/XMLSchema#decimal>", "?s=:b"])
    );
    // agg-empty-group: no GROUP BY over no solutions gives one row; with GROUP BY none.
    assert_eq!(
        select(data, "SELECT (COUNT(*) AS ?c) { ?s :none ?o }"),
        lines(&["?c=0"])
    );
    assert!(select(data, "SELECT ?s (COUNT(*) AS ?c) { ?s :none ?o } GROUP BY ?s").is_empty());
}

#[test]
fn order_by_with_limit_returns_the_sequence_order() {
    let data = ":a :v 3 . :b :v 1 . :c :v 2 . :d :v :iri .";
    let out = solutions(run(&store(data), "SELECT ?s { ?s :v ?v } ORDER BY DESC(?v) LIMIT 3"));
    assert_eq!(out, vec!["?s=:a", "?s=:c", "?s=:b"]);
}

#[test]
fn construct_ask_and_describe() {
    let data = ":a :p :b ; :q [ :r 1 ] . :b :p :c .";
    let st = store(data);
    let OracleOutput::Graph(triples) = run(&st, "CONSTRUCT { ?y :inv ?x . _:n :from ?x } WHERE { ?x :p ?y }") else {
        panic!("graph expected")
    };
    assert_eq!(triples.len(), 4);
    assert_eq!(run(&st, "ASK { :a :p/:p :c }"), OracleOutput::Boolean(true));
    assert_eq!(run(&st, "ASK { :c :p ?x }"), OracleOutput::Boolean(false));
    let OracleOutput::Graph(cbd) = run(&st, "DESCRIBE :a") else { panic!("graph expected") };
    // :a's two triples plus the blank node's.
    assert_eq!(cbd.len(), 3);
}
