//! Interrupts, deadlines and the memory budget.
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

#[test]
fn too_many_slots_and_bad_layouts_are_rejected() {
    let fx = Fx::new();
    let r = Q::new(&fx.store, 300, plan(Op::Unit)).try_exec(fx.rc());
    assert!(matches!(r, Err(Error::Unsupported(_))));
    let p = fx.iri("p");
    let r = Q::new(&fx.store, 1, scan(v(0), c(p), v(5))).try_exec(fx.rc());
    assert!(matches!(r, Err(Error::Internal(_))));
}
