//! `sparql_view` (§8.3b): typed columns, hidden `__term` / `__id` columns and their pushdown.
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
        .unwrap_or_else(|e| panic!("{sql}: {e}"))
}

fn text(s: &str) -> Value {
    Value::Text(s.into())
}

fn people(c: &Connection) {
    c.execute_batch(
        "CREATE VIRTUAL TABLE temp.people USING sparql_view(
           'SELECT ?person ?name ?age WHERE { ?person <http://ex/name> ?name OPTIONAL { ?person <http://ex/age> ?age } }',
           engine=native, timeout_ms=2000)",
    )
    .unwrap();
}

#[test]
fn typed_columns_and_declared_schema() {
    let c = loaded();
    people(&c);
    let cols: Vec<String> = {
        let st = c.prepare("SELECT * FROM people").unwrap();
        st.column_names().iter().map(|s| s.to_string()).collect()
    };
    assert_eq!(cols, ["person", "name", "age"], "hidden columns stay out of SELECT *");
    let r = rows(&c, "SELECT name, age, typeof(age) FROM people ORDER BY name");
    assert_eq!(
        r,
        vec![
            vec![text("Alice"), Value::Integer(30), text("integer")],
            vec![text("Bob"), Value::Integer(25), text("integer")],
            vec![text("Carol"), Value::Integer(41), text("integer")],
            vec![text("Dave the very long name"), Value::Null, text("null")],
        ]
    );
    let r =
        rows(&c, "SELECT person__term, name__term, age__id IS NULL FROM people WHERE name = 'Dave the very long name'");
    assert_eq!(r, vec![vec![text("<http://ex/dave>"), text("\"Dave the very long name\""), Value::Integer(1)]]);
    // Plain SQL on top of the view.
    let r = rows(&c, "SELECT avg(age), count(*), count(age) FROM people");
    assert_eq!(r, vec![vec![Value::Real(32.0), Value::Integer(4), Value::Integer(3)]]);
}

#[test]
fn term_and_id_constraints_are_pushed_down() {
    let c = loaded();
    people(&c);
    let r = rows(&c, "SELECT name, age FROM people WHERE person__term = '<http://ex/alice>'");
    assert_eq!(r, vec![vec![text("Alice"), Value::Integer(30)]]);
    let bob: i64 = one(&c, "SELECT sparql_term_id('<http://ex/bob>')");
    let r = rows(&c, &format!("SELECT name FROM people WHERE person__id = {bob}"));
    assert_eq!(r, vec![vec![text("Bob")]]);
    // A literal constraint on a lang-tagged value column.
    let r = rows(&c, "SELECT person FROM people WHERE name__term = '\"Carol\"@en'");
    assert_eq!(r, vec![vec![text("http://ex/carol")]]);
    // SQL semantics: only the exact N-Triples text matches.
    assert!(rows(&c, "SELECT person FROM people WHERE name__term = '\"Carol\"@EN'").is_empty());
    assert!(rows(&c, "SELECT person FROM people WHERE name__term = 'Carol'").is_empty());
    // Unbound OPTIONAL values: NULL never equals anything.
    assert!(rows(&c, "SELECT person FROM people WHERE age__term = NULL").is_empty());
    let r = rows(&c, "SELECT name FROM people WHERE age__term = '\"30\"^^<http://www.w3.org/2001/XMLSchema#integer>'");
    assert_eq!(r, vec![vec![text("Alice")]]);
    // Terms absent from the store, invalid ids, wrong types: no rows (and no error).
    assert!(rows(&c, "SELECT * FROM people WHERE person__term = '<http://ex/nobody>'").is_empty());
    assert!(rows(&c, "SELECT * FROM people WHERE person__term = 'garbage'").is_empty());
    assert!(rows(&c, "SELECT * FROM people WHERE person__id = 0").is_empty());
    assert!(rows(&c, &format!("SELECT * FROM people WHERE person__id = {}", (99_999i64 << 4) | 1)).is_empty());
    assert!(rows(&c, "SELECT * FROM people WHERE person__id = 14").is_empty(), "TEMP ids never match");
    assert!(
        rows(&c, &format!("SELECT * FROM people WHERE person__id = '{bob}'")).is_empty(),
        "TEXT never equals an INTEGER id"
    );
    // Two constraints combine.
    let r = rows(&c, &format!("SELECT name FROM people WHERE person__id = {bob} AND name__term = '\"Bob\"'"));
    assert_eq!(r, vec![vec![text("Bob")]]);
    assert!(
        rows(&c, &format!("SELECT name FROM people WHERE person__id = {bob} AND name__term = '\"Alice\"'")).is_empty()
    );
    // A join drives the pushdown per outer row.
    c.execute_batch(
        "CREATE TEMP TABLE wanted(t TEXT); INSERT INTO wanted VALUES ('<http://ex/carol>'), ('<http://ex/bob>');",
    )
    .unwrap();
    let r = rows(&c, "SELECT p.name FROM wanted w JOIN people p ON p.person__term = w.t ORDER BY p.name");
    assert_eq!(r, vec![vec![text("Bob")], vec![text("Carol")]]);
}

#[test]
fn pushdown_respects_limits_and_computed_values() {
    let c = loaded();
    c.execute_batch(
        "CREATE VIRTUAL TABLE temp.first2 USING sparql_view('SELECT ?s WHERE { ?s <http://ex/age> ?a } ORDER BY ?a LIMIT 2');
         CREATE VIRTUAL TABLE temp.computed USING sparql_view('SELECT ?s ?label WHERE { ?s <http://ex/age> ?a BIND(CONCAT(\"age is \", STR(?a)) AS ?label) }');",
    )
    .unwrap();
    // The filter applies to the view's rows (after LIMIT), not inside the query.
    assert!(rows(&c, "SELECT s FROM first2 WHERE s__term = '<http://ex/carol>'").is_empty());
    assert_eq!(rows(&c, "SELECT s FROM first2 WHERE s__term = '<http://ex/bob>'"), vec![vec![text("http://ex/bob")]]);
    // A computed value absent from the store still matches its text.
    let r = rows(&c, "SELECT s, label__id FROM computed WHERE label__term = '\"age is 25\"'");
    assert_eq!(r, vec![vec![text("http://ex/bob"), Value::Null]], "computed terms have no id");
}

#[test]
fn view_options_and_errors() {
    let c = loaded();
    c.execute_batch("CREATE VIRTUAL TABLE temp.byeng USING sparql_view('SELECT ?n WHERE { ?s <http://ex/name> ?n } ORDER BY ?n', engine=sql)").unwrap();
    assert_eq!(rows(&c, "SELECT count(*) FROM byeng"), vec![vec![Value::Integer(4)]]);
    assert_eq!(rows(&c, "SELECT n FROM byeng WHERE n__term = '\"Bob\"'"), vec![vec![text("Bob")]]);
    // Per-scan options through the hidden opts column.
    c.query_row("SELECT sparql_load(?1, 'trig')", [TRIG], |_| Ok(())).unwrap();
    c.execute_batch("CREATE VIRTUAL TABLE temp.likes USING sparql_view('SELECT ?o WHERE { ?s <http://ex/likes> ?o }')")
        .unwrap();
    assert!(rows(&c, "SELECT o FROM likes").is_empty());
    assert_eq!(
        rows(&c, "SELECT count(*) FROM likes WHERE opts = '{\"union_default_graph\":1}'"),
        vec![vec![Value::Integer(2)]]
    );

    for (sql, msg) in [
        ("CREATE VIRTUAL TABLE temp.v1 USING sparql_view('ASK {}')", "needs a SELECT query"),
        ("CREATE VIRTUAL TABLE temp.v2 USING sparql_view('SELECT ?x WHERE {', engine=native)", "syntax error"),
        ("CREATE VIRTUAL TABLE temp.v3 USING sparql_view('SELECT ?x WHERE {}', allow_files=1)", "connection setting"),
        (
            "CREATE VIRTUAL TABLE temp.v4 USING sparql_view('SELECT ?x WHERE {}', engine=turbo)",
            "expected native or sql",
        ),
        ("CREATE VIRTUAL TABLE temp.v5 USING sparql_view('SELECT ?x ?x__term WHERE {}')", "used twice"),
        ("CREATE VIRTUAL TABLE temp.v6 USING sparql_view()", "needs the SPARQL query"),
    ] {
        let e = c.execute_batch(sql).unwrap_err().to_string();
        assert!(e.contains(msg), "{sql}: {e}");
    }
}

#[test]
fn persistent_views_need_the_extension_and_drop_cleanly() {
    let path = TempPath::new("view");
    {
        let c = Connection::open(&path.0).unwrap();
        sq_api::register(&c).unwrap();
        c.query_row("SELECT sparql_load(?1)", [DATA], |_| Ok(())).unwrap();
        c.execute_batch("CREATE VIRTUAL TABLE names USING sparql_view('SELECT ?n WHERE { ?s <http://ex/name> ?n }')")
            .unwrap();
        assert_eq!(one::<i64>(&c, "SELECT count(*) FROM names"), 4);
    }
    {
        // Without the extension only that table fails.
        let c = Connection::open(&path.0).unwrap();
        assert!(c.prepare("SELECT * FROM names").is_err());
        assert_eq!(one::<i64>(&c, "SELECT count(*) FROM rdf_quad"), 13);
    }
    let c = Connection::open(&path.0).unwrap();
    sq_api::register(&c).unwrap();
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM names"), 4);
    c.execute_batch("DROP TABLE names").unwrap();
    assert!(c.prepare("SELECT * FROM names").is_err());
}
