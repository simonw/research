//! Interrupts, deadlines and the memory budget.
use sq_core::ir::{AggFunc, AggSpec};
use sq_core::spargebra::algebra::Function;

use super::*;

fn big(fx: &Fx, n: i64) -> TermId {
    let p = fx.iri("p");
    let mut quads: Vec<[TermId; 4]> =
        (0..n).map(|i| [TermId::DEFAULT_GRAPH, fx.iri(&format!("s{i}")), p, int(i)]).collect();
    fx.store.insert(&mut quads).unwrap();
    p
}

#[test]
fn interrupt_is_polled_during_cpu_work() {
    let fx = Fx::new();
    let p = big(&fx, 20_000);
    // The first poll (after 4096 rows) succeeds; a later one reports the interrupt.
    let ts = Rc::new(TestStore::new(fx.store.clone()));
    ts.interrupt_after.set(Some(1));
    let mut e = Q::new(&fx.store, 2, scan(v(0), c(p), v(1))).exec(ts.clone());
    let r = collect(&mut e);
    assert!(matches!(r, Err(Error::Interrupted)), "{r:?}");
    assert!(ts.polls.get() >= 2);
    // A hash join build is CPU work without output rows.
    let ts = Rc::new(TestStore::new(fx.store.clone()));
    ts.interrupt_after.set(Some(1));
    let j = hash_join(scan(v(0), c(p), v(1)), values(&[0], vec![]), &[0], &[], JoinKind::Inner, false);
    let mut e = Q::new(&fx.store, 2, j).exec(ts);
    assert!(matches!(e.count(), Err(Error::Interrupted)));
    // A count over a filter is CPU work: the first poll reports the interrupt.
    let ts = Rc::new(TestStore::new(fx.store.clone()));
    ts.interrupt_after.set(Some(0));
    let f = filter(scan(v(0), c(p), v(1)), Expr::Less(Box::new(sv(1)), Box::new(cv(int(0)))));
    let mut e = Q::new(&fx.store, 2, f).exec(ts.clone());
    assert!(matches!(e.count(), Err(Error::Interrupted)));
    assert_eq!(ts.polls.get(), 1);
}

#[test]
fn deadline_reports_timeout() {
    let fx = Fx::new();
    let p = big(&fx, 100);
    let mut q = Q::new(&fx.store, 2, scan(v(0), c(p), v(1)));
    q.opts.timeout_ms = Some(0);
    let mut e = q.exec(fx.rc());
    assert!(matches!(collect(&mut e), Err(Error::Timeout)));
    let mut q = Q::new(&fx.store, 2, scan(v(0), c(p), v(1)));
    q.opts.timeout_ms = Some(60_000);
    assert_eq!(collect(&mut q.exec(fx.rc())).unwrap().len(), 100);
}

#[test]
fn memory_budget_is_charged_by_materializing_operators() {
    let fx = Fx::new();
    let p = big(&fx, 3000);
    let sort = || plan(Op::Sort { input: Box::new(scan(v(0), c(p), v(1))), keys: vec![order_key(sv(1), true)] });
    let hash = || hash_join(scan(v(0), c(p), v(1)), scan(v(0), c(p), v(2)), &[0], &[], JoinKind::Inner, false);
    let distinct = || plan(Op::Distinct { input: Box::new(scan(v(0), c(p), v(1))), slots: vec![1] });
    for (name, p) in [("sort", sort()), ("hash", hash()), ("distinct", distinct())] {
        let mut q = Q::new(&fx.store, 3, p);
        q.opts.max_memory_mb = 0;
        let mut e = q.exec(fx.rc());
        assert!(matches!(collect(&mut e), Err(Error::MemoryBudget(0))), "{name}");
    }
    // Streaming operators charge nothing.
    let mut q = Q::new(&fx.store, 2, scan(v(0), c(p), v(1)));
    q.opts.max_memory_mb = 0;
    assert_eq!(collect(&mut q.exec(fx.rc())).unwrap().len(), 3000);
    // Charges are released when operators close.
    let mut e = Q::new(&fx.store, 3, sort()).exec(fx.rc());
    assert_eq!(collect(&mut e).unwrap().len(), 3000);
    assert_eq!(e.memory_used(), 0);
}

fn budget_error(store: &dyn QuadStore, width: usize, root: Plan, rc: Rc<dyn QuadStore>) -> Result<Vec<Vec<TermId>>> {
    let mut q = Q::new(store, width, root);
    q.opts.max_memory_mb = 1;
    collect(&mut q.exec(rc))
}

fn group_of(input: Plan, keys: &[Slot], aggs: Vec<(Slot, AggSpec)>, streaming: bool) -> Plan {
    plan(Op::Group { input: Box::new(input), keys: keys.to_vec(), aggs, streaming })
}

#[test]
fn aggregate_state_is_charged_against_the_memory_budget() {
    let fx = Fx::new();
    // COUNT(DISTINCT *) over 100K distinct solutions in one group.
    let rows: Vec<Vec<TermId>> = (0..100_000).map(|i| vec![int(i)]).collect();
    let all = AggSpec { func: AggFunc::CountAll, distinct: true, arg: None };
    let r = budget_error(&fx.store, 2, group_of(values(&[0], rows), &[], vec![(1, all)], false), fx.rc());
    assert!(matches!(r, Err(Error::MemoryBudget(1))), "COUNT(DISTINCT *): {:?}", r.map(|v| v.len()));
    // GROUP_CONCAT over 20K stored strings of about 80 bytes.
    let p = fx.iri("p");
    let mut quads: Vec<[TermId; 4]> = (0..20_000)
        .map(|i| [TermId::DEFAULT_GRAPH, fx.iri(&format!("s{i}")), p, fx.lit(&format!("{i:0>80}"))])
        .collect();
    fx.store.insert(&mut quads).unwrap();
    let concat = AggSpec { func: AggFunc::GroupConcat { separator: ",".into() }, distinct: false, arg: Some(sv(1)) };
    let r = budget_error(&fx.store, 3, group_of(scan(v(0), c(p), v(1)), &[], vec![(2, concat)], false), fx.rc());
    assert!(matches!(r, Err(Error::MemoryBudget(1))), "GROUP_CONCAT: {:?}", r.map(|v| v.len()));
}

#[test]
fn streaming_groups_release_their_state() {
    let fx = Fx::new();
    // 2000 groups of 100 distinct values, sorted by key: one group's set at a time, while a
    // hash aggregation holds them all (200K values) and exceeds the budget.
    let rows: Vec<Vec<TermId>> = (0..200_000).map(|i| vec![int(i / 100), int(i)]).collect();
    let agg = || vec![(2, AggSpec { func: AggFunc::Count, distinct: true, arg: Some(sv(1)) })];
    let streaming = group_of(values(&[0, 1], rows.clone()), &[0], agg(), true);
    let mut q = Q::new(&fx.store, 3, streaming);
    q.opts.max_memory_mb = 1;
    let mut e = q.exec(fx.rc());
    let got = collect(&mut e).unwrap();
    assert_eq!(got.len(), 2000);
    assert!(got.iter().all(|r| r[2] == int(100)));
    assert_eq!(e.memory_used(), 0);
    let hash = group_of(values(&[0, 1], rows), &[0], agg(), false);
    assert!(matches!(budget_error(&fx.store, 3, hash, fx.rc()), Err(Error::MemoryBudget(1))));
}

#[test]
fn computed_terms_and_fresh_blank_nodes_are_charged() {
    let fx = Fx::new();
    let p = big(&fx, 20_000);
    // BIND(BNODE() AS ?2) per row: recorded by the infallible resolver call, reported by a poll.
    let bnode = plan(Op::Extend {
        input: Box::new(scan(v(0), c(p), v(1))),
        slot: 2,
        expr: Expr::Call(Function::BNode, vec![]),
        canonical: false,
    });
    let r = budget_error(&fx.store, 3, bnode, fx.rc());
    assert!(matches!(r, Err(Error::MemoryBudget(1))), "BNODE(): {:?}", r.map(|v| v.len()));
    // A computed term stays charged for the whole execution; an already interned one is free.
    let ucase = |input: Plan| plan(Op::Extend {
        input: Box::new(input),
        slot: 2,
        expr: Expr::Call(Function::UCase, vec![cv(fx.lit("computed"))]),
        canonical: false,
    });
    let mut e = Q::new(&fx.store, 3, ucase(scan(v(0), c(p), v(1)))).exec(fx.rc());
    assert_eq!(collect(&mut e).unwrap().len(), 20_000);
    let one = e.memory_used();
    assert!(one > 0 && one < 1024, "one TEMP term: {one} bytes");
}

#[test]
fn too_many_slots_and_bad_layouts_are_rejected() {
    let fx = Fx::new();
    let r = Q::new(&fx.store, 300, plan(Op::Unit)).try_exec(fx.rc());
    assert!(matches!(r, Err(Error::Unsupported(_))));
    let p = fx.iri("p");
    let r = Q::new(&fx.store, 1, scan(v(0), c(p), v(5))).try_exec(fx.rc());
    assert!(matches!(r, Err(Error::Internal(_))));
}

/// Probes that visit every build row (no keys, or an unbound key) and match none produce no
/// rows, so only the visited candidates show the CPU work to the interrupt polling.
#[test]
fn probes_without_matches_are_polled() {
    let fx = Fx::new();
    let left = || values(&[0], (0..2000).map(|i| vec![int(i)]).collect());
    let right = || values(&[0, 1], (0..3000).map(|i| vec![int(10_000 + i), int(i)]).collect());
    let nl = nested(left(), right(), &[0], JoinKind::Inner);
    let minus = plan(Op::Minus { left: Box::new(left()), right: Box::new(right()), keys: vec![], check: vec![0] });
    // An unbound probe key visits every build row of a hash join too (none agrees on ?1).
    let unbound = values(&[1], (0..2000).map(|i| vec![int(-1 - i)]).collect());
    let hj = hash_join(right(), unbound, &[0], &[1], JoinKind::Inner, false);
    for (name, p, want) in [("nested loop", nl, 0), ("minus", minus, 2000), ("hash join", hj, 0)] {
        // 6M compatibility checks: polled about every 4096 (here: every two probe rows).
        let ts = Rc::new(TestStore::new(fx.store.clone()));
        ts.interrupt_after.set(Some(10));
        let mut e = Q::new(&fx.store, 2, p.clone()).exec(ts.clone());
        assert!(matches!(e.count(), Err(Error::Interrupted)), "{name}");
        let ts = Rc::new(TestStore::new(fx.store.clone()));
        assert_eq!(Q::new(&fx.store, 2, p).exec(ts.clone()).count().unwrap(), want, "{name}");
        assert!(ts.polls.get() >= 1000, "{name}: {} polls", ts.polls.get());
    }
}

/// `now_utc` is the value of `DateTime::now()` (UTC, same instant up to the time between the
/// two calls), computed without the general date arithmetic.
#[test]
fn now_utc_agrees_with_datetime_now() {
    let before = oxsdatatypes::DateTime::now();
    let ours = crate::ctx::now_utc();
    let after = oxsdatatypes::DateTime::now();
    assert!(before <= ours && ours <= after, "{before} <= {ours} <= {after}");
    assert_eq!(ours.timezone_offset(), before.timezone_offset());
    assert!(ours.to_string().ends_with('Z'), "{ours}");
}
