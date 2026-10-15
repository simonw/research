//! GROUP BY / aggregates (§5.3, §6.7) and the COUNT(*) operator.
//!
//! Hash aggregation keyed by `SmallVec<[TermId; 2]>` (groups are emitted in first-seen
//! order), or streaming when the input is sorted by its single key. A GROUP BY expression
//! leaves `TermId::ERROR` in its key slot for an error and an unbound result alike (one group,
//! §18.5.1), output as UNBOUND. Without keys an empty input still yields one row; with keys it
//! yields none.
//!
//! DISTINCT aggregates are deduplicated here, in a per-group `FxHashSet` in front of an
//! accumulator built without DISTINCT, so that the sets are charged against the memory budget
//! like the groups themselves; GROUP_CONCAT charges the strings it appends.
use std::rc::Rc;

use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;
use sq_core::ir::{AggFunc, AggSpec};
use sq_core::{BATCH_ROWS, Batch, Error, Result, Slot, Term, TermId};
use sq_expr::{Accumulator, CompiledExpr, EvalCtx, accumulator};

use super::{BoxOp, Operator};
use crate::ctx::{Charge, ExecCtx, Shared};
use crate::seed::{Seed, SeedBuf};

type Key = SmallVec<[TermId; 2]>;

/// Estimated bytes per group and aggregate (accumulator box, map entry share).
const GROUP_BYTES: usize = 96;
/// Bytes per value held by a DISTINCT aggregate's set (the id and its table share).
const DISTINCT_ID_BYTES: usize = 16;
/// Bytes per COUNT(DISTINCT *) solution besides its ids (boxed slice and table share).
const DISTINCT_ROW_BYTES: usize = 32;

/// One aggregate: output slot, spec and compiled argument.
pub(crate) struct Agg {
    pub slot: Slot,
    distinct: bool,
    /// The spec the accumulators are built from: without DISTINCT (deduplicated here) and
    /// without argument (evaluated here).
    acc_spec: AggSpec,
    arg: Option<CompiledExpr>,
}

impl Agg {
    pub fn new(slot: Slot, spec: &AggSpec, arg: Option<CompiledExpr>) -> Self {
        let acc_spec = AggSpec { func: spec.func.clone(), distinct: false, arg: None };
        Agg { slot, distinct: spec.distinct, acc_spec, arg }
    }

    fn state(&self) -> AggState {
        let seen = match (self.distinct, &self.acc_spec.func) {
            (false, _) => Seen::All,
            (true, AggFunc::CountAll) => Seen::Rows(FxHashSet::default()),
            (true, _) => Seen::Ids(FxHashSet::default()),
        };
        AggState { acc: accumulator(&self.acc_spec), seen }
    }
}

/// One aggregate of one group.
struct AggState {
    acc: Box<dyn Accumulator>,
    seen: Seen,
}

/// What a DISTINCT aggregate has passed to its accumulator.
enum Seen {
    /// Not DISTINCT: every value is passed.
    All,
    /// The distinct argument ids (unbound and error values always pass, as in sq-expr).
    Ids(FxHashSet<TermId>),
    /// COUNT(DISTINCT *): the distinct rows projected onto the visible slots.
    Rows(FxHashSet<Box<[TermId]>>),
}

fn new_states(aggs: &[Agg], into: &mut Vec<AggState>) {
    into.extend(aggs.iter().map(Agg::state));
}

/// Length of STR(id) (what GROUP_CONCAT appends); 0 for what it does not append.
fn str_len(id: TermId, ecx: &mut EvalCtx<'_>) -> Result<usize> {
    if !id.is_bound() || id == TermId::ERROR {
        return Ok(0);
    }
    Ok(match &*ecx.resolver.term(id)? {
        Term::Iri(s) => s.len(),
        Term::Literal(l) => l.lex.len(),
        Term::BNode(_) => 0,
    })
}

/// Feeds rows to the aggregates of one group, charging what their state grows by.
struct Feeder<'a> {
    aggs: &'a [Agg],
    sh: &'a Shared,
    charge: &'a mut Charge,
    /// The projection of a COUNT(DISTINCT *) row (reused).
    solution: &'a mut Vec<TermId>,
}

impl Feeder<'_> {
    /// One input row: COUNT(*) takes the row (COUNT(DISTINCT *) dedupes it on the visible
    /// slots), every other aggregate its argument evaluated with canonical ids (None for
    /// unbound / error).
    fn feed(&mut self, states: &mut [AggState], row: &[TermId], ecx: &mut EvalCtx<'_>) -> Result<()> {
        let visible = &self.sh.visible;
        for (a, st) in self.aggs.iter().zip(states.iter_mut()) {
            if a.acc_spec.func == AggFunc::CountAll {
                if let Seen::Rows(seen) = &mut st.seen {
                    self.solution.clear();
                    self.solution
                        .extend(visible.iter().map(|s| row.get(*s as usize).copied().unwrap_or(TermId::UNBOUND)));
                    if seen.contains(&self.solution[..]) {
                        continue;
                    }
                    self.sh.charge(self.charge, DISTINCT_ROW_BYTES + 8 * self.solution.len())?;
                    seen.insert(self.solution[..].into());
                }
                st.acc.add_row(row, visible, ecx)?;
                continue;
            }
            let v = match &a.arg {
                Some(e) => e.eval(row, ecx, true)?,
                None => None,
            };
            if let (Seen::Ids(seen), Some(id)) = (&mut st.seen, v) {
                if !seen.insert(id) {
                    continue;
                }
                self.sh.charge(self.charge, DISTINCT_ID_BYTES)?;
            }
            if let (AggFunc::GroupConcat { separator }, Some(id)) = (&a.acc_spec.func, v) {
                let bytes = str_len(id, ecx)? + separator.len();
                self.sh.charge(self.charge, bytes)?;
            }
            st.acc.add(v, ecx)?;
        }
        Ok(())
    }
}

/// Appends the output row of a group: keys (ERROR -> UNBOUND), aggregate results, and the
/// substituted constants of an EXISTS evaluation.
fn emit(
    keys: &[Slot],
    aggs: &[Agg],
    seed: &SeedBuf,
    key: &[TermId],
    states: &mut [AggState],
    cx: &mut ExecCtx,
    out: &mut Batch,
) -> Result<()> {
    out.push_unbound();
    let i = out.len() - 1;
    for (s, v) in keys.iter().zip(key) {
        out.row_mut(i)[*s as usize] = if *v == TermId::ERROR { TermId::UNBOUND } else { *v };
    }
    cx.with_eval(|ecx| -> Result<()> {
        for (a, st) in aggs.iter().zip(states.iter_mut()) {
            out.row_mut(i)[a.slot as usize] = st.acc.finish(ecx)?.unwrap_or(TermId::UNBOUND);
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
    /// COUNT(DISTINCT *) projection of the current row (reused).
    solution: Vec<TermId>,
    // hash mode
    index: FxHashMap<Key, u32>,
    order: Vec<Key>,
    states: Vec<AggState>,
    emit_pos: usize,
    built: bool,
    // streaming mode
    in_pos: usize,
    in_more: bool,
    cur: Option<TermId>,
    cur_states: Vec<AggState>,
    /// The groups and aggregate state (hash mode), or the current group's state (streaming).
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
            solution: Vec::new(),
            index: FxHashMap::default(),
            order: Vec::new(),
            states: Vec::new(),
            emit_pos: 0,
            built: false,
            in_pos: 0,
            in_more: true,
            cur: None,
            cur_states: Vec::new(),
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
            let Group { keys, aggs, inb, scratch, solution, index, order, states, charge, .. } = self;
            cx.with_eval(|ecx| -> Result<()> {
                let mut feeder = Feeder { aggs, sh: &sh, charge, solution };
                for r in 0..inb.len() {
                    let row = inb.row(r);
                    scratch.clear();
                    scratch.extend(keys.iter().map(|s| row[*s as usize]));
                    let g = match index.get(&scratch[..]) {
                        Some(g) => *g as usize,
                        None => {
                            sh.charge(feeder.charge, GROUP_BYTES * n.max(1) + 8 * scratch.len())?;
                            let g = order.len();
                            index.insert(scratch.clone(), g as u32);
                            order.push(scratch.clone());
                            new_states(aggs, states);
                            g
                        }
                    };
                    feeder.feed(&mut states[g * n..(g + 1) * n], row, ecx)?;
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
            new_states(&self.aggs, &mut self.states);
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
            emit(&self.keys, &self.aggs, &self.seed, key, &mut self.states[g * n..(g + 1) * n], cx, out)?;
        }
        Ok(false)
    }

    /// Emits the current group of the streaming mode and releases its state.
    fn emit_current(&mut self, cx: &mut ExecCtx, key: TermId, out: &mut Batch) -> Result<()> {
        emit(&self.keys, &self.aggs, &self.seed, &[key], &mut self.cur_states, cx, out)?;
        self.cur_states.clear();
        cx.release(&mut self.charge);
        Ok(())
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
                        self.emit_current(cx, k, out)?;
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
                    self.emit_current(cx, k, out)?;
                    self.cur = None;
                    continue;
                }
                Some(_) => {}
                None => {
                    self.cur = Some(v);
                    new_states(&self.aggs, &mut self.cur_states);
                }
            }
            let Group { aggs, inb, in_pos, cur_states, solution, charge, .. } = self;
            cx.with_eval(|ecx| -> Result<()> {
                let mut feeder = Feeder { aggs, sh: &sh, charge, solution };
                while *in_pos < inb.len() && inb.get(*in_pos, key_slot) == v {
                    feeder.feed(cur_states, inb.row(*in_pos), ecx)?;
                    *in_pos += 1;
                }
                Ok(())
            })?;
        }
    }
}

impl Group {
    /// Frees the groups and their aggregate states.
    fn release(&mut self, cx: &ExecCtx) {
        self.index = FxHashMap::default();
        self.order = Vec::new();
        self.states = Vec::new();
        self.cur_states = Vec::new();
        cx.release(&mut self.charge);
    }
}

impl Operator for Group {
    fn open(&mut self, cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        self.index.clear();
        self.order.clear();
        self.states.clear();
        self.emit_pos = 0;
        self.built = false;
        self.inb.clear();
        self.in_pos = 0;
        self.in_more = true;
        self.cur = None;
        self.cur_states.clear();
        cx.release(&mut self.charge);
        self.input.open(cx, self.seed.full())
    }

    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        if self.streaming { self.next_streaming(cx, out) } else { self.next_hash(cx, out) }
    }

    fn reset(&mut self, cx: &mut ExecCtx) {
        self.release(cx);
        self.input.reset(cx)
    }

    fn close(&mut self, cx: &mut ExecCtx) {
        self.release(cx);
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
        let n = match self.input.fast_count(cx, None) {
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
        self.input.reset(cx);
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

    fn fast_count(&mut self, _cx: &mut ExecCtx, _cap: Option<u64>) -> Option<Result<u64>> {
        let n = u64::from(!self.done);
        self.done = true;
        Some(Ok(n))
    }

    fn reset(&mut self, cx: &mut ExecCtx) {
        self.done = true;
        self.input.reset(cx)
    }

    fn close(&mut self, cx: &mut ExecCtx) {
        self.done = true;
        self.input.close(cx)
    }
}
