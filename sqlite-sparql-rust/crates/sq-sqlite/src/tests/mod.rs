//! sq-sqlite tests (§12.1). Every test runs against the system libsqlite3 (3.45.1, the same
//! engine the Python host loads the extension into) through `rusqlite::Connection`.
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use rusqlite::Connection;
use sq_core::store::QuadStoreWriter;
use sq_core::TermId;

use crate::{RawDb, SqliteStore, StorageConfig, StorageShared};

mod analyze;
mod btree;
mod btree_perf;
mod conformance;
mod dict;
mod eqp;
mod loader;
mod maint;
mod perf;
mod review;
mod review_btree;
mod review_perf;
mod scope;
mod writer;

/// Serializes the release performance tests (`--ignored`): they measure timings, and the
/// 1M-triple ones start with a multi-threaded bulk load, so running them in parallel (the
/// default for `cargo test -- --ignored`) makes every budget load-sensitive. Each holds the
/// guard for its whole run; a failed (panicked) test does not block the others.
pub(crate) fn perf_lock() -> MutexGuard<'static, ()> {
    static PERF: Mutex<()> = Mutex::new(());
    PERF.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Removes a temporary database file (and its journals) after the connection closed.
pub(crate) struct TempPath(pub PathBuf);

impl TempPath {
    pub fn new(tag: &str) -> TempPath {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("sq-sqlite-{tag}-{}-{n}.db", std::process::id()));
        let t = TempPath(p);
        t.remove();
        t
    }
    fn remove(&self) {
        for suffix in ["", "-journal", "-wal", "-shm"] {
            let mut p = self.0.clone().into_os_string();
            p.push(suffix);
            let _ = std::fs::remove_file(p);
        }
    }
}

impl Drop for TempPath {
    fn drop(&mut self) {
        self.remove();
    }
}

/// A connection plus an initialized, armed store. Fields drop in declaration order: the
/// store (and its pooled statements) before the connection, the file last.
pub(crate) struct Db {
    pub store: SqliteStore,
    pub conn: Connection,
    pub path: Option<TempPath>,
}

impl Db {
    pub fn mem() -> Db {
        Self::with(Connection::open_in_memory().unwrap(), None)
    }

    pub fn file() -> Db {
        let path = TempPath::new("t");
        let conn = Connection::open(&path.0).unwrap();
        Self::with(conn, Some(path))
    }

    /// Store over an existing connection; `ensure_schema` already run.
    pub fn with(conn: Connection, path: Option<TempPath>) -> Db {
        let store = store_for(&conn);
        store.ensure_schema().unwrap();
        Db { store, conn, path }
    }

    /// Another connection to the same file (file databases only).
    pub fn reopen(&self) -> Db {
        let conn = Connection::open(&self.path.as_ref().expect("file database").0).unwrap();
        let store = store_for(&conn);
        Db { store, conn, path: None }
    }

    pub fn shared(&self) -> &Rc<StorageShared> {
        &self.store.0
    }

    /// Inserts inside a committed write scope (one transaction, as real callers do).
    pub fn insert(&self, quads: &[[TermId; 4]]) -> u64 {
        let mut v = quads.to_vec();
        let ws = self.store.write_scope(0).unwrap();
        let n = self.store.insert(&mut v).unwrap();
        ws.commit().unwrap();
        n
    }

    pub fn remove(&self, quads: &[[TermId; 4]]) -> u64 {
        let mut v = quads.to_vec();
        let ws = self.store.write_scope(0).unwrap();
        let n = self.store.remove(&mut v).unwrap();
        ws.commit().unwrap();
        n
    }

    pub fn query_i64(&self, sql: &str) -> i64 {
        self.conn.query_row(sql, [], |r| r.get(0)).unwrap()
    }
}

/// An armed store over `conn` (schema not touched).
pub(crate) fn store_for(conn: &Connection) -> SqliteStore {
    // SAFETY: the connection outlives the store in every test (see `Db` field order).
    let db = unsafe { RawDb::from_ptr(conn.handle()) };
    let shared = StorageShared::new(db, StorageConfig::default());
    shared.arm_pool();
    SqliteStore::new(shared)
}

pub(crate) fn iri(s: &str) -> oxrdf::NamedNode {
    oxrdf::NamedNode::new(s).unwrap()
}

#[test]
fn links_the_host_sqlite_version() {
    // SAFETY: plain C call without arguments.
    assert_eq!(unsafe { rusqlite::ffi::sqlite3_libversion_number() }, 3_045_001);
}
