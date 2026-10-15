//! Transactions, savepoint scopes and cache validity across rollbacks and connections (§4.4,
//! §8.6).
mod common;

use common::*;
use rusqlite::types::Value;
use rusqlite::{Connection, ErrorCode};

fn count(c: &Connection) -> i64 {
    one(
        c,
        "SELECT sparql_count('SELECT * WHERE { GRAPH ?g { ?s ?p ?o } } ') + sparql_count('SELECT * WHERE { ?s ?p ?o }')",
    )
}

#[test]
fn rollback_of_a_user_transaction_undoes_loads_and_leaves_no_stale_cache() {
    let c = conn();
    c.query_row("SELECT sparql_init()", [], |_| Ok(())).unwrap();
    c.execute_batch("BEGIN").unwrap();
    assert_eq!(one::<i64>(&c, "SELECT sparql_load('<http://ex/s> <http://ex/p> \"rolled back literal\" .')"), 1);
    // Decoded inside the transaction (the row is not committed: per-execution caches only).
    assert_eq!(one::<String>(&c, "SELECT sparql_value('SELECT ?o WHERE { ?s ?p ?o }')"), "rolled back literal");
    c.execute_batch("ROLLBACK").unwrap();
    assert_eq!(count(&c), 0);
    // The next load reuses the same dictionary row numbers for different terms.
    assert_eq!(one::<i64>(&c, "SELECT sparql_load('<http://ex/t> <http://ex/q> \"committed literal!!\" .')"), 1);
    assert_eq!(one::<String>(&c, "SELECT sparql_value('SELECT ?o WHERE { ?s ?p ?o }')"), "committed literal!!");
    assert_eq!(one::<String>(&c, "SELECT sparql_value('SELECT ?s WHERE { ?s ?p ?o }')"), "http://ex/t");
    assert!(one::<String>(&c, "SELECT sparql_check()").starts_with("ok"));
}

#[test]
fn commit_of_a_user_transaction_keeps_every_write() {
    let c = conn();
    c.execute_batch("BEGIN").unwrap();
    c.query_row("SELECT sparql_load(?1)", [DATA], |_| Ok(())).unwrap();
    c.query_row("SELECT sparql_update('INSERT DATA { <http://ex/x> <http://ex/p> 1 }')", [], |_| Ok(())).unwrap();
    assert!(!c.is_autocommit());
    c.execute_batch("COMMIT").unwrap();
    assert_eq!(count(&c), 14);
}

#[test]
fn a_failing_update_request_is_undone_as_a_whole() {
    let c = loaded();
    // Three operations; the last fails (the graph exists): the inserts are rolled back.
    let req =
        "INSERT DATA { <http://ex/new> <http://ex/p> 1 } ; CREATE GRAPH <http://ex/g> ; CREATE GRAPH <http://ex/g>";
    let e = err(&c, "SELECT sparql_update(?1)", [req]);
    assert!(e.starts_with("sparql:"), "{e}");
    assert_eq!(count(&c), 13);
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM rdf_graph"), 0);
    assert!(c.is_autocommit(), "no transaction is left open");
    // Inside a user transaction only the failing request is undone.
    c.execute_batch("BEGIN").unwrap();
    c.query_row("SELECT sparql_update('INSERT DATA { <http://ex/kept> <http://ex/p> 1 }')", [], |_| Ok(())).unwrap();
    let _ = err(&c, "SELECT sparql_update(?1)", [req]);
    assert!(!c.is_autocommit(), "the user's transaction goes on");
    c.execute_batch("COMMIT").unwrap();
    assert_eq!(count(&c), 14);
    assert_eq!(one::<i64>(&c, "SELECT sparql_ask('ASK { <http://ex/new> ?p ?o }')"), 0);
}

#[test]
fn scopes_inside_writing_host_statements() {
    let c = loaded();
    c.execute_batch(
        "CREATE TEMP TABLE r AS SELECT sparql_query('SELECT ?n WHERE { ?s <http://ex/name> ?n } ORDER BY ?n', 'csv') AS csv;
         CREATE TEMP TABLE names(v TEXT);
         INSERT INTO names SELECT v0 FROM sparql_rows('SELECT ?n WHERE { ?s <http://ex/name> ?n }');",
    )
    .unwrap();
    assert!(one::<String>(&c, "SELECT csv FROM r").starts_with("n\r\nAlice"));
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM names"), 4);
    // A load driven by a table: one call per row, each its own write scope.
    c.execute_batch("CREATE TEMP TABLE docs(d TEXT); INSERT INTO docs VALUES ('<http://ex/d1> <http://ex/p> 1 .'), ('<http://ex/d2> <http://ex/p> 2 .');").unwrap();
    assert_eq!(one::<i64>(&c, "SELECT sum(sparql_load(d)) FROM docs"), 2);
    assert_eq!(count(&c), 15);
}

#[test]
fn plain_sql_writes_to_rdf_quad_are_visible() {
    let c = loaded();
    c.execute(
        "INSERT INTO rdf_quad(g, s, p, o) SELECT 0, sparql_term_id('<http://ex/bob>'), sparql_term_id('<http://ex/knows>'), sparql_term_id('<http://ex/alice>')",
        [],
    )
    .unwrap();
    assert_eq!(one::<i64>(&c, "SELECT sparql_ask('ASK { <http://ex/bob> <http://ex/knows> <http://ex/alice> }')"), 1);
    c.execute("DELETE FROM rdf_quad WHERE s = sparql_term_id('<http://ex/alice>')", []).unwrap();
    assert_eq!(one::<i64>(&c, "SELECT sparql_count('SELECT * WHERE { <http://ex/alice> ?p ?o }')"), 0);
}

#[test]
fn two_connections_share_one_file_store() {
    let path = TempPath::new("two");
    let a = Connection::open(&path.0).unwrap();
    let b = Connection::open(&path.0).unwrap();
    sq_api::register(&a).unwrap();
    sq_api::register(&b).unwrap();
    a.query_row("SELECT sparql_load(?1)", [DATA], |_| Ok(())).unwrap();
    assert_eq!(count(&b), 13, "B sees A's committed load");
    assert_eq!(
        one::<String>(&b, "SELECT sparql_value('SELECT ?n WHERE { <http://ex/dave> <http://ex/name> ?n }')"),
        "Dave the very long name"
    );
    // B deletes Dave's name and vacuums the dictionary; A then reuses the row for a new term.
    b.query_row("SELECT sparql_update('DELETE WHERE { <http://ex/dave> <http://ex/name> ?n }')", [], |_| Ok(()))
        .unwrap();
    assert_eq!(one::<i64>(&b, "SELECT sparql_vacuum_terms()"), 1);
    a.query_row(
        "SELECT sparql_update('INSERT DATA { <http://ex/dave> <http://ex/name> \"A brand new long name\" }')",
        [],
        |_| Ok(()),
    )
    .unwrap();
    for c in [&a, &b] {
        assert_eq!(
            one::<String>(c, "SELECT sparql_value('SELECT ?n WHERE { <http://ex/dave> <http://ex/name> ?n }')"),
            "A brand new long name"
        );
        assert!(one::<String>(c, "SELECT sparql_check()").starts_with("ok"));
    }
}

#[test]
fn a_locked_database_reports_sqlite_busy() {
    let path = TempPath::new("busy");
    let a = Connection::open(&path.0).unwrap();
    let b = Connection::open(&path.0).unwrap();
    sq_api::register(&a).unwrap();
    sq_api::register(&b).unwrap();
    a.query_row("SELECT sparql_init()", [], |_| Ok(())).unwrap();
    a.execute_batch("BEGIN IMMEDIATE").unwrap();
    // The write scope takes the write lock first, from "no transaction": busy_timeout applies.
    b.busy_timeout(std::time::Duration::from_millis(300)).unwrap();
    let start = std::time::Instant::now();
    let e = b
        .query_row("SELECT sparql_update('INSERT DATA { <http://ex/a> <http://ex/b> 1 }')", [], |r| r.get::<_, i64>(0))
        .unwrap_err();
    // Scalar functions report SQLITE_ERROR; the message carries SQLite's code and text.
    assert_eq!(e.sqlite_error_code(), Some(ErrorCode::Unknown), "{e}");
    assert!(e.to_string().starts_with("sparql: SQLite error 5: database is locked"), "{e}");
    assert!(start.elapsed() >= std::time::Duration::from_millis(250), "the busy handler waited");
    assert!(b.is_autocommit());
    a.execute_batch("COMMIT").unwrap();
    assert_eq!(one::<i64>(&b, "SELECT sparql_update('INSERT DATA { <http://ex/a> <http://ex/b> 1 }')"), 1);
    assert_eq!(status(&b, "last_op"), Value::Text("update".into()));
}

#[test]
fn an_update_inside_a_writing_statement_commits_and_rolls_back_with_it() {
    let c = loaded();
    c.execute_batch(
        "CREATE TEMP TABLE r(n);
         CREATE TABLE src(u TEXT);
         INSERT INTO src VALUES ('INSERT DATA { <http://ex/x> <http://ex/p> \"first long literal\" }'),
                                ('INSERT DATA { <http://ex/x> <http://ex/p> 2 }');",
    )
    .unwrap();
    // SQLite refuses savepoints here; the host statement's transaction holds the updates.
    assert_eq!(c.execute("INSERT INTO r SELECT sparql_update(u) FROM src", []).unwrap(), 2);
    assert_eq!(count(&c), 15);
    assert!(c.is_autocommit());
    // A failure after another call's writes, and after this call's own partial writes, fails
    // the host statement: SQLite rolls its whole transaction back.
    c.execute_batch(
        "DELETE FROM src;
         INSERT INTO src VALUES ('INSERT DATA { <http://ex/y> <http://ex/p> \"rolled back literal\" }'),
           ('INSERT DATA { <http://ex/w> <http://ex/p> 1 } ; CREATE GRAPH <http://ex/g> ; CREATE GRAPH <http://ex/g>');",
    )
    .unwrap();
    let e = c.execute("INSERT INTO r SELECT sparql_update(u) FROM src", []).unwrap_err();
    assert!(!e.to_string().contains("own SELECT statement"), "{e}");
    assert_eq!(count(&c), 15);
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM r"), 2);
    assert_eq!(one::<i64>(&c, "SELECT sparql_ask('ASK { GRAPH <http://ex/g> {} }')"), 0);
    // Rolled-back dictionary rows are reused for other terms: nothing stale is cached.
    c.query_row("SELECT sparql_update('INSERT DATA { <http://ex/z> <http://ex/p> \"a committed literal\" }')", [], |_| {
        Ok(())
    })
    .unwrap();
    assert_eq!(
        one::<String>(&c, "SELECT sparql_value('SELECT ?o WHERE { <http://ex/z> <http://ex/p> ?o }')"),
        "a committed literal"
    );
    assert!(one::<String>(&c, "SELECT sparql_check()").starts_with("ok"));
}

#[test]
fn writers_that_need_their_own_savepoint_are_refused_inside_a_writing_statement() {
    let c = loaded();
    c.execute_batch("CREATE TEMP TABLE r(x)").unwrap();
    let refused = |c: &Connection, sql: &str| {
        let e = c.execute(sql, []).unwrap_err().to_string();
        assert!(e.contains("cannot run inside a statement that writes") && e.contains("own SELECT statement"), "{sql}: {e}");
    };
    // Inside a user transaction a failed host statement could keep partial changes.
    c.execute_batch("BEGIN").unwrap();
    refused(&c, "INSERT INTO r SELECT sparql_update('INSERT DATA { <http://ex/a> <http://ex/b> 1 }')");
    c.execute_batch("COMMIT").unwrap();
    // The storage layer opens its own write scope for these.
    for w in ["sparql_load('<http://ex/a> <http://ex/b> <http://ex/c> .')", "sparql_analyze()", "sparql_vacuum_terms()"] {
        refused(&c, &format!("INSERT INTO r SELECT {w}"));
    }
    // An absent schema needs DDL in a write scope first.
    let fresh = conn();
    fresh.execute_batch("CREATE TEMP TABLE r(x)").unwrap();
    refused(&fresh, "INSERT INTO r SELECT sparql_update('INSERT DATA { <http://ex/a> <http://ex/b> 1 }')");
    refused(&fresh, "INSERT INTO r SELECT sparql_init()");
    assert_eq!(count(&c), 13);
    assert!(c.is_autocommit());
}

#[test]
fn a_temp_view_scan_keeps_autocommit_and_one_snapshot() {
    let path = TempPath::new("tempview");
    let c = Connection::open(&path.0).unwrap();
    c.pragma_update(None, "journal_mode", "wal").unwrap();
    sq_api::register(&c).unwrap();
    let ttl: String = (0..3000).map(|i| format!("<http://ex/s{i}> <http://ex/v> {i} .\n")).collect();
    c.query_row("SELECT sparql_load(?1, 'turtle')", [&ttl], |_| Ok(())).unwrap();
    c.execute_batch(
        "CREATE VIRTUAL TABLE temp.vals USING sparql_view('SELECT ?s ?v WHERE { ?s <http://ex/v> ?v }');
         CREATE TABLE log(x);",
    )
    .unwrap();
    let other = Connection::open(&path.0).unwrap();
    sq_api::register(&other).unwrap();
    // A statement over a temp table holds no transaction on main: xFilter's read entry opens
    // a savepoint, which must be gone when the first row comes back.
    let mut st = c.prepare("SELECT v FROM vals").unwrap();
    let mut it = st.query([]).unwrap();
    let mut n = 0;
    while it.next().unwrap().is_some() {
        n += 1;
        if n == 1 {
            assert!(c.is_autocommit(), "no savepoint is held between rows");
            c.execute("INSERT INTO log VALUES (1)", []).unwrap();
            assert_eq!(one::<i64>(&other, "SELECT count(*) FROM log"), 1, "the autocommit write committed");
            c.execute_batch("BEGIN; COMMIT;").unwrap();
            // Another connection's commit is not seen by the running scan.
            other.query_row("SELECT sparql_load('<http://ex/new> <http://ex/v> 1 .')", [], |_| Ok(())).unwrap();
        }
    }
    assert_eq!(n, 3000, "the whole scan read one snapshot");
    drop(it);
    drop(st);
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM vals"), 3001);
    assert!(c.is_autocommit());
}
