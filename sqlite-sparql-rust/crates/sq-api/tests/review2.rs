//! Adversarial review tests, round 2 (WP11). Tests that pin down a defect fail until it is
//! fixed; the others are regression tests for behaviour the review verified.
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

/// The nesting check (`nesting.rs`) is the only thing standing between untrusted request text
/// and a stack overflow, which kills the host (catch_unwind cannot contain it; Python dies
/// with SIGSEGV). It does not count plain triple patterns (nor object / predicate lists) or
/// DESCRIBE targets, yet
/// * the native engine turns a basic graph pattern of N triples into a left-deep join N
///   levels deep and recurses over it (`sq_exec::build::Builder::build_in`, seen in gdb).
///   Patterns that add no new variable dodge the 256-variable limit: ~2 200 of them (≈150 KB
///   of text) overflow Python's 8 MiB main thread with the release extension, through
///   `sparql_count`, `sparql_rows`, `sparql_view` scans and `sparql_update` (`DELETE WHERE`);
/// * `DESCRIBE <a1> … <aN>` is a chain of N BIND (Extend) nodes in spargebra, and
///   `sq_plan::lower::Lowerer::pattern` recurses over it before the variable limit is
///   checked: 6 000 IRIs overflow the same thread.
/// Each child below runs one such request on an 8 MiB thread and must survive (a refusal or
/// a result are both fine).
#[test]
fn review_wide_requests_do_not_crash_the_host() {
    const CHILD: &str = "SQ_API_REVIEW2_WIDE";
    let shape = |name: &str| -> (&'static str, String) {
        match name {
            "bgp" => (
                "SELECT sparql_count(?1)",
                format!(
                    "SELECT * WHERE {{ {} }}",
                    (0..3000).map(|i| format!("?s <http://ex/p{i}> <http://ex/o{i}> . ")).collect::<String>()
                ),
            ),
            "object_list" => (
                "SELECT count(*) FROM sparql_rows(?1)",
                format!(
                    "SELECT * WHERE {{ ?s <http://ex/p> {} }}",
                    (0..3000).map(|i| format!("<http://ex/o{i}>")).collect::<Vec<_>>().join(", ")
                ),
            ),
            "delete_where" => (
                "SELECT sparql_update(?1)",
                format!(
                    "DELETE WHERE {{ {} }}",
                    (0..3000).map(|i| format!("<http://ex/alice> <http://ex/p{i}> ?o . ")).collect::<String>()
                ),
            ),
            "describe" => (
                "SELECT sparql_count(?1)",
                format!("DESCRIBE {}", (0..8000).map(|i| format!("<http://ex/a{i}> ")).collect::<String>()),
            ),
            other => panic!("unknown shape {other}"),
        }
    };
    if let Ok(name) = std::env::var(CHILD) {
        let t = std::thread::Builder::new()
            .stack_size(8 << 20)
            .spawn(move || {
                let c = loaded();
                let (sql, text) = shape(&name);
                assert!(text.len() < 200_000);
                let r = c.query_row(sql, [&text], |r| r.get::<_, Value>(0));
                eprintln!("{name}: {:?}", r.map_err(|e| e.to_string()));
            })
            .unwrap();
        t.join().unwrap();
        return;
    }
    let mut died = Vec::new();
    for name in ["bgp", "object_list", "delete_where", "describe"] {
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "review_wide_requests_do_not_crash_the_host", "--nocapture", "--test-threads=1"])
            .env(CHILD, name)
            .output()
            .unwrap();
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let tail = stderr.lines().filter(|l| l.contains("overflow") || l.contains("fatal")).take(2).collect::<Vec<_>>();
            died.push(format!("{name}: {:?} {}", out.status, tail.join(" | ")));
        }
    }
    assert!(died.is_empty(), "the host process died on:\n{}", died.join("\n"));
}

/// `ConnState::write_call` skips the write scope whenever *some* statement holds a write
/// transaction in autocommit mode (`host_statement_writes`), trusting that statement to roll
/// everything back if the update fails. That holds only when the writer is the host statement
/// itself. Here the host is a read-only `SELECT sparql_update(…)` and the writer is another,
/// still-pending statement (an `INSERT … RETURNING` whose rows are not all read yet): the
/// failed request leaves its first operation behind, and the pending statement then commits
/// it. §8.2: "one write scope per request" (all or nothing).
#[test]
fn review_failed_update_beside_a_pending_writer_is_not_half_applied() {
    let c = loaded();
    c.execute_batch("CREATE TABLE t(x)").unwrap();
    let req =
        "INSERT DATA { <http://ex/new> <http://ex/p> 1 } ; CREATE GRAPH <http://ex/g> ; CREATE GRAPH <http://ex/g>";
    {
        let mut ins = c.prepare("INSERT INTO t VALUES (1), (2) RETURNING x").unwrap();
        let mut pending = ins.query([]).unwrap();
        pending.next().unwrap().unwrap();
        assert!(c.is_autocommit());
        let r = c.query_row("SELECT sparql_update(?1)", [req], |r| r.get::<_, i64>(0));
        assert!(r.is_err(), "the request fails: {r:?}");
    }
    // The pending INSERT finished (its statement was reset): its transaction committed.
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM t"), 2);
    assert_eq!(
        one::<i64>(&c, "SELECT sparql_ask('ASK { <http://ex/new> ?p ?o }')"),
        0,
        "the failed request's INSERT DATA was committed by the unrelated pending statement"
    );
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM rdf_graph"), 0);
}

/// §8.3b: pushdown "keeps SQL semantics". A `__term` constraint carries the comparison's
/// collation (`COLLATE NOCASE`, `RTRIM`, or the user's own), but xBestIndex consumes it
/// (omit = true) and xFilter compares terms exactly, so rows SQL would keep are dropped.
/// `IndexInfo::collation` (sqlite3_vtab_collation, in the 3.34.1 bindings and not gated on
/// modern_sqlite) tells BINARY constraints, the only ones sameTerm implements, apart.
#[test]
fn review_view_term_pushdown_respects_the_constraint_collation() {
    let c = loaded();
    c.execute_batch(
        "CREATE VIRTUAL TABLE temp.people USING sparql_view('SELECT ?s ?n WHERE { ?s <http://ex/name> ?n }')",
    )
    .unwrap();
    for cond in [
        "s__term = '<HTTP://EX/BOB>' COLLATE NOCASE",
        "s__term = '<http://ex/bob>   ' COLLATE RTRIM",
        "n__term = '\"BOB\"' COLLATE NOCASE",
    ] {
        let in_sql = rows(&c, &format!("SELECT n FROM people WHERE +{cond}"));
        assert_eq!(in_sql, vec![vec![Value::Text("Bob".into())]], "{cond}");
        let pushed = rows(&c, &format!("SELECT n FROM people WHERE {cond}"));
        assert_eq!(pushed, in_sql, "pushed-down {cond} must select the rows SQL selects");
    }
}

/// §8.5 / R6: `timeout_ms` bounds a query. The baseline engine checks the deadline only
/// between 1024-row batches (`BaselineRun::fill`), so a batch SQLite takes long to produce —
/// a selective FILTER over a large join — or the single row of `sparql_count_sql`'s
/// `SELECT count(*) FROM (…)` runs unbounded (until a watchdog interrupts it here), while the
/// native engine stops at the deadline. (`sqlite3_interrupt`, on the allow-list, from a timer
/// would bound it without taking the user's progress-handler slot.)
#[test]
fn review_baseline_engine_honours_timeout_ms() {
    let c = conn();
    let nt: String =
        (0..300).map(|i| format!("<http://ex/s{i}> <http://ex/p{}> <http://ex/o{}> .\n", i % 7, i % 13)).collect();
    c.query_row("SELECT sparql_load(?1, 'nt')", [&nt], |_| Ok(())).unwrap();
    c.query_row("SELECT sparql_config('timeout_ms', 200)", [], |_| Ok(())).unwrap();
    let never = "SELECT * WHERE { ?a ?p ?b . ?c ?q ?d . ?e ?r ?f FILTER(CONCAT(STR(?b), STR(?d), STR(?f)) = \"never\") }";
    let endless =
        "SELECT * WHERE { ?a ?p ?b . ?c ?q ?d . ?e ?r ?f . ?g ?s ?h FILTER(?b != ?d && ?d != ?f && ?f != ?h) }";
    for (sql, q) in [
        ("SELECT sparql_count(?1)", never),
        ("SELECT sparql_query_sql(?1)", never),
        ("SELECT sparql_count_sql(?1)", endless),
    ] {
        let handle = c.get_interrupt_handle();
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let watchdog = std::thread::spawn(move || {
            if rx.recv_timeout(std::time::Duration::from_secs(8)).is_err() {
                handle.interrupt();
            }
        });
        let start = std::time::Instant::now();
        let r = c.query_row(sql, [q], |r| r.get::<_, Value>(0));
        let elapsed = start.elapsed();
        let _ = tx.send(());
        watchdog.join().unwrap();
        let e = r.expect_err(sql).to_string();
        assert!(
            e.contains("timeout") && elapsed < std::time::Duration::from_secs(4),
            "{sql}: timeout_ms = 200 not honoured: {e} after {elapsed:?}"
        );
    }
}

// ------------------------------------------------------------------ verified behaviour

/// `__term` pushdown of a computed term that is not in the store (a TEMP constant) agrees
/// with filtering in SQL, on both engines.
#[test]
fn review_view_term_pushdown_of_a_computed_term_matches_sql_filtering() {
    let c = loaded();
    for engine in ["native", "sql"] {
        c.execute_batch(&format!(
            "DROP TABLE IF EXISTS temp.cv;
             CREATE VIRTUAL TABLE temp.cv USING sparql_view(
               'SELECT ?x WHERE {{ BIND(CONCAT(\"not in the \", \"store at all\") AS ?x) }}', engine={engine})"
        ))
        .unwrap();
        let pushed = rows(&c, r#"SELECT x, x__id FROM cv WHERE x__term = '"not in the store at all"'"#);
        let in_sql = rows(&c, r#"SELECT x, x__id FROM cv WHERE +x__term = '"not in the store at all"'"#);
        assert_eq!(pushed, in_sql, "{engine}");
        assert_eq!(pushed, vec![vec![Value::Text("not in the store at all".into()), Value::Null]], "{engine}");
    }
}
