//! The baseline translator executed on the same connection: its SQL calls this crate's
//! helper functions while this crate steps it (re-entrancy, rule R1), and its results flow
//! through the same output path. Every query here must give the native engine's answer.
mod common;

use common::*;
use rusqlite::Connection;

/// Solutions as sorted JSON bindings (or in order when `ordered`).
fn solutions(c: &Connection, q: &str, engine: &str, ordered: bool) -> Vec<String> {
    let opts = format!("{{\"engine\":\"{engine}\"}}");
    let mut st = c.prepare("SELECT json FROM sparql_rows(?1, ?2)").unwrap();
    let mut out: Vec<String> = st
        .query_map([q, &opts], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap_or_else(|e| panic!("{engine}: {q}: {e}"));
    if !ordered {
        out.sort();
    }
    out
}

const QUERIES: &[(&str, bool)] = &[
    ("SELECT ?s ?n WHERE { ?s <http://ex/name> ?n }", false),
    ("SELECT ?s WHERE { ?s <http://ex/age> ?a FILTER(?a > 26) }", false),
    ("SELECT ?s WHERE { ?s <http://ex/score> ?x FILTER(?x < 3) }", false),
    ("SELECT ?s ?a WHERE { ?s <http://ex/age> ?a FILTER(?a = 41) }", false),
    ("SELECT ?n WHERE { ?s <http://ex/name> ?n FILTER(REGEX(?n, \"^[ab]\", \"i\")) }", false),
    ("SELECT ?n WHERE { ?s <http://ex/name> ?n FILTER(CONTAINS(?n, \"long\") || STRSTARTS(?n, \"Car\")) }", false),
    ("SELECT ?s ?a WHERE { ?s <http://ex/name> ?n OPTIONAL { ?s <http://ex/age> ?a } }", false),
    ("SELECT ?x WHERE { { ?x <http://ex/age> 30 } UNION { ?x <http://ex/age> 25 } }", false),
    ("SELECT ?s WHERE { ?s <http://ex/name> ?n MINUS { ?s <http://ex/knows> ?o } }", false),
    ("SELECT ?s WHERE { ?s <http://ex/name> ?n FILTER NOT EXISTS { ?s <http://ex/age> ?a } }", false),
    ("SELECT ?s ?v WHERE { VALUES ?v { 25 30 } ?s <http://ex/age> ?v }", false),
    ("SELECT ?s ?b WHERE { ?s <http://ex/age> ?a BIND(?a * 2 + 1 AS ?b) }", false),
    ("SELECT ?s ?u WHERE { ?s <http://ex/name> ?n BIND(UCASE(STR(?n)) AS ?u) }", false),
    ("SELECT DISTINCT ?p WHERE { ?s ?p ?o }", false),
    ("SELECT ?n WHERE { ?s <http://ex/name> ?n } ORDER BY DESC(?n) LIMIT 2 OFFSET 1", true),
    ("SELECT ?s ?a WHERE { ?s <http://ex/age> ?a } ORDER BY ?a", true),
    (
        "SELECT (SUM(?a) AS ?sum) (AVG(?a) AS ?avg) (MIN(?a) AS ?min) (MAX(?a) AS ?max) (COUNT(?a) AS ?n) \
         WHERE { ?s <http://ex/age> ?a }",
        false,
    ),
    ("SELECT ?s (COUNT(?o) AS ?k) WHERE { ?s <http://ex/knows> ?o } GROUP BY ?s", false),
    ("SELECT ?s (GROUP_CONCAT(?n; SEPARATOR=\", \") AS ?all) WHERE { ?s <http://ex/name> ?n } GROUP BY ?s", false),
    ("SELECT (SAMPLE(?a) AS ?one) WHERE { ?s <http://ex/age> ?a FILTER(?a = 30) }", false),
    ("SELECT ?y WHERE { <http://ex/alice> <http://ex/knows>+ ?y }", false),
    ("SELECT ?y WHERE { <http://ex/alice> <http://ex/knows>* ?y }", false),
    ("SELECT ?o WHERE { GRAPH <http://ex/g1> { ?s ?p ?o } }", false),
    ("SELECT ?g ?o WHERE { GRAPH ?g { ?s <http://ex/likes> ?o } }", false),
    ("SELECT ?o FROM <http://ex/g2> WHERE { ?s ?p ?o }", false),
    ("SELECT ?s WHERE { ?s <http://ex/label> \"nick\" }", false),
    ("SELECT ?x WHERE { BIND(\"a long computed string value\" AS ?x) }", false),
    ("SELECT * WHERE { ?s <http://ex/nothing> ?o }", false),
];

#[test]
fn baseline_equals_native() {
    let c = loaded();
    c.query_row("SELECT sparql_load(?1, 'trig')", [TRIG], |_| Ok(())).unwrap();
    for (q, ordered) in QUERIES {
        let native = solutions(&c, q, "native", *ordered);
        let baseline = solutions(&c, q, "sql", *ordered);
        assert_eq!(native, baseline, "{q}");
        assert_eq!(
            one_p::<i64, _>(&c, "SELECT sparql_count(?1)", [q]),
            one_p::<i64, _>(&c, "SELECT sparql_count_sql(?1)", [q]),
            "{q}"
        );
    }
    // ASK through both engines.
    for q in ["ASK { ?s <http://ex/age> 30 }", "ASK { ?s <http://ex/age> 31 }"] {
        assert_eq!(
            one_p::<String, _>(&c, "SELECT sparql_query(?1)", [q]),
            one_p::<String, _>(&c, "SELECT sparql_query_sql(?1)", [q]),
        );
    }
    assert_eq!(status_i64(&c, "open_cursors"), 0);
}

#[test]
fn helper_results_flow_back_through_the_query_temp_dictionary() {
    let c = loaded();
    // A computed string (TEMP id interned by sparql_eval during stepping) decoded at output.
    let q = "SELECT ?l WHERE { ?s <http://ex/name> ?n BIND(CONCAT(?n, \" (computed long)\") AS ?l) } ORDER BY ?l";
    let csv: String = one_p(&c, "SELECT sparql_query_sql(?1, 'csv')", [q]);
    assert!(csv.starts_with("l\r\nAlice (computed long)\r\n"), "{csv}");
    // Aggregates over computed values; GROUP_CONCAT's result is a TEMP id as well.
    let q =
        "SELECT (GROUP_CONCAT(?n) AS ?all) (SUM(?a + 0.5) AS ?s) WHERE { ?x <http://ex/name> ?n ; <http://ex/age> ?a }";
    let v: serde_json::Value =
        serde_json::from_str(&one_p::<String, _>(&c, "SELECT sparql_query_sql(?1)", [q])).unwrap();
    let b = &v["results"]["bindings"][0];
    assert_eq!(b["s"]["value"], "97.5");
    let all = b["all"]["value"].as_str().unwrap();
    let mut names: Vec<&str> = all.split(' ').collect();
    names.sort();
    assert_eq!(names, ["Alice", "Bob", "Carol"]);
    // Helpers called directly while a baseline scan is open see no foreign context.
    let r: i64 = one(
        &c,
        "SELECT count(sparql_num(id1)) FROM sparql_rows('SELECT ?s ?a WHERE { ?s <http://ex/age> ?a }', '{\"engine\":\"sql\"}')",
    );
    assert_eq!(r, 3);
}

#[test]
fn baseline_respects_the_dataset_options() {
    let c = loaded();
    c.query_row("SELECT sparql_load(?1, 'trig')", [TRIG], |_| Ok(())).unwrap();
    let q = "SELECT ?o WHERE { ?s <http://ex/likes> ?o }";
    for engine in ["native", "sql"] {
        let opts = format!("{{\"engine\":\"{engine}\",\"union_default_graph\":1}}");
        let n: i64 = one_p(&c, "SELECT count(*) FROM sparql_rows(?1, ?2)", [q, &opts]);
        assert_eq!(n, 2, "{engine}");
    }
}
