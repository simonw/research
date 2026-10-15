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

/// W3C sparql10 optional-filter `dawg-optional-filter-005-not-simplified` (data-1.ttl,
/// expr-5.rq): `?book dc:title ?title OPTIONAL { { ?book x:price ?price FILTER(?title = "TITLE 2") } }`.
/// The manifest lists only this test ("Preferred reading and SPARQL 1.1"; its `-simplified` twin
/// is commented out), and §18.2.2 says "Applying the simplification step after all the
/// translation of graph patterns is the preferred reading". Translating OPTIONAL{P} sees
/// `Join(Z, Filter(F, BGP))`, which is not of the form Filter(F, A2), so the result is
/// `LeftJoin(BGP_title, Filter(F, BGP_price), true)`: ?title is unbound inside the filter, the
/// optional part never matches and no ?price is bound. Given that algebra the oracle produces
/// the expected answer, so the W3C failure is not a "spec subtlety the oracle transcribes":
/// spargebra applies the simplification step while parsing and hands sq-plan
/// `LeftJoin(BGP_title, BGP_price, F)`, the non-preferred SPARQL 1.0 reading.
#[test]
fn review_optional_filter_005_preferred_reading() {
    let mut fx = Fx::new();
    let (title, price) = (fx.iri("title"), fx.iri("price"));
    let (b1, b2, b3) = (fx.iri("book1"), fx.iri("book2"), fx.iri("book3"));
    let (t1, t2, t3) = (fx.lit("TITLE 1"), fx.lit("TITLE 2"), fx.lit("TITLE 3"));
    fx.add(b1, title, t1);
    fx.add(b1, price, fx.int(10));
    fx.add(b2, title, t2);
    fx.add(b2, price, fx.int(20));
    fx.add(b3, title, t3);
    let (book, t, p) = (fx.var("book"), fx.var("title"), fx.var("price"));
    let titles = || bgp(vec![tp(v(book), c(title), v(t))]);
    let prices = || bgp(vec![tp(v(book), c(price), v(p))]);
    let is_title_2 = || eq(es(t), ec(t2));

    // §18.2 preferred reading (expr-5-result-not-simplified.ttl): three titles, no price.
    let preferred = Logical::LeftJoin {
        left: Box::new(titles()),
        right: Box::new(filter(prices(), is_title_2())),
        filter: None,
    };
    assert_eq!(
        fx.bag(&[t, p], preferred),
        bag(&[&["\"TITLE 1\"", "-"], &["\"TITLE 2\"", "-"], &["\"TITLE 3\"", "-"]])
    );

    // What spargebra produces for expr-5.rq (early simplification): TITLE 2 gets its price.
    let simplified =
        Logical::LeftJoin { left: Box::new(titles()), right: Box::new(prices()), filter: Some(is_title_2()) };
    assert_eq!(
        fx.bag(&[t, p], simplified),
        bag(&[&["\"TITLE 1\"", "-"], &["\"TITLE 2\"", &int(20)], &["\"TITLE 3\"", "-"]])
    );
}

/// `ir::validate` promises to reject IR "the evaluator could not index safely", but it never
/// bounds the variable table by `SlotSet::MAX_SLOTS` (256). `COUNT(DISTINCT *)` computes the
/// in-scope variables with a `SlotSet` (`ir::in_scope`), whose fixed `[u64; 4]` is indexed by
/// `slot >> 6`: a slot >= 256 panics with an index out of bounds instead of returning
/// `Error::Invalid`. sq-plan caps lowering at 256 slots, so only hand-built IR reaches it.
#[test]
fn review_validate_rejects_ir_wider_than_a_slot_set() {
    use sq_core::ir::{AggFunc, AggSpec};

    let mut fx = Fx::new();
    let (a, p) = (fx.iri("a"), fx.iri("p"));
    fx.add(a, p, a);
    let slots: Vec<Slot> = (0..300).map(|i| fx.var(&format!("v{i}"))).collect();
    let (s, n) = (slots[299], slots[0]);
    let count_distinct_star = AggSpec { func: AggFunc::CountAll, distinct: true, arg: None };
    let root = Logical::Group {
        input: Box::new(bgp(vec![tp(v(s), c(p), v(s))])),
        keys: vec![],
        aggs: vec![(n, count_distinct_star)],
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        fx.try_run(sq_core::ir::QueryForm::Select(vec![n]), root)
    }));
    match result {
        Ok(Err(sq_core::Error::Invalid(_))) => {}
        Ok(other) => panic!("expected Error::Invalid for a {}-slot variable table, got {other:?}", slots.len()),
        Err(_) => panic!("evaluation panicked on a {}-slot variable table instead of failing validation", slots.len()),
    }
}
