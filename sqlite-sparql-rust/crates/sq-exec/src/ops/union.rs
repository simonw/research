//! UNION: the branches one after another (bag semantics), each opened lazily with the seed.
use sq_core::{Batch, Result};

use super::{BoxOp, Operator};
use crate::ctx::ExecCtx;
use crate::seed::{Seed, SeedBuf};

pub(crate) struct Union {
    branches: Vec<BoxOp>,
    seed: SeedBuf,
    cur: usize,
    opened: bool,
}

impl Union {
    pub fn new(branches: Vec<BoxOp>, width: usize) -> Self {
        let cur = branches.len();
        Union { branches, seed: SeedBuf::new(width), cur, opened: false }
    }
}

impl Operator for Union {
    fn open(&mut self, _cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        self.cur = 0;
        self.opened = false;
        Ok(())
    }

    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        while self.cur < self.branches.len() {
            if out.is_full() {
                return Ok(true);
            }
            let b = &mut self.branches[self.cur];
            if !self.opened {
                b.open(cx, self.seed.full())?;
                self.opened = true;
            }
            if !b.next(cx, out)? {
                self.cur += 1;
                self.opened = false;
            }
        }
        Ok(false)
    }

    fn fast_count(&mut self, cx: &mut ExecCtx) -> Option<Result<u64>> {
        // All-or-nothing: only when no branch has been started.
        if self.cur != 0 || self.opened {
            return None;
        }
        let mut total = 0;
        for (i, b) in self.branches.iter_mut().enumerate() {
            if let Err(e) = b.open(cx, self.seed.full()) {
                return Some(Err(e));
            }
            match b.fast_count(cx) {
                Some(Ok(n)) => total += n,
                Some(Err(e)) => return Some(Err(e)),
                None => {
                    // Fall back to iteration from this branch on (earlier branches are counted).
                    self.cur = i;
                    self.opened = true;
                    return self.count_rest(cx, total);
                }
            }
        }
        self.cur = self.branches.len();
        Some(Ok(total))
    }

    fn close(&mut self, cx: &mut ExecCtx) {
        for b in &mut self.branches {
            b.close(cx);
        }
        self.cur = self.branches.len();
    }
}

impl Union {
    /// Counts the remaining rows by iteration (after a partial fast count).
    fn count_rest(&mut self, cx: &mut ExecCtx, mut total: u64) -> Option<Result<u64>> {
        let mut b = Batch::new(self.seed.base().len());
        loop {
            b.clear();
            match self.next(cx, &mut b) {
                Ok(more) => {
                    total += b.len() as u64;
                    if !more {
                        return Some(Ok(total));
                    }
                }
                Err(e) => return Some(Err(e)),
            }
        }
    }
}
