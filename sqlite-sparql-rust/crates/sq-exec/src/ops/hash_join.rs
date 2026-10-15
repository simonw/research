//! HashJoin (§5.3) and its building blocks, shared with NestedLoop, Minus and the BindJoin
//! adaptive switch.
//!
//! The build side lives in a flat row arena with `FxHashMap<hash, chain>` heads (single-slot
//! keys hash the raw id). Keys are the certainly-bound shared slots; build rows whose key is
//! not fully bound go to a wildcard list that every probe visits, and a probe row whose key
//! is not fully bound visits every build row, so the join stays exact (SPARQL compatibility)
//! even when a key slot turns out to be unbound. Maybe-bound shared slots (`check`) are
//! compared on every candidate. With no keys at all every build row is a candidate: that is
//! the NestedLoop operator.
use std::hash::Hasher;

use rustc_hash::{FxHashMap, FxHasher};
use sq_core::plan::JoinKind;
use sq_core::{Batch, Result, Slot, TermId};
use sq_expr::CompiledExpr;

use super::{BoxOp, Operator};
use crate::ctx::{Charge, ExecCtx, POLL_ROWS};
use crate::rows::{Rows, compatible, push_merged, refill};
use crate::seed::{Seed, SeedBuf};

const NIL: u32 = u32::MAX;
/// Estimated bytes per build row besides the row itself (chain link and map share).
const ENTRY_BYTES: usize = 24;

/// A compiled `JoinKind`.
pub(crate) enum JoinMode {
    Inner,
    /// OPTIONAL: the filter is evaluated on the merged row (an error counts as false).
    Left(Option<CompiledExpr>),
    Semi,
    Anti,
}

impl JoinMode {
    pub fn compile(kind: &JoinKind, temps: &sq_core::eval_api::TempDict) -> Result<Self> {
        Ok(match kind {
            JoinKind::Inner => JoinMode::Inner,
            JoinKind::Left { filter } => {
                JoinMode::Left(filter.as_ref().map(|f| sq_expr::compile(f, temps)).transpose()?)
            }
            JoinKind::Semi => JoinMode::Semi,
            JoinKind::Anti => JoinMode::Anti,
        })
    }

    pub fn is_inner(&self) -> bool {
        matches!(self, JoinMode::Inner)
    }
}

/// Evaluates an OPTIONAL filter on row `i` of `out`; drops the row unless it passes.
pub(crate) fn keep_if(filter: &Option<CompiledExpr>, cx: &mut ExecCtx, out: &mut Batch, i: usize) -> Result<bool> {
    let pass = match filter {
        None => true,
        Some(f) => cx.with_eval(|ecx| f.ebv(out.row(i), ecx))? == Some(true),
    };
    if !pass {
        out.truncate(i);
    }
    Ok(pass)
}

/// Position in the candidate sequence of one probe row.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Cand {
    Chain(u32),
    Wild(u32),
    All(u32),
    Done,
}

/// The build side of a hash join.
pub(crate) struct HashTable {
    keys: Vec<Slot>,
    check: Vec<Slot>,
    pub rows: Rows,
    heads: FxHashMap<u64, (u32, u32)>,
    next: Vec<u32>,
    wild: Vec<u32>,
    charge: Charge,
}

impl HashTable {
    pub fn new(width: usize, keys: Vec<Slot>, check: Vec<Slot>) -> Self {
        HashTable {
            keys,
            check,
            rows: Rows::new(width),
            heads: FxHashMap::default(),
            next: Vec::new(),
            wild: Vec::new(),
            charge: Charge::default(),
        }
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Hash of the key of `row`; None when a key slot is unbound (or there are no keys).
    #[inline]
    fn hash(&self, row: &[TermId]) -> Option<u64> {
        match self.keys.as_slice() {
            [] => None,
            [k] => {
                let v = row[*k as usize];
                v.is_bound().then_some(v.0 as u64)
            }
            keys => {
                let mut h = FxHasher::default();
                for k in keys {
                    let v = row[*k as usize];
                    if !v.is_bound() {
                        return None;
                    }
                    h.write_i64(v.0);
                }
                Some(h.finish())
            }
        }
    }

    pub fn clear(&mut self, cx: &ExecCtx) {
        self.rows.clear();
        self.heads.clear();
        self.next.clear();
        self.wild.clear();
        cx.release(&mut self.charge);
    }

    /// Frees the storage.
    pub fn reset(&mut self, cx: &ExecCtx) {
        self.clear(cx);
        self.rows.reset();
        self.heads = FxHashMap::default();
        self.next = Vec::new();
        self.wild = Vec::new();
    }

    pub fn insert(&mut self, row: &[TermId]) {
        let i = self.rows.len() as u32;
        self.rows.push(row);
        self.next.push(NIL);
        match self.hash(row) {
            None => self.wild.push(i),
            Some(h) => match self.heads.get_mut(&h) {
                Some((_, last)) => {
                    self.next[*last as usize] = i;
                    *last = i;
                }
                None => {
                    self.heads.insert(h, (i, i));
                }
            },
        }
    }

    /// Drains `op` (already opened) into the table, charging the memory budget.
    pub fn build_from(&mut self, cx: &mut ExecCtx, op: &mut dyn Operator, b: &mut Batch) -> Result<()> {
        let per_row = self.rows.row_bytes() + ENTRY_BYTES;
        loop {
            let more = refill(op, cx, b)?;
            cx.sh.charge(&mut self.charge, b.len() * per_row)?;
            for i in 0..b.len() {
                self.insert(b.row(i));
            }
            if !more {
                return Ok(());
            }
        }
    }

    /// Starts the candidate sequence of a probe row.
    #[inline]
    pub fn start(&self, probe: &[TermId]) -> Cand {
        if self.keys.iter().any(|k| !probe[*k as usize].is_bound()) {
            return Cand::All(0);
        }
        match self.hash(probe).and_then(|h| self.heads.get(&h)) {
            Some((first, _)) => Cand::Chain(*first),
            None => Cand::Wild(0),
        }
    }

    /// The next build row compatible with `probe` on the keys and the check slots. Adds the
    /// number of candidates visited to `work`: a probe row that visits every build row (no
    /// keys, or an unbound key) and matches none is CPU work without output rows, which the
    /// caller charges to the interrupt-polling counter.
    #[inline]
    pub fn next_match(&self, probe: &[TermId], cur: &mut Cand, work: &mut usize) -> Option<u32> {
        loop {
            *work += 1;
            match *cur {
                Cand::Chain(i) if i != NIL => {
                    *cur = Cand::Chain(self.next[i as usize]);
                    let r = self.rows.row(i as usize);
                    let keys_eq =
                        self.keys.len() == 1 || self.keys.iter().all(|k| probe[*k as usize] == r[*k as usize]);
                    if keys_eq && compatible(probe, r, &self.check) {
                        return Some(i);
                    }
                }
                Cand::Chain(_) => *cur = Cand::Wild(0),
                Cand::Wild(j) => match self.wild.get(j as usize) {
                    Some(&i) => {
                        *cur = Cand::Wild(j + 1);
                        let r = self.rows.row(i as usize);
                        if compatible(probe, r, &self.keys) && compatible(probe, r, &self.check) {
                            return Some(i);
                        }
                    }
                    None => *cur = Cand::Done,
                },
                Cand::All(i) => {
                    if i as usize >= self.rows.len() {
                        *cur = Cand::Done;
                        continue;
                    }
                    *cur = Cand::All(i + 1);
                    let r = self.rows.row(i as usize);
                    if compatible(probe, r, &self.keys) && compatible(probe, r, &self.check) {
                        return Some(i);
                    }
                }
                Cand::Done => return None,
            }
        }
    }
}

/// The probe phase of a hash join: streams the probe input against a built table.
///
/// With `build_is_left == false` the probe rows are the left (preserved) side: Left emits a
/// probe row alone when no merged row passes the filter, Semi / Anti emit probe rows. With
/// `build_is_left == true` the build rows are the left side: matches are recorded in `marks`
/// and the build rows are emitted by the caller after the probe input is exhausted.
pub(crate) struct Prober {
    pb: Batch,
    pi: usize,
    more: bool,
    cur: Option<Cand>,
    matched: bool,
    /// Candidates visited since the last tick.
    work: usize,
}

impl Prober {
    pub fn new(width: usize) -> Self {
        Prober { pb: Batch::new(width), pi: 0, more: true, cur: None, matched: false, work: 0 }
    }

    /// Restarts with an empty probe batch.
    pub fn reset(&mut self) {
        self.pb.clear();
        self.pi = 0;
        self.more = true;
        self.cur = None;
        self.matched = false;
    }

    /// Restarts with `b` as the first probe batch (`more`: whether the input has more rows).
    pub fn start_with(&mut self, b: &mut Batch, more: bool) {
        std::mem::swap(&mut self.pb, b);
        self.pi = 0;
        self.more = more;
        self.cur = None;
        self.matched = false;
    }

    /// Returns true when `out` is full, false when the probe input is exhausted.
    #[allow(clippy::too_many_arguments)]
    pub fn next(
        &mut self,
        cx: &mut ExecCtx,
        input: &mut dyn Operator,
        table: &HashTable,
        mode: &JoinMode,
        build_is_left: bool,
        marks: &mut [bool],
        out: &mut Batch,
    ) -> Result<bool> {
        loop {
            if out.is_full() {
                return Ok(true);
            }
            if self.pi == self.pb.len() {
                if !self.more {
                    return Ok(false);
                }
                self.more = refill(input, cx, &mut self.pb)?;
                self.pi = 0;
                self.cur = None;
                continue;
            }
            let prow = self.pb.row(self.pi);
            let cur = self.cur.get_or_insert_with(|| {
                self.matched = false;
                table.start(prow)
            });
            match (mode, build_is_left) {
                (JoinMode::Inner, _) => {
                    while let Some(b) = table.next_match(prow, cur, &mut self.work) {
                        push_merged(out, prow, table.rows.row(b as usize));
                        if out.is_full() {
                            return Ok(true);
                        }
                    }
                }
                (JoinMode::Left(filter), false) => {
                    while let Some(b) = table.next_match(prow, cur, &mut self.work) {
                        let i = push_merged(out, prow, table.rows.row(b as usize));
                        if keep_if(filter, cx, out, i)? {
                            self.matched = true;
                            if out.is_full() {
                                return Ok(true);
                            }
                        }
                    }
                    if !self.matched {
                        if out.is_full() {
                            return Ok(true);
                        }
                        out.push_row(prow);
                    }
                }
                (JoinMode::Semi, false) => {
                    if table.next_match(prow, cur, &mut self.work).is_some() {
                        out.push_row(prow);
                    }
                }
                (JoinMode::Anti, false) => {
                    if table.next_match(prow, cur, &mut self.work).is_none() {
                        out.push_row(prow);
                    }
                }
                (JoinMode::Left(filter), true) => {
                    while let Some(b) = table.next_match(prow, cur, &mut self.work) {
                        let i = push_merged(out, prow, table.rows.row(b as usize));
                        if keep_if(filter, cx, out, i)? {
                            marks[b as usize] = true;
                            if out.is_full() {
                                return Ok(true);
                            }
                        }
                    }
                }
                (JoinMode::Semi | JoinMode::Anti, true) => {
                    while let Some(b) = table.next_match(prow, cur, &mut self.work) {
                        marks[b as usize] = true;
                    }
                }
            }
            self.pi += 1;
            self.cur = None;
            tick_work(cx, &mut self.work)?;
        }
    }
}

/// Charges accumulated probe work to the interrupt-polling counter once it amounts to a
/// polling interval.
#[inline]
pub(crate) fn tick_work(cx: &ExecCtx, work: &mut usize) -> Result<()> {
    if *work >= POLL_ROWS as usize { cx.tick(std::mem::take(work)) } else { Ok(()) }
}

/// `HashJoin { build, probe, keys, check, kind, build_is_left }`; also NestedLoop (no keys,
/// the right side built, `check` = every shared slot).
pub(crate) struct HashJoin {
    build: BoxOp,
    probe: BoxOp,
    table: HashTable,
    mode: JoinMode,
    build_is_left: bool,
    seed: SeedBuf,
    built: bool,
    prober: Prober,
    marks: Vec<bool>,
    /// Final phase (left-side build of Left / Semi / Anti): next build row to consider.
    emit_pos: usize,
    done: bool,
    buf: Batch,
}

impl HashJoin {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        build: BoxOp,
        probe: BoxOp,
        keys: Vec<Slot>,
        check: Vec<Slot>,
        mode: JoinMode,
        build_is_left: bool,
        width: usize,
    ) -> Self {
        HashJoin {
            build,
            probe,
            table: HashTable::new(width, keys, check),
            // Inner joins have no preserved side: always probe-major.
            build_is_left: build_is_left && !mode.is_inner(),
            mode,
            seed: SeedBuf::new(width),
            built: false,
            prober: Prober::new(width),
            marks: Vec::new(),
            emit_pos: 0,
            done: true,
            buf: Batch::new(width),
        }
    }

    fn build_table(&mut self, cx: &mut ExecCtx) -> Result<()> {
        // Join-mode seeds reach both inputs of an inner join, only the left one otherwise.
        let left_seed = self.seed.full();
        let right_seed = if self.mode.is_inner() { self.seed.full() } else { self.seed.subst_only() };
        let (bseed, pseed) = if self.build_is_left { (left_seed, right_seed) } else { (right_seed, left_seed) };
        self.table.clear(cx);
        self.build.open(cx, bseed)?;
        self.table.build_from(cx, self.build.as_mut(), &mut self.buf)?;
        self.build.reset(cx);
        let n = self.table.len();
        if self.build_is_left {
            self.marks.clear();
            self.marks.resize(n, false);
        }
        self.probe.open(cx, pseed)?;
        self.prober.reset();
        self.built = true;
        Ok(())
    }
}

impl HashJoin {
    fn release_table(&mut self, cx: &ExecCtx) {
        self.table.reset(cx);
        self.marks = Vec::new();
        self.built = false;
        self.done = true;
    }
}

impl Operator for HashJoin {
    fn open(&mut self, _cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        self.built = false;
        self.done = false;
        self.emit_pos = 0;
        Ok(())
    }

    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        if self.done {
            return Ok(false);
        }
        if !self.built {
            self.build_table(cx)?;
            let nothing = self.build_is_left || matches!(self.mode, JoinMode::Inner | JoinMode::Semi);
            if self.table.len() == 0 && nothing {
                // An empty build side that can produce no row: skip the probe input.
                self.done = true;
                self.probe.reset(cx);
                return Ok(false);
            }
        }
        let HashJoin { probe, table, mode, build_is_left, prober, marks, .. } = self;
        if prober.next(cx, probe.as_mut(), table, mode, *build_is_left, marks, out)? {
            return Ok(true);
        }
        if self.build_is_left {
            // Emit the left (build) rows the join kind keeps.
            while self.emit_pos < self.table.len() {
                if out.is_full() {
                    return Ok(true);
                }
                let b = self.emit_pos;
                self.emit_pos += 1;
                let keep = match self.mode {
                    JoinMode::Semi => self.marks[b],
                    _ => !self.marks[b],
                };
                if keep {
                    out.push_row(self.table.rows.row(b));
                }
            }
        }
        self.done = true;
        Ok(false)
    }

    fn reset(&mut self, cx: &mut ExecCtx) {
        self.release_table(cx);
        self.build.reset(cx);
        self.probe.reset(cx);
    }

    fn close(&mut self, cx: &mut ExecCtx) {
        self.release_table(cx);
        self.build.close(cx);
        self.probe.close(cx);
    }
}
