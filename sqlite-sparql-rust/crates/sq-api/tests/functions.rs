//! Every scalar function of §8.2 through SQL on the system libsqlite3.
mod common;

use common::*;
use rusqlite::types::Value;
use rusqlite::{Connection, params};

const NAMES: &str = "SELECT ?n WHERE { ?s <http://ex/name> ?n } ORDER BY ?n";

#[test]
fn runs_on_the_host_sqlite_version() {
    // Native tests must exercise exactly the engine the Python host loads the extension into.
    assert_eq!(rusqlite::version_number(), 3_045_001);
    let c = conn();
    let v: String = one(&c, "SELECT sparql_version()");
    assert_eq!(v, format!("sparqlite {} (SQLite 3.45.1)", env!("CARGO_PKG_VERSION")));
}

#[test]
fn init_is_idempotent_and_queries_on_an_absent_schema_see_an_empty_store() {
    let c = conn();
    assert_eq!(one::<i64>(&c, "SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }')"), 0);
    assert_eq!(one::<i64>(&c, "SELECT sparql_ask('ASK { ?s ?p ?o }')"), 0);
    assert_eq!(one::<i64>(&c, "SELECT sparql_init()"), 1);
    assert_eq!(one::<i64>(&c, "SELECT sparql_init()"), 1);
    let tables: i64 = one(&c, "SELECT count(*) FROM sqlite_schema WHERE name LIKE 'rdf_%' AND type = 'table'");
    assert_eq!(tables, 8);
    assert!(one::<String>(&c, "SELECT sparql_check()").starts_with("ok"));
    assert_eq!(status(&c, "last_op"), Value::Text("init".into()));
}

#[test]
fn load_formats_base_and_target_graph() {
    let c = conn();
    let nt = "<http://ex/a> <http://ex/p> \"1\" .\n";
    assert_eq!(one_p::<i64, _>(&c, "SELECT sparql_load(?1, 'ntriples')", [nt]), 1);
    assert_eq!(one_p::<i64, _>(&c, "SELECT sparql_load(?1, 'nt')", [nt]), 0, "duplicates are not new");
    // BLOB data, relative IRIs against the base, into a named graph.
    let ttl = b"<a> <p> <b> .".to_vec();
    assert_eq!(one_p::<i64, _>(&c, "SELECT sparql_load(?1, 'ttl', 'http://base/', 'http://ex/g')", params![ttl]), 1);
    assert_eq!(
        one::<i64>(
            &c,
            "SELECT sparql_ask('ASK { GRAPH <http://ex/g> { <http://base/a> <http://base/p> <http://base/b> } }')"
        ),
        1
    );
    let nq = "<http://ex/s> <http://ex/p> <http://ex/o> <http://ex/g2> .\n";
    assert_eq!(one_p::<i64, _>(&c, "SELECT sparql_load(?1, 'nquads')", [nq]), 1);
    assert_eq!(one_p::<i64, _>(&c, "SELECT sparql_load(?1, 'trig')", [TRIG]), 2);
    let xml = r#"<?xml version="1.0"?>
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:ex="http://ex/">
  <rdf:Description rdf:about="http://ex/x"><ex:p>xml</ex:p></rdf:Description>
</rdf:RDF>"#;
    assert_eq!(one_p::<i64, _>(&c, "SELECT sparql_load(?1, 'rdfxml')", [xml]), 1);
    assert_eq!(one_p::<i64, _>(&c, "SELECT sparql_load(?1, 'n3')", ["<http://ex/n3> <http://ex/p> 3 ."]), 1);
    let jsonld = r#"{"@id": "http://ex/j", "http://ex/p": "json"}"#;
    assert_eq!(one_p::<i64, _>(&c, "SELECT sparql_load(?1, 'jsonld')", [jsonld]), 1);
    assert_eq!(one::<i64>(&c, "SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }')"), 4);
    assert_eq!(one::<i64>(&c, "SELECT sparql_count('SELECT * WHERE { GRAPH ?g { ?s ?p ?o } }')"), 4);

    assert!(err(&c, "SELECT sparql_load(?1, 'csv')", ["x"]).contains("unknown RDF format 'csv'"));
    assert!(err(&c, "SELECT sparql_load(NULL)", []).contains("data is NULL"));
    assert!(
        err(&c, "SELECT sparql_load(?1)", ["<http://ex/a> <http://ex/p> ."]).starts_with("sparql: RDF parse error")
    );
    assert!(err(&c, "SELECT sparql_load(42)", []).contains("TEXT or BLOB"));
}

#[test]
fn load_file_requires_allow_files() {
    let c = conn();
    let path = std::env::temp_dir().join(format!("sq-api-load-{}.ttl", std::process::id()));
    std::fs::write(&path, "<a> <http://ex/p> 1 .").unwrap();
    let p = path.to_str().unwrap();
    assert!(err(&c, "SELECT sparql_load_file(?1)", [p]).contains("allow_files"));
    assert_eq!(one::<String>(&c, "SELECT sparql_config('allow_files', 1)"), "1");
    assert_eq!(one_p::<i64, _>(&c, "SELECT sparql_load_file(?1)", [p]), 1);
    // The base IRI defaults to the file's URL.
    let s: String = one(&c, "SELECT sparql_value('SELECT ?s WHERE { ?s <http://ex/p> 1 }')");
    assert!(s.starts_with("file:///") && s.ends_with("/a"), "{s}");
    let other = path.with_extension("data");
    std::fs::write(&other, "<http://ex/x> <http://ex/p> \"2\" .").unwrap();
    assert!(err(&c, "SELECT sparql_load_file(?1)", [other.to_str().unwrap()]).contains("cannot guess the format"));
    assert_eq!(one_p::<i64, _>(&c, "SELECT sparql_load_file(?1, 'nt')", [other.to_str().unwrap()]), 1);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&other);
}

#[test]
fn query_formats() {
    let c = loaded();
    let json: String = one_p(&c, "SELECT sparql_query(?1)", [NAMES]);
    assert!(json.starts_with(r#"{"head":{"vars":["n"]}"#), "{json}");
    let csv: String = one_p(&c, "SELECT sparql_query(?1, 'csv')", [NAMES]);
    assert_eq!(csv, "n\r\nAlice\r\nBob\r\nDave the very long name\r\nCarol\r\n");
    let tsv: String = one_p(&c, "SELECT sparql_query(?1, 'TSV')", [NAMES]);
    assert!(tsv.starts_with("?n\n\"Alice\"\n"), "{tsv}");
    let xml: String = one_p(&c, "SELECT sparql_query(?1, 'xml')", [NAMES]);
    assert!(xml.contains("<literal>Alice</literal>"), "{xml}");
    let ask: String = one(&c, "SELECT sparql_query('ASK { ?s <http://ex/age> 30 }')");
    assert_eq!(ask, r#"{"head":{},"boolean":true}"#);
    let construct = "CONSTRUCT { ?s <http://ex/n> ?n } WHERE { ?s <http://ex/name> ?n FILTER(?n = \"Bob\") }";
    let nt: String = one_p(&c, "SELECT sparql_query(?1)", [construct]);
    assert_eq!(nt, "<http://ex/bob> <http://ex/n> \"Bob\" .\n");
    let ttl: String = one_p(&c, "SELECT sparql_query(?1, 'turtle')", [construct]);
    assert!(ttl.contains("<http://ex/bob> <http://ex/n> \"Bob\""), "{ttl}");
    let nq: String = one_p(&c, "SELECT sparql_query(?1, 'nquads')", [construct]);
    assert_eq!(nq, nt);
    let describe: String = one(&c, "SELECT sparql_query('DESCRIBE <http://ex/bob>')");
    assert_eq!(describe.lines().count(), 3);

    assert!(err(&c, "SELECT sparql_query(?1, 'ntriples')", [NAMES]).contains("is not a SELECT result format"));
    assert!(err(&c, "SELECT sparql_query(?1, 'json')", [construct]).contains("CONSTRUCT/DESCRIBE"));
    assert!(err(&c, "SELECT sparql_query(?1, 'yaml')", [NAMES]).contains("unknown result format"));
    let e = err(&c, "SELECT sparql_query('SELEC ?x')", []);
    assert!(e.starts_with("sparql: syntax error"), "{e}");
    assert!(err(&c, "SELECT sparql_query(NULL)", []).contains("query is NULL"));
}

#[test]
fn ask_count_value() {
    let c = loaded();
    assert_eq!(one::<i64>(&c, "SELECT sparql_ask('ASK { <http://ex/alice> <http://ex/knows> <http://ex/bob> }')"), 1);
    assert_eq!(one::<i64>(&c, "SELECT sparql_ask('SELECT * WHERE { ?s <http://ex/nope> ?o }')"), 0);
    assert_eq!(one::<i64>(&c, "SELECT sparql_ask('CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }')"), 1);
    assert_eq!(one::<i64>(&c, "SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }')"), 13);
    assert_eq!(
        one::<i64>(&c, "SELECT sparql_count('CONSTRUCT { ?s <http://ex/k> ?o } WHERE { ?s <http://ex/knows> ?o }')"),
        3
    );
    // Template triples with an unbound position are skipped.
    assert_eq!(one::<i64>(&c, "SELECT sparql_count('CONSTRUCT { ?s ?p ?o } WHERE { ?s <http://ex/knows> ?o }')"), 0);
    assert_eq!(one::<i64>(&c, "SELECT sparql_count('ASK {}')"), 1);

    // Native mapping (§8.4).
    let v = |q: &str| -> Value { one_p(&c, "SELECT sparql_value(?1)", [q]) };
    assert_eq!(v("SELECT ?a WHERE { <http://ex/alice> <http://ex/age> ?a }"), Value::Integer(30));
    assert_eq!(v("SELECT ?a WHERE { <http://ex/carol> <http://ex/score> ?a }"), Value::Real(2.5));
    assert_eq!(v("SELECT ?n WHERE { <http://ex/carol> <http://ex/name> ?n }"), Value::Text("Carol".into()));
    assert_eq!(v("SELECT ?s WHERE { ?s <http://ex/age> 25 }"), Value::Text("http://ex/bob".into()));
    assert_eq!(v("SELECT ?x WHERE { BIND(true AS ?x) }"), Value::Integer(1));
    assert_eq!(
        v("SELECT ?x WHERE { BIND(\"2020-01-02\"^^<http://www.w3.org/2001/XMLSchema#date> AS ?x) }"),
        Value::Text("2020-01-02".into())
    );
    assert_eq!(
        v("SELECT ?x WHERE { BIND(CONCAT(\"computed \", \"value!\") AS ?x) }"),
        Value::Text("computed value!".into())
    );
    assert_eq!(v("SELECT ?x WHERE { <http://ex/dave> <http://ex/nick> ?x }"), Value::Text("_:b1".into()));
    assert_eq!(v("SELECT ?x WHERE { ?s <http://ex/nope> ?x }"), Value::Null);
    assert_eq!(v("SELECT ?x ?y WHERE { OPTIONAL { ?x <http://ex/nope> ?y } }"), Value::Null);
    assert_eq!(v("ASK { ?s <http://ex/age> 30 }"), Value::Integer(1));
    assert_eq!(v("CONSTRUCT { <http://ex/s> <http://ex/p> 1 } WHERE {}"), Value::Text("http://ex/s".into()));
}

#[test]
fn update_reports_changes_and_auto_inits() {
    let c = conn();
    let n: i64 = one(
        &c,
        "SELECT sparql_update('INSERT DATA { <http://ex/a> <http://ex/p> 1, 2 . GRAPH <http://ex/g> { <http://ex/a> <http://ex/p> 3 } }')",
    );
    assert_eq!(n, 3);
    assert_eq!(status(&c, "last_inserted"), Value::Integer(3));
    let n: i64 = one(&c, "SELECT sparql_update('DELETE { ?s ?p 1 } INSERT { ?s ?p 10 } WHERE { ?s ?p 1 }')");
    assert_eq!(n, 2);
    assert_eq!((status_i64(&c, "last_inserted"), status_i64(&c, "last_deleted")), (1, 1));
    assert_eq!(one::<i64>(&c, "SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }')"), 2);
    assert_eq!(one::<i64>(&c, "SELECT sparql_update('DROP GRAPH <http://ex/g>')"), 1);
    assert!(err(&c, "SELECT sparql_update('DROP GRAPH <http://ex/g>')", []).starts_with("sparql:"));
    assert_eq!(one::<i64>(&c, "SELECT sparql_update('DROP SILENT GRAPH <http://ex/g>')"), 0);
    assert!(err(&c, "SELECT sparql_update('INSERT DATA {')", []).starts_with("sparql: syntax error"));
    // LOAD of local files follows allow_files.
    let path = std::env::temp_dir().join(format!("sq-api-update-{}.nt", std::process::id()));
    std::fs::write(&path, "<http://ex/f> <http://ex/p> \"file\" .\n").unwrap();
    let load = format!("LOAD <file://{}>", path.display());
    assert!(err(&c, "SELECT sparql_update(?1)", [&load]).starts_with("sparql:"));
    c.query_row("SELECT sparql_config('allow_files', 1)", [], |_| Ok(())).unwrap();
    assert_eq!(one_p::<i64, _>(&c, "SELECT sparql_update(?1)", [&load]), 1);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn explain_modes() {
    let c = loaded();
    let q = "SELECT ?n WHERE { ?s <http://ex/name> ?n ; <http://ex/age> ?a FILTER(?a > 20) }";
    let ex = |mode: &str| -> String { one_p(&c, "SELECT sparql_explain(?1, ?2)", params![q, mode]) };
    assert!(ex("algebra").contains("filter"), "{}", ex("algebra"));
    assert!(!ex("logical").is_empty());
    let plan: String = one_p(&c, "SELECT sparql_explain(?1)", [q]);
    assert!(plan.contains("#0") && plan.contains("est="), "{plan}");
    assert!(ex("analyze").contains("rows="), "{}", ex("analyze"));
    let sql = ex("sql");
    assert!(sql.contains("main.rdf_quad") && sql.contains("-- ?1 = "), "{sql}");
    assert!(sql.contains("<http://ex/name>"), "parameters are annotated: {sql}");
    assert!(ex("SQLPLAN").contains("USING COVERING INDEX"), "{}", ex("sqlplan"));
    assert!(err(&c, "SELECT sparql_explain(?1, 'tree')", [q]).contains("unknown explain mode"));
}

#[test]
fn analyze_vacuum_and_check() {
    let c = loaded();
    let json: String = one(&c, "SELECT sparql_analyze()");
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["analyzed_quads"], 13);
    assert_eq!(v["default_graph"]["triples"], 13);
    assert_eq!(v["named_graphs"]["triples"], 0);
    assert_eq!(status(&c, "last_op"), Value::Text("analyze".into()));
    // Drop every quad mentioning Dave's long name, then vacuum the dictionary.
    c.query_row("SELECT sparql_update('DELETE WHERE { ?s <http://ex/name> \"Dave the very long name\" }')", [], |_| {
        Ok(())
    })
    .unwrap();
    assert_eq!(one::<i64>(&c, "SELECT sparql_vacuum_terms()"), 1);
    assert_eq!(one::<i64>(&c, "SELECT sparql_vacuum_terms()"), 0);
    let check: String = one(&c, "SELECT sparql_check()");
    assert!(check.starts_with("ok:"), "{check}");
    assert_eq!(one::<i64>(&c, "SELECT sparql_count('SELECT * WHERE { ?s <http://ex/name> ?n }')"), 3);
}

#[test]
fn analyze_on_an_absent_store_is_a_schema_error() {
    let c = conn();
    assert!(err(&c, "SELECT sparql_analyze()", []).starts_with("sparql: RDF store schema missing"));
}

#[test]
fn config_get_set_and_validation() {
    let c = conn();
    for (k, v) in [
        ("engine", "native"),
        ("timeout_ms", "0"),
        ("max_memory_mb", "1024"),
        ("union_default_graph", "0"),
        ("decode_cache_mb", "64"),
        ("join_policy", "auto"),
        ("reorder", "1"),
        ("probe_counts", "32"),
        ("probe_cap", "1000"),
        ("allow_files", "0"),
        ("reader", "auto"),
        ("page_cache_mb", "128"),
    ] {
        assert_eq!(one_p::<String, _>(&c, "SELECT sparql_config(?1)", [k]), v, "{k}");
    }
    assert_eq!(one::<String>(&c, "SELECT sparql_config('engine', 'SQL')"), "sql");
    assert_eq!(one::<String>(&c, "SELECT sparql_config('timeout_ms', 250)"), "250");
    assert_eq!(one::<String>(&c, "SELECT sparql_config('reorder', 'false')"), "0");
    assert_eq!(one::<String>(&c, "SELECT sparql_config('join_policy', 'hash')"), "hash");
    assert_eq!(one::<String>(&c, "SELECT sparql_config('reader', 'sql')"), "sql");
    assert_eq!(one::<String>(&c, "SELECT sparql_config('page_cache_mb', 16)"), "16");
    assert_eq!(one::<String>(&c, "SELECT sparql_config('decode_cache_mb', 8)"), "8");
    assert_eq!(status(&c, "config.engine"), Value::Text("sql".into()));
    assert_eq!(status(&c, "config.decode_cache_mb"), Value::Integer(8));
    assert!(err(&c, "SELECT sparql_config('nope')", []).contains("unknown sparql_config key"));
    assert!(err(&c, "SELECT sparql_config('engine', 'fast')", []).contains("expected native or sql"));
    assert!(err(&c, "SELECT sparql_config('reader', 'mmap')", []).starts_with("sparql:"));
    assert!(err(&c, "SELECT sparql_config('timeout_ms', NULL)", []).contains("NULL"));
}

#[test]
fn term_functions() {
    let c = loaded();
    let id = |nt: &str| -> Option<i64> { one_p(&c, "SELECT sparql_term_id(?1)", [nt]) };
    let alice = id("<http://ex/alice>").unwrap();
    let carol_name = id("\"Carol\"@en").unwrap();
    let thirty = id("30").unwrap_or_else(|| panic!("inline integer"));
    let long = id("\"Dave the very long name\"").unwrap();
    assert_eq!(id("<http://ex/absent>"), None);
    assert_eq!(id("\"absent literal, long enough\""), None);
    assert!(id("\"5\"^^<http://www.w3.org/2001/XMLSchema#integer>").is_some(), "inline terms need no store");
    let e = err(&c, "SELECT sparql_term_id('not a term')", []);
    assert!(e.contains("not an N-Triples term"), "{e}");
    // A prefixed name is the usual mistake; the message says so instead of the parser's
    // "Double serialization with an invalid exponent".
    let e = err(&c, "SELECT sparql_term_id('ex:Python')", []);
    assert!(e.starts_with("sparql: invalid argument: not an N-Triples term: \"ex:Python\""), "{e}");
    assert!(e.contains("is a prefixed name") && e.contains("<http://example.org/Python>"), "{e}");
    assert!(!e.contains("exponent"), "{e}");

    let q = |f: &str, id: i64| -> Value { one_p(&c, &format!("SELECT {f}(?1)"), [id]) };
    assert_eq!(q("sparql_term", alice), Value::Text("<http://ex/alice>".into()));
    assert_eq!(q("sparql_term", carol_name), Value::Text("\"Carol\"@en".into()));
    assert_eq!(q("sparql_term", thirty), Value::Text("\"30\"^^<http://www.w3.org/2001/XMLSchema#integer>".into()));
    assert_eq!(q("sparql_term_value", thirty), Value::Integer(30));
    assert_eq!(q("sparql_term_value", alice), Value::Text("http://ex/alice".into()));
    assert_eq!(q("sparql_term_kind", alice), Value::Text("iri".into()));
    assert_eq!(q("sparql_term_kind", long), Value::Text("literal".into()));
    assert_eq!(q("sparql_term_lex", carol_name), Value::Text("Carol".into()));
    assert_eq!(q("sparql_term_lang", carol_name), Value::Text("en".into()));
    assert_eq!(q("sparql_term_lang", long), Value::Null);
    assert_eq!(q("sparql_term_datatype", long), Value::Text("http://www.w3.org/2001/XMLSchema#string".into()));
    assert_eq!(
        q("sparql_term_datatype", carol_name),
        Value::Text("http://www.w3.org/1999/02/22-rdf-syntax-ns#langString".into())
    );
    assert_eq!(q("sparql_term_datatype", thirty), Value::Text("http://www.w3.org/2001/XMLSchema#integer".into()));
    assert_eq!(q("sparql_term_datatype", alice), Value::Null);
    // Blank nodes, unbound, TEMP and invalid ids.
    let bnode: i64 = one(&c, "SELECT o FROM rdf_quad WHERE p = sparql_term_id('<http://ex/nick>')");
    assert_eq!(q("sparql_term", bnode), Value::Text("_:b1".into()));
    assert_eq!(q("sparql_term_kind", bnode), Value::Text("bnode".into()));
    assert_eq!(q("sparql_term_lex", bnode), Value::Text("b1".into()));
    assert_eq!(id("_:b1"), Some(bnode));
    for nothing in [0, 14 /* TEMP */, 15 /* ERROR */, 12, 13] {
        assert_eq!(q("sparql_term", nothing), Value::Null, "{nothing}");
        assert_eq!(q("sparql_term_kind", nothing), Value::Null, "{nothing}");
    }
    assert_eq!(one::<Value>(&c, "SELECT sparql_term(NULL)"), Value::Null);
    assert!(err(&c, "SELECT sparql_term(?1)", [(999_999i64 << 4) | 1]).contains("is not a term id of this store"));
    assert!(err(&c, "SELECT sparql_term('x')", []).contains("must be an INTEGER"));
    // The dump the W3C harness uses.
    let quads: i64 =
        one(&c, "SELECT count(*) FROM rdf_quad WHERE sparql_term(s) IS NOT NULL AND sparql_term(o) IS NOT NULL");
    assert_eq!(quads, 13);
}

#[test]
fn direct_helper_calls() {
    let c = loaded();
    let id = |nt: &str| -> i64 { one_p::<Option<i64>, _>(&c, "SELECT sparql_term_id(?1)", [nt]).unwrap() };
    let q = |sql: &str, a: i64| -> Value { one_p(&c, sql, [a]) };
    let (alice, thirty, score, carol) = (id("<http://ex/alice>"), id("30"), id("2.5"), id("\"Carol\"@en"));
    let long = id("\"Dave the very long name\"");
    let typed41 = id("\"41\"^^<http://www.w3.org/2001/XMLSchema#integer>");
    assert_eq!(q("SELECT sparql_num(?1)", thirty), Value::Integer(30));
    assert_eq!(q("SELECT sparql_num(?1)", score), Value::Real(2.5));
    assert_eq!(q("SELECT sparql_num(?1)", typed41), Value::Integer(41));
    assert_eq!(q("SELECT sparql_num(?1)", alice), Value::Null);
    assert_eq!(q("SELECT sparql_str(?1)", carol), Value::Text("Carol".into()));
    assert_eq!(q("SELECT sparql_str(?1)", long), Value::Text("Dave the very long name".into()));
    assert_eq!(q("SELECT sparql_str(?1)", thirty), Value::Null);
    assert_eq!(q("SELECT sparql_str(?1)", alice), Value::Null);
    assert_eq!(q("SELECT sparql_ebv(?1)", thirty), Value::Integer(1));
    assert_eq!(q("SELECT sparql_ebv(?1)", long), Value::Integer(1));
    assert_eq!(q("SELECT sparql_ebv(?1)", alice), Value::Null);
    assert_eq!(one::<Value>(&c, "SELECT sparql_num(NULL)"), Value::Null);
    // Sort keys order like ORDER BY: numbers before strings, 2.5 < 30.
    let k = |a: i64| -> Vec<u8> { one_p(&c, "SELECT sparql_sortkey(?1)", [a]) };
    assert!(k(score) < k(thirty) && k(thirty) < k(long), "numeric then string order");
    assert_eq!(one_p::<i64, _>(&c, "SELECT sparql_regex(?1, ?2)", [long, id("\"^Dave\"")]), 1);
    assert_eq!(one_p::<i64, _>(&c, "SELECT sparql_regex(?1, ?2, ?3)", [long, id("\"^dave\""), id("\"i\"")]), 1);
    assert_eq!(one_p::<Value, _>(&c, "SELECT sparql_regex(?1, ?2)", [alice, id("\"x\"")]), Value::Null);
    // sparql_eval: computed inline results come back as ids; TEMP results are NULL here.
    let sum: i64 = one_p(&c, "SELECT sparql_eval('+', ?1, ?2)", [thirty, typed41]);
    assert_eq!(
        one_p::<String, _>(&c, "SELECT sparql_term(?1)", [sum]),
        "\"71\"^^<http://www.w3.org/2001/XMLSchema#integer>"
    );
    assert_eq!(one_p::<Value, _>(&c, "SELECT sparql_eval('CONCAT', ?1, ?1)", [long]), Value::Null);
    let same: i64 = one_p(&c, "SELECT sparql_eval('STR', ?1)", [long]);
    assert_eq!(same, long, "a stored result is returned canonically");
    assert!(err(&c, "SELECT sparql_eval('NOPE', 1)", []).contains("unknown operator"));
    assert!(err(&c, "SELECT sparql_eval(1, 2)", []).contains("must be TEXT"));
    // Aggregates over a user table of ids.
    c.execute_batch("CREATE TEMP TABLE t(x INTEGER)").unwrap();
    for v in [thirty, typed41, score] {
        c.execute("INSERT INTO t VALUES (?1)", [v]).unwrap();
    }
    let sum: i64 = one(&c, "SELECT sparql_sum(x) FROM t");
    assert_eq!(one_p::<f64, _>(&c, "SELECT sparql_term_value(?1)", [sum]), 73.5);
    let max: i64 = one(&c, "SELECT sparql_max(x) FROM t");
    assert_eq!(max, typed41);
    let min: i64 = one(&c, "SELECT sparql_min(x) FROM t");
    assert_eq!(min, score);
    let empty: i64 = one(&c, "SELECT sparql_sum(x) FROM t WHERE 0");
    assert_eq!(one_p::<i64, _>(&c, "SELECT sparql_term_value(?1)", [empty]), 0);
    let gc: Value = one(&c, "SELECT sparql_group_concat(x) FROM t");
    assert_eq!(gc, Value::Null, "a computed long string is a TEMP id outside a baseline query");
    let sample: i64 = one(&c, "SELECT sparql_sample(x) FROM t");
    assert!([thirty, typed41, score].contains(&sample));
}

#[test]
fn baseline_functions() {
    let c = loaded();
    let q = "SELECT ?s ?n WHERE { ?s <http://ex/name> ?n ; <http://ex/age> ?a FILTER(?a >= 25) } ORDER BY ?n";
    let native: String = one_p(&c, "SELECT sparql_query(?1)", [q]);
    let baseline: String = one_p(&c, "SELECT sparql_query_sql(?1)", [q]);
    assert_eq!(native, baseline);
    assert_eq!(one_p::<i64, _>(&c, "SELECT sparql_count_sql(?1)", [q]), 3);
    let sql: String = one_p(&c, "SELECT sparql_to_sql(?1)", [q]);
    assert!(sql.contains("main.rdf_quad"), "{sql}");
    assert_eq!(one::<i64>(&c, "SELECT sparql_count_sql('ASK { ?s <http://ex/age> 30 }')"), 1);
    assert!(err(&c, "SELECT sparql_query_sql('CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }')", []).contains("baseline"));
    // engine=sql routes sparql_query through the baseline.
    c.query_row("SELECT sparql_config('engine', 'sql')", [], |_| Ok(())).unwrap();
    assert_eq!(one_p::<String, _>(&c, "SELECT sparql_query(?1)", [q]), native);
    assert_eq!(one_p::<i64, _>(&c, "SELECT sparql_count(?1)", [q]), 3);
    assert_eq!(one_p::<i64, _>(&c, "SELECT sparql_ask(?1)", [q]), 1);
    assert_eq!(one_p::<Value, _>(&c, "SELECT sparql_value(?1)", [q]), Value::Text("http://ex/alice".into()));
}

#[test]
fn directonly_functions_are_rejected_in_triggers_and_views() {
    let c = loaded();
    c.execute_batch(
        "CREATE TABLE log(q TEXT);
         CREATE TRIGGER log_update AFTER INSERT ON log BEGIN SELECT sparql_update(NEW.q); END;
         CREATE VIEW v_cfg AS SELECT sparql_config('engine', 'sql') AS x;
         CREATE VIEW v_q AS SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }') AS n;",
    )
    .unwrap();
    let e = c.execute("INSERT INTO log VALUES ('INSERT DATA { <http://ex/a> <http://ex/b> 1 }')", []).unwrap_err();
    assert!(e.to_string().contains("unsafe use of sparql_update()"), "{e}");
    let e = c.query_row("SELECT x FROM v_cfg", [], |r| r.get::<_, String>(0)).unwrap_err();
    assert!(e.to_string().contains("unsafe use of sparql_config()"), "{e}");
    // Readers are allowed in views.
    assert_eq!(one::<i64>(&c, "SELECT n FROM v_q"), 13);
    assert_eq!(one::<i64>(&c, "SELECT sparql_count('SELECT * WHERE { ?s <http://ex/b> ?o }')"), 0);
}

#[test]
fn errors_carry_the_sparql_prefix_and_result_codes() {
    let c = loaded();
    let e = c.query_row("SELECT sparql_query('SELECT * WHERE { SERVICE <http://x/> { ?s ?p ?o } }')", [], |r| {
        r.get::<_, String>(0)
    });
    let e = e.unwrap_err();
    assert!(e.to_string().starts_with("sparql: unsupported"), "{e}");
    assert_eq!(e.sqlite_error_code(), Some(rusqlite::ErrorCode::Unknown), "SQLITE_ERROR, not a constraint error");
}

#[test]
fn registering_twice_replaces_the_state_cleanly() {
    let c = Connection::open_in_memory().unwrap();
    sq_api::register(&c).unwrap();
    c.query_row("SELECT sparql_load(?1)", [DATA], |_| Ok(())).unwrap();
    sq_api::register(&c).unwrap();
    assert_eq!(one::<i64>(&c, "SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }')"), 13);
    assert_eq!(status_i64(&c, "armed"), 1);
}

#[test]
fn every_helper_of_the_baseline_contract_is_registered() {
    let c = loaded();
    for h in sq_sqlgen::HELPER_FUNCTIONS {
        for n in h.min_args..=h.max_args.min(4) {
            let args: Vec<&str> =
                (0..n).map(|i| if i == 0 && h.name == "sparql_eval" { "'STR'" } else { "NULL" }).collect();
            let sql = format!("SELECT {}({})", h.name, args.join(", "));
            if let Err(e) = c.query_row(&sql, [], |r| r.get::<_, Value>(0)) {
                // Only a semantic error (e.g. STR with no argument) is acceptable here.
                let e = e.to_string();
                assert!(
                    !e.contains("no such function") && !e.contains("wrong number of arguments to function"),
                    "{sql}: {e}"
                );
            }
        }
    }
}

#[test]
fn count_decodes_nothing() {
    let c = loaded();
    let before = status_i64(&c, "decode_calls");
    assert_eq!(one::<i64>(&c, "SELECT sparql_count('SELECT ?s ?o WHERE { ?s <http://ex/name> ?o }')"), 4);
    assert_eq!(
        one::<i64>(&c, "SELECT sparql_count('CONSTRUCT { ?s <http://ex/n> ?o } WHERE { ?s <http://ex/name> ?o }')"),
        4
    );
    assert_eq!(status_i64(&c, "decode_calls"), before);
}
