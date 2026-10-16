//! Reviewer tests of the engine-lane performance pass: `sparql_count` of an aggregate query
//! evaluates it (E1), and lazily built ORDER BY keys (E2) give the baseline engine's order.
mod common;

use common::*;
use rusqlite::Connection;

/// A cross product nobody can finish (`FILTER` defeats every counting shortcut).
const ENDLESS: &str = "?a ?p ?b . ?c ?q ?d . ?e ?r ?f . ?g ?s ?h FILTER(?b != ?d && ?d != ?f && ?f != ?h)";

fn big() -> Connection {
    let c = conn();
    let mut nt = String::new();
    for i in 0..300 {
        nt.push_str(&format!("<http://ex/s{i}> <http://ex/p{}> <http://ex/o{}> .\n", i % 7, i % 13));
    }
    c.query_row("SELECT sparql_load(?1, 'nt')", [&nt], |_| Ok(())).unwrap();
    c
}

/// `sparql_count` of `SELECT (COUNT(*) AS ?n) { P }` runs P: an endless P times out instead of
/// reporting the implicit group's one row for free (it used to return 1 in 0 ms).
#[test]
fn review_perf_count_of_an_aggregate_query_runs_the_aggregation() {
    let c = big();
    assert_eq!(one::<i64>(&c, "SELECT sparql_count('SELECT (COUNT(*) AS ?n) { ?s ?p ?o }')"), 1);
    assert_eq!(one::<i64>(&c, "SELECT sparql_value('SELECT (COUNT(*) AS ?n) { ?s ?p ?o }')"), 300);
    c.query_row("SELECT sparql_config('timeout_ms', 100)", [], |_| Ok(())).unwrap();
    for q in [
        format!("SELECT (COUNT(*) AS ?n) {{ {ENDLESS} }}"),
        format!("SELECT ?n {{ {{ SELECT (COUNT(*) AS ?n) {{ {ENDLESS} }} }} }} LIMIT 1"),
    ] {
        let e = err(&c, "SELECT sparql_count(?1)", [&q]);
        assert!(e.contains("timeout"), "{q}: {e}");
    }
}

/// Mixed-type data: inline and dictionary integers (non-canonical, beyond the inline range),
/// decimals, doubles, strings, language strings, custom datatypes, IRIs, blank nodes, dates,
/// booleans, and subjects without `:b` (unbound keys through OPTIONAL).
fn mixed() -> Connection {
    let c = conn();
    let vals = [
        "1", "-3", "\"01\"^^xsd:integer", "\"1152921504606846976\"^^xsd:integer", "1.5", "2.0", "1e0",
        "\"NaN\"^^xsd:double", "\"a\"", "\"a longer string\"", "\"chat\"@fr", "\"chat\"@en",
        "\"x\"^^:dt", ":i1", ":i2", "_:b1", "_:b2", "\"2020-01-01\"^^xsd:date", "true", "\"07\"^^xsd:int",
    ];
    let mut ttl = String::from("@prefix : <http://e/> . @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .\n");
    for i in 0..240usize {
        ttl.push_str(&format!(":s{i:03} :a {} ; :c {} .\n", vals[(i * 7) % vals.len()], i % 3));
        if i % 4 != 0 {
            ttl.push_str(&format!(":s{i:03} :b {} .\n", vals[(i * 11 + 3) % vals.len()]));
        }
    }
    c.query_row("SELECT sparql_load(?1, 'turtle')", [&ttl], |_| Ok(())).unwrap();
    c
}

fn seq(c: &Connection, q: &str, engine: &str) -> Vec<String> {
    let opts = format!("{{\"engine\":\"{engine}\"}}");
    let mut st = c.prepare("SELECT t0, t1, t2, t3 FROM sparql_rows(?1, ?2)").unwrap();
    st.query_map([q, &opts], |r| {
        Ok((0..4).map(|i| r.get::<_, Option<String>>(i).unwrap().unwrap_or_default()).collect::<Vec<_>>().join(" | "))
    })
    .unwrap()
    .map(Result::unwrap)
    .collect()
}

/// Sort (no LIMIT) and TopK (LIMIT) with lazily built keys give exactly the baseline engine's
/// row sequence, for ASC / DESC mixes, unbound keys and expression keys (total orders: ?s
/// breaks every tie).
#[test]
fn review_perf_lazy_order_by_keys_agree_with_the_baseline_engine() {
    let c = mixed();
    let body = "?s :a ?a ; :c ?c OPTIONAL { ?s :b ?b }";
    let orders = [
        "DESC(?c) ?b ?s",
        "?a DESC(?b) ?s",
        "?c DESC(?a) ?s",
        "DESC(?b) DESC(?a) DESC(?s)",
        "(STR(?a)) ?c ?s",
        "DESC(COALESCE(?b, ?c)) ?s",
        "?b ?c ?a ?s",
    ];
    for o in orders {
        for lim in ["", " LIMIT 1", " LIMIT 7", " OFFSET 5 LIMIT 20", " LIMIT 1000"] {
            let q = format!("PREFIX : <http://e/> SELECT ?s ?a ?b ?c {{ {body} }} ORDER BY {o}{lim}");
            let native = seq(&c, &q, "native");
            assert!(!native.is_empty(), "{q}");
            assert_eq!(native, seq(&c, &q, "sql"), "{q}");
        }
    }
}
