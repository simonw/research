//! Property test: on random small graphs and random left-deep join trees, BindJoin, HashJoin
//! (both build sides), NestedLoop and MergeJoin (inputs behind SortBySlot enforcers) give the
//! same multiset of solutions, for inner joins and for a final OPTIONAL / semi / anti join.
use sq_core::testing::Rng;

use super::*;

const SLOTS: u16 = 5;

/// A random quad pattern over slots 0..SLOTS, with constants from the data.
fn random_pattern(r: &mut Rng, subjects: &[TermId], preds: &[TermId]) -> [PatTerm; 3] {
    let term = |r: &mut Rng, consts: &[TermId], const_odds: u64| {
        if r.below(const_odds) == 0 {
            PatTerm::Const(consts[r.below(consts.len() as u64) as usize])
        } else {
            PatTerm::Slot(r.below(SLOTS as u64) as Slot)
        }
    };
    let s = term(r, subjects, 6);
    let p = term(r, preds, 2);
    let o = term(r, subjects, 4);
    [s, p, o]
}

fn pattern_slots(t: &[PatTerm; 3]) -> Vec<Slot> {
    let mut v: Vec<Slot> = t.iter().filter_map(|x| if let PatTerm::Slot(s) = x { Some(*s) } else { None }).collect();
    v.sort();
    v.dedup();
    v
}

fn scan_of(t: &[PatTerm; 3], bound: &[Slot]) -> Plan {
    scan_bound(GraphSel::Default, t[0], t[1], t[2], bound)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Algo {
    Bind,
    HashBuildRight,
    HashBuildLeft,
    Nested,
    Merge,
}

/// Joins `left` (binding `lslots`, all certain) with pattern `t` using `algo`.
fn join(algo: Algo, left: Plan, lslots: &[Slot], t: &[PatTerm; 3], kind: JoinKind) -> Plan {
    let rslots = pattern_slots(t);
    let shared: Vec<Slot> = rslots.iter().copied().filter(|s| lslots.contains(s)).collect();
    let inner = kind == JoinKind::Inner;
    match algo {
        Algo::Bind if !shared.is_empty() => bind_join(left, scan_of(t, &shared), &shared, kind),
        Algo::HashBuildRight if !shared.is_empty() => hash_join(scan_of(t, &[]), left, &shared, &[], kind, false),
        Algo::HashBuildLeft if !shared.is_empty() => hash_join(left, scan_of(t, &[]), &shared, &[], kind, true),
        Algo::Merge if !shared.is_empty() && inner => {
            let key = shared[0];
            merge_join(sort_by(left, key), sort_by(scan_of(t, &[]), key), key, &shared[1..], 6)
        }
        _ => nested(left, scan_of(t, &[]), &shared, kind),
    }
}

fn random_store(seed: u64) -> (Fx, Vec<TermId>, Vec<TermId>) {
    let fx = Fx::new();
    let mut r = Rng(seed);
    let subjects: Vec<TermId> = (0..8).map(|i| fx.iri(&format!("s{i}"))).collect();
    let preds: Vec<TermId> = (0..3).map(|i| fx.iri(&format!("p{i}"))).collect();
    let mut quads = Vec::new();
    for _ in 0..60 {
        let s = subjects[r.below(8) as usize];
        let p = preds[r.below(3) as usize];
        let o = if r.below(4) == 0 { int(r.below(3) as i64) } else { subjects[r.below(8) as usize] };
        quads.push([TermId::DEFAULT_GRAPH, s, p, o]);
    }
    fx.store.insert(&mut quads).unwrap();
    (fx, subjects, preds)
}

#[test]
fn join_algorithms_agree_on_random_join_trees() {
    let algos = [Algo::Bind, Algo::HashBuildRight, Algo::HashBuildLeft, Algo::Nested, Algo::Merge];
    let mut cases = 0;
    let mut nonempty = 0;
    for seed in 1..=60u64 {
        let (fx, subjects, preds) = random_store(seed * 7919);
        let mut r = Rng(seed);
        let n = 2 + r.below(3) as usize;
        let pats: Vec<[PatTerm; 3]> = (0..n).map(|_| random_pattern(&mut r, &subjects, &preds)).collect();
        let last_kind = match r.below(4) {
            0 => JoinKind::Inner,
            1 => left(None),
            2 => JoinKind::Semi,
            _ => JoinKind::Anti,
        };
        let mut results: Vec<(Algo, Vec<Vec<TermId>>)> = Vec::new();
        for algo in algos {
            let mut p = scan_of(&pats[0], &[]);
            let mut bound = pattern_slots(&pats[0]);
            for (i, t) in pats.iter().enumerate().skip(1) {
                let kind = if i == n - 1 { last_kind.clone() } else { JoinKind::Inner };
                p = join(algo, p, &bound, t, kind.clone());
                if kind == JoinKind::Inner {
                    bound.extend(pattern_slots(t));
                    bound.sort();
                    bound.dedup();
                }
            }
            results.push((algo, Q::new(&fx.store, SLOTS as usize, p).sorted(fx.rc())));
        }
        let (_, base) = &results[0];
        for (algo, rows) in &results[1..] {
            assert_eq!(rows, base, "seed {seed}: {algo:?} differs from BindJoin for {pats:?} ({last_kind:?})");
        }
        cases += 1;
        nonempty += usize::from(!base.is_empty());
    }
    assert_eq!(cases, 60);
    assert!(nonempty >= 15, "the generator produces non-trivial joins ({nonempty})");
}

#[test]
fn merge_join_fast_count_equals_iterated_count() {
    for seed in 1..=30u64 {
        let (fx, subjects, preds) = random_store(seed * 104_729);
        let mut r = Rng(seed + 1000);
        let a = random_pattern(&mut r, &subjects, &preds);
        let b = random_pattern(&mut r, &subjects, &preds);
        let (sa, sb) = (pattern_slots(&a), pattern_slots(&b));
        let shared: Vec<Slot> = sa.iter().copied().filter(|s| sb.contains(s)).collect();
        let Some(&key) = shared.first() else { continue };
        let mj = || merge_join(sort_by(scan_of(&a, &[]), key), sort_by(scan_of(&b, &[]), key), key, &shared[1..], 6);
        let iterated = Q::new(&fx.store, SLOTS as usize, mj()).rows(fx.rc()).len() as u64;
        let mut e = Q::new(&fx.store, SLOTS as usize, mj()).exec(fx.rc());
        assert_eq!(e.count().unwrap(), iterated, "seed {seed}");
        let count = plan(Op::Count { input: Box::new(mj()), out: 0 });
        let got = Q::new(&fx.store, SLOTS as usize, count).form(QueryForm::Select(vec![0])).rows(fx.rc());
        assert_eq!(got, vec![vec![int(iterated as i64)]], "seed {seed}");
    }
}
