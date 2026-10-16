//! Function flags against SQLite's schema-trust rules: readers are `SQLITE_INNOCUOUS`, so
//! views and triggers stored in the database may call them even with
//! `PRAGMA trusted_schema = OFF` (the sqlite3 shell's default); writers and file access stay
//! `SQLITE_DIRECTONLY` (refused in stored views and triggers, allowed in TEMP ones). Readers
//! never write: they all work under `PRAGMA query_only`. Results longer than
//! `SQLITE_LIMIT_LENGTH` stop early.
mod common;

use common::*;
use rusqlite::Connection;
use rusqlite::types::Value;

const ALL: &str = "SELECT * WHERE { ?s ?p ?o }";

/// Every reader, as a SQL expression (`ids` is a table of term ids, `x` its column).
fn readers(c: &Connection) -> Vec<String> {
    let alice: i64 = one(c, "SELECT sparql_term_id('<http://ex/alice>')");
    let long: i64 = one(c, "SELECT sparql_term_id('\"Dave the very long name\"')");
    let q = "'SELECT ?n WHERE { ?s <http://ex/name> ?n } ORDER BY ?n'";
    [
        "sparql_version()".to_string(),
        "sparql_check()".into(),
        "sparql_config('engine')".into(),
        format!("sparql_query({q})"),
        format!("sparql_query({q}, 'csv')"),
        format!("sparql_ask({q})"),
        format!("sparql_count({q})"),
        format!("sparql_value({q})"),
        format!("sparql_explain({q})"),
        format!("sparql_explain({q}, 'algebra')"),
        format!("sparql_explain({q}, 'logical')"),
        format!("sparql_explain({q}, 'analyze')"),
        format!("sparql_explain({q}, 'sql')"),
        format!("sparql_explain({q}, 'sqlplan')"),
        format!("sparql_query_sql({q})"),
        format!("sparql_query_sql({q}, 'tsv')"),
        format!("sparql_count_sql({q})"),
        format!("sparql_to_sql({q})"),
        format!("sparql_term({alice})"),
        format!("sparql_term_value({long})"),
        "sparql_term_id('<http://ex/bob>')".into(),
        format!("sparql_term_kind({alice})"),
        format!("sparql_term_lex({long})"),
        format!("sparql_term_lang({long})"),
        format!("sparql_term_datatype({long})"),
        format!("sparql_num({long})"),
        format!("sparql_str({long})"),
        format!("sparql_ebv({long})"),
        format!("sparql_sortkey({long})"),
        format!("sparql_regex({long}, sparql_term_id('\"long\"'))"),
        format!("sparql_eval('STRLEN', {long})"),
        "(SELECT sparql_sum(x) FROM ids)".into(),
        "(SELECT sparql_avg(x) FROM ids)".into(),
        "(SELECT sparql_min(x) FROM ids)".into(),
        "(SELECT sparql_max(x) FROM ids)".into(),
        "(SELECT sparql_sample(x) FROM ids)".into(),
        "(SELECT sparql_group_concat(x) FROM ids)".into(),
        "(SELECT sparql_group_concat(x, sparql_term_id('\",\"')) FROM ids)".into(),
    ]
    .into()
}

const WRITERS: [&str; 7] = [
    "sparql_init()",
    "sparql_load('<http://ex/a> <http://ex/b> <http://ex/c> .', 'nt')",
    "sparql_load_file('/nonexistent.ttl')",
    "sparql_update('INSERT DATA { <http://ex/a> <http://ex/b> 1 }')",
    "sparql_analyze()",
    "sparql_vacuum_terms()",
    "sparql_config('engine', 'sql')",
];

fn with_ids() -> Connection {
    let c = loaded();
    c.execute_batch("CREATE TABLE ids(x INTEGER)").unwrap();
    c.execute(
        "INSERT INTO ids SELECT id0 FROM sparql_rows('SELECT ?a WHERE { ?s <http://ex/age> ?a }')",
        [],
    )
    .unwrap();
    c
}

fn untrusted(c: &Connection) {
    c.execute_batch("PRAGMA trusted_schema = OFF").unwrap();
    assert_eq!(one::<i64>(c, "PRAGMA trusted_schema"), 0);
}

#[test]
fn a_stored_view_calling_sparql_value_works_with_trusted_schema_off() {
    let c = loaded();
    c.execute_batch(
        "CREATE VIEW first_name AS
           SELECT sparql_value('SELECT ?n WHERE { ?s <http://ex/name> ?n } ORDER BY ?n') AS name",
    )
    .unwrap();
    untrusted(&c);
    assert_eq!(one::<String>(&c, "SELECT name FROM first_name"), "Alice");
}

#[test]
fn every_reader_works_in_stored_views_and_triggers_with_trusted_schema_off() {
    let c = with_ids();
    let exprs = readers(&c);
    // The reference values, from top-level SQL.
    let direct: Vec<Value> = exprs.iter().map(|e| one(&c, &format!("SELECT {e}"))).collect();
    for (i, e) in exprs.iter().enumerate() {
        c.execute_batch(&format!("CREATE VIEW main.r{i} AS SELECT {e} AS x")).unwrap();
    }
    c.execute_batch(
        "CREATE TABLE log(q TEXT, n INTEGER);
         CREATE TRIGGER main.count_on_insert AFTER INSERT ON log
           BEGIN UPDATE log SET n = sparql_count(NEW.q) WHERE rowid = NEW.rowid; END;",
    )
    .unwrap();
    untrusted(&c);
    for (i, e) in exprs.iter().enumerate() {
        let v: Value = c.query_row(&format!("SELECT x FROM r{i}"), [], |r| r.get(0)).unwrap_or_else(|e2| {
            panic!("{e} in a stored view with trusted_schema OFF: {e2}")
        });
        // EXPLAIN ANALYZE reports timings; every other reader gives what it gives directly.
        if !e.contains("'analyze'") {
            assert_eq!(v, direct[i], "{e}");
        }
    }
    c.execute("INSERT INTO log(q) VALUES (?1)", [ALL]).unwrap();
    assert_eq!(one::<i64>(&c, "SELECT n FROM log"), 13);
}

#[test]
fn writers_stay_directonly_but_temp_views_and_triggers_may_call_them() {
    let c = loaded();
    for trusted in [1, 0] {
        c.execute_batch(&format!("PRAGMA trusted_schema = {trusted}")).unwrap();
        for (i, w) in WRITERS.iter().enumerate() {
            c.execute_batch(&format!("CREATE VIEW main.w{i} AS SELECT {w} AS x")).unwrap();
            let e = c.query_row(&format!("SELECT x FROM w{i}"), [], |r| r.get::<_, Value>(0)).unwrap_err();
            let fname = &w[..w.find('(').unwrap()];
            assert!(e.to_string().contains(&format!("unsafe use of {fname}()")), "{w}: {e}");
            c.execute_batch(&format!("DROP VIEW w{i}")).unwrap();
        }
    }
    // SQLite exempts the TEMP schema: only this connection can create objects there.
    c.execute_batch(
        "CREATE TEMP VIEW tload AS
           SELECT sparql_load('<http://ex/t1> <http://ex/p> <http://ex/o> .', 'nt') AS n;
         CREATE TABLE inbox(q TEXT);
         CREATE TEMP TRIGGER apply AFTER INSERT ON inbox BEGIN SELECT sparql_update(NEW.q); END;",
    )
    .unwrap();
    assert_eq!(one::<i64>(&c, "SELECT n FROM tload"), 1);
    c.execute("INSERT INTO inbox VALUES ('INSERT DATA { <http://ex/t2> <http://ex/p> 1 }')", []).unwrap();
    assert_eq!(one::<i64>(&c, &format!("SELECT sparql_count('{ALL}')")), 15);
    // The same trigger stored in main is refused.
    c.execute_batch(
        "DROP TRIGGER apply;
         CREATE TRIGGER main.apply AFTER INSERT ON inbox BEGIN SELECT sparql_update(NEW.q); END;",
    )
    .unwrap();
    let e = c.execute("INSERT INTO inbox VALUES ('INSERT DATA { <http://ex/t3> <http://ex/p> 1 }')", []).unwrap_err();
    assert!(e.to_string().contains("unsafe use of sparql_update()"), "{e}");
}

#[test]
fn readers_never_write() {
    // `PRAGMA query_only` makes every attempt to change a database file fail.
    let path = TempPath::new("query-only");
    let c = Connection::open(&path.0).unwrap();
    sq_api::register(&c).unwrap();
    c.query_row("SELECT sparql_load(?1)", [DATA], |_| Ok(())).unwrap();
    c.execute_batch("CREATE TABLE ids(x INTEGER)").unwrap();
    c.execute("INSERT INTO ids SELECT id0 FROM sparql_rows('SELECT ?a WHERE { ?s <http://ex/age> ?a }')", [])
        .unwrap();
    let exprs = readers(&c);
    c.execute_batch("PRAGMA query_only = 1").unwrap();
    for e in &exprs {
        c.query_row(&format!("SELECT {e}"), [], |r| r.get::<_, Value>(0))
            .unwrap_or_else(|e2| panic!("{e} under query_only: {e2}"));
    }
    for q in ["SELECT count(*) FROM sparql_rows(?1)", "SELECT count(*) FROM sparql_rows(?1, '{\"engine\":\"sql\"}')"] {
        assert_eq!(one_p::<i64, _>(&c, q, [ALL]), 13);
    }
    // A writer does fail there.
    let e = err(&c, "SELECT sparql_update('INSERT DATA { <http://ex/a> <http://ex/b> 1 }')", []);
    assert!(e.contains("readonly"), "{e}");
}

#[test]
fn a_result_longer_than_sqlite_limit_length_stops_early() {
    let c = loaded();
    let json: String = one_p(&c, "SELECT sparql_query(?1)", [ALL]);
    // SAFETY: the connection is open; this only lowers one of its limits.
    unsafe { rusqlite::ffi::sqlite3_limit(c.handle(), rusqlite::ffi::SQLITE_LIMIT_LENGTH, 500) };
    assert!(json.len() > 500);
    for sql in ["SELECT sparql_query(?1)", "SELECT sparql_query_sql(?1)"] {
        let e = err(&c, sql, [ALL]);
        assert!(e.starts_with("sparql: SQLite error 18: string or blob too big"), "{sql}: {e}");
        assert!(e.contains("SQLITE_LIMIT_LENGTH (500 bytes)"), "{e}");
    }
    let e = err(&c, "SELECT sparql_query(?1)", ["CONSTRUCT WHERE { ?s ?p ?o }"]);
    assert!(e.contains("string or blob too big"), "{e}");
    // Results that fit are unchanged.
    let bob = "SELECT ?n WHERE { <http://ex/bob> <http://ex/name> ?n }";
    let small: String = one_p(&c, "SELECT sparql_query(?1, 'csv')", [bob]);
    assert_eq!(small, "n\r\nBob\r\n");
    unsafe { rusqlite::ffi::sqlite3_limit(c.handle(), rusqlite::ffi::SQLITE_LIMIT_LENGTH, json.len() as i32) };
    assert_eq!(one_p::<String, _>(&c, "SELECT sparql_query(?1)", [ALL]), json, "exactly the limit fits");
}

// ------------------------------------------------------------------ hostile database files

/// Replaces the store's `rdf_meta` table by a VIEW of the same rows whose `dict_gen` value
/// also evaluates `call` (a SQL expression), so that every API entry's epoch query reaches it.
fn hostile_meta(c: &Connection, call: &str) {
    c.execute_batch(&format!(
        "ALTER TABLE rdf_meta RENAME TO meta_real;
         CREATE VIEW main.rdf_meta(key, value) AS
           SELECT key, CASE WHEN key = 'dict_gen' THEN value + 0 * ({call}) ELSE value END FROM meta_real;"
    ))
    .unwrap();
}

/// A database whose `rdf_meta` is a view calling a reader made every reader recurse into
/// itself until the host's stack overflowed (with `trusted_schema = OFF` too, the readers being
/// innocuous). R8 stops the recursion at `MAX_ENTRY_DEPTH` with one clear message, on a 2 MiB
/// thread (Rust's default for spawned threads), for scalar readers and for `sparql_rows`. Runs
/// in a child process: a regression would abort the whole test binary.
#[test]
fn a_view_that_calls_sparqlite_from_sparqlites_own_sql_fails_cleanly_on_a_2mib_stack() {
    const TEST: &str = "a_view_that_calls_sparqlite_from_sparqlites_own_sql_fails_cleanly_on_a_2mib_stack";
    const CHILD: &str = "SQ_API_TRUST_RECURSION_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture", "--test-threads=1"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        let tail: Vec<&str> = stderr.lines().rev().take(12).collect();
        assert!(out.status.success(), "child process: {:?}\n{}", out.status, tail.join("\n"));
        return;
    }
    let shapes = [
        ("sparql_count('ASK {}')", "SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }')"),
        ("sparql_value('ASK {}')", "SELECT sparql_value('SELECT ?s WHERE { ?s ?p ?o }')"),
        ("sparql_term_id('<http://ex/alice>')", "SELECT sparql_term(sparql_term_id('<http://ex/bob>'))"),
        ("SELECT count(*) FROM sparql_rows('ASK {}')", "SELECT count(*) FROM sparql_rows('SELECT * WHERE { ?s ?p ?o }')"),
    ];
    for (call, sql) in shapes {
        let e = std::thread::Builder::new()
            .stack_size(2 << 20)
            .spawn(move || {
                let c = loaded();
                hostile_meta(&c, call);
                untrusted(&c);
                c.query_row(sql, [], |r| r.get::<_, Value>(0)).map_err(|e| e.to_string())
            })
            .unwrap()
            .join()
            .unwrap()
            .unwrap_err();
        assert!(e.starts_with("sparql: unsupported: sparqlite calls nest too deeply (8 calls are active"), "{call}: {e}");
    }
}

/// R8 also covers the steps of an open scan. `xNext` fetches the next 1024-row batch and
/// decodes its terms outside any `xFilter` entry; a database whose `rdf_term` is a view that
/// calls `sparql_rows` for one term found only in the second batch made every level of that
/// chain look like an outermost entry, so it recursed until the host's stack overflowed. Each
/// step now counts as an entry, and the chain stops at `MAX_ENTRY_DEPTH`. Child process: a
/// regression would abort the test binary.
#[test]
fn a_nested_call_reached_from_a_scans_second_batch_fails_cleanly() {
    const TEST: &str = "a_nested_call_reached_from_a_scans_second_batch_fails_cleanly";
    const CHILD: &str = "SQ_API_TRUST_STEP_RECURSION_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture", "--test-threads=1"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        let tail: Vec<&str> = stderr.lines().rev().take(12).collect();
        assert!(out.status.success(), "child process: {:?}\n{}", out.status, tail.join("\n"));
        return;
    }
    const Q: &str = "SELECT ?s WHERE { ?s ?p ?o } ORDER BY ?o";
    let e = std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(move || {
            let c = conn();
            let mut ttl = String::new();
            for i in 0..1100 {
                ttl.push_str(&format!("<http://ex/s{i}> <http://ex/k> {i} .\n"));
            }
            assert_eq!(one_p::<i64, _>(&c, "SELECT sparql_load(?1)", [&ttl]), 1100);
            // Without the hostile view, the scan runs past its first batch normally.
            assert_eq!(one_p::<i64, _>(&c, "SELECT count(v0) FROM sparql_rows(?1)", [Q]), 1100);
            c.execute_batch(&format!(
                "ALTER TABLE rdf_term RENAME TO term_real;
                 CREATE VIEW main.rdf_term(id, tag, lex, dt, lang, h) AS
                   SELECT id, tag,
                          CASE WHEN lex = 'http://ex/s1050'
                               THEN lex || substr((SELECT group_concat(v0) FROM sparql_rows('{Q}')), 1, 0)
                               ELSE lex END,
                          dt, lang, h
                   FROM term_real;"
            ))
            .unwrap();
            untrusted(&c);
            // The first batch (rows 1..=1024) does not reach the hostile row.
            let first: String = one_p(&c, "SELECT v0 FROM sparql_rows(?1) LIMIT 1", [Q]);
            assert_eq!(first, "http://ex/s0");
            c.query_row("SELECT count(v0) FROM sparql_rows(?1)", [Q], |r| r.get::<_, Value>(0))
                .map_err(|e| e.to_string())
        })
        .unwrap()
        .join()
        .unwrap()
        .unwrap_err();
    assert!(e.contains("sparqlite calls nest too deeply"), "{e}");
}

/// Dictionary rows are not re-validated when decoded, so a hostile file could plant an IRI or
/// language tag that N-Triples cannot represent and have `sparql_term`, the `__term` columns,
/// N-Triples / Turtle and TSV print it verbatim (SPARQL injection for an application that
/// splices `sparql_term()` into an update). Such terms are refused with one clear message in
/// every N-Triples-syntax output; JSON and XML escape them and keep working.
#[test]
fn terms_planted_in_the_dictionary_are_not_printed_unescaped() {
    let path = TempPath::new("planted-terms");
    {
        let c = Connection::open(&path.0).unwrap();
        sq_api::register(&c).unwrap();
        assert_eq!(one_p::<i64, _>(&c, "SELECT sparql_load(?1)", [DATA]), 13);
        c.execute_batch(
            "UPDATE rdf_term SET lex = 'http://ex/b> } ; DROP ALL ; INSERT DATA { <x' WHERE lex = 'http://ex/bob';
             UPDATE rdf_term SET lang = 'en\"><script>' WHERE lang IS NOT NULL;",
        )
        .unwrap();
    }
    let c = Connection::open(&path.0).unwrap();
    sq_api::register(&c).unwrap();
    let bob = "SELECT ?o WHERE { <http://ex/alice> <http://ex/knows> ?o } ORDER BY ?o LIMIT 1";
    let carol = "SELECT ?o WHERE { ?s <http://ex/name> ?o FILTER(lang(?o) != '') }";
    let refused = |sql: &str, q: &str| {
        let e = err(&c, sql, [q]);
        assert!(e.contains("not valid RDF"), "{sql} / {q}: {e}");
    };
    for q in [bob, carol] {
        refused("SELECT sparql_term(id0) FROM sparql_rows(?1)", q);
        refused("SELECT t0 FROM sparql_rows(?1)", q);
        refused("SELECT sparql_query(?1, 'tsv')", q);
        // JSON and XML escape the text.
        let json: String = one_p(&c, "SELECT sparql_query(?1, 'json')", [q]);
        assert!(serde_json::from_str::<serde_json::Value>(&json).is_ok(), "{json}");
        let _: String = one_p(&c, "SELECT sparql_query(?1, 'xml')", [q]);
    }
    for fmt in ["nt", "ttl", "nq"] {
        let e = err(&c, "SELECT sparql_query('CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }', ?1)", [fmt]);
        assert!(e.contains("not valid RDF"), "{fmt}: {e}");
    }
    // Untouched terms still print.
    assert_eq!(
        one::<String>(&c, "SELECT t0 FROM sparql_rows('SELECT ?o WHERE { <http://ex/alice> <http://ex/age> ?o }')"),
        "\"30\"^^<http://www.w3.org/2001/XMLSchema#integer>"
    );
}

/// The premise of `direct_guard`: SQLite (3.45.1) does not apply `SQLITE_DIRECTONLY` to CHECK
/// constraints stored in a database file. A file from someone else can name a writer in a
/// CHECK, which then runs inside every INSERT or UPDATE of that table and inside
/// `PRAGMA integrity_check` / `quick_check`, with `trusted_schema = OFF` too. sparqlite refuses
/// the call there, so the store, `allow_files`, `timeout_ms` and the engine stay as they were.
/// (The CHECK reads `msg` because SQLite skips, on UPDATE, a CHECK that reads no changed
/// column.)
#[test]
fn writers_do_not_run_from_a_check_constraint_stored_in_the_database() {
    let path = TempPath::new("trust-check");
    {
        let c = Connection::open(&path.0).unwrap();
        sq_api::register(&c).unwrap();
        c.query_row("SELECT sparql_load(?1)", [DATA], |_| Ok(())).unwrap();
        c.execute_batch(
            "CREATE TABLE inbox(msg TEXT CHECK (
                 length(msg) >= 0
                 AND sparql_config('engine', 'sql') IS NOT NULL
                 AND sparql_config('allow_files', 1) IS NOT NULL
                 AND sparql_update('DELETE WHERE { ?s ?p ?o }') >= 0));
             PRAGMA ignore_check_constraints = 1;
             INSERT INTO inbox VALUES ('planted');
             PRAGMA ignore_check_constraints = 0;",
        )
        .unwrap();
    }
    let c = Connection::open(&path.0).unwrap();
    sq_api::register(&c).unwrap();
    untrusted(&c);
    let state = |c: &Connection| {
        (
            one::<i64>(c, &format!("SELECT sparql_count('{ALL}')")),
            one::<String>(c, "SELECT sparql_config('engine')"),
            one::<String>(c, "SELECT sparql_config('allow_files')"),
        )
    };
    let before = state(&c);
    assert_eq!(before, (13, "native".to_string(), "0".to_string()));
    for sql in [
        "INSERT INTO inbox VALUES ('hello')",
        "UPDATE inbox SET msg = 'changed'",
        "INSERT INTO inbox SELECT 'x' FROM (SELECT 1) WHERE sparql_count('ASK {}') = 1",
        "PRAGMA integrity_check",
        "PRAGMA quick_check",
    ] {
        let r = c.prepare(sql).and_then(|mut st| st.query_map([], |r| r.get::<_, Value>(0))?.collect::<rusqlite::Result<Vec<_>>>());
        let e = r.expect_err(sql).to_string();
        assert!(e.contains("refused: the definition of table main.inbox names a sparqlite writer"), "{sql}: {e}");
        assert_eq!(state(&c), before, "{sql}");
    }
    // The application's own top-level calls still work...
    assert_eq!(one::<String>(&c, "SELECT sparql_config('timeout_ms', 500)"), "500");
    assert_eq!(one::<i64>(&c, "SELECT sparql_update('INSERT DATA { <http://ex/a> <http://ex/b> 1 }')"), 1);
    // ...but a writer inside a writing statement or a TEMP trigger cannot prove it is not the
    // CHECK constraint, so it is refused while that table exists.
    c.execute_batch(
        "CREATE TEMP TABLE r(n);
         CREATE TEMP TABLE q(u TEXT);
         CREATE TEMP TRIGGER apply AFTER INSERT ON q BEGIN SELECT sparql_update(NEW.u); END;",
    )
    .unwrap();
    let insert_select = "INSERT INTO r SELECT sparql_update('INSERT DATA { <http://ex/a> <http://ex/b> 2 }')";
    let via_trigger = "INSERT INTO q VALUES ('INSERT DATA { <http://ex/a> <http://ex/b> 3 }')";
    for sql in [insert_select, via_trigger] {
        let e = c.execute(sql, []).unwrap_err().to_string();
        assert!(e.contains("writers run only from a top-level SELECT that calls them by name"), "{sql}: {e}");
    }
    // Two statements running: the writer cannot tell which one called it.
    let mut pending = c.prepare("SELECT x FROM (SELECT 1 AS x UNION ALL SELECT 2)").unwrap();
    let mut rows = pending.query([]).unwrap();
    rows.next().unwrap();
    let e = err(&c, "SELECT sparql_update('INSERT DATA { <http://ex/a> <http://ex/b> 4 }')", []);
    assert!(e.contains("with no other statement of this connection running"), "{e}");
    drop(rows);
    drop(pending);
    assert_eq!(one::<i64>(&c, &format!("SELECT sparql_count('{ALL}')")), 14);
    // Without the table, everything works as before.
    c.execute_batch("DROP TABLE inbox").unwrap();
    assert_eq!(c.execute(insert_select, []).unwrap(), 1);
    assert_eq!(c.execute(via_trigger, []).unwrap(), 1);
    assert_eq!(one::<i64>(&c, &format!("SELECT sparql_count('{ALL}')")), 16);
    assert_eq!(one::<String>(&c, "SELECT sparql_config('timeout_ms')"), "500");
}

/// The same CHECK in a database ATTACHed to the application's connection: inserting into the
/// attached table used to wipe `main`'s RDF store.
#[test]
fn a_check_constraint_in_an_attached_database_cannot_write_mains_store() {
    let evil = TempPath::new("trust-attached");
    {
        let e = Connection::open(&evil.0).unwrap();
        sq_api::register(&e).unwrap();
        e.execute_batch("CREATE TABLE inbox(msg TEXT CHECK (sparql_update('DELETE WHERE { ?s ?p ?o }') >= 0))")
            .unwrap();
    }
    let c = loaded();
    untrusted(&c);
    c.execute("ATTACH ?1 AS evil", [evil.0.to_str().unwrap()]).unwrap();
    let e = c.execute("INSERT INTO evil.inbox VALUES ('hi')", []).unwrap_err().to_string();
    assert!(e.contains("table evil.inbox names a sparqlite writer"), "{e}");
    assert_eq!(one::<i64>(&c, &format!("SELECT sparql_count('{ALL}')")), 13);
    c.execute_batch("DETACH evil").unwrap();
    c.execute_batch("CREATE TEMP TABLE r(n)").unwrap();
    assert_eq!(c.execute("INSERT INTO r SELECT sparql_update('INSERT DATA { <http://ex/a> <http://ex/b> 1 }')", []).unwrap(), 1);
}

/// A writer reached from sparqlite's own SQL (here a TEMP trigger on `rdf_quad`, fired by an
/// update's INSERT) is refused: a nested write has no scope of its own to run in, and a
/// database file could plant the same call in a CHECK on `rdf_quad`.
#[test]
fn writers_never_run_nested_inside_another_sparqlite_call() {
    let c = loaded();
    c.execute_batch(
        "CREATE TEMP TRIGGER nested AFTER INSERT ON main.rdf_quad BEGIN SELECT sparql_config('engine', 'sql'); END;",
    )
    .unwrap();
    let e = err(&c, "SELECT sparql_update('INSERT DATA { <http://ex/n> <http://ex/p> 1 }')", []);
    assert!(e.contains("sparql_config cannot run inside another sparqlite call"), "{e}");
    assert_eq!(one::<String>(&c, "SELECT sparql_config('engine')"), "native");
    assert_eq!(one::<i64>(&c, &format!("SELECT sparql_count('{ALL}')")), 13, "the update rolled back");
    // Readers may nest.
    c.execute_batch(
        "DROP TRIGGER nested;
         CREATE TEMP TABLE seen(t);
         CREATE TEMP TRIGGER nested AFTER INSERT ON main.rdf_quad BEGIN INSERT INTO seen VALUES (sparql_term(NEW.o)); END;",
    )
    .unwrap();
    assert_eq!(one::<i64>(&c, "SELECT sparql_update('INSERT DATA { <http://ex/n> <http://ex/p> <http://ex/nested> }')"), 1);
    assert_eq!(one::<String>(&c, "SELECT t FROM seen"), "<http://ex/nested>");
}
