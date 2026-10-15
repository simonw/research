//! `StmtPool` (§5.2): lazily prepared `SQLITE_PREPARE_PERSISTENT` statements, keyed by
//! `StmtKey`, fixed statement, transaction op + depth, or SQL text (LRU of 256 for
//! `sql_rows`). At most 8 idle statements per key. Cursors lease statements and give them back
//! on drop (reset + clear bindings).
//!
//! The pool is **armed** only while the close hook (the `sparql_status` vtab, §8.7) is
//! connected. While disarmed, released leases are finalized instead of kept, so the
//! connection can always close cleanly.
//!
//! Rule R1: the `RefCell`s are borrowed only for O(1) check-out / check-in, never while a
//! statement is prepared or stepped.
use std::cell::{Cell, RefCell};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use rustc_hash::FxHashMap;
use sq_core::{Error, Result};

use crate::raw::{RawDb, RawStmt};
use crate::template::{Fixed, StmtKey, TxnOp};

/// Idle statements kept per key.
pub(crate) const MAX_IDLE_PER_KEY: usize = 8;
/// Distinct SQL texts (SqlJoin statements) kept prepared.
pub(crate) const SQL_LRU_CAPACITY: usize = 256;
/// Savepoint depths with pooled statements; deeper nesting prepares one-shot statements.
const POOLED_TXN_DEPTH: u32 = 16;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) enum PoolKey {
    Scan(StmtKey),
    Fixed(Fixed),
    Txn(TxnOp, u32),
    Sql(Rc<str>),
}

impl PoolKey {
    fn sql(&self) -> std::borrow::Cow<'_, str> {
        match self {
            PoolKey::Scan(k) => k.sql().into(),
            PoolKey::Fixed(f) => f.sql().into(),
            PoolKey::Txn(op, depth) => op.sql(*depth).into(),
            PoolKey::Sql(s) => (&**s).into(),
        }
    }

    fn poolable(&self) -> bool {
        !matches!(self, PoolKey::Txn(_, d) if *d >= POOLED_TXN_DEPTH)
    }
}

#[derive(Default)]
struct SqlLru {
    tick: u64,
    last_use: FxHashMap<Rc<str>, u64>,
}

/// Pool counters (reported by `StorageShared::status`).
#[derive(Copy, Clone, Default, Debug)]
pub struct PoolCounters {
    pub prepared: u64,
    pub reused: u64,
    pub finalized: u64,
}

pub(crate) struct StmtPool {
    db: RawDb,
    idle: RefCell<FxHashMap<PoolKey, Vec<RawStmt>>>,
    sql_lru: RefCell<SqlLru>,
    armed: Cell<bool>,
    leased: Cell<usize>,
    counters: Cell<PoolCounters>,
}

impl StmtPool {
    pub fn new(db: RawDb) -> Rc<StmtPool> {
        Rc::new(StmtPool {
            db,
            idle: RefCell::new(FxHashMap::default()),
            sql_lru: RefCell::new(SqlLru::default()),
            armed: Cell::new(false),
            leased: Cell::new(0),
            counters: Cell::new(PoolCounters::default()),
        })
    }

    pub fn arm(&self) {
        self.armed.set(true);
    }

    pub fn is_armed(&self) -> bool {
        self.armed.get()
    }

    /// Disarms and finalizes every idle statement (close hook). Outstanding leases finalize
    /// when they are dropped.
    pub fn disarm_and_finalize(&self) {
        self.armed.set(false);
        // Take the map out first so no borrow is held while sqlite3_finalize runs.
        let idle = std::mem::take(&mut *self.idle.borrow_mut());
        let n: usize = idle.values().map(Vec::len).sum();
        drop(idle);
        self.sql_lru.borrow_mut().last_use.clear();
        self.bump(|c| c.finalized += n as u64);
    }

    pub fn idle_statements(&self) -> usize {
        self.idle.try_borrow().map(|m| m.values().map(Vec::len).sum()).unwrap_or(0)
    }

    pub fn leased_statements(&self) -> usize {
        self.leased.get()
    }

    pub fn counters(&self) -> PoolCounters {
        self.counters.get()
    }

    fn bump(&self, f: impl FnOnce(&mut PoolCounters)) {
        let mut c = self.counters.get();
        f(&mut c);
        self.counters.set(c);
    }

    /// Checks out (or prepares) a statement for `key`.
    pub fn lease(self: &Rc<Self>, key: PoolKey) -> Result<Lease> {
        if let PoolKey::Sql(text) = &key {
            self.touch_sql(text)?;
        }
        let cached = {
            let mut idle = self.idle.try_borrow_mut().map_err(|_| Error::Reentrant)?;
            idle.get_mut(&key).and_then(Vec::pop)
        };
        let stmt = match cached {
            Some(s) => {
                self.bump(|c| c.reused += 1);
                s
            }
            None => {
                let s = RawStmt::prepare(self.db, &key.sql(), key.poolable())?;
                self.bump(|c| c.prepared += 1);
                s
            }
        };
        self.leased.set(self.leased.get() + 1);
        Ok(Lease { pool: self.clone(), key: Some(key), stmt: Some(stmt) })
    }

    pub fn lease_fixed(self: &Rc<Self>, f: Fixed) -> Result<Lease> {
        self.lease(PoolKey::Fixed(f))
    }

    /// Records a use of an SQL text; evicts the least recently used text beyond capacity.
    fn touch_sql(&self, text: &Rc<str>) -> Result<()> {
        let evicted = {
            let mut lru = self.sql_lru.try_borrow_mut().map_err(|_| Error::Reentrant)?;
            lru.tick += 1;
            let tick = lru.tick;
            if let Some(t) = lru.last_use.get_mut(text) {
                *t = tick;
                None
            } else {
                lru.last_use.insert(text.clone(), tick);
                if lru.last_use.len() > SQL_LRU_CAPACITY {
                    let victim = lru.last_use.iter().min_by_key(|(_, t)| **t).map(|(k, _)| k.clone());
                    if let Some(v) = &victim {
                        lru.last_use.remove(v);
                    }
                    victim
                } else {
                    None
                }
            }
        };
        if let Some(v) = evicted {
            let stmts = self.idle.try_borrow_mut().map_err(|_| Error::Reentrant)?.remove(&PoolKey::Sql(v));
            let n = stmts.as_ref().map_or(0, Vec::len);
            drop(stmts);
            self.bump(|c| c.finalized += n as u64);
        }
        Ok(())
    }

    fn check_in(&self, key: PoolKey, mut stmt: RawStmt) {
        self.leased.set(self.leased.get().saturating_sub(1));
        stmt.reset();
        stmt.clear_bindings();
        let keep = self.armed.get()
            && key.poolable()
            && match &key {
                PoolKey::Sql(text) => self.sql_lru.try_borrow().map(|l| l.last_use.contains_key(text)).unwrap_or(false),
                _ => true,
            };
        if keep && let Ok(mut idle) = self.idle.try_borrow_mut() {
            let v = idle.entry(key).or_default();
            if v.len() < MAX_IDLE_PER_KEY {
                v.push(stmt);
                return;
            }
        }
        drop(stmt);
        self.bump(|c| c.finalized += 1);
    }
}

/// A checked-out statement. Dropping it resets it, clears its bindings and returns it to
/// the pool (or finalizes it when the pool is disarmed / full).
pub(crate) struct Lease {
    pool: Rc<StmtPool>,
    key: Option<PoolKey>,
    stmt: Option<RawStmt>,
}

impl Deref for Lease {
    type Target = RawStmt;
    #[inline]
    fn deref(&self) -> &RawStmt {
        self.stmt.as_ref().expect("lease already released")
    }
}

impl DerefMut for Lease {
    #[inline]
    fn deref_mut(&mut self) -> &mut RawStmt {
        self.stmt.as_mut().expect("lease already released")
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        if let (Some(key), Some(stmt)) = (self.key.take(), self.stmt.take()) {
            self.pool.check_in(key, stmt);
        }
    }
}
