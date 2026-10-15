//! `SqlCursor` (§5.2): a positioned range scan over one index through a leased pooled
//! statement that selects only the free suffix columns, in index order.
//!
//! Positioning:
//! * `rebind` / 1-column `seek`: reset the base statement and rebind `?L` (no allocation);
//! * `seek` with `m >= 2` key columns: the `seek_cols = m` row-value variant;
//! * a forward `seek` from a positioned statement first steps up to `GALLOP` rows (the
//!   galloping threshold G of the SQL path) before re-seeking.
use std::ffi::c_int;
use std::ops::Bound;
use std::rc::Rc;

use sq_core::store::{CursorCounters, QuadCursor, ReaderKind, ScanSpec};
use sq_core::{Error, Result, TermId};

use crate::pool::{Lease, PoolKey, StmtPool};
use crate::template::{LowerOp, StmtKey, StmtKind};

/// Forward seeks within this many rows step instead of re-seeking.
pub(crate) const GALLOP: usize = 6;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum State {
    /// Bound/reset, not stepped since.
    Fresh,
    Running,
    Done,
}

/// Effective inclusive lower bound of the first free column; None = empty range.
fn lower_incl(b: Bound<i64>) -> Option<i64> {
    match b {
        Bound::Unbounded => Some(i64::MIN),
        Bound::Included(v) => Some(v),
        Bound::Excluded(v) => v.checked_add(1),
    }
}

fn upper_incl(b: Bound<i64>) -> Option<i64> {
    match b {
        Bound::Unbounded => Some(i64::MAX),
        Bound::Included(v) => Some(v),
        Bound::Excluded(v) => v.checked_sub(1),
    }
}

pub struct SqlCursor {
    pool: Rc<StmtPool>,
    /// Base scan key (seek_cols 0) for the current bounds.
    key: StmtKey,
    prefix: [i64; 4],
    lower: Bound<i64>,
    upper: Bound<i64>,
    width: usize,
    /// `stmts[0]`: base statement; `stmts[m - 1]`: row-value variant for m = 2..=4 key columns.
    stmts: [Option<Lease>; 4],
    active: usize,
    state: State,
    /// The range is statically empty (contradictory bounds): no statement is stepped.
    empty: bool,
    /// A stepped row not yet returned by `fill` (values in `cur`).
    pending: bool,
    cur: [i64; 4],
    /// Last row the statement moved past (returned, or skipped by a forward seek).
    has_prev: bool,
    prev: [i64; 4],
    counters: CursorCounters,
}

impl SqlCursor {
    pub(crate) fn open(pool: Rc<StmtPool>, spec: &ScanSpec) -> Result<SqlCursor> {
        let mut c = SqlCursor {
            pool,
            key: StmtKey::for_spec(spec, StmtKind::Scan),
            prefix: [0; 4],
            lower: spec.lower,
            upper: spec.upper,
            width: 4 - spec.prefix_len.min(4) as usize,
            stmts: [None, None, None, None],
            active: 0,
            state: State::Fresh,
            empty: false,
            pending: false,
            cur: [0; 4],
            has_prev: false,
            prev: [0; 4],
            counters: CursorCounters { reader: ReaderKind::SqlCursor, ..Default::default() },
        };
        for (dst, src) in c.prefix.iter_mut().zip(&spec.prefix[..spec.prefix_len.min(4) as usize]) {
            *dst = src.0;
        }
        c.position_base(None)?;
        Ok(c)
    }

    fn prefix_len(&self) -> usize {
        4 - self.width
    }

    fn compute_empty(&self) -> bool {
        if self.width == 0 {
            return false;
        }
        match (lower_incl(self.lower), upper_incl(self.upper)) {
            (Some(l), Some(u)) => l > u,
            _ => true,
        }
    }

    /// Leases (once) the statement of slot `i` (0 = base, i = m - 1 for m seek columns).
    fn stmt(&mut self, i: usize) -> Result<&mut Lease> {
        if self.stmts[i].is_none() {
            let key = if i == 0 { self.key } else { self.key.seek_variant(i as u8 + 1) };
            self.stmts[i] = Some(self.pool.lease(PoolKey::Scan(key))?);
        }
        Ok(self.stmts[i].as_mut().expect("leased above"))
    }

    /// Resets the active statement (if it may be mid-step) and switches to slot `i`.
    /// Invariant: every statement other than the active one is reset, and the active one is
    /// reset whenever `state == Fresh`, so after this call slot `i` needs no further reset.
    fn switch_to(&mut self, i: usize) {
        if (self.active != i || self.state != State::Fresh)
            && let Some(s) = self.stmts[self.active].as_mut()
        {
            s.reset();
        }
        self.active = i;
        self.state = State::Fresh;
        self.pending = false;
        self.has_prev = false;
    }

    fn bind_prefix_and_upper(prefix: &[i64], upper: Option<(c_int, i64)>, st: &mut Lease) -> Result<()> {
        for (i, v) in prefix.iter().enumerate() {
            st.bind_i64(i as c_int + 1, *v)?;
        }
        if let Some((idx, v)) = upper {
            st.bind_i64(idx, v)?;
        }
        Ok(())
    }

    fn upper_value(&self) -> i64 {
        match self.upper {
            Bound::Included(v) | Bound::Excluded(v) => v,
            Bound::Unbounded => i64::MAX,
        }
    }

    /// Positions the base statement at the range start, or at `max(lower, key0)` for a
    /// 1-column seek.
    fn position_base(&mut self, key0: Option<i64>) -> Result<()> {
        self.empty = self.compute_empty();
        self.switch_to(0);
        if self.empty {
            self.state = State::Done;
            return Ok(());
        }
        let params = self.key.params();
        let lower_v = match self.lower {
            Bound::Included(v) | Bound::Excluded(v) => v,
            Bound::Unbounded => i64::MIN,
        };
        // Keep the template's operator: with `c > ?L`, `c >= k` is bound as `?L = k - 1`.
        let l = match (key0, self.key.lower) {
            (Some(k), LowerOp::Ge) => lower_v.max(k),
            (Some(k), LowerOp::Gt) if k > lower_v => k - 1,
            _ => lower_v,
        };
        let upper = (params.upper != 0).then(|| (params.upper, self.upper_value()));
        let n = self.prefix_len();
        let prefix = self.prefix;
        let st = self.stmt(0)?;
        Self::bind_prefix_and_upper(&prefix[..n], upper, st)?;
        if params.lower != 0 {
            st.bind_i64(params.lower, l)?;
        }
        Ok(())
    }

    /// Positions the `m`-column row-value variant at `max(key, (lower, MIN, …))`.
    fn position_seek(&mut self, key: &[TermId]) -> Result<()> {
        let m = key.len();
        self.switch_to(m - 1);
        let Some(lo) = lower_incl(self.lower) else {
            self.state = State::Done;
            return Ok(());
        };
        let variant = self.key.seek_variant(m as u8);
        let params = variant.params();
        let upper = (params.upper != 0).then(|| (params.upper, self.upper_value()));
        let n = self.prefix_len();
        let prefix = self.prefix;
        let st = self.stmt(m - 1)?;
        Self::bind_prefix_and_upper(&prefix[..n], upper, st)?;
        let below = key[0].0 < lo;
        for (i, k) in key.iter().enumerate() {
            let v = match (below, i) {
                (false, _) => k.0,
                (true, 0) => lo,
                (true, _) => i64::MIN,
            };
            st.bind_i64(params.seek + i as c_int, v)?;
        }
        Ok(())
    }

    /// Steps the active statement; on a row, loads the free columns into `cur`.
    #[inline]
    fn step_into_cur(&mut self) -> Result<bool> {
        let w = self.width;
        let st = self.stmts[self.active].as_mut().expect("active statement is leased");
        match st.step() {
            Ok(true) => {
                for i in 0..w {
                    self.cur[i] = st.col_i64(i as c_int);
                }
                self.state = State::Running;
                Ok(true)
            }
            Ok(false) => {
                self.state = State::Done;
                Ok(false)
            }
            Err(e) => {
                self.state = State::Done;
                Err(e)
            }
        }
    }
}

/// `a[..k] < key` over signed i64.
#[inline]
fn prefix_lt(a: &[i64; 4], key: &[TermId]) -> bool {
    for (x, k) in a.iter().zip(key) {
        if *x != k.0 {
            return *x < k.0;
        }
    }
    false
}

impl QuadCursor for SqlCursor {
    fn width(&self) -> usize {
        self.width
    }

    fn fill(&mut self, out: &mut Vec<TermId>, max_rows: usize) -> Result<usize> {
        self.counters.fills += 1;
        if max_rows == 0 || (self.state == State::Done && !self.pending) {
            return Ok(0);
        }
        let w = self.width;
        out.reserve(max_rows.min(sq_core::BATCH_ROWS) * w);
        let mut n = 0;
        if self.pending {
            out.extend(self.cur[..w].iter().map(|v| TermId(*v)));
            self.pending = false;
            n = 1;
        }
        if self.state != State::Done {
            let st = self.stmts[self.active].as_mut().expect("active statement is leased");
            while n < max_rows {
                match st.step() {
                    Ok(true) => {
                        for i in 0..w {
                            out.push(TermId(st.col_i64(i as c_int)));
                        }
                        n += 1;
                    }
                    Ok(false) => {
                        self.state = State::Done;
                        break;
                    }
                    Err(e) => {
                        self.state = State::Done;
                        return Err(e);
                    }
                }
            }
            if self.state != State::Done {
                self.state = State::Running;
            }
        }
        if n > 0 {
            let last = &out[out.len() - w..];
            for (dst, src) in self.prev.iter_mut().zip(last) {
                *dst = src.0;
            }
            self.has_prev = true;
        }
        self.counters.rows += n as u64;
        Ok(n)
    }

    fn seek(&mut self, key: &[TermId]) -> Result<()> {
        self.counters.seeks += 1;
        if key.len() > self.width {
            return Err(Error::Invalid(format!("seek key of {} columns on a cursor of width {}", key.len(), self.width)));
        }
        if key.is_empty() {
            return self.position_base(None);
        }
        if self.empty {
            return Ok(());
        }
        // Forward fast path: every row the statement has moved past is < key.
        if self.state != State::Fresh && self.has_prev && prefix_lt(&self.prev, key) {
            if self.pending {
                if !prefix_lt(&self.cur, key) {
                    return Ok(());
                }
                self.prev = self.cur;
                self.pending = false;
            }
            if self.state == State::Done {
                return Ok(());
            }
            for _ in 0..GALLOP {
                if !self.step_into_cur()? {
                    return Ok(());
                }
                if !prefix_lt(&self.cur, key) {
                    self.pending = true;
                    return Ok(());
                }
                self.prev = self.cur;
            }
        }
        if key.len() == 1 { self.position_base(Some(key[0].0)) } else { self.position_seek(key) }
    }

    fn rebind(&mut self, prefix: &[TermId], lower: Bound<i64>, upper: Bound<i64>) -> Result<()> {
        if prefix.len() != self.prefix_len() {
            return Err(Error::Invalid(format!("rebind with {} prefix values, cursor has {}", prefix.len(), self.prefix_len())));
        }
        for (dst, src) in self.prefix.iter_mut().zip(prefix) {
            *dst = src.0;
        }
        self.lower = lower;
        self.upper = upper;
        let spec_like = ScanSpec { order: self.key.order, prefix: [TermId::UNBOUND; 4], prefix_len: self.key.prefix_len, lower, upper };
        let key = StmtKey::for_spec(&spec_like, StmtKind::Scan);
        if key != self.key {
            // Different bound operators: give the old statements back, lease lazily.
            self.switch_to(0);
            self.stmts = [None, None, None, None];
            self.key = key;
        }
        self.position_base(None)
    }

    fn counters(&self) -> CursorCounters {
        self.counters
    }
}

impl std::fmt::Debug for SqlCursor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqlCursor").field("key", &self.key).field("state", &self.state).finish()
    }
}

/// A cursor over generated read-only SQL whose `width` result columns are TermIds (the
/// SqlJoin hybrid). NULL columns read as UNBOUND (0). `seek` / `rebind` are unsupported.
pub struct SqlRowsCursor {
    stmt: Lease,
    width: usize,
    done: bool,
    counters: CursorCounters,
}

impl SqlRowsCursor {
    pub(crate) fn open(pool: &Rc<StmtPool>, sql: &str, params: &[i64], width: usize) -> Result<SqlRowsCursor> {
        let mut stmt = pool.lease(PoolKey::Sql(Rc::from(sql)))?;
        if stmt.column_count() != width {
            return Err(Error::Invalid(format!("sql_rows: statement returns {} columns, expected {width}", stmt.column_count())));
        }
        for (i, p) in params.iter().enumerate() {
            stmt.bind_i64(i as c_int + 1, *p)?;
        }
        Ok(SqlRowsCursor { stmt, width, done: false, counters: CursorCounters { reader: ReaderKind::SqlCursor, ..Default::default() } })
    }
}

impl QuadCursor for SqlRowsCursor {
    fn width(&self) -> usize {
        self.width
    }

    fn fill(&mut self, out: &mut Vec<TermId>, max_rows: usize) -> Result<usize> {
        self.counters.fills += 1;
        if self.done {
            return Ok(0);
        }
        let w = self.width;
        out.reserve(max_rows.min(sq_core::BATCH_ROWS) * w);
        let mut n = 0;
        while n < max_rows {
            match self.stmt.step() {
                Ok(true) => {
                    for i in 0..w {
                        out.push(TermId(self.stmt.col_i64(i as c_int)));
                    }
                    n += 1;
                }
                Ok(false) => {
                    self.done = true;
                    break;
                }
                Err(e) => {
                    self.done = true;
                    return Err(e);
                }
            }
        }
        self.counters.rows += n as u64;
        Ok(n)
    }

    fn seek(&mut self, _key: &[TermId]) -> Result<()> {
        Err(Error::Unsupported("seek on a sql_rows cursor".into()))
    }

    fn rebind(&mut self, _prefix: &[TermId], _lower: Bound<i64>, _upper: Bound<i64>) -> Result<()> {
        Err(Error::Unsupported("rebind on a sql_rows cursor".into()))
    }

    fn counters(&self) -> CursorCounters {
        self.counters
    }
}

/// Cursor of a store whose RDF tables do not exist yet (queries see an empty dataset).
pub(crate) struct EmptyCursor {
    pub width: usize,
}

impl QuadCursor for EmptyCursor {
    fn width(&self) -> usize {
        self.width
    }
    fn fill(&mut self, _out: &mut Vec<TermId>, _max_rows: usize) -> Result<usize> {
        Ok(0)
    }
    fn seek(&mut self, _key: &[TermId]) -> Result<()> {
        Ok(())
    }
    fn rebind(&mut self, _prefix: &[TermId], _lower: Bound<i64>, _upper: Bound<i64>) -> Result<()> {
        Ok(())
    }
    fn counters(&self) -> CursorCounters {
        CursorCounters { reader: ReaderKind::SqlCursor, ..Default::default() }
    }
}

/// Binds the parameters of a count template for `spec`; returns false if the range is
/// statically empty.
pub(crate) fn bind_count(st: &mut Lease, key: &StmtKey, spec: &ScanSpec, cap: Option<u64>) -> Result<bool> {
    let k = spec.prefix_len.min(4) as usize;
    for i in 0..k {
        st.bind_i64(i as c_int + 1, spec.prefix[i].0)?;
    }
    let p = key.params();
    if k < 4 {
        match (lower_incl(spec.lower), upper_incl(spec.upper)) {
            (Some(l), Some(u)) if l <= u => {}
            _ => return Ok(false),
        }
        if p.lower != 0 {
            let v = match spec.lower {
                Bound::Included(v) | Bound::Excluded(v) => v,
                Bound::Unbounded => i64::MIN,
            };
            st.bind_i64(p.lower, v)?;
        }
        if p.upper != 0 {
            let v = match spec.upper {
                Bound::Included(v) | Bound::Excluded(v) => v,
                Bound::Unbounded => i64::MAX,
            };
            st.bind_i64(p.upper, v)?;
        }
    }
    if p.limit != 0 {
        let limit = cap.unwrap_or(u64::MAX).saturating_add(1).min(i64::MAX as u64) as i64;
        st.bind_i64(p.limit, limit)?;
    }
    Ok(true)
}
