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
    assert!(out.status.success(), "{test} in a child process: {:?}\n{}", out.status, stderr.lines().rev().take(12).collect::<Vec<_>>().join("\n"));
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
    let shapes: [Shape; 10] = [
        ("parentheses", |d| format!("SELECT * WHERE {{ ?s ?p ?o FILTER({}?o{}) }}", "(".repeat(d), ")".repeat(d)), 190),
        ("groups", |d| format!("SELECT * WHERE {}?s ?p ?o{}", "{ ".repeat(d), " }".repeat(d)), 80),
        ("functions", |d| format!("SELECT * WHERE {{ ?s ?p ?o FILTER({}?o{} != 0) }}", "STR(".repeat(d), ")".repeat(d)), 190),
        ("plus", |d| format!("SELECT * WHERE {{ ?s ?p ?o FILTER(?o{} > 0) }}", " + 1".repeat(d)), 950),
        ("or", |d| format!("SELECT * WHERE {{ ?s ?p ?o FILTER(?o = 0{}) }}", " || ?o = 1".repeat(d)), 950),
        ("union", |d| format!("SELECT * WHERE {{ {{ ?s ?p ?o }}{} }}", " UNION { ?s ?p ?o }".repeat(d)), 240),
        ("optional", |d| format!("SELECT * WHERE {{ ?s ?p ?o{} }}", " OPTIONAL { ?s ?p ?o }".repeat(d)), 240),
        ("exists", |d| format!("SELECT * WHERE {{ ?s ?p ?o {}{} }}", "FILTER EXISTS { ?s ?p ?o ".repeat(d), "}".repeat(d)), 80),
        ("subqueries", |d| format!("SELECT * WHERE {}?s ?p ?o{}", "{ SELECT * WHERE { ".repeat(d), " } }".repeat(d)), 34),
        ("blank nodes", |d| format!("SELECT * WHERE {{ ?a <http://ex/p> {}1{} }}", "[ <http://ex/q> ".repeat(d), " ]".repeat(d)), 190),
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
