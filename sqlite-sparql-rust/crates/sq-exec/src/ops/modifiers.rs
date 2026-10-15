//! Solution modifiers that work in place on their input's rows: Project, Distinct, Reduced
//! and Slice.
use rustc_hash::FxHashSet;
use smallvec::SmallVec;
use sq_core::{BATCH_ROWS, Batch, Result, Slot, TermId};

use super::{BoxOp, Operator};
use crate::ctx::{Charge, ExecCtx};
use crate::rows::Pull;
use crate::seed::{Seed, SeedBuf};

/// Keeps `slots`, resets every other slot to UNBOUND (inside an EXISTS pattern the substituted
/// constants are kept as well: they are constants, not variables of the sub-select).
pub(crate) struct Project {
    input: BoxOp,
    keep: Vec<bool>,
    /// Slots reset by the current open.
    clear: Vec<Slot>,
    seed: SeedBuf,
}

impl Project {
    pub fn new(input: BoxOp, slots: &[Slot], width: usize) -> Self {
        let mut keep = vec![false; width];
        for s in slots {
            if let Some(k) = keep.get_mut(*s as usize) {
                *k = true;
            }
        }
        Project { input, keep, clear: Vec::new(), seed: SeedBuf::new(width) }
    }
}

impl Operator for Project {
    fn open(&mut self, cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        self.clear.clear();
        for (s, keep) in self.keep.iter().enumerate() {
            if !keep && !self.seed.is_subst(s as Slot) {
                self.clear.push(s as Slot);
            }
        }
        self.input.open(cx, self.seed.full())
    }

    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        let start = out.len();
        let more = self.input.next(cx, out)?;
        for i in start..out.len() {
            let row = out.row_mut(i);
            for &s in &self.clear {
                row[s as usize] = TermId::UNBOUND;
            }
            self.seed.refill_subst(row);
        }
        Ok(more)
    }

    fn fast_count(&mut self, cx: &mut ExecCtx) -> Option<Result<u64>> {
        self.input.fast_count(cx)
    }

    fn close(&mut self, cx: &mut ExecCtx) {
        self.input.close(cx)
    }
}

/// Distinct keys, specialised for one, two and n slots.
enum Seen {
    /// No slots: every row is the same solution; true once one was kept.
    Zero(bool),
    One(FxHashSet<TermId>),
    Two(FxHashSet<(TermId, TermId)>),
    /// Keys are looked up through `scratch`; only new keys are stored (no per-row allocation).
    Many(FxHashSet<SmallVec<[TermId; 4]>>, Vec<TermId>),
}

impl Seen {
    fn new(n: usize) -> Self {
        match n {
            0 => Seen::Zero(false),
            1 => Seen::One(FxHashSet::default()),
            2 => Seen::Two(FxHashSet::default()),
            _ => Seen::Many(FxHashSet::default(), Vec::with_capacity(n)),
        }
    }

    /// Inserts the key of `row`; returns (was new, bytes charged).
    #[inline]
    fn insert(&mut self, row: &[TermId], slots: &[Slot]) -> (bool, usize) {
        match self {
            Seen::Zero(any) => (!std::mem::replace(any, true), 0),
            Seen::One(s) => (s.insert(row[slots[0] as usize]), 16),
            Seen::Two(s) => (s.insert((row[slots[0] as usize], row[slots[1] as usize])), 24),
            Seen::Many(s, scratch) => {
                scratch.clear();
                scratch.extend(slots.iter().map(|x| row[*x as usize]));
                if s.contains(&scratch[..]) {
                    return (false, 0);
                }
                let k = SmallVec::from_slice(scratch);
                let bytes = 48 + if k.spilled() { 8 * k.len() } else { 0 };
                (s.insert(k), bytes)
            }
        }
    }

    fn clear(&mut self) {
        match self {
            Seen::Zero(any) => *any = false,
            Seen::One(s) => *s = FxHashSet::default(),
            Seen::Two(s) => *s = FxHashSet::default(),
            Seen::Many(s, _) => *s = FxHashSet::default(),
        }
    }
}

/// DISTINCT on `slots` (the slots the input may bind): the first row of each key is kept.
pub(crate) struct Distinct {
    input: BoxOp,
    slots: Vec<Slot>,
    seen: Seen,
    seed: SeedBuf,
    pull: Pull,
    charge: Charge,
}

impl Distinct {
    pub fn new(input: BoxOp, slots: Vec<Slot>, width: usize) -> Self {
        let seen = Seen::new(slots.len());
        Distinct { input, slots, seen, seed: SeedBuf::new(width), pull: Pull::new(width), charge: Charge::default() }
    }
}

impl Operator for Distinct {
    fn open(&mut self, cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        self.seen.clear();
        self.pull.reset();
        cx.release(&mut self.charge);
        self.input.open(cx, self.seed.full())
    }

    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        let (seen, slots, charge) = (&mut self.seen, &self.slots[..], &mut self.charge);
        self.pull.pump(cx, self.input.as_mut(), out, |cx, row| {
            let (new, bytes) = seen.insert(row, slots);
            if new {
                cx.charge(charge, bytes)?;
            }
            Ok(new)
        })
    }

    fn close(&mut self, cx: &mut ExecCtx) {
        self.seen.clear();
        cx.release(&mut self.charge);
        self.input.close(cx)
    }
}

/// REDUCED: adjacent duplicates (on `slots`) are dropped.
pub(crate) struct Reduced {
    input: BoxOp,
    slots: Vec<Slot>,
    last: Vec<TermId>,
    has_last: bool,
    seed: SeedBuf,
    pull: Pull,
}

impl Reduced {
    pub fn new(input: BoxOp, slots: Vec<Slot>, width: usize) -> Self {
        let n = slots.len();
        let last = vec![TermId::UNBOUND; n];
        Reduced { input, slots, last, has_last: false, seed: SeedBuf::new(width), pull: Pull::new(width) }
    }
}

impl Operator for Reduced {
    fn open(&mut self, cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        self.has_last = false;
        self.pull.reset();
        self.input.open(cx, self.seed.full())
    }

    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        let (slots, last, has_last) = (&self.slots, &mut self.last, &mut self.has_last);
        self.pull.pump(cx, self.input.as_mut(), out, |_, row| {
            let same = *has_last && slots.iter().zip(last.iter()).all(|(s, v)| row[*s as usize] == *v);
            if !same {
                for (s, v) in slots.iter().zip(last.iter_mut()) {
                    *v = row[*s as usize];
                }
                *has_last = true;
            }
            Ok(!same)
        })
    }

    fn close(&mut self, cx: &mut ExecCtx) {
        self.input.close(cx)
    }
}

/// OFFSET / LIMIT with early stop: once the limit is reached the input is closed and no
/// longer pulled. The input fills a private batch that is padded so that only
/// min(offset + limit left, 1024) rows are free: streaming inputs stop early under small
/// limits, and skipped rows never reach the output batch.
pub(crate) struct Slice {
    input: BoxOp,
    offset: u64,
    limit: Option<u64>,
    skip: u64,
    left: u64,
    done: bool,
    seed: SeedBuf,
    buf: Batch,
    /// Next unread row of `buf`.
    pos: usize,
    more: bool,
}

impl Slice {
    pub fn new(input: BoxOp, offset: u64, limit: Option<u64>, width: usize) -> Self {
        Slice {
            input,
            offset,
            limit,
            skip: offset,
            left: limit.unwrap_or(u64::MAX),
            done: true,
            seed: SeedBuf::new(width),
            buf: Batch::new(width),
            pos: 0,
            more: false,
        }
    }

    fn finish(&mut self, cx: &mut ExecCtx) -> Result<bool> {
        self.done = true;
        self.input.close(cx);
        Ok(false)
    }
}

impl Operator for Slice {
    fn open(&mut self, cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        self.skip = self.offset;
        self.left = self.limit.unwrap_or(u64::MAX);
        self.done = self.left == 0;
        self.buf.clear();
        self.pos = 0;
        self.more = true;
        if self.done {
            return Ok(());
        }
        // Never seeded with Join-mode values (safe_inject(Slice) is empty).
        self.input.open(cx, self.seed.subst_only())
    }

    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        loop {
            if self.done {
                return Ok(false);
            }
            let unread = self.buf.len() - self.pos;
            let skipped = (self.skip.min(unread as u64)) as usize;
            self.pos += skipped;
            self.skip -= skipped as u64;
            while self.pos < self.buf.len() {
                if self.left == 0 {
                    return self.finish(cx);
                }
                if out.is_full() {
                    return Ok(true);
                }
                out.push_row(self.buf.row(self.pos));
                self.pos += 1;
                self.left -= 1;
            }
            if self.left == 0 || !self.more {
                return self.finish(cx);
            }
            if out.is_full() {
                return Ok(true);
            }
            // Pad so that only the rows still usable are free; the padding is never read.
            let want = self.skip.saturating_add(self.left).min(BATCH_ROWS as u64) as usize;
            let pad = BATCH_ROWS - want;
            self.buf.truncate(pad);
            while self.buf.len() < pad {
                self.buf.push_unbound();
            }
            self.pos = pad;
            self.more = self.input.next(cx, &mut self.buf)?;
        }
    }

    fn fast_count(&mut self, cx: &mut ExecCtx) -> Option<Result<u64>> {
        if self.done {
            return Some(Ok(0));
        }
        let n = self.input.fast_count(cx)?;
        Some(n.map(|n| {
            self.done = true;
            n.saturating_sub(self.offset).min(self.limit.unwrap_or(u64::MAX))
        }))
    }

    fn close(&mut self, cx: &mut ExecCtx) {
        self.done = true;
        self.input.close(cx)
    }
}
