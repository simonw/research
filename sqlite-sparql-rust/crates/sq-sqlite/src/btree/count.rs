//! Range counts on the direct B-tree reader: `QuadStore::count` without decoding the rows.
//!
//! **Why cell counts are enough.** Every entry of an index b-tree is one cell: leaf cells, and
//! interior cells too (an interior cell sits between its left child and the next child in key
//! order). The number of entries of a subtree is therefore the sum of the cell counts of its
//! pages, which SQLite's own `sqlite3BtreeCount` (`OP_Count`) also relies on. That opcode only
//! serves a `count(*)` without a `WHERE` clause; our templates always constrain the index
//! prefix, so the VDBE steps every row of the range instead.
//!
//! **The walk.** Each subtree is visited together with the open key interval its parent's
//! separator cells place it in (`lo < key < hi`, unbounded at the edges of the b-tree). A
//! subtree whose interval lies inside the counted range `[start, end]` contributes every cell
//! of its pages; its leaves are counted from their page headers. Only the pages on the two
//! boundary paths (at most two per level) are binary-searched for the exact first and last
//! entry of the range, and children whose interval lies outside the range are never read. A
//! cap stops the walk as soon as the total exceeds it.
//!
//! **Validation.** Every byte is untrusted, as in the cursor. The interior cells that delimit
//! visited children are decoded and must increase strictly within the parent's interval; the
//! first and last cell of every visited leaf must lie inside its interval too. Sibling
//! intervals are thus disjoint, so a damaged b-tree that links one page twice cannot have it
//! counted twice, and a cycle ends at `MAX_DEPTH`. Rows inside a counted leaf are not decoded:
//! their order and the partial-index predicate (which the cursor re-checks per row) are trusted
//! to the index definition that `roots` verified, exactly as SQLite's own count trusts them.
//!
//! **Fallback.** The count runs only where a cursor would: the same mode, preconditions and
//! verified root pages (`BtreeReader::snapshot`). A page or record the reader does not support,
//! or an inconsistency, abandons the walk and the caller runs the VDBE count instead, which
//! reports the corruption itself. A pending interrupt is noticed every `POLL_PAGES` pages.
use std::cmp::Ordering;
use std::ops::ControlFlow;

use sq_core::store::ScanSpec;
use sq_core::{Error, Result};

use super::Fallback;
use super::cursor::key_range;
use super::format::{self, Cells, Key, MAX_DEPTH, NodeHeader, partition};
use super::reader::{BtreeReader, ReaderMode, Snapshot};

/// Pages read between two interrupt polls.
const POLL_PAGES: u32 = 1024;

/// Why a walk stopped early.
enum Stop {
    /// The reader cannot finish this count: the caller runs the VDBE count.
    Fallback(Fallback),
    /// The interrupt poll failed: the count fails with this error.
    Error(Error),
}

impl From<Fallback> for Stop {
    fn from(f: Fallback) -> Stop {
        Stop::Fallback(f)
    }
}

const fn corrupt(what: &'static str) -> Stop {
    Stop::Fallback(Fallback::Corrupt(what))
}

/// One count: the range, the running total and the cap.
struct Walk<'r> {
    reader: &'r BtreeReader,
    snap: Snapshot,
    start: Key,
    end: Key,
    /// The range has no lower / upper end (`start` / `end` is the smallest / largest key).
    from_first: bool,
    to_last: bool,
    /// The walk stops once `total >= limit` (`cap + 1`; `u64::MAX` for an exact count).
    limit: u64,
    total: u64,
    pages: u32,
    poll: &'r mut dyn FnMut() -> Result<()>,
}

/// An exclusive bound of a subtree's keys, `None` at the edge of the b-tree.
type Fence = Option<Key>;

/// `lo < key < hi`.
fn inside(key: &Key, lo: &Fence, hi: &Fence) -> bool {
    lo.as_ref().is_none_or(|l| format::cmp_keys(key, l) == Ordering::Greater)
        && hi.as_ref().is_none_or(|h| format::cmp_keys(key, h) == Ordering::Less)
}

impl Walk<'_> {
    /// Every key above `lo` is `>= start`.
    fn lo_inside(&self, lo: &Fence) -> bool {
        match lo {
            None => self.from_first,
            Some(l) => format::cmp_keys(&self.start, l) != Ordering::Greater,
        }
    }

    /// Every key below `hi` is `<= end`.
    fn hi_inside(&self, hi: &Fence) -> bool {
        match hi {
            None => self.to_last,
            Some(h) => format::cmp_keys(h, &self.end) != Ordering::Greater,
        }
    }

    /// Adds `n` entries; breaks once the total reaches the limit.
    fn add(&mut self, n: u64) -> ControlFlow<()> {
        self.total = self.total.saturating_add(n);
        if self.total >= self.limit { ControlFlow::Break(()) } else { ControlFlow::Continue(()) }
    }

    /// Adds the entries in `[start, end]` of the subtree rooted at `pgno`, whose keys all lie
    /// strictly between `lo` and `hi`. The parent visits only children whose interval meets
    /// the range.
    fn subtree(&mut self, pgno: u32, lo: Fence, hi: Fence, depth: usize) -> Result<ControlFlow<()>, Stop> {
        if depth >= MAX_DEPTH {
            return Err(corrupt("b-tree deeper than SQLite allows"));
        }
        self.pages += 1;
        if self.pages % POLL_PAGES == 0 {
            (self.poll)().map_err(Stop::Error)?;
        }
        let g = self.snap.geom;
        let page = self.reader.page(&self.snap, pgno)?;
        let hdr = NodeHeader::parse(&page, pgno, &g)?;
        let cells = Cells::new(&page, &hdr, &g)?;
        let n = hdr.cells;
        let (lo_in, hi_in) = (self.lo_inside(&lo), self.hi_inside(&hi));
        if hdr.leaf {
            if n == 0 {
                return Ok(ControlFlow::Continue(()));
            }
            let (first, last) = (cells.key(0)?, cells.key(n - 1)?);
            if !inside(&first, &lo, &hi) || !inside(&last, &lo, &hi) || format::cmp_keys(&first, &last) == Ordering::Greater {
                return Err(corrupt("leaf keys outside the bounds of their parent cells"));
            }
            let a = if lo_in { 0 } else { partition(n, |i| Ok(cells.cmp(i, &self.start)? == Ordering::Less))? };
            let b = if hi_in { n } else { partition(n, |i| Ok(cells.cmp(i, &self.end)? != Ordering::Greater))? };
            return Ok(self.add(b.saturating_sub(a) as u64));
        }
        // Child i lies between cell i-1 and cell i (the parent's bounds at the edges). Children
        // before `a` lie below `start`, children after `b` above `end`; the cells a..b are in
        // the range.
        let a = if lo_in { 0 } else { partition(n, |i| Ok(cells.cmp(i, &self.start)? == Ordering::Less))? };
        let b = if hi_in { n } else { partition(n, |i| Ok(cells.cmp(i, &self.end)? != Ordering::Greater))? };
        let mut left = lo;
        if a > 0 {
            let k = cells.key(a - 1)?;
            if !inside(&k, &lo, &hi) {
                return Err(corrupt("interior keys out of order"));
            }
            left = Some(k);
        }
        for i in a..=b {
            let right = if i < n {
                let k = cells.key(i)?;
                if !inside(&k, &left, &hi) {
                    return Err(corrupt("interior keys out of order"));
                }
                Some(k)
            } else {
                hi
            };
            if self.subtree(cells.child(i)?, left, right, depth + 1)?.is_break() {
                return Ok(ControlFlow::Break(()));
            }
            if i < b && self.add(1).is_break() {
                return Ok(ControlFlow::Break(()));
            }
            left = right;
        }
        Ok(ControlFlow::Continue(()))
    }
}

/// Number of rows of `spec`, by summing cell counts in the order's b-tree: exact when `cap` is
/// `None`, otherwise `min(count, cap + 1)` like `QuadStore::count`. `Ok(None)` when the reader
/// cannot serve the count (switched off, a §13 precondition, an index layout it does not read,
/// or a page it cannot use); the caller then runs the VDBE count. `poll` is the interrupt check.
pub(crate) fn count(
    reader: &BtreeReader,
    spec: &ScanSpec,
    cap: Option<u64>,
    poll: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<u64>> {
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
    let limit = cap.map_or(u64::MAX, |c| c.saturating_add(1));
    let Some((start, end)) = key_range(&spec.prefix[..k], spec.lower, spec.upper) else {
        reader.count(|c| c.counts += 1);
        return Ok(Some(0));
    };
    let mut walk = Walk {
        reader,
        snap,
        start,
        end,
        from_first: start == [i64::MIN; format::KEY_COLS],
        to_last: end == [i64::MAX; format::KEY_COLS],
        limit,
        total: 0,
        pages: 0,
        poll,
    };
    match walk.subtree(root, None, None, 0) {
        Ok(_) => {
            reader.count(|c| c.counts += 1);
            Ok(Some(walk.total.min(limit)))
        }
        Err(Stop::Fallback(why)) => {
            reader.note(why, true);
            Ok(None)
        }
        Err(Stop::Error(e)) => Err(e),
    }
}
