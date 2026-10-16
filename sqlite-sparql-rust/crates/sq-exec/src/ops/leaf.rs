//! Leaves without index access: Unit, Empty, Values, GraphNames and SqlJoin. Every leaf merges
//! its seed into the rows it produces.
use sq_core::store::QuadCursor;
use sq_core::{Batch, Error, Result, Slot, TermId};

use super::Operator;
use crate::ctx::ExecCtx;
use crate::seed::{Seed, SeedBuf};

/// One solution: the seed (the empty group pattern `{}`).
pub(crate) struct Unit {
    seed: SeedBuf,
    done: bool,
}

impl Unit {
    pub fn new(width: usize) -> Self {
        Unit { seed: SeedBuf::new(width), done: true }
    }
}

impl Operator for Unit {
    fn open(&mut self, _cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        self.done = false;
        Ok(())
    }

    fn next(&mut self, _cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        if !self.done {
            if out.is_full() {
                return Ok(true);
            }
            out.push_row(self.seed.base());
            self.done = true;
        }
        Ok(false)
    }

    fn fast_count(&mut self, _cx: &mut ExecCtx, _cap: Option<u64>) -> Option<Result<u64>> {
        let n = u64::from(!self.done);
        self.done = true;
        Some(Ok(n))
    }

    fn close(&mut self, _cx: &mut ExecCtx) {
        self.done = true;
    }
}

/// No solutions.
pub(crate) struct Empty;

impl Operator for Empty {
    fn open(&mut self, _cx: &mut ExecCtx, _seed: Option<Seed<'_>>) -> Result<()> {
        Ok(())
    }

    fn next(&mut self, _cx: &mut ExecCtx, _out: &mut Batch) -> Result<bool> {
        Ok(false)
    }

    fn fast_count(&mut self, _cx: &mut ExecCtx, _cap: Option<u64>) -> Option<Result<u64>> {
        Some(Ok(0))
    }

    fn close(&mut self, _cx: &mut ExecCtx) {}
}

/// VALUES: each row that is compatible with the seed, merged with it (UNDEF = 0).
pub(crate) struct Values {
    slots: Vec<Slot>,
    rows: Vec<Vec<TermId>>,
    seed: SeedBuf,
    pos: usize,
}

impl Values {
    pub fn new(slots: Vec<Slot>, rows: Vec<Vec<TermId>>, width: usize) -> Result<Self> {
        if let Some(bad) = slots.iter().find(|s| **s as usize >= width) {
            return Err(Error::internal(format!("VALUES slot {bad} outside the row layout")));
        }
        if rows.iter().any(|r| r.len() != slots.len()) {
            return Err(Error::internal("VALUES row width differs from its slot list"));
        }
        let len = rows.len();
        Ok(Values { slots, rows, seed: SeedBuf::new(width), pos: len })
    }
}

impl Operator for Values {
    fn open(&mut self, _cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        self.pos = 0;
        Ok(())
    }

    fn next(&mut self, _cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        while self.pos < self.rows.len() {
            if out.is_full() {
                return Ok(true);
            }
            let r = &self.rows[self.pos];
            self.pos += 1;
            let compatible =
                self.slots.iter().zip(r).all(|(s, v)| !v.is_bound() || self.seed.value(*s).is_none_or(|sv| sv == *v));
            if compatible {
                out.push_row(self.seed.base());
                let i = out.len() - 1;
                let row = out.row_mut(i);
                for (s, v) in self.slots.iter().zip(r) {
                    if v.is_bound() {
                        row[*s as usize] = *v;
                    }
                }
            }
        }
        Ok(false)
    }

    fn close(&mut self, _cx: &mut ExecCtx) {
        self.pos = self.rows.len();
    }
}

/// `GRAPH ?g {}`: one row per named graph of the dataset (only the seeded value, when the slot
/// is seeded and names a graph of the dataset).
pub(crate) struct GraphNames {
    slot: Slot,
    seed: SeedBuf,
    pos: usize,
    end: usize,
}

impl GraphNames {
    pub fn new(slot: Slot, width: usize) -> Self {
        GraphNames { slot, seed: SeedBuf::new(width), pos: 0, end: 0 }
    }
}

impl Operator for GraphNames {
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
        Ok(())
    }

    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        while self.pos < self.end {
            if out.is_full() {
                return Ok(true);
            }
            out.push_row(self.seed.base());
            let i = out.len() - 1;
            out.row_mut(i)[self.slot as usize] = cx.sh.named_graphs[self.pos];
            self.pos += 1;
        }
        Ok(false)
    }

    fn fast_count(&mut self, _cx: &mut ExecCtx, _cap: Option<u64>) -> Option<Result<u64>> {
        let n = (self.end - self.pos) as u64;
        self.pos = self.end;
        Some(Ok(n))
    }

    fn close(&mut self, _cx: &mut ExecCtx) {
        self.pos = self.end;
    }
}

/// Hybrid leaf: a join pipeline rendered as one SQL statement and stepped by SQLite's VDBE
/// (`store.sql_rows`); result column i binds `out[i]`. It takes no seeds from the planner;
/// a Substitute-mode seed (inside EXISTS) is applied as a compatibility filter.
pub(crate) struct SqlJoin {
    sql: String,
    params: Vec<i64>,
    out_slots: Vec<Slot>,
    seed: SeedBuf,
    cursor: Option<Box<dyn QuadCursor>>,
    buf: Vec<TermId>,
}

impl SqlJoin {
    pub fn new(sql: String, params: Vec<i64>, out_slots: Vec<Slot>, width: usize) -> Self {
        SqlJoin { sql, params, out_slots, seed: SeedBuf::new(width), cursor: None, buf: Vec::new() }
    }
}

impl Operator for SqlJoin {
    fn open(&mut self, cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        self.cursor = Some(cx.store().sql_rows(&self.sql, &self.params, self.out_slots.len())?);
        Ok(())
    }

    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        let Some(cursor) = self.cursor.as_mut() else { return Ok(false) };
        let w = self.out_slots.len();
        loop {
            if out.is_full() {
                return Ok(true);
            }
            self.buf.clear();
            let n = cursor.fill(&mut self.buf, sq_core::BATCH_ROWS - out.len())?;
            cx.tick(n)?;
            if n == 0 {
                self.cursor = None;
                return Ok(false);
            }
            for i in 0..n {
                let vals = &self.buf[i * w..(i + 1) * w];
                let ok = self.out_slots.iter().zip(vals).all(|(s, v)| self.seed.value(*s).is_none_or(|sv| sv == *v));
                if ok {
                    out.push_row(self.seed.base());
                    let r = out.len() - 1;
                    let row = out.row_mut(r);
                    for (s, v) in self.out_slots.iter().zip(vals) {
                        row[*s as usize] = *v;
                    }
                }
            }
        }
    }

    fn close(&mut self, _cx: &mut ExecCtx) {
        self.cursor = None;
    }
}
