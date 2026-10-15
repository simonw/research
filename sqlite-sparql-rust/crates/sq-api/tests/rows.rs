//! `sparql_rows` (§8.3a): columns, colUsed, query forms, options, streaming, correlation.
mod common;

use common::*;
use rusqlite::Connection;
use rusqlite::types::Value;

fn rows(c: &Connection, sql: &str) -> Vec<Vec<Value>> {
    let mut st = c.prepare(sql).unwrap_or_else(|e| panic!("{sql}: {e}"));
    let n = st.column_count();
    st.query_map([], |r| (0..n).map(|i| r.get::<_, Value>(i)).collect())
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn text(s: &str) -> Value {
    Value::Text(s.into())
}

#[test]
fn columns_of_a_select() {
    let c = loaded();
    let r = rows(
        &c,
        "SELECT n, v0, v1, t0, t1, id0, id1 IS NOT NULL, v2, t2, id2, vars, json
         FROM sparql_rows('SELECT ?s ?a WHERE { ?s <http://ex/age> ?a } ORDER BY ?a')",
    );
    assert_eq!(r.len(), 3);
    let bob = &r[0];
    assert_eq!(bob[0], Value::Integer(1));
    assert_eq!(bob[1], text("http://ex/bob"));
    assert_eq!(bob[2], Value::Integer(25));
    assert_eq!(bob[3], text("<http://ex/bob>"));
    assert_eq!(bob[4], text("\"25\"^^<http://www.w3.org/2001/XMLSchema#integer>"));
    let Value::Integer(bob_id) = bob[5] else { panic!("id0 {:?}", bob[5]) };
    assert_eq!(one_p::<String, _>(&c, "SELECT sparql_term(?1)", [bob_id]), "<http://ex/bob>");
    assert_eq!(bob[6], Value::Integer(1));
    assert_eq!((&bob[7], &bob[8], &bob[9]), (&Value::Null, &Value::Null, &Value::Null), "past the variables");
    assert_eq!(bob[10], text(r#"["s","a"]"#));
    assert_eq!(
        bob[11],
        text(
            r#"{"s":{"type":"uri","value":"http://ex/bob"},"a":{"type":"literal","value":"25","datatype":"http://www.w3.org/2001/XMLSchema#integer"}}"#
        )
    );
    assert_eq!(r.iter().map(|r| r[0].clone()).collect::<Vec<_>>(), [1, 2, 3].map(Value::Integer));
    // Unbound values are NULL in every column and absent from json.
    let r = rows(
        &c,
        "SELECT v1, t1, id1, json FROM sparql_rows('SELECT ?s ?x WHERE { ?s <http://ex/age> 25 OPTIONAL { ?s <http://ex/none> ?x } }')",
    );
    assert_eq!(
        r,
        vec![vec![Value::Null, Value::Null, Value::Null, text(r#"{"s":{"type":"uri","value":"http://ex/bob"}}"#)]]
    );
    // The hidden columns read back as the arguments.
    let r = rows(&c, "SELECT query, opts FROM sparql_rows('ASK {}', '{}')");
    assert_eq!(r, vec![vec![text("ASK {}"), text("{}")]]);
}

#[test]
fn count_star_decodes_nothing() {
    let c = loaded();
    let decoded = |c: &Connection| (status_i64(c, "terms_decoded"), status_i64(c, "decode_calls"));
    let before = decoded(&c);
    let n: i64 = one(&c, "SELECT count(*) FROM sparql_rows('SELECT ?s ?p ?o WHERE { ?s ?p ?o }')");
    assert_eq!(n, 13);
    assert_eq!(decoded(&c), before, "count(*) decodes nothing");
    // Ids only: still no decode.
    let ids: i64 = one(&c, "SELECT count(DISTINCT id0) FROM sparql_rows('SELECT ?s ?p ?o WHERE { ?s ?p ?o }')");
    assert_eq!(ids, 5);
    assert_eq!(decoded(&c), before, "id columns decode nothing");
    // Reading v1 decodes exactly that column (13 rows).
    let _: i64 = one(&c, "SELECT count(v1) FROM sparql_rows('SELECT ?s ?p ?o WHERE { ?s ?p ?o }')");
    assert_eq!(status_i64(&c, "terms_decoded") - before.0, 13);
    assert!(status_i64(&c, "rows_emitted") >= 39);
}

#[test]
fn ask_construct_and_many_variables() {
    let c = loaded();
    assert_eq!(
        rows(&c, "SELECT n, v0, id0 > 0, json FROM sparql_rows('ASK { ?s <http://ex/age> 30 }')"),
        vec![vec![Value::Integer(1), Value::Integer(1), Value::Integer(1), text(r#"{"boolean":true}"#)]]
    );
    assert_eq!(
        rows(&c, "SELECT v0, vars FROM sparql_rows('ASK { ?s <http://ex/age> 31 }')"),
        vec![vec![Value::Integer(0), text("[]")]]
    );
    let r = rows(
        &c,
        "SELECT v0, v1, t2, vars FROM sparql_rows('CONSTRUCT { ?s <http://ex/older> ?a } WHERE { ?s <http://ex/age> ?a FILTER(?a > 26) }') ORDER BY v0",
    );
    assert_eq!(r.len(), 2);
    assert_eq!(
        r[0],
        vec![
            text("http://ex/alice"),
            text("http://ex/older"),
            text("\"30\"^^<http://www.w3.org/2001/XMLSchema#integer>"),
            text(r#"["subject","predicate","object"]"#)
        ]
    );
    // 20 variables: v0..v15 hold the first 16, json holds all.
    let vars: Vec<String> = (0..20).map(|i| format!("?x{i}")).collect();
    let binds: String = (0..20).map(|i| format!("BIND({i} AS ?x{i}) ")).collect();
    let q = format!("SELECT {} WHERE {{ {binds} }}", vars.join(" "));
    let r = rows(&c, &format!("SELECT v15, json FROM sparql_rows('{q}')"));
    assert_eq!(r[0][0], Value::Integer(15));
    let Value::Text(json) = &r[0][1] else { panic!() };
    let v: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(v["x19"]["value"], "19");
}

#[test]
fn options_override_the_connection_config() {
    let c = loaded();
    let q = "SELECT ?n WHERE { ?s <http://ex/name> ?n ; <http://ex/age> ?a FILTER(?a > 26) } ORDER BY ?n";
    let native = rows(&c, &format!("SELECT v0 FROM sparql_rows('{q}')"));
    let sql = rows(&c, &format!("SELECT v0 FROM sparql_rows('{q}', '{{\"engine\":\"sql\"}}')"));
    assert_eq!(native, sql);
    assert_eq!(native, vec![vec![text("Alice")], vec![text("Carol")]]);
    let hashed = rows(&c, &format!("SELECT v0 FROM sparql_rows('{q}', '{{\"join_policy\":\"hash\",\"reorder\":0}}')"));
    assert_eq!(hashed, native);
    let e = c
        .prepare("SELECT * FROM sparql_rows('ASK {}', '{\"allow_files\":1}')")
        .unwrap()
        .query_row([], |_| Ok(()))
        .unwrap_err();
    assert!(e.to_string().contains("cannot be set per call"), "{e}");
    let e =
        c.prepare("SELECT * FROM sparql_rows('ASK {}', 'not json')").unwrap().query_row([], |_| Ok(())).unwrap_err();
    assert!(e.to_string().contains("opts is not valid JSON"), "{e}");
}

#[test]
fn union_default_graph_option() {
    let c = loaded();
    c.query_row("SELECT sparql_load(?1, 'trig')", [TRIG], |_| Ok(())).unwrap();
    let q = "SELECT ?o WHERE { ?s <http://ex/likes> ?o } ORDER BY ?o";
    assert!(rows(&c, &format!("SELECT v0 FROM sparql_rows('{q}')")).is_empty());
    let r = rows(&c, &format!("SELECT v0 FROM sparql_rows('{q}', '{{\"union_default_graph\":true}}')"));
    assert_eq!(r, vec![vec![text("http://ex/coffee")], vec![text("http://ex/tea")]]);
}

#[test]
fn a_scan_runs_in_the_outer_statements_read_transaction() {
    let c = loaded();
    let mut st = c.prepare("SELECT n FROM sparql_rows('SELECT ?s WHERE { ?s ?p ?o }')").unwrap();
    let mut it = st.query([]).unwrap();
    it.next().unwrap().unwrap();
    // xFilter found the statement's read transaction (txn_state READ) and opened no savepoint.
    assert!(c.is_autocommit());
    assert_eq!(status_i64(&c, "open_cursors"), 1);
}

#[test]
fn query_argument_is_required() {
    let c = loaded();
    let e = c.prepare("SELECT * FROM sparql_rows").unwrap_err();
    assert!(e.to_string().contains("no query solution"), "{e}");
    let e = c.prepare("SELECT * FROM sparql_rows(NULL)").unwrap().query_row([], |_| Ok(())).unwrap_err();
    assert!(e.to_string().contains("the query is NULL"), "{e}");
    let e = c.prepare("SELECT * FROM sparql_rows('SELECT')").unwrap().query_row([], |_| Ok(())).unwrap_err();
    assert!(e.to_string().starts_with("sparql: syntax error"), "{e}");
}

#[test]
fn correlated_queries_from_a_table() {
    let c = loaded();
    c.execute_batch(
        "CREATE TABLE jobs(id INTEGER PRIMARY KEY, q TEXT);
         INSERT INTO jobs(q) VALUES ('SELECT ?n WHERE { <http://ex/alice> <http://ex/name> ?n }'),
                                   ('SELECT ?x WHERE { <http://ex/alice> <http://ex/knows> ?x }'),
                                   ('ASK { <http://ex/bob> <http://ex/knows> <http://ex/alice> }');",
    )
    .unwrap();
    let r = rows(&c, "SELECT j.id, r.n, r.v0 FROM jobs j, sparql_rows(j.q) r ORDER BY j.id, r.v0");
    assert_eq!(
        r,
        vec![
            vec![Value::Integer(1), Value::Integer(1), text("Alice")],
            vec![Value::Integer(2), Value::Integer(1), text("http://ex/bob")],
            vec![Value::Integer(2), Value::Integer(2), text("http://ex/carol")],
            vec![Value::Integer(3), Value::Integer(1), Value::Integer(0)],
        ]
    );
    assert_eq!(status_i64(&c, "open_cursors"), 0);
}

#[test]
fn streams_a_large_result_in_batches() {
    let c = conn();
    let mut nt = String::new();
    for i in 0..10_000 {
        nt.push_str(&format!("<http://ex/s{i}> <http://ex/v> \"value number {i}\" .\n"));
    }
    assert_eq!(one_p::<i64, _>(&c, "SELECT sparql_load(?1, 'nt')", [&nt]), 10_000);
    let (n, max_n, distinct): (i64, i64, i64) = c
        .query_row(
            "SELECT count(*), max(n), count(DISTINCT t1) FROM sparql_rows('SELECT ?s ?v WHERE { ?s <http://ex/v> ?v }')",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!((n, max_n, distinct), (10_000, 10_000, 10_000));
    // Stop early: LIMIT closes the scan after the first batch.
    let mut st = c.prepare("SELECT v1 FROM sparql_rows('SELECT ?s ?v WHERE { ?s <http://ex/v> ?v }') LIMIT 3").unwrap();
    let got: Vec<String> = st.query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
    assert_eq!(got.len(), 3);
    assert!(got.iter().all(|v| v.starts_with("value number ")));
    drop(st);
    assert_eq!(status_i64(&c, "open_cursors"), 0);
}

#[test]
fn nested_scans_and_functions_inside_a_scan() {
    let c = loaded();
    // Our own functions run between xNext calls of an open scan (rule R1).
    let r = rows(
        &c,
        "SELECT sparql_term(r.id0), sparql_count('SELECT * WHERE { ?s <http://ex/knows> ?o }'),
                (SELECT count(*) FROM sparql_rows('SELECT ?o WHERE { ?s <http://ex/knows> ?o }') i WHERE i.id0 = r.id0)
         FROM sparql_rows('SELECT ?p WHERE { ?p <http://ex/age> ?a } ORDER BY ?p') r",
    );
    assert_eq!(
        r,
        vec![
            vec![text("<http://ex/alice>"), Value::Integer(3), Value::Integer(0)],
            vec![text("<http://ex/bob>"), Value::Integer(3), Value::Integer(1)],
            vec![text("<http://ex/carol>"), Value::Integer(3), Value::Integer(2)],
        ]
    );
}

#[test]
fn writes_are_refused_while_a_scan_is_open() {
    let c = loaded();
    let e = c
        .query_row(
            "SELECT sparql_update('INSERT DATA { <http://ex/x> <http://ex/p> 1 }') FROM sparql_rows('SELECT ?s WHERE { ?s ?p ?o }')",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_err();
    assert!(e.to_string().contains("cannot modify the RDF store while a sparql_rows cursor is open"), "{e}");
    // An open statement over sparql_rows blocks loads on the same connection…
    let mut st = c.prepare("SELECT v0 FROM sparql_rows('SELECT ?s WHERE { ?s ?p ?o }')").unwrap();
    let mut it = st.query([]).unwrap();
    it.next().unwrap().unwrap();
    assert_eq!(status_i64(&c, "open_cursors"), 1);
    let e = err(&c, "SELECT sparql_load('<http://ex/x> <http://ex/p> 1 .')", []);
    assert!(e.contains("sparql_rows cursor is open"), "{e}");
    assert!(err(&c, "SELECT sparql_init()", []).contains("cursor is open"));
    assert!(err(&c, "SELECT sparql_analyze()", []).contains("cursor is open"));
    // …reads are fine…
    assert_eq!(one::<i64>(&c, "SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }')"), 13);
    drop(it);
    drop(st);
    // …and once it is closed, writes go through.
    assert_eq!(status_i64(&c, "open_cursors"), 0);
    assert_eq!(one::<i64>(&c, "SELECT sparql_load('<http://ex/x> <http://ex/p> 1 .')"), 1);
}

#[test]
fn arguments_must_be_text() {
    let c = loaded();
    for (sql, msg) in [
        ("SELECT * FROM sparql_rows(42)", "sparql: invalid argument: sparql_rows: the query must be TEXT, got Integer"),
        ("SELECT * FROM sparql_rows(x'00')", "sparql: invalid argument: sparql_rows: the query must be TEXT, got Blob"),
        ("SELECT * FROM sparql_rows('ASK {}', 1.5)", "sparql: invalid argument: sparql_rows: the opts must be TEXT, got Real"),
        ("SELECT * FROM sparql_rows(CAST(x'ff' AS TEXT))", "sparql: invalid argument: sparql_rows: the query is not valid UTF-8"),
    ] {
        let e = c.prepare(sql).unwrap().query_row([], |_| Ok(())).unwrap_err().to_string();
        assert_eq!(e, msg, "{sql}");
    }
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM sparql_rows('SELECT * WHERE { ?s ?p ?o }', NULL)"), 13);
}

#[test]
fn ids_depend_on_the_term_not_on_how_it_was_computed() {
    let c = loaded();
    let dave: i64 = one(&c, "SELECT sparql_term_id('\"Dave the very long name\"')");
    let q = "SELECT ?x ?y WHERE { BIND(CONCAT(\"Dave the very \", \"long name\") AS ?x) \
             BIND(CONCAT(\"not in the store \", \"at all\") AS ?y) }";
    for engine in ["native", "sql"] {
        let opts = format!("{{\"engine\":\"{engine}\"}}");
        let r: (Value, Value, String) = c
            .query_row("SELECT id0, id1, t1 FROM sparql_rows(?1, ?2)", [q, &opts], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!(r, (Value::Integer(dave), Value::Null, "\"not in the store at all\"".into()), "{engine}");
    }
}
