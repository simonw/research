//! Query forms, output decoding, evaluation options and IR validation.
use sq_core::ir::{ExistsId, Expr, Logical, QueryForm, TemplateTerm};
use sq_core::spargebra::algebra::Function;
use sq_core::{Error, Kind, Term};

use super::*;

fn graph_of(out: OracleOutput) -> Vec<[Arc<Term>; 3]> {
    match out {
        OracleOutput::Graph(triples) => triples,
        other => panic!("expected a graph, got {other:?}"),
    }
}

fn show_triple(t: &[Arc<Term>; 3]) -> String {
    t.iter().map(|x| show(Some(x))).collect::<Vec<_>>().join(" ")
}

#[test]
fn select_names_and_orders_columns_by_projection() {
    let mut fx = Fx::new();
    let (x, y) = (fx.var("x"), fx.var("y"));
    let a = fx.iri("a");
    let out = fx.run(QueryForm::Select(vec![y, x]), values(&[x, y], &[&[a, TermId::UNBOUND]]));
    let OracleOutput::Solutions { vars, rows } = out else { panic!("solutions expected") };
    assert_eq!(vars, vec!["y".to_string(), "x".to_string()]);
    assert_eq!(rows, vec![vec![None, Some(Arc::new(Term::Iri(format!("{NS}a").into())))]]);
}

#[test]
fn construct_instantiates_fresh_blank_nodes_skips_invalid_triples_and_dedupes() {
    let mut fx = Fx::new();
    let (a, b, p, q) = (fx.iri("a"), fx.iri("b"), fx.iri("p"), fx.iri("q"));
    fx.add(a, p, fx.int(1));
    fx.add(b, p, fx.int(1));
    let (s, o, z) = (fx.var("s"), fx.var("o"), fx.var("z"));
    let template = vec![
        // _:n :q ?s  (fresh blank node per solution, shared within it)
        [TemplateTerm::BNode(0), TemplateTerm::Const(q), TemplateTerm::Slot(s)],
        [TemplateTerm::BNode(0), TemplateTerm::Const(p), TemplateTerm::Slot(o)],
        // literal subject: skipped
        [TemplateTerm::Slot(o), TemplateTerm::Const(p), TemplateTerm::Slot(s)],
        // unbound ?z: skipped
        [TemplateTerm::Slot(s), TemplateTerm::Const(p), TemplateTerm::Slot(z)],
        // non-IRI predicate: skipped
        [TemplateTerm::Slot(s), TemplateTerm::Slot(o), TemplateTerm::Slot(s)],
        // same triple for both solutions: once
        [TemplateTerm::Const(a), TemplateTerm::Const(q), TemplateTerm::Const(b)],
    ];
    let triples = graph_of(fx.run(QueryForm::Construct(template), bgp(vec![tp(v(s), c(p), v(o))])));
    assert_eq!(triples.len(), 5, "{:?}", triples.iter().map(show_triple).collect::<Vec<_>>());
    let bnodes: Vec<&Arc<Term>> = triples.iter().filter(|t| t[0].kind() == Kind::BNode).map(|t| &t[0]).collect();
    assert_eq!(bnodes.len(), 4);
    // Two solutions: two distinct blank nodes, each used twice.
    assert_eq!(bnodes[0], bnodes[1]);
    assert_eq!(bnodes[2], bnodes[3]);
    assert_ne!(bnodes[0], bnodes[2]);
    assert_eq!(triples.iter().filter(|t| show_triple(t) == ":a :q :b").count(), 1);
}

#[test]
fn ask_is_true_iff_there_is_a_solution() {
    let mut fx = Fx::new();
    let (a, p) = (fx.iri("a"), fx.iri("p"));
    fx.add(a, p, a);
    let x = fx.var("x");
    assert!(fx.ask(bgp(vec![tp(v(x), c(p), c(a))])));
    assert!(!fx.ask(bgp(vec![tp(v(x), c(p), c(p))])));
}

#[test]
fn describe_returns_the_concise_bounded_description() {
    let mut fx = Fx::new();
    let (a, b, p, q) = (fx.iri("a"), fx.iri("b"), fx.iri("p"), fx.iri("q"));
    let (n1, n2) = (fx.bnode(), fx.bnode());
    fx.add(a, p, n1);
    fx.add(n1, q, n2);
    fx.add(n2, q, fx.lit("deep"));
    fx.add(a, q, b);
    fx.add(b, p, fx.lit("not described"));
    fx.add(n2, p, n1); // a blank-node cycle
    let x = fx.var("x");
    let triples = graph_of(fx.run(QueryForm::Describe(vec![x]), values(&[x], &[&[a], &[a], &[fx.lit("lit")]])));
    assert_eq!(triples.len(), 5, "{:?}", triples.iter().map(show_triple).collect::<Vec<_>>());
    assert!(!triples.iter().any(|t| show_triple(t).contains("not described")));
    let zz = fx.absent("zz");
    assert!(graph_of(fx.run(QueryForm::Describe(vec![x]), values(&[x], &[&[zz]]))).is_empty());
}

#[test]
fn now_and_random_values_come_from_the_options() {
    let mut fx = Fx::new();
    let (t, r) = (fx.var("t"), fx.var("r"));
    let root = extend(extend(Logical::Unit, t, call(Function::Now, vec![])), r, call(Function::Rand, vec![]));
    let first = fx.select(&[t, r], root.clone());
    assert_eq!(first[0][0], "\"2026-10-15T12:00:00Z\"^^<http://www.w3.org/2001/XMLSchema#dateTime>");
    assert_eq!(first, fx.select(&[t, r], root), "fixed seed: reproducible");
}

#[test]
fn malformed_ir_is_rejected_before_evaluation() {
    let mut fx = Fx::new();
    let x = fx.var("x");
    let invalid = |r: sq_core::Result<OracleOutput>| matches!(r, Err(Error::Invalid(_)));
    assert!(invalid(fx.try_run(QueryForm::Select(vec![x + 1]), Logical::Unit)));
    assert!(invalid(fx.try_run(QueryForm::Select(vec![x]), extend(Logical::Unit, x + 5, ec(fx.int(1))))));
    assert!(invalid(fx.try_run(QueryForm::Select(vec![x]), values(&[x], &[&[]]))));
    assert!(invalid(fx.try_run(QueryForm::Select(vec![x]), filter(Logical::Unit, Expr::Exists(ExistsId(3))))));
    assert!(invalid(fx.try_run(QueryForm::Select(vec![x]), filter(Logical::Unit, gt(es(x + 9), ec(fx.int(0)))))));
    let template = vec![[TemplateTerm::Slot(x + 2), TemplateTerm::Slot(x), TemplateTerm::Slot(x)]];
    assert!(invalid(fx.try_run(QueryForm::Construct(template), Logical::Unit)));
}
