//! Filter, Extend, Group / Count, Sort / TopK / SortBySlot, Distinct, Reduced, Slice and
//! Project.
use super::*;
use sq_core::ir::{AggFunc, AggSpec};
use sq_core::spargebra::algebra::Function;

fn agg(func: AggFunc, distinct: bool, arg: Option<Expr>) -> AggSpec {
    AggSpec { func, distinct, arg }
}

fn group(input: Plan, keys: &[Slot], aggs: Vec<(Slot, AggSpec)>) -> Plan {
    plan(Op::Group { input: Box::new(input), keys: keys.to_vec(), aggs, streaming: false })
}

fn add(a: Expr, b: Expr) -> Expr {
    Expr::Add(Box::new(a), Box::new(b))
}

#[test]
fn filter_errors_are_false() {
    let fx = Fx::new();
    let s = fx.lit("text");
    let rows = values(&[0], vec![vec![int(1)], vec![int(5)], vec![s], vec![u()]]);
    let f = filter(rows, Expr::Greater(Box::new(sv(0)), Box::new(cv(int(2)))));
    assert_eq!(Q::new(&fx.store, 1, f).rows(fx.rc()), vec![vec![int(5)]]);
    // NOT(error) is still an error.
    let rows = values(&[0], vec![vec![int(1)], vec![s]]);
    let f = filter(rows, Expr::Not(Box::new(Expr::Greater(Box::new(sv(0)), Box::new(cv(int(2)))))));
    assert_eq!(Q::new(&fx.store, 1, f).rows(fx.rc()), vec![vec![int(1)]]);
}

#[test]
fn extend_errors_leave_the_slot_unbound_and_canonical_ids() {
    let fx = Fx::new();
    let s = fx.lit("text");
    let rows = values(&[0], vec![vec![int(1)], vec![s]]);
    let got = Q::new(&fx.store, 2, extend(rows, 1, add(sv(0), cv(int(1))))).rows(fx.rc());
    assert_eq!(got, vec![vec![int(1), int(2)], vec![s, u()]]);
    // CONCAT of a stored long string: canonical -> the dictionary id; else a TEMP id.
    let long = fx.lit("a long stored string");
    let piece = fx.lit("a long stored");
    let concat = Expr::Call(Function::Concat, vec![sv(0), cv(sq_core::codec::encode_short_str(" string").unwrap())]);
    let mut p = extend(values(&[0], vec![vec![piece]]), 1, concat.clone());
    let got = Q::new(&fx.store, 2, p.clone()).rows(fx.rc());
    assert_eq!(got[0][1], long, "canonical: the stored id");
    if let Op::Extend { canonical, .. } = &mut p.op {
        *canonical = false;
    }
    let mut e = Q::new(&fx.store, 2, p).exec(fx.rc());
    let got = collect(&mut e).unwrap();
    assert!(got[0][1].is_temp(), "output-only: a TEMP id");
    assert_eq!(show(&mut e, &[got[0][1]]), vec!["\"a long stored string\"".to_string()]);
}

#[test]
fn group_aggregates() {
    let fx = Fx::new();
    let (ka, kb) = (fx.iri("ka"), fx.iri("kb"));
    let rows =
        values(&[0, 1], vec![vec![ka, int(1)], vec![kb, int(10)], vec![ka, int(2)], vec![ka, int(2)], vec![kb, u()]]);
    let aggs = vec![
        (2, agg(AggFunc::CountAll, false, None)),
        (3, agg(AggFunc::Count, false, Some(sv(1)))),
        (4, agg(AggFunc::Sum, false, Some(sv(1)))),
        (5, agg(AggFunc::Count, true, Some(sv(1)))),
        (6, agg(AggFunc::Min, false, Some(sv(1)))),
        (7, agg(AggFunc::Max, false, Some(sv(1)))),
    ];
    let mut e = Q::new(&fx.store, 8, group(rows, &[0], aggs)).exec(fx.rc());
    let got = collect(&mut e).unwrap();
    // Groups in first-seen order.
    assert_eq!(got[0], vec![ka, u(), int(3), int(3), int(5), int(2), int(1), int(2)]);
    // An unbound argument is an error for SUM / MIN / MAX: those are unbound (agg-err-01);
    // COUNT skips it.
    assert_eq!(got[1], vec![kb, u(), int(2), int(1), u(), int(1), u(), u()]);
    assert_eq!(got.len(), 2);
}

#[test]
fn group_empty_input() {
    let fx = Fx::new();
    let empty = || plan(Op::Empty);
    let aggs = || {
        vec![
            (1, agg(AggFunc::CountAll, false, None)),
            (2, agg(AggFunc::Sum, false, Some(sv(0)))),
            (3, agg(AggFunc::Min, false, Some(sv(0)))),
            (4, agg(AggFunc::GroupConcat { separator: " ".into() }, false, Some(sv(0)))),
        ]
    };
    let got = Q::new(&fx.store, 5, group(empty(), &[], aggs())).rows(fx.rc());
    let empty_str = sq_core::codec::encode_short_str("").unwrap();
    assert_eq!(got, vec![vec![u(), int(0), int(0), u(), empty_str]], "no GROUP BY: one row");
    let got = Q::new(&fx.store, 5, group(empty(), &[0], aggs())).rows(fx.rc());
    assert!(got.is_empty(), "GROUP BY over no rows: no row");
}

#[test]
fn erroring_group_keys_form_their_own_group() {
    let fx = Fx::new();
    let s = fx.lit("text");
    let rows = values(&[0], vec![vec![int(1)], vec![s], vec![int(1)], vec![u()], vec![int(2)], vec![s]]);
    // GROUP BY (?0 + 1 AS ?1): errors (string, unbound) group together as ERROR, output UNBOUND.
    let keyed = extend(rows.clone(), 1, add(sv(0), cv(int(1))));
    let p = group(keyed, &[1], vec![(2, agg(AggFunc::CountAll, false, None))]);
    let got = Q::new(&fx.store, 3, p).form(QueryForm::Select(vec![1, 2])).sorted(fx.rc());
    assert_eq!(got, ms(vec![vec![int(2), int(2)], vec![u(), int(3)], vec![int(3), int(1)]]));
    // A plain unbound key is the UNBOUND group, distinct from the ERROR group.
    let keyed = extend(rows, 1, add(sv(0), cv(int(1))));
    let p = group(keyed, &[0, 1], vec![(2, agg(AggFunc::CountAll, false, None))]);
    let got = Q::new(&fx.store, 3, p).sorted(fx.rc());
    assert_eq!(
        got,
        ms(vec![
            vec![int(1), int(2), int(2)],
            vec![s, u(), int(2)],
            vec![u(), u(), int(1)],
            vec![int(2), int(3), int(1)],
        ])
    );
}

#[test]
fn count_distinct_star_ignores_hidden_slots() {
    let fx = Fx::new();
    // ?0 user, ?1 a blank-node pattern variable (hidden).
    let rows = values(&[0, 1], vec![vec![int(1), int(10)], vec![int(1), int(11)], vec![int(2), int(10)]]);
    let p = group(rows, &[], vec![(2, agg(AggFunc::CountAll, true, None)), (3, agg(AggFunc::CountAll, false, None))]);
    let got = Q::new(&fx.store, 4, p).origin(1, VarOrigin::BlankNode).form(QueryForm::Select(vec![2, 3])).rows(fx.rc());
    assert_eq!(got, vec![vec![int(2), int(3)]]);
}

#[test]
fn streaming_group_matches_hash_group() {
    let fx = Fx::new();
    let rows: Vec<Vec<TermId>> = (0..3000).map(|i| vec![int(i / 7), int(i % 5)]).collect();
    let aggs = || vec![(2, agg(AggFunc::Sum, false, Some(sv(1)))), (3, agg(AggFunc::Count, true, Some(sv(1))))];
    let hash = Q::new(&fx.store, 4, group(values(&[0, 1], rows.clone()), &[0], aggs())).rows(fx.rc());
    let streaming =
        plan(Op::Group { input: Box::new(values(&[0, 1], rows)), keys: vec![0], aggs: aggs(), streaming: true });
    let got = Q::new(&fx.store, 4, streaming).rows(fx.rc());
    assert_eq!(got, hash);
    assert_eq!(got.len(), 429);
}

#[test]
fn count_operator_iterates_without_fast_path() {
    let fx = Fx::new();
    let rows: Vec<Vec<TermId>> = (0..2500).map(|i| vec![int(i)]).collect();
    let f = filter(values(&[0], rows), Expr::Greater(Box::new(sv(0)), Box::new(cv(int(99)))));
    let p = plan(Op::Count { input: Box::new(f), out: 1 });
    let got = Q::new(&fx.store, 2, p).form(QueryForm::Select(vec![1])).rows(fx.rc());
    assert_eq!(got, vec![vec![int(2400)]]);
}

#[test]
fn execution_count_of_an_aggregate_evaluates_its_input() {
    // `Execution::count` of `SELECT (COUNT(*) AS ?n)` is 1, but it must still run the count
    // (it used to report the implicit group's one row without opening the input).
    let fx = Fx::new();
    let rows: Vec<Vec<TermId>> = (0..2500).map(|i| vec![int(i)]).collect();
    let f = filter(values(&[0], rows), Expr::Greater(Box::new(sv(0)), Box::new(cv(int(99)))));
    let p = plan(Op::Count { input: Box::new(f), out: 1 });
    let mut e = Q::new(&fx.store, 2, p).form(QueryForm::Select(vec![1])).exec(fx.rc());
    assert_eq!(e.count().unwrap(), 1);
    // Node ids: 0 Count, 1 Filter, 2 Values.
    let prof = e.profile();
    assert_eq!(prof.nodes[1].opens, 1);
    assert_eq!(prof.nodes[1].rows_out, 2400, "the filtered input was evaluated");
}

#[test]
fn sort_orders_mixed_terms_stably() {
    let fx = Fx::new();
    let (iri, b) = (fx.iri("z"), fx.bnode());
    let (s1, s2) = (fx.lit("apple"), fx.lit("banana"));
    let dec = fx.typed("1.5", "decimal");
    let input = || {
        values(
            &[0, 1],
            vec![
                vec![int(10), int(0)],
                vec![s2, int(1)],
                vec![u(), int(2)],
                vec![dec, int(3)],
                vec![iri, int(4)],
                vec![b, int(5)],
                vec![int(10), int(6)],
                vec![s1, int(7)],
                vec![int(-3), int(8)],
            ],
        )
    };
    let sort = |desc: bool| plan(Op::Sort { input: Box::new(input()), keys: vec![order_key(sv(0), desc)] });
    let got: Vec<TermId> = Q::new(&fx.store, 2, sort(false)).rows(fx.rc()).into_iter().map(|r| r[1]).collect();
    // unbound < bnode < IRI < literals (numbers by value, then strings); ties keep input order.
    assert_eq!(got, [2, 5, 4, 8, 3, 0, 6, 7, 1].map(int).to_vec());
    let got: Vec<TermId> = Q::new(&fx.store, 2, sort(true)).rows(fx.rc()).into_iter().map(|r| r[1]).collect();
    assert_eq!(got, [1, 7, 0, 6, 3, 8, 4, 5, 2].map(int).to_vec(), "DESC reverses, ties stay stable");
}

#[test]
fn sort_int_fast_path_and_multiple_keys() {
    let fx = Fx::new();
    let rows: Vec<Vec<TermId>> = (0..2000).map(|i| vec![int((i * 7919) % 101 - 50), int(i % 3), int(i)]).collect();
    let keys = vec![order_key(sv(1), true), order_key(sv(0), false)];
    let got =
        Q::new(&fx.store, 3, plan(Op::Sort { input: Box::new(values(&[0, 1, 2], rows.clone())), keys })).rows(fx.rc());
    let mut want = rows.clone();
    want.sort_by(|a, b| b[1].as_int().cmp(&a[1].as_int()).then(a[0].as_int().cmp(&b[0].as_int())));
    assert_eq!(got, want);
    // An expression key (not a bare variable): computed SortKeys.
    let keys = vec![order_key(Expr::UnaryMinus(Box::new(sv(0))), false)];
    let got =
        Q::new(&fx.store, 3, plan(Op::Sort { input: Box::new(values(&[0, 1, 2], rows.clone())), keys })).rows(fx.rc());
    let mut want = rows;
    want.sort_by(|a, b| b[0].as_int().cmp(&a[0].as_int()));
    assert_eq!(got, want);
}

#[test]
fn topk_equals_sort_then_slice_and_is_stable() {
    let fx = Fx::new();
    let s = fx.lit("a string literal");
    let mut rows: Vec<Vec<TermId>> = (0..3000).map(|i| vec![int((i * 37) % 50), int(i)]).collect();
    rows[1500][0] = s; // forces the switch from the INT fast path to SortKeys
    for (k, desc) in [(1u64, false), (10, false), (25, true), (2999, false), (5000, true)] {
        let keys = vec![order_key(sv(0), desc)];
        let topk = plan(Op::TopK { input: Box::new(values(&[0, 1], rows.clone())), keys: keys.clone(), k });
        let sorted = slice(plan(Op::Sort { input: Box::new(values(&[0, 1], rows.clone())), keys }), 0, Some(k));
        let a = Q::new(&fx.store, 2, topk).rows(fx.rc());
        let b = Q::new(&fx.store, 2, sorted).rows(fx.rc());
        assert_eq!(a, b, "k={k} desc={desc}");
    }
    let topk = plan(Op::TopK { input: Box::new(values(&[0, 1], rows)), keys: vec![order_key(sv(0), false)], k: 0 });
    assert!(Q::new(&fx.store, 2, topk).rows(fx.rc()).is_empty());
}

#[test]
fn sort_by_slot_is_a_stable_raw_id_sort() {
    let fx = Fx::new();
    let (x, y) = (fx.iri("x"), fx.iri("y"));
    let rows = vec![vec![y, int(0)], vec![int(-5), int(1)], vec![x, int(2)], vec![y, int(3)], vec![int(-5), int(4)]];
    let got = Q::new(&fx.store, 2, sort_by(values(&[0, 1], rows.clone()), 0)).rows(fx.rc());
    let mut want = rows;
    want.sort_by_key(|r| r[0]);
    assert_eq!(got, want);
}

#[test]
fn distinct_reduced_and_project() {
    let fx = Fx::new();
    let rows = vec![
        vec![int(1), int(1), int(7)],
        vec![int(1), int(1), int(8)],
        vec![int(2), int(1), int(7)],
        vec![int(1), int(1), int(9)],
    ];
    let distinct = |slots: &[Slot]| {
        let p = project(values(&[0, 1, 2], rows.clone()), slots);
        plan(Op::Distinct { input: Box::new(p), slots: slots.to_vec() })
    };
    assert_eq!(
        Q::new(&fx.store, 3, distinct(&[0])).rows(fx.rc()),
        vec![vec![int(1), u(), u()], vec![int(2), u(), u()]]
    );
    assert_eq!(Q::new(&fx.store, 3, distinct(&[0, 1])).rows(fx.rc()).len(), 2);
    assert_eq!(Q::new(&fx.store, 3, distinct(&[0, 1, 2])).rows(fx.rc()).len(), 4);
    assert_eq!(Q::new(&fx.store, 3, distinct(&[])).rows(fx.rc()), vec![vec![u(), u(), u()]]);
    let reduced =
        plan(Op::Reduced { input: Box::new(project(values(&[0, 1, 2], rows.clone()), &[0])), slots: vec![0] });
    assert_eq!(Q::new(&fx.store, 3, reduced).rows(fx.rc()).len(), 3, "only adjacent duplicates are dropped");
    // Distinct over many rows with spilled keys (5 slots).
    let many: Vec<Vec<TermId>> = (0..5000).map(|i| (0..5).map(|j| int((i % 777) + j)).collect()).collect();
    let d = plan(Op::Distinct { input: Box::new(values(&[0, 1, 2, 3, 4], many)), slots: vec![0, 1, 2, 3, 4] });
    assert_eq!(Q::new(&fx.store, 5, d).rows(fx.rc()).len(), 777);
}

#[test]
fn slice_offsets_limits_and_stops_early() {
    let fx = Fx::new();
    let p = fx.iri("p");
    let mut quads: Vec<[TermId; 4]> =
        (0..5000).map(|i| [TermId::DEFAULT_GRAPH, fx.iri(&format!("s{i}")), p, int(i)]).collect();
    fx.store.insert(&mut quads).unwrap();
    let all = Q::new(&fx.store, 2, scan(v(0), c(p), v(1))).rows(fx.rc());
    for (off, lim) in
        [(0, Some(0)), (0, Some(3)), (10, Some(5)), (4990, Some(100)), (6000, None), (1023, Some(2)), (0, None)]
    {
        let got = Q::new(&fx.store, 2, slice(scan(v(0), c(p), v(1)), off, lim)).rows(fx.rc());
        let want: Vec<Vec<TermId>> =
            all.iter().skip(off as usize).take(lim.unwrap_or(u64::MAX) as usize).cloned().collect();
        assert_eq!(got, want, "OFFSET {off} LIMIT {lim:?}");
    }
    // LIMIT 3 reads a few rows of the scan, not all 5000.
    let mut e = Q::new(&fx.store, 2, slice(scan(v(0), c(p), v(1)), 0, Some(3))).exec(fx.rc());
    assert_eq!(collect(&mut e).unwrap().len(), 3);
    assert!(e.profile().nodes[1].rows_out < 100, "early stop: {}", e.profile().nodes[1].rows_out);
    // A Slice that starts in a partially filled output batch (after a Union branch).
    let head: Vec<Vec<TermId>> = (0..1000).map(|i| vec![u(), int(-i)]).collect();
    let u2 = union(vec![values(&[0, 1], head), slice(scan(v(0), c(p), v(1)), 4000, Some(30))]);
    let got = Q::new(&fx.store, 2, u2).rows(fx.rc());
    assert_eq!(got.len(), 1030);
    assert_eq!(&got[1000..], &all[4000..4030]);
    // count() through a Slice uses the input's fast count.
    let mut e = Q::new(&fx.store, 2, slice(scan(v(0), c(p), v(1)), 100, Some(7000))).exec(fx.rc());
    assert_eq!(e.count().unwrap(), 4900);
}

/// count() of a LIMIT query asks for capped counts (offset + limit) and equals the number of
/// rows the query returns, through every fast-countable operator and through nested slices.
#[test]
fn limited_counts_are_capped_and_exact() {
    let fx = Fx::new();
    let [p, g1, g2, g3] = ["p", "g1", "g2", "g3"].map(|n| fx.iri(n));
    let subjects: Vec<TermId> = (0..300).map(|i| fx.iri(&format!("s{i}"))).collect();
    let mut quads: Vec<[TermId; 4]> = Vec::new();
    for (g, n) in [(TermId::DEFAULT_GRAPH, 300), (g1, 100), (g2, 50), (g3, 200)] {
        quads.extend(subjects[..n].iter().enumerate().map(|(i, s)| [g, *s, p, int(i as i64)]));
    }
    fx.store.insert(&mut quads).unwrap();
    let named = vec![g1, g2, g3];
    let only = NamedGraphs::Only(Arc::from(named.clone()));
    let sp = move || scan(v(0), c(p), v(1));
    let per_graph = move || scan_bound(GraphSel::Slot(2), v(0), c(p), v(1), &[]);
    let not_fast = move || filter(sp(), Expr::Less(Box::new(sv(1)), Box::new(cv(int(250)))));
    type Input = (&'static str, Box<dyn Fn() -> Plan>);
    let inputs: Vec<Input> = vec![
        ("scan", Box::new(sp)),
        ("per graph", Box::new(per_graph)),
        ("union", Box::new(move || union(vec![sp(), sp(), sp()]))),
        ("union with iteration", Box::new(move || union(vec![sp(), not_fast(), sp()]))),
        ("merge join", Box::new(move || merge_join(sp(), scan(v(0), c(p), v(2)), 0, &[], 6))),
        ("nested slice", Box::new(move || slice(slice(sp(), 10, Some(200)), 5, Some(150)))),
    ];
    for (name, input) in &inputs {
        for (off, lim) in [(0, Some(0)), (0, Some(3)), (7, Some(10)), (290, Some(100)), (0, Some(1000)), (5000, Some(1))] {
            let q = || {
                Q::new(&fx.store, 3, slice(input(), off, lim)).dataset(DefaultGraph::StoreDefault, only.clone(), named.clone())
            };
            let want = q().rows(fx.rc()).len() as u64;
            let ts = Rc::new(TestStore::new(fx.store.clone()));
            let mut e = q().exec(ts.clone());
            assert_eq!(e.count().unwrap(), want, "{name}: OFFSET {off} LIMIT {lim:?}");
            assert_eq!(ts.uncapped.get(), 0, "{name}: OFFSET {off} LIMIT {lim:?} counted a whole range");
            if lim != Some(0) && *name != "merge join" {
                assert!(ts.counts.get() > 0, "{name}: OFFSET {off} LIMIT {lim:?} is counted by the store");
            }
        }
    }
}

/// The SortKey of a stored or inline id, computed independently of the operators (decode,
/// then the value's key). Only for canonical values, whose stored lexical form is canonical.
fn ref_key(fx: &Fx, id: TermId) -> sq_expr::SortKey {
    if !sq_expr::is_present(id) {
        return sq_expr::SortKey::lowest();
    }
    let t = match sq_expr::inline_value(id) {
        Some(v) => v.to_term(),
        None => (*fx.store.decode_one(id).unwrap()).clone(),
    };
    sq_expr::sort_key_value(&sq_expr::Value::from_term(&t))
}

/// Rows over every term class, with many ties on each key column; column 3 is the row number.
fn lazy_rows(fx: &Fx, r: &mut sq_core::testing::Rng, n: usize) -> Vec<Vec<TermId>> {
    let mut pool: Vec<TermId> = (-3..=3).map(int).collect();
    pool.extend((0..5).map(|i| fx.iri(&format!("lazy{}", (i * 7) % 5))));
    pool.extend(["a", "bb", "a longer literal", "another long literal"].iter().map(|s| fx.lit(s)));
    pool.extend(["1.5", "-2.25", "0.5"].iter().map(|x| fx.typed(x, "decimal")));
    pool.extend([TermId::from_bool(true), TermId::from_bool(false), fx.bnode(), fx.bnode()]);
    pool.extend([TermId::UNBOUND, TermId::ERROR]);
    let ints_only = |r: &mut sq_core::testing::Rng| int(r.below(5) as i64 - 2);
    (0..n)
        .map(|i| {
            let mut pick = |mostly_ints: bool| {
                if mostly_ints && r.below(4) != 0 { ints_only(r) } else { pool[r.below(pool.len() as u64) as usize] }
            };
            let (a, b, c) = (pick(true), pick(false), pick(i % 2 == 0));
            vec![a, b, c, int(i as i64)]
        })
        .collect()
}

/// Lazily built keys (TopK's raw-id comparisons, Sort's run refinement) give exactly the
/// order of eagerly built SortKeys, ties in input order, for every term class, ASC / DESC,
/// bare variables and expressions, and k below, around and above the input size.
#[test]
fn lazy_sort_keys_agree_with_eager_sort_keys() {
    let fx = Fx::new();
    for seed in 1..=4u64 {
        let mut r = sq_core::testing::Rng(seed * 977);
        let rows = lazy_rows(&fx, &mut r, 1500);
        let keysets: Vec<Vec<(usize, bool, bool)>> = vec![
            // (column, desc, as an expression rather than a bare variable)
            vec![(0, true, false), (1, false, false)],
            vec![(1, false, false), (0, true, false), (2, false, false)],
            vec![(2, true, false), (1, true, true)],
            vec![(0, false, true), (2, false, false), (1, true, false)],
            vec![(1, true, false)],
        ];
        for ks in keysets {
            let keys: Vec<OrderKey> = ks
                .iter()
                .map(|&(col, desc, expr)| {
                    let e = if expr { Expr::Coalesce(vec![sv(col as Slot)]) } else { sv(col as Slot) };
                    order_key(e, desc)
                })
                .collect();
            let mut want = rows.clone();
            want.sort_by(|a, b| {
                ks.iter()
                    .map(|&(col, desc, _)| {
                        let o = ref_key(&fx, a[col]).cmp(&ref_key(&fx, b[col]));
                        if desc { o.reverse() } else { o }
                    })
                    .find(|o| *o != std::cmp::Ordering::Equal)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a[3].as_int().cmp(&b[3].as_int()))
            });
            // ERROR (an erroring group key) is output as unbound.
            want.iter_mut().flatten().filter(|id| **id == TermId::ERROR).for_each(|id| *id = TermId::UNBOUND);
            let input = || values(&[0, 1, 2, 3], rows.clone());
            let sorted = Q::new(&fx.store, 4, plan(Op::Sort { input: Box::new(input()), keys: keys.clone() })).rows(fx.rc());
            assert_eq!(sorted, want, "Sort, seed {seed}, keys {ks:?}");
            for k in [1u64, 3, 10, 100, 1024, 1500, 1600] {
                let topk = plan(Op::TopK { input: Box::new(input()), keys: keys.clone(), k });
                let got = Q::new(&fx.store, 4, topk).rows(fx.rc());
                assert_eq!(got, want[..(k as usize).min(want.len())], "TopK k={k}, seed {seed}, keys {ks:?}");
            }
        }
    }
}

/// `ORDER BY DESC(?int) ?iri LIMIT 10`: the IRI keys are built only for rows that tie on the
/// integer at the heap boundary, not for every row (they used to be decoded for all rows).
#[test]
fn topk_builds_secondary_keys_only_for_ties() {
    let fx = Fx::new();
    let n = 5000i64;
    let rows: Vec<Vec<TermId>> =
        (0..n).map(|i| vec![int((i * 7919) % n), fx.iri(&format!("x{}", (i * 31) % 97))]).collect();
    let keys = vec![order_key(sv(0), true), order_key(sv(1), false)];
    let topk = plan(Op::TopK { input: Box::new(values(&[0, 1], rows.clone())), keys: keys.clone(), k: 10 });
    let mut e = Q::new(&fx.store, 2, topk).exec(fx.rc());
    let got = collect(&mut e).unwrap();
    let mut want = rows;
    want.sort_by(|a, b| b[0].as_int().cmp(&a[0].as_int()));
    assert_eq!(got, want[..10]);
    let p = e.profile();
    assert_eq!(p.decode_hits + p.decode_misses, 0, "distinct integers decide every comparison");
    // Every row ties on the first key: then the IRIs must be compared (and built lazily).
    let rows: Vec<Vec<TermId>> = (0..300).map(|i| vec![int(1), fx.iri(&format!("y{}", (i * 31) % 97))]).collect();
    let topk = plan(Op::TopK { input: Box::new(values(&[0, 1], rows)), keys, k: 3 });
    let got = Q::new(&fx.store, 2, topk).rows(fx.rc());
    // "y0" sorts first; it is the IRI of rows 0, 97, 194 and 291.
    assert_eq!(got, vec![vec![int(1), fx.iri("y0")]; 3]);
}
