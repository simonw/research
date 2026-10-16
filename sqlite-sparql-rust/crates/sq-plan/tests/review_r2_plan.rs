//! Adversarial review of the round-2 plan lane (many-to-many merge costs, several-key join
//! estimates). Each test names what it guards or what it shows.
mod common;

use std::rc::Rc;

use common::*;
use sq_core::plan::{JoinPolicy, Op, QueryOptions, QueryPlan};
use sq_core::store::QuadStore;

fn with(policy: JoinPolicy) -> QueryOptions {
    QueryOptions { join_policy: policy, ..Default::default() }
}

/// Turtle (with its prefix) of `n` people with a city (20 values), an age (100 values), a
/// joined year-day (365 values) and three random `:knows` edges each. None of the attributes
/// is correlated with `:knows` or with each other.
fn attrs_ttl(n: usize) -> String {
    let mut ttl = String::from("@prefix : <http://ex/> .\n");
    let mut x: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    for i in 0..n {
        let (age, day) = (next() % 100, next() % 365);
        ttl.push_str(&format!(":p{i} :city :c{} ; :age {age} ; :joined {day} .\n", i % 20));
        for _ in 0..3 {
            ttl.push_str(&format!(":p{i} :knows :p{} .\n", next() % n as u64));
        }
    }
    ttl
}

/// MergeJoins / StarMerges keyed on `key` that check other shared slots.
fn checked_merges_on(p: &QueryPlan, key: u16) -> usize {
    nodes(p)
        .iter()
        .filter(|n| match &n.op {
            Op::MergeJoin { key: k, check, .. } | Op::StarMerge { key: k, check, .. } => *k == key && !check.is_empty(),
            _ => false,
        })
        .count()
}

/// Generalisation of the lane's fix beyond "friends of friends in the same city": joins whose
/// last step shares a selective slot (?b) and one or two low-cardinality attributes. A merge on
/// the attribute (20, 100 or 365 values) that checks ?b visits |L| × |R| / dv pairs; on the
/// 1.6M-triple benchmark graph the old planner's `MergeJoin key=?x check=[?b]` for the
/// city/joined pair timed out at 120 s where the new plan takes 1.1 s. No policy that merges may
/// pick such a key, and every plan must still match the oracle.
#[test]
fn review_r2_attribute_pairs_never_merge_on_the_attribute_checking_the_person() {
    let ttl = attrs_ttl(3_000);
    let mem = sq_memstore::MemStore::new();
    load_ttl(&mem, &ttl, None);
    mem.analyze();
    let db = Sqlite::new();
    db.load(&ttl, None);
    db.store.analyze().unwrap();
    let queries = [
        "SELECT ?a ?b { ?a :city ?x . ?a :joined ?j . ?b :city ?x . ?b :joined ?j }",
        "SELECT (COUNT(*) AS ?n) { ?a :city ?x . ?a :age ?g . ?b :city ?x . ?b :age ?g }",
        "SELECT ?a ?b { ?a :city ?x . ?a :age ?g . ?a :joined ?j . ?b :city ?x . ?b :age ?g . ?b :joined ?j }",
        "SELECT ?a ?b { ?a :knows ?b . ?a :city ?x . ?b :city ?x }",
        "SELECT ?a ?c { ?a :knows ?b . ?b :knows ?c . ?a :joined ?j . ?c :joined ?j }",
    ];
    let stores: [(&str, Rc<dyn QuadStore>); 2] = [("mem", Rc::new(mem.clone())), ("sqlite", Rc::new(db.store.clone()))];
    let mut failures = Vec::new();
    for q in queries {
        let expected = oracle(&lower(q, &mem), &mem).unwrap();
        for (name, st) in &stores {
            for policy in [JoinPolicy::Auto, JoinPolicy::NoSql, JoinPolicy::PreferMerge] {
                let opts = with(policy);
                let _read = (*name == "sqlite").then(|| {
                    let scope = db.store.read_scope(0).expect("read scope");
                    db.store.shared().validate_epoch().expect("epoch");
                    scope
                });
                let p = plan_on(q, &**st, &opts);
                let text = sq_plan::explain(&p, None);
                let low: usize = ["x", "g", "j"]
                    .iter()
                    .filter(|v| p.vars.vars.iter().any(|(n, _)| n == *v))
                    .map(|v| checked_merges_on(&p, slot(&p, v)))
                    .sum();
                if low > 0 {
                    failures.push(format!("[{name} {policy:?}] merges on an attribute: {q}\n{text}"));
                }
                match execute(Rc::clone(st), p, &opts) {
                    Ok(got) if got == expected => {}
                    Ok(got) => failures.push(format!(
                        "[{name} {policy:?}] {q}\n{text}expected {} rows, got {}",
                        expected.len(),
                        got.len()
                    )),
                    Err(e) => failures.push(format!("[{name} {policy:?}] {q}\n{text}error: {e}")),
                }
            }
        }
    }
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures[..failures.len().min(3)].join("\n\n"));
}

/// Estimate of the top of a group (the first join below Project / Extend / Count).
fn top_join_est(p: &QueryPlan) -> f64 {
    nodes(p)
        .into_iter()
        .find(|n| matches!(n.op, Op::HashJoin { .. } | Op::MergeJoin { .. } | Op::BindJoin { .. } | Op::StarMerge { .. }))
        .expect("a join")
        .est_rows
}

/// Guard (this failed while every second key was backed off; entity keys now multiply, see
/// `choice::combine_keys`). The exponential backoff of several join keys over-estimates cycle
/// closures, whose keys are independent, by a factor that grows with the square root of the
/// key's distinct values: 14-16x at 200 people, ~50x at 5,000. On the golden 200-person random
/// graph the triangle has 26 solutions; the old estimate (independent keys, SPEC §7.4.5) was
/// 27.8, the new one is 378. On the 1.6M-triple benchmark graph (200K people): 21 triangles
/// (+ name/age lookups) are estimated at 1.2e4 (old 37.8), 8 mutual `:knows` pairs (+ lookups)
/// at 4.0e3 (old 12.6), i.e. ~500x. The README only states the golden case. The inflated
/// groups already change the operators above them there (bind joins of 8-21 rows became
/// galloping merges over 200K-row scans; no slowdown was measured because the merges gallop),
/// and anything costed on top of a cycle (outer joins, sub-selects, GROUP BY) sees the same
/// factor.
#[test]
fn review_r2_cycle_closures_are_estimated_near_their_size_on_random_graphs() {
    let mut out = Vec::new();
    for n in [200usize, 5_000] {
        let mem = social(n);
        for q in [
            "SELECT * { ?x :knows ?y . ?y :knows ?z . ?z :knows ?x }",
            "SELECT * { ?a :knows ?b . ?b :knows ?a }",
        ] {
            let actual = oracle(&lower(q, &mem), &mem).unwrap().len() as f64;
            let est = top_join_est(&plan(q, &mem));
            let ratio = est / actual.max(1.0);
            if !(0.25..=4.0).contains(&ratio) {
                out.push(format!("{n} people: {q}: est {est:.1} for {actual} rows ({ratio:.1}x)"));
            }
        }
    }
    assert!(out.is_empty(), "cycle estimates off by more than 4x:\n{}", out.join("\n"));
}
