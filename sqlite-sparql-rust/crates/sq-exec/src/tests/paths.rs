//! Property paths: every PathPlan form, both directions, zero-length rules (constants, the
//! has_node rule for Join seeds, both endpoints free), cycles, graphs, and a brute-force
//! reachability cross-check that exercises both frontier-expansion strategies.
use std::collections::{BTreeMap, BTreeSet};

use sq_core::ir::{DefaultGraph, NamedGraphs, PathPlan};
use sq_core::testing::Rng;

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

fn star(p: PathPlan) -> PathPlan {
    PathPlan::ZeroOrMore(Box::new(p))
}

fn plus(p: PathPlan) -> PathPlan {
    PathPlan::OneOrMore(Box::new(p))
}

fn opt(p: PathPlan) -> PathPlan {
    PathPlan::ZeroOrOne(Box::new(p))
}

/// a -p-> b -p-> c -p-> a (cycle), d -p-> e, a -name-> "A", a -r-> m1, a -r-> m2,
/// m1 -s-> z, m2 -s-> z.
struct G {
    fx: Fx,
    a: TermId,
    b: TermId,
    c: TermId,
    d: TermId,
    e: TermId,
    z: TermId,
    p: TermId,
    name: TermId,
    r: TermId,
    s: TermId,
    lit: TermId,
}

fn graph() -> G {
    let fx = Fx::new();
    let [a, b, cc, d, e, m1, m2, z, p, name, r, s] =
        ["a", "b", "c", "d", "e", "m1", "m2", "z", "p", "name", "r", "s"].map(|n| fx.iri(n));
    let lit = fx.lit("A");
    for (x, y, w) in [(a, p, b), (b, p, cc), (cc, p, a), (d, p, e), (a, name, lit), (a, r, m1), (a, r, m2), (m1, s, z), (m2, s, z)] {
        fx.add(x, y, w);
    }
    G { fx, a, b, c: cc, d, e, z, p, name, r, s, lit }
}

impl G {
    fn col(&self, p: Plan, slot: usize, width: usize) -> Vec<TermId> {
        let mut v: Vec<TermId> = Q::new(&self.fx.store, width, p).rows(self.fx.rc()).into_iter().map(|r| r[slot]).collect();
        v.sort();
        v
    }
    fn count(&self, p: Plan) -> usize {
        Q::new(&self.fx.store, 2, p).rows(self.fx.rc()).len()
    }
}

fn sorted(mut v: Vec<TermId>) -> Vec<TermId> {
    v.sort();
    v
}

#[test]
fn predicate_inverse_sequence_alternative() {
    let g = graph();
    assert_eq!(g.col(path(c(g.a), pred(g.p), v(0)), 0, 1), vec![g.b]);
    assert_eq!(g.col(path(v(0), pred(g.p), c(g.a)), 0, 1), vec![g.c]);
    assert_eq!(g.col(path(c(g.a), inv(pred(g.p)), v(0)), 0, 1), vec![g.c]);
    assert_eq!(g.col(path(c(g.a), seq(pred(g.p), pred(g.p)), v(0)), 0, 1), vec![g.c]);
    assert_eq!(g.col(path(v(0), seq(pred(g.p), pred(g.p)), c(g.a)), 0, 1), vec![g.b]);
    // Sequences and alternatives keep duplicates (bag semantics).
    assert_eq!(g.col(path(c(g.a), seq(pred(g.r), pred(g.s)), v(0)), 0, 1), vec![g.z, g.z]);
    assert_eq!(g.col(path(c(g.a), alt(pred(g.p), pred(g.p)), v(0)), 0, 1), vec![g.b, g.b]);
    assert_eq!(g.col(path(v(0), alt(pred(g.p), pred(g.name)), c(g.lit)), 0, 1), vec![g.a]);
    // Both endpoints bound: multiplicity of the bag.
    assert_eq!(g.count(path(c(g.a), seq(pred(g.r), pred(g.s)), c(g.z))), 2);
    // Both free.
    let rows = Q::new(&g.fx.store, 2, path(v(0), seq(pred(g.p), pred(g.p)), v(1))).sorted(g.fx.rc());
    assert_eq!(rows, ms(vec![vec![g.a, g.c], vec![g.b, g.a], vec![g.c, g.b]]));
}

#[test]
fn negated_property_sets() {
    let g = graph();
    // :a !(:p|:r) ?x -> "A"
    assert_eq!(g.col(path(c(g.a), PathPlan::Nps(vec![g.p, g.r]), v(0)), 0, 1), vec![g.lit]);
    // ?x !:p :z -> m1, m2 (via :s); backward over the inverse direction.
    let m = sorted(vec![g.fx.lookup("m1").unwrap(), g.fx.lookup("m2").unwrap()]);
    assert_eq!(g.col(path(v(0), PathPlan::Nps(vec![g.p]), c(g.z)), 0, 1), m);
    // :z ^!(:p) ?x (inverse set) -> m1, m2.
    assert_eq!(g.col(path(c(g.z), inv(PathPlan::Nps(vec![g.p])), v(0)), 0, 1), m);
    // Both free: every triple whose predicate is not excluded.
    let rows = Q::new(&g.fx.store, 2, path(v(0), PathPlan::Nps(vec![g.p, g.r, g.s]), v(1))).rows(g.fx.rc());
    assert_eq!(rows, vec![vec![g.a, g.lit]]);
}

#[test]
fn closures_terminate_on_cycles_with_set_semantics() {
    let g = graph();
    assert_eq!(g.col(path(c(g.a), star(pred(g.p)), v(0)), 0, 1), sorted(vec![g.a, g.b, g.c]));
    assert_eq!(g.col(path(c(g.a), plus(pred(g.p)), v(0)), 0, 1), sorted(vec![g.a, g.b, g.c]));
    assert_eq!(g.col(path(c(g.d), plus(pred(g.p)), v(0)), 0, 1), vec![g.e]);
    assert_eq!(g.col(path(c(g.a), opt(pred(g.p)), v(0)), 0, 1), sorted(vec![g.a, g.b]));
    assert_eq!(g.col(path(v(0), star(pred(g.p)), c(g.e)), 0, 1), sorted(vec![g.d, g.e]));
    // Set semantics: two derivations of z, one solution.
    assert_eq!(g.col(path(c(g.a), plus(seq(pred(g.r), pred(g.s))), v(0)), 0, 1), vec![g.z]);
    assert_eq!(g.col(path(c(g.a), star(alt(pred(g.p), pred(g.p))), v(0)), 0, 1), sorted(vec![g.a, g.b, g.c]));
    // Both bound, early exit.
    assert_eq!(g.count(path(c(g.a), star(pred(g.p)), c(g.c))), 1);
    assert_eq!(g.count(path(c(g.a), plus(pred(g.p)), c(g.a))), 1, "a cycle back to the start");
    assert_eq!(g.count(path(c(g.d), plus(pred(g.p)), c(g.d))), 0);
    assert_eq!(g.count(path(c(g.d), star(pred(g.p)), c(g.a))), 0);
    // Inverse closure.
    assert_eq!(g.col(path(c(g.e), star(inv(pred(g.p))), v(0)), 0, 1), sorted(vec![g.d, g.e]));
}

#[test]
fn zero_length_matches_constants_absent_from_the_graph() {
    let g = graph();
    let mut temps = TempDict::new();
    let absent = temps.intern(Term::Iri(format!("{EX}absent").into()));
    let stored_elsewhere = g.fx.iri("unused"); // in the dictionary, not in any triple
    let run = |p: Plan, w: usize| Q::new(&g.fx.store, w, p).temps(temps.clone()).rows(g.fx.rc());
    assert_eq!(run(path(c(absent), star(pred(g.p)), v(0)), 1), vec![vec![absent]]);
    assert_eq!(run(path(v(0), star(pred(g.p)), c(absent)), 1), vec![vec![absent]]);
    assert_eq!(run(path(c(absent), opt(pred(g.p)), c(absent)), 0).len(), 1);
    assert_eq!(run(path(c(stored_elsewhere), star(pred(g.p)), v(0)), 1), vec![vec![stored_elsewhere]]);
    assert!(run(path(c(absent), plus(pred(g.p)), v(0)), 1).is_empty());
    assert!(run(path(c(g.a), star(pred(g.p)), c(absent)), 0).is_empty());
    // A literal constant.
    let x = sq_core::codec::encode_short_str("x").unwrap();
    assert_eq!(run(path(c(x), star(pred(g.p)), v(0)), 1), vec![vec![x]]);
}

#[test]
fn join_seeded_endpoints_follow_the_has_node_rule() {
    let g = graph();
    let unused = g.fx.iri("unused");
    // VALUES ?0 {:a :unused :lit} . ?0 :p* ?1 — :unused is not a node: no zero-length match.
    let left = values(&[0], vec![vec![g.a], vec![unused], vec![g.lit]]);
    let right = plan(Op::Path { g: GraphSel::Default, s: v(0), path: star(pred(g.p)), o: v(1) });
    let rows = Q::new(&g.fx.store, 2, bind_join(left, right, &[0], JoinKind::Inner)).sorted(g.fx.rc());
    assert_eq!(rows, ms(vec![vec![g.a, g.a], vec![g.a, g.b], vec![g.a, g.c], vec![g.lit, g.lit]]));
    // values_and_path: VALUES ?0 {1 :a :unused} . ?0 :p? ?0 -> only graph nodes.
    let left = values(&[0], vec![vec![int(1)], vec![g.a], vec![unused]]);
    let right = plan(Op::Path { g: GraphSel::Default, s: v(0), path: opt(pred(g.p)), o: v(0) });
    let rows = Q::new(&g.fx.store, 1, bind_join(left, right, &[0], JoinKind::Inner)).rows(g.fx.rc());
    assert_eq!(rows, vec![vec![g.a]]);
    // With a constant on the other side the zero-length match comes from the constant
    // (`?0 :p* :unused` binds ?0 = :unused even though :unused is absent).
    let left = values(&[0], vec![vec![unused], vec![g.a]]);
    let right = plan(Op::Path { g: GraphSel::Default, s: v(0), path: star(pred(g.p)), o: c(unused) });
    let rows = Q::new(&g.fx.store, 1, bind_join(left, right, &[0], JoinKind::Inner)).rows(g.fx.rc());
    assert_eq!(rows, vec![vec![unused]]);
    let left = values(&[0], vec![vec![g.a], vec![unused]]);
    let right = plan(Op::Path { g: GraphSel::Default, s: c(unused), path: star(pred(g.p)), o: v(0) });
    let rows = Q::new(&g.fx.store, 1, bind_join(left, right, &[0], JoinKind::Inner)).rows(g.fx.rc());
    assert_eq!(rows, vec![vec![unused]]);
    // Object seeded: backward.
    let left = values(&[1], vec![vec![g.e], vec![unused]]);
    let right = plan(Op::Path { g: GraphSel::Default, s: v(0), path: star(pred(g.p)), o: v(1) });
    let rows = Q::new(&g.fx.store, 2, bind_join(left, right, &[1], JoinKind::Inner)).sorted(g.fx.rc());
    assert_eq!(rows, ms(vec![vec![g.d, g.e], vec![g.e, g.e]]));
}

#[test]
fn both_endpoints_free_and_same_variable() {
    let fx = Fx::new();
    let [a, b, cc, d, x, p, q] = ["a", "b", "c", "d", "x", "p", "q"].map(|n| fx.iri(n));
    let lit = fx.lit("lit");
    for (s, pp, o) in [(a, p, b), (b, p, a), (cc, p, d), (x, q, lit)] {
        fx.add(s, pp, o);
    }
    let rows = Q::new(&fx.store, 2, path(v(0), star(pred(p)), v(1))).sorted(fx.rc());
    let want = ms(vec![
        vec![a, a], vec![a, b], vec![b, b], vec![b, a], vec![cc, cc], vec![cc, d], vec![d, d], vec![x, x], vec![lit, lit],
    ]);
    assert_eq!(rows, want, "every node of the graph matches itself");
    let rows = Q::new(&fx.store, 1, path(v(0), star(pred(p)), v(0))).sorted(fx.rc());
    assert_eq!(rows, ms(vec![vec![a], vec![b], vec![cc], vec![d], vec![x], vec![lit]]));
    let rows = Q::new(&fx.store, 1, path(v(0), plus(pred(p)), v(0))).sorted(fx.rc());
    assert_eq!(rows, ms(vec![vec![a], vec![b]]), "p+ from a node to itself needs a cycle");
    let rows = Q::new(&fx.store, 2, path(v(0), plus(pred(p)), v(1))).sorted(fx.rc());
    assert_eq!(rows, ms(vec![vec![a, a], vec![a, b], vec![b, b], vec![b, a], vec![cc, d]]));
    let rows = Q::new(&fx.store, 2, path(v(0), opt(pred(q)), v(1))).rows(fx.rc());
    assert_eq!(rows.len(), 7, "6 zero-length matches + x q lit");
}

#[test]
fn paths_never_cross_graphs() {
    let fx = Fx::new();
    let [a, b, cc, p, g1, g2] = ["a", "b", "c", "p", "g1", "g2"].map(|n| fx.iri(n));
    fx.add_g(g1, a, p, b);
    fx.add_g(g2, b, p, cc);
    let named = vec![g1, g2];
    // GRAPH ?g { :a :p* ?x }: per graph; the constant matches itself in every graph.
    let gp = plan(Op::Path { g: GraphSel::Slot(1), s: c(a), path: star(pred(p)), o: v(0) });
    let rows = Q::new(&fx.store, 2, gp).sorted(fx.rc());
    assert_eq!(rows, ms(vec![vec![a, g1], vec![b, g1], vec![a, g2]]));
    // The merged default graph FROM g1 FROM g2 is one graph: a reaches c.
    let rows = Q::new(&fx.store, 1, path(c(a), plus(pred(p)), v(0)))
        .dataset(DefaultGraph::Graphs(Arc::from(named.clone())), NamedGraphs::Only(Arc::from(vec![])), vec![])
        .sorted(fx.rc());
    assert_eq!(rows, ms(vec![vec![b], vec![cc]]));
    // UnionAll default graph, both endpoints free.
    let rows = Q::new(&fx.store, 2, path(v(0), plus(pred(p)), v(1)))
        .dataset(DefaultGraph::UnionAll, NamedGraphs::All, named.clone())
        .sorted(fx.rc());
    assert_eq!(rows, ms(vec![vec![a, b], vec![a, cc], vec![b, cc]]));
    // GRAPH <g2>.
    let gp = plan(Op::Path { g: GraphSel::Const(g2), s: v(0), path: plus(pred(p)), o: v(1) });
    assert_eq!(Q::new(&fx.store, 2, gp).rows(fx.rc()), vec![vec![b, cc]]);
    // Seeded graph slot: only that graph.
    let left = values(&[1], vec![vec![g2]]);
    let right = plan(Op::Path { g: GraphSel::Slot(1), s: v(0), path: plus(pred(p)), o: v(2) });
    let rows = Q::new(&fx.store, 3, bind_join(left, right, &[1], JoinKind::Inner)).rows(fx.rc());
    assert_eq!(rows, vec![vec![b, g2, cc]]);
}

/// Reachability by brute force: pairs (x, y) with y reachable from x in >= 1 steps.
fn closure_pairs(edges: &[(TermId, TermId)]) -> BTreeSet<(TermId, TermId)> {
    let mut adj: BTreeMap<TermId, Vec<TermId>> = BTreeMap::new();
    for (x, y) in edges {
        adj.entry(*x).or_default().push(*y);
    }
    let mut out = BTreeSet::new();
    for &x in adj.keys() {
        let mut stack = adj[&x].clone();
        let mut seen = BTreeSet::new();
        while let Some(y) = stack.pop() {
            if seen.insert(y) {
                out.insert((x, y));
                stack.extend(adj.get(&y).into_iter().flatten().copied());
            }
        }
    }
    out
}

#[test]
fn closure_matches_brute_force_on_random_graphs() {
    for seed in [3u64, 11, 29] {
        let fx = Fx::new();
        let p = fx.iri("p");
        let mut r = Rng(seed);
        let nodes: Vec<TermId> = (0..150).map(|i| fx.iri(&format!("n{i}"))).collect();
        let mut edges = Vec::new();
        for _ in 0..260 {
            let (x, y) = (nodes[r.below(150) as usize], nodes[r.below(150) as usize]);
            edges.push((x, y));
            fx.add(x, p, y);
        }
        edges.sort();
        edges.dedup();
        let want = closure_pairs(&edges);
        // Both free: chunks of start candidates (large frontiers use the full-scan strategy).
        let rows = Q::new(&fx.store, 2, path(v(0), plus(pred(p)), v(1))).rows(fx.rc());
        let got: BTreeSet<(TermId, TermId)> = rows.iter().map(|r| (r[0], r[1])).collect();
        assert_eq!(got.len(), rows.len(), "set semantics");
        assert_eq!(got, want, "seed {seed}");
        // Backward from every node (constants: small frontiers, probes).
        for &y in nodes.iter().step_by(37) {
            let rows = Q::new(&fx.store, 1, path(v(0), plus(pred(p)), c(y))).sorted(fx.rc());
            let want: Vec<Vec<TermId>> = want.iter().filter(|(_, b)| *b == y).map(|(a, _)| vec![*a]).collect();
            assert_eq!(rows, ms(want), "seed {seed} backward to {y:?}");
        }
        // Star = plus + every node to itself.
        let n_nodes = {
            let mut s: BTreeSet<TermId> = BTreeSet::new();
            for (x, y) in &edges {
                s.insert(*x);
                s.insert(*y);
            }
            s
        };
        let rows = Q::new(&fx.store, 2, path(v(0), star(pred(p)), v(1))).rows(fx.rc());
        let mut star_want = want.clone();
        star_want.extend(n_nodes.iter().map(|x| (*x, *x)));
        let got: BTreeSet<(TermId, TermId)> = rows.iter().map(|r| (r[0], r[1])).collect();
        assert_eq!(got.len(), rows.len());
        assert_eq!(got, star_want, "seed {seed} star");
    }
}

#[test]
fn nested_closures_and_sequences() {
    let g = graph();
    // (:p/:p)* from a: a, c, b (a -2-> c -2-> b -2-> a).
    assert_eq!(g.col(path(c(g.a), star(seq(pred(g.p), pred(g.p))), v(0)), 0, 1), sorted(vec![g.a, g.b, g.c]));
    // :p*/:name from c: c->a->"A".
    assert_eq!(g.col(path(c(g.c), seq(star(pred(g.p)), pred(g.name)), v(0)), 0, 1), vec![g.lit]);
    // ^(:p+) from a = p+ backward.
    assert_eq!(g.col(path(c(g.a), inv(plus(pred(g.p))), v(0)), 0, 1), sorted(vec![g.a, g.b, g.c]));
    // (:p?)+ is p*.
    assert_eq!(g.col(path(c(g.d), plus(opt(pred(g.p))), v(0)), 0, 1), sorted(vec![g.d, g.e]));
}
