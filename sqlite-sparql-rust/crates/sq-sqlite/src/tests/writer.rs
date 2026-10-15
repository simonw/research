//! `QuadStoreWriter` behaves exactly like `sq_memstore::MemStore`.
use sq_core::store::{Dictionary, GraphTarget, IndexOrder, QuadStore, QuadStoreWriter, ScanSpec};
use sq_core::testing::{random_quads, Rng};
use sq_core::{Error, Tag, TermId};
use sq_memstore::MemStore;

use super::{iri, Db};

fn sqlite_quads(db: &Db) -> Vec<[TermId; 4]> {
    let mut cur = db.store.cursor(&ScanSpec::new(IndexOrder::Gspo, &[])).unwrap();
    let mut buf = Vec::new();
    while cur.fill(&mut buf, 1000).unwrap() > 0 {}
    buf.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect()
}

#[test]
fn graphs_registry_and_clear() {
    let db = Db::mem();
    let s = &db.store;
    let g1 = s.intern(iri("http://g/1").as_ref().into()).unwrap();
    let g2 = s.intern(iri("http://g/2").as_ref().into()).unwrap();
    let a = s.intern(iri("http://ex/a").as_ref().into()).unwrap();
    let one = TermId::from_int(1).unwrap();
    let mut q = vec![[TermId::DEFAULT_GRAPH, a, a, one], [g1, a, a, one], [g1, a, a, one]];
    assert_eq!(s.insert(&mut q).unwrap(), 2);
    assert_eq!(q.len(), 2, "sorted and deduplicated in place");
    assert_eq!(s.named_graphs().unwrap(), vec![g1]);
    assert!(s.create_graph(g2).unwrap());
    assert!(!s.create_graph(g2).unwrap());
    assert!(!s.create_graph(g1).unwrap(), "a graph with quads already exists");
    assert_eq!(s.named_graphs().unwrap(), vec![g1, g2]);
    assert_eq!(s.clear(GraphTarget::Named(g1)).unwrap(), 1);
    assert_eq!(s.named_graphs().unwrap(), vec![g1, g2], "CLEAR keeps the (now empty) graph registered");
    assert!(s.drop_graph(g1).unwrap());
    assert!(!s.drop_graph(g1).unwrap());
    assert_eq!(s.named_graphs().unwrap(), vec![g2]);
    assert_eq!(s.clear(GraphTarget::All).unwrap(), 1);
    assert!(sqlite_quads(&db).is_empty());
    let mut bad = vec![[TermId::DEFAULT_GRAPH, TermId::temp(0, Tag::Iri), a, a]];
    assert!(matches!(s.insert(&mut bad), Err(Error::Internal(_))));
    assert!(s.create_graph(TermId::DEFAULT_GRAPH).is_err());
    assert!(s.create_graph(TermId::temp(3, Tag::Iri)).is_err());
}

#[test]
fn named_graphs_include_unregistered_graphs_and_negative_ids() {
    let db = Db::mem();
    let p = TermId::from_dict(1100, Tag::Iri);
    let gs = [TermId::from_int(-7).unwrap(), TermId::from_dict(2001, Tag::Iri), TermId::bnode(4), TermId::from_dict(2003, Tag::Iri)];
    for g in gs {
        db.conn.execute("INSERT INTO rdf_quad(g,s,p,o) VALUES (?1,?2,?2,?2)", [g.0, p.0]).unwrap();
    }
    db.conn.execute("INSERT INTO rdf_graph(g) VALUES (?1)", [TermId::from_dict(2002, Tag::Iri).0]).unwrap();
    let mut want: Vec<TermId> = gs.to_vec();
    want.push(TermId::from_dict(2002, Tag::Iri));
    want.sort();
    assert_eq!(db.store.named_graphs().unwrap(), want);
}

/// Random operation sequences give identical results and states on both stores.
#[test]
fn random_operations_match_memstore() {
    for seed in [3u64, 4, 5] {
        let db = Db::mem();
        let mem = MemStore::new();
        let pool = random_quads(seed, 600, true);
        let graphs: Vec<TermId> = (0..4).map(|i| TermId::from_dict(2000 + i, Tag::Iri)).collect();
        let mut r = Rng(seed);
        for step in 0..200 {
            let pick = |r: &mut Rng, n: usize| -> Vec<[TermId; 4]> {
                (0..1 + r.below(n as u64) as usize).map(|_| pool[r.below(pool.len() as u64) as usize]).collect()
            };
            let (a, b) = match r.below(10) {
                0..=3 => {
                    let v = pick(&mut r, 40);
                    (db.store.insert(&mut v.clone()).unwrap(), mem.insert(&mut v.clone()).unwrap())
                }
                4..=5 => {
                    let v = pick(&mut r, 40);
                    (db.store.remove(&mut v.clone()).unwrap(), mem.remove(&mut v.clone()).unwrap())
                }
                6 => {
                    let t = match r.below(4) {
                        0 => GraphTarget::Default,
                        1 => GraphTarget::AllNamed,
                        2 => GraphTarget::All,
                        _ => GraphTarget::Named(graphs[r.below(4) as usize]),
                    };
                    (db.store.clear(t).unwrap(), mem.clear(t).unwrap())
                }
                7 | 8 => {
                    let g = graphs[r.below(4) as usize];
                    (db.store.create_graph(g).unwrap() as u64, mem.create_graph(g).unwrap() as u64)
                }
                _ => {
                    let g = graphs[r.below(4) as usize];
                    (db.store.drop_graph(g).unwrap() as u64, mem.drop_graph(g).unwrap() as u64)
                }
            };
            assert_eq!(a, b, "seed {seed} step {step}: result");
            assert_eq!(sqlite_quads(&db), mem.quads(), "seed {seed} step {step}: quads");
            assert_eq!(db.store.named_graphs().unwrap(), mem.named_graphs().unwrap(), "seed {seed} step {step}: graphs");
        }
    }
}

#[test]
fn writes_inside_a_rolled_back_scope_vanish() {
    let db = Db::mem();
    let q = random_quads(9, 50, true);
    {
        let _ws = db.store.write_scope(0).unwrap();
        db.insert(&q);
        db.store.intern(iri("http://ex/temporary").as_ref().into()).unwrap();
        db.store.fresh_bnode().unwrap();
        assert!(!sqlite_quads(&db).is_empty());
    }
    assert!(sqlite_quads(&db).is_empty());
    assert!(db.store.named_graphs().unwrap().is_empty());
    assert_eq!(db.store.lookup(iri("http://ex/temporary").as_ref().into()).unwrap(), None);
    assert_eq!(db.store.fresh_bnode().unwrap(), TermId::bnode(1));
}
