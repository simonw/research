//! Adversarial review (round 3) of the executor: large inputs that cross batch boundaries,
//! operator equivalences on hand-built plans, and resource handling.
use rustc_hash::FxHashSet;
use sq_core::ir::{AggFunc, AggSpec, ExistsId, PathPlan};
use sq_core::testing::Rng;

use super::*;

fn exists(id: u32) -> Expr {
    Expr::Exists(ExistsId(id))
}

fn count_star() -> AggSpec {
    AggSpec { func: AggFunc::CountAll, distinct: false, arg: None }
}

/// A streaming `FILTER EXISTS` keeps nothing per row: its memo is a cache. Memoizing every
/// distinct correlated tuple without bound (charged against the budget and only released at
/// the end of the execution) makes a plain streaming filter fail with MemoryBudget once the
/// outer input has enough distinct subjects (about 13K per MB here, ~13M under the default
/// 1 GB budget), although no operator materializes anything.
#[test]
fn review_exists_memo_does_not_fail_a_streaming_filter() {
    let fx = Fx::new();
    let (p, q) = (fx.iri("p"), fx.iri("q"));
    let n = 40_000i64;
    let mut quads = Vec::new();
    for i in 0..n {
        let s = fx.iri(&format!("s{i}"));
        quads.push([TermId::DEFAULT_GRAPH, s, p, int(i)]);
        if i % 2 == 0 {
            quads.push([TermId::DEFAULT_GRAPH, s, q, int(i)]);
        }
    }
    fx.store.insert(&mut quads).unwrap();
    // SELECT * { ?0 :p ?1 FILTER EXISTS { ?0 :q ?2 } }
    let root = filter(scan(v(0), c(p), v(1)), exists(0));
    let mut q0 = Q::new(&fx.store, 3, root).exists(vec![scan(v(0), c(q), v(2))]);
    q0.opts.max_memory_mb = 1;
    let mut e = q0.exec(fx.rc());
    let r = e.count();
    assert!(
        matches!(r, Ok(m) if m == (n / 2) as u64),
        "a streaming FILTER EXISTS must not run out of memory: {r:?} (memory in use {})",
        e.memory_used()
    );
}

/// Random rows over mixed term kinds (ints, IRIs, strings, unbound) with many ties.
fn mixed_rows(fx: &Fx, r: &mut Rng, n: usize) -> Vec<Vec<TermId>> {
    let iris: Vec<TermId> = (0..4).map(|i| fx.iri(&format!("k{i}"))).collect();
    let lits: Vec<TermId> = ["alpha", "a much longer literal", "b"].iter().map(|s| fx.lit(s)).collect();
    let key = |r: &mut Rng, ints_only: bool| -> TermId {
        if ints_only {
            return int(r.below(7) as i64 - 3);
        }
        match r.below(10) {
            0..=5 => int(r.below(7) as i64 - 3),
            6 => iris[r.below(4) as usize],
            7 => lits[r.below(3) as usize],
            8 => fx.typed("2.5", "decimal"),
            _ => u(),
        }
    };
    // The first rows are all ints (INT mode), later ones mixed (leaves INT mode mid-stream).
    (0..n)
        .map(|i| {
            let ints = i < n / 2;
            vec![key(r, ints), key(r, ints), int(i as i64)]
        })
        .collect()
}

/// TopK{k} must produce exactly the first k rows of the stable Sort, for ASC / DESC keys,
/// one or two keys, inputs that switch from all-INT to mixed values, and k around the batch
/// size.
#[test]
fn review_topk_equals_stable_sort_prefix_on_large_mixed_inputs() {
    let fx = Fx::new();
    for seed in 1..=6u64 {
        let mut r = Rng(seed * 31);
        let rows = mixed_rows(&fx, &mut r, 2600);
        for keys in [
            vec![order_key(sv(0), true)],
            vec![order_key(sv(0), false), order_key(sv(1), true)],
            vec![order_key(sv(1), true), order_key(sv(0), true)],
        ] {
            for k in [1u64, 7, 1023, 1025, 3000] {
                let input = || values(&[0, 1, 2], rows.clone());
                let topk = plan(Op::TopK { input: Box::new(input()), keys: keys.clone(), k });
                let sort = slice(plan(Op::Sort { input: Box::new(input()), keys: keys.clone() }), 0, Some(k));
                let a = Q::new(&fx.store, 3, topk).rows(fx.rc());
                let b = Q::new(&fx.store, 3, sort).rows(fx.rc());
                assert_eq!(a, b, "seed {seed}, k {k}, keys {keys:?}");
            }
        }
    }
}

/// A store with `n` subjects; :p on all of them (two objects each), :q on every `step`-th
/// subject (three objects each), :r on every `step2`-th.
fn skewed(n: i64, step: i64, step2: i64) -> (Fx, [TermId; 3]) {
    let fx = Fx::new();
    let [p, q, rr] = ["p", "q", "r"].map(|x| fx.iri(x));
    let mut quads = Vec::new();
    for i in 0..n {
        let s = fx.iri(&format!("s{i:05}"));
        for j in 0..2 {
            quads.push([TermId::DEFAULT_GRAPH, s, p, int(j)]);
        }
        if i % step == 0 {
            for j in 0..3 {
                quads.push([TermId::DEFAULT_GRAPH, s, q, int(10 + j)]);
            }
        }
        if i % step2 == 0 {
            quads.push([TermId::DEFAULT_GRAPH, s, rr, int(i)]);
        }
    }
    fx.store.insert(&mut quads).unwrap();
    (fx, [p, q, rr])
}

fn scan_ord(s: PatTerm, p: PatTerm, o: PatTerm, order: IndexOrder) -> Plan {
    scan_order(GraphSel::Default, s, p, o, order)
}

/// MergeJoin with galloping seeks on both Scan inputs (sparse overlap, runs that cross batch
/// boundaries) and StarMerge over three inputs equal the hash joins, and their factorized
/// counts equal the iterated ones.
#[test]
fn review_merge_and_star_merge_match_hash_joins_on_large_skewed_inputs() {
    let (fx, [p, q, rr]) = skewed(6000, 37, 5);
    let sp = || scan_ord(v(0), c(p), v(1), IndexOrder::Gpso);
    let sq = || scan_ord(v(0), c(q), v(2), IndexOrder::Gpso);
    let sr = || scan_ord(v(0), c(rr), v(3), IndexOrder::Gpso);
    for gallop in [0u32, 1, 6, 10] {
        let mj = merge_join(sq(), sp(), 0, &[], gallop);
        let hj = hash_join(sq(), sp(), &[0], &[], JoinKind::Inner, false);
        let a = Q::new(&fx.store, 4, mj).sorted(fx.rc());
        let b = Q::new(&fx.store, 4, hj).sorted(fx.rc());
        assert_eq!(a.len(), 163 * 6);
        assert_eq!(a, b, "gallop {gallop}");
        let mut e = Q::new(&fx.store, 4, merge_join(sp(), sq(), 0, &[], gallop)).exec(fx.rc());
        assert_eq!(e.count().unwrap(), (163 * 6) as u64);
    }
    let star = plan(Op::StarMerge { key: 0, inputs: vec![sp(), sq(), sr()], check: vec![] });
    let hj =
        hash_join(sr(), hash_join(sq(), sp(), &[0], &[], JoinKind::Inner, false), &[0], &[], JoinKind::Inner, false);
    let a = Q::new(&fx.store, 4, star.clone()).sorted(fx.rc());
    let b = Q::new(&fx.store, 4, hj).sorted(fx.rc());
    // Subjects divisible by 37 and by 5: 0, 185, ... (33 of them), 2 x 3 x 1 rows each.
    assert_eq!(a.len(), 33 * 6);
    assert_eq!(a, b);
    assert_eq!(Q::new(&fx.store, 4, star).exec(fx.rc()).count().unwrap(), 33 * 6);
}

/// BindJoin (OPTIONAL with a filter on the merged row, run reuse for equal keys, sorted left
/// batches) equals the hash and nested-loop left joins when the output crosses many batches.
#[test]
fn review_bind_join_left_filter_matches_hash_join_across_batches() {
    let (fx, [p, q, _]) = skewed(3000, 3, 7);
    // left: ?0 :p ?1 (two rows per subject, so every seed key is probed for a run of two)
    // right: ?0 :q ?2; filter on the merged row mixes a left and a right slot.
    let f = Expr::Less(Box::new(sv(1)), Box::new(Expr::Subtract(Box::new(sv(2)), Box::new(cv(int(11))))));
    let lp = || scan_ord(v(0), c(p), v(1), IndexOrder::Gpso);
    for sort_left in [false, true] {
        let bj = plan(Op::BindJoin {
            left: Box::new(lp()),
            right: Box::new(scan_bound(GraphSel::Default, v(0), c(q), v(2), &[0])),
            seed: vec![0],
            kind: left(Some(f.clone())),
            sort_left,
            adaptive: false,
        });
        let hj = hash_join(scan(v(0), c(q), v(2)), lp(), &[0], &[], left(Some(f.clone())), false);
        let nl = nested(lp(), scan(v(0), c(q), v(2)), &[0], left(Some(f.clone())));
        let a = Q::new(&fx.store, 3, bj).sorted(fx.rc());
        let b = Q::new(&fx.store, 3, hj).sorted(fx.rc());
        let c3 = Q::new(&fx.store, 3, nl).sorted(fx.rc());
        assert!(a.len() > 3 * 1024);
        assert_eq!(a, b, "sort_left {sort_left}");
        assert_eq!(a, c3, "sort_left {sort_left}");
    }
}

/// Streaming and hash aggregation agree when groups straddle input batches.
#[test]
fn review_streaming_group_matches_hash_group_across_batches() {
    let (fx, [p, q, _]) = skewed(2500, 2, 3);
    // ?0 :p ?1 . ?0 :q ?2 sorted by ?0 via a merge join; group by ?0 (6 rows per group).
    let input = || {
        merge_join(
            scan_ord(v(0), c(p), v(1), IndexOrder::Gpso),
            scan_ord(v(0), c(q), v(2), IndexOrder::Gpso),
            0,
            &[],
            6,
        )
    };
    let aggs = vec![
        (3, count_star()),
        (4, AggSpec { func: AggFunc::Sum, distinct: false, arg: Some(sv(2)) }),
        (5, AggSpec { func: AggFunc::Count, distinct: true, arg: Some(sv(1)) }),
    ];
    let g = |streaming| plan(Op::Group { input: Box::new(input()), keys: vec![0], aggs: aggs.clone(), streaming });
    let a = Q::new(&fx.store, 6, g(true)).sorted(fx.rc());
    let b = Q::new(&fx.store, 6, g(false)).sorted(fx.rc());
    assert_eq!(a.len(), 1250);
    assert_eq!(a, b);
    assert!(a.iter().all(|r| r[3] == int(6) && r[4] == int(66) && r[5] == int(2)));
}

/// The default graph merged from several FROM graphs, and the union default graph, are SETS
/// of triples even when the sources are read in chunks and the duplicates fall on chunk
/// boundaries; counts agree with iteration.
#[test]
fn review_merged_default_graphs_are_sets_across_chunks() {
    let fx = Fx::new();
    let p = fx.iri("p");
    let gs: Vec<TermId> = (0..3).map(|i| fx.iri(&format!("g{i}"))).collect();
    let mut quads = Vec::new();
    let mut expect = FxHashSet::default();
    let mut r = Rng(99);
    for i in 0..1500 {
        let s = fx.iri(&format!("s{}", r.below(700)));
        let o = int((i % 13) as i64);
        // Each triple in one to three graphs, and sometimes in the store's default graph.
        for (k, g) in gs.iter().enumerate() {
            if k == 0 || r.below(2) == 0 {
                quads.push([*g, s, p, o]);
            }
        }
        if r.below(3) == 0 {
            quads.push([TermId::DEFAULT_GRAPH, s, p, o]);
        }
        expect.insert(vec![s, o]);
    }
    fx.store.insert(&mut quads).unwrap();
    let named = fx.store.named_graphs().unwrap();
    let mut want: Vec<Vec<TermId>> = expect.into_iter().collect();
    want.sort();
    // FROM g0 FROM g1 FROM g2 (a 3-way merge), and the union default graph (the store default
    // graph merged with the named-only index); every triple is in g0, so both are `want`.
    for default in [DefaultGraph::Graphs(gs.clone().into()), DefaultGraph::UnionAll] {
        let q0 = Q::new(&fx.store, 2, scan(v(0), c(p), v(1))).dataset(default.clone(), NamedGraphs::All, named.clone());
        assert_eq!(ms(q0.rows(fx.rc())), want, "{default:?}");
        let q0 = Q::new(&fx.store, 2, scan(v(0), c(p), v(1))).dataset(default.clone(), NamedGraphs::All, named.clone());
        assert_eq!(q0.exec(fx.rc()).count().unwrap(), want.len() as u64, "{default:?}");
    }
}

/// `?x :p+ ?y` and `?x :p* ?y` with both ends free over a chain longer than a batch and a
/// candidate chunk: n(n-1)/2 and n(n+1)/2 pairs, each exactly once.
#[test]
fn review_closures_with_free_ends_over_long_chains() {
    let fx = Fx::new();
    let p = fx.iri("p");
    let n = 400i64;
    let nodes: Vec<TermId> = (0..n).map(|i| fx.iri(&format!("n{i}"))).collect();
    let mut quads: Vec<[TermId; 4]> =
        (0..n as usize - 1).map(|i| [TermId::DEFAULT_GRAPH, nodes[i], p, nodes[i + 1]]).collect();
    fx.store.insert(&mut quads).unwrap();
    let path = |pp: PathPlan| plan(Op::Path { g: GraphSel::Default, s: v(0), path: pp, o: v(1) });
    let plus = PathPlan::OneOrMore(Box::new(PathPlan::Pred(p)));
    let star = PathPlan::ZeroOrMore(Box::new(PathPlan::Pred(p)));
    let rows = Q::new(&fx.store, 2, path(plus)).rows(fx.rc());
    assert_eq!(rows.len() as i64, n * (n - 1) / 2);
    assert_eq!(rows.iter().collect::<FxHashSet<_>>().len(), rows.len());
    let rows = Q::new(&fx.store, 2, path(star)).rows(fx.rc());
    assert_eq!(rows.len() as i64, n * (n + 1) / 2);
}

/// OFFSET / LIMIT windows that start and end inside later batches.
#[test]
fn review_slice_windows_across_batches() {
    let fx = Fx::new();
    let p = fx.iri("p");
    let mut quads: Vec<[TermId; 4]> =
        (0..5000).map(|i| [TermId::DEFAULT_GRAPH, fx.iri(&format!("s{i:05}")), p, int(i)]).collect();
    fx.store.insert(&mut quads).unwrap();
    let all = Q::new(&fx.store, 2, scan_ord(v(0), c(p), v(1), IndexOrder::Gpso)).rows(fx.rc());
    for (off, lim) in [(1023u64, Some(2u64)), (1024, Some(1024)), (2047, Some(2050)), (4990, Some(100)), (3000, None)] {
        let s = slice(scan_ord(v(0), c(p), v(1), IndexOrder::Gpso), off, lim);
        let got = Q::new(&fx.store, 2, s).rows(fx.rc());
        let end = lim.map_or(all.len(), |l| (off + l).min(all.len() as u64) as usize);
        assert_eq!(got, all[off as usize..end].to_vec(), "offset {off} limit {lim:?}");
    }
}

/// ADDENDUM 2 (f): a GROUP BY expression that errors and one whose argument is unbound give
/// the same key (one group, output unbound).
#[test]
fn review_error_and_unbound_group_keys_form_one_group() {
    let fx = Fx::new();
    let a = fx.iri("a");
    // BIND(1/?1 AS ?2) GROUP BY ?2: ?1 = 0 and ?1 = <x> error, ?1 unbound gives an unbound
    // argument; 1/2 and 1/4 are two more groups.
    let rows = vec![vec![a, int(0)], vec![a, int(2)], vec![a, fx.iri("x")], vec![a, u()], vec![a, int(4)]];
    let key = Expr::Divide(Box::new(cv(int(1))), Box::new(sv(1)));
    let ext = extend(values(&[0, 1], rows), 2, key);
    let g = plan(Op::Group { input: Box::new(ext), keys: vec![2], aggs: vec![(3, count_star())], streaming: false });
    let got = ms(Q::new(&fx.store, 4, project(g, &[2, 3])).rows(fx.rc()));
    assert_eq!(got.len(), 3, "{got:?}");
    let unbound_groups: Vec<_> = got.iter().filter(|r| !r[2].is_bound()).collect();
    assert_eq!(unbound_groups.len(), 1, "{got:?}");
    assert_eq!(unbound_groups[0][3], int(3));
}

/// The path step's probe-or-scan decision (§5.5: probe while |starts| x seek_sorted <
/// |edges(p)| x scan_row) only needs to know whether |edges(p)| exceeds |starts| x
/// seek_sorted / scan_row (about 9 x |starts| with the SQL cost constants). Counting every p
/// edge exactly is an O(|edges(p)|) VDBE count (~40 ns/row: 0.4 s for 10M edges) to choose
/// between strategies that cost ~0.1 ms here; it should be a capped count (spec §7.4: runtime
/// counts are bounded).
#[test]
fn review_path_strategy_choice_does_not_count_every_edge() {
    let fx = Fx::new();
    let [a, p, q] = ["a", "p", "q"].map(|n| fx.iri(n));
    let mut quads = Vec::new();
    // :a :q m_i (100 middles), m_i :p :h; plus 20,000 unrelated :p edges.
    let h = fx.iri("h");
    for i in 0..100 {
        let m = fx.iri(&format!("m{i}"));
        quads.push([TermId::DEFAULT_GRAPH, a, q, m]);
        quads.push([TermId::DEFAULT_GRAPH, m, p, h]);
    }
    for i in 0..20_000 {
        quads.push([TermId::DEFAULT_GRAPH, fx.iri(&format!("x{i}")), p, int(i)]);
    }
    fx.store.insert(&mut quads).unwrap();
    // :a :q/:p ?y
    let path = PathPlan::Seq(Box::new(PathPlan::Pred(q)), Box::new(PathPlan::Pred(p)));
    let root = plan(Op::Path { g: GraphSel::Default, s: c(a), path, o: v(0) });
    let ts = Rc::new(TestStore::new(fx.store.clone()));
    let rows = Q::new(&fx.store, 1, root).rows(ts.clone());
    assert_eq!(rows.len(), 100);
    assert_eq!(ts.uncapped.get(), 0, "an exact count of all 20,100 :p edges was issued to pick a strategy");
}

/// The exact edge count of `review_path_strategy_choice_does_not_count_every_edge` is cached
/// in the path's scans, but closing the operator tree drops it, and every EXISTS evaluation
/// closes its tree: a correlated `FILTER EXISTS { ?s :q/:p ?y }` repeats the O(|edges(p)|)
/// count for every outer row (memo miss).
#[test]
fn review_path_edge_count_is_repeated_per_exists_evaluation() {
    let fx = Fx::new();
    let [p, q] = ["p", "q"].map(|n| fx.iri(n));
    let h = fx.iri("h");
    let mut quads = Vec::new();
    let subjects: Vec<TermId> = (0..5).map(|i| fx.iri(&format!("a{i}"))).collect();
    for (k, a) in subjects.iter().enumerate() {
        for i in 0..100 {
            let m = fx.iri(&format!("m{k}_{i}"));
            quads.push([TermId::DEFAULT_GRAPH, *a, q, m]);
            quads.push([TermId::DEFAULT_GRAPH, m, p, h]);
        }
    }
    for i in 0..20_000 {
        quads.push([TermId::DEFAULT_GRAPH, fx.iri(&format!("x{i}")), p, int(i)]);
    }
    fx.store.insert(&mut quads).unwrap();
    let path = PathPlan::Seq(Box::new(PathPlan::Pred(q)), Box::new(PathPlan::Pred(p)));
    let ex = plan(Op::Path { g: GraphSel::Default, s: v(0), path, o: v(1) });
    let outer = values(&[0], subjects.iter().map(|s| vec![*s]).collect());
    let ts = Rc::new(TestStore::new(fx.store.clone()));
    let rows = Q::new(&fx.store, 2, filter(outer, exists(0))).exists(vec![ex]).rows(ts.clone());
    assert_eq!(rows.len(), 5);
    assert!(ts.uncapped.get() <= 1, "{} exact counts of every :p edge for 5 EXISTS evaluations", ts.uncapped.get());
}

/// §5.3 Scan: a re-opened scan whose planned prefix is satisfied is `rebind`-ed (no cursor
/// lease, no allocation). A BindJoin probe works that way, but every correlated EXISTS
/// evaluation closes the pattern's operator tree, so each outer row (memo miss) leases and
/// boxes a fresh cursor for every scan of the pattern instead of rebinding the one it had.
#[test]
fn review_exists_scans_rebind_instead_of_leasing_a_cursor_per_row() {
    let fx = Fx::new();
    let p = fx.iri("p");
    let subjects: Vec<TermId> = (0..1000).map(|i| fx.iri(&format!("s{i}"))).collect();
    let mut quads: Vec<[TermId; 4]> =
        subjects.iter().step_by(2).map(|s| [TermId::DEFAULT_GRAPH, *s, p, int(1)]).collect();
    fx.store.insert(&mut quads).unwrap();
    let outer = || values(&[0], subjects.iter().map(|s| vec![*s]).collect());
    // The same correlated lookup as a BindJoin semi join: one cursor, rebound per probe.
    let ts = Rc::new(TestStore::new(fx.store.clone()));
    let bj = bind_join(outer(), scan_bound(GraphSel::Default, v(0), c(p), v(1), &[0]), &[0], JoinKind::Semi);
    assert_eq!(Q::new(&fx.store, 2, bj).rows(ts.clone()).len(), 500);
    let bind_join_cursors = ts.cursors.get();
    // FILTER EXISTS { ?0 :p ?1 }
    let ts = Rc::new(TestStore::new(fx.store.clone()));
    let rows = Q::new(&fx.store, 2, filter(outer(), exists(0))).exists(vec![scan(v(0), c(p), v(1))]).rows(ts.clone());
    assert_eq!(rows.len(), 500);
    assert!(
        ts.cursors.get() <= bind_join_cursors + 1,
        "EXISTS leased {} cursors for 1000 outer rows (BindJoin: {bind_join_cursors})",
        ts.cursors.get()
    );
}

/// Path steps from more than 64 starts over a predicate with few edges use one scan of all
/// the edges semi-joined with the starts (forward on GPSO, backward on GPOS); the pairs must
/// equal the probe evaluation's, multiplicities included.
#[test]
fn review_path_full_scan_strategy_matches_probes() {
    let fx = Fx::new();
    let p = fx.iri("p");
    let n = 300usize;
    let nodes: Vec<TermId> = (0..n).map(|i| fx.iri(&format!("n{i:03}"))).collect();
    let mut edges = Vec::new();
    let mut r = Rng(7);
    for _ in 0..450 {
        let (a, b) = (r.below(n as u64) as usize, r.below(n as u64) as usize);
        edges.push((a, b));
    }
    edges.sort();
    edges.dedup();
    let mut quads: Vec<[TermId; 4]> =
        edges.iter().map(|(a, b)| [TermId::DEFAULT_GRAPH, nodes[*a], p, nodes[*b]]).collect();
    fx.store.insert(&mut quads).unwrap();
    // Expected `?x p/p ?y` and `?x ^p/^p ?y` (bags) from the edge list.
    let mut fwd = Vec::new();
    let mut bwd = Vec::new();
    for (a, b) in &edges {
        for (c2, d) in &edges {
            if b == c2 {
                fwd.push(vec![nodes[*a], nodes[*d]]);
            }
            if a == d {
                bwd.push(vec![nodes[*b], nodes[*c2]]);
            }
        }
    }
    let pred = || Box::new(PathPlan::Pred(p));
    let inv = || Box::new(PathPlan::Inv(pred()));
    for (pp, want) in [(PathPlan::Seq(pred(), pred()), fwd), (PathPlan::Seq(inv(), inv()), bwd)] {
        let root = plan(Op::Path { g: GraphSel::Default, s: v(0), path: pp.clone(), o: v(1) });
        let got = ms(Q::new(&fx.store, 2, root).rows(fx.rc()));
        assert_eq!(got, ms(want), "{pp:?}");
    }
}
