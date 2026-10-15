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

/// A statement that keeps running while our code executes, standing in for the host
/// statement of `SELECT sparql_update(…)`: it reads no database, so it neither changes
/// `txn_state(main)` nor autocommit, and while it runs a pending interrupt stays pending.
const HOST: &str = "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM c LIMIT 10) SELECT x FROM c";

/// The forced-rollback statement is a possible writer for SQLite (so an interrupted step
/// rolls the transaction back) yet changes nothing and starts no write transaction when it
/// runs normally.
#[test]
fn abort_statement_is_a_harmless_possible_writer() {
    use crate::template::Fixed;
    let db = Db::mem();
    assert!(!db.conn.prepare(Fixed::Abort.sql()).unwrap().readonly());
    let tables = db.query_i64("SELECT count(*) FROM sqlite_schema");
    let r = db.store.read_scope(0).unwrap();
    db.shared().refresh_abort().unwrap();
    assert_eq!(db.shared().db().txn_state(), ffi::SQLITE_TXN_READ);
    drop(r);
    assert_eq!(db.query_i64("SELECT count(*) FROM sqlite_schema"), tables);
}

/// `CREATE INDEX` (here: user DDL between calls, and our own DDL inside the scope) expires
/// every prepared statement, and an expired statement cannot be re-prepared while an
/// interrupt is pending. The abort statement is refreshed when the scope opens and after DDL,
/// so the forced rollback still works.
#[test]
fn interrupted_write_scope_rolls_back_after_statement_expiring_ddl() {
    let db = Db::mem();
    // Pool every scope statement, then expire them all.
    db.insert(&random_quads(20, 5, false));
    db.conn.execute_batch("CREATE TABLE unrelated(x); CREATE INDEX unrelated_x ON unrelated(x);").unwrap();
    let before = db.query_i64("SELECT count(*) FROM rdf_quad");
    let mut host = db.conn.prepare(HOST).unwrap();
    {
        let mut rows = host.query([]).unwrap();
        assert!(rows.next().unwrap().is_some());
        let ws = db.store.write_scope(0).unwrap();
        db.store.insert(&mut random_quads(21, 40, false)).unwrap();
        // DDL inside the scope, as the loader runs it (exec, then refresh).
        db.shared().db().exec("CREATE INDEX IF NOT EXISTS main.unrelated_x2 ON unrelated(x)").unwrap();
        db.shared().refresh_abort().unwrap();
        db.shared().db().interrupt();
        assert!(matches!(db.store.check_interrupt(), Err(Error::Interrupted)));
        assert!(matches!(ws.rollback(), Ok(())), "the forced rollback ends the transaction");
    }
    drop(host);
    assert!(db.shared().db().autocommit());
    assert_eq!(db.query_i64("SELECT count(*) FROM rdf_quad"), before);
    assert_eq!(db.query_i64("SELECT count(*) FROM sqlite_schema WHERE name='unrelated_x2'"), 0);
    // The store is usable afterwards.
    db.insert(&random_quads(22, 3, false));
}

/// Inside a user transaction, an update that cannot be undone on its own because of an
/// interrupt rolls back the whole user transaction (SQLite's rule for interrupted writes)
/// rather than leaving half of it to a later COMMIT.
#[test]
fn interrupted_write_scope_inside_a_user_transaction_rolls_it_back() {
    let db = Db::mem();
    db.conn.execute_batch("BEGIN; INSERT INTO rdf_graph(g) VALUES (4242);").unwrap();
    let mut host = db.conn.prepare(HOST).unwrap();
    {
        let mut rows = host.query([]).unwrap();
        assert!(rows.next().unwrap().is_some());
        let ws = db.store.write_scope(0).unwrap();
        db.store.insert(&mut random_quads(23, 30, false)).unwrap();
        db.shared().db().interrupt();
        assert!(matches!(db.store.check_interrupt(), Err(Error::Interrupted)));
        ws.rollback().unwrap();
    }
    drop(host);
    assert!(db.shared().db().autocommit(), "no half-applied transaction is left open");
    assert_eq!(db.query_i64("SELECT count(*) FROM rdf_quad"), 0);
    assert_eq!(db.query_i64("SELECT count(*) FROM rdf_graph"), 0);
}

/// A read-only savepoint inside the user's (deferred) transaction is left to that
/// transaction when its RELEASE is interrupted: the user's work survives and COMMIT ends both.
#[test]
fn interrupted_read_scope_inside_a_user_transaction_keeps_it() {
    let db = Db::mem();
    db.conn.execute_batch("BEGIN").unwrap();
    let mut host = db.conn.prepare(HOST).unwrap();
    {
        let mut rows = host.query([]).unwrap();
        assert!(rows.next().unwrap().is_some());
        let scope = db.store.read_scope(0).unwrap();
        assert!(scope.is_active());
        db.shared().validate_epoch().unwrap();
        db.shared().db().interrupt();
        assert!(matches!(scope.close(), Err(Error::Interrupted)));
    }
    drop(host);
    assert!(!db.shared().db().autocommit(), "the user's transaction is still open");
    db.conn.execute_batch("INSERT INTO rdf_graph(g) VALUES (77); COMMIT;").unwrap();
    assert!(db.shared().db().autocommit());
    assert_eq!(db.query_i64("SELECT count(*) FROM rdf_graph"), 1);
}

/// A read scope that owns the transaction commits nested writes when it closes; a refused
/// COMMIT (SQLITE_BUSY: another connection reads a rollback-journal file) is reported by
/// `close`, and the transaction is rolled back rather than left open.
#[test]
fn read_scope_close_reports_a_refused_commit_and_rolls_back() {
    let db = Db::file();
    db.insert(&random_quads(24, 10, false));
    db.conn.busy_timeout(std::time::Duration::ZERO).unwrap();
    let reader = db.reopen();
    reader.conn.execute_batch("BEGIN; SELECT count(*) FROM rdf_quad;").unwrap();
    let scope = db.store.read_scope(0).unwrap();
    assert!(scope.is_active());
    db.shared().validate_epoch().unwrap();
    // The write lock (RESERVED) is compatible with the reader's SHARED lock …
    let ws = db.store.write_scope(1).unwrap();
    db.store.insert(&mut random_quads(25, 20, false)).unwrap();
    ws.commit().unwrap();
    // … the COMMIT (EXCLUSIVE) is not.
    let closed = scope.close();
    assert!(matches!(&closed, Err(Error::Sqlite { code, .. }) if code & 0xff == ffi::SQLITE_BUSY), "{closed:?}");
    assert!(db.shared().db().autocommit());
    reader.conn.execute_batch("COMMIT").unwrap();
    assert_eq!(db.query_i64("SELECT count(*) FROM rdf_quad"), 10);
}

#[test]
fn write_scope_rollback_undoes_and_reports() {
    let db = Db::mem();
    let ws = db.store.write_scope(0).unwrap();
    db.store.insert(&mut random_quads(26, 15, true)).unwrap();
    ws.rollback().unwrap();
    assert!(db.shared().db().autocommit());
    assert_eq!(db.query_i64("SELECT count(*) FROM rdf_quad"), 0);
    assert_eq!(db.query_i64("SELECT count(*) FROM rdf_graph"), 0);
    // Nested: only the inner scope is undone.
    let outer = db.store.write_scope(0).unwrap();
    db.store.insert(&mut random_quads(27, 5, false)).unwrap();
    let inner = db.store.write_scope(0).unwrap();
    db.store.insert(&mut random_quads(28, 5, false)).unwrap();
    inner.rollback().unwrap();
    outer.commit().unwrap();
    assert_eq!(db.query_i64("SELECT count(*) FROM rdf_quad"), 5);
}

/// A failed positioning is reported again by `fill` (never a silent end of the scan) until
/// the cursor is positioned anew.
#[test]
fn cursor_fill_after_a_failed_rebind_reports_until_repositioned() {
    let db = Db::mem();
    let quads = random_quads(29, 200, false);
    db.insert(&quads);
    let spec = ScanSpec::new(IndexOrder::Gpso, &[TermId::DEFAULT_GRAPH]);
    let mut cur = db.store.cursor(&spec).unwrap();
    let mut running = db.store.cursor(&ScanSpec::new(IndexOrder::Gspo, &[])).unwrap();
    assert_eq!(running.fill(&mut Vec::new(), 1).unwrap(), 1);
    db.shared().db().interrupt();
    let (lo, hi) = (std::ops::Bound::Excluded(i64::MIN), std::ops::Bound::Included(i64::MAX));
    assert!(matches!(cur.rebind(&[TermId::DEFAULT_GRAPH], lo, hi), Err(Error::Interrupted)));
    assert!(matches!(cur.fill(&mut Vec::new(), 10), Err(Error::Interrupted)));
    assert!(matches!(cur.fill(&mut Vec::new(), 10), Err(Error::Interrupted)), "still failed");
    drop(running);
    cur.rebind(&[TermId::DEFAULT_GRAPH], lo, hi).unwrap();
    let mut out = Vec::new();
    while cur.fill(&mut out, 64).unwrap() > 0 {}
    assert_eq!(out.len() / 3, quads.len());
}
