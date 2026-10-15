//! Solution modifiers that work in place on their input's rows: Project, Distinct, Reduced
//! and Slice.
use rustc_hash::FxHashSet;
use smallvec::SmallVec;
use sq_core::{BATCH_ROWS, Batch, Result, Slot, TermId};

use super::{BoxOp, Operator};
use crate::ctx::{Charge, ExecCtx};
use crate::rows::{Pull, retain_from};
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

/// OFFSET / LIMIT with early stop: once the limit is reached the input is no longer pulled.
/// When the remaining limit is smaller than the free space of the output batch, the input
/// fills a private batch holding only that many free rows, so streaming inputs stop early.
pub(crate) struct Slice {
    input: BoxOp,
    offset: u64,
    limit: Option<u64>,
    skip: u64,
    left: u64,
    done: bool,
    seed: SeedBuf,
    scratch: Batch,
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
            scratch: Batch::new(width),
        }
    }

    /// Takes the rows the input appended from `start` on: drops the offset, cuts at the limit.
    fn take(&mut self, out: &mut Batch, start: usize) -> Result<()> {
        let skip = &mut self.skip;
        if *skip > 0 {
            retain_from(out, start, |_| {
                if *skip > 0 {
                    *skip -= 1;
                    Ok(false)
                } else {
                    Ok(true)
                }
            })?;
        }
        let got = (out.len() - start) as u64;
        if got >= self.left {
            out.truncate(start + self.left as usize);
            self.left = 0;
            self.done = true;
        } else {
            self.left -= got;
        }
        Ok(())
    }
}

impl Operator for Slice {
    fn open(&mut self, cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        self.skip = self.offset;
        self.left = self.limit.unwrap_or(u64::MAX);
        self.done = self.left == 0;
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
            if out.is_full() {
                return Ok(true);
            }
            let free = (BATCH_ROWS - out.len()) as u64;
            let want = self.left.saturating_add(self.skip);
            let more = if want < free && self.scratch.width() == out.width() {
                // Leave only `want` free rows so that streaming inputs stop early.
                // The padding rows are never read; only their number matters.
                let pad = BATCH_ROWS - want as usize;
                self.scratch.truncate(pad);
                while self.scratch.len() < pad {
                    self.scratch.push_unbound();
                }
                let more = self.input.next(cx, &mut self.scratch)?;
                let start = out.len();
                for i in pad..self.scratch.len() {
                    out.push_row(self.scratch.row(i));
                }
                self.take(out, start)?;
                more
            } else {
                let start = out.len();
                let more = self.input.next(cx, out)?;
                self.take(out, start)?;
                more
            };
            if !more {
                self.done = true;
                return Ok(false);
            }
            if self.done {
                self.input.close(cx);
                return Ok(false);
            }
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
