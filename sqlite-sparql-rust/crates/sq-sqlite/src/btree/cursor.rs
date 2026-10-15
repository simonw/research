//! [`BtreeCursor`]: a `QuadCursor` that walks the covering-index b-tree of one order directly.
//!
//! **Position.** `path` holds the pages from the root to the position, like SQLite's own
//! `apPage[]`/`aiIdx[]`. Each level pins its page (`Arc<[u8]>`), so a scan reads its current
//! leaf without any page-cache lookup per row. The interior cells of an index b-tree are
//! entries as well: an in-order walk emits child 0, then cell 0, child 1, cell 1, …, and the
//! right-most child last. The top of the path is the next entry to read. That is a leaf cell,
//! or an interior cell reached after its left subtree is exhausted. An empty path means the
//! end of the b-tree.
//!
//! **Watermark.** `wm` records how far the scan got. The position is the first entry `>= wm`
//! (`wm_incl`), or the entry just returned, which equals `wm`, when `consumed`. Every row
//! that is read must lie above the watermark (or on it, right after a repositioning). That
//! one comparison catches keys out of order, so a corrupt or cyclic b-tree cannot produce
//! duplicate rows or loop forever. It also lets every repositioning to a target `t >= wm`
//! (forward seeks, and `rebind`s to later prefixes, which are the sorted probes of
//! index-nested-loop joins) start from the current position instead of the root: a **finger
//! search**. That search walks up only as far as the first ancestor whose separator key bounds
//! `t`, then gallops forward. A target below the watermark (a repeated probe of the same key)
//! is searched within the pinned leaf when the leaf's first key bounds it.
//!
//! **Preconditions and fallback.** Every entry point (`fill`, `seek`, `rebind`) first re-checks
//! the §13 preconditions against the cursor's snapshot. That is two `sqlite3_*` calls, and it
//! covers every leaf refill, since nothing outside the cursor can run between two pages of
//! one call. When a check fails, or a page or record turns out to be unsupported or
//! inconsistent, the cursor re-opens as a `SqlCursor` over the same spec. That cursor is
//! positioned by the row-value resume template just after the last returned row (or at the
//! last seek target). The caller sees the rows continue exactly where they stopped. A later
//! `rebind` returns to the reader once a transient precondition holds again.
use std::cmp::Ordering;
use std::ops::Bound;
use std::rc::Rc;
use std::sync::Arc;

use sq_core::store::{Col, CursorCounters, IndexOrder, QuadCursor, ReaderKind, ScanSpec};
use sq_core::{Error, Result, TermId};

use super::Fallback;
use super::format::{self, Geometry, KEY_COLS, Key, MAX_DEPTH, NodeHeader};
use super::reader::{BtreeReader, ReaderMode, Snapshot};
use crate::cursor::{SqlCursor, lower_incl, upper_incl};
use crate::pool::StmtPool;

/// One level of the root-to-position path.
struct Node {
    page: Arc<[u8]>,
    hdr: NodeHeader,
    /// Leaf: the cell at the position. Interior: the child the position lies in, or the
    /// cell at the position when this is the top level.
    idx: usize,
}

impl Node {
    /// `key(i)` compared with `t` (decoding only up to the first differing column).
    #[inline]
    fn cmp(&self, i: usize, t: &Key, g: &Geometry) -> Result<Ordering, Fallback> {
        format::cell_cmp(&self.page, &self.hdr, i, g, t)
    }

    fn child(&self, i: usize, g: &Geometry) -> Result<u32, Fallback> {
        format::child(&self.page, &self.hdr, i, g)
    }

    /// First cell in `[lo, hi)` whose key is `>= t`, given that every cell before `lo` is `< t`.
    fn binary_search(&self, t: &Key, mut lo: usize, mut hi: usize, g: &Geometry) -> Result<usize, Fallback> {
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if self.cmp(mid, t, g)? == Ordering::Less {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        Ok(lo)
    }

    /// Like `binary_search` over `[lo, cells)`, galloping from `lo` first: forward seeks
    /// usually land a few cells ahead.
    fn gallop(&self, t: &Key, mut lo: usize, g: &Geometry) -> Result<usize, Fallback> {
        let n = self.hdr.cells;
        let mut step = 1;
        loop {
            let probe = lo + step - 1;
            if probe >= n {
                return self.binary_search(t, lo, n, g);
            }
            if self.cmp(probe, t, g)? != Ordering::Less {
                return self.binary_search(t, lo, probe, g);
            }
            lo = probe + 1;
            step *= 2;
        }
    }
}

/// The partial-index predicate of an order, re-checked on every row. The index definitions
/// are verified when the root pages are loaded, so a violation means the b-tree is not what it
/// should be.
#[derive(Copy, Clone)]
enum RowFilter {
    All,
    /// Named graphs only (`g <> 0`); `g` is the record position of the graph column.
    NamedOnly { g: usize },
    /// Objects tagged TYPED (5) or DECIMAL (8), `rdf_gpox`.
    NumericExceptions { o: usize },
}

impl RowFilter {
    fn new(order: IndexOrder) -> RowFilter {
        let pos = |c: Col| order.cols().iter().position(|x| *x == c).unwrap_or(0);
        if order.named_only() {
            RowFilter::NamedOnly { g: pos(Col::G) }
        } else if order.numeric_exceptions() {
            RowFilter::NumericExceptions { o: pos(Col::O) }
        } else {
            RowFilter::All
        }
    }

    #[inline]
    fn check(self, key: &Key) -> Result<(), Fallback> {
        let ok = match self {
            RowFilter::All => true,
            RowFilter::NamedOnly { g } => key[g] != 0,
            RowFilter::NumericExceptions { o } => matches!(key[o] & 15, 5 | 8),
        };
        if ok { Ok(()) } else { Err(Fallback::Unsupported("index row outside the partial-index predicate")) }
    }
}

/// How `reposition` reaches its target.
#[derive(Copy, Clone)]
enum Route {
    Forward,
    Backward,
    Root,
}

/// The SQL cursor a scan continues on after a fallback.
struct SqlFallback {
    cur: SqlCursor,
    /// No row can follow the resume key (it is the largest possible key).
    exhausted: bool,
}

pub struct BtreeCursor {
    reader: Rc<BtreeReader>,
    pool: Rc<StmtPool>,
    order: IndexOrder,
    /// Prefix length k; the width is `4 - k`.
    k: usize,
    prefix: [TermId; 4],
    lower: Bound<i64>,
    upper: Bound<i64>,
    /// Smallest and largest key of the range: prefix, then the bounds on column k, then
    /// MIN / MAX. `empty` when the bounds contradict each other.
    start: Key,
    end: Key,
    empty: bool,
    filter: RowFilter,
    snap: Snapshot,
    root: u32,
    path: Vec<Node>,
    /// `path` describes a position in the current snapshot.
    positioned: bool,
    /// The entry at the top of `path` has been returned: advance before reading.
    consumed: bool,
    /// The range ended: the entry at the top of `path` (not returned) lies beyond `end`, or
    /// the b-tree is exhausted.
    done: bool,
    wm: Key,
    wm_incl: bool,
    sql: Option<SqlFallback>,
    /// The last fallback was structural (unsupported or corrupt): never return to the reader.
    sticky: bool,
    /// A fallback could not open its SQL cursor (`Some(true)`: interrupted). Like a failed
    /// `SqlCursor`, `fill` reports an error, never a silent end of the scan, until a `seek` or
    /// `rebind` positions the cursor again.
    failed: Option<bool>,
    counters: CursorCounters,
}

impl BtreeCursor {
    /// A B-tree cursor positioned at the start of `spec`. `None` when the reader is switched
    /// off, its preconditions do not hold, or the order's b-tree cannot be read directly. The
    /// caller then uses a `SqlCursor`.
    pub(crate) fn open(reader: &Rc<BtreeReader>, pool: &Rc<StmtPool>, spec: &ScanSpec) -> Result<Option<BtreeCursor>> {
        if reader.mode() == ReaderMode::Sql {
            return Ok(None);
        }
        let snap = match reader.snapshot() {
            Ok(s) => s,
            Err(why) => {
                reader.note(why, false);
                return Ok(None);
            }
        };
        let root = snap.roots.get(spec.order);
        if root == 0 {
            reader.note(Fallback::Unsupported("the index b-tree does not have the expected layout"), false);
            return Ok(None);
        }
        let k = usize::from(spec.prefix_len.min(4));
        let mut c = BtreeCursor {
            reader: reader.clone(),
            pool: pool.clone(),
            order: spec.order,
            k,
            prefix: [TermId::UNBOUND; 4],
            lower: Bound::Unbounded,
            upper: Bound::Unbounded,
            start: [i64::MIN; KEY_COLS],
            end: [i64::MAX; KEY_COLS],
            empty: false,
            filter: RowFilter::new(spec.order),
            snap,
            root,
            path: Vec::with_capacity(MAX_DEPTH),
            positioned: false,
            consumed: false,
            done: false,
            wm: [i64::MIN; KEY_COLS],
            wm_incl: true,
            sql: None,
            sticky: false,
            failed: None,
            counters: CursorCounters { reader: ReaderKind::Btree, ..Default::default() },
        };
        c.set_range(&spec.prefix[..k], spec.lower, spec.upper);
        reader.count(|c| c.cursors += 1);
        if c.empty {
            c.done = true;
        } else {
            c.reposition(c.start)?;
        }
        Ok(Some(c))
    }

    fn width(&self) -> usize {
        KEY_COLS - self.k
    }

    /// Levels of the current root-to-position path.
    #[cfg(test)]
    pub(crate) fn depth(&self) -> usize {
        self.path.len()
    }

    fn spec(&self) -> ScanSpec {
        ScanSpec { order: self.order, prefix: self.prefix, prefix_len: self.k as u8, lower: self.lower, upper: self.upper }
    }

    /// Installs a new prefix and bounds and derives the key range.
    fn set_range(&mut self, prefix: &[TermId], lower: Bound<i64>, upper: Bound<i64>) {
        let k = self.k;
        self.prefix = [TermId::UNBOUND; 4];
        self.prefix[..k].copy_from_slice(&prefix[..k]);
        self.lower = lower;
        self.upper = upper;
        match key_range(&prefix[..k], lower, upper) {
            Some((start, end)) => {
                self.start = start;
                self.end = end;
                self.empty = false;
            }
            None => self.empty = true,
        }
    }

    // ------------------------------------------------------------------ positioning

    /// Positions the reader at the first entry `>= t` (`start <= t`). When the snapshot is
    /// unchanged it searches from the current position: forward when `t` lies at or beyond the
    /// watermark, backward within the pinned leaf otherwise (repeated probes of one key).
    /// Otherwise it descends from the root, re-validating the snapshot first if needed. Falls
    /// back to SQL when the reader cannot serve.
    fn reposition(&mut self, t: Key) -> Result<()> {
        debug_assert!(self.sql.is_none());
        let route = match self.reader.check(&self.snap) {
            Ok(()) if !self.positioned => Route::Root,
            Ok(()) if t > self.wm || (t == self.wm && self.wm_incl) => Route::Forward,
            Ok(()) => Route::Backward,
            Err(_) => match self.reader.snapshot() {
                Ok(snap) => {
                    self.snap = snap;
                    self.root = snap.roots.get(self.order);
                    self.positioned = false;
                    if self.root == 0 {
                        self.set_watermark(t);
                        return self.fall_back(Fallback::Unsupported("the index b-tree no longer has the expected layout"));
                    }
                    Route::Root
                }
                Err(why) => {
                    self.set_watermark(t);
                    return self.fall_back(why);
                }
            },
        };
        self.set_watermark(t);
        let moved = match route {
            Route::Forward => self.finger(&t),
            Route::Backward => self.finger_back(&t),
            Route::Root => self.descend_from_root(&t),
        };
        match moved {
            Ok(()) => Ok(()),
            Err(why) => self.fall_back(why),
        }
    }

    fn set_watermark(&mut self, t: Key) {
        self.wm = t;
        self.wm_incl = true;
        self.done = false;
    }

    fn descend_from_root(&mut self, t: &Key) -> Result<(), Fallback> {
        self.consumed = false;
        if self.positioned && !self.path.is_empty() {
            // The pinned root of the current snapshot.
            self.path.truncate(1);
        } else {
            self.path.clear();
            self.positioned = false;
            self.push(self.root)?;
            self.positioned = true;
        }
        self.descend(t)
    }

    /// Descends from the page at the top of `path` to the first entry `>= t` in its subtree;
    /// when the subtree has none, the position moves up to the next entry above it.
    fn descend(&mut self, t: &Key) -> Result<(), Fallback> {
        let g = self.snap.geom;
        loop {
            let node = self.path.last_mut().ok_or(Fallback::Corrupt("empty b-tree path"))?;
            let n = node.hdr.cells;
            let c = node.binary_search(t, 0, n, &g)?;
            node.idx = c;
            if node.hdr.leaf {
                if c == n {
                    self.pop_exhausted();
                }
                return Ok(());
            }
            if c < n && node.cmp(c, t, &g)? == Ordering::Equal {
                return Ok(()); // the interior cell itself
            }
            let child = node.child(c, &g)?;
            self.push(child)?;
        }
    }

    /// Forward finger search to the first entry `>= t`, given that the position is the first
    /// entry `>= wm` (or the returned entry equal to `wm`) and `t >= wm`.
    fn finger(&mut self, t: &Key) -> Result<(), Fallback> {
        let g = self.snap.geom;
        let Some(top) = self.path.last() else { return Ok(()) }; // end of the b-tree
        if !self.consumed && top.cmp(top.idx, t, &g)? != Ordering::Less {
            return Ok(());
        }
        // The deepest level whose subtree is bounded above by a separator `>= t`: the cell
        // of its parent at the parent's position (inherited when the parent's position is its
        // right-most child). The root is bounded by +infinity.
        let h = self.path.len() - 1;
        let mut level = h;
        let mut j = h;
        while j > 0 {
            let parent = &self.path[j - 1];
            if parent.idx < parent.hdr.cells {
                if parent.cmp(parent.idx, t, &g)? != Ordering::Less {
                    break;
                }
                level = j - 1;
            }
            j -= 1;
        }
        self.path.truncate(level + 1);
        self.consumed = false;
        let node = &mut self.path[level];
        // Every entry of this page up to and including the cell at `idx` is `< t`: on the top
        // level that cell is the position (checked above); higher up it is the separator `t`
        // exceeds. Near targets are found by galloping; when the search climbed to the root,
        // the target is far away and a plain binary search is cheaper.
        let c = if level == 0 && h > 0 {
            node.binary_search(t, node.idx + 1, node.hdr.cells, &g)?
        } else {
            node.gallop(t, node.idx + 1, &g)?
        };
        node.idx = c;
        let n = node.hdr.cells;
        if node.hdr.leaf {
            if c == n {
                self.pop_exhausted();
            }
            return Ok(());
        }
        if c < n && node.cmp(c, t, &g)? == Ordering::Equal {
            return Ok(());
        }
        let child = node.child(c, &g)?;
        self.push(child)?;
        self.descend(t)
    }

    /// Backward repositioning (`t` below the watermark, hence below the entry at the position):
    /// stays in the pinned leaf when its first cell is `<= t`, since every earlier entry is
    /// smaller; otherwise descends from the root.
    fn finger_back(&mut self, t: &Key) -> Result<(), Fallback> {
        let g = self.snap.geom;
        if let Some(top) = self.path.last_mut()
            && top.hdr.leaf
            && top.hdr.cells > 0
            && top.cmp(0, t, &g)? != Ordering::Greater
        {
            let c = top.binary_search(t, 0, (top.idx + 1).min(top.hdr.cells), &g)?;
            top.idx = c;
            self.consumed = false;
            if c == top.hdr.cells {
                self.pop_exhausted();
            }
            return Ok(());
        }
        self.descend_from_root(t)
    }

    /// Loads page `pgno` and pushes it as the new top level.
    fn push(&mut self, pgno: u32) -> Result<(), Fallback> {
        if self.path.len() >= MAX_DEPTH {
            return Err(Fallback::Corrupt("b-tree deeper than SQLite allows"));
        }
        let page = self.reader.page(&self.snap, pgno)?;
        let hdr = NodeHeader::parse(&page, pgno, &self.snap.geom)?;
        self.path.push(Node { page, hdr, idx: 0 });
        Ok(())
    }

    /// The top leaf is exhausted: move up to the next interior cell, or to the end.
    fn pop_exhausted(&mut self) {
        self.path.pop();
        while let Some(top) = self.path.last() {
            if top.idx < top.hdr.cells {
                return;
            }
            self.path.pop();
        }
    }

    /// Moves past the returned entry at the top of `path` (a leaf refill when it was the
    /// last cell of its leaf).
    fn advance(&mut self) -> Result<(), Fallback> {
        self.consumed = false;
        let g = self.snap.geom;
        let Some(top) = self.path.last_mut() else { return Ok(()) };
        top.idx += 1;
        if top.hdr.leaf {
            if top.idx >= top.hdr.cells {
                self.pop_exhausted();
            }
            return Ok(());
        }
        // After an interior cell comes the leftmost entry of the next child's subtree.
        let mut child = top.child(top.idx, &g)?;
        loop {
            self.push(child)?;
            let node = self.path.last_mut().ok_or(Fallback::Corrupt("empty b-tree path"))?;
            node.idx = 0;
            if node.hdr.leaf {
                if node.hdr.cells == 0 {
                    self.pop_exhausted();
                }
                return Ok(());
            }
            child = node.child(0, &g)?;
        }
    }

    // ------------------------------------------------------------------ scanning

    /// Appends rows until `max` rows (counted in `n`) or the end of the range.
    fn fill_btree(&mut self, out: &mut Vec<TermId>, max: usize, n: &mut usize) -> Result<(), Fallback> {
        let g = self.snap.geom;
        let (k, end, filter) = (self.k, self.end, self.filter);
        while *n < max {
            if self.consumed {
                self.advance()?;
            }
            let Some(top) = self.path.last_mut() else {
                self.done = true;
                return Ok(());
            };
            // Leaf: every remaining cell of the pinned page in one tight loop. Interior: the
            // single cell at the position.
            let cells = format::Cells::new(&top.page, &top.hdr, &g)?;
            loop {
                let key = cells.key(top.idx)?;
                match format::cmp_keys(&key, &self.wm) {
                    Ordering::Greater => {}
                    Ordering::Equal if self.wm_incl => {}
                    _ => return Err(Fallback::Corrupt("index keys out of order")),
                }
                if format::cmp_keys(&key, &end) == Ordering::Greater {
                    self.done = true;
                    return Ok(());
                }
                filter.check(&key)?;
                // Only the free suffix columns are output (capacity is reserved by `fill`).
                for &v in &key[k..] {
                    out.push(TermId(v));
                }
                self.wm = key;
                self.wm_incl = false;
                *n += 1;
                if !top.hdr.leaf || *n == max || top.idx + 1 >= top.hdr.cells {
                    self.consumed = true;
                    break;
                }
                top.idx += 1;
            }
        }
        Ok(())
    }

    // ------------------------------------------------------------------ fallback

    /// Continues the scan as a `SqlCursor` positioned at the watermark: after the last
    /// returned row, or at the last seek target.
    fn fall_back(&mut self, why: Fallback) -> Result<()> {
        self.reader.note(why, true);
        self.sticky = !why.is_transient();
        self.path.clear();
        self.positioned = false;
        self.consumed = false;
        match self.open_sql() {
            Ok(sql) => {
                self.counters.reader = ReaderKind::SqlCursor;
                self.sql = Some(sql);
                Ok(())
            }
            Err(e) => {
                self.failed = Some(matches!(e, Error::Interrupted));
                Err(e)
            }
        }
    }

    /// A `SqlCursor` over the spec, positioned at the watermark.
    fn open_sql(&self) -> Result<SqlFallback> {
        let mut cur = SqlCursor::open(self.pool.clone(), &self.spec())?;
        let mut exhausted = false;
        let (k, w) = (self.k, self.width());
        if w == 0 {
            // The single possible row was already returned.
            exhausted = !self.wm_incl;
        } else {
            let mut key = [TermId::UNBOUND; 4];
            for (dst, src) in key.iter_mut().zip(&self.wm[k..]) {
                *dst = TermId(*src);
            }
            if !self.wm_incl {
                match successor(&mut key[..w]) {
                    Some(len) => cur.seek(&key[..len])?,
                    None => exhausted = true,
                }
            } else if self.wm != self.start {
                cur.seek(&key[..w])?;
            }
        }
        Ok(SqlFallback { cur, exhausted })
    }

    /// After a fallback, a `rebind` returns to the reader when the fallback was transient and
    /// the preconditions hold again.
    fn try_resume(&mut self) {
        if self.sticky {
            return;
        }
        // `snapshot` also refuses while the reader is switched off (`reader = sql`).
        let Ok(snap) = self.reader.snapshot() else { return };
        let root = snap.roots.get(self.order);
        if root == 0 {
            return;
        }
        self.snap = snap;
        self.root = root;
        self.positioned = false;
        self.sql = None;
        self.counters.reader = ReaderKind::Btree;
        self.reader.count(|c| c.resumes += 1);
    }
}

/// The smallest and largest key of a scan range: the prefix, then the bounds on the first free
/// column, then MIN / MAX. `None` when the bounds contradict each other (an empty range).
pub(super) fn key_range(prefix: &[TermId], lower: Bound<i64>, upper: Bound<i64>) -> Option<(Key, Key)> {
    let k = prefix.len().min(KEY_COLS);
    let mut start = [i64::MIN; KEY_COLS];
    let mut end = [i64::MAX; KEY_COLS];
    for (i, p) in prefix[..k].iter().enumerate() {
        start[i] = p.0;
        end[i] = p.0;
    }
    if k < KEY_COLS {
        match (lower_incl(lower), upper_incl(upper)) {
            (Some(l), Some(u)) if l <= u => {
                start[k] = l;
                end[k] = u;
            }
            _ => return None,
        }
    }
    Some((start, end))
}

/// Smallest key greater than every row starting with `key`'s columns. Returns the length of
/// the seek key, or `None` when no such key exists (every column is `i64::MAX`).
fn successor(key: &mut [TermId]) -> Option<usize> {
    for j in (0..key.len()).rev() {
        if key[j].0 != i64::MAX {
            key[j].0 += 1;
            return Some(j + 1);
        }
    }
    None
}

impl QuadCursor for BtreeCursor {
    fn width(&self) -> usize {
        BtreeCursor::width(self)
    }

    fn fill(&mut self, out: &mut Vec<TermId>, max_rows: usize) -> Result<usize> {
        self.counters.fills += 1;
        if let Some(interrupted) = self.failed {
            return Err(if interrupted {
                Error::Interrupted
            } else {
                Error::Invalid("BtreeCursor::fill after a failed fallback: reposition the cursor first".into())
            });
        }
        let mut n = 0;
        if self.sql.is_none() {
            if max_rows == 0 || self.done || self.empty {
                return Ok(0);
            }
            let checked = self.reader.check(&self.snap);
            let r = match checked {
                Ok(()) => {
                    out.reserve(max_rows.min(sq_core::BATCH_ROWS) * self.width());
                    self.fill_btree(out, max_rows, &mut n)
                }
                Err(why) => Err(why),
            };
            if let Err(why) = r {
                self.fall_back(why)?;
            }
        }
        if let Some(sql) = &mut self.sql
            && !sql.exhausted
            && n < max_rows
        {
            n += sql.cur.fill(out, max_rows - n)?;
        }
        self.counters.rows += n as u64;
        Ok(n)
    }

    fn seek(&mut self, key: &[TermId]) -> Result<()> {
        self.counters.seeks += 1;
        let (k, w) = (self.k, self.width());
        if key.len() > w {
            return Err(Error::Invalid(format!("seek key of {} columns on a cursor of width {w}", key.len())));
        }
        self.failed = None;
        if let Some(sql) = &mut self.sql {
            sql.exhausted = false;
            return sql.cur.seek(key);
        }
        if self.empty {
            return Ok(());
        }
        let mut t = [i64::MIN; KEY_COLS];
        t[..k].copy_from_slice(&self.start[..k]);
        for (dst, src) in t[k..].iter_mut().zip(key) {
            *dst = src.0;
        }
        self.reposition(t.max(self.start))
    }

    fn rebind(&mut self, prefix: &[TermId], lower: Bound<i64>, upper: Bound<i64>) -> Result<()> {
        if prefix.len() != self.k {
            return Err(Error::Invalid(format!("rebind with {} prefix values, cursor has {}", prefix.len(), self.k)));
        }
        self.set_range(prefix, lower, upper);
        self.failed = None;
        if self.sql.is_some() {
            self.try_resume();
        }
        if let Some(sql) = &mut self.sql {
            sql.exhausted = false;
            return sql.cur.rebind(prefix, lower, upper);
        }
        if self.empty {
            // Keep the position (and watermark) for later forward repositioning.
            self.done = true;
            return Ok(());
        }
        self.reposition(self.start)
    }

    fn counters(&self) -> CursorCounters {
        self.counters
    }
}

impl std::fmt::Debug for BtreeCursor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BtreeCursor")
            .field("spec", &self.spec())
            .field("depth", &self.path.len())
            .field("done", &self.done)
            .field("sql", &self.sql.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successors() {
        let mut k = [TermId(5), TermId(i64::MAX)];
        assert_eq!(successor(&mut k), Some(1));
        assert_eq!(k[0], TermId(6));
        let mut k = [TermId(-1), TermId(7)];
        assert_eq!(successor(&mut k), Some(2));
        assert_eq!(k, [TermId(-1), TermId(8)]);
        let mut k = [TermId(i64::MAX); 3];
        assert_eq!(successor(&mut k), None);
    }

    #[test]
    fn row_filters() {
        let f = RowFilter::new(IndexOrder::Psog);
        assert!(f.check(&[1, 2, 3, 0]).is_err() && f.check(&[1, 2, 3, 17]).is_ok());
        let f = RowFilter::new(IndexOrder::Gpox);
        assert!(f.check(&[0, 1, 0x15, 2]).is_ok() && f.check(&[0, 1, 0x18, 2]).is_ok());
        assert!(f.check(&[0, 1, 0x16, 2]).is_err());
        assert!(RowFilter::new(IndexOrder::Gspo).check(&[0, 0, 0, 0]).is_ok());
    }
}
