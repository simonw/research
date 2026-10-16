//! Adversarial review of lane api, round 2 (INNOCUOUS readers, interrupt messages,
//! `sparql_term_id` messages). Tests that pin down a defect fail until it is fixed; the
//! others are regression tests for behaviour the review verified.
mod common;

use common::*;
use rusqlite::Connection;
use rusqlite::types::Value;

// ------------------------------------------------------------------ helpers

/// Runs this test binary again with `child=<shape>` and reports whether the child process
/// survived (a stack overflow aborts the whole process, so it cannot be tested in-process).
fn run_child(test: &str, var: &str, shape: &str) -> (bool, String) {
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture", "--test-threads=1"])
        .env(var, shape)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let lines: Vec<&str> = stderr
        .lines()
        .chain(stdout.lines())
        .filter(|l| l.contains("overflow") || l.contains("fatal") || l.contains("RESULT"))
        .take(3)
        .collect();
    (out.status.success(), format!("{:?} {}", out.status, lines.join(" | ")))
}

/// Replaces the store's `rdf_meta` table by a VIEW of the same rows whose `dict_gen` value also
/// calls `call` (a SQL expression). Schema detection only checks names (`sq_sqlite::schema::
/// detect`), so the store stays "Ready", and every API entry's epoch query
/// (`SELECT value FROM main.rdf_meta WHERE key='dict_gen'`) evaluates `call`.
fn hostile_meta(c: &Connection, call: &str) {
    c.execute_batch(&format!(
        "ALTER TABLE rdf_meta RENAME TO meta_real;
         CREATE VIEW main.rdf_meta(key, value) AS
           SELECT key, CASE WHEN key = 'dict_gen' THEN value + 0 * ({call}) ELSE value END FROM meta_real;"
    ))
    .unwrap();
}

// ------------------------------------------------------------------ defects

/// SQLite's bar for `SQLITE_INNOCUOUS` includes "cannot be misused to cause harm". Every
/// newly innocuous reader runs SQL of its own against `main` (the §4.4 epoch query reads
/// `main.rdf_meta`; dictionary decodes read `main.rdf_term`, ...), and schema detection only
/// checks that objects with those *names* exist, not that they are tables. A database file
/// whose `rdf_meta` is a VIEW calling a reader therefore makes every reader call recurse into
/// itself through SQLite with no depth limit (`ConnState::depth` is tracked but never
/// bounded), until the stack overflows and the host process dies (SIGSEGV / abort; Python
/// and the sqlite3 shell die too). With `PRAGMA trusted_schema = OFF`, before this change
/// the scalar route was refused ("unsafe use of sparql_count()"); now it crashes. The
/// `sparql_rows` route (shape "vtab") was already open because that table was already
/// innocuous, so the fix is an entry-depth limit (refuse a reader nested more than a few
/// levels deep), not removing the flag.
///
/// Each child runs one shape on an 8 MiB thread and must survive (an error is fine).
#[test]
fn review_r2_hostile_schema_recursion_does_not_crash_the_host() {
    const CHILD: &str = "SQ_API_REVIEW_R2_RECURSION";
    const TEST: &str = "review_r2_hostile_schema_recursion_does_not_crash_the_host";
    if let Ok(shape) = std::env::var(CHILD) {
        let t = std::thread::Builder::new()
            .stack_size(8 << 20)
            .spawn(move || {
                let c = loaded();
                let (call, sql) = match shape.as_str() {
                    // New with INNOCUOUS: a scalar reader inside the hostile view.
                    "scalar" => ("sparql_count('ASK {}')", "SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }')"),
                    "value" => ("sparql_value('ASK {}')", "SELECT sparql_value('SELECT ?s WHERE { ?s ?p ?o }')"),
                    // Already possible before: the innocuous sparql_rows table in the view.
                    "vtab" => (
                        "SELECT count(*) FROM sparql_rows('ASK {}')",
                        "SELECT count(*) FROM sparql_rows('SELECT * WHERE { ?s ?p ?o }')",
                    ),
                    other => panic!("unknown shape {other}"),
                };
                hostile_meta(&c, call);
                c.execute_batch("PRAGMA trusted_schema = OFF").unwrap();
                let r = c.query_row(sql, [], |r| r.get::<_, Value>(0));
                println!("RESULT {shape}: {:?}", r.map_err(|e| e.to_string()));
            })
            .unwrap();
        t.join().unwrap();
        return;
    }
    let mut died = Vec::new();
    for shape in ["scalar", "value", "vtab"] {
        let (ok, what) = run_child(TEST, CHILD, shape);
        if !ok {
            died.push(format!("{shape}: {what}"));
        }
    }
    assert!(died.is_empty(), "the host process died with trusted_schema = OFF on:\n{}", died.join("\n"));
}

/// The docs this lane rewrote (SPEC §8.2, the crate README's Security section, the
/// `functions/mod.rs` module docs) say SQLite refuses `SQLITE_DIRECTONLY` functions in "views,
/// triggers and other schema objects (CHECK, DEFAULT, index expressions, generated columns)
/// stored in `main`", and the security argument rests on writers running "only from
/// top-level SQL and from TEMP views and triggers". SQLite 3.45.1 (the project's SQLite, also
/// Python's and the sqlite3 shell's here) does not apply DIRECTONLY to CHECK constraints: its
/// own DIRECTONLY `readfile()` works in a stored CHECK too. sparqlite then lets the writer run,
/// because a CHECK fires inside the host INSERT, and `sparql_update` is allowed inside a
/// writing statement in autocommit mode (the `sparql_config` setter has no such refusal at
/// all). So a database file from someone else, opened with `trusted_schema = OFF`, rewrites
/// the RDF store, turns `allow_files` on, turns the application's `timeout_ms` off, and can
/// `LOAD <file:...>` a local RDF file into itself, the moment the application inserts a row
/// into one of its tables (reproduced in the sqlite3 shell with both the old and the new
/// extension; the INNOCUOUS change did not cause it, the new wording hides it).
#[test]
fn review_r2_directonly_writers_do_not_run_from_a_stored_check_constraint() {
    let path = TempPath::new("review-r2-check");
    {
        let c = Connection::open(&path.0).unwrap();
        sq_api::register(&c).unwrap();
        c.query_row("SELECT sparql_load(?1)", [DATA], |_| Ok(())).unwrap();
        c.execute_batch(
            "CREATE TABLE inbox(msg TEXT CHECK (
                sparql_config('allow_files', 1) IS NOT NULL
                AND sparql_config('timeout_ms', 0) IS NOT NULL
                AND sparql_update('DELETE WHERE { ?s ?p ?o }') >= 0));",
        )
        .unwrap();
    }
    // The victim: a fresh connection to a file it did not write, schema untrusted.
    let c = Connection::open(&path.0).unwrap();
    sq_api::register(&c).unwrap();
    c.execute_batch("PRAGMA trusted_schema = OFF").unwrap();
    one::<String>(&c, "SELECT sparql_config('timeout_ms', 500)");
    let r = c.execute("INSERT INTO inbox VALUES ('hello')", []);
    let triples: i64 = one(&c, "SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }')");
    let allow: String = one(&c, "SELECT sparql_config('allow_files')");
    let timeout: String = one(&c, "SELECT sparql_config('timeout_ms')");
    assert!(
        r.is_err() && triples == 13 && allow == "0" && timeout == "500",
        "a stored CHECK constraint ran DIRECTONLY writers: insert {r:?}, triples left {triples}, \
         allow_files {allow}, timeout_ms {timeout}"
    );
}

// ------------------------------------------------------------------ probes (regressions)

/// A reader inside a trigger stored on `rdf_quad` (allowed with `trusted_schema = OFF` now
/// that the readers are innocuous) runs nested inside a writer's own INSERT. It must not leave
/// the dictionary caches holding a term of a request that then rolled back: the next request
/// reuses the row number for another term.
#[test]
fn review_r2_reader_in_an_rdf_quad_trigger_during_a_rolled_back_update() {
    let c = loaded();
    c.execute_batch(
        "CREATE TABLE audit(t TEXT);
         CREATE TRIGGER main.aud AFTER INSERT ON rdf_quad
           BEGIN INSERT INTO audit VALUES (sparql_term(NEW.o)); END;
         PRAGMA trusted_schema = OFF;",
    )
    .unwrap();
    let failing = "INSERT DATA { <http://ex/x> <http://ex/p> <http://ex/ROLLED_BACK_TERM> } ; \
                   CREATE GRAPH <http://ex/g9> ; CREATE GRAPH <http://ex/g9>";
    let r = c.query_row("SELECT sparql_update(?1)", [failing], |r| r.get::<_, i64>(0));
    assert!(r.is_err(), "{r:?}");
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM audit"), 0, "the trigger's row rolled back too");
    let n: i64 = one(&c, "SELECT sparql_update('INSERT DATA { <http://ex/y> <http://ex/p> <http://ex/KEPT_TERM> }')");
    assert_eq!(n, 1);
    let audit: Vec<String> = {
        let mut st = c.prepare("SELECT t FROM audit").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect()
    };
    assert_eq!(audit, ["<http://ex/KEPT_TERM>"]);
    let v: String = one(&c, "SELECT sparql_value('SELECT ?o WHERE { <http://ex/y> <http://ex/p> ?o }')");
    assert_eq!(v, "http://ex/KEPT_TERM");
    let v: Option<String> = one(&c, "SELECT sparql_value('SELECT ?o WHERE { <http://ex/x> <http://ex/p> ?o }')");
    assert_eq!(v, None);
    assert!(one::<String>(&c, "SELECT sparql_check()").starts_with("ok"));
    // And during a bulk load.
    let ttl: String = (0..3000).map(|i| format!("<http://ex/s{i}> <http://ex/p> \"value number {i}\" .\n")).collect();
    let n: i64 = one_p(&c, "SELECT sparql_load(?1, 'nt')", [&ttl]);
    assert_eq!(n, 3000);
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM audit"), 3001);
    assert_eq!(one::<String>(&c, "SELECT t FROM audit WHERE rowid = 3001"), "\"value number 2999\"");
    assert!(one::<String>(&c, "SELECT sparql_check()").starts_with("ok"));
}

/// Readers that mint blank nodes or other computed terms (BNODE(), CONSTRUCT templates with
/// blank nodes, UUID(), computed strings through `sparql_rows` id columns) still write
/// nothing: they all run under `PRAGMA query_only`, on both engines.
#[test]
fn review_r2_readers_that_mint_terms_never_write() {
    let path = TempPath::new("review-r2-mint");
    let c = Connection::open(&path.0).unwrap();
    sq_api::register(&c).unwrap();
    c.query_row("SELECT sparql_load(?1)", [DATA], |_| Ok(())).unwrap();
    let meta_before: String = one(&c, "SELECT group_concat(key || '=' || value, ',') FROM rdf_meta");
    let terms_before: i64 = one(&c, "SELECT count(*) FROM rdf_term");
    c.execute_batch("PRAGMA query_only = 1").unwrap();
    for q in [
        "SELECT (BNODE() AS ?b) WHERE { ?s ?p ?o }",
        "SELECT (BNODE(STR(?s)) AS ?b) (UUID() AS ?u) (STRUUID() AS ?v) WHERE { ?s ?p ?o }",
        "CONSTRUCT { _:x <http://ex/p> ?o . ?s <http://ex/q> [ <http://ex/r> ?o ] } WHERE { ?s ?p ?o }",
        "DESCRIBE <http://ex/dave>",
        "SELECT (CONCAT(STR(?o), \" fresh string that is not in the store\") AS ?c) WHERE { ?s ?p ?o }",
    ] {
        for f in ["sparql_query(?1)", "sparql_count(?1)", "sparql_value(?1)", "sparql_explain(?1, 'analyze')"] {
            c.query_row(&format!("SELECT {f}"), [q], |r| r.get::<_, Value>(0))
                .unwrap_or_else(|e| panic!("{f} of {q} under query_only: {e}"));
        }
        if !q.starts_with("CONSTRUCT") && !q.starts_with("DESCRIBE") && !q.contains("UUID") && !q.contains("BNODE") {
            c.query_row("SELECT sparql_query_sql(?1)", [q], |r| r.get::<_, Value>(0))
                .unwrap_or_else(|e| panic!("sparql_query_sql of {q} under query_only: {e}"));
        }
        c.query_row("SELECT count(id0), count(t0), count(json) FROM sparql_rows(?1)", [q], |r| r.get::<_, i64>(0))
            .unwrap_or_else(|e| panic!("sparql_rows of {q} under query_only: {e}"));
    }
    c.execute_batch("PRAGMA query_only = 0").unwrap();
    assert_eq!(one::<String>(&c, "SELECT group_concat(key || '=' || value, ',') FROM rdf_meta"), meta_before);
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM rdf_term"), terms_before);
}

/// Side effect of `vtab::column_error` (keeping `SQLITE_INTERRUPT` for an interrupt noticed in
/// `xColumn`): SQLite treats an interrupted *writing* statement inside an explicit transaction
/// as a "special error" (`sqlite3VdbeHalt`) and rolls the whole transaction back, as its
/// `sqlite3_interrupt` documentation says. Before the change the same interrupt, reported as
/// `SQLITE_ERROR`, undid only the statement. The new behaviour matches SQLite and what
/// `xFilter` / `xNext` already did; this pins it down.
#[test]
fn review_r2_interrupt_in_xcolumn_of_a_writing_statement_rolls_back_the_transaction() {
    let c = loaded();
    let h = c.get_interrupt_handle();
    c.create_scalar_function("interrupt_now", 0, rusqlite::functions::FunctionFlags::SQLITE_UTF8, move |_| {
        h.interrupt();
        Ok(1i64)
    })
    .unwrap();
    c.execute_batch("CREATE TABLE kept(x); CREATE TABLE sink(a, b); BEGIN; INSERT INTO kept VALUES (1);").unwrap();
    let computed = "SELECT ?c WHERE { ?s ?p ?o BIND(CONCAT(STR(?o), \" computed\") AS ?c) }";
    let e = c.execute("INSERT INTO sink SELECT interrupt_now(), id0 FROM sparql_rows(?1)", [computed]).unwrap_err();
    assert_eq!(e.sqlite_error_code(), Some(rusqlite::ErrorCode::OperationInterrupted), "{e}");
    assert!(c.is_autocommit(), "SQLite rolled the explicit transaction back");
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM kept"), 0);
}
