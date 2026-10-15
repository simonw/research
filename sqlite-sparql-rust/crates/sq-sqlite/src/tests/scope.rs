//! §8.6 scopes, the statement pool (armed / disarmed, limits, close hook), interrupts and
//! `sql_rows`.
use rusqlite::ffi;
use sq_core::store::{IndexOrder, QuadStore, QuadStoreWriter, ScanSpec};
use sq_core::testing::random_quads;
use sq_core::{Error, Tag, TermId};

use super::{iri, store_for, Db};

#[test]
fn read_scope_opens_a_savepoint_only_outside_transactions() {
    let db = Db::mem();
    let d = db.shared().db();
    assert_eq!(d.txn_state(), ffi::SQLITE_TXN_NONE);
    {
        let r = db.store.read_scope(0).unwrap();
        assert!(r.is_active());
        assert!(!d.autocommit());
        db.shared().validate_epoch().unwrap();
        assert_eq!(d.txn_state(), ffi::SQLITE_TXN_READ, "the epoch read starts the snapshot");
        // Nested scopes reuse the transaction.
        let inner = db.store.read_scope(0).unwrap();
        assert!(!inner.is_active());
        assert!(inner.depth() > r.depth() || !inner.is_active());
    }
    assert!(d.autocommit());
    assert_eq!(d.txn_state(), ffi::SQLITE_TXN_NONE);
    // A deferred BEGIN has not started a transaction on `main` yet (txn_state NONE): the
    // scope opens a savepoint, whose RELEASE does not end the user's transaction …
    db.conn.execute_batch("BEGIN").unwrap();
    assert!(db.store.read_scope(0).unwrap().is_active());
    assert!(!d.autocommit());
    // … once the user transaction has read, it is reused as is.
    db.query_i64("SELECT count(*) FROM rdf_quad");
    assert_eq!(d.txn_state(), ffi::SQLITE_TXN_READ);
    assert!(!db.store.read_scope(0).unwrap().is_active());
    db.conn.execute_batch("COMMIT").unwrap();
}

#[test]
fn write_scopes_commit_roll_back_and_nest() {
    let db = Db::mem();
    let q = random_quads(1, 40, true);
    let wg = || db.query_i64("SELECT value FROM rdf_meta WHERE key='write_gen'");
    let w0 = wg();
    // commit
    let ws = db.store.write_scope(0).unwrap();
    assert_eq!(db.shared().db().txn_state(), ffi::SQLITE_TXN_WRITE, "write lock taken up front");
    db.store.insert(&mut q[..10].to_vec()).unwrap();
    ws.commit().unwrap();
    assert_eq!(wg(), w0 + 1);
    assert_eq!(db.query_i64("SELECT count(*) FROM rdf_quad"), 10);
    // drop = rollback (including the write_gen bump)
    {
        let _ws = db.store.write_scope(0).unwrap();
        db.store.insert(&mut q[10..].to_vec()).unwrap();
    }
    assert_eq!(db.query_i64("SELECT count(*) FROM rdf_quad"), 10);
    assert_eq!(wg(), w0 + 1);
    // nested: inner rolled back, outer committed
    let outer = db.store.write_scope(0).unwrap();
    db.store.insert(&mut q[10..20].to_vec()).unwrap();
    {
        let inner = db.store.write_scope(0).unwrap();
        assert!(inner.depth() > outer.depth());
        db.store.insert(&mut q[20..].to_vec()).unwrap();
    }
    outer.commit().unwrap();
    assert_eq!(db.query_i64("SELECT count(*) FROM rdf_quad"), 20);
    // inside a user transaction that is rolled back afterwards
    db.conn.execute_batch("BEGIN").unwrap();
    let ws = db.store.write_scope(0).unwrap();
    db.store.insert(&mut q[20..].to_vec()).unwrap();
    ws.commit().unwrap();
    assert!(!db.shared().db().autocommit(), "RELEASE inside BEGIN does not commit");
    db.conn.execute_batch("ROLLBACK").unwrap();
    assert_eq!(db.query_i64("SELECT count(*) FROM rdf_quad"), 20);
    let mut graphs: Vec<TermId> = q[..20].iter().map(|x| x[0]).filter(|g| g.is_bound()).collect();
    graphs.sort();
    graphs.dedup();
    assert_eq!(db.store.named_graphs().unwrap(), graphs, "rolled-back registrations are gone");
}

#[test]
fn pool_reuses_statements_and_respects_limits() {
    let db = Db::mem();
    db.insert(&random_quads(2, 300, true));
    let spec = ScanSpec::new(IndexOrder::Gpso, &[TermId::DEFAULT_GRAPH]);
    let prepared = || db.shared().pool.counters().prepared;
    {
        let _c = db.store.cursor(&spec).unwrap();
    }
    let p0 = prepared();
    for _ in 0..50 {
        let mut c = db.store.cursor(&spec).unwrap();
        c.fill(&mut Vec::new(), 10).unwrap();
    }
    assert_eq!(prepared(), p0, "pooled statements are reused");
    // Nested scans of one shape hold separate leases; at most 8 stay idle per key.
    let idle_before = db.shared().pool.idle_statements();
    let cursors: Vec<_> = (0..12).map(|_| db.store.cursor(&spec).unwrap()).collect();
    assert_eq!(db.shared().pool.leased_statements(), 12);
    drop(cursors);
    assert_eq!(db.shared().pool.leased_statements(), 0);
    assert_eq!(db.shared().pool.idle_statements(), idle_before - 1 + crate::pool::MAX_IDLE_PER_KEY);
    // sql_rows texts: an LRU of 256.
    let idle_before = db.shared().pool.idle_statements();
    for i in 0..300 {
        let mut c = db.store.sql_rows(&format!("SELECT {i}"), &[], 1).unwrap();
        let mut out = Vec::new();
        c.fill(&mut out, 5).unwrap();
        assert_eq!(out, [TermId(i)]);
    }
    assert_eq!(db.shared().pool.idle_statements() - idle_before, crate::pool::SQL_LRU_CAPACITY);
}

#[test]
fn disarmed_pool_finalizes_and_lets_the_connection_close() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    let store = store_for(&conn);
    store.ensure_schema().unwrap();
    store.insert(&mut random_quads(3, 50, false)).unwrap();
    let mut c = store.cursor(&ScanSpec::new(IndexOrder::Gspo, &[])).unwrap();
    c.fill(&mut Vec::new(), 3).unwrap();
    drop(c);
    assert!(store.shared().pool.idle_statements() > 0);
    // Armed with idle statements: sqlite3_close refuses (SQLITE_BUSY) …
    let (conn, _) = conn.close().unwrap_err();
    // … the close hook disarms and finalizes; later leases finalize on drop.
    store.shared().disarm_and_finalize();
    assert!(!store.shared().is_armed());
    assert_eq!(store.shared().pool.idle_statements(), 0);
    let mut c = store.cursor(&ScanSpec::new(IndexOrder::Gspo, &[])).unwrap();
    assert!(c.fill(&mut Vec::new(), 100).unwrap() > 0);
    drop(c);
    store.intern(iri("http://ex/x").as_ref().into()).unwrap();
    assert_eq!(store.shared().pool.idle_statements(), 0);
    let status: std::collections::HashMap<String, String> = store.shared().status().into_iter().collect();
    assert_eq!(status["armed"], "0");
    assert_eq!(status["pool_statements"], "0");
    // The store (an Rc) is still alive, yet the connection closes cleanly.
    conn.close().map_err(|(_, e)| e).unwrap();
    drop(store);
}

#[test]
fn check_interrupt_reports_sqlite3_interrupt() {
    let db = Db::mem();
    db.insert(&random_quads(4, 500, false));
    db.store.check_interrupt().unwrap();
    // While one of our statements runs, an interrupt makes every statement fail.
    let mut c = db.store.cursor(&ScanSpec::new(IndexOrder::Gspo, &[])).unwrap();
    assert_eq!(c.fill(&mut Vec::new(), 1).unwrap(), 1);
    db.shared().db().interrupt();
    assert!(matches!(db.store.check_interrupt(), Err(Error::Interrupted)));
    assert!(matches!(c.fill(&mut Vec::new(), 100), Err(Error::Interrupted)));
    drop(c);
    // Once nothing runs any more, the flag is cleared by the next statement.
    db.store.check_interrupt().unwrap();
}

#[test]
fn sql_rows_runs_generated_joins() {
    let db = Db::mem();
    let p = TermId::from_dict(1100, Tag::Iri);
    let q = TermId::from_dict(1101, Tag::Iri);
    let quads: Vec<[TermId; 4]> = (0..10i64)
        .flat_map(|i| {
            let a = TermId::from_dict(2000 + i, Tag::Iri);
            let b = TermId::from_dict(3000 + i, Tag::Iri);
            [[TermId::DEFAULT_GRAPH, a, p, b], [TermId::DEFAULT_GRAPH, b, q, TermId::from_int(i).unwrap()]]
        })
        .collect();
    db.insert(&quads);
    let sql = "SELECT t0.s, t1.o, NULL FROM main.rdf_quad AS t0 INDEXED BY rdf_gpso \
               CROSS JOIN main.rdf_quad AS t1 INDEXED BY sqlite_autoindex_rdf_quad_1 \
               WHERE t0.g=0 AND t0.p=?1 AND t1.g=0 AND t1.s=t0.o AND t1.p=?2 ORDER BY t0.s";
    let mut c = db.store.sql_rows(sql, &[p.0, q.0], 3).unwrap();
    assert_eq!(c.width(), 3);
    let mut out = Vec::new();
    while c.fill(&mut out, 4).unwrap() > 0 {}
    assert_eq!(out.len(), 30);
    for (i, row) in out.chunks_exact(3).enumerate() {
        assert_eq!(row, [TermId::from_dict(2000 + i as i64, Tag::Iri), TermId::from_int(i as i64).unwrap(), TermId::UNBOUND]);
    }
    assert_eq!(c.counters().rows, 10);
    assert!(matches!(c.seek(&[TermId(1)]), Err(Error::Unsupported(_))));
    assert!(matches!(c.rebind(&[], std::ops::Bound::Unbounded, std::ops::Bound::Unbounded), Err(Error::Unsupported(_))));
    assert!(matches!(db.store.sql_rows("SELECT 1, 2", &[], 1), Err(Error::Invalid(_))));
    assert!(matches!(db.store.sql_rows("SELECT * FROM no_such_table", &[], 1), Err(Error::Sqlite { .. })));
    assert!(db.store.caps().sql);
}

#[test]
fn status_reports_storage_state() {
    let db = Db::mem();
    db.shared().validate_epoch().unwrap();
    let status: std::collections::HashMap<String, String> = db.shared().status().into_iter().collect();
    for key in ["schema_state", "dict_gen", "stats_gen", "committed_max_row", "decode_cache_entries", "decode_cache_bytes", "pool_statements", "armed"] {
        assert!(status.contains_key(key), "{key}");
    }
    assert!(status["schema_state"].starts_with("ready"));
    assert_eq!(status["armed"], "1");
    assert_eq!(status["committed_max_row"], sq_core::codec::WELL_KNOWN.len().to_string());
}

/// A cursor still stepping when its read scope ends: RELEASE succeeds (SQLite commits a
/// read-only transaction with readers in flight), the scan keeps working and the
/// connection returns to autocommit once the cursor is gone.
#[test]
fn read_scope_release_with_a_live_cursor() {
    let db = Db::file();
    db.insert(&random_quads(6, 400, false));
    let scope = db.store.read_scope(0).unwrap();
    assert!(scope.is_active());
    let mut c = db.store.cursor(&ScanSpec::new(IndexOrder::Gspo, &[])).unwrap();
    let mut out = Vec::new();
    assert_eq!(c.fill(&mut out, 10).unwrap(), 10);
    drop(scope);
    assert!(db.shared().db().autocommit(), "savepoint released despite the running statement");
    while c.fill(&mut out, 100).unwrap() > 0 {}
    assert_eq!(out.len() as i64 / 4, db.query_i64("SELECT count(*) FROM rdf_quad"));
    drop(c);
    assert_eq!(db.shared().db().txn_state(), ffi::SQLITE_TXN_NONE);
}

#[test]
fn zz_debug_forced_rollback() {
    use crate::raw::RawStmt;
    for sql in ["PRAGMA main.journal_mode", "UPDATE main.rdf_meta SET value=value WHERE key='write_gen'", "UPDATE main.rdf_meta SET value=value WHERE 0", "DELETE FROM main.rdf_graph WHERE 0"] {
        let db = Db::mem();
        let mut host = db.conn.prepare("WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM c LIMIT 10) SELECT x FROM c").unwrap();
        let mut rows = host.query([]).unwrap();
        assert!(rows.next().unwrap().is_some());
        let mut abort = RawStmt::prepare(db.shared().db(), sql, true).unwrap();
        db.conn.execute_batch("SAVEPOINT x; INSERT INTO rdf_graph VALUES (5)").unwrap();
        db.shared().db().interrupt();
        let r = abort.run();
        eprintln!("{sql}: {r:?}, autocommit after {}", db.shared().db().autocommit());
    }
}
