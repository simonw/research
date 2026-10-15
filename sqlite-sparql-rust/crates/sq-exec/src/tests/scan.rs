//! Scan: bound positions, repeated variables, TEMP constants, IntRange, fast counts and
//! every graph-selection mode.
use super::*;
use sq_core::ir::{DefaultGraph, NamedGraphs};
use sq_core::plan::IntRange;

/// :a :knows :b, :b :knows :c, :c :knows :a, :d :knows :d, :a :name "Alice".
fn social() -> (Fx, [TermId; 6]) {
    let fx = Fx::new();
    let (a, b, cc, d, knows, name) =
        (fx.iri("a"), fx.iri("b"), fx.iri("c"), fx.iri("d"), fx.iri("knows"), fx.iri("name"));
    fx.add(a, knows, b);
    fx.add(b, knows, cc);
    fx.add(cc, knows, a);
    fx.add(d, knows, d);
    let alice = fx.lit("Alice");
    fx.add(a, name, alice);
    (fx, [a, b, cc, d, knows, name])
}

#[test]
fn free_and_constant_positions() {
    let (fx, [a, b, cc, d, knows, _]) = social();
    let rows = Q::new(&fx.store, 2, scan(v(0), c(knows), v(1))).sorted(fx.rc());
    assert_eq!(rows, ms(vec![vec![a, b], vec![b, cc], vec![cc, a], vec![d, d]]));
    let rows = Q::new(&fx.store, 1, scan(c(a), c(knows), v(0))).rows(fx.rc());
    assert_eq!(rows, vec![vec![b]]);
    // Everything bound: one empty-width match, or nothing.
    assert_eq!(Q::new(&fx.store, 0, scan(c(a), c(knows), c(b))).rows(fx.rc()).len(), 1);
    assert_eq!(Q::new(&fx.store, 0, scan(c(b), c(knows), c(a))).rows(fx.rc()).len(), 0);
}

#[test]
fn output_is_sorted_by_the_first_free_column() {
    let (fx, [a, b, cc, d, knows, _]) = social();
    // Gpso: subjects ascending; Gpos (explicit order): objects ascending.
    let rows = Q::new(&fx.store, 2, scan(v(0), c(knows), v(1))).rows(fx.rc());
    let subj: Vec<TermId> = rows.iter().map(|r| r[0]).collect();
    assert_eq!(subj, vec![a, b, cc, d]);
    let p = scan_order(GraphSel::Default, v(0), c(knows), v(1), IndexOrder::Gpos);
    let rows = Q::new(&fx.store, 2, p).rows(fx.rc());
    let obj: Vec<TermId> = rows.iter().map(|r| r[1]).collect();
    assert_eq!(obj, vec![a, b, cc, d]);
}

#[test]
fn repeated_variable_is_post_checked() {
    let (fx, [_, _, _, d, knows, _]) = social();
    let rows = Q::new(&fx.store, 1, scan(v(0), c(knows), v(0))).rows(fx.rc());
    assert_eq!(rows, vec![vec![d]]);
    // ?x ?x ?y: the predicate equals the subject.
    let p = fx.iri("p");
    fx.add(p, p, d);
    let rows = Q::new(&fx.store, 2, scan(v(0), v(0), v(1))).rows(fx.rc());
    assert_eq!(rows, vec![vec![p, d]]);
}

#[test]
fn temp_constant_returns_nothing_without_touching_the_store() {
    let (fx, _) = social();
    let mut temps = TempDict::new();
    let absent = temps.intern(Term::Iri(format!("{EX}absent").into()));
    let ts = Rc::new(TestStore::new(fx.store.clone()));
    let store: Rc<dyn QuadStore> = ts.clone();
    let rows = Q::new(&fx.store, 2, scan(v(0), c(absent), v(1))).temps(temps).rows(store);
    assert!(rows.is_empty());
    assert_eq!(ts.cursors.get(), 0, "no cursor for a TEMP constant");
}

#[test]
fn count_uses_the_store_count_unless_rows_need_checks() {
    let (fx, [_, _, _, _, knows, _]) = social();
    let count = |p: Plan| plan(Op::Count { input: Box::new(p), out: 0 });
    let only0 = QueryForm::Select(vec![0]);
    let ts = Rc::new(TestStore::new(fx.store.clone()));
    let rows = Q::new(&fx.store, 3, count(scan(v(1), c(knows), v(2)))).form(only0.clone()).rows(ts.clone());
    assert_eq!(rows, vec![vec![int(4)]]);
    assert_eq!((ts.counts.get(), ts.cursors.get()), (1, 1), "fast path: one store count");
    // A repeated variable needs a post-check: iterate.
    let ts = Rc::new(TestStore::new(fx.store.clone()));
    let rows = Q::new(&fx.store, 3, count(scan(v(1), c(knows), v(1)))).form(only0).rows(ts.clone());
    assert_eq!(rows, vec![vec![int(1)]]);
    assert_eq!(ts.counts.get(), 0);
}

#[test]
fn int_range_reads_the_int_range_then_the_numeric_exceptions() {
    let fx = Fx::new();
    let (s, val) = (fx.iri("s"), fx.iri("val"));
    for i in -3..20 {
        fx.add(fx.iri(&format!("n{i}")), val, int(i));
    }
    let dec = fx.typed("7.5", "decimal");
    let dbl = fx.typed("8.0E0", "double");
    let noncanon = fx.typed("09", "integer");
    let text = fx.lit("12");
    for o in [dec, dbl, noncanon, text] {
        fx.add(s, val, o);
    }
    let range = |lo, hi| {
        plan(Op::Scan(ScanOp {
            pat: QuadPat { g: GraphSel::Default, s: v(0), p: c(val), o: v(1) },
            order: IndexOrder::Gpos,
            range: Some(IntRange { lo, hi }),
        }))
    };
    let objs = |rows: Vec<Vec<TermId>>| {
        let mut o: Vec<TermId> = rows.into_iter().map(|r| r[1]).collect();
        o.sort();
        o
    };
    let got = objs(Q::new(&fx.store, 2, range(5, 10)).rows(fx.rc()));
    let mut want: Vec<TermId> = (5..=10).map(int).collect();
    want.extend([dec, dbl, noncanon]);
    want.sort();
    assert_eq!(got, want, "ints in range + every numeric exception, no plain strings");
    // An empty int range still reads the exceptions.
    let got = objs(Q::new(&fx.store, 2, range(100, 50)).rows(fx.rc()));
    let mut want = vec![dec, dbl, noncanon];
    want.sort();
    assert_eq!(got, want);
    // Negative bounds: signed order.
    let got = objs(Q::new(&fx.store, 2, range(-2, 0)).rows(fx.rc()));
    assert!(got.contains(&int(-2)) && got.contains(&int(0)) && !got.contains(&int(-3)));
}

#[test]
fn seeded_scan_rebinds_and_carries_the_seed() {
    let (fx, [a, b, cc, _, knows, name]) = social();
    // VALUES ?x {a b} then ?x :knows ?y (seeded on ?x), the seed values are merged.
    let left = values(&[0], vec![vec![a], vec![b], vec![a]]);
    let right = scan_bound(GraphSel::Default, v(0), c(knows), v(1), &[0]);
    let ts = Rc::new(TestStore::new(fx.store.clone()));
    let rows = Q::new(&fx.store, 2, bind_join(left, right, &[0], JoinKind::Inner)).rows(ts.clone());
    assert_eq!(rows, vec![vec![a, b], vec![b, cc], vec![a, b]]);
    assert_eq!(ts.cursors.get(), 1, "one cursor, re-bound per probe");
    // A seed that makes the planned prefix unusable re-picks the order.
    let right = scan_order(GraphSel::Default, v(0), v(2), v(1), IndexOrder::Gpos);
    let left = values(&[0, 2], vec![vec![a, name]]);
    let rows = Q::new(&fx.store, 3, bind_join(left, right, &[0, 2], JoinKind::Inner)).rows(fx.rc());
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0], a);
}

/// Graph data: default {a p b}; g1 {a p b, a p c}; g2 {a p c, d p e}.
fn graphs() -> (Fx, [TermId; 8]) {
    let fx = Fx::new();
    let [a, b, cc, d, e, p, g1, g2] = ["a", "b", "c", "d", "e", "p", "g1", "g2"].map(|n| fx.iri(n));
    fx.add(a, p, b);
    fx.add_g(g1, a, p, b);
    fx.add_g(g1, a, p, cc);
    fx.add_g(g2, a, p, cc);
    fx.add_g(g2, d, p, e);
    (fx, [a, b, cc, d, e, p, g1, g2])
}

#[test]
fn default_graph_modes() {
    let (fx, [a, b, cc, d, e, p, g1, g2]) = graphs();
    let run = |default: DefaultGraph| {
        Q::new(&fx.store, 2, scan(v(0), c(p), v(1)))
            .dataset(default, NamedGraphs::Only(Arc::from(vec![])), vec![])
            .sorted(fx.rc())
    };
    assert_eq!(run(DefaultGraph::StoreDefault), vec![vec![a, b]]);
    // FROM g1 FROM g2: RDF merge, the shared triple once.
    let merged = run(DefaultGraph::Graphs(Arc::from(vec![g1, g2])));
    assert_eq!(merged, ms(vec![vec![a, b], vec![a, cc], vec![d, e]]));
    assert_eq!(run(DefaultGraph::Graphs(Arc::from(vec![g2]))), ms(vec![vec![a, cc], vec![d, e]]));
    assert!(run(DefaultGraph::Graphs(Arc::from(vec![]))).is_empty(), "FROM NAMED only: empty default graph");
    let mut temps = TempDict::new();
    let absent = temps.intern(Term::Iri(format!("{EX}nowhere").into()));
    assert!(run(DefaultGraph::Graphs(Arc::from(vec![absent]))).is_empty());
    // union_default_graph: g=0 merged with every named graph, deduplicated.
    let all = run(DefaultGraph::UnionAll);
    assert_eq!(all, ms(vec![vec![a, b], vec![a, cc], vec![d, e]]));
    // The merged output is still sorted by subject (and deduplicated adjacently).
    let rows = Q::new(&fx.store, 2, scan(v(0), c(p), v(1)))
        .dataset(DefaultGraph::UnionAll, NamedGraphs::All, vec![g1, g2])
        .rows(fx.rc());
    assert_eq!(rows, ms(rows.clone()));
}

#[test]
fn graph_slot_modes() {
    let (fx, [a, b, cc, d, e, p, g1, g2]) = graphs();
    let gscan = || scan_bound(GraphSel::Slot(2), v(0), c(p), v(1), &[]);
    // NamedGraphs::All: the partial g-last order binds the graph, never the default graph.
    let rows = Q::new(&fx.store, 3, gscan()).sorted(fx.rc());
    assert_eq!(rows, ms(vec![vec![a, b, g1], vec![a, cc, g1], vec![a, cc, g2], vec![d, e, g2]]));
    // FROM NAMED g2: per-graph cursors.
    let rows = Q::new(&fx.store, 3, gscan())
        .dataset(DefaultGraph::Graphs(Arc::from(vec![])), NamedGraphs::Only(Arc::from(vec![g2])), vec![g2])
        .sorted(fx.rc());
    assert_eq!(rows, ms(vec![vec![a, cc, g2], vec![d, e, g2]]));
    // More than 8 named graphs: partial cursor + membership.
    let mut many: Vec<TermId> = (0..9).map(|i| fx.iri(&format!("unused{i}"))).collect();
    many.push(g1);
    many.sort();
    let rows = Q::new(&fx.store, 3, gscan())
        .dataset(DefaultGraph::Graphs(Arc::from(vec![])), NamedGraphs::Only(Arc::from(many.clone())), many)
        .sorted(fx.rc());
    assert_eq!(rows, ms(vec![vec![a, b, g1], vec![a, cc, g1]]));
    // GRAPH <g2> (constant).
    let rows = Q::new(&fx.store, 2, scan_bound(GraphSel::Const(g2), v(0), c(p), v(1), &[])).sorted(fx.rc());
    assert_eq!(rows, ms(vec![vec![a, cc], vec![d, e]]));
    // GRAPH ?g { ?g ... }: the graph slot repeated in a position.
    fx.add_g(g1, g1, p, a);
    let rows = Q::new(&fx.store, 2, scan_bound(GraphSel::Slot(0), v(0), c(p), v(1), &[])).rows(fx.rc());
    assert_eq!(rows, vec![vec![g1, a]]);
    let rows = Q::new(&fx.store, 2, scan_bound(GraphSel::Slot(0), v(0), c(p), v(1), &[]))
        .dataset(DefaultGraph::StoreDefault, NamedGraphs::Only(Arc::from(vec![g1, g2])), vec![g1, g2])
        .rows(fx.rc());
    assert_eq!(rows, vec![vec![g1, a]]);
}

#[test]
fn seeded_graph_slot() {
    let (fx, [a, b, cc, _, _, p, g1, g2]) = graphs();
    let other = fx.iri("not_a_graph");
    let left = values(&[2], vec![vec![g1], vec![other], vec![g2]]);
    let right = scan_bound(GraphSel::Slot(2), v(0), c(p), v(1), &[2]);
    let rows = Q::new(&fx.store, 3, bind_join(left, right, &[2], JoinKind::Inner)).sorted(fx.rc());
    let d = fx.lookup("d").unwrap();
    let e = fx.lookup("e").unwrap();
    assert_eq!(rows, ms(vec![vec![a, b, g1], vec![a, cc, g1], vec![a, cc, g2], vec![d, e, g2]]));
}

#[test]
fn values_merge_and_filter_with_the_seed() {
    let (fx, [a, b, cc, ..]) = social();
    // VALUES (?0 ?1) {(a b) (a UNDEF) (c b)} seeded with ?0 = a.
    let vals = values(&[0, 1], vec![vec![a, b], vec![a, u()], vec![cc, b]]);
    let left = values(&[0], vec![vec![a]]);
    let rows = Q::new(&fx.store, 2, bind_join(left, vals, &[0], JoinKind::Inner)).rows(fx.rc());
    assert_eq!(rows, vec![vec![a, b], vec![a, u()]]);
}
