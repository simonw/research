//! Joins: every join operator x JoinKind, seeds, run reuse, the adaptive switch, merge-join
//! galloping and factorized counts, StarMerge, Minus (domain rule) and Union.
use super::*;

/// p1 {name A, age 30}, p2 {name B, age 20}, p3 {name C}, p4 {age 40}.
struct People {
    fx: Fx,
    p: [TermId; 4],
    names: [TermId; 3],
    name: TermId,
    age: TermId,
}

fn people() -> People {
    let fx = Fx::new();
    let p = ["p1", "p2", "p3", "p4"].map(|n| fx.iri(n));
    let (name, age) = (fx.iri("name"), fx.iri("age"));
    let names = ["A", "B", "C"].map(|n| fx.lit(n));
    fx.add(p[0], name, names[0]);
    fx.add(p[0], age, int(30));
    fx.add(p[1], name, names[1]);
    fx.add(p[1], age, int(20));
    fx.add(p[2], name, names[2]);
    fx.add(p[3], age, int(40));
    People { fx, p, names, name, age }
}

impl People {
    /// ?0 :name ?1
    fn left(&self) -> Plan {
        scan(v(0), c(self.name), v(1))
    }
    /// ?0 :age ?2
    fn right(&self) -> Plan {
        scan(v(0), c(self.age), v(2))
    }
    fn right_seeded(&self) -> Plan {
        scan_bound(GraphSel::Default, v(0), c(self.age), v(2), &[0])
    }
    fn run(&self, p: Plan) -> Vec<Vec<TermId>> {
        Q::new(&self.fx.store, 3, p).sorted(self.fx.rc())
    }
    fn expected(&self, kind: &JoinKind) -> Vec<Vec<TermId>> {
        let [p1, p2, p3, _] = self.p;
        let [a, b, cc] = self.names;
        let rows = match kind {
            JoinKind::Inner => vec![vec![p1, a, int(30)], vec![p2, b, int(20)]],
            JoinKind::Left { filter: None } => vec![vec![p1, a, int(30)], vec![p2, b, int(20)], vec![p3, cc, u()]],
            JoinKind::Left { filter: Some(_) } => vec![vec![p1, a, int(30)], vec![p2, b, u()], vec![p3, cc, u()]],
            JoinKind::Semi => vec![vec![p1, a, u()], vec![p2, b, u()]],
            JoinKind::Anti => vec![vec![p3, cc, u()]],
        };
        ms(rows)
    }
}

/// ?2 > 25
fn older_than_25() -> Expr {
    Expr::Greater(Box::new(sv(2)), Box::new(cv(int(25))))
}

fn kinds() -> Vec<JoinKind> {
    vec![JoinKind::Inner, left(None), left(Some(older_than_25())), JoinKind::Semi, JoinKind::Anti]
}

#[test]
fn bind_join_every_kind() {
    let t = people();
    for kind in kinds() {
        let got = t.run(bind_join(t.left(), t.right_seeded(), &[0], kind.clone()));
        assert_eq!(got, t.expected(&kind), "BindJoin {kind:?}");
        // sort_left gives the same multiset.
        let mut p = bind_join(t.left(), t.right_seeded(), &[0], kind.clone());
        if let Op::BindJoin { sort_left, .. } = &mut p.op {
            *sort_left = true;
        }
        assert_eq!(t.run(p), t.expected(&kind), "BindJoin sort_left {kind:?}");
    }
}

#[test]
fn hash_join_every_kind_and_build_side() {
    let t = people();
    for kind in kinds() {
        for build_is_left in [false, true] {
            let (b, p) = if build_is_left { (t.left(), t.right()) } else { (t.right(), t.left()) };
            let got = t.run(hash_join(b, p, &[0], &[], kind.clone(), build_is_left));
            assert_eq!(got, t.expected(&kind), "HashJoin {kind:?} build_is_left={build_is_left}");
        }
    }
}

#[test]
fn nested_loop_every_kind() {
    let t = people();
    for kind in kinds() {
        let got = t.run(nested(t.left(), t.right(), &[0], kind.clone()));
        assert_eq!(got, t.expected(&kind), "NestedLoop {kind:?}");
    }
    // No shared slot: a cross product.
    let a = values(&[0], vec![vec![int(1)], vec![int(2)]]);
    let b = values(&[1], vec![vec![int(3)], vec![int(4)], vec![int(5)]]);
    assert_eq!(Q::new(&t.fx.store, 2, nested(a, b, &[], JoinKind::Inner)).rows(t.fx.rc()).len(), 6);
}

#[test]
fn merge_join_matches_hash_join() {
    let t = people();
    let got = t.run(merge_join(t.left(), t.right(), 0, &[], 6));
    assert_eq!(got, t.expected(&JoinKind::Inner));
}

#[test]
fn hash_join_maybe_bound_check_slots_and_unbound_keys() {
    let t = people();
    let [p1, p2, ..] = t.p;
    // Left rows may leave ?1 unbound; the right binds ?1 too: compared via `check`.
    let l = values(&[0, 1], vec![vec![p1, int(1)], vec![p2, u()], vec![p1, int(2)]]);
    let r = values(&[0, 1], vec![vec![p1, int(1)], vec![p2, int(7)]]);
    let want = ms(vec![vec![p1, int(1)], vec![p2, int(7)]]);
    for build_is_left in [false, true] {
        let (b, p) = if build_is_left { (l.clone(), r.clone()) } else { (r.clone(), l.clone()) };
        let got = Q::new(&t.fx.store, 2, hash_join(b, p, &[0], &[1], JoinKind::Inner, build_is_left)).sorted(t.fx.rc());
        assert_eq!(got, want, "build_is_left={build_is_left}");
    }
    // A key slot that turns out unbound on either side is compatible with everything.
    let l = values(&[0, 1], vec![vec![u(), int(1)], vec![p1, int(2)]]);
    let r = values(&[0, 2], vec![vec![p1, int(3)], vec![u(), int(4)]]);
    for build_is_left in [false, true] {
        let (b, p) = if build_is_left { (l.clone(), r.clone()) } else { (r.clone(), l.clone()) };
        let got = Q::new(&t.fx.store, 3, hash_join(b, p, &[0], &[], JoinKind::Inner, build_is_left)).sorted(t.fx.rc());
        let want = ms(vec![
            vec![p1, int(1), int(3)],
            vec![u(), int(1), int(4)],
            vec![p1, int(2), int(3)],
            vec![p1, int(2), int(4)],
        ]);
        assert_eq!(got, want, "unbound keys, build_is_left={build_is_left}");
    }
}

#[test]
fn bind_join_reuses_runs_of_equal_seed_tuples() {
    let t = people();
    let [p1, p2, ..] = t.p;
    let rows: Vec<Vec<TermId>> = [p2, p1, p2, p1, p1, p2].iter().map(|x| vec![*x]).collect();
    for sort_left in [false, true] {
        let mut p = bind_join(values(&[0], rows.clone()), t.right_seeded(), &[0], JoinKind::Inner);
        if let Op::BindJoin { sort_left: s, .. } = &mut p.op {
            *s = sort_left;
        }
        let mut e = Q::new(&t.fx.store, 3, p).exec(t.fx.rc());
        let got = collect(&mut e).unwrap();
        assert_eq!(got.len(), 6);
        // Node ids: 0 BindJoin, 1 Values, 2 Scan.
        let opens = e.profile().nodes[2].opens;
        // Unsorted: p2 p1 p2 p1 (p1 reused) p2 = 5 probes; sorted: one per distinct key.
        assert_eq!(opens, if sort_left { 2 } else { 5 }, "probes with sort_left={sort_left}");
        if sort_left {
            let keys: Vec<TermId> = got.iter().map(|r| r[0]).collect();
            assert!(keys.windows(2).all(|w| w[0] <= w[1]), "left rows processed in key order");
        }
    }
    // Adjacent duplicates reuse the run even without sorting.
    let rows = vec![vec![p1], vec![p1], vec![p1], vec![p2]];
    let mut e = Q::new(&t.fx.store, 3, bind_join(values(&[0], rows), t.right_seeded(), &[0], JoinKind::Inner)).exec(t.fx.rc());
    assert_eq!(collect(&mut e).unwrap().len(), 4);
    assert_eq!(e.profile().nodes[2].opens, 2);
}

#[test]
fn bind_join_adaptive_switch_to_hash() {
    let fx = Fx::new();
    let (p, q) = (fx.iri("p"), fx.iri("q"));
    let mut quads = Vec::new();
    for i in 0..300 {
        let s = fx.iri(&format!("s{i}"));
        quads.push([TermId::DEFAULT_GRAPH, s, p, int(i)]);
        if i % 3 == 0 {
            quads.push([TermId::DEFAULT_GRAPH, s, q, int(i * 10)]);
        }
    }
    fx.store.insert(&mut quads).unwrap();
    // 60K left rows (VALUES over ?1 = 0..299 repeated), estimated at 10.
    let left_rows: Vec<Vec<TermId>> = (0..60_000).map(|i| vec![int(i % 300)]).collect();
    let mk = |adaptive: bool, kind: JoinKind| {
        let left = plan(Op::Values { slots: vec![1], rows: left_rows.clone() });
        let right = plan(Op::BindJoin {
            left: Box::new(scan_bound(GraphSel::Default, v(0), c(p), v(1), &[1])),
            right: Box::new(scan_bound(GraphSel::Default, v(0), c(q), v(2), &[0])),
            seed: vec![0],
            kind: JoinKind::Inner,
            sort_left: false,
            adaptive: false,
        });
        plan(Op::BindJoin { left: Box::new(left), right: Box::new(right), seed: vec![1], kind, sort_left: false, adaptive })
    };
    for kind in [JoinKind::Inner, left(None), JoinKind::Semi, JoinKind::Anti] {
        let mut e = Q::new(&fx.store, 3, mk(true, kind.clone())).exec(fx.rc());
        let adaptive = ms(collect(&mut e).unwrap());
        let probes = e.profile().nodes[2].opens;
        assert!(probes < 60_000, "{kind:?}: switched to a hash join after {probes} probes");
        let plain = Q::new(&fx.store, 3, mk(false, kind.clone())).sorted(fx.rc());
        assert_eq!(adaptive.len(), plain.len(), "{kind:?}");
        assert_eq!(adaptive, plain, "{kind:?}");
    }
}

#[test]
fn merge_join_many_to_many_check_and_factorized_count() {
    let fx = Fx::new();
    let (p, q) = (fx.iri("p"), fx.iri("q"));
    let mut quads = Vec::new();
    // ?x p ?a: keys 0..50, key k has (k % 3) + 1 rows; ?x q ?b: key k has (k % 4) rows.
    for k in 0..50i64 {
        let x = fx.iri(&format!("x{k}"));
        for j in 0..(k % 3) + 1 {
            quads.push([TermId::DEFAULT_GRAPH, x, p, int(j)]);
        }
        for j in 0..k % 4 {
            quads.push([TermId::DEFAULT_GRAPH, x, q, int(j)]);
        }
    }
    fx.store.insert(&mut quads).unwrap();
    let l = || scan(v(0), c(p), v(1));
    let r = || scan(v(0), c(q), v(2));
    let r_same = || scan(v(0), c(q), v(1));
    let expected: u64 = (0..50i64).map(|k| ((k % 3 + 1) * (k % 4)) as u64).sum();
    let mj = Q::new(&fx.store, 3, merge_join(l(), r(), 0, &[], 6)).sorted(fx.rc());
    let hj = Q::new(&fx.store, 3, hash_join(r(), l(), &[0], &[], JoinKind::Inner, false)).sorted(fx.rc());
    assert_eq!(mj.len() as u64, expected);
    assert_eq!(mj, hj);
    // Factorized count: no row is produced.
    let count = |input: Plan| plan(Op::Count { input: Box::new(input), out: 2 });
    let mut e = Q::new(&fx.store, 3, count(merge_join(l(), r(), 0, &[], 6))).form(QueryForm::Select(vec![2])).exec(fx.rc());
    assert_eq!(collect(&mut e).unwrap(), vec![vec![int(expected as i64)]]);
    assert_eq!(e.profile().nodes[1].rows_out, expected, "fast_count reports the count, not rows");
    // Execution::count on the join itself.
    let mut e = Q::new(&fx.store, 3, merge_join(l(), r(), 0, &[], 6)).exec(fx.rc());
    assert_eq!(e.count().unwrap(), expected);
    // With a check slot the count is iterated and the check applies (?1 shared).
    let checked = Q::new(&fx.store, 3, merge_join(l(), r_same(), 0, &[1], 6)).sorted(fx.rc());
    let hashed = Q::new(&fx.store, 3, hash_join(r_same(), l(), &[0], &[1], JoinKind::Inner, false)).sorted(fx.rc());
    assert_eq!(checked, hashed);
    let mut e = Q::new(&fx.store, 3, count(merge_join(l(), r_same(), 0, &[1], 6))).form(QueryForm::Select(vec![2])).exec(fx.rc());
    assert_eq!(collect(&mut e).unwrap(), vec![vec![int(checked.len() as i64)]]);
}

#[test]
fn merge_join_gallops_with_seeks() {
    let fx = Fx::new();
    let (p, q) = (fx.iri("p"), fx.iri("q"));
    let mut quads = Vec::new();
    // Left: 2000 subjects; right: only every 400th subject has q.
    let xs: Vec<TermId> = (0..2000).map(|k| fx.iri(&format!("x{k:05}"))).collect();
    for (k, x) in xs.iter().enumerate() {
        quads.push([TermId::DEFAULT_GRAPH, *x, p, int(k as i64)]);
    }
    for k in (0..2000).step_by(400) {
        quads.push([TermId::DEFAULT_GRAPH, xs[k], q, int(k as i64)]);
    }
    fx.store.insert(&mut quads).unwrap();
    // The sparse side on the left: the dense right side gallops.
    let mut e = Q::new(&fx.store, 3, merge_join(scan(v(0), c(q), v(2)), scan(v(0), c(p), v(1)), 0, &[], 6)).exec(fx.rc());
    let rows = collect(&mut e).unwrap();
    assert_eq!(rows.len(), 5);
    let seeks = e.profile().nodes[2].seeks;
    assert!(seeks > 1, "the right scan was re-positioned by galloping ({seeks})");
    // gallop = 0 disables seeking.
    let mut e = Q::new(&fx.store, 3, merge_join(scan(v(0), c(q), v(2)), scan(v(0), c(p), v(1)), 0, &[], 0)).exec(fx.rc());
    assert_eq!(collect(&mut e).unwrap(), rows);
    assert_eq!(e.profile().nodes[2].seeks, 1, "only the initial positioning");
    // Galloping over a SortBySlot enforcer (binary search in its buffer).
    let mut e = Q::new(&fx.store, 3, merge_join(scan(v(0), c(q), v(2)), sort_by(scan_order(GraphSel::Default, v(0), c(p), v(1), IndexOrder::Gpos), 0), 0, &[], 6))
        .exec(fx.rc());
    assert_eq!(ms(collect(&mut e).unwrap()), ms(rows));
}

#[test]
fn merge_join_rejects_unsorted_input() {
    let t = people();
    let [p1, p2, ..] = t.p;
    let unsorted = values(&[0], vec![vec![p2], vec![p1]]);
    let mut e = Q::new(&t.fx.store, 3, merge_join(unsorted, t.right(), 0, &[], 6)).exec(t.fx.rc());
    assert!(matches!(collect(&mut e), Err(Error::Internal(_))));
}

#[test]
fn star_merge_intersects_sorted_inputs() {
    let fx = Fx::new();
    let [a, b, cc] = ["a", "b", "c"].map(|n| fx.iri(n));
    let mut quads = Vec::new();
    let xs: Vec<TermId> = (0..30).map(|k| fx.iri(&format!("x{k:03}"))).collect();
    for (k, x) in xs.iter().enumerate() {
        if k % 2 == 0 {
            quads.push([TermId::DEFAULT_GRAPH, *x, a, int(k as i64)]);
        }
        if k % 3 == 0 {
            quads.push([TermId::DEFAULT_GRAPH, *x, b, int(k as i64)]);
            quads.push([TermId::DEFAULT_GRAPH, *x, b, int(1000 + k as i64)]);
        }
        if k % 5 == 0 {
            quads.push([TermId::DEFAULT_GRAPH, *x, cc, int(1)]);
        }
    }
    fx.store.insert(&mut quads).unwrap();
    let inputs = || vec![scan(v(0), c(a), v(1)), scan(v(0), c(b), v(2)), scan(v(0), c(cc), v(3))];
    let star = plan(Op::StarMerge { key: 0, inputs: inputs(), check: vec![] });
    let got = Q::new(&fx.store, 4, star).sorted(fx.rc());
    // k divisible by 30: only x000 (two b rows).
    assert_eq!(got.len(), 2);
    assert!(got.iter().all(|r| r[0] == xs[0]));
    let mut e = Q::new(&fx.store, 4, plan(Op::StarMerge { key: 0, inputs: inputs(), check: vec![] })).exec(fx.rc());
    assert_eq!(e.count().unwrap(), 2);
    // Two inputs, compared with a hash join.
    let two = plan(Op::StarMerge { key: 0, inputs: vec![scan(v(0), c(a), v(1)), scan(v(0), c(b), v(2))], check: vec![] });
    let got = Q::new(&fx.store, 3, two).sorted(fx.rc());
    let hj = Q::new(&fx.store, 3, hash_join(scan(v(0), c(b), v(2)), scan(v(0), c(a), v(1)), &[0], &[], JoinKind::Inner, false))
        .sorted(fx.rc());
    assert_eq!(got, hj);
    assert_eq!(got.len(), 10);
}

#[test]
fn minus_domain_rule() {
    let t = people();
    let [p1, p2, p3, p4] = t.p;
    let minus = |left: Plan, right: Plan, keys: &[Slot], check: &[Slot]| {
        plan(Op::Minus { left: Box::new(left), right: Box::new(right), keys: keys.to_vec(), check: check.to_vec() })
    };
    // { ?x :name ?n } MINUS { ?x :age ?a }: p3 has no age.
    let got = Q::new(&t.fx.store, 3, minus(t.left(), t.right(), &[0], &[])).sorted(t.fx.rc());
    assert_eq!(got, ms(vec![vec![p3, t.names[2], u()]]));
    // Compatible but disjoint domains: nothing removed (the shared slot is unbound on the left).
    let l = values(&[0, 1], vec![vec![u(), int(1)], vec![p4, int(2)]]);
    let r = values(&[0], vec![vec![p4]]);
    let got = Q::new(&t.fx.store, 2, minus(l, r, &[], &[0])).sorted(t.fx.rc());
    assert_eq!(got, ms(vec![vec![u(), int(1)]]), "the left row without ?0 shares no variable");
    // Incompatible on a check slot: kept.
    let l = values(&[0, 1], vec![vec![p1, int(1)], vec![p2, int(2)]]);
    let r = values(&[0, 1], vec![vec![p1, int(9)], vec![p2, u()]]);
    let got = Q::new(&t.fx.store, 2, minus(l, r, &[0], &[1])).sorted(t.fx.rc());
    assert_eq!(got, ms(vec![vec![p1, int(1)]]));
}

#[test]
fn union_concatenates_with_bag_semantics() {
    let t = people();
    let u2 = union(vec![t.left(), t.left(), t.right()]);
    let mut e = Q::new(&t.fx.store, 3, u2).exec(t.fx.rc());
    assert_eq!(collect(&mut e).unwrap().len(), 3 + 3 + 3);
    let mut e = Q::new(&t.fx.store, 3, union(vec![t.left(), t.right()])).exec(t.fx.rc());
    assert_eq!(e.count().unwrap(), 6, "fast count over every branch");
    // Union under a BindJoin: each branch gets the seed.
    let [p1, ..] = t.p;
    let right = union(vec![
        scan_bound(GraphSel::Default, v(0), c(t.name), v(1), &[0]),
        scan_bound(GraphSel::Default, v(0), c(t.age), v(2), &[0]),
    ]);
    let got = Q::new(&t.fx.store, 3, bind_join(values(&[0], vec![vec![p1]]), right, &[0], JoinKind::Inner)).sorted(t.fx.rc());
    assert_eq!(got, ms(vec![vec![p1, t.names[0], u()], vec![p1, u(), int(30)]]));
}

#[test]
fn sql_join_maps_columns_to_slots() {
    let t = people();
    let [p1, p2, ..] = t.p;
    let mut ts = TestStore::new(t.fx.store.clone());
    ts.sql = Some((2, vec![p1, int(30), p2, int(20)]));
    let p = plan(Op::SqlJoin { sql: "SELECT ...".into(), params: vec![], out: vec![2, 0] });
    let got = Q::new(&t.fx.store, 3, p).sorted(Rc::new(ts));
    assert_eq!(got, ms(vec![vec![int(30), u(), p1], vec![int(20), u(), p2]]));
    // A store without SQL support reports Unsupported.
    let p = plan(Op::SqlJoin { sql: "SELECT ...".into(), params: vec![], out: vec![0] });
    let mut e = Q::new(&t.fx.store, 1, p).exec(t.fx.rc());
    assert!(matches!(collect(&mut e), Err(Error::Unsupported(_))));
}
