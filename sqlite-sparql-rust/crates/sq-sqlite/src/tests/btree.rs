//! Phase-2 direct B-tree reader (§13): the conformance suite against `BtreeCursor` on file
//! databases (page sizes 4K–64K, reserved bytes, named graphs, deletes + re-inserts, VACUUM),
//! random operation sequences against a brute-force model, every fallback path (WAL, own
//! write transaction, interleaved writes, data-version changes, unsupported records, corrupt
//! pages), the kill switch, and cost / capability reporting.
use std::collections::BTreeSet;
use std::ops::Bound;

use rusqlite::{Connection, ffi};
use sq_core::stats::{BTREE_COSTS, SQL_CURSOR_COSTS};
use sq_core::store::{IndexOrder, QuadCursor, QuadStore, QuadStoreWriter, ReaderKind, ScanSpec};
use sq_core::testing::{Rng, cursor_conformance, expected_rows, random_id, random_quads};
use sq_core::{Error, Tag, TermId};

use super::{Db, TempPath};
use crate::btree::BtreeCursor;
use crate::{ReaderMode, SqliteStore};

/// A file database with `page_size` (and `reserved` bytes at the end of every page) holding
/// `quads`.
fn file_db(page_size: u32, reserved: u8, quads: &[[TermId; 4]]) -> Db {
    let path = TempPath::new("bt");
    let conn = Connection::open(&path.0).unwrap();
    conn.execute_batch(&format!("PRAGMA page_size={page_size};")).unwrap();
    if reserved > 0 {
        let mut n = i32::from(reserved);
        // SAFETY: valid connection; "main" is NUL-terminated; SQLITE_FCNTL_RESERVE_BYTES reads
        // and writes one int through the argument. The database is still empty, so the
        // reserve applies when the first page is written.
        let rc = unsafe {
            ffi::sqlite3_file_control(conn.handle(), c"main".as_ptr(), ffi::SQLITE_FCNTL_RESERVE_BYTES, (&raw mut n).cast())
        };
        assert_eq!(rc, ffi::SQLITE_OK);
    }
    let db = Db::with(conn, Some(path));
    db.insert(quads);
    db
}

/// Byte `at` of the database header, read from the file.
fn header_byte(db: &Db, at: usize) -> u8 {
    std::fs::read(&db.path.as_ref().unwrap().0).unwrap()[at]
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

fn all_quads(store: &SqliteStore) -> Vec<[TermId; 4]> {
    let mut cur = store.cursor(&ScanSpec::new(IndexOrder::Gspo, &[])).unwrap();
    drain(cur.as_mut(), 1024).into_iter().map(|r| [r[0], r[1], r[2], r[3]]).collect()
}

/// The conformance suite inside a read scope, asserting that every cursor was a B-tree
/// cursor and that none fell back.
fn conformance_btree(db: &Db, quads: &[[TermId; 4]], seed: u64, iterations: usize) {
    let _read = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    assert!(db.shared().btree_active(), "reader inactive: {}", last_fallback(db));
    let before = db.shared().btree_counters();
    cursor_conformance(&db.store, quads, seed, iterations);
    let after = db.shared().btree_counters();
    // One cursor per order x prefix length x iteration.
    assert_eq!(after.cursors - before.cursors, 45 * iterations as u64, "every cursor is a B-tree cursor");
    assert_eq!(after.fallbacks, before.fallbacks, "unexpected fallback: {}", last_fallback(db));
}

/// Depth of the b-tree of `order` (root to leaf), through a full scan cursor.
fn depth(db: &Db, order: IndexOrder) -> usize {
    let _read = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    let shared = db.shared();
    let cur = BtreeCursor::open(&shared.btree, &shared.pool, &ScanSpec::new(order, &[])).unwrap().unwrap();
    cur.depth()
}

#[test]
fn conformance_random_seeds_with_and_without_named_graphs() {
    for (seed, named) in [(1u64, false), (3, true), (5, true), (7, false)] {
        let q = random_quads(seed, 3000, named);
        let db = file_db(4096, 0, &q);
        conformance_btree(&db, &q, seed, 4);
    }
}

/// 4K–64K pages; enough quads for interior pages at every size (64K pages hold ~3000 cells).
#[test]
fn conformance_page_sizes() {
    for (page_size, n) in [(4096u32, 6000usize), (8192, 8000), (16384, 12_000), (65536, 25_000)] {
        let q = random_quads(100 + u64::from(page_size), n, true);
        let db = file_db(page_size, 0, &q);
        let stored = u32::from(u16::from_be_bytes([header_byte(&db, 16), header_byte(&db, 17)]));
        assert_eq!(stored, if page_size == 65536 { 1 } else { page_size }, "65536 is stored as 1");
        for order in [IndexOrder::Gspo, IndexOrder::Gpos] {
            assert!(depth(&db, order) >= 2, "page size {page_size}: {order:?} has no interior page");
        }
        conformance_btree(&db, &q, u64::from(page_size), 2);
    }
}

#[test]
fn conformance_with_reserved_bytes_per_page() {
    for (page_size, reserved) in [(1024u32, 32u8), (4096, 200)] {
        let q = random_quads(77, 5000, true);
        let db = file_db(page_size, reserved, &q);
        assert_eq!(header_byte(&db, 20), reserved, "the reserve applies to the new file");
        assert!(depth(&db, IndexOrder::Gspo) >= 2);
        conformance_btree(&db, &q, 77, 2);
    }
}

#[test]
fn conformance_after_deletes_reinserts_and_vacuum() {
    let q = random_quads(41, 12_000, true);
    let db = file_db(1024, 0, &q);
    let del: Vec<_> = q.iter().copied().step_by(3).collect();
    assert_eq!(db.remove(&del), del.len() as u64);
    let re: Vec<_> = del.iter().copied().step_by(2).collect();
    assert_eq!(db.insert(&re), re.len() as u64);
    let del_set: BTreeSet<_> = del.iter().copied().collect();
    let re_set: BTreeSet<_> = re.iter().copied().collect();
    let want: Vec<_> = q.iter().copied().filter(|x| !del_set.contains(x) || re_set.contains(x)).collect();
    conformance_btree(&db, &want, 41, 2);
    // VACUUM rebuilds every b-tree (new root pages, schema cookie + 1).
    let roots_before = db.shared().btree_counters().root_loads;
    db.conn.execute_batch("VACUUM").unwrap();
    conformance_btree(&db, &want, 42, 2);
    assert!(db.shared().btree_counters().root_loads > roots_before, "VACUUM changes the schema cookie");
    // Page splits and merges on a store that keeps changing between executions.
    let mut r = Rng(99);
    let mut model: BTreeSet<_> = want.into_iter().collect();
    for round in 0..3 {
        let extra = random_quads(1000 + round, 3000, true);
        db.insert(&extra);
        model.extend(extra);
        let gone: Vec<_> = model.iter().copied().filter(|_| r.below(4) == 0).collect();
        db.remove(&gone);
        for x in &gone {
            model.remove(x);
        }
        let now: Vec<_> = model.iter().copied().collect();
        conformance_btree(&db, &now, 50 + round, 1);
    }
}

#[test]
fn the_whole_table_through_every_order() {
    let q = random_quads(8, 20_000, true);
    let db = file_db(1024, 0, &q);
    let _read = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    for order in IndexOrder::ALL {
        let spec = ScanSpec::new(order, &[]);
        for max in [1, 7, 1024] {
            let mut cur = db.store.cursor(&spec).unwrap();
            assert_eq!(cur.counters().reader, ReaderKind::Btree);
            assert_eq!(drain(cur.as_mut(), max), expected_rows(&q, &spec), "{order:?} max {max}");
            assert_eq!(cur.counters().reader, ReaderKind::Btree);
        }
    }
    assert_eq!(db.shared().btree_counters().fallbacks, 0, "{}", last_fallback(&db));
}

fn random_bound(r: &mut Rng, rows: &[Vec<TermId>]) -> Bound<i64> {
    let v = if rows.is_empty() || r.below(4) == 0 { random_id(r, 60).0 } else { rows[r.below(rows.len() as u64) as usize][0].0 };
    match r.below(3) {
        0 => Bound::Unbounded,
        1 => Bound::Included(v),
        _ => Bound::Excluded(v),
    }
}

/// Random interleavings of fills, forward seeks (short ones within a leaf and long ones
/// across leaves and interior cells), backward seeks and rebinds to random prefixes (mostly
/// increasing, like sorted probes), checked after every operation against a brute-force model.
#[test]
fn random_operations_match_the_model() {
    for (seed, named) in [(201u64, true), (202, false)] {
        let quads = random_quads(seed, 15_000, named);
        let db = file_db(1024, 0, &quads);
        assert!(depth(&db, IndexOrder::Gspo) >= 3, "finger searches must climb several levels");
        let _read = db.store.read_scope(0).unwrap();
        db.shared().validate_epoch().unwrap();
        let mut r = Rng(seed * 7 + 1);
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
                    assert_eq!(cur.counters().reader, ReaderKind::Btree);
                    let w = cur.width();
                    let mut log: Vec<String> = Vec::new();
                    for _ in 0..60 {
                        match r.below(7) {
                            0..=2 => {
                                let k = [1usize, 2, 5, 40, 300][r.below(5) as usize];
                                let mut buf = Vec::new();
                                let n = cur.fill(&mut buf, k).unwrap();
                                let end = (pos + k).min(rows.len());
                                log.push(format!("fill({k}) -> {n}"));
                                assert_eq!(buf, rows[pos..end].concat(), "{spec:?} after {log:?}");
                                pos = end;
                            }
                            3 | 4 if w > 0 => {
                                let klen = 1 + r.below(w as u64) as usize;
                                let key: Vec<TermId> = if rows.is_empty() || r.below(6) == 0 {
                                    (0..klen).map(|_| random_id(&mut r, 60)).collect()
                                } else {
                                    let delta = [r.below(4) as i64, r.below(64) as i64, r.below(2000) as i64, -(r.below(50) as i64)][r.below(4) as usize];
                                    let j = (pos as i64 + delta).clamp(0, rows.len() as i64 - 1) as usize;
                                    let mut key = rows[j][..klen].to_vec();
                                    if r.below(3) == 0 {
                                        let last = key.len() - 1;
                                        key[last] = TermId(key[last].0 + if r.below(2) == 0 { 1 } else { -1 });
                                    }
                                    key
                                };
                                cur.seek(&key).unwrap();
                                log.push(format!("seek({key:?})"));
                                pos = rows.partition_point(|row| row[..klen] < key[..]);
                            }
                            _ => {
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
                        }
                    }
                    assert_eq!(cur.counters().reader, ReaderKind::Btree, "{spec:?}");
                }
            }
        }
        assert_eq!(db.shared().btree_counters().fallbacks, 0, "{}", last_fallback(&db));
    }
}

/// Sorted probes (rebind to increasing prefixes, the index-nested-loop shape) and galloping
/// seeks across a large b-tree, row for row against the model.
#[test]
fn sorted_probes_and_long_seeks() {
    let p = TermId::from_dict(1100, Tag::Iri);
    let mut quads = Vec::new();
    for s in 0..3000i64 {
        for o in 0..(1 + s % 4) {
            quads.push([TermId::DEFAULT_GRAPH, TermId::from_dict(5000 + s, Tag::Iri), p, TermId::from_int(o * 7 - 3).unwrap()]);
        }
    }
    let db = file_db(512, 0, &quads);
    let _read = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    // Probes (g, s, p) through GSPO with increasing subjects, including absent ones.
    let mut cur = db.store.cursor(&ScanSpec::new(IndexOrder::Gspo, &[TermId::DEFAULT_GRAPH, quads[0][1], p])).unwrap();
    for s in (0..3100i64).step_by(3) {
        let subject = TermId::from_dict(5000 + s, Tag::Iri);
        cur.rebind(&[TermId::DEFAULT_GRAPH, subject, p], Bound::Unbounded, Bound::Unbounded).unwrap();
        let got = drain(cur.as_mut(), 2);
        let want: Vec<Vec<TermId>> = quads.iter().filter(|q| q[1] == subject).map(|q| vec![q[3]]).collect();
        assert_eq!(got, want, "probe {s}");
    }
    // A merge-join-like walk: alternate short fills and seeks of growing distance.
    let rows: Vec<Vec<TermId>> = quads.iter().map(|q| vec![q[1], q[3]]).collect();
    let mut cur = db.store.cursor(&ScanSpec::new(IndexOrder::Gpso, &[TermId::DEFAULT_GRAPH, p])).unwrap();
    let mut pos = 0;
    let mut jump = 1;
    while pos < rows.len() {
        let key = &rows[pos][..1];
        cur.seek(key).unwrap();
        pos = rows.partition_point(|r| r[..1] < *key);
        let mut buf = Vec::new();
        let n = cur.fill(&mut buf, 2).unwrap();
        let end = (pos + 2).min(rows.len());
        assert_eq!(buf, rows[pos..end].concat());
        assert_eq!(n, end - pos);
        pos = end + jump;
        jump = (jump * 3) % 997 + 1;
    }
    assert_eq!(cur.counters().reader, ReaderKind::Btree);
    assert_eq!(db.shared().btree_counters().fallbacks, 0, "{}", last_fallback(&db));
}

#[test]
fn width_zero_and_empty_ranges() {
    let q = random_quads(9, 2000, true);
    let db = file_db(1024, 0, &q);
    let _read = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    let mut cur = db.store.cursor(&ScanSpec::new(IndexOrder::Gspo, &q[17])).unwrap();
    assert_eq!(cur.counters().reader, ReaderKind::Btree);
    let mut buf = Vec::new();
    assert_eq!(cur.fill(&mut buf, 10).unwrap(), 1);
    assert_eq!(cur.fill(&mut buf, 10).unwrap(), 0);
    assert!(buf.is_empty());
    assert!(cur.seek(&[TermId(1)]).is_err(), "key wider than the cursor");
    let mut absent = q[17];
    absent[3] = TermId(absent[3].0 + 16 * 1000);
    cur.rebind(&absent, Bound::Unbounded, Bound::Unbounded).unwrap();
    assert_eq!(cur.fill(&mut buf, 10).unwrap(), 0);
    cur.rebind(&q[18], Bound::Unbounded, Bound::Unbounded).unwrap();
    assert_eq!(cur.fill(&mut buf, 10).unwrap(), 1);
    for (lo, hi) in [
        (Bound::Excluded(i64::MAX), Bound::Unbounded),
        (Bound::Unbounded, Bound::Excluded(i64::MIN)),
        (Bound::Included(10), Bound::Included(9)),
    ] {
        let spec = ScanSpec::new(IndexOrder::Gpos, &[]).with_range(lo, hi);
        let mut cur = db.store.cursor(&spec).unwrap();
        assert_eq!(cur.fill(&mut buf, 100).unwrap(), 0);
        cur.seek(&[TermId(5), TermId(5)]).unwrap();
        assert_eq!(cur.fill(&mut buf, 100).unwrap(), 0);
        // A rebind out of the empty range works again.
        cur.rebind(&[], Bound::Unbounded, Bound::Unbounded).unwrap();
        assert_eq!(drain(cur.as_mut(), 1024), expected_rows(&q, &ScanSpec::new(IndexOrder::Gpos, &[])));
    }
}

// ---------------------------------------------------------------------- preconditions

#[test]
fn wal_databases_use_sql_cursors() {
    let q = random_quads(13, 2000, true);
    let path = TempPath::new("wal");
    let conn = Connection::open(&path.0).unwrap();
    conn.query_row("PRAGMA journal_mode=WAL", [], |_| Ok(())).unwrap();
    let db = Db::with(conn, Some(path));
    db.insert(&q);
    assert_eq!((header_byte(&db, 18), header_byte(&db, 19)), (2, 2));
    let _read = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    assert!(!db.shared().btree_active());
    assert_eq!(db.store.caps().reader, ReaderKind::SqlCursor);
    let spec = ScanSpec::new(IndexOrder::Gpso, &[]);
    let mut cur = db.store.cursor(&spec).unwrap();
    assert_eq!(cur.counters().reader, ReaderKind::SqlCursor);
    assert_eq!(drain(cur.as_mut(), 100), expected_rows(&q, &spec));
    assert!(last_fallback(&db).contains("WAL"), "{}", last_fallback(&db));
    assert_eq!(db.shared().btree_counters().cursors, 0);
}

#[test]
fn memory_databases_and_missing_read_transactions_use_sql_cursors() {
    let q = random_quads(14, 500, false);
    let mem = Db::mem();
    mem.insert(&q);
    let _read = mem.store.read_scope(0).unwrap();
    mem.shared().validate_epoch().unwrap();
    assert!(!mem.shared().btree_active());
    assert_eq!(mem.store.cursor(&ScanSpec::new(IndexOrder::Gspo, &[])).unwrap().counters().reader, ReaderKind::SqlCursor);
    // A file database outside any read transaction (autocommit, no statement running).
    let db = file_db(4096, 0, &q);
    assert_eq!(db.shared().db().txn_state(), ffi::SQLITE_TXN_NONE);
    let spec = ScanSpec::new(IndexOrder::Gspo, &[]);
    let mut cur = db.store.cursor(&spec).unwrap();
    assert_eq!(cur.counters().reader, ReaderKind::SqlCursor);
    assert_eq!(drain(cur.as_mut(), 64), expected_rows(&q, &spec));
}

#[test]
fn own_write_transaction_uses_sql_cursors_and_sees_its_writes() {
    let q = random_quads(15, 2000, true);
    let db = file_db(4096, 0, &q);
    db.conn.execute_batch("BEGIN").unwrap();
    let extra = random_quads(16, 300, true);
    {
        let ws = db.store.write_scope(0).unwrap();
        db.store.insert(&mut extra.clone()).unwrap();
        ws.commit().unwrap(); // RELEASE inside BEGIN: still uncommitted
    }
    assert_eq!(db.shared().db().txn_state(), ffi::SQLITE_TXN_WRITE);
    assert!(!db.shared().btree_active());
    let mut all: Vec<_> = q.iter().chain(&extra).copied().collect();
    all.sort();
    all.dedup();
    for order in IndexOrder::ALL {
        let spec = ScanSpec::new(order, &[]);
        let mut cur = db.store.cursor(&spec).unwrap();
        assert_eq!(cur.counters().reader, ReaderKind::SqlCursor);
        assert_eq!(drain(cur.as_mut(), 333), expected_rows(&all, &spec), "{order:?}");
    }
    db.conn.execute_batch("ROLLBACK").unwrap();
}

/// A scan that falls back mid-way (here: the connection starts a write transaction on an
/// unrelated table between two fills) returns exactly the remaining rows, whatever the
/// position: inside a leaf, at an interior cell, right after a seek, at the end.
#[test]
fn fallback_mid_scan_returns_exactly_the_remaining_rows() {
    let q = random_quads(17, 8000, true);
    let db = file_db(1024, 0, &q);
    let mut r = Rng(17);
    let mut graph = 900_000i64;
    let mut expected_fallbacks = 0;
    for order in IndexOrder::ALL {
        for prefix_len in 0..=3usize {
            for case in 0..6 {
                db.conn.execute_batch("BEGIN; SELECT count(*) FROM rdf_quad;").unwrap();
                assert_eq!(db.shared().db().txn_state(), ffi::SQLITE_TXN_READ);
                let pick = q[r.below(q.len() as u64) as usize];
                let prefix: Vec<TermId> = order.cols()[..prefix_len].iter().map(|c| pick[*c as usize]).collect();
                let spec = ScanSpec::new(order, &prefix);
                let rows = expected_rows(&q, &spec);
                let mut cur = db.store.cursor(&spec).unwrap();
                assert_eq!(cur.counters().reader, ReaderKind::Btree);
                let mut got = Vec::new();
                let first = r.below(rows.len() as u64 + 2) as usize;
                let mut pos = 0;
                if case % 2 == 1 && !rows.is_empty() {
                    // Seek first, so that the fallback resumes at a seek target.
                    let j = r.below(rows.len() as u64) as usize;
                    cur.seek(&rows[j][..1]).unwrap();
                    pos = rows.partition_point(|row| row[..1] < rows[j][..1]);
                }
                let n = cur.fill(&mut got, first).unwrap();
                let w = cur.width();
                assert_eq!(n, first.min(rows.len() - pos));
                // Any write makes txn_state WRITE: the next fill must fall back.
                graph += 16;
                db.conn.execute(&format!("INSERT INTO rdf_graph(g) VALUES ({graph})"), []).unwrap();
                let rest = drain(cur.as_mut(), [1, 3, 1024][case % 3]);
                if n < rows.len() - pos || first == n {
                    // The scan had not ended (or not noticed its end) before the write.
                    assert_eq!(cur.counters().reader, ReaderKind::SqlCursor, "{spec:?}");
                    expected_fallbacks += 1;
                }
                let mut all: Vec<Vec<TermId>> = got.chunks(w.max(1)).map(<[TermId]>::to_vec).collect();
                if w == 0 {
                    all = vec![vec![]; n];
                }
                all.extend(rest);
                assert_eq!(all, rows[pos..].to_vec(), "{spec:?}, {first} rows before the fallback");
                drop(cur);
                db.conn.execute_batch("ROLLBACK").unwrap();
            }
        }
    }
    assert_eq!(db.shared().btree_counters().fallbacks, expected_fallbacks);
    assert!(expected_fallbacks > 100);
}

/// An interleaved write by the same connection inside a user transaction: the rows returned
/// before the write come from the reader, the rest from the SQL cursor, which sees the
/// transaction's own changes after the last returned key.
#[test]
fn interleaved_write_inside_a_user_transaction() {
    let q = random_quads(18, 6000, false);
    let db = file_db(1024, 0, &q);
    db.conn.execute_batch("BEGIN; SELECT count(*) FROM rdf_quad;").unwrap();
    let spec = ScanSpec::new(IndexOrder::Gspo, &[]);
    let mut cur = db.store.cursor(&spec).unwrap();
    let mut first = Vec::new();
    assert_eq!(cur.fill(&mut first, 1000).unwrap(), 1000);
    let last: [TermId; 4] = first[first.len() - 4..].try_into().unwrap();
    // Delete every 5th quad and insert new ones on both sides of the scan position.
    let gone: Vec<_> = q.iter().copied().step_by(5).collect();
    let added: Vec<[TermId; 4]> = random_quads(19, 2000, false).into_iter().filter(|x| !q.contains(x)).collect();
    {
        let ws = db.store.write_scope(0).unwrap();
        db.store.remove(&mut gone.clone()).unwrap();
        db.store.insert(&mut added.clone()).unwrap();
        ws.commit().unwrap();
    }
    let rest = drain(cur.as_mut(), 500);
    assert_eq!(cur.counters().reader, ReaderKind::SqlCursor);
    let gone: BTreeSet<_> = gone.into_iter().collect();
    let mut now: Vec<[TermId; 4]> = q.iter().copied().filter(|x| !gone.contains(x)).chain(added).collect();
    now.sort();
    now.dedup();
    let want: Vec<Vec<TermId>> = now.iter().filter(|x| **x > last).map(|x| x.to_vec()).collect();
    assert_eq!(rest, want);
    let before: Vec<Vec<TermId>> = q[..1000].iter().map(|x| x.to_vec()).collect();
    assert_eq!(first.chunks(4).map(<[TermId]>::to_vec).collect::<Vec<_>>(), before);
    db.conn.execute_batch("COMMIT").unwrap();
}

/// The data version changes under a held read transaction (this connection commits while
/// another of its statements keeps main open for reading): the next fill falls back, and a
/// later rebind returns to the reader on the new snapshot.
#[test]
fn data_version_change_under_a_read_transaction() {
    let q = random_quads(20, 4000, true);
    let db = file_db(1024, 0, &q);
    let mut held = db.conn.prepare("SELECT g FROM rdf_quad").unwrap();
    let mut rows = held.query([]).unwrap();
    assert!(rows.next().unwrap().is_some());
    assert_eq!(db.shared().db().txn_state(), ffi::SQLITE_TXN_READ);
    let spec = ScanSpec::new(IndexOrder::Gpos, &[]);
    let mut cur = db.store.cursor(&spec).unwrap();
    assert_eq!(cur.counters().reader, ReaderKind::Btree);
    let mut first = Vec::new();
    cur.fill(&mut first, 700).unwrap();
    // An autocommit write commits even though `held` is still reading; main stays READ.
    let extra = random_quads(21, 500, true);
    db.insert(&extra);
    assert_eq!(db.shared().db().txn_state(), ffi::SQLITE_TXN_READ);
    let rest = drain(cur.as_mut(), 100);
    assert_eq!(cur.counters().reader, ReaderKind::SqlCursor);
    assert!(last_fallback(&db).contains("data version"), "{}", last_fallback(&db));
    let mut now: Vec<_> = q.iter().chain(&extra).copied().collect();
    now.sort();
    now.dedup();
    let all_now = expected_rows(&now, &spec);
    let last = first[first.len() - 4..].to_vec();
    let want: Vec<_> = all_now.iter().filter(|r| **r > last).cloned().collect();
    assert_eq!(rest, want);
    // The rebind re-validates: back on the reader, with a fresh page cache.
    let flushes = db.shared().btree_counters().cache_flushes;
    let g = now.iter().find(|x| x[0] != TermId::DEFAULT_GRAPH).unwrap()[0];
    let spec2 = ScanSpec::new(IndexOrder::Gpos, &[g]);
    let mut cur = db.store.cursor(&ScanSpec::new(IndexOrder::Gpos, &[TermId::DEFAULT_GRAPH])).unwrap();
    let mut buf = Vec::new();
    cur.fill(&mut buf, 3).unwrap();
    db.insert(&random_quads(22, 50, true));
    cur.fill(&mut buf, 3).unwrap();
    assert_eq!(cur.counters().reader, ReaderKind::SqlCursor);
    cur.rebind(&[g], Bound::Unbounded, Bound::Unbounded).unwrap();
    assert_eq!(cur.counters().reader, ReaderKind::Btree);
    let now = all_quads(&db.store);
    assert_eq!(drain(cur.as_mut(), 64), expected_rows(&now, &spec2));
    let c = db.shared().btree_counters();
    assert!(c.resumes >= 1 && c.cache_flushes > flushes);
    drop(rows);
}

/// Another connection commits between two executions (rollback journal, so it can only do so
/// while no read lock is held): the data version changes and the page cache is flushed.
#[test]
fn commits_between_executions_invalidate_the_page_cache() {
    let q = random_quads(23, 3000, true);
    let db = file_db(1024, 0, &q);
    let other = db.reopen();
    let spec = ScanSpec::new(IndexOrder::Gspo, &[]);
    let scan = |want: &[[TermId; 4]]| {
        let _read = db.store.read_scope(0).unwrap();
        db.shared().validate_epoch().unwrap();
        let mut cur = db.store.cursor(&spec).unwrap();
        assert_eq!(cur.counters().reader, ReaderKind::Btree);
        assert_eq!(drain(cur.as_mut(), 512), expected_rows(want, &spec));
    };
    scan(&q);
    let c0 = db.shared().btree_counters();
    scan(&q);
    let c1 = db.shared().btree_counters();
    assert_eq!(c1.pages_read, c0.pages_read, "second scan is served from the page cache");
    assert!(c1.cache_hits > c0.cache_hits);
    // Another connection changes the store.
    let extra = random_quads(24, 400, true);
    other.insert(&extra);
    let gone: Vec<_> = q.iter().copied().step_by(7).collect();
    other.remove(&gone);
    let gone: BTreeSet<_> = gone.into_iter().collect();
    let mut now: Vec<_> = q.iter().copied().chain(extra).filter(|x| !gone.contains(x)).collect();
    now.sort();
    now.dedup();
    scan(&now);
    let c2 = db.shared().btree_counters();
    assert!(c2.cache_flushes > c1.cache_flushes && c2.pages_read > c1.pages_read);
    // This connection's own commit, too.
    let mine = random_quads(25, 100, false);
    db.insert(&mine);
    now.extend(mine);
    now.sort();
    now.dedup();
    scan(&now);
    assert!(db.shared().btree_counters().cache_flushes > c2.cache_flushes);
}

/// A cursor that outlives its read transaction continues on SQL (autocommit statements).
#[test]
fn cursor_outliving_its_read_scope() {
    let q = random_quads(26, 3000, true);
    let db = file_db(1024, 0, &q);
    let spec = ScanSpec::new(IndexOrder::Psog, &[]);
    let rows = expected_rows(&q, &spec);
    let read = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    let mut cur = db.store.cursor(&spec).unwrap();
    let mut buf = Vec::new();
    let n = cur.fill(&mut buf, 100).unwrap();
    read.close().unwrap();
    let mut got: Vec<Vec<TermId>> = buf.chunks(4).map(<[TermId]>::to_vec).collect();
    assert_eq!(n, 100);
    got.extend(drain(cur.as_mut(), 64));
    assert_eq!(got, rows);
    assert_eq!(cur.counters().reader, ReaderKind::SqlCursor);
}

/// A fallback whose SQL cursor cannot be prepared (an interrupt is pending while another
/// statement runs) makes every later `fill` report the error, like a failed `SqlCursor`, never
/// a silent end of the scan, until a `rebind` positions the cursor again.
#[test]
fn failed_fallback_keeps_reporting_until_repositioned() {
    let q = random_quads(33, 3000, true);
    let db = file_db(1024, 0, &q);
    let mut held = db.conn.prepare("SELECT g FROM rdf_quad").unwrap();
    let mut rows = held.query([]).unwrap();
    assert!(rows.next().unwrap().is_some());
    // Bound operators (Gt + Le) whose SQL template this connection never prepared.
    let spec = ScanSpec::new(IndexOrder::Ospg, &[]).with_range(Bound::Excluded(i64::MIN), Bound::Included(i64::MAX - 1));
    let mut cur = db.store.cursor(&spec).unwrap();
    assert_eq!(cur.counters().reader, ReaderKind::Btree);
    let mut buf = Vec::new();
    assert_eq!(cur.fill(&mut buf, 10).unwrap(), 10);
    let extra = random_quads(34, 10, true);
    db.insert(&extra); // the data version changes; `held` keeps main READ
    db.shared().db().interrupt();
    assert!(matches!(cur.fill(&mut buf, 10), Err(Error::Interrupted)));
    assert!(matches!(cur.fill(&mut buf, 10), Err(Error::Interrupted)), "the failure persists");
    assert_eq!(cur.counters().reader, ReaderKind::Btree, "no SQL cursor could be opened");
    drop(rows);
    drop(held); // no statement runs any more: the interrupt is over
    cur.rebind(&[], spec.lower, spec.upper).unwrap();
    let mut now: Vec<_> = q.iter().chain(&extra).copied().collect();
    now.sort();
    now.dedup();
    assert_eq!(drain(cur.as_mut(), 100), expected_rows(&now, &spec));
}

// ---------------------------------------------------------------------- unsupported / corrupt

/// Plain-SQL writes can put non-integer values into rdf_quad. The reader falls back when it
/// meets one (and stays on SQL for that cursor); results equal the SQL cursor's.
#[test]
fn non_integer_values_fall_back_to_sql() {
    let q = random_quads(27, 3000, false);
    let db = file_db(1024, 0, &q);
    let s = q[1500][1].0;
    db.conn.execute_batch(&format!("INSERT INTO rdf_quad VALUES (0, {s}, 2.5, 7), (0, {s}, 'text', 7), (0, {s}, x'01', 7)")).unwrap();
    let _read = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    let shared = db.shared();
    for order in [IndexOrder::Gspo, IndexOrder::Gpso, IndexOrder::Gosp] {
        let spec = ScanSpec::new(order, &[]);
        let mut sql = crate::cursor::SqlCursor::open(shared.pool.clone(), &spec).unwrap();
        let want = drain(&mut sql, 1024);
        // The fallback may already happen while positioning at the start.
        let mut cur = db.store.cursor(&spec).unwrap();
        assert_eq!(drain(cur.as_mut(), 97), want, "{order:?}");
        assert_eq!(cur.counters().reader, ReaderKind::SqlCursor);
        // Structural fallbacks are sticky: a rebind stays on SQL.
        cur.rebind(&[], Bound::Unbounded, Bound::Unbounded).unwrap();
        assert_eq!(cur.counters().reader, ReaderKind::SqlCursor);
    }
    assert!(last_fallback(&db).starts_with("unsupported"), "{}", last_fallback(&db));
}

/// Index definitions that differ from §2.1 (other column order, DESC, another partial
/// predicate) are never read directly.
#[test]
fn unexpected_index_layouts_are_read_through_sql() {
    let q = random_quads(28, 2000, true);
    let db = file_db(4096, 0, &q);
    db.conn
        .execute_batch(
            "DROP INDEX rdf_gpso; CREATE INDEX rdf_gpso ON rdf_quad(g, p, s DESC, o);
             DROP INDEX rdf_spog; CREATE INDEX rdf_spog ON rdf_quad(s, p, o, g) WHERE g>0;
             DROP INDEX rdf_gpox; CREATE INDEX rdf_gpox ON rdf_quad(g, p, o, s) WHERE (o & 15) IN (5,8);",
        )
        .unwrap();
    let _read = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    for (order, btree) in [(IndexOrder::Gpso, false), (IndexOrder::Gpox, true), (IndexOrder::Gspo, true)] {
        let spec = ScanSpec::new(order, &[]);
        let mut cur = db.store.cursor(&spec).unwrap();
        let kind = if btree { ReaderKind::Btree } else { ReaderKind::SqlCursor };
        assert_eq!(cur.counters().reader, kind, "{order:?}");
        assert_eq!(drain(cur.as_mut(), 100), expected_rows(&q, &spec), "{order:?}");
    }
    // `g>0` does not imply the `g<>0` the SQL template states, so SQLite refuses the index;
    // the reader must not read it either.
    assert!(db.store.cursor(&ScanSpec::new(IndexOrder::Spog, &[])).is_err());
    assert!(last_fallback(&db).contains("layout"), "{}", last_fallback(&db));
}

/// Randomly damaged index pages: every scan either returns rows or reports an error (from
/// SQLite, after the fallback), and never panics.
#[test]
fn corrupt_pages_never_panic() {
    let q = random_quads(29, 6000, true);
    let path = TempPath::new("corrupt-src");
    {
        let conn = Connection::open(&path.0).unwrap();
        conn.execute_batch("PRAGMA page_size=1024;").unwrap();
        let db = Db::with(conn, None);
        db.insert(&q);
    }
    let pristine = std::fs::read(&path.0).unwrap();
    let pages = pristine.len() / 1024;
    let mut r = Rng(29);
    let mut fallbacks = 0;
    for round in 0..40 {
        let mut bytes = pristine.clone();
        // Damage a few pages (never page 1: SQLite would refuse to open the schema).
        for _ in 0..1 + round % 4 {
            let page = 1 + r.below(pages as u64 - 1) as usize;
            let at = page * 1024 + r.below(1024) as usize;
            match r.below(3) {
                0 => bytes[at] ^= 1 << r.below(8),
                1 => bytes[page * 1024..page * 1024 + 16].iter_mut().for_each(|b| *b = r.below(256) as u8),
                _ => bytes[at] = 0xff,
            }
        }
        let copy = TempPath::new("corrupt");
        std::fs::write(&copy.0, &bytes).unwrap();
        let conn = Connection::open(&copy.0).unwrap();
        let store = super::store_for(&conn);
        let Ok(read) = store.read_scope(0) else { continue };
        if store.shared().validate_epoch().is_err() {
            continue;
        }
        for order in IndexOrder::ALL {
            let Ok(mut cur) = store.cursor(&ScanSpec::new(order, &[])) else { continue };
            let mut buf = Vec::new();
            for _ in 0..100_000 {
                match cur.fill(&mut buf, 256) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => buf.clear(),
                }
            }
            let _ = cur.seek(&[TermId(12345)]);
            let _ = cur.fill(&mut buf, 10);
        }
        fallbacks += store.shared().btree_counters().fallbacks;
        drop(read);
        drop(store);
        drop(conn);
    }
    assert!(fallbacks > 0, "some damage must have been detected");
}

// ---------------------------------------------------------------------- configuration

#[test]
fn kill_switch_page_cache_budget_and_status() {
    let q = random_quads(30, 5000, true);
    let db = file_db(1024, 0, &q);
    assert_eq!(ReaderMode::parse(" BTree ").unwrap(), ReaderMode::Btree);
    assert_eq!(ReaderMode::parse("sql").unwrap().as_str(), "sql");
    assert!(ReaderMode::parse("fast").is_err());
    let shared = db.shared();
    assert_eq!((shared.reader(), shared.page_cache_mb()), (ReaderMode::Auto, crate::DEFAULT_PAGE_CACHE_MB));
    let _read = db.store.read_scope(0).unwrap();
    shared.validate_epoch().unwrap();
    let spec = ScanSpec::new(IndexOrder::Gspo, &[]);
    shared.set_reader(ReaderMode::Sql);
    assert!(!shared.btree_active());
    assert_eq!(db.store.cursor(&spec).unwrap().counters().reader, ReaderKind::SqlCursor);
    for mode in [ReaderMode::Btree, ReaderMode::Auto] {
        shared.set_reader(mode);
        assert!(shared.btree_active());
        let mut cur = db.store.cursor(&spec).unwrap();
        assert_eq!(cur.counters().reader, ReaderKind::Btree);
        assert_eq!(drain(cur.as_mut(), 1024), expected_rows(&q, &spec));
    }
    // A 0 MB budget caches nothing (each cursor still pins its own path); 1 MB stays bounded.
    for mb in [0u64, 1] {
        shared.set_page_cache_mb(mb).unwrap();
        shared.clear_page_cache().unwrap();
        for order in IndexOrder::ALL {
            let s = ScanSpec::new(order, &[]);
            let mut cur = db.store.cursor(&s).unwrap();
            assert_eq!(drain(cur.as_mut(), 300), expected_rows(&q, &s));
        }
        let status = shared.status();
        let bytes: usize = status.iter().find(|(k, _)| k == "btree_cache_bytes").unwrap().1.parse().unwrap();
        assert!(bytes <= (mb as usize) << 20, "{bytes} bytes cached with a {mb} MB budget");
    }
    let status = shared.status();
    for key in ["reader", "page_cache_mb", "btree_cursors", "btree_pages_read", "btree_fallbacks", "btree_last_fallback"] {
        assert!(status.iter().any(|(k, _)| k == key), "status lacks {key}");
    }
}

#[test]
fn costs_and_caps_follow_the_active_reader() {
    let q = random_quads(31, 500, false);
    let db = file_db(4096, 0, &q);
    assert_eq!(db.store.caps().reader, ReaderKind::SqlCursor, "no read transaction yet");
    assert_eq!(db.store.costs().scan_row, SQL_CURSOR_COSTS.scan_row);
    {
        let _read = db.store.read_scope(0).unwrap();
        db.shared().validate_epoch().unwrap();
        assert_eq!(db.store.caps().reader, ReaderKind::Btree);
        assert!(db.store.caps().sql);
        assert_eq!(db.store.costs().scan_row, BTREE_COSTS.scan_row);
        db.shared().set_reader(ReaderMode::Sql);
        assert_eq!(db.store.caps().reader, ReaderKind::SqlCursor);
        assert_eq!(db.store.costs().seek_sorted, SQL_CURSOR_COSTS.seek_sorted);
        db.shared().set_reader(ReaderMode::Auto);
    }
    let mem = Db::mem();
    let _read = mem.store.read_scope(0).unwrap();
    mem.shared().validate_epoch().unwrap();
    assert_eq!(mem.store.caps().reader, ReaderKind::SqlCursor);
}

/// Counts are not affected by the reader (they always run in the VDBE), and an absent
/// schema still yields empty cursors.
#[test]
fn counts_and_absent_schema() {
    let path = TempPath::new("absent");
    let conn = Connection::open(&path.0).unwrap();
    conn.execute_batch("CREATE TABLE t(x); INSERT INTO t VALUES (1);").unwrap();
    let store = super::store_for(&conn);
    let read = store.read_scope(0).unwrap();
    store.shared().validate_epoch().unwrap();
    let mut cur = store.cursor(&ScanSpec::new(IndexOrder::Gspo, &[])).unwrap();
    assert_eq!(drain(cur.as_mut(), 10), Vec::<Vec<TermId>>::new());
    assert!(!store.shared().btree_active());
    drop(read);
    drop(cur);
    drop(store);
    let q = random_quads(32, 1000, true);
    let db = file_db(4096, 0, &q);
    let _read = db.store.read_scope(0).unwrap();
    db.shared().validate_epoch().unwrap();
    for order in IndexOrder::ALL {
        let spec = ScanSpec::new(order, &[]);
        assert_eq!(db.store.count(&spec, None).unwrap() as usize, expected_rows(&q, &spec).len());
    }
}
