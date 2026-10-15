//! CONSTRUCT (template instantiation, fresh blank nodes, skipped triples, set semantics),
//! DESCRIBE (concise bounded description), ASK and decoding.
use super::*;
use sq_core::ir::TemplateTerm as T;

#[test]
fn construct_instantiates_dedupes_and_skips_invalid_triples() {
    let fx = Fx::new();
    let [a, b, p, q, r] = ["a", "b", "p", "q", "r"].map(|n| fx.iri(n));
    let lit = fx.lit("a literal object");
    fx.add(a, p, b);
    fx.add(b, p, lit);
    fx.add(a, q, b);
    // CONSTRUCT { ?0 :r ?1 . _:x :r ?1 . ?1 :r ?0 . ?0 ?2 ?1 . :a :r :b } WHERE { ?0 :p ?1 }
    let template = vec![
        [T::Slot(0), T::Const(r), T::Slot(1)],
        [T::BNode(0), T::Const(r), T::Slot(1)],
        [T::Slot(1), T::Const(r), T::Slot(0)], // literal subject for the second solution
        [T::Slot(0), T::Slot(2), T::Slot(1)],  // ?2 unbound: skipped
        [T::Const(a), T::Const(r), T::Const(b)],
        [T::Slot(0), T::Slot(1), T::Slot(0)], // predicate b (an IRI) once, a literal once
    ];
    let mut e = Q::new(&fx.store, 3, scan(v(0), c(p), v(1))).form(QueryForm::Construct(template.clone())).exec(fx.rc());
    let got = triples(&mut e).unwrap();
    // Solution 1 (a, b): (a r b), (_:x1 r b), (b r a), (a r b) dup, (a b a).
    // Solution 2 (b, lit): (b r lit), (_:x2 r lit), literal subject skipped, (a r b) dup,
    // predicate is a literal: skipped.
    assert_eq!(got.len(), 6, "{got:?}");
    assert_eq!(got[0], [a, r, b]);
    assert_eq!(got[2], [b, r, a]);
    assert_eq!(got[3], [a, b, a]);
    assert_eq!(got[4], [b, r, lit]);
    let (x1, x2) = (got[1][0], got[5][0]);
    assert!(x1.is_temp() && x1.is_bnode() && x2.is_bnode());
    assert_ne!(x1, x2, "fresh blank node per solution");
    assert_eq!(got[1][1..], [r, b]);
    let shown = show(&mut e, &[x1, lit]);
    assert!(shown[0].starts_with("_:"), "{shown:?}");
    assert_eq!(shown[1], "\"a literal object\"");
    let mut e = Q::new(&fx.store, 3, scan(v(0), c(p), v(1))).form(QueryForm::Construct(template)).exec(fx.rc());
    assert_eq!(e.count().unwrap(), 6);
}

#[test]
fn describe_follows_blank_node_objects() {
    let fx = Fx::new();
    let [a, b, p, q] = ["a", "b", "p", "q"].map(|n| fx.iri(n));
    let (n1, n2) = (fx.bnode(), fx.bnode());
    fx.add(a, p, n1);
    fx.add(n1, q, n2);
    fx.add(n2, q, n1); // a blank-node cycle
    fx.add(n2, p, b);
    fx.add(b, p, a); // not part of a's description
    let rows = values(&[0], vec![vec![a], vec![a], vec![int(1)]]);
    let mut e = Q::new(&fx.store, 1, rows).form(QueryForm::Describe(vec![0])).exec(fx.rc());
    let mut got = triples(&mut e).unwrap();
    got.sort();
    let mut want = vec![[a, p, n1], [n1, q, n2], [n2, q, n1], [n2, p, b]];
    want.sort();
    assert_eq!(got, want);
    // Describing two resources: each description once.
    let rows = values(&[0], vec![vec![a], vec![b]]);
    let mut e = Q::new(&fx.store, 1, rows).form(QueryForm::Describe(vec![0])).exec(fx.rc());
    assert_eq!(e.count().unwrap(), 5);
}

#[test]
fn ask_and_solution_source_shapes() {
    let fx = Fx::new();
    let [a, p] = ["a", "p"].map(|n| fx.iri(n));
    fx.add(a, p, a);
    let mut e = Q::new(&fx.store, 1, scan(v(0), c(p), v(0))).form(QueryForm::Ask).exec(fx.rc());
    assert!(e.variables().is_empty());
    let mut buf = Vec::new();
    assert_eq!(e.next_rows(&mut buf, 10).unwrap(), 1, "ASK true: one row of no variables");
    assert_eq!(e.next_rows(&mut buf, 10).unwrap(), 0);
    assert!(buf.is_empty());
    let mut e = Q::new(&fx.store, 1, scan(v(0), c(a), v(0))).form(QueryForm::Ask).exec(fx.rc());
    assert_eq!(e.next_rows(&mut buf, 10).unwrap(), 0, "ASK false: no row");
    let mut e = Q::new(&fx.store, 1, scan(v(0), c(p), v(0))).form(QueryForm::Ask).exec(fx.rc());
    assert!(e.ask().unwrap());
    let mut e = Q::new(&fx.store, 1, scan(v(0), c(p), v(0))).form(QueryForm::Ask).exec(fx.rc());
    assert_eq!(e.count().unwrap(), 1);
    // Variables in projection order, names from the VarTable.
    let e = Q::new(&fx.store, 3, plan(Op::Unit)).form(QueryForm::Select(vec![2, 0])).exec(fx.rc());
    assert_eq!(e.variables(), &["v2".to_string(), "v0".to_string()]);
    assert!(matches!(e.form(), QueryForm::Select(_)));
    // Wrong source for the form.
    let mut e = Q::new(&fx.store, 1, plan(Op::Unit)).exec(fx.rc());
    assert!(matches!(e.next_triples(&mut Vec::new(), 1), Err(Error::Invalid(_))));
    let mut e = Q::new(&fx.store, 1, plan(Op::Unit)).form(QueryForm::Construct(vec![])).exec(fx.rc());
    assert!(matches!(e.next_rows(&mut Vec::new(), 1), Err(Error::Invalid(_))));
}

#[test]
fn decode_batches_dictionary_misses_and_handles_every_id_class() {
    let fx = Fx::new();
    let iri = fx.iri("a");
    let long = fx.lit("a long literal");
    let typed = fx.typed("1.0E0", "double");
    let bn = fx.bnode();
    let mut temps = TempDict::new();
    let temp = temps.intern(simple_term("temp string value"));
    let mut e = Q::new(&fx.store, 1, plan(Op::Unit)).temps(temps).exec(fx.rc());
    let ids = [iri, long, typed, bn, temp, int(7), TermId::UNBOUND, TermId::ERROR, iri];
    let shown = show(&mut e, &ids);
    assert_eq!(shown[0], format!("<{EX}a>"));
    assert_eq!(shown[1], "\"a long literal\"");
    assert_eq!(shown[2], "\"1.0E0\"^^<http://www.w3.org/2001/XMLSchema#double>");
    assert!(shown[3].starts_with("_:b"));
    assert_eq!(shown[4], "\"temp string value\"");
    assert_eq!(shown[5], "\"7\"^^<http://www.w3.org/2001/XMLSchema#integer>");
    assert_eq!(&shown[6..8], &["UNDEF", "UNDEF"]);
    assert_eq!(shown[8], shown[0]);
    let prof = e.profile();
    assert_eq!(prof.decode_misses, 3, "three distinct dictionary ids, one batch");
    assert!(prof.decode_hits >= 4);
    // TripleSource::decode is the same resolver.
    let mut out = Vec::new();
    TripleSource::decode(&mut e, &[iri], &mut out).unwrap();
    assert_eq!(out[0].as_ref().unwrap().to_ntriples(), format!("<{EX}a>"));
}

#[test]
fn profile_counts_rows_opens_and_time() {
    let fx = Fx::new();
    let p = fx.iri("p");
    for i in 0..10 {
        fx.add(fx.iri(&format!("s{i}")), p, int(i));
    }
    let f = filter(scan(v(0), c(p), v(1)), Expr::Greater(Box::new(sv(1)), Box::new(cv(int(4)))));
    let mut e = Q::new(&fx.store, 2, f).exec(fx.rc());
    e.enable_timing();
    assert_eq!(collect(&mut e).unwrap().len(), 5);
    let prof = e.profile();
    assert_eq!(prof.nodes.len(), 2);
    assert_eq!((prof.nodes[0].rows_out, prof.nodes[1].rows_out), (5, 10));
    assert_eq!((prof.nodes[0].opens, prof.nodes[1].opens), (1, 1));
    assert!(prof.nodes[0].time_ns > 0 && prof.exec_ns > 0);
}
