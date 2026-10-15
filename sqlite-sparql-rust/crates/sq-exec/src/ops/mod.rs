//! Physical operators (§5.3): pull-based Volcano over 1024-row batches in the query-wide slot
//! layout. One module per family; [`crate::build`] turns a `Plan` into a tree of them.
use sq_core::{Batch, Result, Slot, TermId};

use crate::ctx::ExecCtx;
use crate::seed::Seed;

pub(crate) mod bind_join;
pub(crate) mod filter;
pub(crate) mod graph;
pub(crate) mod group;
pub(crate) mod hash_join;
pub(crate) mod leaf;
pub(crate) mod merge_join;
pub(crate) mod minus;
pub(crate) mod modifiers;
pub(crate) mod nested_loop;
pub(crate) mod path;
pub(crate) mod scan;
pub(crate) mod sort;
pub(crate) mod union;

/// The executor's operator interface (internal to sq-exec).
///
/// Protocol: `open` (re)starts the operator — it may be called again at any time to restart
/// with another seed, without an intervening `close`. `next` appends rows to `out` until it is
/// full (then returns `Ok(true)`) or the operator is exhausted (`Ok(false)`; rows appended by
/// that last call are valid). `fast_count`, when supported, counts the remaining rows of a
/// freshly opened operator without producing them. `close` releases cursors and memory.
pub(crate) trait Operator {
    /// (Re)start. `seed` holds the values bound by the caller: Join-mode seeds (BindJoin
    /// probes) and/or Substitute-mode seeds (EXISTS), see [`crate::seed`].
    fn open(&mut self, cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()>;

    /// Appends rows to `out` until full (`Ok(true)`) or exhausted (`Ok(false)`).
    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool>;

    /// COUNT fast path on a freshly opened operator (Scan -> `store.count`, MergeJoin -> sum of
    /// run products, row-preserving operators -> their input's). None = iterate instead.
    fn fast_count(&mut self, _cx: &mut ExecCtx) -> Option<Result<u64>> {
        None
    }

    /// Galloping support for merge joins: when this operator's remaining output is sorted by
    /// `slot`, skip to the first row whose `slot` is >= `key` and return true; false when
    /// unsupported (nothing changed).
    fn seek(&mut self, _cx: &mut ExecCtx, _slot: Slot, _key: TermId) -> Result<bool> {
        Ok(false)
    }

    fn close(&mut self, cx: &mut ExecCtx);
}

pub(crate) type BoxOp = Box<dyn Operator>;

/// Profiling and interrupt-polling wrapper around every planned operator: counts opens and
/// output rows per `Plan::id`, times `next` calls when timing is enabled, and charges the
/// produced rows to the interrupt-polling counter.
pub(crate) struct Instrumented {
    pub id: u32,
    pub inner: BoxOp,
}

impl Operator for Instrumented {
    fn open(&mut self, cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        cx.sh.prof(Some(self.id), |n| n.opens += 1);
        self.inner.open(cx, seed)
    }

    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        let before = out.len();
        let start = cx.sh.timing.get().then(std::time::Instant::now);
        let r = self.inner.next(cx, out);
        let rows = out.len().saturating_sub(before);
        cx.sh.prof(Some(self.id), |n| {
            n.rows_out += rows as u64;
            if let Some(t) = start {
                n.time_ns += t.elapsed().as_nanos() as u64;
            }
        });
        let more = r?;
        cx.tick(rows)?;
        Ok(more)
    }

    fn fast_count(&mut self, cx: &mut ExecCtx) -> Option<Result<u64>> {
        let start = cx.sh.timing.get().then(std::time::Instant::now);
        let r = self.inner.fast_count(cx);
        if let (Some(Ok(n)), id) = (&r, self.id) {
            cx.sh.prof(Some(id), |p| {
                p.rows_out += *n;
                if let Some(t) = start {
                    p.time_ns += t.elapsed().as_nanos() as u64;
                }
            });
        }
        r
    }

    fn seek(&mut self, cx: &mut ExecCtx, slot: Slot, key: TermId) -> Result<bool> {
        self.inner.seek(cx, slot, key)
    }

    fn close(&mut self, cx: &mut ExecCtx) {
        self.inner.close(cx)
    }
}
