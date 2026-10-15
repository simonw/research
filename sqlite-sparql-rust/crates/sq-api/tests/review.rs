//! Adversarial review tests (WP11). Tests that pin down a defect fail until it is fixed; the
//! others are regression tests for behaviour the review verified.
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

// ------------------------------------------------------------------ defects

/// §8.6: "In xFilter the outer statement already holds a read transaction, so nothing is
/// opened." A statement over a `temp.` sparql_view (the form §8.3b recommends) only opens a
/// transaction on `temp`, so xFilter finds txn_state(main) == NONE and the scan's ReadScope
/// opens `SAVEPOINT sq_r0` — and keeps it until xClose, across every return to SQLite. While
/// the cursor is open the connection is no longer in autocommit mode: the user's own
/// "autocommit" writes are silently swallowed into that transaction (lost if the cursor is
/// still open when the connection closes) and BEGIN fails.
#[test]
fn review_temp_view_scan_keeps_autocommit_and_user_writes_commit() {
    let path = TempPath::new("review-tempview");
    let c = Connection::open(&path.0).unwrap();
    sq_api::register(&c).unwrap();
    c.query_row("SELECT sparql_load(?1)", [DATA], |_| Ok(())).unwrap();
    c.execute_batch(
        "CREATE VIRTUAL TABLE temp.people USING sparql_view('SELECT ?n WHERE { ?s <http://ex/name> ?n }');
         CREATE TABLE log(x);",
    )
    .unwrap();
    let mut st = c.prepare("SELECT n FROM people").unwrap();
    let mut it = st.query([]).unwrap();
    it.next().unwrap().unwrap();
    assert!(c.is_autocommit(), "a temp.sparql_view scan holds a SAVEPOINT (open transaction) between rows");
    c.execute("INSERT INTO log VALUES (1)", []).unwrap();
    let other = Connection::open(&path.0).unwrap();
    let seen: i64 = other.query_row("SELECT count(*) FROM log", [], |r| r.get(0)).unwrap();
    assert_eq!(seen, 1, "an autocommit INSERT made while the scan is open is not committed");
    c.execute_batch("BEGIN; COMMIT;").expect("BEGIN while a view scan is open");
}

/// Untrusted query text can crash the host process: spargebra's recursive-descent parser
/// (reached through `ConnState::parse_query` / `sq_plan::parse_query` / `parse_update`)
/// overflows an 8 MiB stack — the size of a typical host's main thread (Python's) — on
/// ~5 000 nested brackets, and a stack overflow is not a panic: catch_unwind cannot contain
/// it (Python dies with SIGSEGV). The child process below runs the requests; it must survive.
#[test]
fn review_deeply_nested_query_does_not_crash_the_host() {
    const CHILD: &str = "SQ_API_REVIEW_DEEP_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let t = std::thread::Builder::new()
            .stack_size(8 << 20)
            .spawn(|| {
                let c = loaded();
                let depth = 20_000;
                let q = format!("SELECT * WHERE {{ ?s ?p ?o FILTER({}?o{}) }}", "(".repeat(depth), ")".repeat(depth));
                let r = c.query_row("SELECT sparql_count(?1)", [&q], |r| r.get::<_, i64>(0));
                eprintln!("nested FILTER: {r:?}");
                let q = format!("SELECT * WHERE {}?s ?p ?o{}", "{ ".repeat(depth), " }".repeat(depth));
                let r = c.query_row("SELECT count(*) FROM sparql_rows(?1)", [&q], |r| r.get::<_, i64>(0));
                eprintln!("nested groups: {r:?}");
                let u = format!(
                    "INSERT DATA {{ <http://ex/a> <http://ex/p> {}1{} }}",
                    "( ".repeat(depth),
                    " )".repeat(depth)
                );
                let r = c.query_row("SELECT sparql_update(?1)", [&u], |r| r.get::<_, i64>(0));
                eprintln!("nested collection update: {r:?}");
            })
            .unwrap();
        t.join().unwrap();
        return;
    }
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "review_deeply_nested_query_does_not_crash_the_host", "--nocapture", "--test-threads=1"])
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "the process running sparql_count / sparql_rows / sparql_update on a deeply nested request died: {:?}\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr).lines().rev().take(6).collect::<Vec<_>>().join("\n")
    );
}

/// §8.3b: pushdown must keep SQL semantics. A computed term that equals a stored term is a
/// TEMP id in the native engine, so its `__id` column is NULL — yet `WHERE x__id = <stored
/// id>` (pushed down as sameTerm, which compares terms) returns that row, now showing the
/// stored id. The same row both "IS NULL" and "= <id>" depending on the plan; `+x__id`
/// (no pushdown) gives the SQL answer.
#[test]
fn review_view_id_pushdown_agrees_with_the_id_column() {
    let c = loaded();
    c.execute_batch(
        "CREATE VIRTUAL TABLE temp.cv USING sparql_view('SELECT ?x WHERE { BIND(CONCAT(\"Dave the very \", \"long name\") AS ?x) }')",
    )
    .unwrap();
    let dave: i64 = one(&c, "SELECT sparql_term_id('\"Dave the very long name\"')");
    let column = rows(&c, "SELECT x__id FROM cv");
    let pushed = rows(&c, &format!("SELECT x__id FROM cv WHERE x__id = {dave}"));
    let filtered_in_sql = rows(&c, &format!("SELECT x__id FROM cv WHERE +x__id = {dave}"));
    assert_eq!(
        pushed, filtered_in_sql,
        "pushed-down x__id = {dave} must select exactly the rows whose x__id column equals it (unconstrained column: {column:?})"
    );
}

/// The write scope's SAVEPOINT is refused while the host statement writes, so a writer used
/// as an ordinary expression of INSERT … SELECT fails — with a misleading "SQLite error 5"
/// (BUSY) message. DIRECTONLY only forbids views, triggers and schema, not top-level DML.
#[test]
fn review_writers_work_inside_a_writing_host_statement() {
    let c = loaded();
    c.execute_batch("CREATE TEMP TABLE r(n INTEGER)").unwrap();
    let r = c.execute("INSERT INTO r SELECT sparql_update('INSERT DATA { <http://ex/x> <http://ex/p> 1 }')", []);
    assert!(r.is_ok(), "INSERT INTO r SELECT sparql_update(...): {r:?}");
    assert_eq!(one::<i64>(&c, "SELECT n FROM r"), 1);
}

/// The view consumes (omit = true) every EQ constraint on its hidden `opts` column but
/// ignores non-TEXT values, so `opts = NULL` / `opts = 42` — never true in SQL — return every
/// row.
#[test]
fn review_view_opts_constraint_follows_sql_semantics() {
    let c = loaded();
    c.execute_batch("CREATE VIRTUAL TABLE temp.names USING sparql_view('SELECT ?n WHERE { ?s <http://ex/name> ?n }')")
        .unwrap();
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM names"), 4);
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM names WHERE opts = NULL"), 0, "NULL = NULL is never true");
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM names WHERE opts = 42"), 0, "the opts column is NULL here");
}

/// `__id` holds INTEGERs; SQL compares an INTEGER and an equal REAL as equal, but the
/// pushdown drops every non-INTEGER value as "matches nothing".
#[test]
fn review_view_id_constraint_compares_numerically() {
    let c = loaded();
    c.execute_batch(
        "CREATE VIRTUAL TABLE temp.people USING sparql_view('SELECT ?person ?name WHERE { ?person <http://ex/name> ?name }')",
    )
    .unwrap();
    let bob: i64 = one(&c, "SELECT sparql_term_id('<http://ex/bob>')");
    assert_eq!(one::<i64>(&c, &format!("SELECT count(*) FROM people WHERE +person__id = {bob}.0")), 1);
    assert_eq!(one::<i64>(&c, &format!("SELECT count(*) FROM people WHERE person__id = {bob}.0")), 1);
}

// ------------------------------------------------------------------ verified behaviour

/// Column subsets decoded per 1024-row batch (late materialization) agree with the `json`
/// column, across several batches and with unbound OPTIONAL values, on both engines.
#[test]
fn review_rows_multi_batch_column_subsets_agree_with_json() {
    let c = conn();
    let mut nt = String::new();
    for i in 0..3000 {
        nt.push_str(&format!("<http://ex/s{i}> <http://ex/v> \"value number {i}\" .\n"));
        if i % 3 == 0 {
            nt.push_str(&format!("<http://ex/s{i}> <http://ex/w> \"w{i} long enough\" .\n"));
        }
    }
    c.query_row("SELECT sparql_load(?1, 'nt')", [&nt], |_| Ok(())).unwrap();
    let q = "SELECT ?s ?v ?w WHERE { ?s <http://ex/v> ?v OPTIONAL { ?s <http://ex/w> ?w } } ORDER BY ?s";
    for engine in ["native", "sql"] {
        let opts = format!("{{\"engine\":\"{engine}\"}}");
        let only_w: Vec<Value> = c
            .prepare("SELECT v2 FROM sparql_rows(?1, ?2)")
            .unwrap()
            .query_map([q, &opts], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let with_json: Vec<(Value, String)> = c
            .prepare("SELECT v2, json FROM sparql_rows(?1, ?2)")
            .unwrap()
            .query_map([q, &opts], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(only_w.len(), 3000, "{engine}");
        assert_eq!(only_w.iter().filter(|v| **v != Value::Null).count(), 1000, "{engine}");
        for (i, (a, (b, json))) in only_w.iter().zip(&with_json).enumerate() {
            assert_eq!(a, b, "{engine} row {i}");
            let v: serde_json::Value = serde_json::from_str(json).unwrap();
            match a {
                Value::Text(t) => assert_eq!(v["w"]["value"], t.as_str(), "{engine} row {i}"),
                _ => assert!(v.get("w").is_none(), "{engine} row {i}: {json}"),
            }
        }
    }
}

/// Every writer and filesystem function (and the 2-argument sparql_config) is DIRECTONLY;
/// readers work in views; no function is DETERMINISTIC (index expressions reject them).
#[test]
fn review_writers_are_directonly_and_nothing_is_deterministic() {
    let c = loaded();
    for (i, w) in [
        "sparql_init()",
        "sparql_load('<http://ex/a> <http://ex/b> <http://ex/c> .')",
        "sparql_load_file('/nonexistent.ttl')",
        "sparql_update('INSERT DATA { <http://ex/a> <http://ex/b> 1 }')",
        "sparql_analyze()",
        "sparql_vacuum_terms()",
        "sparql_config('engine', 'sql')",
    ]
    .iter()
    .enumerate()
    {
        c.execute_batch(&format!("CREATE VIEW mw{i} AS SELECT {w} AS x")).unwrap();
        let e = c.query_row(&format!("SELECT x FROM mw{i}"), [], |r| r.get::<_, Value>(0)).unwrap_err();
        assert!(e.to_string().contains("unsafe use of"), "{w}: {e}");
    }
    for f in ["sparql_config('engine')", "sparql_count('ASK {}')", "sparql_term(0)", "sparql_version()"] {
        c.execute_batch(&format!("CREATE VIEW vr AS SELECT {f} AS x")).unwrap();
        c.query_row("SELECT x FROM vr", [], |r| r.get::<_, Value>(0)).unwrap_or_else(|e| panic!("{f}: {e}"));
        c.execute_batch("DROP VIEW vr").unwrap();
    }
    c.execute_batch("CREATE TABLE t(x INTEGER)").unwrap();
    for f in ["sparql_term(x)", "sparql_num(x)", "sparql_term_kind(x)", "sparql_version()", "sparql_sortkey(x)"] {
        let e = c.execute_batch(&format!("CREATE INDEX ti ON t({f})")).unwrap_err();
        assert!(e.to_string().contains("non-deterministic"), "{f}: {e}");
    }
}

/// The baseline's helper functions give the native engine's answers on mixed-type data
/// (numeric promotion, ill-typed literals, the `=` matrix, EBV, ORDER BY keys, aggregates).
#[test]
fn review_baseline_helpers_agree_with_native_on_mixed_types() {
    const MIXED: &str = r#"
@prefix ex: <http://ex/> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
ex:a ex:n 1 ; ex:s "abc" ; ex:l "chat"@fr ; ex:t "x"^^xsd:string ; ex:g ex:k1 .
ex:b ex:n 2.5 ; ex:s "a much longer string value" ; ex:l "chien"@fr ; ex:g ex:k1 .
ex:c ex:n "3"^^xsd:double ; ex:s "ABC" ; ex:l "dog"@en ; ex:g ex:k2 .
ex:d ex:n "4"^^xsd:float ; ex:s "zz" ; ex:g ex:k2 .
ex:e ex:n "x"^^xsd:integer ; ex:s "0012"^^xsd:integer ; ex:g ex:k2 .
ex:f ex:n "007"^^xsd:integer ; ex:s 12 ; ex:g ex:k3 .
ex:h ex:n true ; ex:s "2020-01-01"^^xsd:date .
ex:i ex:n "1.50"^^xsd:decimal ; ex:s _:bn .
ex:j ex:n "-0"^^xsd:integer ; ex:s ex:iri .
"#;
    let c = conn();
    c.query_row("SELECT sparql_load(?1)", [MIXED], |_| Ok(())).unwrap();
    let queries = [
        "SELECT ?x ?n WHERE { ?x <http://ex/n> ?n FILTER(?n > 1) }",
        "SELECT ?x ?n WHERE { ?x <http://ex/n> ?n FILTER(?n != 1) }",
        "SELECT ?x WHERE { ?x <http://ex/s> ?s FILTER(!?s) }",
        "SELECT ?x WHERE { ?x <http://ex/s> ?s FILTER(REGEX(?s, \"^a\", \"i\")) }",
        "SELECT ?x WHERE { ?x <http://ex/s> ?s FILTER(?s = 12) }",
        "SELECT ?x ?y WHERE { ?x <http://ex/n> ?n BIND(?n / 2 AS ?y) }",
        "SELECT ?x ?y WHERE { ?x <http://ex/n> ?n BIND(<http://www.w3.org/2001/XMLSchema#integer>(?n) AS ?y) }",
        "SELECT ?n WHERE { ?x <http://ex/n> ?n } ORDER BY ?n",
        "SELECT ?s WHERE { ?x <http://ex/s> ?s } ORDER BY DESC(?s)",
        "SELECT (SUM(?n) AS ?s) (AVG(?n) AS ?a) (MIN(?n) AS ?mi) (MAX(?n) AS ?ma) WHERE { ?x <http://ex/n> ?n }",
        "SELECT ?k (MIN(?s) AS ?mi) (MAX(?s) AS ?ma) WHERE { ?x <http://ex/g> ?k ; <http://ex/s> ?s } GROUP BY ?k",
        "SELECT ?k (GROUP_CONCAT(?l ; SEPARATOR = \"|\") AS ?c) WHERE { ?x <http://ex/g> ?k OPTIONAL { ?x <http://ex/l> ?l } } GROUP BY ?k",
        "SELECT (SUM(?s) AS ?bad) WHERE { ?x <http://ex/s> ?s }",
        "SELECT DISTINCT ?y WHERE { ?x <http://ex/n> ?n BIND(?n + 0 AS ?y) }",
    ];
    for q in queries {
        let get = |engine: &str| -> Vec<String> {
            let opts = format!("{{\"engine\":\"{engine}\"}}");
            let mut st = c.prepare("SELECT json FROM sparql_rows(?1, ?2)").unwrap();
            let mut v: Vec<String> = st
                .query_map([q, opts.as_str()], |r| r.get(0))
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap_or_else(|e| panic!("{engine}: {q}: {e}"));
            if !q.contains("ORDER BY") {
                v.sort();
            }
            v
        };
        assert_eq!(get("native"), get("sql"), "{q}");
    }
}

/// `sqlite3_close` (v1) disconnects the eponymous status table — the close hook disarms the
/// pool — before it fails with SQLITE_BUSY on a user statement: the connection stays usable,
/// the next entry re-arms, and the second close closes for real (WAL file removed).
#[test]
fn review_close_after_a_failed_close_rearms_and_closes_fully() {
    let path = TempPath::new("review-close");
    let c = Connection::open(&path.0).unwrap();
    c.pragma_update(None, "journal_mode", "wal").unwrap();
    sq_api::register(&c).unwrap();
    c.query_row("SELECT sparql_load(?1)", [DATA], |_| Ok(())).unwrap();
    let mut stmt = std::ptr::null_mut();
    // SAFETY: a plain prepare on the live handle; finalized below while the connection is open.
    let rc = unsafe {
        rusqlite::ffi::sqlite3_prepare_v2(
            c.handle(),
            c"SELECT v0 FROM sparql_rows('SELECT ?s WHERE { ?s ?p ?o }')".as_ptr(),
            -1,
            &mut stmt,
            std::ptr::null_mut(),
        )
    };
    assert_eq!(rc, rusqlite::ffi::SQLITE_OK);
    let c = match c.close() {
        Ok(()) => panic!("close must fail while a statement is unfinalized"),
        Err((c, _)) => c,
    };
    // SAFETY: the statement belongs to the still-open connection.
    unsafe { rusqlite::ffi::sqlite3_finalize(stmt) };
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM sparql_rows('SELECT * WHERE { ?s ?p ?o }')"), 13);
    assert_eq!(status_i64(&c, "armed"), 1);
    c.close().map_err(|(_, e)| e).unwrap();
    assert!(!path.sibling("-wal").exists(), "WAL file removed: no zombie connection");
}
