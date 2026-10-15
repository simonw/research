//! Adversarial review tests, round 2 (WP11).
mod common;

use common::*;
use rusqlite::Connection;
use rusqlite::types::Value;

/// Runs `shape` in a child process (this binary, `--exact <test>`) on an 8 MiB thread — the
/// main-thread stack of a typical host such as Python — and reports how the child ended.
fn child_survives(test: &str, env: &str, shape: &str) -> Result<String, String> {
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture", "--test-threads=1"])
        .env(env, shape)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let tail = stderr.lines().rev().take(4).collect::<Vec<_>>().join(" | ");
    if out.status.success() { Ok(tail) } else { Err(format!("{:?}: {tail}", out.status)) }
}

fn probe_shape(shape: &str) -> String {
    let n = 20_000;
    match shape {
        "star" => format!(
            "SELECT * WHERE {{ {} }}",
            (0..n).map(|i| format!("?s <http://ex/p{i}> ?o{i} .")).collect::<Vec<_>>().join(" ")
        ),
        "chain" => format!(
            "SELECT * WHERE {{ {} }}",
            (0..n).map(|i| format!("?x{i} <http://ex/knows> ?x{} .", i + 1)).collect::<Vec<_>>().join(" ")
        ),
        "collection" => format!(
            "SELECT * WHERE {{ ?s <http://ex/p> ({}) }}",
            (0..n).map(|i| i.to_string()).collect::<Vec<_>>().join(" ")
        ),
        "onevar" => format!(
            "SELECT * WHERE {{ {} }}",
            (0..n).map(|i| format!("?s <http://ex/p{i}> <http://ex/o{i}> .")).collect::<Vec<_>>().join(" ")
        ),
        "consts" => format!(
            "ASK {{ {} }}",
            (0..n).map(|i| format!("<http://ex/alice> <http://ex/p{i}> <http://ex/o{i}> .")).collect::<Vec<_>>().join(" ")
        ),
        "onevar_union" => format!(
            "SELECT * WHERE {{ {{ ?s ?p ?o }} UNION {{ {} }} }}",
            (0..n).map(|i| format!("?s <http://ex/p{i}> <http://ex/o{i}> .")).collect::<Vec<_>>().join(" ")
        ),
        "in" => format!(
            "SELECT * WHERE {{ ?s ?p ?o FILTER(?o IN ({})) }}",
            (0..n).map(|i| format!("\"v{i}\"")).collect::<Vec<_>>().join(", ")
        ),
        "concat" => format!(
            "SELECT * WHERE {{ ?s ?p ?o BIND(CONCAT({}) AS ?c) }}",
            (0..n).map(|i| format!("\"v{i}\"")).collect::<Vec<_>>().join(", ")
        ),
        "values_vars" => format!(
            "SELECT * WHERE {{ VALUES ({}) {{ ({}) }} }}",
            (0..n).map(|i| format!("?v{i}")).collect::<Vec<_>>().join(" "),
            (0..n).map(|i| i.to_string()).collect::<Vec<_>>().join(" ")
        ),
        "group_by" => format!(
            "SELECT (COUNT(*) AS ?c) WHERE {{ ?s ?p ?o }} GROUP BY {}",
            (0..n).map(|_| "?s").collect::<Vec<_>>().join(" ")
        ),
        "order_by" => format!(
            "SELECT * WHERE {{ ?s ?p ?o }} ORDER BY {}",
            (0..n).map(|_| "?s").collect::<Vec<_>>().join(" ")
        ),
        "graph_star" => format!(
            "SELECT * WHERE {{ GRAPH ?g {{ {} }} }}",
            (0..n).map(|i| format!("?s <http://ex/p{i}> ?o{i} .")).collect::<Vec<_>>().join(" ")
        ),
        "update_where" => format!(
            "DELETE {{ ?s ?p ?o }} WHERE {{ {} }}",
            (0..n).map(|i| format!("?x{i} <http://ex/knows> ?x{} .", i + 1)).collect::<Vec<_>>().join(" ")
        ),
        "update_ops" => (0..n).map(|i| format!("INSERT DATA {{ <http://ex/s{i}> <http://ex/p> {i} }}")).collect::<Vec<_>>().join(" ;\n"),
        other => panic!("unknown shape {other}"),
    }
}

const SHAPE_ENV: &str = "SQ_API_REVIEW2_SHAPE";

/// Requests whose size the nesting check does not count must still not overflow an 8 MiB
/// host thread (a stack overflow kills the host process; catch_unwind cannot contain it).
#[test]
fn review_wide_flat_requests_do_not_crash_the_host() {
    if let Ok(shape) = std::env::var(SHAPE_ENV) {
        let t = std::thread::Builder::new()
            .stack_size(8 << 20)
            .spawn(move || {
                let c = loaded();
                let text = probe_shape(&shape);
                if shape.starts_with("update") {
                    let r = c.query_row("SELECT sparql_update(?1)", [&text], |r| r.get::<_, i64>(0));
                    eprintln!("{shape}: {r:?}");
                    return;
                }
                for engine in ["native", "sql"] {
                    let opts = format!("{{\"engine\":\"{engine}\"}}");
                    let r = c.query_row("SELECT count(*) FROM sparql_rows(?1, ?2)", [&text, &opts], |r| r.get::<_, i64>(0));
                    eprintln!("{shape} {engine}: {:?}", r.as_ref().map_err(|e| e.to_string().chars().take(160).collect::<String>()));
                }
            })
            .unwrap();
        t.join().unwrap();
        return;
    }
    let shapes = std::env::var("SQ_API_REVIEW2_SHAPES").unwrap_or_else(|_| {
        "star,chain,collection,in,concat,values_vars,group_by,order_by,graph_star,update_where,update_ops".into()
    });
    let mut died = Vec::new();
    for shape in shapes.split(',') {
        match child_survives("review_wide_flat_requests_do_not_crash_the_host", SHAPE_ENV, shape) {
            Ok(tail) => eprintln!("ok {shape}: {tail}"),
            Err(e) => died.push(format!("{shape}: {e}")),
        }
    }
    assert!(died.is_empty(), "the host process died on:\n{}", died.join("\n"));
}

#[allow(dead_code)]
fn rows(c: &Connection, sql: &str) -> Vec<Vec<Value>> {
    let mut st = c.prepare(sql).unwrap_or_else(|e| panic!("{sql}: {e}"));
    let n = st.column_count();
    st.query_map([], |r| (0..n).map(|i| r.get::<_, Value>(i)).collect())
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap_or_else(|e| panic!("{sql}: {e}"))
}
