//! Targeted tests of the optimizer package (spec §7.3–§7.6): interesting orders and merge
//! joins with enforcers, join policies, SQL chains (where they are and are not used, where they
//! close), IntRange choice, COUNT fast paths, StarMerge, cardinality hooks, and `validate()`
//! rejecting corrupted plans that use the new operators.
mod common;

use std::rc::Rc;

use common::*;
use sq_core::ir::PatTerm;
use sq_core::plan::{IntRange, JoinKind, JoinPolicy, Op, Plan, QueryOptions, QueryPlan};
use sq_core::store::{IndexOrder, QuadStore};

fn with(policy: JoinPolicy) -> QueryOptions {
    QueryOptions { join_policy: policy, ..Default::default() }
}

fn count(p: &QueryPlan, pred: impl Fn(&Op) -> bool) -> usize {
    count_ops(p, pred)
}

fn is_join(o: &Op) -> bool {
    matches!(
        o,
        Op::BindJoin { .. } | Op::HashJoin { .. } | Op::MergeJoin { .. } | Op::NestedLoop { .. } | Op::StarMerge { .. }
    )
}

#[test]
fn prefer_merge_merges_every_keyed_join() {
    let st = social(100);
    for q in [
        "SELECT * { ?x a :C3 ; :age ?a ; :city ?c }",
        "SELECT * { ?a :knows ?b . ?b :knows ?c . ?c :city :city7 }",
        "SELECT * { ?x :knows ?y . ?y :knows ?z . ?z :knows ?x }",
        "SELECT * { { ?x :age ?a } UNION { ?x :city ?a } ?x :knows ?y }",
    ] {
        let p = plan_with(q, &st, &with(JoinPolicy::PreferMerge));
        let joins = count(&p, is_join);
        let merges = count(&p, |o| matches!(o, Op::MergeJoin { .. } | Op::StarMerge { .. }));
        assert!(joins > 0 && joins == merges, "{q}\n{}", sq_plan::explain(&p, None));
    }
    // Inputs that are not sorted by the key get SortBySlot enforcers (both sides if need be).
    let q = "SELECT * { { ?x :age ?a } UNION { ?x :city ?a } ?x :knows ?y }";
    let p = plan_with(q, &st, &with(JoinPolicy::PreferMerge));
    assert_eq!(count(&p, |o| matches!(o, Op::SortBySlot { .. })), 1, "{}", sq_plan::explain(&p, None));
}

#[test]
fn hash_and_bind_policies_never_merge() {
    let st = social(100);
    let q = "SELECT * { ?a :knows ?b . ?b :knows ?c }";
    for policy in [JoinPolicy::PreferHash, JoinPolicy::PreferBind] {
        let p = plan_with(q, &st, &with(policy));
        assert_eq!(count(&p, |o| matches!(o, Op::MergeJoin { .. } | Op::SortBySlot { .. })), 0, "{policy:?}");
    }
    // Auto merges the two knows streams on ?b without any enforcer: the left pattern is read
    // o-sorted (Gpos), the right one s-sorted (Gpso).
    let p = plan(q, &st);
    let merge = nodes(&p).into_iter().find(|n| matches!(n.op, Op::MergeJoin { .. })).expect("merge join");
    let Op::MergeJoin { left, right, key, check, gallop } = &merge.op else { unreachable!() };
    assert_eq!(*key, slot(&p, "b"));
    assert!(check.is_empty() && *gallop == 6);
    assert!(matches!(&left.op, Op::Scan(s) if s.order == IndexOrder::Gpos));
    assert!(matches!(&right.op, Op::Scan(s) if s.order == IndexOrder::Gpso));
}

#[test]
fn orders_are_claimed_only_where_the_executor_keeps_them() {
    let st = social(100);
    // A BindJoin keeps its left order when it does not re-sort batches by another slot; a
    // hash join keeps its probe order. validate() re-derives every claim.
    for policy in [JoinPolicy::Auto, JoinPolicy::PreferBind, JoinPolicy::PreferHash, JoinPolicy::PreferMerge] {
        let p = plan_with("SELECT * { ?x a :C3 ; :age ?a ; :knows ?y . ?y :city ?c }", &st, &with(policy));
        for n in nodes(&p) {
            if let Op::BindJoin { left, sort_left: true, seed, .. } = &n.op
                && n.sorted_by.is_some()
            {
                assert_eq!(seed.first().copied(), left.sorted_by, "{}", sq_plan::explain(&p, None));
            }
        }
    }
    // Inside EXISTS nothing is sorted (scans may re-pick orders per substitution).
    let p = plan("SELECT * { ?x a :C1 FILTER (EXISTS { ?x :knows ?y . ?y :knows ?z } || false) }", &st);
    for e in &p.exists {
        let mut sorted = 0;
        sq_plan_walk(e, &mut |n| sorted += usize::from(n.sorted_by.is_some()));
        assert_eq!(sorted, 0);
    }
}

fn sq_plan_walk(p: &Plan, f: &mut impl FnMut(&Plan)) {
    f(p);
    for c in p.children() {
        sq_plan_walk(c, f);
    }
}

#[test]
fn counts_use_the_fast_paths() {
    let st = social(100);
    // One scan: counted in the store (VDBE count on SQLite).
    let p = plan("SELECT (COUNT(*) AS ?n) { ?x :age ?a }", &st);
    assert!(sq_plan::explain(&p, None).contains("fast=scan-vdbe"), "{}", sq_plan::explain(&p, None));
    // Repeated variables and ranges post-check rows: no VDBE count.
    for q in ["SELECT (COUNT(*) AS ?n) { ?x :knows ?x }", "SELECT (COUNT(*) AS ?n) { ?x :age ?a FILTER(?a > 95) }"] {
        let p = plan(q, &st);
        assert!(!sq_plan::explain(&p, None).contains("fast="), "{q}\n{}", sq_plan::explain(&p, None));
    }
    // A chain is a factorized merge join under every policy that merges.
    for policy in [JoinPolicy::Auto, JoinPolicy::NoSql, JoinPolicy::PreferMerge] {
        let p = plan_with("SELECT (COUNT(*) AS ?n) { ?a :knows ?b . ?b :knows ?c }", &st, &with(policy));
        let factorized = |o: &Op| matches!(o, Op::MergeJoin { check, .. } if check.is_empty());
        let count_of_merge = count(&p, |o| matches!(o, Op::Count { input, .. } if factorized(&input.op)));
        assert_eq!(count_of_merge, 1, "{policy:?}\n{}", sq_plan::explain(&p, None));
    }
}

/// Turtle of a subject star with fan-out: `subjects` subjects, each with `fan` values of :p1,
/// :p2 and :p3.
fn fanout_ttl(subjects: usize, fan: usize) -> String {
    let mut ttl = String::from("@prefix : <http://ex/> .\n");
    for x in 0..subjects {
        for p in 1..=3 {
            for v in 0..fan {
                ttl.push_str(&format!(":x{x} :p{p} :v{} .\n", (x * 7 + v * 13 + p) % 997));
            }
        }
    }
    ttl
}

/// StarMerge (spec §7.5) is taken where it is cheaper than the merge chain: under COUNT(*) of a
/// star with fan-out, its leapfrog multiplies the runs per key while the chain produces every
/// intermediate row (measured 0.6 ms against 2.6 ms in release). Producing every row, with one
/// value per subject, or under PreferMerge (binary merge joins), the chain stays.
#[test]
fn star_merges_are_taken_where_cheaper_than_the_chain() {
    let counted = "SELECT (COUNT(*) AS ?n) { ?x :p1 ?a ; :p2 ?b ; :p3 ?c }";
    let all_rows = "SELECT * { ?x :p1 ?a ; :p2 ?b ; :p3 ?c }";
    let stars = |p: &QueryPlan| count(p, |o| matches!(o, Op::StarMerge { .. }));
    let ttl = fanout_ttl(300, 20);
    let mem = sq_memstore::MemStore::new();
    load_ttl(&mem, &ttl, None);
    mem.analyze();
    let db = Sqlite::new();
    db.load(&ttl, None);
    db.store.analyze().unwrap();
    let expected = oracle(&lower(counted, &mem), &mem).unwrap();
    let stores: [(&str, Rc<dyn QuadStore>); 2] = [("mem", Rc::new(mem.clone())), ("sqlite", Rc::new(db.store.clone()))];
    for (name, st) in stores {
        let p = plan_on(counted, &*st, &QueryOptions::default());
        let text = sq_plan::explain(&p, None);
        assert_eq!(stars(&p), 1, "[{name}]\n{text}");
        assert!(text.contains("fast=star-factorized"), "[{name}]\n{text}");
        assert_eq!(execute(Rc::clone(&st), p, &QueryOptions::default()).unwrap(), expected, "[{name}]");
        let p = plan_on(all_rows, &*st, &QueryOptions::default());
        assert_eq!(stars(&p), 0, "[{name}] 2.4M output rows\n{}", sq_plan::explain(&p, None));
        let p = plan_on(counted, &*st, &with(JoinPolicy::PreferMerge));
        assert_eq!(stars(&p), 0, "[{name}] PreferMerge\n{}", sq_plan::explain(&p, None));
        assert_eq!(count(&p, |o| matches!(o, Op::MergeJoin { .. })), 2, "[{name}]");
    }
    // One value per subject: a leapfrog step per key costs more than the few intermediate rows.
    let flat = store(fanout_ttl(3000, 1).strip_prefix("@prefix : <http://ex/> .\n").unwrap());
    flat.analyze();
    let p = plan(counted, &flat);
    assert_eq!(stars(&p), 0, "{}", sq_plan::explain(&p, None));
}

/// Galloping (spec §7.5 MergeJoin): only a right input read through one cursor can seek to the
/// next left key. A default graph merged from several graphs (FROM :g1 FROM :g2, the union
/// default graph) is read through several cursors and cannot seek, so its merge join pays for
/// the whole right input.
#[test]
fn merge_joins_gallop_only_through_one_cursor() {
    let ttl = social_ttl(2_000);
    let lines: Vec<&str> = ttl.lines().collect();
    let split = sq_memstore::MemStore::new();
    for (i, half) in lines.chunks(lines.len() / 2 + 1).enumerate() {
        load_ttl(&split, &format!("@prefix : <http://ex/> .\n{}", half.join("\n")), Some(&format!("http://ex/g{}", i + 1)));
    }
    split.analyze();
    let default = social(2_000);
    let fixed = QueryOptions { join_policy: JoinPolicy::PreferMerge, reorder: false, ..Default::default() };
    let union = QueryOptions { union_default_graph: true, ..fixed.clone() };
    for (from, st, opts, gallops) in [
        ("FROM :g1", &split, &fixed, true),
        ("FROM :g1 FROM :g2", &split, &fixed, false),
        ("", &default, &fixed, true),
        ("", &default, &union, false),
    ] {
        // The selective left side leaves most of the right one to skip.
        let q = format!("SELECT * {from} {{ ?x :city :city3 . ?x :knows ?y }}");
        let p = plan_with(&q, st, opts);
        let text = sq_plan::explain(&p, None);
        let merge = nodes(&p).into_iter().find(|n| matches!(n.op, Op::MergeJoin { .. })).expect("merge join");
        let Op::MergeJoin { left, right, .. } = &merge.op else { unreachable!() };
        assert!(matches!(&right.op, Op::Scan(_)), "{q}\n{text}");
        let skips = merge.est_cost_ns < left.est_cost_ns + right.est_cost_ns;
        assert_eq!(skips, gallops, "{q} (union flag {})\n{text}", opts.union_default_graph);
    }
}

/// Groups too large for the DP are ordered greedily from the unit with the fewest rows once its
/// filters apply, read through its cheapest access path: here the IntRange scan of `?x :age ?a
/// FILTER(?a > 98)`, as the DP picks for a smaller group.
#[test]
fn greedy_orders_start_from_the_cheapest_access_path() {
    let st = social(2_000);
    for n in [8, 12] {
        let knows: String = (0..n).map(|i| format!(" . ?x :knows ?k{i}")).collect();
        let q = format!("SELECT * {{ ?x :age ?a FILTER(?a > 98){knows} }}");
        let p = plan(&q, &st);
        let text = sq_plan::explain(&p, None);
        let ranges = count(&p, |o| matches!(o, Op::Scan(s) if s.range.is_some()));
        assert_eq!(ranges, 1, "{n} :knows patterns\n{text}");
        let mut first = &p.root;
        while let Some(c) = first.children().first() {
            first = c;
        }
        assert!(matches!(&first.op, Op::Scan(s) if s.range.is_some()), "{n} :knows patterns\n{text}");
    }
}

#[test]
fn groups_stream_over_input_sorted_by_their_key() {
    let st = social(100);
    // One pattern read s-sorted: the group by its subject streams.
    let q = "SELECT ?x (COUNT(*) AS ?n) { ?x :knows ?y } GROUP BY ?x";
    let p = plan(q, &st);
    assert_eq!(count(&p, |o| matches!(o, Op::Group { streaming: true, .. })), 1, "{}", sq_plan::explain(&p, None));
    let mem: Rc<dyn QuadStore> = Rc::new(st.clone());
    assert_eq!(execute(Rc::clone(&mem), p, &QueryOptions::default()).unwrap(), oracle(&lower(q, &st), &st).unwrap());
    // Grouping by a computed key or by two keys hashes.
    for q in [
        "SELECT ?k (COUNT(*) AS ?n) { ?x :knows ?y } GROUP BY (STR(?x) AS ?k)",
        "SELECT ?x ?y (COUNT(*) AS ?n) { ?x :knows ?y } GROUP BY ?x ?y",
    ] {
        let p = plan(q, &st);
        assert_eq!(count(&p, |o| matches!(o, Op::Group { streaming: true, .. })), 0, "{q}");
        assert_eq!(execute(Rc::clone(&mem), p, &QueryOptions::default()).unwrap(), oracle(&lower(q, &st), &st).unwrap());
    }
}

#[test]
fn int_range_scans_are_cost_based() {
    let st = social(200);
    let range_of = |p: &QueryPlan| {
        nodes(p).into_iter().find_map(|n| match &n.op {
            Op::Scan(s) => s.range,
            _ => None,
        })
    };
    // Selective: pushed down with exact integer bounds; the FILTER stays above the scan.
    for (q, lo, hi) in [
        ("SELECT * { ?x :age ?a FILTER(?a > 90.5) }", 91, sq_core::TermId::INT_MAX),
        ("SELECT * { ?x :age ?a FILTER(?a > 90) }", 91, sq_core::TermId::INT_MAX),
        ("SELECT * { ?x :age ?a FILTER(?a >= 90) }", 90, sq_core::TermId::INT_MAX),
        ("SELECT * { ?x :age ?a FILTER(?a < 10.5) }", sq_core::TermId::INT_MIN, 10),
        ("SELECT * { ?x :age ?a FILTER(?a <= -3) }", sq_core::TermId::INT_MIN, -3),
        ("SELECT * { ?x :age ?a FILTER(?a > 3 && ?a < 7) }", 4, 6),
        ("SELECT * { ?x :age ?a FILTER(95 < ?a) }", 96, sq_core::TermId::INT_MAX),
    ] {
        let p = plan(q, &st);
        assert_eq!(range_of(&p), Some(IntRange { lo, hi }), "{q}\n{}", sq_plan::explain(&p, None));
        assert_eq!(count(&p, |o| matches!(o, Op::Filter { .. })), 1, "{q}");
        let scan = nodes(&p).into_iter().find(|n| matches!(n.op, Op::Scan(_))).unwrap();
        assert!(matches!(&scan.op, Op::Scan(s) if s.order == IndexOrder::Gpos) && scan.sorted_by.is_none());
    }
    // Unselective (every age passes), a bound subject, a non-numeric constant: no range.
    for q in [
        "SELECT * { ?x :age ?a FILTER(?a > -10) }",
        "SELECT * { :p1 :age ?a FILTER(?a > 90) }",
        "SELECT * { ?x :age ?a FILTER(?a > :p1) }",
        "SELECT * { ?x :age ?a FILTER(?a > \"90\") }",
    ] {
        let p = plan(q, &st);
        assert_eq!(range_of(&p), None, "{q}\n{}", sq_plan::explain(&p, None));
    }
    // GRAPH ?g patterns read several graphs: no range.
    let q = "SELECT * { GRAPH ?g { ?x :age ?a FILTER(?a > 90) } }";
    assert_eq!(range_of(&plan(q, &st)), None);
}

#[test]
fn equality_selectivity_uses_distinct_values() {
    let st = social(200);
    // 200 ages over ~100 distinct values: `= 5` keeps about two rows.
    let p = plan("SELECT * { ?x :age ?a FILTER(?a = 5) }", &st);
    let f = nodes(&p).into_iter().find(|n| matches!(n.op, Op::Filter { .. })).unwrap();
    assert!(f.est_rows > 1.0 && f.est_rows < 4.0, "{}", sq_plan::explain(&p, None));
}

#[test]
fn sql_chains_only_where_sqlite_runs_them() {
    let db = sqlite_social(200);
    let star = "SELECT * { ?x a :C3 ; :age ?a ; :city ?c ; :name ?n }";
    let sql = |p: &QueryPlan| count(p, |o| matches!(o, Op::SqlJoin { .. }));
    assert_eq!(sql(&plan_on(star, &db.store, &QueryOptions::default())), 1);
    for policy in [JoinPolicy::NoSql, JoinPolicy::PreferBind, JoinPolicy::PreferHash, JoinPolicy::PreferMerge] {
        assert_eq!(sql(&plan_on(star, &db.store, &with(policy))), 0, "{policy:?}");
    }
    // Never on a MemStore, never inside EXISTS plans, never in seeded groups.
    assert_eq!(sql(&plan(star, &social(200))), 0);
    let q = "SELECT * { ?v :name \"Person 5\" \
             FILTER (EXISTS { ?v :knows ?y . ?y :knows ?z . ?z :city :city1 } || false) }";
    let p = plan_on(q, &db.store, &QueryOptions::default());
    assert!(p.exists.iter().all(|e| {
        let mut n = 0;
        sq_plan_walk(e, &mut |x| n += usize::from(matches!(x.op, Op::SqlJoin { .. })));
        n == 0
    }));
    let q = "SELECT * { ?v :name \"Person 5\" OPTIONAL { ?v :knows ?y . ?y :knows ?z . ?z :name ?n } }";
    let p = plan_on(q, &db.store, &QueryOptions::default());
    for n in nodes(&p) {
        if let Op::BindJoin { right, .. } = &n.op {
            let mut inner = 0;
            sq_plan_walk(right, &mut |x| inner += usize::from(matches!(x.op, Op::SqlJoin { .. })));
            assert_eq!(inner, 0, "{}", sq_plan::explain(&p, None));
        }
    }
}

#[test]
fn sql_chains_close_before_filters() {
    let db = sqlite_social(400);
    let q = "SELECT * { ?x a :C3 ; :age ?a ; :city ?c ; :name ?n FILTER(?a > 20) }";
    let p = plan_on(q, &db.store, &QueryOptions::default());
    // Whatever the shape, a filter is never inside a SqlJoin, and a SqlJoin covers whole
    // patterns: its output binds every slot of its aliases.
    for n in nodes(&p) {
        if let Op::SqlJoin { out, sql, params } = &n.op {
            assert!(out.len() >= 2 && !params.is_empty(), "{sql}");
            assert!(!sql.contains('>'), "{sql}");
        }
    }
    let text = sq_plan::explain(&p, None);
    assert_eq!(count(&p, |o| matches!(o, Op::Filter { .. })), 1, "{text}");
    // And the plan still answers like the oracle.
    let expected = oracle(&lower_on(q, &db.store, &QueryOptions::default()), &db.store).unwrap();
    let _read = db.store.read_scope(0).unwrap();
    let store: Rc<dyn QuadStore> = Rc::new(db.store.clone());
    assert_eq!(execute(store, p, &QueryOptions::default()).unwrap(), expected, "{text}");
}

/// SQLite prepares at most 64 tables per statement: a wider sqlable star (greedy-ordered, 71
/// units) fills one SqlJoin with exactly 64 aliases, joins the rest outside it, and answers
/// like the oracle; `validate()` rejects a 65-table statement.
#[test]
fn sql_chains_stop_at_sqlites_join_limit() {
    let mut ttl = String::from("@prefix : <http://ex/> .\n");
    for s in 0..40 {
        if s % 20 == 0 {
            ttl.push_str(&format!(":s{s} a :C .\n"));
        }
        for p in 0..70 {
            ttl.push_str(&format!(":s{s} :p{p} {} .\n", s * 100 + p));
        }
    }
    let db = Sqlite::new();
    db.load(&ttl, None);
    db.store.analyze().unwrap();
    let mut q = String::from("SELECT * { ?x a :C ");
    for p in 0..70 {
        q.push_str(&format!("; :p{p} ?o{p} "));
    }
    q.push('}');
    let expected = oracle(&lower_on(&q, &db.store, &QueryOptions::default()), &db.store).unwrap();
    assert_eq!(expected.len(), 2);
    let _read = db.store.read_scope(0).unwrap();
    let p = plan_on(&q, &db.store, &QueryOptions::default());
    let text = sq_plan::explain(&p, None);
    let aliases: Vec<usize> = nodes(&p)
        .iter()
        .filter_map(|n| match &n.op {
            Op::SqlJoin { sql, .. } => Some(sql.matches(" CROSS JOIN ").count() + 1),
            _ => None,
        })
        .collect();
    assert_eq!(aliases, vec![64], "{text}");
    // SQLite keeps our order and an index search for each of the 64 aliases.
    let sql = nodes(&p).iter().find_map(|n| match &n.op {
        Op::SqlJoin { sql, .. } => Some(sql.clone()),
        _ => None,
    });
    let eqp = db.store.explain_query_plan(&sql.unwrap()).unwrap();
    let lines: Vec<&str> = eqp.lines().map(str::trim).collect();
    assert_eq!(lines.len(), 64, "{eqp}");
    for (i, line) in lines.iter().enumerate() {
        assert!(line.starts_with(&format!("SEARCH t{i} USING ")), "{eqp}");
    }
    let store: Rc<dyn QuadStore> = Rc::new(db.store.clone());
    assert_eq!(execute(store, p.clone(), &QueryOptions::default()).unwrap(), expected, "{text}");

    rejects(p, "SqlJoin of 65 tables", |n| match &mut n.op {
        Op::SqlJoin { sql, .. } => {
            let extra = " CROSS JOIN main.rdf_quad AS t64 INDEXED BY rdf_gpso WHERE ";
            *sql = sql.replacen(" WHERE ", extra, 1);
            true
        }
        _ => false,
    });
}

/// A StarMerge built from a PreferMerge chain (the planner takes it only when its cost model
/// says so) validates and executes like the chain.
#[test]
fn star_merge_of_a_merge_chain_executes_like_the_chain() {
    let st = social(100);
    let q = "SELECT * { ?x a :C3 ; :age ?a ; :city ?c ; :name ?n }";
    let opts = with(JoinPolicy::PreferMerge);
    let chain = plan_with(q, &st, &opts);
    let expected = oracle(&lower(q, &st), &st).unwrap();
    let mem: Rc<dyn QuadStore> = Rc::new(st.clone());
    assert_eq!(execute(Rc::clone(&mem), chain.clone(), &opts).unwrap(), expected);

    fn flatten(p: Plan, key: u16, out: &mut Vec<Plan>) {
        match p.op {
            Op::MergeJoin { left, right, key: k, .. } if k == key => {
                flatten(*left, key, out);
                out.push(*right);
            }
            op => out.push(Plan { op, ..p }),
        }
    }
    let mut star = chain.clone();
    let Op::Project { input, .. } = &mut star.root.op else { panic!("project") };
    let Op::MergeJoin { key, .. } = input.op else { panic!("{}", sq_plan::explain(&chain, None)) };
    let mut inputs = Vec::new();
    let empty = Plan { id: 0, est_rows: 0.0, est_cost_ns: 0.0, sorted_by: None, op: Op::Empty };
    flatten(std::mem::replace(&mut **input, empty), key, &mut inputs);
    assert_eq!(inputs.len(), 4);
    let op = Op::StarMerge { key, inputs, check: vec![] };
    **input = Plan { id: 0, est_rows: 20.0, est_cost_ns: 1.0, sorted_by: Some(key), op };
    renumber(&mut star);
    sq_plan::validate(&star).unwrap();
    assert!(sq_plan::explain(&star, None).contains("StarMerge key=?x"));
    assert_eq!(execute(mem, star, &opts).unwrap(), expected);
}

/// Pre-order ids for a hand-edited plan.
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
    for e in &mut q.exists {
        go(e, &mut n);
    }
}

/// Apply `f` to the first node (pre-order) it accepts, then expect validate() to fail.
fn rejects(mut q: QueryPlan, what: &str, mut f: impl FnMut(&mut Plan) -> bool) {
    fn visit(p: &mut Plan, f: &mut impl FnMut(&mut Plan) -> bool) -> bool {
        if f(p) {
            return true;
        }
        match &mut p.op {
            Op::BindJoin { left, right, .. }
            | Op::MergeJoin { left, right, .. }
            | Op::NestedLoop { left, right, .. }
            | Op::Minus { left, right, .. } => visit(left, f) || visit(right, f),
            Op::HashJoin { build, probe, .. } => visit(build, f) || visit(probe, f),
            Op::StarMerge { inputs, .. } | Op::Union(inputs) => inputs.iter_mut().any(|i| visit(i, f)),
            Op::Filter { input, .. }
            | Op::Extend { input, .. }
            | Op::Count { input, .. }
            | Op::SortBySlot { input, .. }
            | Op::TopK { input, .. }
            | Op::Slice { input, .. }
            | Op::Project { input, .. } => visit(input, f),
            _ => false,
        }
    }
    assert!(visit(&mut q.root, &mut f), "{what}: nothing to mutate");
    renumber(&mut q);
    let err = sq_plan::validate(&q).expect_err(what);
    assert!(matches!(err, sq_core::Error::Internal(_)), "{what}: {err}");
}

#[test]
fn validate_rejects_corrupted_optimizer_plans() {
    let st = social(100);
    let chain = plan("SELECT * { ?a :knows ?b . ?b :knows ?c }", &st);
    let c = slot(&chain, "c");
    rejects(chain.clone(), "merge input not sorted", |p| match &mut p.op {
        Op::MergeJoin { right, .. } => {
            right.sorted_by = None;
            true
        }
        _ => false,
    });
    rejects(chain.clone(), "merge key bound on one side only", |p| match &mut p.op {
        Op::MergeJoin { key, left, right, .. } => {
            *key = c;
            left.sorted_by = Some(c);
            right.sorted_by = Some(c);
            true
        }
        _ => false,
    });
    rejects(chain.clone(), "zero gallop", |p| match &mut p.op {
        Op::MergeJoin { gallop, .. } => {
            *gallop = 0;
            true
        }
        _ => false,
    });
    rejects(chain.clone(), "sort enforcer on a slot the input never binds", |p| match &p.op {
        Op::Scan(s) if s.order == IndexOrder::Gpso => {
            let inner = p.clone();
            p.op = Op::SortBySlot { input: Box::new(inner), key: slot(&chain, "a") };
            true
        }
        _ => false,
    });

    let ranged = plan("SELECT * { ?x :age ?a FILTER(?a > 90) }", &st);
    rejects(ranged.clone(), "IntRange on another order", |p| match &mut p.op {
        Op::Scan(s) if s.range.is_some() => {
            s.order = IndexOrder::Gpso;
            true
        }
        _ => false,
    });
    rejects(ranged.clone(), "empty IntRange", |p| match &mut p.op {
        Op::Scan(s) if s.range.is_some() => {
            s.range = Some(IntRange { lo: 5, hi: 4 });
            true
        }
        _ => false,
    });
    rejects(ranged.clone(), "IntRange over a constant object", |p| match &mut p.op {
        Op::Scan(s) if s.range.is_some() => {
            s.pat.o = PatTerm::Const(sq_core::TermId::from_int(3).unwrap());
            true
        }
        _ => false,
    });
    rejects(ranged, "sorted IntRange scan", |p| match &mut p.op {
        Op::Scan(s) if s.range.is_some() => {
            p.sorted_by = Some(match s.pat.s {
                PatTerm::Slot(x) => x,
                PatTerm::Const(_) => unreachable!(),
            });
            true
        }
        _ => false,
    });

    // A hash join keeps the probe order, never the build order.
    let hashed = plan_with("SELECT * { ?a :knows ?b . ?b :age ?c }", &st, &with(JoinPolicy::PreferHash));
    rejects(hashed, "hash join claiming its build order", |p| match &p.op {
        Op::HashJoin { build, .. } if build.sorted_by.is_some() => {
            p.sorted_by = build.sorted_by;
            true
        }
        _ => false,
    });
    // A BindJoin that re-sorts batches by a seed tuple loses the left order.
    let bound = plan_with("SELECT * { ?x a :C1 . ?x :knows ?y . ?y :age ?a }", &st, &with(JoinPolicy::PreferBind));
    rejects(bound, "re-sorted bind join claiming the left order", |p| match &mut p.op {
        Op::BindJoin { left, seed, sort_left, .. }
            if left.sorted_by.is_some() && seed.first() != left.sorted_by.as_ref() =>
        {
            *sort_left = true;
            p.sorted_by = left.sorted_by;
            true
        }
        _ => false,
    });

    let db = sqlite_social(200);
    let star = plan_on("SELECT * { ?x a :C3 ; :age ?a ; :city ?c }", &db.store, &QueryOptions::default());
    rejects(star.clone(), "SqlJoin out slots", |p| match &mut p.op {
        Op::SqlJoin { out, .. } => {
            out.pop();
            true
        }
        _ => false,
    });
    rejects(star.clone(), "SqlJoin parameters", |p| match &mut p.op {
        Op::SqlJoin { params, .. } => {
            params.push(0);
            true
        }
        _ => false,
    });
    rejects(star, "SqlJoin text", |p| match &mut p.op {
        Op::SqlJoin { sql, .. } => {
            *sql = sql.replace("CROSS JOIN", "JOIN");
            true
        }
        _ => false,
    });
}

#[test]
fn left_joins_keep_left_order_for_merges_above() {
    // OPTIONAL output keeps the left input's order (hash join probing the left side, or a
    // bind join without re-sorting): validate() re-derives it, and the rows stay correct.
    let st = social(60);
    let q = "SELECT * { ?x a :C1 . ?x :name ?n OPTIONAL { ?x :knows ?y } ?x :age ?a }";
    for policy in [JoinPolicy::Auto, JoinPolicy::PreferMerge, JoinPolicy::PreferHash] {
        let p = plan_with(q, &st, &with(policy));
        let expected = oracle(&lower(q, &st), &st).unwrap();
        let mem: Rc<dyn QuadStore> = Rc::new(st.clone());
        assert_eq!(execute(mem, p, &with(policy)).unwrap(), expected, "{policy:?}");
    }
    let p = plan_with("SELECT * { ?x a :C1 OPTIONAL { ?x :knows ?y } }", &st, &with(JoinPolicy::PreferHash));
    let is_left_join = |o: &Op| matches!(o, Op::HashJoin { kind: JoinKind::Left { .. }, .. });
    let left_join = nodes(&p).into_iter().find(|n| is_left_join(&n.op)).unwrap();
    assert_eq!(left_join.sorted_by, Some(slot(&p, "x")), "{}", sq_plan::explain(&p, None));
}
