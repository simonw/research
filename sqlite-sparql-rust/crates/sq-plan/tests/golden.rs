//! Golden EXPLAIN snapshots on a deterministic 200-person social graph with statistics
//! (MemStore cost constants; the SqlJoin star runs on an in-memory SQLite store). They pin the
//! optimizer's shapes: a star driven by its class pattern through (g, p, s) probes (a SqlJoin
//! on SQLite), a chain started from its selective end and probed from there (its 31 middle
//! keys are too sparse in the 598 `knows` rows for a galloping merge to pay off), OPTIONAL,
//! ORDER BY + LIMIT as a TopK, COUNT(*) of a chain as a factorized MergeJoin of the o-sorted and
//! s-sorted streams, a FILTER range as an IntRange scan, and a cyclic join whose hash join keeps
//! its probe order for the closing merge (on ?x, whose runs are short, checking ?z; both keys
//! are entity keys, so the estimate multiplies their selectivities: 26.7 rows for the 26
//! triangles of this random graph). Update them deliberately when the cost model changes.
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
#0 Project [?x ?a ?c ?n]                            est=20 cost=11.7us
#1  BindJoin seed=[?x] adaptive                     est=20 cost=11.7us
#2   BindJoin seed=[?x] adaptive                    est=20 cost=8.0us
#3    BindJoin seed=[?x] adaptive                   est=20 cost=4.4us
#4     Scan Gpos ?x rdf:type <http://ex/C3> (sorted ?x) est=20 cost=740ns
#5     Scan Gpso ?x <http://ex/age> ?a (sorted ?a)  est=1 cost=322ns
#6    Scan Gpso ?x <http://ex/city> ?c (sorted ?c)  est=1 cost=322ns
#7   Scan Gpso ?x <http://ex/name> ?n (sorted ?n)   est=1 cost=322ns
",
    );
}

#[test]
fn star_on_sqlite_is_one_sql_join() {
    let db = sqlite_social(200);
    let q = "SELECT ?x ?a ?c ?n { ?x a :C3 ; :age ?a ; :city ?c ; :name ?n }";
    let p = plan_on(q, &db.store, &Default::default());
    let got = sq_plan::explain_with(&p, None, Some(&db.store));
    let expected = "\
#0 Project [?x ?a ?c ?n]                            est=20 cost=51.8us
#1  SqlJoin t0:Gpos(p=rdf:type,o=<http://ex/C3>) t1:Gpso(p=<http://ex/age>,s=t0.s) \
t2:Gpso(p=<http://ex/city>,s=t0.s) t3:Gpso(p=<http://ex/name>,s=t0.s) -> [?x ?a ?c ?n] est=20 cost=51.8us
";
    assert_eq!(got, expected, "\n--- got:\n{got}");
}

#[test]
fn chain_from_selective_end() {
    check(
        "SELECT ?a ?c { ?a :knows ?b . ?b :knows ?c . ?c :city :city7 }",
        "\
#0 Project [?a ?c]                                  est=97 cost=14.9us
#1  BindJoin seed=[?b] adaptive                     est=97 cost=14.9us
#2   BindJoin seed=[?c] adaptive                    est=31.1 cost=3.1us
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
#0 Project [?x ?y]                                  est=20 cost=10.6us
#1  HashJoin keys=[?x] build=right left             est=20 cost=10.6us
#2   BindJoin seed=[?y] adaptive                    est=62.3 cost=5.6us
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
#0 Project [?n]                                     est=1 cost=29.3us
#1  Extend ?n := ?_3 (output-only)                  est=1 cost=29.3us
#2   Count -> ?_3 fast=merge-factorized             est=1 cost=29.3us
#3    MergeJoin key=?b gallop=6                     est=1.8e3 cost=41.2us
#4     Scan Gpos ?a <http://ex/knows> ?b (sorted ?b) est=598 cost=14.7us
#5     Scan Gpso ?b <http://ex/knows> ?c (sorted ?b) est=598 cost=14.7us
",
    );
}

#[test]
fn range_filter_is_an_int_range_scan() {
    check(
        "SELECT ?x ?age { ?x :age ?age FILTER(?age > 90) }",
        "\
#0 Project [?x ?age]                                est=18 cost=1.5us
#1  Filter (?age > 90)                              est=18 cost=1.5us
#2   Scan Gpos ?x <http://ex/age> ?age range=[91..] est=18 cost=1.0us
",
    );
}

#[test]
fn unselective_cycle_hashes_in_probe_order_then_merges() {
    check(
        "SELECT * { ?x :knows ?y . ?y :knows ?z . ?z :knows ?x }",
        "\
#0 Project [?x ?y ?z]                               est=26.7 cost=123.4us
#1  MergeJoin key=?x gallop=6 check=[?z]            est=26.7 cost=123.4us
#2   HashJoin keys=[?y] build=right                 est=1.8e3 cost=85.5us
#3    Scan Gpso ?y <http://ex/knows> ?z (sorted ?y) est=598 cost=14.7us
#4    Scan Gpso ?x <http://ex/knows> ?y (sorted ?x) est=598 cost=14.7us
#5   Scan Gpos ?z <http://ex/knows> ?x (sorted ?x)  est=598 cost=14.7us
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
