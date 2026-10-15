//! Adversarial review tests (WP7). Each test names the SPARQL 1.1 definition it checks.
use sq_core::ir::{GraphSel, Logical, PathPlan};

use super::*;

fn opt(p: PathPlan) -> PathPlan {
    PathPlan::ZeroOrOne(Box::new(p))
}

fn seq(a: PathPlan, b: PathPlan) -> PathPlan {
    PathPlan::Seq(Box::new(a), Box::new(b))
}

fn path(s: PatTerm, p: PathPlan, o: PatTerm) -> Logical {
    Logical::Path { g: GraphSel::Default, s, path: p, o }
}

/// §18.4 "Evaluation of Sequence Property Path": V is a FRESH VARIABLE,
/// `eval(Path(X, seq(P,Q), Y)) = Project(Join(eval(Path(X, P, V)), eval(Path(V, Q, Y))), Var(X,Y))`.
/// With V free, `Path(V, Q?, Y)` only produces zero-length pairs for `nodes(G)`, so a constant X
/// that is not a node of G cannot reach Y through the zero-length step of Q. The oracle instead
/// evaluates Q with the middle FIXED (bind-join style), which makes the middle a "constant"
/// and lets the zero-length rule for constants fire.
#[test]
fn review_sequence_middle_is_a_fresh_variable() {
    let mut fx = Fx::new();
    let (a, b, p, q) = (fx.iri("a"), fx.iri("b"), fx.iri("p"), fx.iri("q"));
    fx.add(a, p, b);
    let zz = fx.absent("zz");
    let (y, m) = (fx.var("y"), fx.var("m"));
    let pq = || seq(opt(PathPlan::Pred(p)), opt(PathPlan::Pred(q)));

    // The definition, written out with an explicit fresh variable ?m: no solution.
    let by_definition = join(path(c(zz), opt(PathPlan::Pred(p)), v(m)), path(v(m), opt(PathPlan::Pred(q)), v(y)));
    assert!(fx.select(&[y], by_definition).is_empty());

    // The same sequence as one Path node must agree (pyoxigraph agrees: no solution).
    assert_eq!(fx.select(&[y], path(c(zz), pq(), v(y))), Vec::<Vec<String>>::new(), ":zz (:p?/:q?) ?y");

    // A stored term used only as a predicate is not in nodes(G) either.
    assert_eq!(fx.select(&[y], path(c(p), pq(), v(y))), Vec::<Vec<String>>::new(), ":p (:p?/:q?) ?y");

    // Inside a closure: eval(:zz, seq) is empty, so (:p?/:q?)+ from :zz has no solution.
    let plus = PathPlan::OneOrMore(Box::new(pq()));
    assert_eq!(fx.select(&[y], path(c(zz), plus, v(y))), Vec::<Vec<String>>::new(), ":zz (:p?/:q?)+ ?y");
}
