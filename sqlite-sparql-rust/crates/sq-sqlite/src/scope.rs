//! Transaction scopes (§8.6).
//!
//! * `ReadScope`: `SAVEPOINT sq_r{d}` only when `sqlite3_txn_state(db, "main") == NONE` (a bare
//!   `SELECT f()`), released at the end, so every statement of the call sees one snapshot and
//!   probes run inside a held read transaction. Inside `xFilter` (outer statement already
//!   reading) nothing is opened.
//! * `WriteScope`: `SAVEPOINT sq_w{d}` then a `write_gen` bump that takes the write lock first.
//!   `commit()` releases; dropping without commit undoes the scope's changes.
//!
//! Both nest inside user transactions and inside running statements (`SELECT
//! sparql_update(…)`). Savepoint names carry the depth (rule R3).
//!
//! **Undoing a scope.** When the savepoint started the transaction (autocommit was on at
//! entry), a plain `ROLLBACK` ends it: it needs no lock, whereas `RELEASE` after `ROLLBACK TO`
//! commits an (empty) write transaction and fails with SQLITE_BUSY while another connection
//! reads a rollback-journal database, leaving the RESERVED lock and the savepoint behind.
//! Nested in a user transaction, `ROLLBACK TO` + `RELEASE` undo just the scope.
//!
//! **Interrupts.** After `sqlite3_interrupt` the flag stays set until no statement of the
//! connection runs — and the host statement that called us is running — so every statement
//! fails at its first instruction, the savepoint statements included, and so does every
//! prepare (including the automatic re-prepare of an expired statement). SQLite rolls the
//! transaction back by itself only when a statement that may write is interrupted
//! (`sqlite3VdbeHalt`); savepoint statements are read-only, so nothing would end the scope's
//! transaction and a later COMMIT would make half an update durable. A scope that cannot end
//! its savepoint because of an interrupt therefore steps the connection's [`Abort`] statement
//! (`Fixed::Abort`) to get exactly that rollback. Inside a user transaction this rolls back
//! the whole user transaction, which is SQLite's documented behaviour for an interrupted
//! INSERT / UPDATE / DELETE.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rusqlite::ffi;
use sq_core::{Error, Result};

use crate::pool::{Lease, PoolKey, StmtPool};
use crate::shared::StorageShared;
use crate::template::{Fixed, TxnOp};

fn run_txn(shared: &StorageShared, op: TxnOp, depth: u32) -> Result<()> {
    shared.pool.lease(PoolKey::Txn(op, depth))?.run()
}

/// The connection's forced-rollback statement, leased while any scope savepoint is open.
///
/// It must be runnable without a re-prepare when it is needed, but SQLite expires every
/// prepared statement on `CREATE INDEX` (and a few other events), and an expired statement is
/// re-prepared on its next step. So it is stepped normally ("refreshed") when the outermost
/// scope opens and after every DDL statement this crate runs inside a scope
/// (`StorageShared::refresh_abort`). A normal run changes nothing.
#[derive(Default)]
pub(crate) struct Abort {
    stmt: RefCell<Option<Lease>>,
    holders: Cell<u32>,
}

impl Abort {
    fn acquire(&self, pool: &Rc<StmtPool>) -> Result<()> {
        if self.holders.get() == 0 {
            let lease = pool.lease_fixed(Fixed::Abort)?;
            *self.stmt.try_borrow_mut().map_err(|_| Error::Reentrant)? = Some(lease);
        }
        self.holders.set(self.holders.get() + 1);
        Ok(())
    }

    fn release(&self) {
        let n = self.holders.get().saturating_sub(1);
        self.holders.set(n);
        if n == 0 {
            // Back to the pool outside the borrow (rule R1).
            let lease = self.stmt.try_borrow_mut().ok().and_then(|mut s| s.take());
            drop(lease);
        }
    }

    /// Steps the statement once (outside any `RefCell` borrow, rule R1) and resets it.
    /// No-op while no scope holds it.
    fn step(&self) -> Result<()> {
        let taken = self.stmt.try_borrow_mut().map_err(|_| Error::Reentrant)?.take();
        let Some(mut st) = taken else { return Ok(()) };
        let r = st.run();
        st.reset();
        *self.stmt.try_borrow_mut().map_err(|_| Error::Reentrant)? = Some(st);
        r
    }
}

impl StorageShared {
    /// Re-validates the forced-rollback statement after DDL run inside a scope (see [`Abort`]).
    /// Must run outside a pending interrupt to help; if one is pending, the step itself is
    /// interrupted and SQLite rolls the transaction back (the error is returned).
    pub(crate) fn refresh_abort(&self) -> Result<()> {
        self.abort.step()
    }

    /// Steps the abort statement while an interrupt is pending: SQLite then rolls back the
    /// whole transaction. `Ok` when the connection is back in autocommit mode.
    fn force_rollback(&self) -> Result<()> {
        let stepped = self.abort.step();
        if self.db().autocommit() {
            return Ok(());
        }
        stepped?;
        Err(Error::internal("forced rollback did not end the transaction"))
    }
}

#[derive(Copy, Clone)]
enum Kind {
    Read,
    Write,
}

/// A savepoint opened by a scope; holds the connection's [`Abort`] statement while it lives.
struct Savepoint {
    shared: Rc<StorageShared>,
    kind: Kind,
    depth: u32,
    /// `SAVEPOINT` started the transaction (autocommit was on): its `RELEASE` commits.
    owns_txn: bool,
}

impl Savepoint {
    fn open(shared: &Rc<StorageShared>, kind: Kind, depth: u32) -> Result<Savepoint> {
        shared.abort.acquire(&shared.pool)?;
        // From here on `Drop` releases the abort statement.
        let sp = Savepoint { shared: shared.clone(), kind, depth, owns_txn: shared.db().autocommit() };
        let op = match kind {
            Kind::Read => TxnOp::SavepointRead,
            Kind::Write => TxnOp::SavepointWrite,
        };
        run_txn(shared, op, depth)?;
        Ok(sp)
    }

    /// `RELEASE`: keeps the changes (commits when the savepoint owns the transaction).
    fn release(&self) -> Result<()> {
        let op = match self.kind {
            Kind::Read => TxnOp::ReleaseRead,
            Kind::Write => TxnOp::ReleaseWrite,
        };
        run_txn(&self.shared, op, self.depth)
    }

    /// Undoes the scope's changes and ends its savepoint (see the module docs), forcing the
    /// rollback when an interrupt is pending. `Ok` means the savepoint is gone.
    fn discard(&self) -> Result<()> {
        match self.discard_once() {
            Err(Error::Interrupted) => self.shared.force_rollback(),
            r => r,
        }
    }

    fn discard_once(&self) -> Result<()> {
        let shared = &*self.shared;
        if shared.db().autocommit() {
            // SQLite already rolled the transaction back (an interrupted or failed write:
            // SQLITE_INTERRUPT / _FULL / _IOERR / _NOMEM), closing every savepoint.
            return Ok(());
        }
        if self.owns_txn {
            return shared.pool.lease_fixed(Fixed::Rollback)?.run();
        }
        let (to, release) = match self.kind {
            Kind::Read => (TxnOp::RollbackRead, TxnOp::ReleaseRead),
            Kind::Write => (TxnOp::RollbackWrite, TxnOp::ReleaseWrite),
        };
        run_txn(shared, to, self.depth)?;
        run_txn(shared, release, self.depth)?;
        // The enclosing transaction goes on; undoing DDL expired every statement.
        shared.refresh_abort()
    }
}

impl Drop for Savepoint {
    fn drop(&mut self) {
        self.shared.abort.release();
    }
}

/// RAII read scope. `close()` reports whether the savepoint ended cleanly; dropping the scope
/// closes it too, ignoring the outcome.
pub struct ReadScope {
    shared: Rc<StorageShared>,
    depth: u32,
    /// The savepoint, when one was opened (no transaction was active on `main` at entry).
    savepoint: Option<Savepoint>,
    closed: bool,
}

impl ReadScope {
    pub(crate) fn open(shared: &Rc<StorageShared>, depth: u32) -> Result<ReadScope> {
        let depth = shared.enter_scope(depth);
        // Built first: on any error below, `Drop` ends whatever was opened.
        let mut scope = ReadScope { shared: shared.clone(), depth, savepoint: None, closed: false };
        if shared.db().txn_state() == ffi::SQLITE_TXN_NONE {
            scope.savepoint = Some(Savepoint::open(shared, Kind::Read, depth)?);
            // Starts the read transaction a moment before the caller's first read would.
            shared.refresh_abort()?;
        }
        Ok(scope)
    }

    /// Whether this scope opened its own savepoint.
    pub fn is_active(&self) -> bool {
        self.savepoint.is_some()
    }

    pub fn depth(&self) -> u32 {
        self.depth
    }

    /// Ends the scope. When its savepoint started the transaction, `RELEASE` commits whatever
    /// nested write scopes did, so callers that write inside a read scope must close it
    /// explicitly to learn whether that commit succeeded. On failure the transaction is rolled
    /// back (never left open) and the error returned.
    pub fn close(mut self) -> Result<()> {
        self.finish()
    }

    fn finish(&mut self) -> Result<()> {
        if std::mem::replace(&mut self.closed, true) {
            return Ok(());
        }
        let Some(sp) = &self.savepoint else { return Ok(()) };
        let db = self.shared.db();
        if db.autocommit() {
            // A nested write scope's forced rollback (or SQLite itself) already ended the
            // transaction, savepoint included.
            return Ok(());
        }
        // txn_state was NONE at entry: a write transaction now comes from nested write scopes.
        let wrote = db.txn_state() == ffi::SQLITE_TXN_WRITE;
        let Err(e) = sp.release() else { return Ok(()) };
        if !sp.owns_txn && !wrote {
            // A read-only savepoint inside the user's transaction holds nothing that
            // transaction does not, and ends with it. Rolling back would undo the user's work
            // for an interrupted read, which SQLite never does.
            return Err(e);
        }
        let discarded = sp.discard();
        if wrote {
            self.shared.after_rollback();
        }
        discarded.and(Err(e))
    }
}

impl Drop for ReadScope {
    fn drop(&mut self) {
        // Errors cannot be reported from drop (`close` is the reporting variant); `finish`
        // leaves no owned transaction open unless even the forced rollback failed.
        let _ = self.finish();
        self.savepoint = None;
        self.shared.leave_scope();
    }
}

/// RAII write scope: `commit()`, or undo the scope's changes with `rollback()` / on drop.
pub struct WriteScope {
    shared: Rc<StorageShared>,
    depth: u32,
    savepoint: Option<Savepoint>,
    finished: bool,
}

impl WriteScope {
    pub(crate) fn open(shared: &Rc<StorageShared>, depth: u32) -> Result<WriteScope> {
        let depth = shared.enter_scope(depth);
        // Built first: on any error below, `Drop` undoes whatever was opened.
        let mut scope = WriteScope { shared: shared.clone(), depth, savepoint: None, finished: false };
        scope.savepoint = Some(Savepoint::open(shared, Kind::Write, depth)?);
        // Take the write lock first, directly from "no transaction": busy_timeout applies
        // here (SQLite skips the busy handler when upgrading a held read lock), not halfway
        // through a load. Before sparql_init there is no rdf_meta yet: the DDL itself takes
        // the lock and `schema::create` refreshes the abort statement afterwards.
        if shared.tables_present()? {
            let mut st = shared.pool.lease_fixed(Fixed::MetaBump)?;
            st.bind_text(1, "write_gen")?;
            st.run()?;
            drop(st);
            shared.refresh_abort()?;
        }
        Ok(scope)
    }

    pub fn depth(&self) -> u32 {
        self.depth
    }

    /// `RELEASE` the savepoint (commits if it is the outermost transaction). On failure (e.g.
    /// SQLITE_BUSY at COMMIT) the scope's changes are rolled back, the savepoint is gone and
    /// the error is returned; if even the rollback fails, its error is returned instead.
    pub fn commit(mut self) -> Result<()> {
        self.finished = true;
        let released = match &self.savepoint {
            Some(sp) => sp.release(),
            None => Ok(()),
        };
        let Err(e) = released else { return Ok(()) };
        self.undo().and(Err(e))
    }

    /// Undoes the scope's changes now, reporting failure (dropping the scope does the same
    /// silently).
    pub fn rollback(mut self) -> Result<()> {
        self.finished = true;
        self.undo()
    }

    fn undo(&mut self) -> Result<()> {
        let Some(sp) = &self.savepoint else { return Ok(()) };
        let r = sp.discard();
        self.shared.after_rollback();
        r
    }
}

impl Drop for WriteScope {
    fn drop(&mut self) {
        if !std::mem::replace(&mut self.finished, true) {
            // See ReadScope::drop: `rollback()` is the reporting variant.
            let _ = self.undo();
        }
        self.savepoint = None;
        self.shared.leave_scope();
    }
}
