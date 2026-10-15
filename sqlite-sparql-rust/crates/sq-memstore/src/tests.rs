use super::*;
use sq_core::testing::{cursor_conformance, random_quads};

fn store_with(quads: &[[TermId; 4]]) -> MemStore {
    let s = MemStore::new();
    let mut v = quads.to_vec();
    s.insert(&mut v).unwrap();
    s
}

#[test]
fn conformance_default_graph() {
    for seed in [1u64, 2, 3] {
        let q = random_quads(seed, 2000, false);
        let s = store_with(&q);
        cursor_conformance(&s, &q, seed, 4);
    }
}

#[test]
fn conformance_named_graphs() {
    for seed in [11u64, 12, 13] {
        let q = random_quads(seed, 2000, true);
        let s = store_with(&q);
        cursor_conformance(&s, &q, seed, 4);
    }
}

#[test]
fn conformance_after_deletes_and_reinserts() {
    let q = random_quads(21, 3000, true);
    let s = store_with(&q);
    let mut del: Vec<_> = q.iter().copied().step_by(3).collect();
    assert_eq!(s.remove(&mut del).unwrap(), del.len() as u64);
    let mut re: Vec<_> = del.iter().copied().step_by(2).collect();
    assert_eq!(s.insert(&mut re).unwrap(), re.len() as u64);
    let expect = s.quads();
    cursor_conformance(&s, &expect, 21, 3);
    // remaining = original minus deleted plus reinserted
    let mut want: Vec<_> = q.iter().copied().filter(|x| !del.contains(x) || re.contains(x)).collect();
    want.sort();
    assert_eq!(expect, want);
}

#[test]
fn dictionary_roundtrip_and_well_known() {
    let s = MemStore::new();
    let iri = oxrdf::NamedNode::new("http://ex/a").unwrap();
    let lit = oxrdf::Literal::new_language_tagged_literal("chat", "FR").unwrap();
    let typed = oxrdf::Literal::new_typed_literal("1.5e0", oxrdf::NamedNode::new(format!("{}double", codec::XSD)).unwrap());
    let long = oxrdf::Literal::new_simple_literal("a long simple literal");
    let ids: Vec<TermId> = [iri.as_ref().into(), lit.as_ref().into(), typed.as_ref().into(), long.as_ref().into()]
        .into_iter()
        .map(|t: oxrdf::TermRef<'_>| s.intern(t).unwrap())
        .collect();
    assert_eq!(ids[0].tag(), Tag::Iri);
    assert_eq!(ids[1].tag(), Tag::Lang);
    assert_eq!(ids[2].tag(), Tag::Typed);
    assert_eq!(ids[3].tag(), Tag::Str);
    // idempotent
    assert_eq!(s.intern(iri.as_ref().into()).unwrap(), ids[0]);
    assert_eq!(s.lookup(lit.as_ref().into()).unwrap(), Some(ids[1]));
    let mut out = Vec::new();
    s.decode(&ids, &mut out).unwrap();
    assert_eq!(out[1].to_oxrdf(), oxrdf::Literal::new_language_tagged_literal("chat", "fr").unwrap().into());
    assert_eq!(out[2].to_oxrdf(), typed.into());
    // well-known rows
    let ty = oxrdf::NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type").unwrap();
    assert_eq!(s.lookup(ty.as_ref().into()).unwrap(), Some(codec::RDF_TYPE));
    // inline: no row
    let five = oxrdf::Literal::new_typed_literal("5", oxrdf::NamedNode::new(codec::XSD_INTEGER_IRI).unwrap());
    assert_eq!(s.intern(five.as_ref().into()).unwrap(), TermId::from_int(5).unwrap());
    // bnodes
    let b = s.fresh_bnode().unwrap();
    let label = codec::bnode_label(b);
    assert_eq!(s.lookup(oxrdf::BlankNode::new(&label).unwrap().as_ref().into()).unwrap(), Some(b));
    assert!(s.intern(oxrdf::BlankNode::new("x").unwrap().as_ref().into()).is_err());
    // unknown
    assert_eq!(s.lookup(oxrdf::NamedNode::new("http://ex/zzz").unwrap().as_ref().into()).unwrap(), None);
}

#[test]
fn graphs_registry_and_clear() {
    let s = MemStore::new();
    let g1 = s.intern(oxrdf::NamedNode::new("http://g/1").unwrap().as_ref().into()).unwrap();
    let g2 = s.intern(oxrdf::NamedNode::new("http://g/2").unwrap().as_ref().into()).unwrap();
    let a = s.intern(oxrdf::NamedNode::new("http://ex/a").unwrap().as_ref().into()).unwrap();
    let one = TermId::from_int(1).unwrap();
    let mut q = vec![[TermId::DEFAULT_GRAPH, a, a, one], [g1, a, a, one]];
    assert_eq!(s.insert(&mut q).unwrap(), 2);
    assert_eq!(s.named_graphs().unwrap(), vec![g1]);
    assert!(s.create_graph(g2).unwrap());
    assert!(!s.create_graph(g2).unwrap());
    assert_eq!(s.named_graphs().unwrap(), vec![g1, g2]);
    assert_eq!(s.clear(GraphTarget::Named(g1)).unwrap(), 1);
    assert_eq!(s.named_graphs().unwrap(), vec![g1, g2], "CLEAR keeps the (now empty) graph registered");
    assert!(s.drop_graph(g1).unwrap());
    assert!(!s.drop_graph(g1).unwrap());
    assert_eq!(s.named_graphs().unwrap(), vec![g2]);
    assert_eq!(s.clear(GraphTarget::All).unwrap(), 1);
    assert!(s.quads().is_empty());
    let mut bad = vec![[TermId::DEFAULT_GRAPH, TermId::temp(0, Tag::Iri), a, a]];
    assert!(s.insert(&mut bad).is_err());
}

#[test]
fn analyze_exact_on_synthetic_data() {
    let s = MemStore::new();
    let iri = |i: i64| TermId::from_dict(2000 + i, Tag::Iri);
    let (ty, age, knows) = (iri(1), iri(2), iri(3));
    let class = iri(4);
    let mut q = Vec::new();
    for x in 0..500 {
        let p = iri(100 + x);
        q.push([TermId::DEFAULT_GRAPH, p, ty, class]);
        q.push([TermId::DEFAULT_GRAPH, p, age, TermId::from_int(x % 50).unwrap()]);
        if x % 2 == 0 {
            q.push([TermId::DEFAULT_GRAPH, p, knows, iri(100 + (x + 1) % 500)]);
            q.push([TermId::DEFAULT_GRAPH, p, knows, iri(100 + (x + 2) % 500)]);
        }
    }
    s.insert(&mut q).unwrap();
    let st = s.analyze();
    let d = &st.scope[0];
    assert_eq!(d.n, 1500);
    assert_eq!(d.ds, 500);
    assert_eq!(d.dp, 3);
    assert_eq!(d.pred[&ty].n, 500);
    assert_eq!(d.pred[&ty].dobj, 1);
    assert_eq!(d.pred[&age].dobj, 50);
    assert_eq!(d.pred[&age].imin, Some(0));
    assert_eq!(d.pred[&age].imax, Some(49));
    assert_eq!(d.pred[&knows].n, 500);
    assert_eq!(d.pred[&knows].ds, 250);
    assert_eq!(d.po[&(ty, class)], 500);
    assert_eq!(d.cs.len(), 2);
    assert_eq!(d.cs[0].subjects, 250);
    assert_eq!(st.scope[1].n, 0);
}
