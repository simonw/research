//! Merge joins over inputs sorted ascending (signed ids) by a certainly-bound key (§5.3).
//!
//! * MergeJoin: the right-hand run of equal keys is buffered for many-to-many joins; after
//!   `gallop` consecutive non-matching rows an input that supports [`Operator::seek`] (a Scan
//!   whose first free column is the key, a SortBySlot enforcer) jumps to the other side's key.
//!   `fast_count` returns the sum over key runs of (left run x right run) when `check` is
//!   empty, without producing a row.
//! * StarMerge: leapfrog intersection of k inputs on the key; each common key's runs are
//!   combined (cartesian product with the `check` compatibility test).
use sq_core::{BATCH_ROWS, Batch, Error, Result, Slot, TermId};

use super::{BoxOp, Operator};
use crate::ctx::{Charge, ExecCtx};
use crate::rows::{Rows, compatible, merge_into, push_merged};
use crate::seed::{Seed, SeedBuf};

/// Rows read per refill right after a gallop seek; doubled per refill up to a full batch.
const MIN_CHUNK: usize = 16;

/// A sorted input read row by row.
pub(crate) struct Sorted {
    op: BoxOp,
    b: Batch,
    pos: usize,
    more: bool,
    chunk: usize,
    /// Last key seen (sortedness check).
    last: TermId,
}

impl Sorted {
    fn new(op: BoxOp, width: usize) -> Self {
        Sorted { op, b: Batch::new(width), pos: 0, more: false, chunk: BATCH_ROWS, last: TermId(i64::MIN) }
    }

    fn open(&mut self, cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.b.clear();
        self.pos = 0;
        self.more = true;
        self.chunk = BATCH_ROWS;
        self.last = TermId(i64::MIN);
        self.op.open(cx, seed)
    }

    /// Ensures a current row; false when exhausted.
    fn head(&mut self, cx: &mut ExecCtx) -> Result<bool> {
        while self.pos == self.b.len() {
            if !self.more {
                return Ok(false);
            }
            self.b.clear();
            self.pos = 0;
            self.more = self.op.next_limited(cx, &mut self.b, self.chunk)?;
            self.chunk = (self.chunk * 2).min(BATCH_ROWS);
        }
        Ok(true)
    }

    #[inline]
    fn key(&self, slot: Slot) -> TermId {
        self.b.get(self.pos, slot)
    }

    #[inline]
    fn row(&self) -> &[TermId] {
        self.b.row(self.pos)
    }

    /// Moves past the current row, checking the sort order of the input.
    #[inline]
    fn advance(&mut self, slot: Slot) -> Result<()> {
        let k = self.key(slot);
        if k < self.last {
            return Err(Error::internal(format!("merge join input not sorted by slot {slot}")));
        }
        self.last = k;
        self.pos += 1;
        Ok(())
    }

    /// Advances to the first row whose key is >= `target`; false when exhausted. After
    /// `gallop` consecutive smaller rows, once the buffer is used up, seeks instead of reading.
    fn advance_to(&mut self, cx: &mut ExecCtx, slot: Slot, target: TermId, gallop: u32) -> Result<bool> {
        let mut misses = 0u32;
        loop {
            if !self.head(cx)? {
                return Ok(false);
            }
            while self.pos < self.b.len() && self.b.get(self.pos, slot) < target {
                self.advance(slot)?;
                misses += 1;
            }
            if self.pos < self.b.len() {
                return Ok(true);
            }
            if gallop > 0 && misses >= gallop && self.more && self.op.seek(cx, slot, target)? {
                self.b.clear();
                self.pos = 0;
                self.chunk = MIN_CHUNK;
                misses = 0;
            }
        }
    }

    /// Drops the buffered rows; `keep_cursors` resets the input instead of closing it.
    fn stop(&mut self, cx: &mut ExecCtx, keep_cursors: bool) {
        self.b.clear();
        self.pos = 0;
        self.more = false;
        if keep_cursors { self.op.reset(cx) } else { self.op.close(cx) }
    }
}

pub(crate) struct MergeJoin {
    left: Sorted,
    right: Sorted,
    key: Slot,
    check: Vec<Slot>,
    gallop: u32,
    seed: SeedBuf,
    run: Rows,
    run_key: TermId,
    run_valid: bool,
    emitting: bool,
    ri: usize,
    done: bool,
    charge: Charge,
}

impl MergeJoin {
    pub fn new(left: BoxOp, right: BoxOp, key: Slot, check: Vec<Slot>, gallop: u32, width: usize) -> Self {
        MergeJoin {
            left: Sorted::new(left, width),
            right: Sorted::new(right, width),
            key,
            check,
            gallop,
            seed: SeedBuf::new(width),
            run: Rows::new(width),
            run_key: TermId::UNBOUND,
            run_valid: false,
            emitting: false,
            ri: 0,
            done: true,
            charge: Charge::default(),
        }
    }

    /// Buffers the right run of `k` (the right input is positioned on it).
    fn load_run(&mut self, cx: &mut ExecCtx, k: TermId) -> Result<()> {
        cx.release(&mut self.charge);
        self.run.clear();
        while self.right.head(cx)? && self.right.key(self.key) == k {
            self.run.push(self.right.row());
            self.right.advance(self.key)?;
        }
        cx.charge(&mut self.charge, self.run.len() * self.run.row_bytes())?;
        self.run_key = k;
        self.run_valid = true;
        Ok(())
    }

    fn finish(&mut self, cx: &mut ExecCtx) -> Result<bool> {
        self.done = true;
        cx.release(&mut self.charge);
        Ok(false)
    }
}

impl Operator for MergeJoin {
    fn open(&mut self, cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        self.run.clear();
        self.run_valid = false;
        self.emitting = false;
        self.done = false;
        cx.release(&mut self.charge);
        // An inner join: the seed reaches both inputs.
        self.left.open(cx, self.seed.full())?;
        self.right.open(cx, self.seed.full())
    }

    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        if self.done {
            return Ok(false);
        }
        let key = self.key;
        loop {
            if out.is_full() {
                return Ok(true);
            }
            if self.emitting {
                let lrow = self.left.row();
                while self.ri < self.run.len() {
                    if out.is_full() {
                        return Ok(true);
                    }
                    let r = self.run.row(self.ri);
                    self.ri += 1;
                    if compatible(lrow, r, &self.check) {
                        push_merged(out, lrow, r);
                    }
                }
                self.emitting = false;
                self.left.advance(key)?;
            }
            if !self.left.head(cx)? {
                return self.finish(cx);
            }
            let lk = self.left.key(key);
            if !(self.run_valid && self.run_key == lk) {
                if !self.right.advance_to(cx, key, lk, self.gallop)? {
                    return self.finish(cx);
                }
                let rk = self.right.key(key);
                if rk > lk {
                    if !self.left.advance_to(cx, key, rk, self.gallop)? {
                        return self.finish(cx);
                    }
                    continue;
                }
                self.load_run(cx, lk)?;
            }
            self.emitting = true;
            self.ri = 0;
        }
    }

    fn fast_count(&mut self, cx: &mut ExecCtx, cap: Option<u64>) -> Option<Result<u64>> {
        if !self.check.is_empty() || self.done {
            return None;
        }
        let key = self.key;
        let mut count = || -> Result<u64> {
            let mut total = 0u64;
            while self.left.head(cx)? {
                let lk = self.left.key(key);
                let mut nl = 0u64;
                while self.left.head(cx)? && self.left.key(key) == lk {
                    nl += 1;
                    self.left.advance(key)?;
                }
                if !self.right.advance_to(cx, key, lk, self.gallop)? {
                    break;
                }
                let mut nr = 0u64;
                while self.right.head(cx)? && self.right.key(key) == lk {
                    nr += 1;
                    self.right.advance(key)?;
                }
                total += nl * nr;
                if cap.is_some_and(|c| total > c) {
                    break;
                }
                if nr == 0 {
                    let rk = self.right.key(key);
                    if !self.left.advance_to(cx, key, rk, self.gallop)? {
                        break;
                    }
                }
                cx.tick((nl + nr) as usize)?;
            }
            Ok(total)
        };
        let r = count();
        self.done = true;
        Some(r)
    }

    fn reset(&mut self, cx: &mut ExecCtx) {
        self.stop(cx, true);
    }

    fn close(&mut self, cx: &mut ExecCtx) {
        self.stop(cx, false);
    }
}

impl MergeJoin {
    /// Frees the buffered run and stops both inputs (`keep_cursors`: reset, else close).
    fn stop(&mut self, cx: &mut ExecCtx, keep_cursors: bool) {
        self.run.reset();
        self.run_valid = false;
        self.done = true;
        cx.release(&mut self.charge);
        self.left.stop(cx, keep_cursors);
        self.right.stop(cx, keep_cursors);
    }
}

/// Leapfrog intersection on `key` over k >= 2 inputs sorted by it.
pub(crate) struct StarMerge {
    inputs: Vec<Sorted>,
    key: Slot,
    check: Vec<Slot>,
    seed: SeedBuf,
    runs: Vec<Rows>,
    /// Odometer over the runs of the current key (last input fastest).
    idx: Vec<usize>,
    emitting: bool,
    done: bool,
    charge: Charge,
}

/// StarMerge seeks after this many consecutive misses (the SQL-path galloping threshold).
const STAR_GALLOP: u32 = 6;

impl StarMerge {
    pub fn new(inputs: Vec<BoxOp>, key: Slot, check: Vec<Slot>, width: usize) -> Self {
        let k = inputs.len();
        StarMerge {
            inputs: inputs.into_iter().map(|op| Sorted::new(op, width)).collect(),
            key,
            check,
            seed: SeedBuf::new(width),
            runs: (0..k).map(|_| Rows::new(width)).collect(),
            idx: vec![0; k],
            emitting: false,
            done: true,
            charge: Charge::default(),
        }
    }

    /// Positions every input on the next common key; None when some input is exhausted.
    fn next_common(&mut self, cx: &mut ExecCtx) -> Result<Option<TermId>> {
        let k = self.inputs.len();
        if !self.inputs[0].head(cx)? {
            return Ok(None);
        }
        let mut target = self.inputs[0].key(self.key);
        let mut agree = 1;
        let mut i = 1;
        while agree < k {
            let inp = &mut self.inputs[i % k];
            if !inp.advance_to(cx, self.key, target, STAR_GALLOP)? {
                return Ok(None);
            }
            let kk = inp.key(self.key);
            if kk == target {
                agree += 1;
            } else {
                target = kk;
                agree = 1;
            }
            i += 1;
        }
        Ok(Some(target))
    }

    /// Buffers the run of `target` of every input.
    fn load_runs(&mut self, cx: &mut ExecCtx, target: TermId) -> Result<()> {
        cx.release(&mut self.charge);
        let mut bytes = 0;
        for (inp, run) in self.inputs.iter_mut().zip(&mut self.runs) {
            run.clear();
            while inp.head(cx)? && inp.key(self.key) == target {
                run.push(inp.row());
                inp.advance(self.key)?;
            }
            bytes += run.len() * run.row_bytes();
        }
        cx.charge(&mut self.charge, bytes)
    }

    /// Advances the odometer; false when every combination was visited.
    fn step(&mut self) -> bool {
        for j in (0..self.idx.len()).rev() {
            self.idx[j] += 1;
            if self.idx[j] < self.runs[j].len() {
                return true;
            }
            self.idx[j] = 0;
        }
        false
    }
}

impl Operator for StarMerge {
    fn open(&mut self, cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        self.emitting = false;
        self.done = false;
        cx.release(&mut self.charge);
        for inp in &mut self.inputs {
            inp.open(cx, self.seed.full())?;
        }
        Ok(())
    }

    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        if self.done {
            return Ok(false);
        }
        loop {
            if out.is_full() {
                return Ok(true);
            }
            if self.emitting {
                loop {
                    if out.is_full() {
                        return Ok(true);
                    }
                    let i = out.len();
                    out.push_row(self.runs[0].row(self.idx[0]));
                    let mut ok = true;
                    for j in 1..self.runs.len() {
                        let r = self.runs[j].row(self.idx[j]);
                        if !compatible(out.row(i), r, &self.check) {
                            ok = false;
                            break;
                        }
                        merge_into(out.row_mut(i), r);
                    }
                    if !ok {
                        out.truncate(i);
                    }
                    if !self.step() {
                        self.emitting = false;
                        break;
                    }
                }
                continue;
            }
            match self.next_common(cx)? {
                None => {
                    self.done = true;
                    cx.release(&mut self.charge);
                    return Ok(false);
                }
                Some(target) => {
                    self.load_runs(cx, target)?;
                    self.idx.fill(0);
                    self.emitting = true;
                }
            }
        }
    }

    fn fast_count(&mut self, cx: &mut ExecCtx, cap: Option<u64>) -> Option<Result<u64>> {
        if !self.check.is_empty() || self.done {
            return None;
        }
        let mut count = || -> Result<u64> {
            let mut total = 0u64;
            while let Some(target) = self.next_common(cx)? {
                let mut product = 1u64;
                for inp in &mut self.inputs {
                    let mut n = 0u64;
                    while inp.head(cx)? && inp.key(self.key) == target {
                        n += 1;
                        inp.advance(self.key)?;
                    }
                    product = product.saturating_mul(n);
                }
                total = total.saturating_add(product);
                if cap.is_some_and(|c| total > c) {
                    break;
                }
            }
            Ok(total)
        };
        let r = count();
        self.done = true;
        Some(r)
    }

    fn reset(&mut self, cx: &mut ExecCtx) {
        self.stop(cx, true);
    }

    fn close(&mut self, cx: &mut ExecCtx) {
        self.stop(cx, false);
    }
}

impl StarMerge {
    /// Frees the buffered runs and stops every input (`keep_cursors`: reset, else close).
    fn stop(&mut self, cx: &mut ExecCtx, keep_cursors: bool) {
        for r in &mut self.runs {
            r.reset();
        }
        self.done = true;
        cx.release(&mut self.charge);
        for inp in &mut self.inputs {
            inp.stop(cx, keep_cursors);
        }
    }
}
