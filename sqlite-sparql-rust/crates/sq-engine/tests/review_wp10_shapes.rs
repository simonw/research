//! Adversarial review (WP10, round 2): hand-written differential cases in shapes the
//! fuzzer's grammar never produces (EXISTS outside FILTER: in BIND, projections, ORDER BY,
//! aggregates and HAVING; nested EXISTS; BNODE() inside OPTIONAL / sub-selects / GROUP BY
//! keys), compared with sq-oracle under every configuration and store.
#[allow(dead_code, unused_imports)]
mod fuzz;

use sq_core::plan::JoinPolicy;

use fuzz::compare::{same, Free, Outcome};
use fuzz::data::Dataset;
use fuzz::engines::{native, oracle, oracle_options, options, Config, StoreKind, Stores};
use fuzz::query::Order;

const POLICIES: [JoinPolicy; 5] =
    [JoinPolicy::Auto, JoinPolicy::PreferHash, JoinPolicy::PreferBind, JoinPolicy::PreferMerge, JoinPolicy::NoSql];

fn mismatches(stores: &Stores, query: &str, order: Order) -> Vec<String> {
    let text = format!("{}{query}", fuzz::query::PREFIXES);
    let parsed = sq_plan::parse_query(&text, None).expect("query parses");
    let oopts = oracle_options(false);
    let mut out = Vec::new();
    for kind in StoreKind::ALL {
        let truth = stores.with(kind, |s| oracle(&*s, &parsed, &oopts));
        if let Outcome::Error(e) = &truth {
            out.push(format!("oracle error on {kind:?}: {e}"));
            continue;
        }
        for cfg in Config::all(&POLICIES).into_iter().filter(|c| c.store == kind) {
            for probes in [false, true] {
                let got = stores.with(kind, |s| native(s, &parsed, &options(&cfg, false, probes)));
                if let Err(d) = same(&truth, &got, order, Free::Minted) {
                    out.push(format!(
                        "{cfg:?} probes={probes}: {d}\n  oracle: {}  native: {}",
                        truth.describe(order, 10),
                        got.describe(order, 10)
                    ));
                }
            }
        }
    }
    out
}

const DATA: &str = "\
:s0 :p0 :s1 .
:s1 :p0 :s2 .
:s1 :p1 \"a\" .
:s2 :p1 1 .
:s3 :p1 2 .
:s3 :p0 :s0 .
_:x0 :p0 :s1 .
_:x0 :p1 \"01\"^^xsd:integer .
:g0 { :s1 :p0 :s2 }
:g0 { :s2 :p2 :s0 }
:g1 { :s0 :p1 \"b\" }
:g1 { :s3 :p0 :s1 }
";

fn check(queries: &[&str], order: Order) {
    let st = Stores::load(&Dataset::from_trig(DATA, false));
    let bad: Vec<String> =
        queries.iter().flat_map(|q| mismatches(&st, q, order).into_iter().map(move |m| format!("{q}\n  {m}"))).collect();
    assert!(bad.is_empty(), "{} mismatches:\n{}", bad.len(), bad.join("\n"));
}

#[test]
fn review_exists_outside_filter_agrees_with_the_oracle() {
    check(
        &[
            "SELECT ?a ?e WHERE { ?a :p0 ?b BIND(EXISTS { ?b :p0 ?c } AS ?e) }",
            "SELECT ?a ?e WHERE { ?a :p0 ?b BIND(NOT EXISTS { ?b :p1 ?v FILTER(isNumeric(?v)) } AS ?e) }",
            "SELECT ?a (EXISTS { ?a :p1 ?v } AS ?e) WHERE { ?a :p0 ?b }",
            "SELECT ?a (IF(EXISTS { ?b :p0 ?c }, ?b, ?a) AS ?x) WHERE { ?a :p0 ?b }",
            "SELECT ?a (COALESCE(IF(EXISTS { ?a :p1 1 }, 1, ?nope), 0) AS ?x) WHERE { ?a :p0 ?b }",
            "SELECT (COUNT(*) AS ?n) (SUM(IF(EXISTS { ?b :p0 ?c }, 1, 0)) AS ?k) WHERE { ?a :p0 ?b }",
            "SELECT ?b (COUNT(?a) AS ?n) WHERE { ?a :p0 ?b } GROUP BY ?b HAVING (EXISTS { ?b :p1 ?v })",
            "SELECT ?e (COUNT(*) AS ?n) WHERE { ?a :p0 ?b } GROUP BY (EXISTS { ?b :p0 ?c } AS ?e)",
            "SELECT ?a ?b WHERE { ?a :p0 ?b BIND(EXISTS { GRAPH ?g { ?a ?p ?o } } AS ?e) FILTER(?e) }",
            "SELECT ?a WHERE { ?a :p0 ?b FILTER EXISTS { ?b :p0 ?c FILTER NOT EXISTS { ?c :p1 ?a } } }",
            "SELECT ?a WHERE { ?a :p0 ?b FILTER EXISTS { ?b :p0 ?c MINUS { ?c :p1 ?v } } }",
            "SELECT ?a WHERE { ?a :p0 ?b FILTER NOT EXISTS { ?b ?p ?c OPTIONAL { ?c :p1 ?v } FILTER(!BOUND(?v)) } }",
            "SELECT ?a ?g WHERE { GRAPH ?g { ?a :p0 ?b } FILTER EXISTS { GRAPH ?g { ?b ?p ?o } } }",
            "SELECT ?a ?b WHERE { ?a :p0 ?b OPTIONAL { ?b :p1 ?v FILTER(EXISTS { ?a :p0 ?b }) } }",
            "SELECT ?a WHERE { ?a :p0 ?b FILTER EXISTS { { SELECT ?b (COUNT(*) AS ?n) WHERE { ?b ?p ?o } GROUP BY ?b } FILTER(?n > 1) } }",
        ],
        Order::Bag,
    );
}

#[test]
fn review_exists_in_order_by_agrees_with_the_oracle() {
    check(
        &[
            "SELECT ?a ?b WHERE { ?a :p0 ?b } ORDER BY DESC(EXISTS { ?b :p0 ?c }) ?a ?b",
            "SELECT ?a ?b WHERE { ?a :p0 ?b } ORDER BY (EXISTS { ?a :p1 ?v }) ?b ?a LIMIT 2",
        ],
        Order::Sequence,
    );
}

#[test]
fn review_minted_blank_nodes_in_nested_patterns_agree_with_the_oracle() {
    check(
        &[
            "SELECT * WHERE { ?a :p0 ?b OPTIONAL { ?b :p0 ?c BIND(BNODE() AS ?n) } }",
            "SELECT * WHERE { ?a :p0 ?b { SELECT ?b (BNODE() AS ?n) WHERE { ?b :p1 ?v } } }",
            "SELECT ?n (COUNT(*) AS ?c) WHERE { ?a :p0 ?b BIND(BNODE() AS ?n) } GROUP BY ?n",
            "SELECT DISTINCT ?b ?n WHERE { ?a :p0 ?b BIND(BNODE(STR(?b)) AS ?n) }",
            "SELECT * WHERE { ?a :p0 ?b { BIND(BNODE() AS ?n) } UNION { BIND(BNODE() AS ?m) } }",
            "SELECT * WHERE { ?a :p0 ?b GRAPH ?g { BIND(BNODE() AS ?n) } }",
            "SELECT * WHERE { ?a :p0 ?b MINUS { BIND(BNODE() AS ?b) } }",
            "SELECT * WHERE { { BIND(BNODE() AS ?n) } ?a :p0 ?b OPTIONAL { ?b :p1 ?v } }",
            "SELECT * WHERE { ?a :p0 ?b OPTIONAL { BIND(BNODE() AS ?n) FILTER(isBlank(?n) && ?a != :s0) } }",
            "SELECT ?b (SAMPLE(?n) AS ?s) WHERE { ?a :p0 ?b { SELECT (BNODE() AS ?n) WHERE {} } } GROUP BY ?b",
        ],
        Order::Bag,
    );
}

#[test]
fn review_minted_blank_nodes_in_construct_agree_with_the_oracle() {
    check(
        &[
            "CONSTRUCT { ?a :q _:t . _:t :r ?n } WHERE { ?a :p0 ?b BIND(BNODE() AS ?n) }",
            "CONSTRUCT { ?n :q ?b } WHERE { ?a :p0 ?b { BIND(BNODE() AS ?n) } }",
            "CONSTRUCT { ?n :q ?a } WHERE { ?a :p0 ?b BIND(BNODE(STR(?b)) AS ?n) }",
        ],
        Order::Bag,
    );
}
