//! Adversarial review (WP10, round 2): shapes the differential fuzzer's small datasets cannot
//! reach (more than 50,000 left rows, so the BindJoin adaptive switch to hashing fires;
//! intermediate results spanning many batches), compared with sq-oracle.
#[allow(dead_code, unused_imports)]
mod common;
#[allow(dead_code, unused_imports)]
mod fuzz;

use std::rc::Rc;

use sq_core::plan::{JoinPolicy, QueryOptions};
use sq_core::store::QuadStore;

use fuzz::compare::{same, Free};
use fuzz::engines::{native, oracle};
use fuzz::query::Order;

fn large_store() -> sq_memstore::MemStore {
    let mut ttl = String::new();
    // 90,000 :p edges into 100 hubs; hubs have :q edges, some :r chains and a few graphs.
    for i in 0..90_000 {
        ttl.push_str(&format!(":s{i} :p :o{} .\n", i % 100));
    }
    for j in 0..100 {
        if j % 3 != 0 {
            ttl.push_str(&format!(":o{j} :q :x{} .\n", j % 7));
        }
        if j % 5 == 0 {
            ttl.push_str(&format!(":o{j} :r :o{} .\n", (j + 5) % 100));
        }
        if j % 4 == 0 {
            ttl.push_str(&format!(":g{} {{ :o{j} :q :y{} }}\n", j % 3, j % 2));
        }
        ttl.push_str(&format!(":o{j} :n {j} .\n"));
    }
    common::mem(&ttl)
}

const QUERIES: &[&str] = &[
    "SELECT ?s ?x { ?s :p ?o . ?o :q ?x }",
    "SELECT ?s ?x { ?s :p ?o OPTIONAL { ?o :q ?x FILTER(?x != :x3) } }",
    "SELECT ?s { ?s :p ?o FILTER EXISTS { ?o :q ?x } }",
    "SELECT ?s { ?s :p ?o FILTER NOT EXISTS { ?o :q :x1 } }",
    "SELECT ?s ?o { ?s :p ?o MINUS { ?o :q :x2 } }",
    "SELECT ?s ?g { ?s :p ?o GRAPH ?g { ?o :q ?y } }",
    "SELECT ?s ?z { ?s :p ?o . ?o :r+ ?z }",
    "SELECT ?s ?z { ?s :p ?o . ?o :r+/:q ?z }",
    "SELECT ?s ?c { ?s :p ?o { SELECT ?o (COUNT(*) AS ?c) { ?o :n ?nn . ?o ?pp ?v } GROUP BY ?o } }",
    "SELECT ?s ?v { ?s :p ?o { ?o :q ?v } UNION { ?o :n ?v FILTER(?v > 90) } }",
    "SELECT ?s ?n { ?s :p ?o . ?o :n ?n FILTER(?n >= 10 && ?n < 20) }",
    "SELECT ?s ?k { ?s :p ?o { SELECT ?o (SAMPLE(?o) AS ?k) { ?o :q ?w } GROUP BY ?o } }",
    "SELECT (COUNT(*) AS ?c) { ?s :p ?o . ?o :q ?x }",
    "SELECT ?o (COUNT(?s) AS ?c) { ?s :p ?o OPTIONAL { ?o :q ?x } } GROUP BY ?o",
    "SELECT DISTINCT ?o ?x { ?s :p ?o . ?o :q ?x }",
    "ASK { ?s :p ?o . ?o :q :x6 . ?o :n 97 }",
    "SELECT ?s ?z { ?s :p ?o GRAPH :g1 { ?o :q* ?z } }",
    "SELECT ?s ?z { ?s :p ?o OPTIONAL { GRAPH ?g { ?o :q ?z } } }",
    "SELECT ?s ?x ?n { ?s :p ?o OPTIONAL { ?o :q ?x OPTIONAL { ?x :n ?n } } }",
    "SELECT ?s ?y { ?s :p ?o { ?o :q ?y } UNION { BIND(:k AS ?y) } }",
];

/// About a minute in the dev profile: `cargo test -p sq-engine --test review_wp10_large --
/// --ignored`. With `max_probe_counts = 0` the left scan is estimated at 10,000 rows, so past
/// 80,000 of its 90,000 rows the BindJoins switch to hashing (EXPLAIN ANALYZE shows ~7,800
/// right-side opens instead of ~8,900): the switch is exercised and agrees with the oracle.
#[test]
#[ignore = "long: 90,000-row store, every query under 8 configurations"]
fn review_large_inputs_agree_with_the_oracle() {
    let mem = large_store();
    let store: Rc<dyn QuadStore> = Rc::new(mem.clone());
    let mut failures = Vec::new();
    for q in QUERIES {
        let text = format!("{}{q}", common::PREFIXES);
        let parsed = sq_plan::parse_query(&text, Some(common::EX)).expect("query parses");
        let truth = oracle(&*store, &parsed, &QueryOptions::default());
        for policy in [JoinPolicy::Auto, JoinPolicy::PreferBind, JoinPolicy::PreferHash, JoinPolicy::PreferMerge] {
            for probes in [0, 32] {
                let opts = QueryOptions { join_policy: policy, max_probe_counts: probes, ..QueryOptions::default() };
                let got = native(store.clone(), &parsed, &opts);
                if let Err(d) = same(&truth, &got, Order::Bag, Free::Minted) {
                    let plan = sq_engine::explain(store.clone(), &text, sq_plan::ExplainMode::Plan, &opts).unwrap_or_default();
                    failures.push(format!("{q}\n  {policy:?} probes={probes}: {d}\n{plan}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{} mismatches:\n{}", failures.len(), failures.join("\n"));
}

/// A join with an aggregating sub-select on its GROUP BY key (sq-plan). `props::group`
/// (sq-plan/src/props.rs) marks GROUP BY keys only "maybe" bound, even plain variables that are
/// certain in the group's input, so the join gets no hash key: the planner picks either a
/// NestedLoop that checks every left row against every group (O(left x groups)) or a BindJoin
/// that re-runs the whole aggregation once per left row (50,000 Group opens here).
#[test]
#[ignore = "sq-plan performance defect (props::group), outside sq-engine: fails until sq-plan is fixed"]
fn review_join_on_a_group_by_key_evaluates_the_aggregation_once() {
    let (rows, groups) = (50_000, 100);
    let mut ttl = String::new();
    for i in 0..rows {
        ttl.push_str(&format!(":s{i} :p :o{} .\n", i % groups));
    }
    for j in 0..groups {
        ttl.push_str(&format!(":o{j} :q {j} , {} .\n", j + 1));
    }
    let store: Rc<dyn QuadStore> = Rc::new(common::mem(&ttl));
    let q = format!("{}SELECT ?s ?c {{ ?s :p ?o {{ SELECT ?o (COUNT(*) AS ?c) {{ ?o :q ?v }} GROUP BY ?o }} }}", common::PREFIXES);
    let analyze = sq_engine::explain(store, &q, sq_plan::ExplainMode::Analyze, &QueryOptions::default()).unwrap();
    let group_opens: u64 = analyze
        .lines()
        .find(|l| l.contains("Group keys="))
        .and_then(|l| l.split("opens=").nth(1))
        .and_then(|o| o.split_whitespace().next())
        .and_then(|n| n.parse().ok())
        .expect("a Group operator with an opens counter");
    assert!(!analyze.contains("NestedLoop") && group_opens <= groups as u64, "{analyze}");
}
