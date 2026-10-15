//! Phase-2 reader vs SQL cursors on a 1M-triple file database (release only):
//! `cargo test -p sq-sqlite --release -- --ignored --nocapture`.
//!
//! Reports ns/row for full pinned scans (2 free columns: GPSO (g, p) -> s, o; 3 free columns:
//! GSPO (g) -> s, p, o, the Q16 shape) and µs/probe for point probes (rebind + fill) with
//! sorted and random subjects. The B-tree reader is measured with a warm page cache and a
//! cold one (flushed before every round; the OS page cache stays warm, so a cold page costs
//! one pread through the VFS). SQLite's own page cache is warm in every SQL measurement.
use std::ops::Bound;
use std::time::Instant;

use oxrdfio::RdfFormat;
use sq_core::TermId;
use sq_core::codec::RDF_TYPE;
use sq_core::store::{Dictionary, IndexOrder, QuadStore, ReaderKind, ScanSpec};
use sq_core::testing::Rng;

use super::loader::social_ntriples;
use super::{Db, iri};
use crate::{BulkMode, LoadOptions, LoadSource, ReaderMode};

const PERSONS: usize = 142_000; // ~1.0M triples
/// Best-of rounds (the machine is shared): scans are cheap, so they get more rounds.
const SCAN_ROUNDS: usize = 9;
const PROBE_ROUNDS: usize = 5;

#[derive(Copy, Clone, PartialEq, Debug)]
enum Reader {
    Sql,
    BtreeWarm,
    BtreeCold,
}

impl Reader {
    fn label(self) -> &'static str {
        match self {
            Reader::Sql => "SqlCursor        ",
            Reader::BtreeWarm => "B-tree (warm)    ",
            Reader::BtreeCold => "B-tree (cold)    ",
        }
    }
}

/// Best-of-`rounds` ns per unit of `f` (which returns the number of units it processed).
fn measure(db: &Db, reader: Reader, rounds: usize, mut f: impl FnMut() -> usize) -> f64 {
    db.shared().set_reader(if reader == Reader::Sql { ReaderMode::Sql } else { ReaderMode::Btree });
    if reader == Reader::BtreeWarm {
        f(); // warm the page cache
    }
    (0..rounds)
        .map(|_| {
            if reader == Reader::BtreeCold {
                db.shared().clear_page_cache().unwrap();
            }
            let t = Instant::now();
            let units = f();
            t.elapsed().as_nanos() as f64 / units.max(1) as f64
        })
        .fold(f64::MAX, f64::min)
}

fn scan(db: &Db, spec: &ScanSpec, expect: ReaderKind) -> usize {
    let mut cur = db.store.cursor(spec).unwrap();
    assert_eq!(cur.counters().reader, expect);
    let mut out = Vec::with_capacity(4 * 1024);
    let mut rows = 0;
    loop {
        out.clear();
        let n = cur.fill(&mut out, 1024).unwrap();
        if n == 0 {
            break;
        }
        rows += n;
    }
    rows
}

#[test]
#[ignore = "performance comparison: run with --release -- --ignored --nocapture"]
fn perf_btree_reader_vs_sql_cursor_1m_triples() {
    if cfg!(debug_assertions) {
        eprintln!("skipped: performance numbers are only meaningful with --release");
        return;
    }
    let _serial = super::perf_lock();
    let db = Db::file();
    db.conn.execute_batch("PRAGMA page_size=8192; VACUUM; PRAGMA cache_size=-524288;").unwrap();
    let data = social_ntriples(PERSONS);
    let opts = LoadOptions { bulk: BulkMode::Auto, ..LoadOptions::new(RdfFormat::NTriples) };
    let report = db.store.bulk_load(LoadSource::Bytes(data.as_bytes()), &opts).unwrap();
    assert!(report.quads_new > 990_000);
    let file_mb = std::fs::metadata(&db.path.as_ref().unwrap().0).unwrap().len() as f64 / 1e6;
    println!("1M-triple file DB: {} quads, {file_mb:.0} MB, page size 8192", report.quads_new);

    let _read = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    assert!(db.shared().btree_active());
    let kind = |r: Reader| if r == Reader::Sql { ReaderKind::SqlCursor } else { ReaderKind::Btree };
    let readers = [Reader::Sql, Reader::BtreeWarm, Reader::BtreeCold];

    // Full pinned scans.
    let knows = db.store.lookup(iri("http://ex.org/knows").as_ref().into()).unwrap().unwrap();
    let two = ScanSpec::new(IndexOrder::Gpso, &[TermId::DEFAULT_GRAPH, knows]);
    let three = ScanSpec::new(IndexOrder::Gspo, &[TermId::DEFAULT_GRAPH]);
    let mut scan_ns = Vec::new();
    for (name, spec) in [("2-column scan GPSO(g,p) -> s,o", &two), ("3-column scan GSPO(g) -> s,p,o", &three)] {
        db.shared().set_reader(ReaderMode::Sql);
        let rows = scan(&db, spec, ReaderKind::SqlCursor);
        println!("{name}: {rows} rows");
        for r in readers {
            let ns = measure(&db, r, SCAN_ROUNDS, || scan(&db, spec, kind(r)));
            println!("  {} {ns:6.1} ns/row", r.label());
            scan_ns.push(ns);
        }
    }

    // Point probes (g=0, s, rdf:type) -> 1 row, through both orders whose prefix covers
    // {g, s, p}; one cursor per run, rebound per probe (the INL join shape).
    let ids: Vec<TermId> = (0..PERSONS)
        .map(|i| db.store.lookup(iri(&format!("http://ex.org/person/{i}")).as_ref().into()).unwrap().unwrap())
        .collect();
    let mut rng = Rng(7);
    let random: Vec<TermId> = (0..200_000).map(|_| ids[rng.below(ids.len() as u64) as usize]).collect();
    let mut sorted = random.clone();
    sorted.sort();
    let mut probe_ns = Vec::new();
    for order in [IndexOrder::Gspo, IndexOrder::Gpso] {
        let prefix = |s: TermId| match order {
            IndexOrder::Gspo => [TermId::DEFAULT_GRAPH, s, RDF_TYPE],
            _ => [TermId::DEFAULT_GRAPH, RDF_TYPE, s],
        };
        for (label, subjects) in [("sorted", &sorted), ("random", &random)] {
            println!("point probes {order:?} {label} (rebind + fill):");
            for r in readers {
                let ns = measure(&db, r, PROBE_ROUNDS, || {
                    let mut cur = db.store.cursor(&ScanSpec::new(order, &prefix(subjects[0]))).unwrap();
                    assert_eq!(cur.counters().reader, kind(r));
                    let mut buf = Vec::with_capacity(16);
                    let mut found = 0;
                    for s in subjects.iter() {
                        cur.rebind(&prefix(*s), Bound::Unbounded, Bound::Unbounded).unwrap();
                        buf.clear();
                        found += cur.fill(&mut buf, 16).unwrap();
                    }
                    assert_eq!(found, subjects.len());
                    subjects.len()
                });
                println!("  {} {:6.3} µs/probe", r.label(), ns / 1000.0);
                probe_ns.push(ns);
            }
        }
    }
    // Cost of the per-call precondition check (txn_state + data version).
    db.shared().set_reader(ReaderMode::Btree);
    let mut cur = db.store.cursor(&two).unwrap();
    let mut buf = Vec::new();
    cur.fill(&mut buf, 1).unwrap();
    const CALLS: usize = 200_000;
    let t = Instant::now();
    for _ in 0..CALLS {
        buf.clear();
        cur.fill(&mut buf, 1).unwrap();
    }
    println!("fill(1) on a pinned leaf (incl. precondition check): {:.0} ns/call", t.elapsed().as_nanos() as f64 / CALLS as f64);
    let c = db.shared().btree_counters();
    println!("reader counters: {c:?}");
    assert_eq!(c.fallbacks, 0);

    // §15 WP15 acceptance: >= 2.5x scan speed-up with a warm page cache.
    for (i, name) in ["2-column", "3-column"].into_iter().enumerate() {
        let (sql, warm) = (scan_ns[3 * i], scan_ns[3 * i + 1]);
        println!("{name} scan speed-up (warm): {:.1}x", sql / warm);
        assert!(sql / warm >= 2.5, "{name} scan: B-tree {warm:.1} ns/row vs SQL {sql:.1} ns/row");
    }
    // Sorted probes must not be slower than through SQL.
    for (i, name) in ["GSPO sorted", "GPSO sorted"].into_iter().enumerate() {
        let (sql, warm) = (probe_ns[6 * i], probe_ns[6 * i + 1]);
        assert!(warm < sql, "{name} probes: B-tree {warm:.0} ns vs SQL {sql:.0} ns");
    }
}
