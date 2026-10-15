//! Targeted planner tests: GRAPH push / loop, filter placement and scope, OPTIONAL filters,
//! seeds, TopK / Count / ASK, join policies and EXISTS plans.
mod common;

use common::*;
use sq_core::ir::{GraphSel, PatTerm, QueryForm};
use sq_core::plan::{JoinKind, JoinPolicy, Op, Plan, QueryOptions, QueryPlan};

fn is_scan_with_graph(op: &Op, pred: impl Fn(GraphSel) -> bool) -> bool {
    matches!(op, Op::Scan(s) if pred(s.pat.g))
}

const GRAPHS: &str = ":a :p :o . :b :p :o .";

fn named_store() -> sq_memstore::MemStore {
    let s = store(":x :p :o .");
    load_ttl(&s, &format!("@prefix : <http://ex/> .\n{GRAPHS}"), Some("http://ex/g1"));
    load_ttl(&s, "@prefix : <http://ex/> .\n:c :p :o .", Some("http://ex/g2"));
    s
}

#[test]
fn graph_minus_is_evaluated_per_graph() {
    let st = named_store();
    // Shared ?a: the MINUS stays, inside a GraphLoop reading the active graph.
    let p = plan("SELECT ?a { GRAPH ?g { ?a :p :o MINUS { ?a :q :o } } }", &st);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::GraphLoop { .. })), 1);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::Minus { .. })), 1);
    assert_eq!(count_ops(&p, |o| is_scan_with_graph(o, |g| g == GraphSel::Active)), 2);
    // The W3C graph-minus shape: no shared variable, so the MINUS removes nothing per graph
    // (pushing ?g into both sides would wrongly share it).
    let p = plan("SELECT ?a { GRAPH ?g { ?a :p :o MINUS { ?b :p :o } } }", &st);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::GraphLoop { .. })), 1);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::Minus { .. })), 0);
}

#[test]
fn graph_var_with_values_bind_or_subselect_loops() {
    let st = named_store();
    for q in [
        "SELECT * { GRAPH ?g { VALUES ?x { 1 2 } } }",
        "SELECT * { GRAPH ?g { VALUES ?g { :g1 } } }",
        "SELECT * { GRAPH ?g { BIND(1 AS ?x) } }",
        "SELECT * { GRAPH ?g { ?s :p ?o FILTER(?g != :g1) } }",
        "SELECT * { GRAPH ?g { SELECT ?s { ?s :p ?o } LIMIT 1 } }",
        "SELECT * { GRAPH ?g { ?s :p ?o FILTER EXISTS { ?s :q ?z } } }",
        "SELECT * { GRAPH ?g { { ?s :p ?o } UNION { VALUES ?s { :a } } } }",
    ] {
        let p = plan(q, &st);
        assert_eq!(count_ops(&p, |o| matches!(o, Op::GraphLoop { .. })), 1, "{q}");
        assert_eq!(count_ops(&p, |o| is_scan_with_graph(o, |g| matches!(g, GraphSel::Slot(_)))), 0, "{q}");
    }
}

#[test]
fn graph_var_patterns_are_pushed() {
    let st = named_store();
    for q in [
        "SELECT * { GRAPH ?g { ?s :p ?o } }",
        "SELECT * { GRAPH ?g { ?s :p ?o . ?o :q ?z FILTER(?z > 1) } }",
        "SELECT * { GRAPH ?g { ?s :p ?o OPTIONAL { ?o :q ?z } } }",
        "SELECT * { GRAPH ?g { { ?s :p ?o } UNION { ?s :q ?o } } }",
        "SELECT * { GRAPH ?g { ?s :p ?o BIND(STR(?o) AS ?t) } }",
    ] {
        let p = plan(q, &st);
        let g = slot(&p, "g");
        assert_eq!(count_ops(&p, |o| matches!(o, Op::GraphLoop { .. })), 0, "{q}");
        assert_eq!(count_ops(&p, |o| is_scan_with_graph(o, |x| x == GraphSel::Active)), 0, "{q}");
        assert!(count_ops(&p, |o| is_scan_with_graph(o, |x| x == GraphSel::Slot(g))) >= 1, "{q}");
    }
}

#[test]
fn graph_const_membership() {
    let st = named_store();
    let p = plan("SELECT * { GRAPH <http://ex/absent> {} }", &st);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::Empty)), 1);
    let p = plan("SELECT * { GRAPH <http://ex/absent> { ?s ?p ?o } }", &st);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::Scan(_))), 0);
    let p = plan("SELECT * { GRAPH :g1 {} }", &st);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::Empty)), 0);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::Unit)), 1);
    let p = plan("SELECT * { GRAPH :g1 { ?s :p ?o } }", &st);
    assert_eq!(count_ops(&p, |o| is_scan_with_graph(o, |g| matches!(g, GraphSel::Const(_)))), 1);
    // FROM NAMED restricts the named graphs: g2 is not part of this dataset.
    let p = plan("SELECT * FROM NAMED :g1 { GRAPH :g2 { ?s :p ?o } }", &st);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::Scan(_))), 0);
    assert_eq!(p.named_graphs.len(), 1);
}

/// Operators of the root plan only (EXISTS plans excluded).
fn root_ops(p: &QueryPlan, pred: impl Fn(&Op) -> bool + Copy) -> usize {
    fn walk(p: &Plan, pred: impl Fn(&Op) -> bool + Copy) -> usize {
        usize::from(pred(&p.op)) + p.children().into_iter().map(|c| walk(c, pred)).sum::<usize>()
    }
    walk(&p.root, pred)
}

#[test]
fn absent_graphs_fold_into_their_ancestors() {
    let st = named_store();
    let empty_root = |q: &str| {
        for opts in [
            QueryOptions::default(),
            QueryOptions { join_policy: JoinPolicy::PreferHash, ..Default::default() },
            QueryOptions { join_policy: JoinPolicy::PreferBind, ..Default::default() },
        ] {
            let p = plan_with(q, &st, &opts);
            assert!(matches!(p.root.op, Op::Empty), "{q}\n{}", sq_plan::explain(&p, None));
        }
    };
    // Join / OPTIONAL-left / MINUS-left / filtered groups / GRAPH ?g / keyed groups over Empty.
    empty_root("SELECT * { ?x :p ?y GRAPH :absent { ?x :q ?z } OPTIONAL { ?x :r ?w } }");
    empty_root("SELECT * { ?x :p ?y GRAPH :absent { ?x :q ?z } MINUS { ?x :r ?w } }");
    empty_root("SELECT ?x { ?x :p ?y { ?y :q ?z GRAPH :absent { ?z :r ?w } FILTER(?x != ?z) } } ORDER BY ?x LIMIT 3");
    empty_root("SELECT * { ?x :p ?y GRAPH ?x { GRAPH :absent { ?s ?p ?o } } }");
    empty_root("SELECT ?x (COUNT(*) AS ?c) { GRAPH :absent { ?x ?p ?o } } GROUP BY ?x");
    // FROM NAMED excludes :g2 although the store has it.
    empty_root("SELECT * FROM NAMED :g1 { ?x :p ?y GRAPH :g2 { ?x :p ?z } }");
    // EXISTS { Empty } is false.
    empty_root("SELECT * { ?x :p ?y FILTER EXISTS { ?y :p ?z GRAPH :absent { ?z :q ?w } } }");

    // The implicit group still yields its row: COUNT over nothing.
    let p = plan("SELECT (COUNT(*) AS ?c) { GRAPH :absent { ?s ?p ?o } }", &st);
    let count_of_empty = |o: &Op| matches!(o, Op::Count { input, .. } if matches!(input.op, Op::Empty));
    assert_eq!(root_ops(&p, count_of_empty), 1, "{}", sq_plan::explain(&p, None));
    // OPTIONAL / MINUS / UNION with an Empty side keep the other side only.
    for q in [
        "SELECT * { ?x :p ?y OPTIONAL { ?y :q ?z GRAPH :absent { ?z :r ?w } } }",
        "SELECT * { ?x :p ?y MINUS { GRAPH :absent { ?x :p ?y } } }",
        "SELECT * { { ?x :p ?y } UNION { GRAPH :absent { ?x :p ?y } } }",
        // NOT EXISTS { Empty } is true: the conjunct goes, the other one stays.
        "SELECT * { ?x :p ?y FILTER(NOT EXISTS { GRAPH :absent { ?x :p ?y } } && ?x != :a) }",
    ] {
        let p = plan(q, &st);
        let scans = root_ops(&p, |o| matches!(o, Op::Scan(_)));
        let others = root_ops(&p, |o| {
            matches!(o, Op::Empty | Op::Union(_) | Op::Minus { .. })
                || matches!(o, Op::BindJoin { .. } | Op::HashJoin { .. } | Op::NestedLoop { .. })
        });
        assert_eq!((scans, others), (1, 0), "{q}\n{}", sq_plan::explain(&p, None));
    }
    let p = plan("SELECT * { ?x :p ?y FILTER(NOT EXISTS { GRAPH :absent { ?x :p ?y } } && ?x != :a) }", &st);
    let filter = nodes(&p).into_iter().find_map(|n| match &n.op {
        Op::Filter { expr, .. } => Some(expr.clone()),
        _ => None,
    });
    assert!(filter.is_some_and(|e| !e.contains_exists()), "{}", sq_plan::explain(&p, None));
}

#[test]
fn optional_binding_the_graph_var_directly_is_evaluated_per_graph() {
    let st = named_store();
    // Per graph G, a right row binding ?g to an object v != G removes the left row (the merged
    // row fails ?g = G): pushing ?g into the patterns would keep it.
    let p = plan("SELECT * { GRAPH ?g { ?s :p ?o OPTIONAL { ?o :q ?g } } }", &st);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::GraphLoop { .. })), 1, "{}", sq_plan::explain(&p, None));
    let p = plan("SELECT * { GRAPH ?g { ?s :p ?o OPTIONAL { GRAPH ?g { ?o :q ?z } } } }", &st);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::GraphLoop { .. })), 1, "{}", sq_plan::explain(&p, None));
    // When the left side certainly binds ?g the same way, the join compares it: pushed.
    let p = plan("SELECT * { GRAPH ?g { ?g :p ?o OPTIONAL { ?o :q ?g } } }", &st);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::GraphLoop { .. })), 0, "{}", sq_plan::explain(&p, None));
}

#[test]
fn empty_graph_var_lists_graph_names() {
    let st = named_store();
    let p = plan("SELECT ?g { GRAPH ?g {} }", &st);
    let g = slot(&p, "g");
    assert_eq!(count_ops(&p, |o| matches!(o, Op::GraphNames { slot } if *slot == g)), 1);
    assert_eq!(p.named_graphs.len(), 2);
}

/// The Filter node whose expression mentions `needle`.
fn filter_node<'a>(p: &'a sq_core::plan::QueryPlan, pred: impl Fn(&sq_core::ir::Expr) -> bool) -> &'a Plan {
    nodes(p)
        .into_iter()
        .find(|n| matches!(&n.op, Op::Filter { expr, .. } if pred(expr)))
        .expect("filter present")
}

fn subtree_has(p: &Plan, pred: &impl Fn(&Op) -> bool) -> bool {
    pred(&p.op) || p.children().into_iter().any(|c| subtree_has(c, pred))
}

#[test]
fn not_bound_stays_above_its_optional() {
    let st = store(":a :p :b . :b :q :c .");
    let p = plan("SELECT * { ?x :p ?y OPTIONAL { ?y :q ?z } FILTER(!bound(?z)) }", &st);
    let f = filter_node(&p, |e| matches!(e, sq_core::ir::Expr::Not(_)));
    let Op::Filter { input, .. } = &f.op else { unreachable!() };
    assert!(subtree_has(input, &|o| matches!(
        o,
        Op::BindJoin { kind: JoinKind::Left { .. }, .. }
            | Op::HashJoin { kind: JoinKind::Left { .. }, .. }
            | Op::NestedLoop { kind: JoinKind::Left { .. }, .. }
    )));
}

#[test]
fn filters_keep_their_group_scope() {
    let st = store(":x :p 1 . :x :q 2 .");
    // ?v is unbound inside the inner group: the filter must stay with the :q pattern only.
    let p = plan("SELECT * { :x :p ?v . { :x :q ?w FILTER(?v = 1) } }", &st);
    let v = slot(&p, "v");
    let f = filter_node(&p, |e| sq_plan_test_mentions(e, v));
    let Op::Filter { input, .. } = &f.op else { unreachable!() };
    assert!(!subtree_has(input, &|o| matches!(o, Op::Scan(s) if s.pat.o == PatTerm::Slot(v))));
    // A group filter is applied right after the pattern that binds its variables.
    let fixed = QueryOptions { reorder: false, ..Default::default() };
    let p = plan_with("SELECT * { ?s :p ?v . ?s :q ?w FILTER(?v > 0) }", &st, &fixed);
    let v = slot(&p, "v");
    let f = filter_node(&p, |e| sq_plan_test_mentions(e, v));
    let Op::Filter { input, .. } = &f.op else { unreachable!() };
    assert!(matches!(&input.op, Op::Scan(s) if s.pat.o == PatTerm::Slot(v)), "{}", sq_plan::explain(&p, None));
}

fn sq_plan_test_mentions(e: &sq_core::ir::Expr, s: u16) -> bool {
    let mut v = Vec::new();
    e.slots(&mut v);
    v.contains(&s)
}

#[test]
fn optional_filter_splits_between_right_side_and_join() {
    let st = store(":a :p :b . :b :q 5 .");
    let p = plan("SELECT * { ?x :p ?y OPTIONAL { ?y :q ?z FILTER(?z > 3 && ?x != ?z) } }", &st);
    let (x, z) = (slot(&p, "x"), slot(&p, "z"));
    let join = nodes(&p)
        .into_iter()
        .find_map(|n| match &n.op {
            Op::BindJoin { kind: JoinKind::Left { filter }, right, .. }
            | Op::HashJoin { kind: JoinKind::Left { filter }, build: right, .. }
            | Op::NestedLoop { kind: JoinKind::Left { filter }, right, .. } => Some((filter.clone(), right)),
            _ => None,
        })
        .expect("outer join");
    let filter = join.0.expect("join keeps the conjunct reading the left side");
    assert!(sq_plan_test_mentions(&filter, x) && sq_plan_test_mentions(&filter, z));
    assert!(subtree_has(join.1, &|o| matches!(o, Op::Filter { expr, .. } if sq_plan_test_mentions(expr, z) && !sq_plan_test_mentions(expr, x))));
}

#[test]
fn same_term_and_iri_equality_become_seeds() {
    let st = store(":a :p :b . :c :p :d .");
    for q in ["SELECT * { ?s :p ?o FILTER(?o = :b) }", "SELECT * { ?s :p ?o FILTER(sameTerm(:b, ?o)) }"] {
        let p = plan(q, &st);
        assert_eq!(count_ops(&p, |o| matches!(o, Op::Filter { .. })), 0, "{q}");
        assert_eq!(count_ops(&p, |o| matches!(o, Op::Values { rows, .. } if rows.len() == 1)), 1, "{q}");
        assert_eq!(count_ops(&p, |o| matches!(o, Op::BindJoin { .. })), 1, "{q}");
    }
    // Numeric equality is value equality: stays a filter.
    let p = plan("SELECT * { ?s :p ?o FILTER(?o = 1) }", &st);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::Filter { .. })), 1);
}

#[test]
fn topk_count_and_ask() {
    let st = store(":a :p 1 . :b :p 2 .");
    let p = plan("SELECT ?s { ?s :p ?o } ORDER BY ?o LIMIT 1", &st);
    assert!(matches!(&p.root.op, Op::Project { input, .. } if matches!(input.op, Op::TopK { k: 1, .. })));
    let p = plan("SELECT ?s { ?s :p ?o } ORDER BY ?o LIMIT 2 OFFSET 3", &st);
    assert!(matches!(&p.root.op, Op::Slice { offset: 3, limit: Some(2), input } if matches!(input.op, Op::Project { .. })));
    assert_eq!(count_ops(&p, |o| matches!(o, Op::TopK { k: 5, .. })), 1);
    // Never across DISTINCT.
    let p = plan("SELECT DISTINCT ?s { ?s :p ?o } ORDER BY ?o LIMIT 1", &st);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::TopK { .. })), 0);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::Sort { .. })), 1);
    let p = plan("SELECT (COUNT(*) AS ?n) { ?s :p ?o }", &st);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::Count { .. })), 1);
    let p = plan("SELECT (COUNT(DISTINCT *) AS ?n) { ?s :p ?o }", &st);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::Count { .. })), 0);
    let p = plan("ASK { ?s :p ?o }", &st);
    assert!(matches!(p.root.op, Op::Slice { offset: 0, limit: Some(1), .. }));
    assert_eq!(p.form, QueryForm::Ask);
}

#[test]
fn join_policies_and_syntactic_order() {
    let st = social(60);
    let q = "SELECT * { ?x a :C3 . ?x :knows ?y . ?y :age ?a }";
    let hash = plan_with(q, &st, &QueryOptions { join_policy: JoinPolicy::PreferHash, ..Default::default() });
    assert_eq!(count_ops(&hash, |o| matches!(o, Op::BindJoin { .. })), 0);
    assert_eq!(count_ops(&hash, |o| matches!(o, Op::HashJoin { .. })), 2);
    let bind = plan_with(q, &st, &QueryOptions { join_policy: JoinPolicy::PreferBind, ..Default::default() });
    assert_eq!(count_ops(&bind, |o| matches!(o, Op::BindJoin { .. })), 2);
    // reorder = false: left-deep in syntactic order (deepest left leaf = first pattern).
    let q2 = "SELECT * { ?y :age ?a . ?x :knows ?y . ?x a :C3 }";
    let fixed = plan_with(q2, &st, &QueryOptions { reorder: false, ..Default::default() });
    let Op::Scan(s) = &first_unit(&fixed.root).op else { panic!("first unit is a scan") };
    assert_eq!(s.pat.o, PatTerm::Slot(slot(&fixed, "a")), "{}", sq_plan::explain(&fixed, None));
    // The selective class pattern drives the reordered plan.
    let auto = plan(q2, &st);
    let Op::Scan(s) = &first_unit(&auto.root).op else { panic!("first unit is a scan") };
    assert!(matches!(s.pat.o, PatTerm::Const(_)), "{}", sq_plan::explain(&auto, None));
}

/// The first unit of a left-deep join pipeline (the probe / left input all the way down).
fn first_unit(p: &Plan) -> &Plan {
    match &p.op {
        Op::BindJoin { left, .. } | Op::NestedLoop { left, .. } | Op::MergeJoin { left, .. } | Op::Minus { left, .. } => {
            first_unit(left)
        }
        Op::HashJoin { build, probe, build_is_left, .. } => first_unit(if *build_is_left { build } else { probe }),
        Op::Project { input, .. } | Op::Filter { input, .. } | Op::Extend { input, .. } => first_unit(input),
        _ => p,
    }
}

#[test]
fn optional_with_unsafe_seed_uses_hash_join() {
    let st = social(40);
    // A LIMIT is not commutable with a seed filter: ?x may not be injected into the sub-select.
    let p = plan("SELECT * { ?x a :C1 OPTIONAL { SELECT ?x ?y { ?x :knows ?y } LIMIT 1 } }", &st);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::BindJoin { .. })), 0);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::HashJoin { kind: JoinKind::Left { .. }, build_is_left: false, .. })), 1);
    // No shared slot at all: nested loop.
    let p = plan("SELECT * { ?x a :C1 OPTIONAL { ?y :knows ?z FILTER(?y != ?x) } }", &st);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::NestedLoop { kind: JoinKind::Left { filter: Some(_) }, .. })), 1);
}

#[test]
fn exists_plans_are_planned_with_outer_bindings() {
    let st = social(40);
    // (Not a whole conjunct, so it stays correlated.)
    let p = plan("SELECT * { ?x a :C1 FILTER (EXISTS { ?x :knows ?y . ?y a :C2 } || false) }", &st);
    assert_eq!(p.exists.len(), 1);
    let x = slot(&p, "x");
    // ?x is bound when the EXISTS runs: the knows scan starts from it.
    let starts_from_x = nodes(&p).into_iter().any(|n| match &n.op {
        Op::Scan(s) => s.pat.s == PatTerm::Slot(x) && s.order == sq_core::store::IndexOrder::Gspo,
        _ => false,
    });
    assert!(starts_from_x, "{}", sq_plan::explain(&p, None));
    assert!(p.exists.iter().all(|e| e.sorted_by.is_none()));
}

#[test]
fn canonical_flags() {
    let st = store(":a :p 1 .");
    let p = plan("SELECT ?z { ?s :p ?o BIND(?o + 1 AS ?z) }", &st);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::Extend { canonical: false, .. })), 1);
    // Every consumer that compares ids forces canonical ids for ?z.
    for q in [
        "SELECT DISTINCT ?z { ?s :p ?o BIND(?o + 1 AS ?z) }",
        "SELECT ?z { ?s :p ?o BIND(?o + 1 AS ?z) ?t :p ?z }",
        "SELECT ?z (COUNT(*) AS ?n) { ?s :p ?o BIND(?o + 1 AS ?z) } GROUP BY ?z",
        "SELECT ?w { ?s :p ?o BIND(?o + 1 AS ?z) BIND(?z AS ?w) ?x :p ?w }",
        "SELECT ?z { ?s :p ?o BIND(?o + 1 AS ?z) FILTER(sameTerm(?z, 2)) }",
        "SELECT ?z { ?s :p ?o BIND(?o + 1 AS ?z) FILTER NOT EXISTS { ?z :p ?o } }",
        "CONSTRUCT { ?s :q ?z } WHERE { ?s :p ?o BIND(?o + 1 AS ?z) }",
    ] {
        let p = plan(q, &st);
        let z = slot(&p, "z");
        assert_eq!(count_ops(&p, |o| matches!(o, Op::Extend { slot, canonical: false, .. } if *slot == z)), 0, "{q}");
    }
}

#[test]
fn service() {
    let st = store(":a :p 1 .");
    let p = plan("SELECT * { ?s :p ?o SERVICE SILENT <http://remote/> { ?s :q ?x } }", &st);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::Unit)), 1);
    let lowered = lower("SELECT * { SERVICE <http://remote/> { ?s :q ?x } }", &st);
    let err = sq_plan::optimize(lowered, &st, &QueryOptions::default()).unwrap_err();
    assert!(matches!(err, sq_core::Error::Unsupported(_)));
}

#[test]
fn exists_decorrelation_follows_the_exact_conditions() {
    let st = social(40);
    let hash = QueryOptions { join_policy: JoinPolicy::PreferHash, ..Default::default() };
    let bind = QueryOptions { join_policy: JoinPolicy::PreferBind, ..Default::default() };
    let q = "SELECT * { ?x a :C1 FILTER NOT EXISTS { ?x :knows ?y } }";
    let p = plan_with(q, &st, &hash);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::HashJoin { kind: JoinKind::Anti, build_is_left: false, .. })), 1);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::Filter { .. })), 0);
    let p = plan_with(q, &st, &bind);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::BindJoin { kind: JoinKind::Anti, .. })), 1);
    let p = plan_with("SELECT * { ?x a :C1 FILTER EXISTS { ?x :knows ?y } }", &st, &hash);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::HashJoin { kind: JoinKind::Semi, .. })), 1);
    // Stays correlated: zero-length path endpoint, unsafe slot, uncorrelated, not a whole
    // conjunct, nested inside another EXISTS.
    for q in [
        "SELECT * { ?x a :C1 FILTER EXISTS { ?x :knows* ?y } }",
        "SELECT * { ?x a :C1 FILTER EXISTS { ?y :knows ?z OPTIONAL { ?z :knows ?x } } }",
        "SELECT * { ?x a :C1 FILTER EXISTS { ?y :knows ?z } }",
        "SELECT * { ?x a :C1 FILTER (EXISTS { ?x :knows ?y } || ?x = :p1) }",
    ] {
        let p = plan_with(q, &st, &hash);
        assert_eq!(count_ops(&p, |o| matches!(o, Op::HashJoin { kind: JoinKind::Semi | JoinKind::Anti, .. })), 0, "{q}");
        assert_eq!(count_ops(&p, |o| matches!(o, Op::Filter { .. })), 1, "{q}");
    }
    // Once the outer EXISTS is a join, its pattern is an ordinary subtree: the inner NOT EXISTS
    // may be decorrelated too.
    let q = "SELECT * { ?x a :C1 FILTER EXISTS { ?x :knows ?y FILTER NOT EXISTS { ?y :age 3 } } }";
    let p = plan_with(q, &st, &hash);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::HashJoin { kind: JoinKind::Semi, .. })), 1);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::HashJoin { kind: JoinKind::Anti, .. })), 1);
    // Inside a correlated EXISTS plan nothing is decorrelated.
    let q = "SELECT * { ?x a :C1 FILTER (EXISTS { ?x :knows ?y FILTER NOT EXISTS { ?y :age 3 } } || false) }";
    let p = plan_with(q, &st, &hash);
    assert_eq!(count_ops(&p, |o| matches!(o, Op::HashJoin { kind: JoinKind::Semi | JoinKind::Anti, .. })), 0);
    // A MINUS whose right side mentions a correlated slot: substitution reaches it (and drops
    // the slot from the domain test), a join does not, so the EXISTS stays correlated under
    // every policy; a MINUS on other slots does not matter.
    let semi = |p: &QueryPlan| {
        count_ops(p, |o| {
            matches!(
                o,
                Op::HashJoin { kind: JoinKind::Semi | JoinKind::Anti, .. }
                    | Op::BindJoin { kind: JoinKind::Semi | JoinKind::Anti, .. }
                    | Op::NestedLoop { kind: JoinKind::Semi | JoinKind::Anti, .. }
            )
        })
    };
    for opts in [&hash, &bind] {
        for q in [
            "SELECT * { ?x a :C1 FILTER NOT EXISTS { ?x :knows ?y MINUS { ?x :age ?a } } }",
            "SELECT * { ?x a :C1 FILTER EXISTS { ?x :knows ?y \
               { ?x :city ?c MINUS { ?x :age ?a } } UNION { ?y :age ?b } } }",
        ] {
            assert_eq!(semi(&plan_with(q, &st, opts)), 0, "{q}");
        }
        let q = "SELECT * { ?x a :C1 FILTER EXISTS { ?x :knows ?y MINUS { ?y :age ?a } } }";
        assert_eq!(semi(&plan_with(q, &st, opts)), 1, "{q}");
    }
}
