//! Unit tests of the physical planner internals (ordering, cross products, contexts).
use sq_core::batch::SlotSet;
use sq_core::id::Tag;
use sq_core::ir::{DatasetSpec, DefaultGraph, GraphSel, Logical, LoweredQuery, NamedGraphs, PatTerm, QuadPat, QueryForm, VarOrigin, VarTable};
use sq_core::plan::{Op, Plan, QueryOptions, QueryPlan};
use sq_core::store::QuadStoreWriter;
use sq_core::TermId;
use sq_memstore::MemStore;

use super::*;
use crate::rewrite;

fn iri(n: i64) -> TermId {
    TermId::from_dict(3000 + n, Tag::Iri)
}

fn pat(s: u16, p: i64, o: u16) -> QuadPat {
    QuadPat { g: GraphSel::Default, s: PatTerm::Slot(s), p: PatTerm::Const(iri(p)), o: PatTerm::Slot(o) }
}

fn query(root: Logical, width: usize) -> LoweredQuery {
    LoweredQuery {
        form: QueryForm::Select((0..width as u16).collect()),
        root,
        vars: VarTable { vars: (0..width).map(|i| (format!("v{i}"), VarOrigin::User)).collect() },
        dataset: DatasetSpec { default: DefaultGraph::StoreDefault, named: NamedGraphs::All },
        exists: Vec::new(),
        temps: Default::default(),
        base_iri: None,
    }
}

fn plan_of(q: &LoweredQuery, store: &MemStore, opts: &QueryOptions) -> QueryPlan {
    let named = named_graphs(q, store).unwrap();
    let rw = rewrite::rewrite(q.root.clone(), q.exists.clone(), &named);
    let p = plan_query(q, rw, named, store, opts).unwrap();
    crate::validate(&p).unwrap();
    p
}

fn scans(p: &Plan, out: &mut Vec<QuadPat>) {
    if let Op::Scan(s) = &p.op {
        out.push(s.pat);
    }
    for c in p.children() {
        scans(c, out);
    }
}

fn chain_store(n: i64) -> MemStore {
    let st = MemStore::new();
    let mut quads = Vec::new();
    for p in 0..n {
        for k in 0..(p + 1) * 3 {
            quads.push([TermId::DEFAULT_GRAPH, iri(1000 + k), iri(p), iri(1000 + k + 1)]);
        }
    }
    st.insert(&mut quads).unwrap();
    st
}

#[test]
fn context_helpers() {
    let ctx = Ctx { seeded: SlotSet::from_slots([1]), subst: SlotSet::from_slots([2]), in_exists: true };
    assert_eq!(ctx.bound(), SlotSet::from_slots([1, 2]));
    assert_eq!(ctx.unseeded(), Ctx { seeded: SlotSet::EMPTY, ..ctx });
}

#[test]
fn large_groups_are_ordered_greedily_and_completely() {
    // A 16-pattern chain: beyond the DP limit, every pattern still appears exactly once.
    let st = chain_store(16);
    let root = Logical::Bgp((0..16).map(|i| pat(i as u16, i, i as u16 + 1)).collect());
    let p = plan_of(&query(root.clone(), 17), &st, &QueryOptions::default());
    let mut got = Vec::new();
    scans(&p.root, &mut got);
    assert_eq!(got.len(), 16);
    let Logical::Bgp(pats) = root else { unreachable!() };
    for q in pats {
        assert!(got.contains(&q));
    }
    // The cheapest pattern (fewest rows) drives the greedy order.
    assert!(
        matches!(&first_unit(&p.root).op, Op::Scan(s) if s.pat.p == PatTerm::Const(iri(0))),
        "{}",
        crate::explain(&p, None)
    );
}

/// The first unit of a left-deep pipeline (probe / left input all the way down).
fn first_unit(p: &Plan) -> &Plan {
    match &p.op {
        Op::BindJoin { left, .. } | Op::NestedLoop { left, .. } => first_unit(left),
        Op::HashJoin { build, probe, build_is_left, .. } => first_unit(if *build_is_left { build } else { probe }),
        Op::Filter { input, .. } | Op::Project { input, .. } => first_unit(input),
        _ => p,
    }
}

#[test]
fn cross_products_come_last() {
    // ?0 :0 ?1 . ?2 :1 ?3 . ?1 :2 ?4 : the disconnected pattern is joined last even though
    // it is the smallest.
    let st = chain_store(3);
    let root = Logical::Bgp(vec![pat(0, 2, 1), pat(2, 0, 3), pat(1, 1, 4)]);
    let p = plan_of(&query(root, 5), &st, &QueryOptions::default());
    let last_is_cross = match &p.root.op {
        Op::NestedLoop { right, check, .. } => check.is_empty() && matches!(&right.op, Op::Scan(s) if s.pat.s == PatTerm::Slot(2)),
        _ => false,
    };
    assert!(last_is_cross, "{}", crate::explain(&p, None));
}

#[test]
fn values_estimates_follow_seeds() {
    let st = MemStore::new();
    let q = query(Logical::Unit, 2);
    let named = named_graphs(&q, &st).unwrap();
    let opts = QueryOptions::default();
    let mut planner = Planner::new(&st, &opts, &q.dataset, named.len(), &[], 2);
    let rows: Vec<Vec<TermId>> = (0..10).map(|i| vec![iri(i % 2), iri(i)]).collect();
    let unseeded = planner.plan_values(&[0, 1], &rows, Ctx::default());
    assert_eq!(unseeded.est_rows, 10.0);
    let seeded = planner.plan_values(&[0, 1], &rows, Ctx { seeded: SlotSet::from_slots([1]), ..Ctx::default() });
    assert_eq!(seeded.est_rows, 1.0);
    let by_first = planner.plan_values(&[0, 1], &rows, Ctx { seeded: SlotSet::from_slots([0]), ..Ctx::default() });
    assert_eq!(by_first.est_rows, 5.0);
}
