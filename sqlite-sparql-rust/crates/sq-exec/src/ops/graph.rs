//! GraphLoop (§5.3): `GRAPH ?g { P }` evaluated per named graph when it cannot be pushed into
//! the patterns. For each named graph g of the dataset (sorted; only the seeded value when the
//! slot is seeded and names a graph of the dataset): push g on the graph register stack,
//! re-open the input, drop rows whose slot is bound to another value, bind the slot to g,
//! pop g. The register is pushed around every `open` and `next` of the input, because inputs
//! open some of their subtrees lazily (Union branches, BindJoin probes, EXISTS evaluations).
use sq_core::{Batch, Result, Slot, TermId};

use super::{BoxOp, Operator};
use crate::ctx::ExecCtx;
use crate::rows::retain_from;
use crate::seed::{Seed, SeedBuf};

pub(crate) struct GraphLoop {
    slot: Slot,
    input: BoxOp,
    seed: SeedBuf,
    /// Range of `named_graphs` still to visit.
    pos: usize,
    end: usize,
    opened: bool,
}

impl GraphLoop {
    pub fn new(slot: Slot, input: BoxOp, width: usize) -> Self {
        GraphLoop { slot, input, seed: SeedBuf::new(width), pos: 0, end: 0, opened: false }
    }

    fn graph(cx: &ExecCtx, i: usize) -> TermId {
        cx.sh.named_graphs[i]
    }
}

impl Operator for GraphLoop {
    fn open(&mut self, cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        let named = &cx.sh.named_graphs;
        (self.pos, self.end) = match self.seed.value(self.slot) {
            Some(v) => match named.binary_search(&v) {
                Ok(i) => (i, i + 1),
                Err(_) => (0, 0),
            },
            None => (0, named.len()),
        };
        self.opened = false;
        Ok(())
    }

    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        loop {
            if self.pos >= self.end {
                return Ok(false);
            }
            if out.is_full() {
                return Ok(true);
            }
            let g = Self::graph(cx, self.pos);
            cx.graphs.push(g);
            let step = (|| -> Result<bool> {
                if !self.opened {
                    // Never seeded with Join-mode values; the substitution reaches the input.
                    self.input.open(cx, self.seed.subst_only())?;
                    self.opened = true;
                }
                let start = out.len();
                let more = self.input.next(cx, out)?;
                let slot = self.slot as usize;
                retain_from(out, start, |r| Ok(!r[slot].is_bound() || r[slot] == g))?;
                for i in start..out.len() {
                    out.row_mut(i)[slot] = g;
                }
                Ok(more)
            })();
            cx.graphs.pop();
            if !step? {
                self.pos += 1;
                self.opened = false;
            }
        }
    }

    fn close(&mut self, cx: &mut ExecCtx) {
        self.pos = self.end;
        self.input.close(cx)
    }
}
