//! Golden EXPLAIN snapshots on a deterministic 200-person social graph with statistics
//! (MemStore cost constants). They pin the correctness-first planner's shapes: a star driven by
//! its class pattern, a chain started from its selective end, OPTIONAL, ORDER BY + LIMIT as a
//! TopK and COUNT(*) as a Count. Update them deliberately when the cost model changes.
mod common;

use common::*;

fn explain(q: &str) -> String {
    let st = social(200);
    let p = plan(q, &st);
    sq_plan::explain_with(&p, None, Some(&st))
}

fn check(q: &str, expected: &str) {
    let got = explain(q);
    assert_eq!(got, expected, "\n{q}\n--- got:\n{got}");
}

#[test]
fn star() {
    check(
        "SELECT ?x ?a ?c ?n { ?x a :C3 ; :age ?a ; :city ?c ; :name ?n }",
        "\
#0 Project [?x ?a ?c ?n]                            est=20 cost=20.3us
#1  BindJoin seed=[?x] adaptive                     est=20 cost=20.3us
#2   BindJoin seed=[?x] adaptive                    est=20 cost=13.8us
#3    BindJoin seed=[?x] adaptive                   est=20 cost=7.3us
#4     Scan Gpos ?x rdf:type <http://ex/C3> (sorted ?x) est=20 cost=740ns
#5     Scan Gspo ?x <http://ex/age> ?a (sorted ?a)  est=1 cost=322ns
#6    Scan Gspo ?x <http://ex/city> ?c (sorted ?c)  est=1 cost=322ns
#7   Scan Gspo ?x <http://ex/name> ?n (sorted ?n)   est=1 cost=322ns
",
    );
}

#[test]
fn chain_from_selective_end() {
    check(
        "SELECT ?a ?c { ?a :knows ?b . ?b :knows ?c . ?c :city :city7 }",
        "\
#0 Project [?a ?c]                                  est=97 cost=16.1us
#1  BindJoin seed=[?b] adaptive                     est=97 cost=16.1us
#2   BindJoin seed=[?c] adaptive                    est=31.1 cost=4.3us
#3    Scan Gpos ?c <http://ex/city> <http://ex/city7> (sorted ?c) est=10 cost=520ns
#4    Scan Gpos ?b <http://ex/knows> ?c (sorted ?b) est=3.1 cost=369ns
#5   Scan Gpos ?a <http://ex/knows> ?b (sorted ?a)  est=3.1 cost=369ns
",
    );
}

#[test]
fn optional() {
    check(
        "SELECT ?x ?y { ?x a :C3 OPTIONAL { ?x :knows ?y . ?y a :C4 } }",
        "\
#0 Project [?x ?y]                                  est=20 cost=13.3us
#1  HashJoin keys=[?x] build=right left             est=20 cost=13.3us
#2   BindJoin seed=[?y] adaptive                    est=62.3 cost=8.2us
#3    Scan Gpos ?y rdf:type <http://ex/C4> (sorted ?y) est=20 cost=740ns
#4    Scan Gpos ?x <http://ex/knows> ?y (sorted ?x) est=3.1 cost=369ns
#5   Scan Gpos ?x rdf:type <http://ex/C3> (sorted ?x) est=20 cost=740ns
",
    );
}

#[test]
fn order_by_limit_is_topk() {
    check(
        "SELECT ?x ?age { ?x :age ?age } ORDER BY DESC(?age) ?x LIMIT 10",
        "\
#0 Project [?x ?age]                                est=10 cost=17.1us
#1  TopK k=10 [DESC(?age) ?x]                       est=10 cost=17.1us
#2   Scan Gpso ?x <http://ex/age> ?age (sorted ?x)  est=200 cost=5.1us
",
    );
}

#[test]
fn count_star() {
    check(
        "SELECT (COUNT(*) AS ?n) { ?a :knows ?b . ?b :knows ?c }",
        "\
#0 Project [?n]                                     est=1 cost=160.6us
#1  Extend ?n := ?_3 (output-only)                  est=1 cost=160.6us
#2   Count -> ?_3                                   est=1 cost=160.6us
#3    HashJoin keys=[?b] build=right                est=1.8e3 cost=89.1us
#4     Scan Gpso ?b <http://ex/knows> ?c (sorted ?b) est=598 cost=14.7us
#5     Scan Gpso ?a <http://ex/knows> ?b (sorted ?a) est=598 cost=14.7us
",
    );
}

#[test]
fn analyze_mode_adds_counters() {
    let st = social(20);
    let p = plan("SELECT * { ?x a :C3 }", &st);
    let profile = sq_core::plan::ExecProfile {
        nodes: vec![
            sq_core::plan::NodeProfile { rows_out: 2, opens: 1, seeks: 0, time_ns: 1_500_000 },
            sq_core::plan::NodeProfile { rows_out: 2, opens: 1, seeks: 1, time_ns: 1_000_000 },
        ],
        ..Default::default()
    };
    let text = sq_plan::explain(&p, Some(&profile));
    assert!(text.contains("rows=2 opens=1 seeks=1 ms=1.000"), "{text}");
    assert!(text.lines().all(|l| l.starts_with('#')), "{text}");
}
