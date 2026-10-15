//! Reviewer tests (WP6 adversarial review). Each test pins a behaviour required by the work
//! package (exec.txt, including ADDENDUM 2, whose path semantics are those of the sq-oracle
//! differential ground truth) or by the spec, on hand-built plans over MemStore.
use std::cell::Cell;

use sq_core::ir::{AggFunc, AggSpec, ExistsId, PathPlan};
use sq_core::spargebra::algebra::Function;

use super::*;

fn path(s: PatTerm, p: PathPlan, o: PatTerm) -> Plan {
    plan(Op::Path { g: GraphSel::Default, s, path: p, o })
}

fn pred(p: TermId) -> PathPlan {
    PathPlan::Pred(p)
}

fn inv(p: PathPlan) -> PathPlan {
    PathPlan::Inv(Box::new(p))
}

fn seq(a: PathPlan, b: PathPlan) -> PathPlan {
    PathPlan::Seq(Box::new(a), Box::new(b))
}

fn alt(a: PathPlan, b: PathPlan) -> PathPlan {
    PathPlan::Alt(Box::new(a), Box::new(b))
}

fn plus(p: PathPlan) -> PathPlan {
    PathPlan::OneOrMore(Box::new(p))
}

fn star(p: PathPlan) -> PathPlan {
    PathPlan::ZeroOrMore(Box::new(p))
}

fn opt(p: PathPlan) -> PathPlan {
    PathPlan::ZeroOrOne(Box::new(p))
}

fn nps(set: Vec<TermId>) -> PathPlan {
    PathPlan::Nps(set)
}

/// a -p-> b, a -r-> b (two predicates between the same pair), a -p-> a, a -r-> a (two
/// self-loops), c -q-> d.
fn nps_data() -> (Fx, [TermId; 7]) {
    let fx = Fx::new();
    let [a, b, cc, d, p, q, r] = ["a", "b", "c", "d", "p", "q", "r"].map(|n| fx.iri(n));
    for (s, pp, o) in [(a, p, b), (a, r, b), (a, p, a), (a, r, a), (cc, q, d)] {
        fx.add(s, pp, o);
    }
    (fx, [a, b, cc, d, p, q, r])
}

/// ADDENDUM 2 (a): a negated property set yields a SET of (x, y) pairs ("there exists a
/// triple x p y with p not in the set"), so two predicates linking the same pair give one
/// solution, in every evaluation direction.
#[test]
fn review_negated_property_set_yields_a_set_of_pairs() {
    let (fx, [a, b, cc, _, _, q, _]) = nps_data();
    // :a !:q ?0 -> {a, b} (a p a / a r a and a p b / a r b each count once).
    let rows = Q::new(&fx.store, 1, path(c(a), nps(vec![q]), v(0))).sorted(fx.rc());
    assert_eq!(rows, ms(vec![vec![a], vec![b]]), "forward from a constant");
    // ?0 !:q :b -> {a} (backward).
    let rows = Q::new(&fx.store, 1, path(v(0), nps(vec![q]), c(b))).rows(fx.rc());
    assert_eq!(rows, vec![vec![a]], "backward from a constant");
    // :b ^!:q ?0 -> {a} (inverse set).
    let rows = Q::new(&fx.store, 1, path(c(b), inv(nps(vec![q])), v(0))).rows(fx.rc());
    assert_eq!(rows, vec![vec![a]], "inverse negated set");
    // ?0 !:q ?1 with both endpoints free -> {(a,a), (a,b)}.
    let rows = Q::new(&fx.store, 2, path(v(0), nps(vec![q]), v(1))).sorted(fx.rc());
    assert_eq!(rows, ms(vec![vec![a, a], vec![a, b]]), "both endpoints free");
    // :a !:q :b (both bound) -> one solution.
    assert_eq!(Q::new(&fx.store, 0, path(c(a), nps(vec![q]), c(b))).rows(fx.rc()).len(), 1, "both bound");
    // Sanity: the excluded predicate is excluded.
    assert!(Q::new(&fx.store, 1, path(c(cc), nps(vec![q]), v(0))).rows(fx.rc()).is_empty());
}

/// ADDENDUM 2 (a)+(b): `!(:q|^:q)` lowers to Alt(Nps, Inv(Nps)); each side is a set and the
/// alternative is a bag union, so the self-loop of `a` (via :p and via :r) counts exactly
/// twice: once found forward, once found backward.
#[test]
fn review_negated_set_both_directions_counts_a_self_loop_twice() {
    let (fx, [a, _, _, _, _, q, _]) = nps_data();
    let both = alt(nps(vec![q]), inv(nps(vec![q])));
    let rows = Q::new(&fx.store, 1, path(c(a), both, c(a))).rows(fx.rc());
    assert_eq!(rows.len(), 2, "one solution per direction, not one per triple: {rows:?}");
}

/// The graph of the sequence tests: x -p-> y, y -q-> z. `zz` is in the dictionary but in no
/// triple, so it is not a node of the graph.
fn seq_data() -> (Fx, [TermId; 7]) {
    let fx = Fx::new();
    let [x, y, z, p, q, r, zz] = ["x", "y", "z", "p", "q", "r", "zz"].map(|n| fx.iri(n));
    fx.add(x, p, y);
    fx.add(y, q, z);
    (fx, [x, y, z, p, q, r, zz])
}

/// ADDENDUM 2 (d): the middle of a nested sequence is a fresh Join-mode variable. From a
/// constant that is not a graph node, `p?` binds the middle to that constant, but the second
/// operand `q?` (other end free) only has a zero-length match on graph nodes, so
/// `:zz (:p?/:q?)+ ?y` is EMPTY (the oracle's documented consequence).
#[test]
fn review_sequence_middle_is_a_join_variable_under_closure() {
    let (fx, [x, y, z, p, q, _, zz]) = seq_data();
    let rows = Q::new(&fx.store, 1, path(c(zz), plus(seq(opt(pred(p)), opt(pred(q)))), v(0))).rows(fx.rc());
    assert!(rows.is_empty(), ":zz (:p?/:q?)+ ?y must be empty, got {rows:?}");
    // The same with `*` operands: `:zz (:p*/:q*)+ ?y` is empty too.
    let rows = Q::new(&fx.store, 1, path(c(zz), plus(seq(star(pred(p)), star(pred(q)))), v(0))).rows(fx.rc());
    assert!(rows.is_empty(), ":zz (:p*/:q*)+ ?y must be empty, got {rows:?}");
    // Control: from a graph node the same path behaves normally.
    let rows = Q::new(&fx.store, 1, path(c(x), plus(seq(opt(pred(p)), opt(pred(q)))), v(0))).sorted(fx.rc());
    assert_eq!(rows, ms(vec![vec![x], vec![y], vec![z]]));
    // Through EXISTS substitution (the endpoint becomes a constant):
    // VALUES ?0 { :x :zz } FILTER EXISTS { ?0 (:p?/:q?)+ ?1 } keeps only :x.
    let ex = path(v(0), plus(seq(opt(pred(p)), opt(pred(q)))), v(1));
    let outer = filter(values(&[0], vec![vec![x], vec![zz]]), Expr::Exists(ExistsId(0)));
    let rows = Q::new(&fx.store, 2, outer).exists(vec![ex]).rows(fx.rc());
    assert_eq!(rows, vec![vec![x, u()]], "EXISTS {{ :zz (:p?/:q?)+ ?y }} is false");
}

/// ADDENDUM 2 (d), the same rule for a sequence nested in an alternative (spargebra only
/// flattens top-level sequences into BGPs): forward and backward evaluation.
#[test]
fn review_sequence_middle_is_a_join_variable_inside_alternative() {
    let (fx, [_, _, _, p, q, r, zz]) = seq_data();
    let fwd = path(c(zz), alt(seq(opt(pred(p)), opt(pred(q))), pred(r)), v(0));
    let rows = Q::new(&fx.store, 1, fwd).rows(fx.rc());
    assert!(rows.is_empty(), ":zz ((:p?/:q?)|:r) ?y must be empty, got {rows:?}");
    let bwd = path(v(0), alt(seq(opt(pred(p)), opt(pred(q))), pred(r)), c(zz));
    let rows = Q::new(&fx.store, 1, bwd).rows(fx.rc());
    assert!(rows.is_empty(), "?x ((:p?/:q?)|:r) :zz must be empty, got {rows:?}");
}

/// ADDENDUM 2 (d): `ASK { :zz :p?/(:q?/:r?) :zz }` is false (the inner middle is free on both
/// sides of `:q?`), while `ASK { :zz :p?/:q? :zz }` is TRUE (each operand binds the middle
/// to :zz from its own constant endpoint). Nested in an alternative so that the planner
/// would keep them as one Path.
#[test]
fn review_nested_sequence_with_constant_endpoints() {
    let (fx, [_, _, _, p, q, r, zz]) = seq_data();
    let s = fx.iri("s");
    let nested = alt(seq(opt(pred(p)), seq(opt(pred(q)), opt(pred(r)))), pred(s));
    let rows = Q::new(&fx.store, 0, path(c(zz), nested, c(zz))).rows(fx.rc());
    assert!(rows.is_empty(), "ASK {{ :zz (:p?/(:q?/:r?))|:s :zz }} must be false, got {} rows", rows.len());
    let flat = alt(seq(opt(pred(p)), opt(pred(q))), pred(s));
    let rows = Q::new(&fx.store, 0, path(c(zz), flat, c(zz))).rows(fx.rc());
    assert_eq!(rows.len(), 1, "ASK {{ :zz (:p?/:q?)|:s :zz }} is true");
}

/// A store wrapper that records `count` calls made without a cap.
struct CountSpy {
    inner: MemStore,
    uncapped: Cell<u64>,
}

impl QuadStore for CountSpy {
    fn cursor(&self, spec: &ScanSpec) -> Result<Box<dyn QuadCursor>> {
        self.inner.cursor(spec)
    }
    fn count(&self, spec: &ScanSpec, cap: Option<u64>) -> Result<u64> {
        if cap.is_none() {
            self.uncapped.set(self.uncapped.get() + 1);
        }
        self.inner.count(spec, cap)
    }
    fn dict(&self) -> &dyn Dictionary {
        self.inner.dict()
    }
    fn named_graphs(&self) -> Result<Vec<TermId>> {
        self.inner.named_graphs()
    }
    fn stats(&self) -> Option<Arc<Stats>> {
        None
    }
    fn costs(&self) -> &CostConstants {
        self.inner.costs()
    }
    fn caps(&self) -> StoreCaps {
        StoreCaps::default()
    }
}

/// `Execution::count()` (the `sparql_count` benchmark primitive) of `SELECT * { ?s :p ?o }
/// LIMIT 3` must not count the whole range: Slice::fast_count asks the Scan for an exact,
/// uncapped `store.count` (O(table) in the VDBE) and only then applies the limit.
#[test]
fn review_count_of_a_limited_scan_does_not_count_the_whole_range() {
    let fx = Fx::new();
    let p = fx.iri("p");
    let mut quads: Vec<[TermId; 4]> =
        (0..5000).map(|i| [TermId::DEFAULT_GRAPH, fx.iri(&format!("s{i}")), p, int(i)]).collect();
    fx.store.insert(&mut quads).unwrap();
    let spy = Rc::new(CountSpy { inner: fx.store.clone(), uncapped: Cell::new(0) });
    let mut e = Q::new(&fx.store, 2, slice(scan(v(0), c(p), v(1)), 0, Some(3))).exec(spy.clone());
    assert_eq!(e.count().unwrap(), 3);
    assert_eq!(spy.uncapped.get(), 0, "an exact count of the 5000-row range was requested for LIMIT 3");
}

/// The memory budget must bound what an execution keeps: a streaming BIND of a computed,
/// non-inline string interns one TEMP term per row into the execution's TempDict, which is
/// never released and never charged, so `max_memory_mb` does not stop it.
#[test]
fn review_computed_terms_are_charged_against_the_memory_budget() {
    let fx = Fx::new();
    let p = fx.iri("p");
    let pad = fx.lit("-a-padding-literal-that-makes-every-computed-string-long-");
    let mut quads: Vec<[TermId; 4]> =
        (0..20_000).map(|i| [TermId::DEFAULT_GRAPH, fx.iri(&format!("s{i}")), p, int(i)]).collect();
    fx.store.insert(&mut quads).unwrap();
    // BIND(CONCAT(STR(?0), pad) AS ?2): 20K distinct strings of ~80 bytes (> 1.5 MB of terms).
    let concat = Expr::Call(Function::Concat, vec![Expr::Call(Function::Str, vec![sv(0)]), cv(pad)]);
    let p = plan(Op::Extend {
        input: Box::new(scan(v(0), c(p), v(1))),
        slot: 2,
        expr: concat,
        canonical: false,
    });
    let mut q = Q::new(&fx.store, 3, p);
    q.opts.max_memory_mb = 1;
    let mut e = q.exec(fx.rc());
    let r = collect(&mut e);
    assert!(
        matches!(r, Err(Error::MemoryBudget(1))),
        "20K computed terms retained with max_memory_mb = 1: {:?} rows, memory_used = {}",
        r.map(|v| v.len()),
        e.memory_used()
    );
}

/// The memory budget covers DISTINCT sets (spec §5.1/§5.3: "DISTINCT aggregates keep a
/// per-group FxHashSet"; the budget is charged by "DISTINCT sets"). COUNT(DISTINCT ?o) over
/// 200K distinct values in one group keeps them all in the accumulator, but Group charges
/// only a fixed 96 bytes per group, so `max_memory_mb = 1` is never reached.
#[test]
fn review_distinct_aggregate_sets_are_charged_against_the_memory_budget() {
    let fx = Fx::new();
    let rows: Vec<Vec<TermId>> = (0..200_000).map(|i| vec![int(i)]).collect();
    let count_distinct = AggSpec { func: AggFunc::Count, distinct: true, arg: Some(sv(0)) };
    let g = plan(Op::Group {
        input: Box::new(values(&[0], rows)),
        keys: vec![],
        aggs: vec![(1, count_distinct)],
        streaming: false,
    });
    let mut q = Q::new(&fx.store, 2, g);
    q.opts.max_memory_mb = 1;
    let mut e = q.exec(fx.rc());
    let r = collect(&mut e);
    assert!(
        matches!(r, Err(Error::MemoryBudget(1))),
        "200K distinct ids held by COUNT(DISTINCT) with max_memory_mb = 1: {:?}",
        r.map(|v| v[0][1])
    );
}
