//! Adversarial review tests for WP10 (sq-engine facade + update executor).
mod common;

use std::rc::Rc;

use common::*;
use sq_core::plan::QueryOptions;
use sq_core::store::QuadStore;
use sq_engine::UpdateOptions;

fn update_opts(store: &sq_memstore::MemStore, text: &str, opts: &QueryOptions) -> sq_core::Result<sq_engine::UpdateReport> {
    let u = sq_plan::parse_update(&format!("{PREFIXES}{text}"), Some(EX))?;
    let uopts = UpdateOptions { now: Some(fixed_now()), ..UpdateOptions::default() };
    sq_engine::update_with(Rc::new(store.clone()), &u, opts, &uopts)
}

#[test]
fn review_probe_bnode_str_distinct_per_solution() {
    let s = mem(":a :p 1 . :b :p 2 .");
    update(&s, "INSERT { ?x :from ?s } WHERE { ?s :p ?v BIND(BNODE(\"k\") AS ?x) }").unwrap();
    assert_eq!(bnode_count(&s), 2, "BNODE(\"k\") must differ between solutions");
}

#[test]
fn review_probe_group_error_key_in_insert() {
    let s = mem(":a :p 1 . :b :p 2 .");
    let r = update(&s, "INSERT { :x :k ?k ; :c ?c } WHERE { SELECT ?k (COUNT(*) AS ?c) { ?s :p ?o } GROUP BY (xsd:integer(\"abc\") AS ?k) }");
    eprintln!("{r:?}");
    eprintln!("{:?}", dump_mem(&s));
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn review_probe_union_default_move() {
    let s = mem(":a :p 1 . :g1 { :b :p 2 }");
    let opts = QueryOptions { union_default_graph: true, ..options() };
    update_opts(&s, "MOVE DEFAULT TO :g2", &opts).unwrap();
    eprintln!("{:?}", dump_mem(&s));
    assert_eq!(select_mem(&s, "SELECT ?s { GRAPH :g2 { ?s :p ?o } }"), ["?s=:a"]);
}

#[test]
fn review_probe_misc() {
    // USING NAMED only: the WHERE default graph is empty.
    let s = mem(":a :p 1 . :g1 { :b :p 2 } :g2 { :c :p 3 }");
    let r = update(&s, "INSERT { :x :saw ?s } USING NAMED :g1 WHERE { ?s :p ?o }").unwrap();
    eprintln!("using named, default: {r:?}");
    let r = update(&s, "INSERT { :x :sawg ?s } USING NAMED :g1 WHERE { GRAPH ?g { ?s :p ?o } }").unwrap();
    eprintln!("using named, graph: {r:?} {:?}", select_mem(&s, "SELECT ?s { :x :sawg ?s }"));
    // WITH + USING.
    let r = update(&s, "WITH :g1 INSERT { ?s :q 1 } USING :g2 WHERE { ?s :p ?o }").unwrap();
    eprintln!("with+using: {r:?} {:?}", select_mem(&s, "SELECT ?g ?s { GRAPH ?g { ?s :q 1 } }"));
    // New constants in VALUES / BIND.
    let r = update(&s, "INSERT { :a :v ?x . :a :w ?y } WHERE { VALUES ?x { \"brand new long literal\" :brandnew } BIND(:newer AS ?y) }").unwrap();
    eprintln!("values: {r:?} {:?}", select_mem(&s, "SELECT ?x ?y { :a :v ?x ; :w ?y }"));
    // STRLANG upper case.
    update(&s, "INSERT { :a :l ?l } WHERE { BIND(STRLANG(\"x\", \"EN\") AS ?l) }").unwrap();
    eprintln!("strlang: {:?} ask={:?}", select_mem(&s, "SELECT ?l { :a :l ?l }"), select_mem(&s, "SELECT * { :a :l \"x\"@en }"));
    // Graph var bound to a literal in DELETE.
    let r = update(&s, "DELETE { GRAPH ?g { :b :p 2 } } WHERE { VALUES ?g { \"g1\" } }").unwrap();
    eprintln!("delete lit graph: {r:?}");
    eprintln!("{:#?}", dump_mem(&s));
}

#[test]
fn review_probe_error_key_dump() {
    let s = mem(":a :p 1 . :b :p 2 .");
    let r = update(&s, "INSERT { :x :k ?k ; :c ?c } WHERE { SELECT ?k (COUNT(*) AS ?c) { ?s :p ?o } GROUP BY (xsd:integer(\"abc\") AS ?k) }");
    eprintln!("{r:?} {:?}", dump_mem(&s));
}

#[test]
fn review_probe_bnode_where_join_in_update() {
    // One BIND(BNODE()) solution joined with three rows: the spec gives all three ?s the
    // same blank node (BNODE is evaluated once per solution of its Extend).
    let s = mem(":a :p 1 . :b :p 2 . :c :p 3 . _:x :p 4 .");
    for policy in [sq_core::plan::JoinPolicy::Auto, sq_core::plan::JoinPolicy::PreferBind] {
        let opts = QueryOptions { join_policy: policy, ..options() };
        update_opts(&s, "DELETE { ?s :tag ?old } WHERE { ?s :tag ?old }", &opts).unwrap();
        update_opts(&s, "INSERT { ?s :tag ?b } WHERE { { BIND(BNODE() AS ?b) } UNION { VALUES ?s { \"x\"^^:dt } } ?s :p ?v }", &opts).unwrap();
        let tags = select_mem(&s, "SELECT DISTINCT ?b { ?s :tag ?b }");
        eprintln!("{policy:?}: {tags:?}");
    }
}
