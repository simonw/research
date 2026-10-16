//! Adversarial review (WP10, round 2): SPARQL 1.1 Update semantics probes (all pass: kept as
//! regression tests for computed graph IRIs, WITH + GRAPH ?h, non-canonical literals, stored
//! blank nodes, sub-select scoping, LOAD blank-node freshness, and new terms on SQLite).
mod common;

use std::rc::Rc;

use common::*;
use sq_core::store::{QuadStore, QuadStoreWriter};
use sq_engine::UpdateReport;

fn report(inserted: u64, deleted: u64) -> UpdateReport {
    UpdateReport { inserted, deleted }
}

fn g(s: &sq_memstore::MemStore, n: &str) -> Option<sq_core::TermId> {
    s.dict().lookup(oxrdf::NamedNodeRef::new_unchecked(&format!("{EX}{n}")).into()).unwrap()
}

#[test]
fn review_computed_graph_iri_in_insert_template() {
    let s = mem(":a :p 1 .");
    let r = update(&s, "INSERT { GRAPH ?g { :a :p 2 } } WHERE { BIND(IRI(CONCAT(STR(:), \"newg\")) AS ?g) }").unwrap();
    assert_eq!(r, report(1, 0));
    assert!(s.named_graphs().unwrap().contains(&g(&s, "newg").unwrap()));
}

#[test]
fn review_with_and_graph_var_in_where() {
    let s = mem(":g1 { :a :p 1 } :g2 { :a :p 1 . :b :p 2 }");
    // WITH :g1: templates write :g1; WHERE default graph is :g1, named graphs = all.
    let r = update(&s, "WITH :g1 INSERT { ?s :from ?h } WHERE { GRAPH ?h { ?s :p ?o } }").unwrap();
    assert_eq!(r, report(3, 0));
    assert_eq!(select_mem(&s, "SELECT ?s ?h { GRAPH :g1 { ?s :from ?h } }").len(), 3);
}

#[test]
fn review_non_canonical_literals_in_data_ops() {
    let s = mem(":a :p \"01\"^^xsd:integer , 1 .");
    let r = update(&s, "DELETE DATA { :a :p 1 }").unwrap();
    assert_eq!(r, report(0, 1));
    assert_eq!(dump_mem(&s), ["(default) :a :p \"01\"^^<http://www.w3.org/2001/XMLSchema#integer>"]);
    let r = update(&s, "DELETE { :a :p ?v } WHERE { BIND(STRDT(\"01\", xsd:integer) AS ?v) }").unwrap();
    assert_eq!(r, report(0, 1));
    assert!(dump_mem(&s).is_empty());
    let r = update(&s, "INSERT DATA { :a :p \"01\"^^xsd:integer } ; DELETE WHERE { :a :p \"01\"^^xsd:integer }").unwrap();
    assert_eq!(r, report(1, 1));
}

#[test]
fn review_delete_stored_bnodes_via_variables() {
    let s = mem("_:x :p 1 . _:y :p 2 . :a :q _:x .");
    let r = update(&s, "DELETE { ?s :p ?o . ?z :q ?s } WHERE { ?s :p ?o OPTIONAL { ?z :q ?s } FILTER(isBlank(?s)) }").unwrap();
    assert_eq!(r, report(0, 3));
}

#[test]
fn review_invalid_computed_iri_is_not_stored() {
    let s = mem(":a :p 1 .");
    let _ = update(&s, "INSERT { ?x :p 1 } WHERE { BIND(IRI(\"a b\") AS ?x) }");
    for line in dump_mem(&s) {
        assert!(!line.contains("a b"), "invalid IRI stored: {line}");
    }
}

#[test]
fn review_subselect_hidden_vars_not_visible_in_templates() {
    let s = mem(":a :p :b .");
    let r = update(&s, "INSERT { :x :y ?o } WHERE { { SELECT ?s { ?s :p ?o } } }").unwrap();
    assert_eq!(r, report(0, 0));
    let r = update(&s, "INSERT { :x :z ?o } WHERE { ?s :p ?q MINUS { ?s :p ?o FILTER(false) } }").unwrap();
    assert_eq!(r, report(0, 0));
    let r = update(&s, "INSERT { :x :w ?o } WHERE { ?s :p ?q FILTER EXISTS { ?s :p ?o } }").unwrap();
    assert_eq!(r, report(0, 0));
}

#[test]
fn review_insert_bnode_template_across_graphs_one_node_per_solution() {
    let s = mem(":a :p 1 . :b :p 2 .");
    update(&s, "INSERT { _:n :v ?o . GRAPH :g { _:n :s ?s } } WHERE { ?s :p ?o }").unwrap();
    assert_eq!(bnode_count(&s), 2);
    assert_eq!(select_mem(&s, "SELECT ?s ?o { ?n :v ?o GRAPH :g { ?n :s ?s } }").len(), 2);
}

#[test]
fn review_load_twice_fresh_bnodes() {
    let dir = std::env::temp_dir().join(format!("sq-engine-review-load-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("d.ttl");
    std::fs::write(&file, "_:x <http://ex/p> 1 .").unwrap();
    let s = mem("");
    let u = sq_plan::parse_update(&format!("LOAD <file://{0}> ; LOAD <file://{0}>", file.display()), None).unwrap();
    let uopts = sq_engine::UpdateOptions { allow_files: true, now: None };
    let r = sq_engine::update_with(Rc::new(s.clone()), &u, &options(), &uopts).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(r, report(2, 0));
    assert_eq!(bnode_count(&s), 2);
}

#[test]
fn review_delete_insert_same_graph_var_unbound_skips() {
    let s = mem(":g1 { :a :p 1 } :a :p 1 .");
    // ?g unbound in the default-graph solution: the GRAPH ?g template quad is skipped.
    let r = update(&s, "DELETE { GRAPH ?g { ?s :p ?o } } WHERE { { ?s :p ?o } UNION { GRAPH ?g { ?s :p ?o } } }").unwrap();
    assert_eq!(r, report(0, 1));
    assert_eq!(dump_mem(&s), ["(default) :a :p \"1\"^^<http://www.w3.org/2001/XMLSchema#integer>"]);
}

#[test]
fn review_copy_move_add_registry() {
    let s = mem(":g1 { :a :p 1 }");
    update(&s, "CREATE GRAPH :empty").unwrap();
    // COPY an existing empty graph to :g1: :g1 becomes empty.
    update(&s, "COPY :empty TO :g1").unwrap();
    assert!(select_mem(&s, "SELECT * { GRAPH :g1 { ?s ?p ?o } }").is_empty());
}

#[test]
fn review_using_with_graph_not_in_store() {
    let s = mem(":a :p 1 . :g1 { :a :p 1 }");
    let r = update(&s, "DELETE { ?s ?p ?o } USING :nowhere WHERE { ?s ?p ?o }").unwrap();
    assert_eq!(r, report(0, 0));
    let r = update(&s, "WITH :nowhere DELETE { ?s ?p ?o } WHERE { ?s ?p ?o }").unwrap();
    assert_eq!(r, report(0, 0));
    assert_eq!(s.quads().len(), 2);
}

#[test]
fn review_insert_data_into_default_with_union_option_and_clear() {
    let s = mem(":a :p 1 . :g1 { :b :p 2 }");
    let union = sq_core::plan::QueryOptions { union_default_graph: true, ..options() };
    let u = sq_plan::parse_update(&format!("{PREFIXES}CLEAR DEFAULT"), Some(EX)).unwrap();
    let r = sq_engine::update(Rc::new(s.clone()), &u, &union).unwrap();
    assert_eq!(r, report(0, 1));
    assert_eq!(s.quads().len(), 1);
}

#[test]
fn review_insert_where_with_values_graph_iri_temp_registered() {
    let s = mem("");
    let r = update(&s, "INSERT { GRAPH ?g { :a :b ?g } } WHERE { VALUES ?g { :fresh1 :fresh2 } }").unwrap();
    assert_eq!(r, report(2, 0));
    assert_eq!(s.named_graphs().unwrap().len(), 2);
}

#[test]
fn review_writer_trait_object_dyn() {
    // The facade takes Rc<dyn QuadStoreWriter>.
    let s = mem(":a :p 1 .");
    let w: Rc<dyn QuadStoreWriter> = Rc::new(s.clone());
    let u = sq_plan::parse_update(&format!("{PREFIXES}DELETE WHERE {{ ?s ?p ?o }}"), Some(EX)).unwrap();
    let r = sq_engine::update(w, &u, &options()).unwrap();
    assert_eq!(r, report(0, 1));
}

#[test]
fn review_sqlite_new_terms_visible_within_request() {
    let db = Sqlite::new();
    let scope = db.store.write_scope(0).unwrap();
    load_trig(&db.store, ":a :p 1 .");
    let r = update_on(
        db.rc(),
        "INSERT DATA { :brandnew :p \"a long brand new literal\" . GRAPH :ng { :brandnew :p 2 } } ;
         INSERT { ?s :q ?o } WHERE { ?s :p \"a long brand new literal\" BIND(STRLEN(\"x\") AS ?o) } ;
         DELETE DATA { :brandnew :p \"a long brand new literal\" } ;
         DELETE WHERE { GRAPH :ng { :brandnew :p ?x } } ;
         DROP GRAPH :ng",
    )
    .unwrap();
    assert_eq!(r, report(3, 2));
    scope.commit().unwrap();
    let store: Rc<dyn QuadStore> = db.rc();
    assert_eq!(select(store.clone(), "SELECT ?s ?o { ?s :q ?o }"), ["?s=:brandnew ?o=\"1\"^^<http://www.w3.org/2001/XMLSchema#integer>"]);
    assert!(store.named_graphs().unwrap().is_empty());
}

#[test]
fn review_computed_graph_iri_in_delete_template_and_where() {
    let s = mem(":g1 { :a :p 1 . :b :p 2 } :g0 { :a :p 1 }");
    let r = update(&s, "DELETE { GRAPH ?g { ?s ?p ?o } } WHERE { BIND(IRI(CONCAT(STR(:), \"g1\")) AS ?g) GRAPH ?g { ?s ?p ?o } }").unwrap();
    assert_eq!(r, report(0, 2));
    assert_eq!(dump_mem(&s), [":g0 :a :p \"1\"^^<http://www.w3.org/2001/XMLSchema#integer>"]);
    let r = update(&s, "DELETE { ?s :p ?o } INSERT { ?s :p ?n } WHERE { GRAPH :g0 { ?s :p ?o } BIND(?o + 1 AS ?n) } ; INSERT { ?s :q ?o } WHERE { ?s :p ?o }").unwrap();
    assert_eq!(r, report(2, 0));
}
