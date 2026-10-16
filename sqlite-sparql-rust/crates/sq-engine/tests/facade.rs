//! The query facade: prepare, query_to_string (every format and form) and explain.
mod common;

use std::rc::Rc;

use common::*;
use sq_core::eval_api::SolutionSource;
use sq_core::ir::QueryForm;
use sq_core::plan::{JoinPolicy, QueryOptions};
use sq_core::store::QuadStore;
use sq_core::Error;
use sq_exec::ExecCaches;
use sq_plan::ExplainMode;
use sq_results::ResultFormat;

const DATA: &str = ":a :name \"Alice\" ; :knows :b . :b :name \"Bob\" . :g { :a :age 30 }";

fn store() -> Rc<dyn QuadStore> {
    Rc::new(mem(DATA))
}

fn run(q: &str, fmt: ResultFormat) -> sq_core::Result<String> {
    sq_engine::query_to_string(store(), &format!("{PREFIXES}{q}"), fmt, &options())
}

#[test]
fn select_in_every_solution_format() {
    let q = "SELECT ?n { ?s :name ?n } ORDER BY ?n";
    let json = run(q, ResultFormat::Json).unwrap();
    assert!(json.contains("\"vars\":[\"n\"]") && json.contains("\"Alice\"") && json.contains("\"Bob\""), "{json}");
    assert!(run(q, ResultFormat::Xml).unwrap().contains("<literal>Alice</literal>"));
    assert_eq!(run(q, ResultFormat::Csv).unwrap(), "n\r\nAlice\r\nBob\r\n");
    assert_eq!(run(q, ResultFormat::Tsv).unwrap(), "?n\n\"Alice\"\n\"Bob\"\n");
}

#[test]
fn ask_construct_and_describe() {
    assert!(run("ASK { :a :knows :b }", ResultFormat::Json).unwrap().contains("true"));
    assert!(run("ASK { :b :knows :a }", ResultFormat::Xml).unwrap().contains("false"));
    let nt = run("CONSTRUCT { ?o :knownBy ?s } WHERE { ?s :knows ?o }", ResultFormat::NTriples).unwrap();
    assert_eq!(nt.trim(), "<http://ex/b> <http://ex/knownBy> <http://ex/a> .");
    let ttl = run("DESCRIBE :b", ResultFormat::Turtle).unwrap();
    assert!(ttl.contains("Bob"), "{ttl}");
    let nq = run("CONSTRUCT WHERE { ?s :name ?n }", ResultFormat::NQuads).unwrap();
    assert_eq!(nq.lines().count(), 2);
}

#[test]
fn mismatching_formats_and_syntax_errors() {
    assert!(matches!(run("SELECT * {}", ResultFormat::NTriples), Err(Error::Invalid(_))));
    assert!(matches!(run("ASK {}", ResultFormat::Turtle), Err(Error::Invalid(_))));
    assert!(matches!(run("CONSTRUCT WHERE { ?s ?p ?o }", ResultFormat::Json), Err(Error::Invalid(_))));
    assert!(matches!(run("SELECT ?x WHERE {", ResultFormat::Json), Err(Error::Syntax(_))));
    assert!(matches!(run("SELECT * { SERVICE <http://x/> { ?s ?p ?o } }", ResultFormat::Json), Err(Error::Unsupported(_))));
}

#[test]
fn base_iri_comes_from_the_options() {
    let s = store();
    let opts = QueryOptions { base_iri: Some("http://ex/".into()), ..QueryOptions::default() };
    let out = sq_engine::query_to_string(s.clone(), "SELECT ?n { <a> <name> ?n }", ResultFormat::Csv, &opts).unwrap();
    assert_eq!(out, "n\r\nAlice\r\n");
    let none = sq_engine::query_to_string(s, "SELECT ?n { <a> <name> ?n }", ResultFormat::Csv, &QueryOptions::default());
    assert!(matches!(none, Err(Error::Syntax(_))), "relative IRIs need a base");
}

#[test]
fn prepare_streams_solutions_under_every_policy() {
    for policy in [JoinPolicy::Auto, JoinPolicy::PreferHash, JoinPolicy::PreferBind, JoinPolicy::PreferMerge, JoinPolicy::NoSql] {
        for reorder in [true, false] {
            let opts = QueryOptions { join_policy: policy, reorder, ..options() };
            let q = sq_plan::parse_query(
                &format!("{PREFIXES}SELECT ?n ?m {{ ?s :knows ?o . ?s :name ?n . ?o :name ?m }}"),
                None,
            )
            .unwrap();
            let mut exec = sq_engine::prepare(store(), &q, &opts, ExecCaches::default()).unwrap();
            assert!(matches!(exec.form(), QueryForm::Select(v) if v.len() == 2));
            assert_eq!(exec.variables(), ["n", "m"]);
            let mut ids = Vec::new();
            assert_eq!(exec.next_rows(&mut ids, 10).unwrap(), 1);
            let mut terms = Vec::new();
            exec.decode(&ids, &mut terms).unwrap();
            let shown: Vec<String> = terms.iter().map(|t| show(t.as_deref().unwrap())).collect();
            assert_eq!(shown, ["\"Alice\"", "\"Bob\""], "{policy:?} reorder={reorder}");
        }
    }
}

#[test]
fn count_and_ask_through_prepare() {
    let q = sq_plan::parse_query(&format!("{PREFIXES}SELECT * {{ ?s ?p ?o }}"), None).unwrap();
    let mut exec = sq_engine::prepare(store(), &q, &options(), ExecCaches::default()).unwrap();
    assert_eq!(exec.count().unwrap(), 3);
    let q = sq_plan::parse_query(&format!("{PREFIXES}ASK {{ GRAPH ?g {{ ?s :age 30 }} }}"), None).unwrap();
    let mut exec = sq_engine::prepare(store(), &q, &options(), ExecCaches::default()).unwrap();
    assert!(exec.ask().unwrap());
}

fn explain(q: &str, mode: ExplainMode) -> String {
    sq_engine::explain(store(), &format!("{PREFIXES}{q}"), mode, &options()).unwrap()
}

#[test]
fn explain_modes() {
    let q = "SELECT ?n { ?s :knows ?o . ?o :name ?n FILTER(?n != \"x\") }";
    let algebra = explain(q, ExplainMode::Algebra);
    assert!(algebra.contains("(project (?n)") && algebra.contains("(bgp"), "{algebra}");
    let logical = explain(q, ExplainMode::Logical);
    assert!(logical.contains("<http://ex/knows>") || logical.contains(":knows"), "dictionary constants are decoded: {logical}");
    let plan = explain(q, ExplainMode::Plan);
    assert!(plan.contains("Scan") && plan.contains("est="), "{plan}");
    assert!(plan.contains("http://ex/name"), "{plan}");
    let analyze = explain(q, ExplainMode::Analyze);
    assert!(analyze.contains("rows="), "{analyze}");
    assert!(analyze.contains("-- 1 rows"), "{analyze}");
    let ask = explain("ASK { ?s :knows ?o }", ExplainMode::Analyze);
    assert!(ask.contains("-- ASK true"), "{ask}");
    let construct = explain("CONSTRUCT { ?o :x ?s } WHERE { ?s :knows ?o }", ExplainMode::Analyze);
    assert!(construct.contains("-- 1 triples"), "{construct}");
    assert!(matches!(
        sq_engine::explain(store(), "SELECT", ExplainMode::Plan, &options()),
        Err(Error::Syntax(_))
    ));
}

#[test]
fn queries_run_on_sqlite() {
    let db = Sqlite::new();
    let scope = db.store.write_scope(0).unwrap();
    load_trig(&db.store, DATA);
    scope.commit().unwrap();
    let read = db.store.read_scope(0).unwrap();
    db.store.shared().validate_epoch().unwrap();
    let out = sq_engine::query_to_string(
        db.rc(),
        &format!("{PREFIXES}SELECT ?n {{ ?s :knows/:name ?n }}"),
        ResultFormat::Csv,
        &options(),
    )
    .unwrap();
    assert_eq!(out, "n\r\nBob\r\n");
    let plan = sq_engine::explain(db.rc(), &format!("{PREFIXES}SELECT * {{ ?s :name ?n }}"), ExplainMode::Analyze, &options()).unwrap();
    assert!(plan.contains("-- 2 rows"), "{plan}");
    read.close().unwrap();
}
