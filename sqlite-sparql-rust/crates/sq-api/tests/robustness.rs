//! Interrupts and timeouts (R6), panic safety (R4), the close hook (§8.7).
mod common;

use std::thread;
use std::time::{Duration, Instant};

use common::*;
use rusqlite::Connection;

/// A cross product nobody can finish (`FILTER` defeats every counting shortcut).
const ENDLESS: &str =
    "SELECT * WHERE { ?a ?p ?b . ?c ?q ?d . ?e ?r ?f . ?g ?s ?h FILTER(?b != ?d && ?d != ?f && ?f != ?h) }";

fn big() -> Connection {
    let c = conn();
    let mut nt = String::new();
    for i in 0..300 {
        nt.push_str(&format!("<http://ex/s{i}> <http://ex/p{}> <http://ex/o{}> .\n", i % 7, i % 13));
    }
    c.query_row("SELECT sparql_load(?1, 'nt')", [&nt], |_| Ok(())).unwrap();
    c
}

fn interrupt_after(c: &Connection, ms: u64) -> thread::JoinHandle<()> {
    let h = c.get_interrupt_handle();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(ms));
        h.interrupt();
    })
}

#[test]
fn interrupt_from_another_thread_stops_a_native_query() {
    let c = big();
    let t = interrupt_after(&c, 150);
    let start = Instant::now();
    let e = c.query_row("SELECT sparql_count(?1)", [ENDLESS], |r| r.get::<_, i64>(0)).unwrap_err();
    t.join().unwrap();
    assert!(e.to_string().contains("interrupted"), "{e}");
    assert!(start.elapsed() < Duration::from_secs(20));
    assert!(c.is_autocommit(), "no transaction left open");
    // The connection is fine afterwards.
    assert_eq!(one::<i64>(&c, "SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }')"), 300);
}

#[test]
fn interrupt_stops_a_streaming_scan_and_the_baseline() {
    let c = big();
    let t = interrupt_after(&c, 150);
    let e = c.query_row("SELECT count(*) FROM sparql_rows(?1)", [ENDLESS], |r| r.get::<_, i64>(0)).unwrap_err();
    t.join().unwrap();
    assert!(e.to_string().contains("interrupt"), "{e}");
    assert_eq!(status_i64(&c, "open_cursors"), 0);

    let t = interrupt_after(&c, 150);
    let e = c.query_row("SELECT sparql_count_sql(?1)", [ENDLESS], |r| r.get::<_, i64>(0)).unwrap_err();
    t.join().unwrap();
    assert!(e.to_string().contains("interrupt"), "{e}");
    assert_eq!(one::<i64>(&c, "SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }')"), 300);
}

/// Never yields a row: a scan of it spends all its time inside sparqlite's own fetch.
const NEVER: &str =
    "SELECT * WHERE { ?a ?p ?b . ?c ?q ?d . ?e ?r ?f FILTER(CONCAT(STR(?b), STR(?d), STR(?f)) = \"never\") }";

/// 90,000 rows, then a branch that never yields one: the interrupt lands in `xNext`.
const FLOW_THEN_NEVER: &str = "SELECT * WHERE { { ?a ?p ?b . ?c ?q ?d } UNION \
    { ?e ?r ?f . ?g ?s ?h . ?i ?t ?j FILTER(CONCAT(STR(?f), STR(?h), STR(?j)) = \"never\") } }";

/// Runs `sql`, interrupted from another thread after 300 ms; returns the error.
fn interrupted(c: &Connection, sql: &str, p: &[&str]) -> rusqlite::Error {
    let t = interrupt_after(c, 300);
    let e = c.query_row(sql, rusqlite::params_from_iter(p), |r| r.get::<_, i64>(0)).unwrap_err();
    t.join().unwrap();
    e
}

fn assert_sparql_interrupt(e: &rusqlite::Error, what: &str) {
    assert_eq!(e.sqlite_error_code(), Some(rusqlite::ErrorCode::OperationInterrupted), "{what}: {e}");
    assert_eq!(e.to_string(), "sparql: interrupted", "{what}");
}

/// A scan reports an interrupt that sparqlite notices (planning, fetching a batch, decoding a
/// column) like the scalar functions do, `sparql: interrupted`, and keeps `SQLITE_INTERRUPT`.
#[test]
fn an_interrupted_scan_says_sparql_interrupted_with_sqlite_interrupt() {
    let c = big();
    let quoted = |q: &str| format!("'{}'", q.replace('\'', "''"));
    c.execute_batch(&format!(
        "CREATE VIRTUAL TABLE temp.never USING sparql_view({});
         CREATE VIRTUAL TABLE temp.never_sql USING sparql_view({}, engine=sql);
         CREATE VIRTUAL TABLE temp.flow USING sparql_view({});",
        quoted(NEVER),
        quoted(NEVER),
        quoted(FLOW_THEN_NEVER)
    ))
    .unwrap();
    // xFilter: the first batch never fills.
    let e = interrupted(&c, "SELECT count(*) FROM sparql_rows(?1)", &[NEVER]);
    assert_sparql_interrupt(&e, "sparql_rows xFilter");
    let e = interrupted(&c, "SELECT count(*) FROM sparql_rows(?1, '{\"engine\":\"sql\"}')", &[NEVER]);
    assert_sparql_interrupt(&e, "sparql_rows xFilter, baseline engine");
    assert_sparql_interrupt(&interrupted(&c, "SELECT count(*) FROM never", &[]), "sparql_view xFilter");
    assert_sparql_interrupt(&interrupted(&c, "SELECT count(*) FROM never_sql", &[]), "sparql_view xFilter, sql");
    // xNext: batches flowed before the interrupt.
    let flowing =
        [("SELECT count(*) FROM sparql_rows(?1)", &[FLOW_THEN_NEVER][..]), ("SELECT count(*) FROM flow", &[])];
    for (sql, p) in flowing {
        let rows = status_i64(&c, "rows_emitted");
        assert_sparql_interrupt(&interrupted(&c, sql, p), sql);
        assert!(status_i64(&c, "rows_emitted") - rows >= 1024, "{sql}: interrupted in xNext");
    }
    assert_eq!(status_i64(&c, "open_cursors"), 0);
}

/// An interrupt that an `xColumn` call notices (here the dictionary probe behind an `id*` /
/// `__id` column of a computed term) keeps `SQLITE_INTERRUPT` too: rusqlite's column error
/// path would report `SQLITE_ERROR`.
#[test]
fn an_interrupt_noticed_in_xcolumn_keeps_sqlite_interrupt() {
    let c = big();
    let h = c.get_interrupt_handle();
    c.create_scalar_function("interrupt_now", 0, rusqlite::functions::FunctionFlags::SQLITE_UTF8, move |_| {
        h.interrupt();
        Ok(1i64)
    })
    .unwrap();
    const COMPUTED: &str = "SELECT ?c WHERE { ?s ?p ?o BIND(CONCAT(STR(?o), \" is a computed long string\") AS ?c) }";
    c.execute_batch(&format!("CREATE VIRTUAL TABLE temp.computed USING sparql_view('{COMPUTED}')")).unwrap();
    for (sql, p) in [
        ("SELECT interrupt_now(), id0 FROM sparql_rows(?1)", &[COMPUTED][..]),
        ("SELECT interrupt_now(), c__id FROM computed", &[]),
    ] {
        let e = c.query_row(sql, rusqlite::params_from_iter(p), |r| r.get::<_, Option<i64>>(1)).unwrap_err();
        assert_sparql_interrupt(&e, sql);
        // Nothing is left interrupted.
        assert_eq!(one::<i64>(&c, "SELECT count(*) FROM computed"), 300);
    }
}

/// Rows flowing: whichever side notices the interrupt first reports it. SQLite's own check
/// between rows (the VDBE loop) says `interrupted`, sparqlite's `sparql: interrupted`; the
/// code is SQLITE_INTERRUPT either way.
#[test]
fn an_interrupted_flowing_scan_always_reports_sqlite_interrupt() {
    let c = big();
    for sql in ["SELECT count(*) FROM sparql_rows(?1)", "SELECT max(v0) FROM sparql_rows(?1)"] {
        let e = interrupted(&c, sql, &[ENDLESS]);
        assert_eq!(e.sqlite_error_code(), Some(rusqlite::ErrorCode::OperationInterrupted), "{sql}: {e}");
        assert!(["sparql: interrupted", "interrupted"].contains(&e.to_string().as_str()), "{sql}: {e}");
    }
}

#[test]
fn timeout_ms_bounds_a_query() {
    let c = big();
    c.query_row("SELECT sparql_config('timeout_ms', 100)", [], |_| Ok(())).unwrap();
    let start = Instant::now();
    let e = c.query_row("SELECT sparql_count(?1)", [ENDLESS], |r| r.get::<_, i64>(0)).unwrap_err();
    assert!(e.to_string().contains("sparql: query timeout"), "{e}");
    assert!(start.elapsed() < Duration::from_secs(20));
    // Per-call override through sparql_rows options.
    c.query_row("SELECT sparql_config('timeout_ms', 0)", [], |_| Ok(())).unwrap();
    let e = c
        .query_row("SELECT count(*) FROM sparql_rows(?1, '{\"timeout_ms\": 50}')", [ENDLESS], |r| r.get::<_, i64>(0))
        .unwrap_err();
    assert!(e.to_string().contains("timeout"), "{e}");
}

/// Per-call options may lower the connection's limits, never raise or remove them: a
/// `sparql_rows` call or `sparql_view` table stored in a database file from someone else
/// cannot lift a limit the application set.
#[test]
fn per_call_options_cannot_lift_the_connection_limits() {
    let c = big();
    c.query_row("SELECT sparql_config('timeout_ms', 100)", [], |_| Ok(())).unwrap();
    c.execute_batch(&format!(
        "CREATE VIRTUAL TABLE temp.endless USING sparql_view('{ENDLESS}', timeout_ms=600000)"
    ))
    .unwrap();
    for (sql, opts) in [
        ("SELECT count(*) FROM sparql_rows(?1, ?2)", "{\"timeout_ms\": 0}"),
        ("SELECT count(*) FROM sparql_rows(?1, ?2)", "{\"timeout_ms\": 600000, \"engine\": \"sql\"}"),
        ("SELECT count(*) FROM endless WHERE ?1 IS NOT NULL AND ?2 IS NOT NULL", ""),
        ("SELECT count(*) FROM endless WHERE opts = ?2 AND ?1 IS NOT NULL", "{\"timeout_ms\": 0}"),
    ] {
        let start = Instant::now();
        let e = c.query_row(sql, [ENDLESS, opts], |r| r.get::<_, i64>(0)).unwrap_err();
        assert!(e.to_string().contains("sparql: query timeout"), "{sql} {opts}: {e}");
        assert!(start.elapsed() < Duration::from_secs(20), "{sql} {opts}");
    }
    c.query_row("SELECT sparql_config('timeout_ms', 0)", [], |_| Ok(())).unwrap();
    c.query_row("SELECT sparql_config('max_memory_mb', 1)", [], |_| Ok(())).unwrap();
    let e = c
        .query_row(
            "SELECT count(*) FROM sparql_rows(?1, '{\"max_memory_mb\": 4096}')",
            ["SELECT DISTINCT ?a ?c ?e WHERE { ?a ?p ?b . ?c ?q ?d . ?e ?r ?f }"],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_err();
    assert!(e.to_string().contains("memory budget exceeded (1 MB)"), "{e}");
}

#[test]
fn max_memory_mb_bounds_a_query() {
    let c = big();
    c.query_row("SELECT sparql_config('max_memory_mb', 1)", [], |_| Ok(())).unwrap();
    let e = c
        .query_row(
            "SELECT sparql_count('SELECT DISTINCT ?a ?c ?e WHERE { ?a ?p ?b . ?c ?q ?d . ?e ?r ?f }')",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_err();
    assert!(e.to_string().contains("memory budget exceeded"), "{e}");
}

/// Runs the named test of this binary in a child process with `env` set, and requires it to
/// succeed. For tests whose failure mode is a dead process (a stack overflow), or that need
/// an environment variable read when a connection registers.
fn in_child(test: &str, env: &str) {
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture", "--test-threads=1"])
        .env(env, "1")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    let tail: Vec<&str> = stderr.lines().rev().take(12).collect();
    assert!(out.status.success(), "{test} in a child process: {:?}\n{}", out.status, tail.join("\n"));
}

/// Release builds compile the `debug_panic` hook in, but arm it only with this switch.
const DEBUG_HOOKS: &str = "SPARQLITE_DEBUG_HOOKS";

#[test]
fn panics_in_every_entry_point_become_sql_errors() {
    if !cfg!(debug_assertions) && std::env::var_os(DEBUG_HOOKS).is_none() {
        return in_child("panics_in_every_entry_point_become_sql_errors", DEBUG_HOOKS);
    }
    let c = loaded();
    let q = "SELECT rowid, n, v0 FROM sparql_rows('SELECT ?s WHERE { ?s <http://ex/age> ?a }')";
    for site in ["scalar", "xbestindex", "xopen", "xfilter", "xnext", "xcolumn", "xrowid"] {
        c.query_row("SELECT sparql_config('debug_panic', ?1)", [site], |_| Ok(())).unwrap();
        let r = if site == "scalar" {
            c.query_row("SELECT sparql_count('ASK {}')", [], |_| Ok(()))
        } else {
            c.prepare(q)
                .and_then(|mut st| st.query_map([], |_| Ok(()))?.collect::<rusqlite::Result<Vec<()>>>().map(|_| ()))
        };
        let e = r.expect_err(site).to_string();
        assert!(e.contains("panic") || e.contains("unwinding"), "{site}: {e}");
        c.query_row("SELECT sparql_config('debug_panic', 'off')", [], |_| Ok(())).unwrap();
        assert_eq!(status_i64(&c, "open_cursors"), 0, "{site}");
        assert_eq!(one::<i64>(&c, "SELECT count(*) FROM sparql_rows('SELECT ?s WHERE { ?s ?p ?o }')"), 13, "{site}");
    }
    // xEof cannot report an error: the scan ends. xClose's panic is swallowed.
    for site in ["xeof", "xclose"] {
        c.query_row("SELECT sparql_config('debug_panic', ?1)", [site], |_| Ok(())).unwrap();
        let mut st = c.prepare(q).unwrap();
        let n = st.query_map([], |_| Ok(())).unwrap().count();
        drop(st);
        c.query_row("SELECT sparql_config('debug_panic', 'off')", [], |_| Ok(())).unwrap();
        assert!(n <= 3, "{site}");
        assert_eq!(status_i64(&c, "open_cursors"), 0, "{site}");
    }
    assert_eq!(one::<String>(&c, "SELECT sparql_config('debug_panic')"), "off");
}

#[test]
fn the_debug_hook_needs_a_debug_build_or_its_switch() {
    let c = conn();
    let r = c.query_row("SELECT sparql_config('debug_panic', 'xfilter')", [], |r| r.get::<_, String>(0));
    if cfg!(debug_assertions) || std::env::var_os(DEBUG_HOOKS).is_some() {
        assert_eq!(r.unwrap(), "xfilter");
    } else {
        let e = r.unwrap_err().to_string();
        assert!(e.contains("debug_panic is a testing hook"), "{e}");
        assert!(c.query_row("SELECT sparql_config('debug_panic')", [], |_| Ok(())).is_err());
        // Nothing panics: every row arrives.
        assert_eq!(one::<i64>(&c, "SELECT count(*) FROM sparql_rows('ASK {}')"), 1);
    }
}

#[test]
fn deeply_nested_requests_are_refused_by_every_entry_point() {
    let c = loaded();
    let query = format!("SELECT * WHERE {{ ?s ?p ?o FILTER({}?o{}) }}", "(".repeat(5000), ")".repeat(5000));
    let chain = format!("SELECT * WHERE {{ ?s ?p ?o FILTER(?o{} > 0) }}", " + 1".repeat(20_000));
    for sql in [
        "SELECT sparql_query(?1)",
        "SELECT sparql_ask(?1)",
        "SELECT sparql_count(?1)",
        "SELECT sparql_value(?1)",
        "SELECT sparql_explain(?1)",
        "SELECT sparql_explain(?1, 'algebra')",
        "SELECT sparql_explain(?1, 'sql')",
        "SELECT sparql_query_sql(?1)",
        "SELECT sparql_count_sql(?1)",
        "SELECT sparql_to_sql(?1)",
        "SELECT count(*) FROM sparql_rows(?1)",
    ] {
        for q in [&query, &chain] {
            let e = err(&c, sql, [q]);
            assert!(e.starts_with("sparql: unsupported: the request nests too deeply"), "{sql}: {e}");
        }
    }
    let update = format!("INSERT DATA {{ <http://ex/a> <http://ex/p> {}1{} }}", "( ".repeat(5000), " )".repeat(5000));
    let e = err(&c, "SELECT sparql_update(?1)", [&update]);
    assert!(e.contains("nests too deeply"), "{e}");
    let e = c
        .execute_batch(&format!("CREATE VIRTUAL TABLE temp.deep USING sparql_view('{query}')"))
        .unwrap_err()
        .to_string();
    assert!(e.contains("nests too deeply"), "{e}");
    assert_eq!(one::<i64>(&c, "SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }')"), 13);
}

/// Requests just inside the nesting limit run on a 2 MiB thread — the default stack of
/// spawned Rust threads — on both engines; a quarter more is refused. A stack overflow kills
/// the process, so the requests run in a child process.
#[test]
fn requests_inside_the_nesting_limit_fit_a_two_mib_stack() {
    const CHILD: &str = "SQ_API_NESTING_CHILD";
    if std::env::var_os(CHILD).is_none() {
        return in_child("requests_inside_the_nesting_limit_fit_a_two_mib_stack", CHILD);
    }
    type Shape = (&'static str, fn(usize) -> String, usize);
    let shapes: [Shape; 14] = [
        ("parentheses", |d| format!("SELECT * WHERE {{ ?s ?p ?o FILTER({}?o{}) }}", "(".repeat(d), ")".repeat(d)), 190),
        ("groups", |d| format!("SELECT * WHERE {}?s ?p ?o{}", "{ ".repeat(d), " }".repeat(d)), 80),
        ("functions", |d| format!("SELECT * WHERE {{ ?s ?p ?o FILTER({}?o{} != 0) }}", "STR(".repeat(d), ")".repeat(d)), 190),
        ("plus", |d| format!("SELECT * WHERE {{ ?s ?p ?o FILTER(?o{} > 0) }}", " + 1".repeat(d)), 950),
        ("or", |d| format!("SELECT * WHERE {{ ?s ?p ?o FILTER(?o = 0{}) }}", " || ?o = 1".repeat(d)), 950),
        ("union", |d| format!("SELECT * WHERE {{ {{ ?s ?p ?o }}{} }}", " UNION { ?s ?p ?o }".repeat(d)), 240),
        ("optional", |d| format!("SELECT * WHERE {{ ?s ?p ?o{} }}", " OPTIONAL { ?s ?p ?o }".repeat(d)), 240),
        ("exists", |d| format!("SELECT * WHERE {{ ?s ?p ?o {}{} }}", "FILTER EXISTS { ?s ?p ?o ".repeat(d), "}".repeat(d)), 80),
        ("subqueries", |d| format!("SELECT * WHERE {}?s ?p ?o{}", "{ SELECT * WHERE { ".repeat(d), " } }".repeat(d)), 34),
        ("blank nodes", |d| format!("SELECT * WHERE {{ ?a <http://ex/p> {}1{} }}", "[ <http://ex/q> ".repeat(d), " ]".repeat(d)), 105),
        // A basic graph pattern is a left-deep join with one level per triple pattern; nested
        // plain groups are flattened into it. (The patterns add no variable: the 256-variable
        // limit does not stop them.)
        ("triples", |d| format!("SELECT * WHERE {{ {} }}", (0..d).map(|i| format!("?s <http://ex/p{i}> <http://ex/o{i}> . ")).collect::<String>()), 240),
        ("object list", |d| format!("SELECT * WHERE {{ ?s <http://ex/p> {} }}", (0..d).map(|i| format!("<http://ex/o{i}>")).collect::<Vec<_>>().join(", ")), 240),
        ("groups of triples", |d| {
            let group = |g: usize| format!("{{ {} }}", (0..d / 4).map(|i| format!("?s <http://ex/p{g}x{i}> 1 . ")).collect::<String>());
            format!("SELECT * WHERE {{ {} }}", (0..4).map(group).collect::<String>())
        }, 236),
        // spargebra binds each DESCRIBE target in a chain of BIND nodes, and lowering recurses
        // over it before the variable limit applies.
        ("describe", |d| format!("DESCRIBE {}", (0..d).map(|i| format!("<http://ex/a{i}> ")).collect::<String>()), 480),
    ];
    std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(move || {
            let c = loaded();
            for (name, build, depth) in shapes {
                for engine in ["native", "sql"] {
                    let opts = format!("{{\"engine\":\"{engine}\"}}");
                    // Executing is what matters (the baseline may refuse SQL that deep).
                    let r = c.query_row("SELECT count(*) FROM sparql_rows(?1, ?2)", [build(depth), opts], |r| {
                        r.get::<_, i64>(0)
                    });
                    if let Err(e) = r {
                        assert!(!e.to_string().contains("nests too deeply"), "{name} at {depth} ({engine}): {e}");
                    }
                }
                let e = err(&c, "SELECT sparql_count(?1)", [build(depth * 5 / 4)]);
                assert!(e.contains("nests too deeply"), "{name} at {}: {e}", depth * 5 / 4);
            }
            let update = |d: usize| format!("INSERT DATA {{ <http://ex/a> <http://ex/p> {}1{} }}", "( ".repeat(d), " )".repeat(d));
            assert!(one_p::<i64, _>(&c, "SELECT sparql_update(?1)", [update(190)]) > 0);
            assert!(err(&c, "SELECT sparql_update(?1)", [update(240)]).contains("nests too deeply"));
            let delete_where = |d: usize| {
                format!("DELETE WHERE {{ {} }}", (0..d).map(|i| format!("<http://ex/alice> <http://ex/p{i}> ?o . ")).collect::<String>())
            };
            assert_eq!(one_p::<i64, _>(&c, "SELECT sparql_update(?1)", [delete_where(240)]), 0);
            assert!(err(&c, "SELECT sparql_update(?1)", [delete_where(300)]).contains("nests too deeply"));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn close_hook_finalizes_the_pool_so_the_connection_closes_fully() {
    let path = TempPath::new("close");
    let c = Connection::open(&path.0).unwrap();
    c.pragma_update(None, "journal_mode", "wal").unwrap();
    sq_api::register(&c).unwrap();
    c.query_row("SELECT sparql_load(?1)", [DATA], |_| Ok(())).unwrap();
    assert_eq!(one::<i64>(&c, "SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }')"), 13);
    assert_eq!(status_i64(&c, "armed"), 1);
    assert!(status_i64(&c, "pool_statements") > 0, "idle pooled statements exist");
    assert!(path.sibling("-wal").exists());
    // sqlite3_close fails with SQLITE_BUSY while statements are unfinalized; the hook
    // finalizes them first, and the last connection's close removes the WAL file.
    c.close().map_err(|(_, e)| e).unwrap();
    assert!(!path.sibling("-wal").exists(), "WAL file removed: no zombie connection");
}

#[test]
fn the_close_hook_survives_ddl_vacuum_and_other_connections() {
    let path = TempPath::new("ddl");
    let c = Connection::open(&path.0).unwrap();
    sq_api::register(&c).unwrap();
    c.query_row("SELECT sparql_load(?1)", [DATA], |_| Ok(())).unwrap();
    let other = Connection::open(&path.0).unwrap();
    other.execute_batch("CREATE TABLE side(x); DROP TABLE side; CREATE INDEX extra ON rdf_term(lex);").unwrap();
    c.execute_batch("CREATE TABLE mine(x); VACUUM;").unwrap();
    assert_eq!(one::<i64>(&c, "SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }')"), 13);
    assert_eq!(status_i64(&c, "armed"), 1);
    assert!(status_i64(&c, "pool_statements") > 0);
    drop(other);
    c.close().map_err(|(_, e)| e).unwrap();
}

#[test]
fn an_interrupted_update_leaves_nothing_behind() {
    let c = big();
    let t = interrupt_after(&c, 150);
    let update = format!(
        "INSERT {{ ?a <http://ex/made> ?h }} WHERE {{ {} }}",
        &ENDLESS["SELECT * WHERE { ".len()..ENDLESS.len() - 1]
    );
    let e = c.query_row("SELECT sparql_update(?1)", [&update], |r| r.get::<_, i64>(0)).unwrap_err();
    t.join().unwrap();
    assert!(e.to_string().contains("interrupt"), "{e}");
    assert!(c.is_autocommit(), "the write scope's transaction is gone");
    assert_eq!(one::<i64>(&c, "SELECT sparql_count('SELECT * WHERE { ?s <http://ex/made> ?o }')"), 0);
    assert_eq!(one::<i64>(&c, "SELECT sparql_update('INSERT DATA { <http://ex/a> <http://ex/made> 1 }')"), 1);
}
