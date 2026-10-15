//! Filter (EBV; an error counts as false) and Extend (an error leaves the slot unbound). Both
//! work in place on the rows their input appends to the output batch.
use sq_core::{Batch, Result, Slot, TermId};
use sq_expr::CompiledExpr;

use super::{BoxOp, Operator};
use crate::ctx::ExecCtx;
use crate::rows::retain_from;
use crate::seed::{Seed, SeedBuf};

pub(crate) struct Filter {
    input: BoxOp,
    expr: CompiledExpr,
    seed: SeedBuf,
}

impl Filter {
    pub fn new(input: BoxOp, expr: CompiledExpr, width: usize) -> Self {
        Filter { input, expr, seed: SeedBuf::new(width) }
    }
}

impl Operator for Filter {
    fn open(&mut self, cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        self.input.open(cx, self.seed.full())
    }

    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        loop {
            if out.is_full() {
                return Ok(true);
            }
            let start = out.len();
            let more = self.input.next(cx, out)?;
            let expr = &self.expr;
            cx.with_eval(|ecx| retain_from(out, start, |row| Ok(expr.ebv(row, ecx)? == Some(true))))?;
            if !more {
                return Ok(false);
            }
        }
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
    canonical: bool,
    error_id: TermId,
    seed: SeedBuf,
}

impl Extend {
    pub fn new(input: BoxOp, slot: Slot, expr: CompiledExpr, canonical: bool, error_id: TermId, width: usize) -> Self {
        Extend { input, slot, expr, canonical, error_id, seed: SeedBuf::new(width) }
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
        let (expr, slot, canonical, error_id) = (&self.expr, self.slot as usize, self.canonical, self.error_id);
        cx.with_eval(|ecx| -> Result<()> {
            for i in start..out.len() {
                let v = expr.eval(out.row(i), ecx, canonical)?;
                out.row_mut(i)[slot] = v.unwrap_or(error_id);
            }
            Ok(())
        })?;
        Ok(more)
    }

    fn fast_count(&mut self, cx: &mut ExecCtx) -> Option<Result<u64>> {
        // One output row per input row, whatever the expression evaluates to.
        self.input.fast_count(cx)
    }

    fn close(&mut self, cx: &mut ExecCtx) {
        self.input.close(cx)
    }
}
