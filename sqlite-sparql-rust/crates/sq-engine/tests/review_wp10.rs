//! Adversarial review tests for WP10 (sq-engine update executor and facade).
mod common;

use std::rc::Rc;

use common::*;
use sq_core::eval_api::SolutionSource;
use sq_core::plan::QueryOptions;
use sq_core::store::QuadStore;
use sq_core::Error;
use sq_engine::{UpdateOptions, UpdateReport};
use sq_memstore::MemStore;
use sq_results::ResultFormat;

fn update_opts(store: &MemStore, text: &str, opts: &QueryOptions) -> sq_core::Result<UpdateReport> {
    let u = sq_plan::parse_update(&format!("{PREFIXES}{text}"), Some(EX))?;
    let uopts = UpdateOptions { now: Some(fixed_now()), ..UpdateOptions::default() };
    sq_engine::update_with(Rc::new(store.clone()), &u, opts, &uopts)
}

/// sq-api passes the connection's `union_default_graph` to `update_with`. spargebra lowers
/// `ADD/COPY/MOVE DEFAULT TO <g>` to `INSERT { GRAPH <g> { ?s ?p ?o } } WHERE { ?s ?p ?o }`
/// with no USING, so the executor evaluates the source as the UNION of every graph: the
/// destination receives the triples of all named graphs, not just the default graph's
/// (SPARQL 1.1 Update §3.2.3-3.2.5 define these operations on stored graphs).
#[test]
fn review_union_default_graph_must_not_leak_into_add_copy_move() {
    let union = QueryOptions { union_default_graph: true, ..options() };
    for op in ["MOVE", "COPY", "ADD"] {
        let s = mem(":a :p 1 . :g1 { :b :p 2 }");
        update_opts(&s, &format!("{op} DEFAULT TO :g2"), &union).unwrap();
        assert_eq!(
            select_mem(&s, "SELECT ?s { GRAPH :g2 { ?s :p ?o } }"),
            ["?s=:a"],
            "{op} DEFAULT TO :g2 under union_default_graph copied other graphs' triples: {:?}",
            dump_mem(&s)
        );
    }
}

/// `max_memory_mb` bounds what a query may buffer (the SELECT below fails with
/// MemoryBudget), but the update executor's materialized WHERE solutions and template quads
/// are never charged: the same WHERE inside an update buffers several MB under a 1 MB budget
/// (unbounded in general: `DELETE WHERE { ?s ?p ?o }` on a large store).
#[test]
fn review_update_materialization_respects_max_memory_mb() {
    let mut ttl = String::new();
    for i in 0..60_000 {
        ttl.push_str(&format!(":s{i} :p \"value number {i}\" .\n"));
    }
    let s = mem(&ttl);
    let tight = QueryOptions { max_memory_mb: 1, ..options() };
    let q = sq_plan::parse_query(&format!("{PREFIXES}SELECT ?s ?o WHERE {{ ?s :p ?o }} ORDER BY ?o"), Some(EX)).unwrap();
    let mut exec = sq_engine::prepare(Rc::new(s.clone()), &q, &tight, sq_exec::ExecCaches::default()).unwrap();
    let mut ids = Vec::new();
    let e = exec.next_rows(&mut ids, 1024).unwrap_err();
    assert!(matches!(e, Error::MemoryBudget(_)), "{e}");
    let r = update_opts(&s, "INSERT { ?s :q ?o } WHERE { ?s :p ?o }", &tight);
    assert!(matches!(r, Err(Error::MemoryBudget(_))), "update under a 1 MB budget: {r:?}");
}

#[test]
fn review_bnode_with_a_string_argument_differs_between_solutions() {
    // BNODE("k") is the same node within one solution and a different node per solution.
    let s = mem(":a :p 1 . :b :p 2 .");
    update(&s, "INSERT { ?x :from ?s . ?y :also ?s } WHERE { ?s :p ?v BIND(BNODE(\"k\") AS ?x) BIND(BNODE(\"k\") AS ?y) }").unwrap();
    assert_eq!(bnode_count(&s), 2);
    assert_eq!(select_mem(&s, "SELECT ?s { ?x :from ?s ; :also ?s }").len(), 2);
}

#[test]
fn review_error_valued_group_key_leaves_the_template_variable_unbound() {
    let s = mem(":a :p 1 . :b :p 2 .");
    let r = update(&s, "INSERT { :x :k ?k ; :c ?c } WHERE { SELECT ?k (COUNT(*) AS ?c) { ?s :p ?o } GROUP BY (xsd:integer(\"abc\") AS ?k) }")
        .unwrap();
    assert_eq!(r, UpdateReport { inserted: 1, deleted: 0 });
    assert_eq!(select_mem(&s, "SELECT ?c { :x :c ?c }"), ["?c=\"2\"^^<http://www.w3.org/2001/XMLSchema#integer>"]);
}

#[test]
fn review_using_named_and_with_plus_using_datasets() {
    let s = mem(":a :p 1 . :g1 { :b :p 2 } :g2 { :c :p 3 }");
    // USING NAMED only: the WHERE default graph is empty, GRAPH ?g sees only :g1.
    assert_eq!(update(&s, "INSERT { :x :saw ?s } USING NAMED :g1 WHERE { ?s :p ?o }").unwrap(), UpdateReport::default());
    update(&s, "INSERT { :x :sawg ?s } USING NAMED :g1 WHERE { GRAPH ?g { ?s :p ?o } }").unwrap();
    assert_eq!(select_mem(&s, "SELECT ?s { :x :sawg ?s }"), ["?s=:b"]);
    // WITH + USING: USING decides the WHERE dataset, WITH the templates' graph.
    update(&s, "WITH :g1 INSERT { ?s :q 1 } USING :g2 WHERE { ?s :p ?o }").unwrap();
    assert_eq!(select_mem(&s, "SELECT ?g ?s { GRAPH ?g { ?s :q 1 } }"), ["?g=:g1 ?s=:c"]);
}

#[test]
fn review_where_constants_absent_from_the_store_are_interned() {
    let s = mem(":a :p 1 .");
    let r = update(&s, "INSERT { :a :v ?x . :a :w ?y } WHERE { VALUES ?x { \"brand new long literal\" :brandnew } BIND(:newer AS ?y) }").unwrap();
    assert_eq!(r, UpdateReport { inserted: 3, deleted: 0 });
    assert_eq!(select_mem(&s, "SELECT ?x { :a :v ?x FILTER(isIRI(?x)) }"), ["?x=:brandnew"]);
    assert_eq!(select_mem(&s, "SELECT ?s { ?s :w :newer }"), ["?s=:a"]);
    // Language tags of computed literals are stored in normalized form.
    update(&s, "INSERT { :a :l ?l } WHERE { BIND(STRLANG(\"x\", \"EN\") AS ?l) }").unwrap();
    assert_eq!(select_mem(&s, "SELECT ?s { ?s :l \"x\"@en }"), ["?s=:a"]);
    // A graph variable bound to a literal never addresses a graph in DELETE templates.
    let s = mem(":g1 { :b :p 2 }");
    assert_eq!(update(&s, "DELETE { GRAPH ?g { :b :p 2 } } WHERE { VALUES ?g { \"g1\" } }").unwrap(), UpdateReport::default());
}

#[test]
fn review_construct_and_describe_outputs_are_sets() {
    let s: Rc<dyn QuadStore> = Rc::new(mem(":a :p :b , :c ; :q _:x . _:x :r 1 . :b :p :a ."));
    for q in [
        "DESCRIBE ?s WHERE { ?s ?p ?o }",
        "DESCRIBE ?s ?o WHERE { ?s :p ?o }",
        "CONSTRUCT { ?s :p ?o } WHERE { ?s :p ?o . ?s ?x ?y }",
        "CONSTRUCT { :k :v 1 } WHERE { ?s ?p ?o }",
    ] {
        for policy in [sq_core::plan::JoinPolicy::Auto, sq_core::plan::JoinPolicy::PreferHash, sq_core::plan::JoinPolicy::PreferBind] {
            let opts = QueryOptions { join_policy: policy, ..options() };
            let nt = sq_engine::query_to_string(s.clone(), &format!("{PREFIXES}{q}"), ResultFormat::NTriples, &opts).unwrap();
            let mut lines: Vec<&str> = nt.lines().collect();
            let n = lines.len();
            lines.sort_unstable();
            lines.dedup();
            assert_eq!(lines.len(), n, "{q} {policy:?}:\n{nt}");
        }
    }
}
