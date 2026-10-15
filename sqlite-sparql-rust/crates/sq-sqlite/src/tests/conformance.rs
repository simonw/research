//! `sq_core::testing::cursor_conformance` against SQL cursors, exactly like
//! `sq-memstore`'s tests, on `:memory:` and on a file database.
use std::ops::Bound;

use sq_core::store::{IndexOrder, QuadStore, ScanSpec};
use sq_core::testing::{cursor_conformance, random_quads};
use sq_core::{Tag, TermId};

use super::Db;

fn all_quads(db: &Db) -> Vec<[TermId; 4]> {
    let mut cur = db.store.cursor(&ScanSpec::new(IndexOrder::Gspo, &[])).unwrap();
    let mut buf = Vec::new();
    while cur.fill(&mut buf, 1024).unwrap() > 0 {}
    buf.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect()
}

fn run(db: &Db, quads: &[[TermId; 4]], seed: u64, iterations: usize) {
    let _read = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    cursor_conformance(&db.store, quads, seed, iterations);
}

#[test]
fn conformance_default_graph_memory() {
    for seed in [1u64, 2, 3] {
        let q = random_quads(seed, 2000, false);
        let db = Db::mem();
        db.insert(&q);
        run(&db, &q, seed, 4);
    }
}

#[test]
fn conformance_named_graphs_memory() {
    for seed in [11u64, 12, 13] {
        let q = random_quads(seed, 2000, true);
        let db = Db::mem();
        db.insert(&q);
        run(&db, &q, seed, 4);
    }
}

#[test]
fn conformance_named_graphs_file() {
    for seed in [31u64, 32] {
        let q = random_quads(seed, 3000, true);
        let db = Db::file();
        db.insert(&q);
        run(&db, &q, seed, 4);
    }
}

fn deletes_and_reinserts(db: &Db, seed: u64, n: usize, iterations: usize) {
    let q = random_quads(seed, n, true);
    db.insert(&q);
    let del: Vec<_> = q.iter().copied().step_by(3).collect();
    assert_eq!(db.remove(&del), del.len() as u64);
    let re: Vec<_> = del.iter().copied().step_by(2).collect();
    assert_eq!(db.insert(&re), re.len() as u64);
    let expect = all_quads(db);
    let del_set: std::collections::BTreeSet<_> = del.iter().copied().collect();
    let re_set: std::collections::BTreeSet<_> = re.iter().copied().collect();
    let mut want: Vec<_> = q.iter().copied().filter(|x| !del_set.contains(x) || re_set.contains(x)).collect();
    want.sort();
    assert_eq!(expect, want);
    run(db, &expect, seed, iterations);
}

#[test]
fn conformance_after_deletes_and_reinserts_memory() {
    deletes_and_reinserts(&Db::mem(), 21, 3000, 3);
}

#[test]
fn conformance_after_deletes_and_reinserts_file() {
    deletes_and_reinserts(&Db::file(), 22, 3000, 3);
}

/// §12.1: 100K quads on a file database, named graphs, deletes and re-inserts.
#[test]
fn conformance_100k_file() {
    let db = Db::file();
    db.conn.execute_batch("PRAGMA page_size=8192; VACUUM;").unwrap();
    deletes_and_reinserts(&db, 41, 100_000, 1);
}

/// The forward-seek fast path (stepping instead of re-seeking) and every seek width, checked
/// row by row against a brute-force expectation on long runs.
#[test]
fn galloping_seeks_match_brute_force() {
    let db = Db::mem();
    let p = TermId::from_dict(1100, Tag::Iri);
    let mut quads = Vec::new();
    for s in 0..200i64 {
        for o in 0..5i64 {
            quads.push([TermId::DEFAULT_GRAPH, TermId::from_dict(2000 + s, Tag::Iri), p, TermId::from_int(o * 3 - 4).unwrap()]);
        }
    }
    db.insert(&quads);
    let spec = ScanSpec::new(IndexOrder::Gpso, &[TermId::DEFAULT_GRAPH, p]);
    let mut cur = db.store.cursor(&spec).unwrap();
    let rows: Vec<[TermId; 2]> = quads.iter().map(|q| [q[1], q[3]]).collect();
    let mut buf = Vec::new();
    // Interleave small fills with forward seeks of 1 and 2 columns (short and long jumps).
    let mut pos = 0usize;
    let mut step = 1usize;
    while pos < rows.len() {
        let key = rows[pos];
        if step.is_multiple_of(2) {
            cur.seek(&key[..1]).unwrap();
            // first row with s >= key.s
            pos = rows.partition_point(|r| r[0] < key[0]);
        } else {
            cur.seek(&key).unwrap();
        }
        buf.clear();
        let n = cur.fill(&mut buf, 3).unwrap();
        let want: Vec<TermId> = rows[pos..(pos + 3).min(rows.len())].iter().flatten().copied().collect();
        assert_eq!(n, want.len() / 2);
        assert_eq!(buf, want, "seek to {key:?}");
        pos += 1 + step % 7;
        step += 1;
    }
    let c = cur.counters();
    assert!(c.seeks > 0 && c.rows > 0);
    // Seeks below an excluded lower bound stay within the bound (Gt template, 1 and 2 cols).
    let lo = TermId::from_dict(2000 + 50, Tag::Iri).0;
    let spec = spec.with_range(Bound::Excluded(lo), Bound::Excluded(TermId::from_dict(2000 + 60, Tag::Iri).0));
    let mut cur = db.store.cursor(&spec).unwrap();
    for key in [vec![TermId::from_dict(2000, Tag::Iri)], vec![TermId::from_dict(2000, Tag::Iri), TermId(0)]] {
        cur.seek(&key).unwrap();
        buf.clear();
        while cur.fill(&mut buf, 4).unwrap() > 0 {}
        assert_eq!(buf.len() / 2, 9 * 5, "rows of s in (50, 60)");
        assert!(buf.chunks_exact(2).all(|r| r[0].0 > lo));
    }
    // Width-0 cursor (complete prefix): one row if the quad exists, then exhausted.
    let q = quads[7];
    let mut cur = db.store.cursor(&ScanSpec::new(IndexOrder::Gspo, &q)).unwrap();
    buf.clear();
    assert_eq!(cur.fill(&mut buf, 10).unwrap(), 1);
    assert_eq!(cur.fill(&mut buf, 10).unwrap(), 0);
    assert!(buf.is_empty());
    assert!(cur.seek(&[TermId(1)]).is_err(), "key wider than the cursor");
}

/// Contradictory bounds never reach SQLite and count as empty.
#[test]
fn empty_ranges() {
    let db = Db::mem();
    let q = random_quads(5, 200, false);
    db.insert(&q);
    for (lo, hi) in [
        (Bound::Excluded(i64::MAX), Bound::Unbounded),
        (Bound::Unbounded, Bound::Excluded(i64::MIN)),
        (Bound::Included(10), Bound::Included(9)),
        (Bound::Excluded(9), Bound::Excluded(10)),
    ] {
        let spec = ScanSpec::new(IndexOrder::Gspo, &[]).with_range(lo, hi);
        let mut cur = db.store.cursor(&spec).unwrap();
        let mut buf = Vec::new();
        assert_eq!(cur.fill(&mut buf, 100).unwrap(), 0);
        cur.seek(&[TermId(5), TermId(5)]).unwrap();
        assert_eq!(cur.fill(&mut buf, 100).unwrap(), 0);
        assert_eq!(db.store.count(&spec, None).unwrap(), 0);
        assert_eq!(db.store.count(&spec, Some(3)).unwrap(), 0);
    }
}
