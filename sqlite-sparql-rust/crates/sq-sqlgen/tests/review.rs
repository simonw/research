//! Adversarial review tests (WP9b review). Each test pins the answer the SPARQL 1.1 spec (and
//! docs/SPEC.md §5.5 / §5.6 / §6.4) requires for a query the baseline accepts; a failure means
//! the generated SQL returns a wrong answer, or (`review_accepted_queries_hit_sqlite_limits`)
//! that an accepted query cannot be executed at all.
mod common;

use common::{Baseline, DataFile, Engine, Outcome};

const DATA: &str = r#"
@prefix : <http://ex.org/> .
:alice :name "Alice" ; :knows :bob ; :rank 1 .
:bob :name "Bob" .
:p1 :sub :p2 . :p2 :sub :p3 .
:g1 { :alice :likes :tea . :p1 :sub :p9 . }
"#;

const PREFIX: &str = "PREFIX : <http://ex.org/> ";

/// The baseline's answer (panics when the query is unsupported or fails).
fn run(engine: &Engine, q: &str) -> (Outcome, String) {
    let text = format!("{PREFIX}{q}");
    let query = sq_plan::parse_query(&text, None).unwrap();
    match engine.baseline(&query).unwrap() {
        Baseline::Ran { sql, outcome } => (outcome, sql.sql),
        Baseline::Unsupported(msg) => panic!("{q}: unexpectedly unsupported: {msg}"),
    }
}

fn rows(o: &Outcome) -> Vec<Vec<String>> {
    let Outcome::Solutions { rows, .. } = o else { panic!("not solutions") };
    let mut out: Vec<Vec<String>> = rows
        .iter()
        .map(|r| r.iter().map(|t| t.as_ref().map_or("UNDEF".to_owned(), |t| t.to_string())).collect())
        .collect();
    out.sort();
    out
}

/// `GRAPH <c>` over an IRI that is not a named graph of the dataset has no solutions (spec
/// 18.6 `eval(D(G), Graph(IRI, P))` is empty when the IRI is not in the dataset; docs/SPEC.md
/// §5.6, DECISIONS "GRAPH <c> checks dataset membership"). `:alice` and `:bob` are stored IRIs
/// (so they are not TEMP ids) but not graphs. The translator skips the membership check when
/// the body "reads the graph in every solution" (`GClass::GForm`), but a zero-length path from
/// a constant (or EXISTS-substituted) endpoint produces a solution without reading any quad.
#[test]
fn review_graph_const_not_a_named_graph_with_zero_length_path() {
    let engine = Engine::new(&[DataFile::trig(DATA)]).unwrap();
    for q in [
        "SELECT ?y WHERE { GRAPH :alice { :p1 :sub* ?y } }",
        "SELECT ?y WHERE { GRAPH :alice { :p1 :sub? ?y } }",
        "SELECT ?y WHERE { GRAPH :alice { { :p1 :sub* ?y } UNION { :p2 :sub? ?y } } }",
        "SELECT ?s WHERE { ?s :name ?n FILTER EXISTS { GRAPH :bob { ?s :knows* ?y } } }",
    ] {
        let (out, sql) = run(&engine, q);
        assert!(rows(&out).is_empty(), "{q}: expected no solutions, got {:?}\nSQL: {sql}", rows(&out));
    }
    let (out, sql) = run(&engine, "ASK { GRAPH :bob { :p1 :sub* :p1 } }");
    assert!(matches!(out, Outcome::Boolean(false)), "expected false, got {out:?}\nSQL: {sql}");
    // Control: the same path in an existing named graph matches the constant itself.
    let (out, _) = run(&engine, "SELECT ?y WHERE { GRAPH :g1 { :p2 :sub* ?y } }");
    assert_eq!(rows(&out), vec![vec!["<http://ex.org/p2>".to_owned()]]);
}

/// With `FROM` and no `FROM NAMED` the dataset has no named graphs, so `GRAPH ?g { … }` has no
/// solutions — and in any case every solution of `GRAPH ?g` binds ?g. The pushed translation
/// sees `QuadSrc::Nothing`, keeps the zero-length pairs of the constant endpoint, and never
/// binds ?g: the result holds a row with ?g unbound.
#[test]
fn review_graph_var_without_named_graphs_and_zero_length_path() {
    let engine = Engine::new(&[DataFile::trig(DATA)]).unwrap();
    for q in [
        "SELECT ?g ?y FROM :g1 WHERE { GRAPH ?g { :p1 :sub* ?y } }",
        "SELECT ?g ?y FROM :g1 WHERE { GRAPH ?g { :p1 :sub? ?y } }",
    ] {
        let (out, sql) = run(&engine, q);
        assert!(rows(&out).is_empty(), "{q}: expected no solutions, got {:?}\nSQL: {sql}", rows(&out));
    }
    let (out, sql) = run(&engine, "ASK FROM :g1 { GRAPH ?g { :p1 (:sub|:other)* :p1 } }");
    assert!(matches!(out, Outcome::Boolean(false)), "expected false, got {out:?}\nSQL: {sql}");
}

const NUMERIC_DATA: &str = r#"
@prefix : <http://ex.org/> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
:a :v "1.1"^^xsd:float .
:b :v "0.7"^^xsd:float .
:c :v "16777217"^^xsd:float .
:d :v 9007199254740992e0 .
:e :v 1.1 .
"#;

/// FILTER comparisons in a "truth only" context compare `sparql_num(x)` (an f64 REAL) with
/// the constant. XPath numeric promotion (SPARQL 17.3, docs/SPEC.md §6.4) instead converts
/// the decimal / integer operand to the *float* (or double) type of the other operand, so
/// `"1.1"^^xsd:float > 1.1` is false (equal after promotion) while REAL(1.1f32) =
/// 1.100000023841858 > 1.1. The NOT form goes through `sparql_eval` and is right, so the
/// baseline even answers `?f > 1.1` and `!(?f > 1.1)` both true for the same row.
#[test]
fn review_truth_context_numeric_comparisons_ignore_float_promotion() {
    let engine = Engine::new(&[DataFile::turtle(NUMERIC_DATA)]).unwrap();
    let cases: &[(&str, &[&str])] = &[
        // "1.1"^^xsd:float equals 1.1 after promotion to float, so it is not > 1.1.
        ("SELECT ?x WHERE { ?x :v ?f FILTER(?f > 1.1) }", &["c", "d"]),
        ("SELECT ?x WHERE { ?x :v ?f FILTER(?f >= 1.1 && ?f <= 1.1) }", &["a", "e"]),
        ("SELECT ?x WHERE { ?x :v ?f FILTER(?f < 0.7) }", &[]),
        // "16777217"^^xsd:float is 16777216f; 16777217 promoted to float is 16777216f too.
        ("SELECT ?x WHERE { ?x :v ?f FILTER(?f = 16777217) }", &["c"]),
        ("SELECT ?x WHERE { ?x :v ?f FILTER(?f < 16777217 && ?f > 16777215) }", &[]),
        // 9007199254740993 promoted to double is 9007199254740992e0.
        ("SELECT ?x WHERE { ?x :v ?f FILTER(?f < 9007199254740993 && ?f > 9007199254740991) }", &[]),
    ];
    let mut failures = Vec::new();
    for (q, want) in cases {
        let (out, sql) = run(&engine, q);
        let want: Vec<Vec<String>> = want.iter().map(|x| vec![format!("<http://ex.org/{x}>")]).collect();
        if rows(&out) != want {
            failures.push(format!("{q}\n  got {:?}, want {want:?}\n  SQL: {sql}", rows(&out)));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Queries the translator accepts must run: SQLite's compile-time limits (64 tables per join,
/// expression depth 1000, 500 terms per compound SELECT, the 3.45 parser's fixed 100-entry
/// stack) turn these into prepare errors instead of answers or a clean
/// `Error::Unsupported("baseline: …")`.
#[test]
fn review_accepted_queries_hit_sqlite_limits() {
    let engine = Engine::new(&[DataFile::trig(DATA)]).unwrap();
    let star: Vec<String> = (0..65).map(|i| format!(":alice :knows ?x{i} .")).collect();
    let ints: Vec<String> = (0..1100).map(|i| i.to_string()).collect();
    let union: Vec<String> = (0..501).map(|i| format!("{{ ?s :rank {i} }}")).collect();
    let mut nested = "?s :knows ?o".to_owned();
    for _ in 0..12 {
        nested = format!("?s :knows ?o FILTER EXISTS {{ {nested} }}");
    }
    let queries = [
        format!("SELECT (COUNT(*) AS ?c) WHERE {{ {} }}", star.join(" ")),
        format!("SELECT ?s WHERE {{ ?s :rank ?a FILTER(?a IN ({})) }}", ints.join(", ")),
        format!("SELECT ?s WHERE {{ {} }}", union.join(" UNION ")),
        format!("SELECT (COUNT(*) AS ?c) WHERE {{ {nested} }}"),
    ];
    let mut failures = Vec::new();
    for q in &queries {
        let query = sq_plan::parse_query(&format!("{PREFIX}{q}"), None).unwrap();
        match engine.baseline(&query) {
            Ok(_) => {}
            Err(e) => failures.push(format!("{}…: {}", &q[..60], e.lines().next().unwrap_or(""))),
        }
    }
    assert!(failures.is_empty(), "accepted but not executable:\n{}", failures.join("\n"));
}
