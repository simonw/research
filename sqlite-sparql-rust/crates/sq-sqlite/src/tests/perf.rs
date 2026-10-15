//! Micro budgets (§12.1), run in release mode:
//! `cargo test -p sq-sqlite --release -- --ignored --nocapture`.
//! * 1M-triple N-Triples bulk load < 10 s;
//! * point probe (rebind + fill) inside a held read transaction < 1.5 µs for sorted probe
//!   batches through both orders — the shape the planner's index-nested-loop joins issue
//!   (sorted, deduplicated probes, §0). Random probes are only reported: they are
//!   memory-bound (~1.4–2.5 µs on this shared machine, the same for the raw SQLite statement)
//!   and would make the gate flaky;
//! * 2-column pinned scan < 150 ns/row.
//!
//! These budgets are for SQL cursors (`reader = sql`); `btree_perf` compares them with the
//! phase-2 B-tree reader.
use std::ops::Bound;
use std::time::Instant;

use oxrdfio::RdfFormat;
use sq_core::codec::RDF_TYPE;
use sq_core::store::{Dictionary, IndexOrder, QuadStore, ScanSpec};
use sq_core::testing::Rng;
use sq_core::TermId;

use super::loader::social_ntriples;
use super::{iri, Db};
use crate::raw::RawStmt;
use crate::template::{StmtKey, StmtKind};
use crate::{BulkMode, LoadOptions, LoadSource, ReaderMode};

const PERSONS: usize = 142_000; // ~1.0M triples
/// Micro-benchmarks report the best of this many rounds (the machine is shared).
const ROUNDS: usize = 5;

fn best_of(rounds: usize, mut f: impl FnMut() -> f64) -> f64 {
    (0..rounds).map(|_| f()).fold(f64::MAX, f64::min)
}

#[test]
#[ignore = "performance budget: run with --release -- --ignored"]
fn perf_budgets_1m_triples() {
    if cfg!(debug_assertions) {
        eprintln!("skipped: performance budgets are only meaningful with --release");
        return;
    }
    let _serial = super::perf_lock();
    let db = Db::file();
    db.shared().set_reader(ReaderMode::Sql);
    db.conn.execute_batch("PRAGMA page_size=8192; VACUUM; PRAGMA cache_size=-524288;").unwrap();
    let data = social_ntriples(PERSONS);
    let opts = LoadOptions { bulk: BulkMode::Auto, ..LoadOptions::new(RdfFormat::NTriples) };
    let t = Instant::now();
    let report = db.store.bulk_load(LoadSource::Bytes(data.as_bytes()), &opts).unwrap();
    let load_s = t.elapsed().as_secs_f64();
    println!(
        "bulk load: {} triples, {} terms, bulk={} in {load_s:.2} s ({:.0} triples/s)",
        report.quads_new,
        report.terms_new,
        report.bulk,
        report.quads_new as f64 / load_s
    );
    assert!(report.bulk && report.quads_new > 990_000);

    let read = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    let ids: Vec<TermId> = (0..PERSONS)
        .map(|i| db.store.lookup(iri(&format!("http://ex.org/person/{i}")).as_ref().into()).unwrap().unwrap())
        .collect();

    // Point probes for (g=0, s, rdf:type) -> 1 row: random and sorted subjects, through both
    // orders whose prefix covers {g, s, p} (IndexOrder::candidates), one rebinding cursor each.
    let mut r = Rng(7);
    let probes: Vec<TermId> = (0..200_000).map(|_| ids[r.below(ids.len() as u64) as usize]).collect();
    let mut sorted = probes.clone();
    sorted.sort();
    let probe = |order: IndexOrder, subjects: &[TermId]| -> f64 {
        let prefix = |s: TermId| match order {
            IndexOrder::Gspo => [TermId::DEFAULT_GRAPH, s, RDF_TYPE],
            _ => [TermId::DEFAULT_GRAPH, RDF_TYPE, s],
        };
        let mut cur = db.store.cursor(&ScanSpec::new(order, &prefix(subjects[0]))).unwrap();
        let mut buf = Vec::with_capacity(16);
        best_of(ROUNDS, || {
            let mut found = 0;
            let t = Instant::now();
            for s in subjects {
                cur.rebind(&prefix(*s), Bound::Unbounded, Bound::Unbounded).unwrap();
                buf.clear();
                found += cur.fill(&mut buf, 16).unwrap();
            }
            assert_eq!(found, subjects.len());
            t.elapsed().as_nanos() as f64 / subjects.len() as f64
        })
    };
    probe(IndexOrder::Gpso, &probes); // warm-up
    let gspo_random = probe(IndexOrder::Gspo, &probes);
    let gspo_sorted = probe(IndexOrder::Gspo, &sorted);
    let gpso_random = probe(IndexOrder::Gpso, &probes);
    let gpso_sorted = probe(IndexOrder::Gpso, &sorted);
    println!(
        "point probe (rebind+fill, held read txn): GSPO(g,s,p) {gspo_random:.0} ns random / {gspo_sorted:.0} ns sorted; \
         GPSO(g,p,s) {gpso_random:.0} ns random / {gpso_sorted:.0} ns sorted"
    );
    // Raw SQLite on the GSPO template, for comparison (the cursor's own overhead).
    let raw_ns = {
        let sql = StmtKey::for_spec(&ScanSpec::new(IndexOrder::Gspo, &[TermId(0), TermId(0), TermId(0)]), StmtKind::Scan).sql();
        let mut st = RawStmt::prepare(db.shared().db(), &sql, true).unwrap();
        best_of(ROUNDS, || {
            let t = Instant::now();
            for s in &probes {
                st.reset();
                for (i, v) in [0, s.0, RDF_TYPE.0, i64::MIN].into_iter().enumerate() {
                    st.bind_i64(i as i32 + 1, v).unwrap();
                }
                while st.step().unwrap() {
                    std::hint::black_box(st.col_i64(0));
                }
            }
            t.elapsed().as_nanos() as f64 / probes.len() as f64
        })
    };
    println!("  raw SQLite, same GSPO statement: {raw_ns:.0} ns");

    // Pinned 2-column scan: (g=0, p=knows) -> s, o.
    let knows = db.store.lookup(iri("http://ex.org/knows").as_ref().into()).unwrap().unwrap();
    let spec = ScanSpec::new(IndexOrder::Gpso, &[TermId::DEFAULT_GRAPH, knows]);
    let mut rows = 0;
    let best = best_of(ROUNDS, || {
        let mut cur = db.store.cursor(&spec).unwrap();
        let mut out = Vec::with_capacity(2 * 1024);
        rows = 0;
        let t = Instant::now();
        loop {
            out.clear();
            let n = cur.fill(&mut out, 1024).unwrap();
            if n == 0 {
                break;
            }
            rows += n;
        }
        t.elapsed().as_nanos() as f64 / rows as f64
    });
    println!("2-column pinned scan: {rows} rows, {best:.0} ns/row");
    // count(*) in the VDBE, for reference.
    let t = Instant::now();
    let n = db.store.count(&ScanSpec::new(IndexOrder::Gspo, &[TermId::DEFAULT_GRAPH]), None).unwrap();
    let count_ns = t.elapsed().as_nanos() as f64 / n as f64;
    println!("count(*) in the VDBE: {n} rows, {count_ns:.1} ns/row");
    // Decode: misses (batches of 1024 dictionary ids, cache flushed first) and hits.
    let batch: Vec<TermId> = ids[..50_000].to_vec();
    let mut terms = Vec::with_capacity(batch.len());
    let mut decode_all = || {
        terms.clear();
        let t = Instant::now();
        for chunk in batch.chunks(1024) {
            db.store.decode(chunk, &mut terms).unwrap();
        }
        t.elapsed().as_nanos() as f64 / batch.len() as f64
    };
    let miss_ns = best_of(ROUNDS, || {
        db.shared().flush_dictionary_caches().unwrap();
        decode_all()
    });
    let hit_ns = best_of(ROUNDS, &mut decode_all);
    println!("decode: {miss_ns:.0} ns/term miss (batched), {hit_ns:.0} ns/term cache hit");
    drop(read);
    // Entry overhead of a bare `SELECT f()` call: read scope (savepoint, forced-rollback
    // guard refresh, release) plus the §4.4 epoch check.
    const CALLS: u32 = 20_000;
    let entry_ns = best_of(ROUNDS, || {
        let t = Instant::now();
        for _ in 0..CALLS {
            let scope = db.store.read_scope(0).unwrap();
            db.shared().validate_epoch().unwrap();
            scope.close().unwrap();
        }
        t.elapsed().as_nanos() as f64 / CALLS as f64
    });
    println!("bare-call entry (read scope + epoch check): {entry_ns:.0} ns");

    assert!(load_s < 10.0, "1M-triple bulk load took {load_s:.2} s (budget 10 s)");
    assert!(gspo_sorted < 1500.0, "sorted GSPO point probe {gspo_sorted:.0} ns (budget 1500 ns)");
    assert!(gpso_sorted < 1500.0, "sorted GPSO point probe {gpso_sorted:.0} ns (budget 1500 ns)");
    assert!(best < 150.0, "2-column scan {best:.0} ns/row (budget 150 ns)");
}
