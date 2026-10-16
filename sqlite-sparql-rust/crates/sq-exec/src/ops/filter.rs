//! Filter (EBV; an error counts as false) and Extend (an error leaves the slot unbound). Both
//! work in place on the rows their input appends to the output batch.
use sq_core::{Batch, Result, Slot, TermId};
use sq_expr::CompiledExpr;

use super::{BoxOp, Operator};
use crate::ctx::ExecCtx;
use crate::rows::Pull;
use crate::seed::{Seed, SeedBuf};

pub(crate) struct Filter {
    input: BoxOp,
    expr: CompiledExpr,
    /// Slots whose strings the expression decodes on every row (`CompiledExpr::string_slots`):
    /// each input batch decodes them in one store call before its rows are evaluated.
    prefetch: Vec<Slot>,
    seed: SeedBuf,
    pull: Pull,
}

impl Filter {
    pub fn new(input: BoxOp, expr: CompiledExpr, width: usize) -> Self {
        let prefetch = expr.string_slots();
        Filter { input, expr, prefetch, seed: SeedBuf::new(width), pull: Pull::new(width) }
    }
}

/// Decodes the strings of `slots` in the rows `from..` of `rows` ahead of their row-by-row
/// evaluation.
fn prefetch(cx: &ExecCtx, rows: &Batch, from: usize, slots: &[Slot]) -> Result<()> {
    if slots.is_empty() || rows.len() <= from {
        return Ok(());
    }
    cx.sh.terms.borrow_mut().prefetch(cx.store(), rows, from, slots)
}

impl Operator for Filter {
    fn open(&mut self, cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        self.pull.reset();
        self.input.open(cx, self.seed.full())
    }

    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        let (expr, slots) = (&self.expr, &self.prefetch);
        self.pull.pump_with(
            cx,
            self.input.as_mut(),
            out,
            |cx, rows| prefetch(cx, rows, 0, slots),
            |cx, i, row| {
                if !slots.is_empty() {
                    cx.sh.terms.borrow_mut().at_row(i);
                }
                Ok(cx.with_eval(|ecx| expr.ebv(row, ecx))? == Some(true))
            },
        )
    }

    fn reset(&mut self, cx: &mut ExecCtx) {
        self.input.reset(cx)
    }

    fn close(&mut self, cx: &mut ExecCtx) {
        self.input.close(cx)
    }
}

/// BIND / SELECT expressions. `canonical` selects canonical ids for the result (§7.3.8);
/// `error_id` is what an error writes: UNBOUND, or `TermId::ERROR` for the key of a GROUP BY
/// expression (an erroring key forms its own group).
pub(crate) struct Extend {
    input: BoxOp,
    slot: Slot,
    expr: CompiledExpr,
    /// As in [`Filter`].
    prefetch: Vec<Slot>,
    canonical: bool,
    error_id: TermId,
    seed: SeedBuf,
    /// Scratch: which rows of the current batch survive (seeded target slot only).
    keep: Vec<bool>,
}

impl Extend {
    pub fn new(input: BoxOp, slot: Slot, expr: CompiledExpr, canonical: bool, error_id: TermId, width: usize) -> Self {
        let prefetch = expr.string_slots();
        Extend { input, slot, expr, prefetch, canonical, error_id, seed: SeedBuf::new(width), keep: Vec::new() }
    }
}

impl Operator for Extend {
    fn open(&mut self, cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        self.input.open(cx, self.seed.full())
    }

    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        let start = out.len();
        let more = self.input.next(cx, out)?;
        prefetch(cx, out, start, &self.prefetch)?;
        let (expr, slot, canonical, error_id) = (&self.expr, self.slot as usize, self.canonical, self.error_id);
        let prefetched = !self.prefetch.is_empty();
        if self.seed.value(self.slot).is_none() {
            let sh = std::rc::Rc::clone(&cx.sh);
            cx.with_eval(|ecx| -> Result<()> {
                for i in start..out.len() {
                    if prefetched {
                        sh.terms.borrow_mut().at_row(i - start);
                    }
                    let v = expr.eval(out.row(i), ecx, canonical)?;
                    out.row_mut(i)[slot] = v.unwrap_or(error_id);
                }
                Ok(())
            })?;
            return Ok(more);
        }
        // The target slot is seeded (an EXISTS substitution of the BIND variable, spec 18.6):
        // it acts as a constant, so the BIND behaves as a join with it. A row survives iff the
        // value is that same term or an error (the seed value is kept). Writing the computed
        // value instead would let a later join merge put the seed back and hide the conflict.
        let keep = &mut self.keep;
        keep.clear();
        let sh = std::rc::Rc::clone(&cx.sh);
        cx.with_eval(|ecx| -> Result<()> {
            for i in start..out.len() {
                if prefetched {
                    sh.terms.borrow_mut().at_row(i - start);
                }
                let v = expr.eval(out.row(i), ecx, canonical)?;
                let current = out.row(i)[slot];
                keep.push(match v {
                    None => {
                        if !current.is_bound() {
                            out.row_mut(i)[slot] = error_id;
                        }
                        true
                    }
                    Some(v) if !current.is_bound() => {
                        out.row_mut(i)[slot] = v;
                        true
                    }
                    Some(v) => sq_expr::same_term_ids(v, current, ecx)?,
                });
            }
            Ok(())
        })?;
        if keep.contains(&false) {
            let mut i = 0;
            out.retain(|_| {
                let k = i < start || keep[i - start];
                i += 1;
                k
            });
        }
        Ok(more)
    }

    fn fast_count(&mut self, cx: &mut ExecCtx, cap: Option<u64>) -> Option<Result<u64>> {
        // One output row per input row, whatever the expression evaluates to, unless the
        // target slot is seeded (then rows may be dropped).
        if self.seed.value(self.slot).is_some() {
            return None;
        }
        self.input.fast_count(cx, cap)
    }

    fn reset(&mut self, cx: &mut ExecCtx) {
        self.input.reset(cx)
    }

    fn close(&mut self, cx: &mut ExecCtx) {
        self.input.close(cx)
    }
}
