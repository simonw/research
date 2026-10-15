//! Adversarial review (round 3) of WP5. Each `review_*` test pins one finding; the ones that
//! fail document open defects (see the review report for the analysis).
mod common;

use common::*;
use sq_core::plan::{JoinPolicy, QueryOptions, QueryPlan};
use sq_memstore::MemStore;

fn policies() -> Vec<(&'static str, QueryOptions)> {
    vec![
        ("auto", QueryOptions::default()),
        ("prefer-bind", QueryOptions { join_policy: JoinPolicy::PreferBind, ..Default::default() }),
        ("prefer-hash", QueryOptions { join_policy: JoinPolicy::PreferHash, ..Default::default() }),
    ]
}

/// lower + optimize (which validates in debug builds) + validate, as text errors.
fn plan_result(q: &str, st: &MemStore, opts: &QueryOptions) -> Result<QueryPlan, String> {
    let parsed = sq_plan::parse_query(&format!("{PREFIXES}{q}"), None).map_err(|e| e.to_string())?;
    let lowered = sq_plan::lower_query(&parsed, st, opts).map_err(|e| e.to_string())?;
    let p = sq_plan::optimize(lowered, st, opts).map_err(|e| e.to_string())?;
    sq_plan::validate(&p).map_err(|e| format!("{e}\n{}", sq_plan::explain(&p, None)))?;
    Ok(p)
}

// ------------------------------------------------------------------------------------------
// Finding A: `GRAPH <c>` with c not a named graph of the dataset is rewritten to
// Logical::Empty, and a join group containing an Empty unit is planned as a bare `Op::Empty`.
// The parent operator (OPTIONAL, MINUS, an enclosing join, a UNION branch) still takes its
// keys / seeds from the LOGICAL props of that subtree (e.g. certain(Join(Bgp, Empty)) = the
// Bgp's slots), while the physical props of `Op::Empty` are all-empty. validate() then rejects
// the plan ("join key not certainly bound on both sides" / "BindJoin seed slot not certainly
// bound in the left rows"), so optimize() returns Error::Internal in every debug build and in
// every test for ordinary queries such as a GRAPH block on a misspelled / FROM NAMED-excluded
// graph next to an OPTIONAL. (Release builds skip validate; the Empty plans then return the
// right — empty — answer.)
// ------------------------------------------------------------------------------------------
#[test]
fn review_absent_named_graph_inside_join_operands_plans_and_validates() {
    let st = social(50);
    let mut failures = Vec::new();
    for q in [
        // left of an OPTIONAL
        "SELECT * { ?x :name ?y GRAPH :absent { ?x :age ?z } OPTIONAL { ?x :city ?w } }",
        // left of a MINUS
        "SELECT * { ?x :name ?y GRAPH :absent { ?x :age ?z } MINUS { ?x :city ?w } }",
        // one UNION branch
        "SELECT * { ?a :knows ?b { { ?b :city ?c } UNION { ?b :age ?c GRAPH :absent { } } } }",
        // a nested (filtered) group joined with the outer pattern
        "SELECT * { ?a :knows ?b { ?b :city ?c GRAPH :absent { ?c :p ?d } FILTER(?a != ?c) } }",
        // right side of an OPTIONAL (fails under prefer-hash)
        "SELECT * { ?a :knows ?b OPTIONAL { ?b :city ?c GRAPH :absent { ?c :p ?d } } }",
        // the graph exists in the store but the dataset excludes it
        "SELECT * FROM NAMED :g1 { ?x :name ?y GRAPH :g2 { ?x :age ?z } OPTIONAL { ?x :city ?w } }",
        // GRAPH ?g whose body is Empty
        "SELECT * { ?x :name ?y GRAPH ?x { GRAPH :absent { ?s ?p ?o } } OPTIONAL { ?x :city ?w } }",
        // a decorrelated [NOT] EXISTS whose pattern is Empty (fails under prefer-hash)
        "SELECT * { ?x :knows ?y FILTER NOT EXISTS { ?x :age ?a GRAPH :absent { ?a :p ?b } } }",
    ] {
        for (name, opts) in policies() {
            if let Err(e) = plan_result(q, &st, &opts) {
                failures.push(format!("[{name}] {q}\n    {}", e.lines().next().unwrap_or("")));
            }
        }
    }
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures.join("\n"));
}

// ------------------------------------------------------------------------------------------
// Finding B: estimates are not clamped. A cross product of many patterns multiplies row
// estimates until they overflow to +inf; validate() requires finite estimates, so optimize()
// returns Error::Internal("estimates must be finite") in debug builds (release builds carry
// inf / NaN costs into every later comparison). 80 disconnected `?a :knows ?b` patterns over a
// 21K-triple store are enough.
// ------------------------------------------------------------------------------------------
#[test]
fn review_large_cross_product_estimates_stay_finite() {
    let st = social(3000);
    let body: String = (0..80).map(|i| format!("?a{i} :knows ?b{i} . ")).collect();
    let q = format!("SELECT * {{ {body} }}");
    let p = plan_result(&q, &st, &QueryOptions::default()).unwrap_or_else(|e| panic!("{e}"));
    assert!(p.root.est_rows.is_finite() && p.root.est_cost_ns.is_finite());
}
