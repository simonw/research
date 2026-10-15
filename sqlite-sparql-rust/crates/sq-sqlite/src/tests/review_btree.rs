//! Adversarial review tests (WP15, the direct B-tree reader). Each `review_*` test pins one
//! claim of the package: transparent fallback / resume at the exact key under random
//! interleavings, b-tree reorganisations while a cursor is positioned, extreme key values,
//! root-page moves and file truncation, page-size / journal-mode changes, targeted page
//! corruption, no reads of another connection's uncommitted data (shared cache), and the cost
//! of the per-call precondition checks (release, ignored).
use std::collections::BTreeSet;
use std::ops::Bound;
use std::time::Instant;

use rusqlite::{Connection, ffi};
use sq_core::store::{IndexOrder, QuadCursor, QuadStore, QuadStoreWriter, ReaderKind, ScanSpec};
use sq_core::testing::{Rng, cursor_conformance, expected_rows, random_id, random_quads};
use sq_core::{Tag, TermId};

use super::{Db, TempPath};

fn file_db_with(pragmas: &str, quads: &[[TermId; 4]]) -> Db {
    let path = TempPath::new("rbt");
    let conn = Connection::open(&path.0).unwrap();
    conn.execute_batch(pragmas).unwrap();
    let db = Db::with(conn, Some(path));
    db.insert(quads);
    db
}

fn last_fallback(db: &Db) -> String {
    db.shared().status().into_iter().find(|(k, _)| k == "btree_last_fallback").map(|(_, v)| v).unwrap_or_default()
}

fn drain(cur: &mut dyn QuadCursor, max: usize) -> Vec<Vec<TermId>> {
    let w = cur.width();
    let mut buf = Vec::new();
    let mut rows = 0;
    loop {
        let n = cur.fill(&mut buf, max).unwrap();
        if n == 0 {
            break;
        }
        rows += n;
    }
    assert_eq!(buf.len(), rows * w);
    (0..rows).map(|i| buf[i * w..(i + 1) * w].to_vec()).collect()
}

fn random_bound(r: &mut Rng, rows: &[Vec<TermId>]) -> Bound<i64> {
    let v = if rows.is_empty() || r.below(4) == 0 { random_id(r, 60).0 } else { rows[r.below(rows.len() as u64) as usize][0].0 };
    match r.below(3) {
        0 => Bound::Unbounded,
        1 => Bound::Included(v),
        _ => Bound::Excluded(v),
    }
}

/// Random fills / seeks / rebinds interleaved with events that invalidate the reader while a
/// cursor is positioned, none of which changes the logical quad set:
/// * a data-version bump (autocommit write to an unrelated table while a held statement
///   keeps main READ);
/// * entering a write transaction on an unrelated table (txn_state WRITE), later committed;
/// * a "churn" commit that inserts and deletes a block of quads in one transaction, so
///   pages split and merge and the b-tree layout under the pinned path changes;
/// * flushing the page cache.
///
/// Every fill must return exactly the model's next rows, whatever reader serves it.
#[test]
fn review_random_ops_with_invalidations_match_the_model() {
    for (seed, named) in [(301u64, true), (302, false)] {
        let quads = random_quads(seed, 6000, named);
        let db = file_db_with("PRAGMA page_size=1024;", &quads);
        db.conn.execute_batch("CREATE TABLE hold(x); INSERT INTO hold VALUES (1), (2); CREATE TABLE bump(x);").unwrap();
        let qset: BTreeSet<[TermId; 4]> = quads.iter().copied().collect();
        let churn: Vec<[TermId; 4]> = random_quads(seed + 1000, 1500, named).into_iter().filter(|q| !qset.contains(q)).collect();
        let mut held = db.conn.prepare("SELECT x FROM hold").unwrap();
        let mut held_rows = held.query([]).unwrap();
        assert!(held_rows.next().unwrap().is_some());
        assert_eq!(db.shared().db().txn_state(), ffi::SQLITE_TXN_READ);
        let mut r = Rng(seed * 13 + 5);
        let mut in_write = false;
        let mut events = 0;
        for order in IndexOrder::ALL {
            for prefix_len in 0..=4u8 {
                let pick_prefix = |r: &mut Rng| -> Vec<TermId> {
                    let q = quads[r.below(quads.len() as u64) as usize];
                    order.cols()[..prefix_len as usize].iter().map(|c| q[*c as usize]).collect()
                };
                for _ in 0..3 {
                    let mut spec = ScanSpec::new(order, &pick_prefix(&mut r));
                    if prefix_len < 4 && r.below(2) == 0 {
                        let all = expected_rows(&quads, &spec);
                        spec.lower = random_bound(&mut r, &all);
                        spec.upper = random_bound(&mut r, &all);
                    }
                    let mut rows = expected_rows(&quads, &spec);
                    let mut pos = 0usize;
                    let mut cur = db.store.cursor(&spec).unwrap();
                    let w = cur.width();
                    let mut log: Vec<String> = Vec::new();
                    for _ in 0..50 {
                        match r.below(12) {
                            0..=3 => {
                                let k = [1usize, 2, 5, 40, 300][r.below(5) as usize];
                                let mut buf = Vec::new();
                                let n = cur.fill(&mut buf, k).unwrap();
                                let end = (pos + k).min(rows.len());
                                log.push(format!("fill({k}) -> {n} [{:?}]", cur.counters().reader));
                                assert_eq!(buf, rows[pos..end].concat(), "{spec:?} after {log:?}");
                                pos = end;
                            }
                            4 | 5 if w > 0 => {
                                let klen = 1 + r.below(w as u64) as usize;
                                let key: Vec<TermId> = if rows.is_empty() || r.below(6) == 0 {
                                    (0..klen).map(|_| random_id(&mut r, 60)).collect()
                                } else {
                                    let delta = [r.below(4) as i64, r.below(64) as i64, -(r.below(50) as i64)][r.below(3) as usize];
                                    let j = (pos as i64 + delta).clamp(0, rows.len() as i64 - 1) as usize;
                                    rows[j][..klen].to_vec()
                                };
                                cur.seek(&key).unwrap();
                                log.push(format!("seek({key:?})"));
                                pos = rows.partition_point(|row| row[..klen] < key[..]);
                            }
                            6 | 7 => {
                                let p2 = pick_prefix(&mut r);
                                let (lo, hi) = if prefix_len < 4 && r.below(2) == 0 {
                                    let base = expected_rows(&quads, &ScanSpec::new(order, &p2));
                                    (random_bound(&mut r, &base), random_bound(&mut r, &base))
                                } else {
                                    (Bound::Unbounded, Bound::Unbounded)
                                };
                                cur.rebind(&p2, lo, hi).unwrap();
                                spec = ScanSpec::new(order, &p2).with_range(lo, hi);
                                rows = expected_rows(&quads, &spec);
                                pos = 0;
                                log.push(format!("rebind({spec:?})"));
                            }
                            8 => {
                                events += 1;
                                db.conn.execute("INSERT INTO bump VALUES (1)", []).unwrap();
                                log.push("bump".into());
                            }
                            9 => {
                                events += 1;
                                if in_write {
                                    db.conn.execute_batch("COMMIT").unwrap();
                                    log.push("commit".into());
                                } else {
                                    db.conn.execute_batch("BEGIN; INSERT INTO bump VALUES (2);").unwrap();
                                    assert_eq!(db.shared().db().txn_state(), ffi::SQLITE_TXN_WRITE);
                                    log.push("begin write".into());
                                }
                                in_write = !in_write;
                            }
                            10 if !in_write => {
                                events += 1;
                                let from = r.below(churn.len() as u64 / 2) as usize;
                                let block = &churn[from..from + churn.len() / 2];
                                let ws = db.store.write_scope(0).unwrap();
                                assert_eq!(db.store.insert(&mut block.to_vec()).unwrap(), block.len() as u64);
                                assert_eq!(db.store.remove(&mut block.to_vec()).unwrap(), block.len() as u64);
                                ws.commit().unwrap();
                                log.push("churn".into());
                            }
                            _ => {
                                db.shared().clear_page_cache().unwrap();
                            }
                        }
                    }
                }
            }
        }
        if in_write {
            db.conn.execute_batch("COMMIT").unwrap();
        }
        assert!(events > 100);
        let c = db.shared().btree_counters();
        assert!(c.fallbacks > 20 && c.resumes > 10, "{c:?}");
        drop(held_rows);
    }
}

/// Keys at every integer serial-type boundary, i64::MIN / i64::MAX in every column (so the
/// fallback's resume key needs a carry into an earlier column), read through every order:
/// full scans, seeks to every key, and a fallback after every row.
#[test]
fn review_extreme_keys_scan_seek_and_resume() {
    let vals = [
        i64::MIN,
        i64::MIN + 1,
        -(1 << 47) - 1,
        -(1 << 47),
        i32::MIN as i64 - 1,
        -8_388_609,
        -32769,
        -129,
        -1,
        0,
        1,
        2,
        127,
        128,
        32768,
        8_388_608,
        i32::MAX as i64 + 1,
        (1 << 47) - 1,
        1 << 47,
        i64::MAX - 7,
        i64::MAX - 1,
        i64::MAX,
        // tags 5 and 8 (rdf_gpox), negative too
        5,
        8,
        -11,
        i64::MIN + 5,
    ];
    let mut r = Rng(77);
    let mut quads = BTreeSet::new();
    while quads.len() < 1500 {
        let mut q = [0i64; 4];
        for v in &mut q {
            *v = vals[r.below(vals.len() as u64) as usize];
        }
        // Named-only orders need g <> 0, rdf_gpox needs tag(o) in {5, 8}; keep a mix.
        quads.insert(q.map(TermId));
    }
    let quads: Vec<[TermId; 4]> = quads.into_iter().collect();
    // Raw SQL inserts: these ids are not all storable through QuadStoreWriter.
    let db = file_db_with("PRAGMA page_size=512;", &[]);
    {
        let tx = db.conn.unchecked_transaction().unwrap();
        let mut st = tx.prepare("INSERT INTO rdf_quad VALUES (?1, ?2, ?3, ?4)").unwrap();
        for q in &quads {
            st.execute([q[0].0, q[1].0, q[2].0, q[3].0]).unwrap();
        }
        drop(st);
        tx.commit().unwrap();
    }
    db.conn.execute_batch("CREATE TABLE hold(x); INSERT INTO hold VALUES (1); CREATE TABLE bump(x);").unwrap();
    {
        let _read = db.store.read_scope(0).unwrap();
        db.shared().validate_epoch().unwrap();
        // (cursor_conformance itself overflows on i64::MAX ids, so the checks are explicit.)
        for order in IndexOrder::ALL {
            let spec = ScanSpec::new(order, &[]);
            let rows = expected_rows(&quads, &spec);
            let mut cur = db.store.cursor(&spec).unwrap();
            assert_eq!(cur.counters().reader, ReaderKind::Btree);
            assert_eq!(drain(cur.as_mut(), 7), rows, "{order:?}");
            for (j, row) in rows.iter().enumerate().step_by(7) {
                for klen in 1..=4 {
                    cur.seek(&row[..klen]).unwrap();
                    let pos = rows.partition_point(|x| x[..klen] < row[..klen]);
                    let mut buf = Vec::new();
                    cur.fill(&mut buf, 3).unwrap();
                    assert_eq!(buf, rows[pos..(pos + 3).min(rows.len())].concat(), "{order:?} seek {j}/{klen}");
                }
            }
        }
        assert_eq!(db.shared().btree_counters().fallbacks, 0, "{}", last_fallback(&db));
    }
    // A fallback after every returned row (prefix scans through every order).
    let mut held = db.conn.prepare("SELECT x FROM hold").unwrap();
    let mut held_rows = held.query([]).unwrap();
    assert!(held_rows.next().unwrap().is_some());
    for order in IndexOrder::ALL {
        for prefix_len in 0..=3usize {
            let q = quads[r.below(quads.len() as u64) as usize];
            let prefix: Vec<TermId> = order.cols()[..prefix_len].iter().map(|c| q[*c as usize]).collect();
            let spec = ScanSpec::new(order, &prefix);
            let rows = expected_rows(&quads, &spec);
            for cut in 0..=rows.len().min(40) {
                let mut cur = db.store.cursor(&spec).unwrap();
                assert_eq!(cur.counters().reader, ReaderKind::Btree);
                let mut buf = Vec::new();
                cur.fill(&mut buf, cut).unwrap();
                db.conn.execute("INSERT INTO bump VALUES (1)", []).unwrap();
                let mut got: Vec<Vec<TermId>> = buf.chunks(cur.width().max(1)).map(<[TermId]>::to_vec).collect();
                if cur.width() == 0 {
                    got = vec![vec![]; buf.len()];
                }
                got.extend(drain(cur.as_mut(), 5));
                assert_eq!(got, rows, "{spec:?} fallback after {cut} rows");
            }
        }
    }
    drop(held_rows);
}

/// Root pages that move while the schema cookie changes: an auto_vacuum=FULL database where
/// dropping an earlier table relocates the last root pages, and incremental vacuum that moves
/// pages and truncates the file. Every order must still read the right b-tree.
#[test]
fn review_root_page_moves_and_file_truncation() {
    for mode in ["FULL", "INCREMENTAL"] {
        let path = TempPath::new("rbt-av");
        let conn = Connection::open(&path.0).unwrap();
        conn.execute_batch(&format!(
            "PRAGMA page_size=1024; PRAGMA auto_vacuum={mode};
             CREATE TABLE pad(x INTEGER PRIMARY KEY, y); CREATE INDEX pad_y ON pad(y);
             WITH RECURSIVE c(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM c WHERE i<3000)
             INSERT INTO pad SELECT i, randomblob(40) FROM c;"
        ))
        .unwrap();
        let quads = random_quads(401, 5000, true);
        let db = Db::with(conn, Some(path));
        db.insert(&quads);
        let check_all = |db: &Db, quads: &[[TermId; 4]], label: &str| {
            let _read = db.store.read_scope(0).unwrap();
            db.shared().validate_epoch().unwrap();
            for order in IndexOrder::ALL {
                let spec = ScanSpec::new(order, &[]);
                let mut cur = db.store.cursor(&spec).unwrap();
                assert_eq!(cur.counters().reader, ReaderKind::Btree, "{label} {order:?}: {}", last_fallback(db));
                assert_eq!(drain(cur.as_mut(), 333), expected_rows(quads, &spec), "{label} {order:?}");
            }
            cursor_conformance(&db.store, quads, 9, 1);
        };
        check_all(&db, &quads, "before");
        let roots_before: Vec<i64> = {
            let mut st = db.conn.prepare("SELECT rootpage FROM sqlite_schema WHERE tbl_name='rdf_quad' ORDER BY name").unwrap();
            st.query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect()
        };
        db.conn.execute_batch("DROP TABLE pad;").unwrap();
        // Shrink the quad set too, then reclaim the free pages.
        let gone: Vec<_> = quads.iter().copied().step_by(2).collect();
        db.remove(&gone);
        if mode == "INCREMENTAL" {
            db.conn.execute_batch("PRAGMA incremental_vacuum;").unwrap();
        }
        let roots_after: Vec<i64> = {
            let mut st = db.conn.prepare("SELECT rootpage FROM sqlite_schema WHERE tbl_name='rdf_quad' ORDER BY name").unwrap();
            st.query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect()
        };
        assert_ne!(roots_before, roots_after, "{mode}: root pages moved");
        let gone: BTreeSet<_> = gone.into_iter().collect();
        let now: Vec<_> = quads.iter().copied().filter(|q| !gone.contains(q)).collect();
        check_all(&db, &now, mode);
    }
}

/// Per-call overhead of the reader (release only): `fill(1)` and `seek` + `fill(1)` on a
/// pinned leaf, against the same calls on a `SqlCursor`. The spec budgets the reader at
/// 25-35 ns per row; a per-call cost far above one SQL step makes small fills (merge-join
/// refills, galloping seeks) slower than through SQL.
#[test]
#[ignore = "performance: run with --release -- --ignored --nocapture"]
fn review_perf_per_call_overhead() {
    if cfg!(debug_assertions) {
        eprintln!("skipped: performance numbers are only meaningful with --release");
        return;
    }
    let _serial = super::perf_lock();
    let p = TermId::from_dict(2000, Tag::Iri);
    let quads: Vec<[TermId; 4]> =
        (0..60_000i64).map(|s| [TermId::DEFAULT_GRAPH, TermId::from_dict(10_000 + s, Tag::Iri), p, TermId::from_int(s).unwrap()]).collect();
    let db = file_db_with("PRAGMA page_size=8192; PRAGMA cache_size=-262144;", &quads);
    let _read = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    let spec = ScanSpec::new(IndexOrder::Gpso, &[TermId::DEFAULT_GRAPH, p]);
    const CALLS: usize = 50_000;
    let mut report = Vec::new();
    for mode in [crate::ReaderMode::Sql, crate::ReaderMode::Btree] {
        db.shared().set_reader(mode);
        let mut best_fill = f64::MAX;
        let mut best_seek = f64::MAX;
        for _ in 0..5 {
            let mut cur = db.store.cursor(&spec).unwrap();
            let mut buf = Vec::with_capacity(16);
            let t = Instant::now();
            for _ in 0..CALLS {
                buf.clear();
                assert_eq!(cur.fill(&mut buf, 1).unwrap(), 1);
            }
            best_fill = best_fill.min(t.elapsed().as_nanos() as f64 / CALLS as f64);
            // Forward seeks by 2 rows + fill(1): the merge-join / galloping shape.
            let mut cur = db.store.cursor(&spec).unwrap();
            let t = Instant::now();
            for i in 0..CALLS / 2 {
                cur.seek(&[quads[2 * i][1]]).unwrap();
                buf.clear();
                assert_eq!(cur.fill(&mut buf, 1).unwrap(), 1);
            }
            best_seek = best_seek.min(t.elapsed().as_nanos() as f64 / (CALLS / 2) as f64);
        }
        println!("{mode:?}: fill(1) {best_fill:.0} ns/call, seek+fill(1) {best_seek:.0} ns/call");
        report.push((best_fill, best_seek));
    }
    let (sql, bt) = (report[0], report[1]);
    println!("fill(1) B-tree/SQL = {:.2}, seek+fill(1) B-tree/SQL = {:.2}", bt.0 / sql.0, bt.1 / sql.1);
}

/// A cursor positioned by the reader survives a page-size change (VACUUM with a new
/// page_size and reserved bytes) and a switch to WAL between executions: the next fill
/// falls back at the exact key, a later rebind resumes on the new geometry (or stays on SQL
/// once the database is in WAL mode), and the kill switch keeps a fallen-back cursor on SQL.
#[test]
fn review_geometry_and_journal_mode_changes_under_a_positioned_cursor() {
    let quads = random_quads(501, 5000, true);
    let db = file_db_with("PRAGMA page_size=1024;", &quads);
    let spec = ScanSpec::new(IndexOrder::Gpos, &[TermId::DEFAULT_GRAPH]);
    let rows = expected_rows(&quads, &spec);
    assert!(rows.len() > 400);
    let g = quads.iter().find(|q| q[0] != TermId::DEFAULT_GRAPH).unwrap()[0];
    let spec_g = ScanSpec::new(IndexOrder::Gpos, &[g]);
    let rows_g = expected_rows(&quads, &spec_g);
    let mut cur;
    let mut got;
    {
        let _read = db.store.read_scope(0).unwrap();
        db.shared().validate_epoch().unwrap();
        cur = db.store.cursor(&spec).unwrap();
        assert_eq!(cur.counters().reader, ReaderKind::Btree);
        let mut buf = Vec::new();
        assert_eq!(cur.fill(&mut buf, 345).unwrap(), 345);
        got = buf.chunks(3).map(<[TermId]>::to_vec).collect::<Vec<_>>();
    }
    // New page size and reserved bytes: every page moves.
    db.conn.execute_batch("PRAGMA page_size=4096;").unwrap();
    let mut n = 16i32;
    // SAFETY: valid connection; SQLITE_FCNTL_RESERVE_BYTES reads and writes one int.
    let rc = unsafe {
        ffi::sqlite3_file_control(db.conn.handle(), c"main".as_ptr(), ffi::SQLITE_FCNTL_RESERVE_BYTES, (&raw mut n).cast())
    };
    assert_eq!(rc, ffi::SQLITE_OK);
    db.conn.execute_batch("VACUUM;").unwrap();
    assert_eq!(db.query_i64("PRAGMA page_size"), 4096);
    {
        let _read = db.store.read_scope(0).unwrap();
        db.shared().validate_epoch().unwrap();
        got.extend(drain(cur.as_mut(), 100));
        assert_eq!(got, rows, "the scan continues at the exact key after the VACUUM");
        assert_eq!(cur.counters().reader, ReaderKind::SqlCursor);
        cur.rebind(&[g], Bound::Unbounded, Bound::Unbounded).unwrap();
        assert_eq!(cur.counters().reader, ReaderKind::Btree, "{}", last_fallback(&db));
        assert_eq!(drain(cur.as_mut(), 77), rows_g);
    }
    // Switch to WAL between executions.
    db.conn.query_row("PRAGMA journal_mode=WAL", [], |_| Ok(())).unwrap();
    {
        let _read = db.store.read_scope(0).unwrap();
        db.shared().validate_epoch().unwrap();
        cur.rebind(&[g], Bound::Unbounded, Bound::Unbounded).unwrap();
        assert_eq!(cur.counters().reader, ReaderKind::SqlCursor);
        assert_eq!(drain(cur.as_mut(), 77), rows_g);
        assert!(last_fallback(&db).contains("WAL"), "{}", last_fallback(&db));
    }
    db.conn.query_row("PRAGMA journal_mode=DELETE", [], |_| Ok(())).unwrap();
    {
        let _read = db.store.read_scope(0).unwrap();
        db.shared().validate_epoch().unwrap();
        db.shared().set_reader(crate::ReaderMode::Sql);
        cur.rebind(&[g], Bound::Unbounded, Bound::Unbounded).unwrap();
        assert_eq!(cur.counters().reader, ReaderKind::SqlCursor, "kill switch");
        db.shared().set_reader(crate::ReaderMode::Auto);
        cur.rebind(&[g], Bound::Unbounded, Bound::Unbounded).unwrap();
        assert_eq!(cur.counters().reader, ReaderKind::Btree, "back in rollback mode");
        assert_eq!(drain(cur.as_mut(), 77), rows_g);
    }
}

/// Targeted corruption of index b-tree pages (page-header fields, cell pointers, payload
/// sizes, record headers and bodies, child pointers, swapped cells), followed by scans,
/// seeks and rebinds through every order. With overflow checks on (debug), any arithmetic or
/// slicing slip in the page decoder panics; every outcome must be rows or an error.
#[test]
fn review_targeted_page_corruption_never_panics() {
    for page_size in [512usize, 65536] {
        let n = if page_size == 512 { 3000 } else { 9000 };
        let quads = random_quads(601, n, true);
        let path = TempPath::new("rbt-corrupt-src");
        {
            let conn = Connection::open(&path.0).unwrap();
            conn.execute_batch(&format!("PRAGMA page_size={page_size};")).unwrap();
            let db = Db::with(conn, None);
            db.insert(&quads);
        }
        let pristine = std::fs::read(&path.0).unwrap();
        let pages = pristine.len() / page_size;
        let index_pages: Vec<usize> =
            (1..pages).filter(|p| matches!(pristine[p * page_size], 0x02 | 0x0A)).collect();
        assert!(index_pages.len() > 5);
        let mut r = Rng(601 + page_size as u64);
        let rounds = if page_size == 512 { 150 } else { 40 };
        let (mut btree_cursors, mut fallbacks, mut opened) = (0, 0, 0);
        for _ in 0..rounds {
            let mut bytes = pristine.clone();
            for _ in 0..1 + r.below(3) {
                let p = index_pages[r.below(index_pages.len() as u64) as usize];
                let base = p * page_size;
                let page = &mut bytes[base..base + page_size];
                let leaf = page[0] == 0x0A;
                let cells = u16::from_be_bytes([page[3], page[4]]) as usize;
                let ptrs = if leaf { 8 } else { 12 };
                let cell_off = |page: &[u8], i: usize| u16::from_be_bytes([page[ptrs + 2 * i], page[ptrs + 2 * i + 1]]) as usize;
                match r.below(9) {
                    0 => page[3..5].copy_from_slice(&(r.below(65536) as u16).to_be_bytes()),
                    1 if cells > 0 => {
                        let i = r.below(cells as u64) as usize;
                        let v = [0u16, 1, 7, (page_size - 1) as u16, (page_size - 2) as u16, r.below(65536) as u16][r.below(6) as usize];
                        page[ptrs + 2 * i..ptrs + 2 * i + 2].copy_from_slice(&v.to_be_bytes());
                    }
                    2 if cells > 1 => {
                        // Swap two cells: structurally valid, keys out of order.
                        let (i, j) = (r.below(cells as u64) as usize, r.below(cells as u64) as usize);
                        let (a, b) = (ptrs + 2 * i, ptrs + 2 * j);
                        let (x, y) = ([page[a], page[a + 1]], [page[b], page[b + 1]]);
                        page[a..a + 2].copy_from_slice(&y);
                        page[b..b + 2].copy_from_slice(&x);
                    }
                    3 if cells > 0 => {
                        // Payload size: huge, 9-byte varint, or zero.
                        let at = cell_off(page, r.below(cells as u64) as usize) + if leaf { 0 } else { 4 };
                        if at + 9 < page_size {
                            match r.below(3) {
                                0 => page[at] = 0,
                                1 => page[at..at + 9].fill(0xff),
                                _ => page[at] = 0x7f,
                            }
                        }
                    }
                    4 if cells > 0 => {
                        // Record header: size and serial types.
                        let at = cell_off(page, r.below(cells as u64) as usize) + if leaf { 1 } else { 5 };
                        if at + 6 < page_size {
                            let k = r.below(6) as usize;
                            page[at + k] = [0u8, 7, 10, 11, 12, 13, 0x80, 0xff, 6, 9][r.below(10) as usize];
                        }
                    }
                    5 if !leaf => {
                        // Child pointers: 0, 1 (the schema page), itself, beyond the file.
                        let v = [0u32, 1, p as u32 + 1, pages as u32 + 5, u32::MAX][r.below(5) as usize];
                        if r.below(2) == 0 || cells == 0 {
                            page[8..12].copy_from_slice(&v.to_be_bytes());
                        } else {
                            let at = cell_off(page, r.below(cells as u64) as usize);
                            if at + 4 <= page_size {
                                page[at..at + 4].copy_from_slice(&v.to_be_bytes());
                            }
                        }
                    }
                    6 => page[0] = [0x02u8, 0x0A, 0x05, 0x0D, 0x00][r.below(5) as usize],
                    7 => {
                        let at = r.below(page_size as u64) as usize;
                        page[at] ^= 1 << r.below(8);
                    }
                    _ => {
                        let at = r.below(page_size as u64 - 16) as usize;
                        for b in &mut page[at..at + 16] {
                            *b = r.below(256) as u8;
                        }
                    }
                }
            }
            let copy = TempPath::new("rbt-corrupt");
            std::fs::write(&copy.0, &bytes).unwrap();
            let conn = Connection::open(&copy.0).unwrap();
            let store = super::store_for(&conn);
            let Ok(read) = store.read_scope(0) else { continue };
            if store.shared().validate_epoch().is_err() {
                continue;
            }
            for order in IndexOrder::ALL {
                for prefix_len in [0usize, 1, 2] {
                    let q = quads[r.below(quads.len() as u64) as usize];
                    let prefix: Vec<TermId> = order.cols()[..prefix_len].iter().map(|c| q[*c as usize]).collect();
                    let Ok(mut cur) = store.cursor(&ScanSpec::new(order, &prefix)) else { continue };
                    let mut buf = Vec::new();
                    for step in 0..400 {
                        let res = match step % 5 {
                            0 | 1 | 2 => cur.fill(&mut buf, [1usize, 3, 50][step % 3]).map(|_| ()),
                            3 if cur.width() > 0 => {
                                let k = quads[r.below(quads.len() as u64) as usize];
                                let key: Vec<TermId> = order.cols()[prefix_len..].iter().map(|c| k[*c as usize]).collect();
                                cur.seek(&key[..1 + r.below(key.len() as u64) as usize])
                            }
                            _ => {
                                let k = quads[r.below(quads.len() as u64) as usize];
                                let p2: Vec<TermId> = order.cols()[..prefix_len].iter().map(|c| k[*c as usize]).collect();
                                cur.rebind(&p2, Bound::Unbounded, Bound::Unbounded)
                            }
                        };
                        buf.clear();
                        if res.is_err() && r.below(4) == 0 {
                            break;
                        }
                    }
                }
            }
            let c = store.shared().btree_counters();
            btree_cursors += c.cursors;
            fallbacks += c.fallbacks;
            opened += 1;
            drop(read);
            drop(store);
            drop(conn);
        }
        assert!(opened > rounds / 2 && btree_cursors > 0 && fallbacks > 0, "{opened} {btree_cursors} {fallbacks}");
        println!("page size {page_size}: {opened} databases, {btree_cursors} B-tree cursors, {fallbacks} fallbacks");
    }
}

/// What connection A observed in [`shared_cache_spill_scenario`].
#[derive(Debug, PartialEq)]
struct SharedCacheOutcome {
    /// The reader of A's cursor when it opened.
    reader: ReaderKind,
    /// B's first INSERT into `rdf_quad` was refused with SQLITE_LOCKED ("database table is
    /// locked"): A's read transaction holds a shared-cache table lock on `rdf_quad`.
    insert_locked: bool,
    /// A's whole scan, and whether it ended without an error.
    rows: Vec<Vec<TermId>>,
    scan_ok: bool,
}

/// Connections A and B share one cache. A scans `rdf_quad` (GSPO) inside a read scope; after
/// its first 10 rows, B opens a write transaction, inserts quads into the range still ahead of
/// A's scan, then writes enough elsewhere for the tiny (shared) page cache to spill its dirty
/// pages into the database file. A drains its cursor and B rolls back.
fn shared_cache_spill_scenario(mode: crate::ReaderMode) -> SharedCacheOutcome {
    use rusqlite::OpenFlags;
    let path = TempPath::new("rbt-shared");
    let uri = format!("file:{}?cache=shared", path.0.display());
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_URI;
    let a = Connection::open_with_flags(&uri, flags).unwrap();
    a.execute_batch("PRAGMA page_size=1024;").unwrap();
    let base = random_quads(701, 3000, false);
    let db_a = Db::with(a, None);
    db_a.insert(&base);
    db_a.conn.execute_batch("CREATE TABLE junk(x);").unwrap();
    db_a.shared().set_reader(mode);
    let b = Connection::open_with_flags(&uri, flags).unwrap();
    let spec = ScanSpec::new(IndexOrder::Gspo, &[]);
    let committed = expected_rows(&base, &spec);
    let read = db_a.store.read_scope(0).unwrap();
    db_a.shared().validate_epoch().unwrap();
    let mut cur = db_a.store.cursor(&spec).unwrap();
    let reader = cur.counters().reader;
    let mut buf = Vec::new();
    assert_eq!(cur.fill(&mut buf, 10).unwrap(), 10);
    b.execute_batch("PRAGMA cache_size=5; BEGIN;").unwrap();
    let mut insert_locked = false;
    {
        let mut st = b.prepare("INSERT INTO rdf_quad VALUES (?1, ?2, ?3, ?4)").unwrap();
        for q in committed.iter().skip(500).step_by(97) {
            // Same g, s, p as a committed quad; an object just above it (not a stored quad).
            match st.execute([q[0].0, q[1].0, q[2].0, q[3].0 + 16 * 7919]) {
                Ok(_) => {}
                Err(e) => {
                    assert_eq!(e.sqlite_error_code(), Some(rusqlite::ErrorCode::DatabaseLocked), "{e}");
                    assert!(e.to_string().contains("database table is locked"), "{e}");
                    insert_locked = true;
                    break;
                }
            }
        }
    }
    b.execute_batch(
        "WITH RECURSIVE c(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM c WHERE i<4000)
         INSERT INTO junk SELECT randomblob(300) FROM c;",
    )
    .unwrap();
    let mut rows: Vec<Vec<TermId>> = buf.chunks(4).map(<[TermId]>::to_vec).collect();
    let mut tail = Vec::new();
    let result = loop {
        match cur.fill(&mut tail, 256) {
            Ok(0) => break Ok(()),
            Ok(_) => {}
            Err(e) => break Err(e),
        }
    };
    rows.extend(tail.chunks(4).map(<[TermId]>::to_vec));
    b.execute_batch("ROLLBACK").unwrap();
    let committed_set: BTreeSet<Vec<TermId>> = committed.iter().cloned().collect();
    let dirty = rows.iter().filter(|r| !committed_set.contains(*r)).count();
    println!(
        "{mode:?}: reader {reader:?} -> {:?}; insert locked: {insert_locked}; {} rows ({} committed), {dirty} \
         uncommitted; result {result:?}; last fallback: {}",
        cur.counters().reader,
        rows.len(),
        committed.len(),
        last_fallback(&db_a)
    );
    assert_eq!(dirty, 0, "{mode:?}: the scan returned {dirty} rows of another connection's uncommitted transaction");
    drop(cur);
    drop(read);
    SharedCacheOutcome { reader, insert_locked, rows, scan_ok: result.is_ok() }
}

/// Shared-cache connections share one pager, so a read transaction of this connection takes
/// no file lock of its own. Another connection of the same cache can then spill its
/// uncommitted write transaction into the database file. With the default `reader = auto`
/// the B-tree reader served this connection's cursors, read the spilled pages and returned
/// rows that were never committed (a dirty read). The scan must never contain uncommitted
/// rows, and the default reader must behave exactly like the SQL path.
///
/// Amended after the review (see the WP15 fix notes): the original test unwrapped B's INSERT
/// into `rdf_quad`, but under SQLite's own rules that INSERT is refused while A's open read
/// transaction holds its shared-cache table lock on `rdf_quad` ("database table is locked"),
/// which is exactly what happens with `reader = sql`. The scenario now records that outcome
/// and requires `auto` to match `sql`.
#[test]
fn review_shared_cache_writer_spill_is_never_read() {
    let sql = shared_cache_spill_scenario(crate::ReaderMode::Sql);
    let auto = shared_cache_spill_scenario(crate::ReaderMode::Auto);
    assert_eq!(auto.reader, ReaderKind::SqlCursor, "a shared-cache connection must not use the B-tree reader");
    assert!(sql.insert_locked && sql.scan_ok, "SQLite's own behaviour: {sql:?}");
    assert_eq!(auto, sql, "reader = auto must behave like reader = sql");
}

// ---------------------------------------------------------------------- second review round

/// Sets the reserved bytes per page of a still empty database (`SQLITE_FCNTL_RESERVE_BYTES`).
fn reserve_bytes(conn: &Connection, n: i32) {
    let mut n = n;
    // SAFETY: valid connection; "main" is NUL-terminated; SQLITE_FCNTL_RESERVE_BYTES reads and
    // writes one int through the argument.
    let rc = unsafe {
        ffi::sqlite3_file_control(conn.handle(), c"main".as_ptr(), ffi::SQLITE_FCNTL_RESERVE_BYTES, (&raw mut n).cast())
    };
    assert_eq!(rc, ffi::SQLITE_OK);
}

/// A random bound for a free column: an existing value (exact or off by one), a random id,
/// or an extreme value.
fn review_bound(r: &mut Rng, rows: &[Vec<TermId>]) -> Bound<i64> {
    let v = match r.below(6) {
        0 => [i64::MIN, i64::MIN + 1, -1, 0, 1, i64::MAX - 1, i64::MAX][r.below(7) as usize],
        1 => random_id(r, 60).0,
        _ if !rows.is_empty() => rows[r.below(rows.len() as u64) as usize][0].0.saturating_add(r.below(3) as i64 - 1),
        _ => random_id(r, 60).0,
    };
    match r.below(3) {
        0 => Bound::Unbounded,
        1 => Bound::Included(v),
        _ => Bound::Excluded(v),
    }
}

/// The B-tree cursor against a `SqlCursor` over the same spec, operation by operation
/// (fills of every size, seeks of every key length to existing, perturbed, random and extreme
/// keys, rebinds to random prefixes and bounds, page-cache flushes), on the smallest usable
/// page SQLite allows (512-byte pages with 32 reserved bytes: 480 usable bytes), so the
/// b-trees are deep and interior cells are frequent. Both cursors must return exactly the
/// same rows, and the reader must never fall back.
#[test]
fn review_differential_against_sql_cursor_on_minimum_usable_pages() {
    let path = TempPath::new("rbt-diff");
    let conn = Connection::open(&path.0).unwrap();
    conn.execute_batch("PRAGMA page_size=512;").unwrap();
    reserve_bytes(&conn, 32);
    let db = Db::with(conn, Some(path));
    let quads = random_quads(811, 20_000, true);
    db.insert(&quads);
    assert_eq!(std::fs::read(&db.path.as_ref().unwrap().0).unwrap()[20], 32);
    let _read = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    let shared = db.shared().clone();
    let mut r = Rng(811);
    let extremes = [i64::MIN, i64::MIN + 1, -1, 0, 1, i64::MAX - 1, i64::MAX];
    let mut ops = 0u64;
    for order in IndexOrder::ALL {
        for prefix_len in 0..=4u8 {
            let pick_prefix = |r: &mut Rng| -> Vec<TermId> {
                let q = quads[r.below(quads.len() as u64) as usize];
                let mut p: Vec<TermId> = order.cols()[..prefix_len as usize].iter().map(|c| q[*c as usize]).collect();
                if !p.is_empty() && r.below(6) == 0 {
                    let i = r.below(p.len() as u64) as usize;
                    p[i] = TermId(p[i].0 + r.below(3) as i64 - 1);
                }
                p
            };
            for _ in 0..4 {
                let mut spec = ScanSpec::new(order, &pick_prefix(&mut r));
                let all = expected_rows(&quads, &ScanSpec::new(order, &spec.prefix[..prefix_len as usize]));
                if prefix_len < 4 && r.below(2) == 0 {
                    spec = spec.with_range(review_bound(&mut r, &all), review_bound(&mut r, &all));
                }
                let mut bt = db.store.cursor(&spec).unwrap();
                assert_eq!(bt.counters().reader, ReaderKind::Btree);
                let mut sql = crate::cursor::SqlCursor::open(shared.pool.clone(), &spec).unwrap();
                let w = bt.width();
                let mut rows = expected_rows(&quads, &spec);
                let mut log: Vec<String> = Vec::new();
                for _ in 0..120 {
                    ops += 1;
                    match r.below(10) {
                        0..=3 => {
                            let k = [0usize, 1, 2, 3, 7, 64, 500][r.below(7) as usize];
                            let (mut a, mut b) = (Vec::new(), Vec::new());
                            let na = bt.fill(&mut a, k).unwrap();
                            let nb = sql.fill(&mut b, k).unwrap();
                            log.push(format!("fill({k}) -> {na}/{nb}"));
                            assert_eq!((na, &a), (nb, &b), "{spec:?} after {log:?}");
                        }
                        4..=6 if w > 0 => {
                            let klen = 1 + r.below(w as u64) as usize;
                            let key: Vec<TermId> = match r.below(5) {
                                0 => (0..klen).map(|_| TermId(extremes[r.below(7) as usize])).collect(),
                                1 => (0..klen).map(|_| random_id(&mut r, 60)).collect(),
                                _ if !rows.is_empty() => {
                                    let mut key = rows[r.below(rows.len() as u64) as usize][..klen].to_vec();
                                    if r.below(3) == 0 {
                                        let last = key.len() - 1;
                                        key[last] = TermId(key[last].0.saturating_add(r.below(3) as i64 - 1));
                                    }
                                    key
                                }
                                _ => (0..klen).map(|_| random_id(&mut r, 60)).collect(),
                            };
                            log.push(format!("seek({key:?})"));
                            bt.seek(&key).unwrap();
                            sql.seek(&key).unwrap();
                        }
                        7 | 8 => {
                            let p2 = pick_prefix(&mut r);
                            let base = expected_rows(&quads, &ScanSpec::new(order, &p2));
                            let (lo, hi) = if prefix_len < 4 && r.below(2) == 0 {
                                (review_bound(&mut r, &base), review_bound(&mut r, &base))
                            } else {
                                (Bound::Unbounded, Bound::Unbounded)
                            };
                            bt.rebind(&p2, lo, hi).unwrap();
                            sql.rebind(&p2, lo, hi).unwrap();
                            spec = ScanSpec::new(order, &p2).with_range(lo, hi);
                            rows = expected_rows(&quads, &spec);
                            log.push(format!("rebind({spec:?})"));
                        }
                        _ => {
                            shared.clear_page_cache().unwrap();
                            log.push("flush".into());
                        }
                    }
                }
                assert_eq!(bt.counters().reader, ReaderKind::Btree, "{spec:?}");
            }
        }
    }
    assert!(ops > 20_000);
    let c = shared.btree_counters();
    assert_eq!(c.fallbacks, 0, "{}", last_fallback(&db));
}

fn status_value(db: &Db, key: &str) -> String {
    db.shared().status().into_iter().find(|(k, _)| k == key).map(|(_, v)| v).unwrap_or_default()
}

/// The page cache belongs to one (file, data version) snapshot and must be invalidated when
/// the data version changes. When the new version is rejected (here: the database switched
/// to WAL, so every later cursor is a SQL cursor), the pages of the dead snapshot must not
/// stay cached: nothing can ever read them again, yet they keep up to `page_cache_mb`
/// (128 MB by default) per connection alive for as long as the database stays in WAL mode.
#[test]
fn review_rejected_snapshot_releases_the_stale_page_cache() {
    let quads = random_quads(821, 6000, true);
    let db = file_db_with("PRAGMA page_size=1024;", &quads);
    {
        let _read = db.store.read_scope(0).unwrap();
        db.shared().validate_epoch().unwrap();
        for order in IndexOrder::ALL {
            let spec = ScanSpec::new(order, &[]);
            let mut cur = db.store.cursor(&spec).unwrap();
            assert_eq!(cur.counters().reader, ReaderKind::Btree);
            assert_eq!(drain(cur.as_mut(), 500), expected_rows(&quads, &spec));
        }
    }
    let cached: usize = status_value(&db, "btree_cache_bytes").parse().unwrap();
    assert!(cached > 100_000, "the scans filled the page cache ({cached} bytes)");
    db.conn.query_row("PRAGMA journal_mode=WAL", [], |_| Ok(())).unwrap();
    {
        let _read = db.store.read_scope(0).unwrap();
        db.shared().validate_epoch().unwrap();
        let spec = ScanSpec::new(IndexOrder::Gspo, &[]);
        let mut cur = db.store.cursor(&spec).unwrap();
        assert_eq!(cur.counters().reader, ReaderKind::SqlCursor);
        assert_eq!(drain(cur.as_mut(), 500), expected_rows(&quads, &spec));
        assert!(last_fallback(&db).contains("WAL"), "{}", last_fallback(&db));
    }
    let stale: usize = status_value(&db, "btree_cache_bytes").parse().unwrap();
    assert_eq!(stale, 0, "{stale} bytes of pages of a superseded data version are still cached");
}

/// Rollback-journal variants the reader accepts (header bytes 18/19 are 1 for all of them):
/// TRUNCATE, PERSIST, MEMORY and OFF journals, and exclusive locking mode (the connection
/// keeps its lock between transactions, and with PERSIST commits may skip work). Commits by
/// this connection between executions must invalidate the page cache in every mode.
#[test]
fn review_rollback_journal_variants_and_exclusive_locking() {
    for pragmas in [
        "PRAGMA journal_mode=TRUNCATE;",
        "PRAGMA journal_mode=PERSIST;",
        "PRAGMA journal_mode=MEMORY;",
        "PRAGMA journal_mode=OFF;",
        "PRAGMA locking_mode=EXCLUSIVE; PRAGMA journal_mode=PERSIST;",
        "PRAGMA locking_mode=EXCLUSIVE; PRAGMA journal_mode=DELETE;",
        "PRAGMA mmap_size=268435456;",
    ] {
        let path = TempPath::new("rbt-jm");
        let conn = Connection::open(&path.0).unwrap();
        conn.execute_batch("PRAGMA page_size=1024;").unwrap();
        for p in pragmas.split(';').filter(|p| !p.trim().is_empty()) {
            conn.query_row(p, [], |_| Ok(())).unwrap();
        }
        let db = Db::with(conn, Some(path));
        let mut model: BTreeSet<[TermId; 4]> = BTreeSet::new();
        let mut r = Rng(831);
        for round in 0..4u64 {
            let add = random_quads(840 + round, 2500, true);
            db.insert(&add);
            model.extend(add);
            let gone: Vec<_> = model.iter().copied().filter(|_| r.below(5) == 0).collect();
            db.remove(&gone);
            for g in &gone {
                model.remove(g);
            }
            let now: Vec<_> = model.iter().copied().collect();
            let _read = db.store.read_scope(0).unwrap();
            db.shared().validate_epoch().unwrap();
            for order in IndexOrder::ALL {
                let spec = ScanSpec::new(order, &[]);
                let mut cur = db.store.cursor(&spec).unwrap();
                assert_eq!(cur.counters().reader, ReaderKind::Btree, "{pragmas}: {}", last_fallback(&db));
                assert_eq!(drain(cur.as_mut(), 300), expected_rows(&now, &spec), "{pragmas} round {round} {order:?}");
            }
        }
        assert_eq!(db.shared().btree_counters().fallbacks, 0, "{pragmas}: {}", last_fallback(&db));
    }
}

/// The rollback-journal counterpart of the shared-cache test: connection B (private cache)
/// holds a RESERVED lock with a large pending transaction, inserting quads into the range
/// still ahead of A's scan and enough other rows for its tiny page cache to want to spill.
/// A spill needs EXCLUSIVE, which A's read transaction (SHARED) blocks, so the file only
/// ever holds committed pages: A's B-tree scan returns exactly the committed rows without
/// falling back, and B can commit only after A's read transaction ended.
#[test]
fn review_other_connection_pending_write_is_never_read_in_rollback_mode() {
    let base = random_quads(851, 4000, false);
    let db = file_db_with("PRAGMA page_size=1024;", &base);
    db.conn.execute_batch("CREATE TABLE junk(x);").unwrap();
    let other = Connection::open(&db.path.as_ref().unwrap().0).unwrap();
    // No busy wait: every spill attempt (and B's early COMMIT) fails at once.
    other.execute_batch("PRAGMA busy_timeout=0; PRAGMA cache_size=5; BEGIN IMMEDIATE;").unwrap();
    let spec = ScanSpec::new(IndexOrder::Gspo, &[]);
    let committed = expected_rows(&base, &spec);
    let read = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    let mut cur = db.store.cursor(&spec).unwrap();
    assert_eq!(cur.counters().reader, ReaderKind::Btree);
    let mut buf = Vec::new();
    assert_eq!(cur.fill(&mut buf, 10).unwrap(), 10);
    {
        let mut st = other.prepare("INSERT INTO rdf_quad VALUES (?1, ?2, ?3, ?4)").unwrap();
        for q in committed.iter().skip(500).step_by(37) {
            st.execute([q[0].0, q[1].0, q[2].0, q[3].0 + 16 * 7919]).unwrap();
        }
    }
    other
        .execute_batch(
            "WITH RECURSIVE c(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM c WHERE i<4000)
             INSERT INTO junk SELECT randomblob(300) FROM c;",
        )
        .unwrap();
    let mut rows: Vec<Vec<TermId>> = buf.chunks(4).map(<[TermId]>::to_vec).collect();
    rows.extend(drain(cur.as_mut(), 256));
    assert_eq!(rows, committed, "only committed rows");
    assert_eq!(cur.counters().reader, ReaderKind::Btree);
    assert_eq!(db.shared().btree_counters().fallbacks, 0, "{}", last_fallback(&db));
    assert!(other.execute_batch("COMMIT").is_err(), "B cannot commit while A reads");
    drop(cur);
    read.close().unwrap();
    other.execute_batch("COMMIT").unwrap();
    let _read = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    let mut cur = db.store.cursor(&spec).unwrap();
    assert_eq!(cur.counters().reader, ReaderKind::Btree);
    let now = drain(cur.as_mut(), 256);
    assert!(now.len() > committed.len(), "B's commit is visible to the next execution");
}
