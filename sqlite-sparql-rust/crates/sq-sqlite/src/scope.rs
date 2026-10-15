//! Transaction scopes (§8.6).
//!
//! * `ReadScope`: `SAVEPOINT sq_r{d}` only when `sqlite3_txn_state(db, "main") == NONE` (a bare
//!   `SELECT f()`), released on drop, so every statement of the call sees one snapshot and
//!   probes run inside a held read transaction. Inside `xFilter` (outer statement already
//!   reading) nothing is opened.
//! * `WriteScope`: `SAVEPOINT sq_w{d}` then a `write_gen` bump that takes the write lock first.
//!   `commit()` releases; dropping without commit rolls back to the savepoint and releases it.
//!
//! Both nest inside user transactions and inside running statements (`SELECT
//! sparql_update(…)`). Savepoint names carry the depth (rule R3).
use std::rc::Rc;

use rusqlite::ffi;
use sq_core::Result;

use crate::pool::PoolKey;
use crate::shared::StorageShared;
use crate::template::{Fixed, TxnOp};

fn run_txn(shared: &StorageShared, op: TxnOp, depth: u32) -> Result<()> {
    let mut st = shared.pool.lease(PoolKey::Txn(op, depth))?;
    st.run()
}

/// RAII read scope.
pub struct ReadScope {
    shared: Rc<StorageShared>,
    depth: u32,
    /// A savepoint was opened (no transaction was active at entry).
    active: bool,
}

impl ReadScope {
    pub(crate) fn open(shared: &Rc<StorageShared>, depth: u32) -> Result<ReadScope> {
        let depth = shared.enter_scope(depth);
        let mut scope = ReadScope { shared: shared.clone(), depth, active: false };
        if shared.db().txn_state() == ffi::SQLITE_TXN_NONE {
            run_txn(shared, TxnOp::SavepointRead, depth)?;
            scope.active = true;
        }
        Ok(scope)
    }

    /// Whether this scope opened its own savepoint.
    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn depth(&self) -> u32 {
        self.depth
    }
}

impl Drop for ReadScope {
    fn drop(&mut self) {
        if self.active {
            // A read-only savepoint: RELEASE cannot lose data; an error (e.g. the transaction
            // was already ended by user SQL) leaves nothing to undo.
            let _ = run_txn(&self.shared, TxnOp::ReleaseRead, self.depth);
        }
        self.shared.leave_scope();
    }
}

/// RAII write scope: `commit()` or roll back on drop.
pub struct WriteScope {
    shared: Rc<StorageShared>,
    depth: u32,
    finished: bool,
}

impl WriteScope {
    pub(crate) fn open(shared: &Rc<StorageShared>, depth: u32) -> Result<WriteScope> {
        let depth = shared.enter_scope(depth);
        if let Err(e) = run_txn(shared, TxnOp::SavepointWrite, depth) {
            shared.leave_scope();
            return Err(e);
        }
        let scope = WriteScope { shared: shared.clone(), depth, finished: false };
        // Take the write lock first (busy_timeout applies here, not halfway through a load).
        // Before sparql_init there is no rdf_meta yet: the DDL itself takes the lock.
        if shared.tables_present()? {
            let mut st = shared.pool.lease_fixed(Fixed::MetaBump)?;
            st.bind_text(1, "write_gen")?;
            st.run()?;
        }
        Ok(scope)
    }

    pub fn depth(&self) -> u32 {
        self.depth
    }

    /// RELEASE the savepoint (commits if it is the outermost transaction).
    pub fn commit(mut self) -> Result<()> {
        self.finished = true;
        let r = run_txn(&self.shared, TxnOp::ReleaseWrite, self.depth);
        if r.is_err() {
            self.rollback();
        }
        r
    }

    fn rollback(&mut self) {
        // ROLLBACK TO keeps the savepoint on the stack; RELEASE removes it. Errors here mean
        // the transaction was already rolled back by SQLite (e.g. SQLITE_FULL), so there is
        // nothing left to undo.
        let _ = run_txn(&self.shared, TxnOp::RollbackWrite, self.depth);
        let _ = run_txn(&self.shared, TxnOp::ReleaseWrite, self.depth);
        self.shared.after_rollback();
    }
}

impl Drop for WriteScope {
    fn drop(&mut self) {
        if !self.finished {
            self.finished = true;
            self.rollback();
        }
        self.shared.leave_scope();
    }
}
