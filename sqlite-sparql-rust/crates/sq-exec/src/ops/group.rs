//! GROUP BY / aggregates (§5.3, §6.7) and the COUNT(*) operator.
//!
//! Hash aggregation keyed by `SmallVec<[TermId; 2]>` (groups are emitted in first-seen
//! order), or streaming when the input is sorted by its single key. A key slot holding
//! `TermId::ERROR` (an erroring GROUP BY expression) forms its own group and is output as
//! UNBOUND. Without keys an empty input still yields one row; with keys it yields none.
use std::rc::Rc;

use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use sq_core::ir::{AggFunc, AggSpec};
use sq_core::{BATCH_ROWS, Batch, Error, Result, Slot, TermId};
use sq_expr::{Accumulator, CompiledExpr, EvalCtx, accumulator};

use super::{BoxOp, Operator};
use crate::ctx::{Charge, ExecCtx};
use crate::seed::{Seed, SeedBuf};

type Key = SmallVec<[TermId; 2]>;

/// One aggregate: output slot, spec and compiled argument.
pub(crate) struct Agg {
    pub slot: Slot,
    pub spec: AggSpec,
    pub arg: Option<CompiledExpr>,
}

/// Estimated bytes per group and aggregate (accumulator box, map entry share).
const GROUP_BYTES: usize = 96;

fn new_accs(aggs: &[Agg], into: &mut Vec<Box<dyn Accumulator>>) {
    into.extend(aggs.iter().map(|a| accumulator(&a.spec)));
}

/// Feeds one input row to a group's accumulators: COUNT(*) takes the row (COUNT(DISTINCT *)
/// dedupes it on the visible slots), every other aggregate its argument evaluated with
/// canonical ids (None for unbound / error).
fn feed(
    aggs: &[Agg],
    accs: &mut [Box<dyn Accumulator>],
    row: &[TermId],
    visible: &[Slot],
    ecx: &mut EvalCtx<'_>,
) -> Result<()> {
    for (a, acc) in aggs.iter().zip(accs.iter_mut()) {
        if a.spec.func == AggFunc::CountAll {
            acc.add_row(row, visible, ecx)?;
        } else {
            let v = match &a.arg {
                Some(e) => e.eval(row, ecx, true)?,
                None => None,
            };
            acc.add(v, ecx)?;
        }
    }
    Ok(())
}

/// Appends the output row of a group: keys (ERROR -> UNBOUND), aggregate results, and the
/// substituted constants of an EXISTS evaluation.
fn emit(
    keys: &[Slot],
    aggs: &[Agg],
    seed: &SeedBuf,
    key: &[TermId],
    accs: &mut [Box<dyn Accumulator>],
    cx: &mut ExecCtx,
    out: &mut Batch,
) -> Result<()> {
    out.push_unbound();
    let i = out.len() - 1;
    for (s, v) in keys.iter().zip(key) {
        out.row_mut(i)[*s as usize] = if *v == TermId::ERROR { TermId::UNBOUND } else { *v };
    }
    cx.with_eval(|ecx| -> Result<()> {
        for (a, acc) in aggs.iter().zip(accs.iter_mut()) {
            out.row_mut(i)[a.slot as usize] = acc.finish(ecx)?.unwrap_or(TermId::UNBOUND);
        }
        Ok(())
    })?;
    seed.refill_subst(out.row_mut(i));
    Ok(())
}

pub(crate) struct Group {
    input: BoxOp,
    keys: Vec<Slot>,
    aggs: Vec<Agg>,
    streaming: bool,
    seed: SeedBuf,
    inb: Batch,
    /// Key of the current row (reused).
    scratch: Key,
    // hash mode
    index: FxHashMap<Key, u32>,
    order: Vec<Key>,
    accs: Vec<Box<dyn Accumulator>>,
    emit_pos: usize,
    built: bool,
    // streaming mode
    in_pos: usize,
    in_more: bool,
    cur: Option<TermId>,
    cur_accs: Vec<Box<dyn Accumulator>>,
    charge: Charge,
}

impl Group {
    pub fn new(input: BoxOp, keys: Vec<Slot>, aggs: Vec<Agg>, streaming: bool, width: usize) -> Self {
        let streaming = streaming && keys.len() == 1;
        Group {
            input,
            keys,
            aggs,
            streaming,
            seed: SeedBuf::new(width),
            inb: Batch::new(width),
            scratch: Key::new(),
            index: FxHashMap::default(),
            order: Vec::new(),
            accs: Vec::new(),
            emit_pos: 0,
            built: false,
            in_pos: 0,
            in_more: true,
            cur: None,
            cur_accs: Vec::new(),
            charge: Charge::default(),
        }
    }

    /// Hash aggregation of the whole input.
    fn build(&mut self, cx: &mut ExecCtx) -> Result<()> {
        let n = self.aggs.len();
        let sh = Rc::clone(&cx.sh);
        loop {
            self.inb.clear();
            let more = self.input.next(cx, &mut self.inb)?;
            let Group { keys, aggs, inb, scratch, index, order, accs, charge, .. } = self;
            cx.with_eval(|ecx| -> Result<()> {
                for r in 0..inb.len() {
                    let row = inb.row(r);
                    scratch.clear();
                    scratch.extend(keys.iter().map(|s| row[*s as usize]));
                    let g = match index.get(&scratch[..]) {
                        Some(g) => *g as usize,
                        None => {
                            sh.charge(charge, GROUP_BYTES * n.max(1) + 8 * scratch.len())?;
                            let g = order.len();
                            index.insert(scratch.clone(), g as u32);
                            order.push(scratch.clone());
                            new_accs(aggs, accs);
                            g
                        }
                    };
                    feed(aggs, &mut accs[g * n..(g + 1) * n], row, &sh.visible, ecx)?;
                }
                Ok(())
            })?;
            if !more {
                break;
            }
        }
        self.input.close(cx);
        if self.order.is_empty() && self.keys.is_empty() {
            // No GROUP BY and no rows: exactly one group (COUNT 0, SUM 0, ...).
            self.order.push(Key::new());
            new_accs(&self.aggs, &mut self.accs);
        }
        self.built = true;
        Ok(())
    }

    fn next_hash(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        if !self.built {
            self.build(cx)?;
        }
        let n = self.aggs.len();
        while self.emit_pos < self.order.len() {
            if out.is_full() {
                return Ok(true);
            }
            let g = self.emit_pos;
            self.emit_pos += 1;
            let key = &self.order[g];
            emit(&self.keys, &self.aggs, &self.seed, key, &mut self.accs[g * n..(g + 1) * n], cx, out)?;
        }
        Ok(false)
    }

    fn next_streaming(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        let key_slot = self.keys[0];
        let sh = Rc::clone(&cx.sh);
        loop {
            if out.is_full() {
                return Ok(true);
            }
            if self.in_pos == self.inb.len() {
                if !self.in_more {
                    // Input exhausted: the last group.
                    if let Some(k) = self.cur.take() {
                        emit(&self.keys, &self.aggs, &self.seed, &[k], &mut self.cur_accs, cx, out)?;
                    }
                    return Ok(false);
                }
                self.inb.clear();
                self.in_more = self.input.next(cx, &mut self.inb)?;
                self.in_pos = 0;
                continue;
            }
            // Feed the run of rows of the current key that this batch holds.
            let v = self.inb.get(self.in_pos, key_slot);
            match self.cur {
                Some(k) if k != v => {
                    emit(&self.keys, &self.aggs, &self.seed, &[k], &mut self.cur_accs, cx, out)?;
                    self.cur = None;
                    continue;
                }
                Some(_) => {}
                None => {
                    self.cur = Some(v);
                    self.cur_accs.clear();
                    new_accs(&self.aggs, &mut self.cur_accs);
                }
            }
            let Group { aggs, inb, in_pos, cur_accs, .. } = self;
            cx.with_eval(|ecx| -> Result<()> {
                while *in_pos < inb.len() && inb.get(*in_pos, key_slot) == v {
                    feed(aggs, cur_accs, inb.row(*in_pos), &sh.visible, ecx)?;
                    *in_pos += 1;
                }
                Ok(())
            })?;
        }
    }
}

impl Operator for Group {
    fn open(&mut self, cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        self.index.clear();
        self.order.clear();
        self.accs.clear();
        self.emit_pos = 0;
        self.built = false;
        self.inb.clear();
        self.in_pos = 0;
        self.in_more = true;
        self.cur = None;
        self.cur_accs.clear();
        cx.release(&mut self.charge);
        self.input.open(cx, self.seed.full())
    }

    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        if self.streaming { self.next_streaming(cx, out) } else { self.next_hash(cx, out) }
    }

    fn close(&mut self, cx: &mut ExecCtx) {
        self.index = FxHashMap::default();
        self.order = Vec::new();
        self.accs = Vec::new();
        self.cur_accs = Vec::new();
        cx.release(&mut self.charge);
        self.input.close(cx)
    }
}

/// COUNT(*) without DISTINCT or GROUP BY: one row binding `out`. Uses the input's
/// `fast_count` (a Scan counts inside the store, a MergeJoin factorizes), else iterates.
pub(crate) struct Count {
    input: BoxOp,
    out: Slot,
    seed: SeedBuf,
    done: bool,
    inb: Batch,
}

impl Count {
    pub fn new(input: BoxOp, out: Slot, width: usize) -> Self {
        Count { input, out, seed: SeedBuf::new(width), done: true, inb: Batch::with_capacity(width, BATCH_ROWS) }
    }

    fn count(&mut self, cx: &mut ExecCtx) -> Result<u64> {
        // Never seeded with Join-mode values; the substitution reaches the input.
        self.input.open(cx, self.seed.subst_only())?;
        let n = match self.input.fast_count(cx) {
            Some(n) => n?,
            None => {
                let mut n = 0u64;
                loop {
                    self.inb.clear();
                    let more = self.input.next(cx, &mut self.inb)?;
                    n += self.inb.len() as u64;
                    if !more {
                        break;
                    }
                }
                n
            }
        };
        self.input.close(cx);
        Ok(n)
    }
}

impl Operator for Count {
    fn open(&mut self, _cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        self.done = false;
        Ok(())
    }

    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        if self.done {
            return Ok(false);
        }
        if out.is_full() {
            return Ok(true);
        }
        let n = self.count(cx)?;
        self.done = true;
        let id = i64::try_from(n).ok().and_then(TermId::from_int).ok_or_else(|| Error::internal("count overflow"))?;
        out.push_unbound();
        let i = out.len() - 1;
        let row = out.row_mut(i);
        row[self.out as usize] = id;
        self.seed.refill_subst(row);
        Ok(false)
    }

    fn fast_count(&mut self, _cx: &mut ExecCtx) -> Option<Result<u64>> {
        let n = u64::from(!self.done);
        self.done = true;
        Some(Ok(n))
    }

    fn close(&mut self, cx: &mut ExecCtx) {
        self.done = true;
        self.input.close(cx)
    }
}
