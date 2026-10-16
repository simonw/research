//! BindJoin (§5.3): index nested loop with sideways information passing.
//!
//! For each left batch (sorted by the seed tuple when `sort_left`), every distinct seed tuple
//! re-opens `right` with a Join-mode seed; for a Scan that is a cursor `rebind`. Runs of equal
//! tuples reuse the previous probe's buffered result. The right rows carry the seed values, so
//! they are merged into the left row without a check (the seed lists every slot both sides
//! may bind). Left{filter} evaluates the filter on the merged row and emits the left row alone
//! when nothing passes; Semi / Anti only need to know whether a probe produced a row.
//!
//! Switch to hashing ([`HashSwitch`]): the rest of the left input is hash-joined against one
//! full evaluation of `right` (keys = the seed slots), which by `safe_inject` is equivalent.
//! Adaptive joins switch when the left input turns out to exceed both 8x its estimate and
//! 50,000 rows. A `right` that mints values (BNODE, RAND, UUID, STRUUID) is never probed:
//! re-opening it per seed tuple would give one solution of it several different values. The
//! builder only lets a join switch when `right` does not rely on input orders that its seeds
//! guarantee, or builds `right` for an unseeded evaluation (see `build::Sortedness`).
use std::cmp::Ordering;

use sq_core::{Batch, Result, Slot, TermId};

use super::hash_join::{HashTable, JoinMode, Prober, keep_if};
use super::{BoxOp, Operator};
use crate::ctx::{Charge, ExecCtx};
use crate::rows::{Rows, push_merged, refill};
use crate::seed::{Seed, SeedBuf};

/// Switch to a hash join only past this many left rows ...
const ADAPTIVE_MIN_ROWS: u64 = 50_000;
/// ... and past this multiple of the left input's estimate.
const ADAPTIVE_FACTOR: f64 = 8.0;

/// When a BindJoin stops probing and hash-joins against one evaluation of `right`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum HashSwitch {
    /// Always probe.
    Never,
    /// Past [`ADAPTIVE_MIN_ROWS`] left rows and [`ADAPTIVE_FACTOR`] times this left-input
    /// estimate.
    Adaptive(f64),
    /// Before the first probe: `right` must be evaluated once per open of the join.
    Immediate,
}

impl HashSwitch {
    fn due(self, left_rows: u64) -> bool {
        match self {
            HashSwitch::Never => false,
            HashSwitch::Adaptive(est) => left_rows > ADAPTIVE_MIN_ROWS && left_rows as f64 > ADAPTIVE_FACTOR * est,
            HashSwitch::Immediate => true,
        }
    }
}

pub(crate) struct BindJoin {
    left: BoxOp,
    right: BoxOp,
    seed_slots: Vec<Slot>,
    mode: JoinMode,
    sort_left: bool,
    switch: HashSwitch,
    seed: SeedBuf,
    /// Seed row of the right probes: the substituted constants plus the seed slots.
    rseed: Vec<TermId>,
    lb: Batch,
    l_more: bool,
    /// Processing order of `lb` (sorted by seed tuple when `sort_left`).
    order: Vec<u32>,
    li: usize,
    /// Buffered result of the last probe and its seed tuple.
    run: Rows,
    run_key: Vec<TermId>,
    run_valid: bool,
    /// Emission state for the current left row.
    ri: usize,
    matched: bool,
    left_rows: u64,
    rb: Batch,
    charge: Charge,
    /// After the switch to hashing: the right side hashed on the seed slots.
    hashed: Option<Box<(HashTable, Prober)>>,
}

impl BindJoin {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        left: BoxOp,
        right: BoxOp,
        seed_slots: Vec<Slot>,
        mode: JoinMode,
        sort_left: bool,
        switch: HashSwitch,
        width: usize,
    ) -> Self {
        let n = seed_slots.len();
        BindJoin {
            left,
            right,
            seed_slots,
            mode,
            sort_left,
            switch,
            seed: SeedBuf::new(width),
            rseed: vec![TermId::UNBOUND; width],
            lb: Batch::new(width),
            l_more: true,
            order: Vec::new(),
            li: 0,
            run: Rows::new(width),
            run_key: vec![TermId::UNBOUND; n],
            run_valid: false,
            ri: 0,
            matched: false,
            left_rows: 0,
            rb: Batch::new(width),
            charge: Charge::default(),
            hashed: None,
        }
    }

    fn key_cmp(slots: &[Slot], a: &[TermId], b: &[TermId]) -> Ordering {
        for s in slots {
            let o = a[*s as usize].cmp(&b[*s as usize]);
            if o != Ordering::Equal {
                return o;
            }
        }
        Ordering::Equal
    }

    /// Pulls the next left batch and prepares its processing order; false when exhausted.
    fn next_left_batch(&mut self, cx: &mut ExecCtx) -> Result<bool> {
        if !self.l_more {
            return Ok(false);
        }
        self.l_more = refill(self.left.as_mut(), cx, &mut self.lb)?;
        self.li = 0;
        self.left_rows += self.lb.len() as u64;
        self.order.clear();
        self.order.extend(0..self.lb.len() as u32);
        if self.sort_left && self.lb.len() > 1 {
            let (lb, slots) = (&self.lb, &self.seed_slots);
            self.order.sort_by(|a, b| Self::key_cmp(slots, lb.row(*a as usize), lb.row(*b as usize)));
        }
        Ok(!self.lb.is_empty() || self.l_more)
    }

    /// Re-opens `right` for the seed tuple of left row `l` and buffers its result.
    fn probe(&mut self, cx: &mut ExecCtx, l: usize) -> Result<()> {
        let lrow = self.lb.row(l);
        for (i, s) in self.seed_slots.iter().enumerate() {
            let v = lrow[*s as usize];
            self.run_key[i] = v;
            self.rseed[*s as usize] = v;
        }
        let subst = self.seed.subst();
        self.right.open(cx, Some(Seed { row: &self.rseed, subst }))?;
        cx.release(&mut self.charge);
        self.run.clear();
        let need_one = matches!(self.mode, JoinMode::Semi | JoinMode::Anti);
        loop {
            let more = refill(self.right.as_mut(), cx, &mut self.rb)?;
            cx.charge(&mut self.charge, self.rb.len() * self.run.row_bytes())?;
            self.run.extend_from_batch(&self.rb);
            if !more || (need_one && !self.run.is_empty()) {
                break;
            }
        }
        self.run_valid = true;
        self.ri = 0;
        self.matched = false;
        Ok(())
    }

    fn same_key(&self, l: usize) -> bool {
        let lrow = self.lb.row(l);
        self.run_valid && self.seed_slots.iter().zip(&self.run_key).all(|(s, v)| lrow[*s as usize] == *v)
    }

    /// Switches to a hash join against one evaluation of `right`, starting with the batch in
    /// `lb` (not yet processed).
    fn switch_to_hash(&mut self, cx: &mut ExecCtx) -> Result<()> {
        let width = self.rseed.len();
        let mut table = HashTable::new(width, self.seed_slots.clone(), Vec::new());
        self.right.open(cx, self.seed.subst_only())?;
        table.build_from(cx, self.right.as_mut(), &mut self.rb)?;
        self.right.reset(cx);
        cx.release(&mut self.charge);
        self.run.reset();
        self.run_valid = false;
        let mut prober = Prober::new(width);
        prober.start_with(&mut self.lb, self.l_more);
        self.hashed = Some(Box::new((table, prober)));
        Ok(())
    }

    /// Emits the result of left row `l` against the buffered run; false when `out` filled up
    /// before the row was finished.
    fn emit_row(&mut self, cx: &mut ExecCtx, l: usize, out: &mut Batch) -> Result<bool> {
        match &self.mode {
            JoinMode::Inner => {
                while self.ri < self.run.len() {
                    if out.is_full() {
                        return Ok(false);
                    }
                    push_merged(out, self.lb.row(l), self.run.row(self.ri));
                    self.ri += 1;
                }
            }
            JoinMode::Left(filter) => {
                while self.ri < self.run.len() {
                    if out.is_full() {
                        return Ok(false);
                    }
                    let i = push_merged(out, self.lb.row(l), self.run.row(self.ri));
                    self.ri += 1;
                    if keep_if(filter, cx, out, i)? {
                        self.matched = true;
                    }
                }
                if !self.matched {
                    if out.is_full() {
                        return Ok(false);
                    }
                    out.push_row(self.lb.row(l));
                }
            }
            JoinMode::Semi | JoinMode::Anti => {
                let keep = self.run.is_empty() == matches!(self.mode, JoinMode::Anti);
                if keep {
                    if out.is_full() {
                        return Ok(false);
                    }
                    out.push_row(self.lb.row(l));
                }
            }
        }
        Ok(true)
    }
}

impl BindJoin {
    /// Frees the buffered probe result and the hash table of the switch to hashing.
    fn release_buffers(&mut self, cx: &ExecCtx) {
        cx.release(&mut self.charge);
        self.drop_hashed(cx);
        self.run.reset();
        self.run_valid = false;
    }

    /// Leaves the hash join of the switch, releasing its table's charge.
    fn drop_hashed(&mut self, cx: &ExecCtx) {
        if let Some(h) = self.hashed.take() {
            let (mut table, _) = *h;
            table.reset(cx);
        }
    }
}

impl Operator for BindJoin {
    fn open(&mut self, cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        // Probe seeds start from the substituted constants (inside EXISTS).
        self.rseed.fill(TermId::UNBOUND);
        for &s in self.seed.subst_slots() {
            self.rseed[s as usize] = self.seed.base()[s as usize];
        }
        self.lb.clear();
        self.l_more = true;
        self.order.clear();
        self.li = 0;
        self.left_rows = 0;
        // The run buffer keeps its capacity for the probes of this open.
        self.run.clear();
        self.run_valid = false;
        cx.release(&mut self.charge);
        self.drop_hashed(cx);
        self.left.open(cx, self.seed.full())
    }

    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        loop {
            if let Some(h) = self.hashed.as_mut() {
                let (table, prober) = &mut **h;
                return prober.next(cx, self.left.as_mut(), table, &self.mode, false, &mut [], out);
            }
            if out.is_full() {
                return Ok(true);
            }
            if self.li == self.lb.len() {
                if !self.next_left_batch(cx)? {
                    return Ok(false);
                }
                if self.switch.due(self.left_rows) {
                    self.switch_to_hash(cx)?;
                }
                continue;
            }
            let l = self.order[self.li] as usize;
            if !self.same_key(l) {
                self.probe(cx, l)?;
            } else if self.ri == usize::MAX {
                // A new left row of the same run: replay the buffered result.
                self.ri = 0;
                self.matched = false;
            }
            if !self.emit_row(cx, l, out)? {
                return Ok(true);
            }
            self.li += 1;
            self.ri = usize::MAX;
        }
    }

    fn reset(&mut self, cx: &mut ExecCtx) {
        self.release_buffers(cx);
        self.left.reset(cx);
        self.right.reset(cx);
    }

    fn close(&mut self, cx: &mut ExecCtx) {
        self.release_buffers(cx);
        self.left.close(cx);
        self.right.close(cx);
    }
}
