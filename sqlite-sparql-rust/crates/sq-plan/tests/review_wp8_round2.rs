//! Adversarial review of WP8, round 2: StarMerge selection, plan legality corner cases and
//! end-to-end checks of shapes the random workloads rarely reach.
mod common;

use std::rc::Rc;

use common::*;
use sq_core::plan::{JoinPolicy, Op, Plan, QueryOptions, QueryPlan};
use sq_core::store::QuadStore;

fn with(policy: JoinPolicy) -> QueryOptions {
    QueryOptions { join_policy: policy, ..Default::default() }
}

/// Pre-order ids for a hand-edited plan (root only).
fn renumber(q: &mut QueryPlan) {
    fn go(p: &mut Plan, next: &mut u32) {
        p.id = *next;
        *next += 1;
        match &mut p.op {
            Op::BindJoin { left, right, .. }
            | Op::MergeJoin { left, right, .. }
            | Op::NestedLoop { left, right, .. }
            | Op::Minus { left, right, .. } => {
                go(left, next);
                go(right, next);
            }
            Op::HashJoin { build, probe, .. } => {
                go(build, next);
                go(probe, next);
            }
            Op::StarMerge { inputs, .. } | Op::Union(inputs) => inputs.iter_mut().for_each(|i| go(i, next)),
            Op::Filter { input, .. }
            | Op::Extend { input, .. }
            | Op::GraphLoop { input, .. }
            | Op::Group { input, .. }
            | Op::Count { input, .. }
            | Op::Sort { input, .. }
            | Op::SortBySlot { input, .. }
            | Op::TopK { input, .. }
            | Op::Distinct { input, .. }
            | Op::Reduced { input, .. }
            | Op::Slice { input, .. }
            | Op::Project { input, .. } => go(input, next),
            _ => {}
        }
    }
    let mut n = 0;
    go(&mut q.root, &mut n);
}

/// `q` with its first MergeJoin chain on one key (along the left spine below Project / Extend /
/// Count / Filter) replaced by the equivalent StarMerge, exactly as `physical::star` builds it.
fn as_star_merge(q: &QueryPlan) -> QueryPlan {
    fn flatten(p: Plan, key: u16, out: &mut Vec<Plan>, check: &mut Vec<u16>) {
        match p.op {
            Op::MergeJoin { left, right, key: k, check: c, .. } if k == key => {
                flatten(*left, key, out, check);
                for s in c {
                    if !check.contains(&s) {
                        check.push(s);
                    }
                }
                out.push(*right);
            }
            op => out.push(Plan { op, ..p }),
        }
    }
    fn go(p: &mut Plan) -> bool {
        if let Op::MergeJoin { key, .. } = p.op {
            let empty = Plan { id: 0, est_rows: 0.0, est_cost_ns: 0.0, sorted_by: None, op: Op::Empty };
            let old = std::mem::replace(p, empty);
            let (est_rows, est_cost_ns) = (old.est_rows, old.est_cost_ns);
            let (mut inputs, mut check) = (Vec::new(), Vec::new());
            flatten(old, key, &mut inputs, &mut check);
            check.sort();
            let op = Op::StarMerge { key, inputs, check };
            *p = Plan { id: 0, est_rows, est_cost_ns, sorted_by: Some(key), op };
            return true;
        }
        match &mut p.op {
            Op::Filter { input, .. } | Op::Extend { input, .. } | Op::Count { input, .. } | Op::Project { input, .. } => {
                go(input)
            }
            _ => false,
        }
    }
    let mut star = q.clone();
    assert!(go(&mut star.root), "no merge chain in\n{}", sq_plan::explain(q, None));
    renumber(&mut star);
    sq_plan::validate(&star).expect("the StarMerge plan validates");
    star
}

/// Wall-clock milliseconds of the fastest of `n` executions of `p`, and its row count.
fn exec_ms(st: &Rc<dyn QuadStore>, p: &QueryPlan, n: usize) -> (f64, usize) {
    let mut best = f64::INFINITY;
    let mut rows = 0;
    for _ in 0..n {
        let t = std::time::Instant::now();
        rows = execute(Rc::clone(st), p.clone(), &QueryOptions::default()).expect("executes").len();
        best = best.min(t.elapsed().as_secs_f64() * 1e3);
    }
    (best, rows)
}

/// A subject star with fan-out: 300 subjects, each with 20 values of :p1, :p2 and :p3.
fn fanout_star() -> sq_memstore::MemStore {
    let mut ttl = String::new();
    for x in 0..300 {
        for p in 1..=3 {
            for v in 0..20 {
                ttl.push_str(&format!(":x{x} :p{p} :v{} .\n", (x * 7 + v * 13 + p) % 997));
            }
        }
    }
    let st = store(&ttl);
    st.analyze();
    st
}

/// StarMerge (spec §7.5: k ≥ 3 inputs s-sorted on one slot, taken when cheaper than chained
/// MergeJoins) is never chosen in practice: `star::star_cost` charges a 300 ns leapfrog step for
/// every input per driver ROW (`inputs.len() × driver × STAR_STEP_NS`) and a sorted seek per
/// driver row, although a leapfrog positions the inputs once per distinct KEY of the driver
/// and then scans the runs; and the choice ignores that under COUNT(*) a StarMerge counts by
/// run products without producing the chain's intermediate rows. For this COUNT(*) of a
/// 3-pattern star (300 subjects × 20 values per predicate: 6 000 rows per input, 120 000
/// intermediate rows, 2.4M results) the planner keeps `Count{MergeJoin(MergeJoin(..), ..)}`,
/// which materializes the 120 000 intermediate rows; the StarMerge plan returns the same count
/// 3-5x faster (measured in the test profile: chain 5.3-6.2 ms, StarMerge 1.1-2.0 ms). The random
/// differential workload never plans a StarMerge either (`differential.rs` tally:
/// `star_merges: 0` over 16 000 plans).
#[test]
fn review_star_merge_is_chosen_where_the_leapfrog_wins() {
    let st = fanout_star();
    let q = "SELECT (COUNT(*) AS ?n) { ?x :p1 ?a ; :p2 ?b ; :p3 ?c }";
    let auto = plan(q, &st);
    let text = sq_plan::explain(&auto, None);
    // The StarMerge rewrite of the planner's own merge chain is legal and answers alike.
    let mem: Rc<dyn QuadStore> = Rc::new(st.clone());
    let star = as_star_merge(&plan_with(q, &st, &with(JoinPolicy::PreferMerge)));
    let expected = oracle(&lower(q, &st), &st).expect("oracle");
    assert_eq!(execute(Rc::clone(&mem), star.clone(), &QueryOptions::default()).expect("star"), expected);
    let (chain_ms, _) = exec_ms(&mem, &auto, 3);
    let (star_ms, _) = exec_ms(&mem, &star, 3);
    eprintln!("planned: {chain_ms:.2} ms, StarMerge: {star_ms:.2} ms\n{text}");
    assert_eq!(
        count_ops(&auto, |o| matches!(o, Op::StarMerge { .. })),
        1,
        "COUNT(*) of a 3-pattern star with fan-out 20 keeps the merge chain ({chain_ms:.2} ms) \
         instead of a StarMerge ({star_ms:.2} ms):\n{text}"
    );
}

/// Cardinality under the union default graph (§7.4, `Card::distinct_values`): a pattern of the
/// default graph is estimated standalone over the store default graph PLUS the named graphs, but
/// the distinct values that turn that estimate into a per-probe one (BindJoin seeds, merge
/// galloping, `=` selectivity, `slot_dv`) are read from the named-graph statistics only
/// (`GraphPart::AllNamed`, `stats.scope[1]`). On a store whose triples are in the default
/// graph, that scope is empty: every dv is 1, so a seeded probe `?x :knows ?y` (with ?x bound)
/// is estimated at all 6 000 :knows triples instead of 3. The dataset below has no named graph
/// at all, so the union default graph IS the default graph, yet the estimates explode
/// (`?x :age 50 . ?x :knows ?y . ?y :name ?n`: 57 rows without the flag, 2.3e8 rows with it)
/// and the plan changes to a HashJoin over all :knows triples. On 20 000 persons (606 rows vs
/// an estimated 2.4e11 rows / 727 s) that plan is measured 10x slower than the BindJoin chain
/// in release (SQLite 12.5 ms vs 1.3 ms, MemStore 7.8 ms vs 0.6 ms).
#[test]
fn review_union_default_graph_estimates_use_default_graph_statistics() {
    let st = social(2000);
    let union = QueryOptions { union_default_graph: true, ..Default::default() };
    let mut failures = Vec::new();
    for q in [
        "SELECT * { ?x a :C3 . ?x :knows ?y }",
        "SELECT * { ?x :age 50 . ?x :knows ?y . ?y :name ?n }",
        "SELECT * { ?x :city :city3 . ?x :knows ?y }",
    ] {
        let plain = plan(q, &st);
        let merged = plan_with(q, &st, &union);
        let (a, b) = (plain.root.est_rows, merged.root.est_rows);
        if b > 2.0 * a.max(1.0) {
            failures.push(format!(
                "{q}: {a:.3e} rows without union_default_graph, {b:.3e} with it\n{}{}",
                sq_plan::explain(&plain, None),
                sq_plan::explain(&merged, None)
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Per-graph statistics scale (§7.4, `Card::scope`): a fixed named graph is estimated as
/// `scope[1] / named_count`, the named-graph statistics divided by the number of named graphs
/// of the QUERY's dataset (`QueryPlan.named_graphs`), not of the store. Under `FROM <g>` without
/// a matching FROM NAMED that count is 0 (clamped to 1) or the number of FROM NAMED graphs, so
/// one graph is estimated as ALL the store's named graphs: here 10 named graphs of 200 :age
/// triples each, `FROM :g1 { ?x :age ?a }` is estimated at 2 000 rows (200 real) while
/// `GRAPH :g1 { ?x :age ?a }` over the same triples is estimated at 200. The error grows with the
/// number of named graphs in the store and skews every join order and operator choice of
/// FROM queries.
#[test]
fn review_from_graph_estimates_scale_by_the_stores_named_graphs() {
    let ttl = social_ttl(2_000);
    let lines: Vec<&str> = ttl.lines().collect();
    let st = sq_memstore::MemStore::new();
    for (i, chunk) in lines.chunks(lines.len() / 10 + 1).enumerate() {
        load_ttl(&st, &format!("@prefix : <http://ex/> .\n{}", chunk.join("\n")), Some(&format!("http://ex/g{i}")));
    }
    st.analyze();
    let rows = |q: &str| {
        let p = plan(q, &st);
        let scan = nodes(&p).into_iter().find(|n| matches!(n.op, Op::Scan(_))).expect("scan").est_rows;
        let real = execute(Rc::new(st.clone()), p, &QueryOptions::default()).expect("executes").len();
        (scan, real)
    };
    let (graph_est, real) = rows("SELECT * { GRAPH :g1 { ?x :age ?a } }");
    assert_eq!(real, 200);
    assert!((graph_est - 200.0).abs() < 50.0, "GRAPH :g1 estimate {graph_est}");
    for q in ["SELECT * FROM :g1 { ?x :age ?a }", "SELECT * FROM :g1 FROM NAMED :g2 { ?x :age ?a }"] {
        let (est, real) = rows(q);
        assert_eq!(real, 200);
        assert!(est <= 2.0 * graph_est, "{q}: estimated {est} rows, {real} real (GRAPH :g1: {graph_est})");
    }
}
