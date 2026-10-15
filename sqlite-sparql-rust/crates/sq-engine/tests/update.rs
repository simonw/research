//! SPARQL Update executor (docs/SPEC.md §5.7) on MemStore, plus one end-to-end run on an
//! in-memory SQLite store inside a write scope.
mod common;

use std::rc::Rc;

use common::*;
use sq_core::store::{QuadStore, QuadStoreWriter};
use sq_core::Error;
use sq_engine::{UpdateOptions, UpdateReport};

fn report(inserted: u64, deleted: u64) -> UpdateReport {
    UpdateReport { inserted, deleted }
}

#[test]
fn insert_data_interns_terms_and_registers_graphs() {
    let s = mem("");
    let r = update(&s, "INSERT DATA { :a :p 1, \"x\"@en . GRAPH :g { :a :p \"long literal value\" } }").unwrap();
    assert_eq!(r, report(3, 0));
    assert_eq!(
        dump_mem(&s),
        [
            "(default) :a :p \"1\"^^<http://www.w3.org/2001/XMLSchema#integer>",
            "(default) :a :p \"x\"@en",
            ":g :a :p \"long literal value\"",
        ]
    );
    assert_eq!(s.named_graphs().unwrap().len(), 1);
    // Inserting existing quads adds nothing.
    assert_eq!(update(&s, "INSERT DATA { :a :p 1 }").unwrap(), report(0, 0));
}

#[test]
fn insert_data_blank_nodes_are_fresh_per_operation() {
    let s = mem("");
    // spargebra rejects a label shared by two INSERT DATA blocks of one request, so the
    // freshness across operations is checked across requests (same label, new node).
    update(&s, "INSERT DATA { _:x :p _:x . _:y :q _:x } ; INSERT DATA { _:z :p 2 }").unwrap();
    update(&s, "INSERT DATA { _:x :p 3 }").unwrap();
    assert_eq!(bnode_count(&s), 4);
    assert_eq!(select_mem(&s, "SELECT ?y { ?x :p ?x . ?y :q ?x }").len(), 1);
}

#[test]
fn delete_data_is_lookup_only() {
    let s = mem(":a :p 1 , 2 . :g { :a :p 1 }");
    let r = update(&s, "DELETE DATA { :a :p 1 . :a :p :never . :unknown :p 2 . GRAPH :nog { :a :p 1 } }").unwrap();
    assert_eq!(r, report(0, 1));
    assert_eq!(
        dump_mem(&s),
        ["(default) :a :p \"2\"^^<http://www.w3.org/2001/XMLSchema#integer>", ":g :a :p \"1\"^^<http://www.w3.org/2001/XMLSchema#integer>"]
    );
    assert!(s.dict().lookup(oxrdf::NamedNodeRef::new_unchecked("http://ex/never").into()).unwrap().is_none());
}

#[test]
fn where_solutions_are_materialized_before_writing() {
    // Without materialization this insert would keep finding the triples it adds.
    let s = mem(":a :n 1 . :b :n 2 .");
    let r = update(&s, "INSERT { ?s :n ?m } WHERE { ?s :n ?v BIND(?v + 10 AS ?m) }").unwrap();
    assert_eq!(r, report(2, 0));
    assert_eq!(select_mem(&s, "SELECT ?v { ?s :n ?v }").len(), 4);
    // DELETE and INSERT see the same solutions; deletes run first.
    let r = update(&s, "DELETE { ?s :n ?v } INSERT { ?s :m ?v } WHERE { ?s :n ?v FILTER(?v < 10) }").unwrap();
    assert_eq!(r, report(2, 2));
    assert_eq!(select_mem(&s, "SELECT ?s ?v { ?s :m ?v }"), ["?s=:a ?v=\"1\"^^<http://www.w3.org/2001/XMLSchema#integer>", "?s=:b ?v=\"2\"^^<http://www.w3.org/2001/XMLSchema#integer>"]);
}

#[test]
fn deleting_and_reinserting_a_quad_keeps_it() {
    let s = mem(":a :p :b .");
    let r = update(&s, "DELETE { ?s :p ?o } INSERT { ?s :p ?o } WHERE { ?s :p ?o }").unwrap();
    assert_eq!(r, report(1, 1));
    assert_eq!(dump_mem(&s), ["(default) :a :p :b"]);
}

#[test]
fn delete_templates_skip_unbound_and_unknown_positions() {
    let s = mem(":a :p :b . :c :p :d .");
    // ?x is never bound by the WHERE clause; :unknown is not in the store.
    let r = update(&s, "DELETE { ?s :p ?x . :unknown :p ?o . ?s :p ?o } WHERE { ?s :p ?o OPTIONAL { ?o :q ?y } }").unwrap();
    assert_eq!(r, report(0, 2));
    assert!(dump_mem(&s).is_empty());
}

#[test]
fn delete_where_and_with_using() {
    let s = mem(":a :p 1 . :g1 { :a :p 1 . :a :q 2 } :g2 { :a :q 3 }");
    // WITH: the WHERE dataset's default graph and the templates' default graph are :g1.
    let r = update(&s, "WITH :g1 DELETE { ?s :q ?o } WHERE { ?s :q ?o }").unwrap();
    assert_eq!(r, report(0, 1));
    // USING :g2 makes :g2 the WHERE default graph; templates still write the store default.
    let r = update(&s, "INSERT { ?s :copied ?o } USING :g2 WHERE { ?s :q ?o }").unwrap();
    assert_eq!(r, report(1, 0));
    let r = update(&s, "DELETE WHERE { GRAPH ?g { :a :p ?o } }").unwrap();
    assert_eq!(r, report(0, 1));
    assert_eq!(
        dump_mem(&s),
        [
            "(default) :a :copied \"3\"^^<http://www.w3.org/2001/XMLSchema#integer>",
            "(default) :a :p \"1\"^^<http://www.w3.org/2001/XMLSchema#integer>",
            ":g2 :a :q \"3\"^^<http://www.w3.org/2001/XMLSchema#integer>",
        ]
    );
}

#[test]
fn insert_templates_make_fresh_blank_nodes_per_solution() {
    let s = mem(":a :p 1 . :b :p 2 .");
    update(&s, "INSERT { ?s :has _:n . _:n :value ?v . _:n :also _:m } WHERE { ?s :p ?v }").unwrap();
    // Two solutions x two labels.
    assert_eq!(bnode_count(&s), 4);
    let rows = select_mem(&s, "SELECT ?s ?v { ?s :has ?n . ?n :value ?v ; :also ?m }");
    assert_eq!(rows.len(), 2);
    assert!(rows[0].starts_with("?s=:a") && rows[1].starts_with("?s=:b"));
}

#[test]
fn computed_values_are_interned_and_illegal_quads_skipped() {
    let s = mem(":a :name \"Alice\" ; :age 30 .");
    let r = update(
        &s,
        "INSERT { ?s :label ?l . ?s :next ?n . ?l :bad ?s . ?s ?l 1 . GRAPH ?l { ?s :p 1 } . ?s :iri ?i }
         WHERE { ?s :name ?name ; :age ?age
                 BIND(CONCAT(?name, \" (\", STR(?age), \")\") AS ?l)
                 BIND(?age * 1.5 AS ?n)
                 BIND(IRI(CONCAT(\"http://ex/\", ?name)) AS ?i) }",
    )
    .unwrap();
    // Literal subject, literal predicate and literal graph are skipped.
    assert_eq!(r, report(3, 0));
    assert_eq!(
        select_mem(&s, "SELECT ?l ?n ?i { :a :label ?l ; :next ?n ; :iri ?i }"),
        ["?l=\"Alice (30)\" ?n=\"45\"^^<http://www.w3.org/2001/XMLSchema#decimal> ?i=:Alice"]
    );
    // The computed IRI is now a dictionary term: a later query constant finds it.
    assert_eq!(select_mem(&s, "SELECT ?s { ?s :iri :Alice }"), ["?s=:a"]);
}

#[test]
fn graph_variables_must_be_bound_to_iris() {
    let s = mem("");
    let r = update(&s, "INSERT { GRAPH ?g { :a :b :c } } WHERE { VALUES ?g { :g1 \"lit\" UNDEF } }").unwrap();
    assert_eq!(r, report(1, 0));
    assert_eq!(dump_mem(&s), [":g1 :a :b :c"]);
    let r = update(&s, "INSERT { GRAPH ?g { :a :b :c } } WHERE { BIND(BNODE() AS ?g) }").unwrap();
    assert_eq!(r, report(0, 0));
}

#[test]
fn bnode_values_from_where_become_one_store_node_per_operation() {
    let s = mem(":a :p 1 . :b :p 2 .");
    // BNODE(str) gives the same blank node for equal arguments within a solution scope; the
    // TEMP value maps to a single new store blank node.
    update(&s, "INSERT { ?x :from ?s } WHERE { ?s :p ?v BIND(BNODE(\"k\") AS ?x) }").unwrap();
    let n = bnode_count(&s);
    assert!((1..=2).contains(&n), "{n} blank nodes");
    // Stored blank nodes in solutions are copied as they are.
    update(&s, "INSERT { ?o :back ?x } WHERE { ?x :from ?o }").unwrap();
    assert_eq!(bnode_count(&s), n);
}

#[test]
fn each_operation_sees_the_previous_ones() {
    let s = mem("");
    // :new is absent when the request is parsed; the second operation is lowered after the
    // first inserted it, so its constant resolves to the stored id.
    let r = update(
        &s,
        "INSERT DATA { :x :p :new } ;
         INSERT { ?s :q :done } WHERE { ?s :p :new } ;
         DELETE WHERE { :x :p :new }",
    )
    .unwrap();
    assert_eq!(r, report(2, 1));
    assert_eq!(dump_mem(&s), ["(default) :x :q :done"]);
}

#[test]
fn now_is_fixed_for_the_whole_request() {
    let s = mem("");
    update(&s, "INSERT { :a :t ?n } WHERE { BIND(NOW() AS ?n) } ; INSERT { :b :t ?n } WHERE { BIND(NOW() AS ?n) }").unwrap();
    let rows = select_mem(&s, "SELECT ?t { ?s :t ?t }");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], rows[1]);
    assert!(rows[0].contains("2026-10-15T12:00:00"), "{rows:?}");
    // Without a fixed value both operations still share one.
    let u = sq_plan::parse_update(&format!("{PREFIXES}INSERT {{ :c :t ?n }} WHERE {{ BIND(NOW() AS ?n) }} ; INSERT {{ :d :t ?n }} WHERE {{ BIND(NOW() AS ?n) }}"), None).unwrap();
    sq_engine::update(Rc::new(s.clone()), &u, &options()).unwrap();
    let rows = select_mem(&s, "SELECT ?t { VALUES ?s { :c :d } ?s :t ?t }");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], rows[1]);
}

#[test]
fn clear_create_drop_follow_the_silent_rules() {
    let s = mem(":a :p 1 . :g1 { :a :p 1 } :g2 { :a :p 2 }");
    let g = |n: &str| s.dict().lookup(oxrdf::NamedNodeRef::new_unchecked(&format!("{EX}{n}")).into()).unwrap();
    assert!(matches!(update(&s, "CREATE GRAPH :g1"), Err(Error::Invalid(_))));
    assert_eq!(update(&s, "CREATE SILENT GRAPH :g1").unwrap(), report(0, 0));
    update(&s, "CREATE GRAPH :empty").unwrap();
    assert!(s.named_graphs().unwrap().contains(&g("empty").unwrap()));
    assert!(matches!(update(&s, "CREATE GRAPH :empty"), Err(Error::Invalid(_))));

    assert!(matches!(update(&s, "DROP GRAPH :missing"), Err(Error::Invalid(_))));
    assert!(matches!(update(&s, "CLEAR GRAPH :missing"), Err(Error::Invalid(_))));
    assert_eq!(update(&s, "DROP SILENT GRAPH :missing ; CLEAR SILENT GRAPH :missing").unwrap(), report(0, 0));

    // CLEAR keeps the (now empty) graph; DROP removes it.
    assert_eq!(update(&s, "CLEAR GRAPH :g1").unwrap(), report(0, 1));
    assert!(s.named_graphs().unwrap().contains(&g("g1").unwrap()));
    assert_eq!(update(&s, "DROP GRAPH :g1").unwrap(), report(0, 0));
    assert!(!s.named_graphs().unwrap().contains(&g("g1").unwrap()));
    assert!(matches!(update(&s, "DROP GRAPH :g1"), Err(Error::Invalid(_))));

    // CLEAR NAMED empties every named graph but keeps them; DROP DEFAULT = CLEAR DEFAULT.
    assert_eq!(update(&s, "CLEAR NAMED").unwrap(), report(0, 1));
    assert_eq!(s.named_graphs().unwrap().len(), 2);
    assert_eq!(update(&s, "DROP DEFAULT").unwrap(), report(0, 1));
    assert!(s.quads().is_empty());

    // DROP ALL clears everything and empties the registry.
    update(&s, "INSERT DATA { :a :p 1 . GRAPH :g3 { :a :p 1 } }").unwrap();
    assert_eq!(update(&s, "DROP ALL").unwrap(), report(0, 2));
    assert!(s.quads().is_empty());
    assert!(s.named_graphs().unwrap().is_empty());

    update(&s, "INSERT DATA { :a :p 1 . GRAPH :g3 { :a :p 1 } } ; CREATE GRAPH :g4").unwrap();
    assert_eq!(update(&s, "DROP NAMED").unwrap(), report(0, 1));
    assert!(s.named_graphs().unwrap().is_empty());
    assert_eq!(update(&s, "CLEAR ALL").unwrap(), report(0, 1));
}

#[test]
fn add_move_copy_arrive_lowered() {
    let s = mem(":a :p 1 . :g1 { :b :p 2 } :g2 { :c :p 3 }");
    update(&s, "ADD :g1 TO :g2").unwrap();
    assert_eq!(select_mem(&s, "SELECT ?s { GRAPH :g2 { ?s :p ?o } }"), ["?s=:b", "?s=:c"]);
    update(&s, "COPY DEFAULT TO :g2").unwrap();
    assert_eq!(select_mem(&s, "SELECT ?s { GRAPH :g2 { ?s :p ?o } }"), ["?s=:a"]);
    update(&s, "MOVE :g1 TO DEFAULT").unwrap();
    assert_eq!(select_mem(&s, "SELECT ?s { ?s :p ?o }"), ["?s=:b"]);
    let g1 = s.dict().lookup(oxrdf::NamedNodeRef::new_unchecked("http://ex/g1").into()).unwrap().unwrap();
    assert!(!s.named_graphs().unwrap().contains(&g1));
    // MOVE from a missing graph fails unless SILENT (the final DROP of the source).
    assert!(update(&s, "MOVE :g1 TO :g5").is_err());
    update(&s, "MOVE SILENT :g1 TO :g5").unwrap();
}

#[test]
fn load_needs_allow_files_and_a_file_iri() {
    let s = mem("");
    let dir = std::env::temp_dir().join(format!("sq-engine-load-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("data.ttl");
    std::fs::write(&file, "<a> <http://ex/p> 1 . _:x <http://ex/p> _:x .").unwrap();
    let bad = dir.join("bad.ttl");
    std::fs::write(&bad, "<a> <http://ex/p> 1 . <broken").unwrap();
    let iri = |p: &std::path::Path| format!("file://{}", p.display());
    let run = |text: &str, allow_files: bool| {
        let u = sq_plan::parse_update(text, None).unwrap();
        let uopts = UpdateOptions { allow_files, now: None };
        sq_engine::update_with(Rc::new(s.clone()), &u, &options(), &uopts)
    };

    let e = run(&format!("LOAD <{}>", iri(&file)), false).unwrap_err();
    assert!(matches!(e, Error::Unsupported(_)), "{e}");
    assert_eq!(run(&format!("LOAD SILENT <{}>", iri(&file)), false).unwrap(), report(0, 0));
    assert!(matches!(run("LOAD <http://example.org/data.ttl>", true), Err(Error::Unsupported(_))));

    let r = run(&format!("LOAD <{}> INTO GRAPH <http://ex/g>", iri(&file)), true).unwrap();
    assert_eq!(r, report(2, 0));
    // Relative IRIs resolve against the document's IRI.
    let rows = select_mem(&s, "SELECT ?s { GRAPH :g { ?s :p 1 } }");
    assert_eq!(rows, [format!("?s=<file://{}>", dir.join("a").display())]);

    // A malformed document writes nothing, SILENT or not.
    assert!(matches!(run(&format!("LOAD <{}>", iri(&bad)), true), Err(Error::Parse(_))));
    assert_eq!(run(&format!("LOAD SILENT <{}>", iri(&bad)), true).unwrap(), report(0, 0));
    assert!(matches!(run(&format!("LOAD <{}>", iri(&dir.join("missing.ttl"))), true), Err(Error::Invalid(_))));
    assert_eq!(s.quads().len(), 2);

    // LOAD INTO GRAPH of an empty document still creates the graph.
    let empty = dir.join("empty.nt");
    std::fs::write(&empty, "").unwrap();
    run(&format!("LOAD <{}> INTO GRAPH <http://ex/e>", iri(&empty)), true).unwrap();
    let e = s.dict().lookup(oxrdf::NamedNodeRef::new_unchecked("http://ex/e").into()).unwrap().unwrap();
    assert!(s.named_graphs().unwrap().contains(&e));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn syntax_and_lowering_errors_surface() {
    let s = mem("");
    assert!(matches!(update(&s, "INSERT DATA { :a :p }"), Err(Error::Syntax(_))));
    // SERVICE inside WHERE is unsupported (non-silent).
    let e = update(&s, "INSERT { :a :p ?o } WHERE { SERVICE <http://remote/> { ?s :p ?o } }").unwrap_err();
    assert!(matches!(e, Error::Unsupported(_)), "{e}");
}

#[test]
fn updates_run_on_sqlite_inside_a_write_scope() {
    let db = Sqlite::new();
    let scope = db.store.write_scope(0).unwrap();
    load_trig(&db.store, ":a :n 1 . :b :n 2 . :g { :a :n 3 }");
    let r = update_on(
        db.rc(),
        "INSERT { ?s :m ?v2 } WHERE { ?s :n ?v BIND(?v * 2 AS ?v2) } ;
         DELETE WHERE { :b ?p ?o } ;
         INSERT DATA { _:x :q \"a string longer than seven bytes\" } ;
         DROP GRAPH :g",
    )
    .unwrap();
    assert_eq!(r, report(3, 3));
    scope.commit().unwrap();
    let store: Rc<dyn QuadStore> = db.rc();
    assert_eq!(select(store.clone(), "SELECT ?s ?v { ?s :m ?v }"), ["?s=:a ?v=\"2\"^^<http://www.w3.org/2001/XMLSchema#integer>"]);
    assert_eq!(select(store.clone(), "SELECT ?o { ?s :q ?o }"), ["?o=\"a string longer than seven bytes\""]);
    assert!(store.named_graphs().unwrap().is_empty());
    let w: Rc<dyn QuadStoreWriter> = db.rc();
    let scope = db.store.write_scope(0).unwrap();
    assert!(matches!(update_on(w, "DROP GRAPH :g"), Err(Error::Invalid(_))));
    scope.rollback().unwrap();
}
