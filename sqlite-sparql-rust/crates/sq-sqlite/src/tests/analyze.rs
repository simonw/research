//! §2.4: `analyze()` produces exactly `sq_memstore::compute_stats` on the same quads, persists
//! it in the `rdf_stat_*` tables (reloaded identically) and bumps `stats_gen`.
use sq_core::stats::{ScopeStats, Stats};
use sq_core::store::QuadStore;
use sq_core::testing::{random_quads, Rng};
use sq_core::{Tag, TermId};

use super::{store_for, Db};

fn assert_scope_eq(got: &ScopeStats, want: &ScopeStats, what: &str) {
    assert_eq!((got.n, got.ds, got.dp, got.dobj), (want.n, want.ds, want.dp, want.dobj), "{what}: globals");
    assert_eq!(got.pred.len(), want.pred.len(), "{what}: predicates");
    for (p, w) in &want.pred {
        let g = got.pred.get(p).unwrap_or_else(|| panic!("{what}: missing predicate {p:?}"));
        assert_eq!((g.n, g.ds, g.dobj, g.imin, g.imax), (w.n, w.ds, w.dobj, w.imin, w.imax), "{what}: predicate {p:?}");
    }
    assert_eq!(got.po, want.po, "{what}: heavy hitters");
    assert_eq!(got.cs.len(), want.cs.len(), "{what}: characteristic sets");
    for (i, (g, w)) in got.cs.iter().zip(&want.cs).enumerate() {
        assert_eq!((g.subjects, &g.preds), (w.subjects, &w.preds), "{what}: characteristic set {i}");
    }
}

fn assert_stats_eq(got: &Stats, want: &Stats) {
    assert_eq!(got.analyzed_quads, want.analyzed_quads, "analyzed_quads");
    assert_scope_eq(&got.scope[0], &want.scope[0], "default graph");
    assert_scope_eq(&got.scope[1], &want.scope[1], "named graphs");
}

/// Data with heavy hitters (rdf:type classes), repeated characteristic sets, int ranges,
/// triples duplicated across named graphs, and random noise in every tag.
fn dataset(seed: u64) -> Vec<[TermId; 4]> {
    let mut r = Rng(seed);
    let iri = |i: i64| TermId::from_dict(3000 + i, Tag::Iri);
    let (ty, age, knows, name) = (iri(1), iri(2), iri(3), iri(4));
    let graphs = [TermId::DEFAULT_GRAPH, iri(10), iri(11)];
    let mut q = Vec::new();
    for x in 0..1500i64 {
        let s = iri(100 + x);
        let g = graphs[(x % 7 % 3) as usize];
        q.push([g, s, ty, iri(20 + x % 3)]);
        q.push([g, s, age, TermId::from_int(x % 97 - 40).unwrap()]);
        if x % 2 == 0 {
            q.push([g, s, knows, iri(100 + (x + 1) % 1500)]);
            q.push([g, s, knows, iri(100 + (x + 7) % 1500)]);
        }
        if x % 5 == 0 {
            q.push([g, s, name, TermId::from_dict(9000 + x, Tag::Str)]);
            // the same triple in another named graph (RDF merge of scope 1 dedupes it)
            q.push([graphs[1 + (x % 2) as usize], s, name, TermId::from_dict(9000 + x, Tag::Str)]);
        }
        if r.below(4) == 0 {
            q.push([g, s, iri(5), sq_core::testing::random_id(&mut r, 30)]);
        }
    }
    q.extend(random_quads(seed, 3000, true));
    q.sort();
    q.dedup();
    q
}

#[test]
fn analyze_equals_compute_stats() {
    for seed in [1u64, 2] {
        let db = Db::file();
        let quads = dataset(seed);
        db.insert(&quads);
        let want = sq_memstore::compute_stats(&quads);
        assert!(!want.scope[0].po.is_empty() && !want.scope[1].po.is_empty(), "data has heavy hitters");
        assert!(want.scope[0].cs.len() > 2 && want.scope[1].n > 0);
        let gen0 = db.query_i64("SELECT value FROM rdf_meta WHERE key='stats_gen'");
        let got = db.store.analyze().unwrap();
        assert_stats_eq(&got, &want);
        assert_eq!(db.query_i64("SELECT value FROM rdf_meta WHERE key='stats_gen'"), gen0 + 1);
        assert_eq!(db.query_i64("SELECT value FROM rdf_meta WHERE key='analyzed_quads'"), quads.len() as i64);
        // The cached copy and a fresh reload from the tables are identical too.
        assert_stats_eq(&db.store.stats().unwrap(), &want);
        let other = store_for(&db.conn);
        other.shared().validate_epoch().unwrap();
        assert_stats_eq(&other.stats().unwrap(), &want);
        drop(other);
        // A second connection sees the new statistics after its epoch check.
        let second = db.reopen();
        second.shared().validate_epoch().unwrap();
        assert_stats_eq(&second.store.stats().unwrap(), &want);
    }
}

#[test]
fn stats_none_until_analyzed_and_reloaded_on_stats_gen_change() {
    let db = Db::file();
    db.shared().validate_epoch().unwrap();
    assert!(db.store.stats().is_none());
    let quads = dataset(7);
    db.insert(&quads);
    let reader = db.reopen();
    reader.shared().validate_epoch().unwrap();
    assert!(reader.store.stats().is_none());
    db.store.analyze().unwrap();
    // Not reloaded before the next epoch check …
    assert!(reader.store.stats().is_none());
    // … but right after it.
    reader.shared().validate_epoch().unwrap();
    assert_eq!(reader.store.stats().unwrap().analyzed_quads, quads.len() as u64);
    // Analyze inside a rolled-back transaction leaves the committed statistics in force.
    let mut more = quads.clone();
    for q in &mut more {
        q[3] = TermId::from_int(1).unwrap();
    }
    db.insert(&more);
    db.conn.execute_batch("BEGIN").unwrap();
    let provisional = db.store.analyze().unwrap();
    assert_ne!(provisional.analyzed_quads, quads.len() as u64);
    db.conn.execute_batch("ROLLBACK").unwrap();
    db.shared().validate_epoch().unwrap();
    assert_eq!(db.store.stats().unwrap().analyzed_quads, quads.len() as u64);
}

#[test]
fn analyze_empty_store() {
    let db = Db::mem();
    let st = db.store.analyze().unwrap();
    assert_stats_eq(&st, &sq_memstore::compute_stats(&[]));
    assert!(db.store.stats().is_some(), "analyzed (empty) is not 'never analyzed'");
}

/// §2.3 step 6 bookkeeping: loads add to `loaded_since_analyze`, every analyze resets it.
#[test]
fn analyze_resets_the_load_counter() {
    use crate::{BulkMode, LoadOptions, LoadSource};
    let db = Db::mem();
    let counter = || db.query_i64("SELECT coalesce((SELECT value FROM rdf_meta WHERE key='loaded_since_analyze'), 0)");
    let opts = LoadOptions { bulk: BulkMode::Never, ..LoadOptions::new(oxrdfio::RdfFormat::NTriples) };
    let nt = |from: usize, to: usize| -> String { (from..to).map(|i| format!("<http://ex/s{i}> <http://ex/p> <http://ex/o{i}> .\n")).collect() };
    db.store.bulk_load(LoadSource::Bytes(nt(0, 100).as_bytes()), &opts).unwrap();
    assert_eq!((counter(), db.query_i64("SELECT value FROM rdf_meta WHERE key='analyzed_quads'")), (0, 100));
    db.store.bulk_load(LoadSource::Bytes(nt(100, 110).as_bytes()), &opts).unwrap();
    db.store.bulk_load(LoadSource::Bytes(nt(105, 120).as_bytes()), &opts).unwrap();
    assert_eq!(counter(), 20, "new quads only");
    db.store.analyze().unwrap();
    assert_eq!(counter(), 0);
    // > 25% of analyzed_quads (120) since the last analyze: 31 new quads re-analyze.
    db.store.bulk_load(LoadSource::Bytes(nt(120, 140).as_bytes()), &opts).unwrap();
    assert_eq!(counter(), 20);
    db.store.bulk_load(LoadSource::Bytes(nt(140, 151).as_bytes()), &opts).unwrap();
    assert_eq!((counter(), db.query_i64("SELECT value FROM rdf_meta WHERE key='analyzed_quads'")), (0, 151));
}
