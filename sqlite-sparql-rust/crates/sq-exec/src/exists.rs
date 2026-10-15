//! Correlated EXISTS (§5.4, spec 18.6): `plan.exists[id]` is evaluated in SeedMode::Substitute
//! with every bound slot of the outer row that the pattern mentions, and the answer is
//! memoized per (id, active graph, values of those slots).
//!
//! The evaluation runs on its own [`ExecCtx`] level (its own `ValueCache`, the register's
//! current graph) over the shared execution state, so it can run while the outer level's
//! expression evaluation holds its `EvalCtx`. The pattern's rows carry the substituted
//! constants; a row only counts if it agrees with the outer row on them (a BIND or aggregate
//! inside the pattern may rebind a substituted slot, which then acts as a join).
use std::rc::Rc;

use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use sq_core::eval_api::ExistsEval;
use sq_core::ir::{ExistsId, Expr, GraphSel, PatTerm};
use sq_core::plan::{JoinKind, Op, Plan};
use sq_core::{BATCH_ROWS, Batch, Error, Result, Slot, SlotSet, TermId};
use sq_expr::is_present;

use crate::ctx::{Charge, ExecCtx, Shared};
use crate::ops::BoxOp;
use crate::seed::Seed;

/// One EXISTS pattern: its operator tree and evaluation buffers.
pub(crate) struct Entry {
    pub op: BoxOp,
    /// Holds `BATCH_ROWS - 1` placeholder rows, so that streaming operators stop after the
    /// first row of each `next` call (EXISTS needs one row).
    batch: Batch,
    seed: Vec<TermId>,
}

type MemoKey = (u32, TermId, SmallVec<[TermId; 6]>);

pub(crate) struct ExistsState {
    /// Operator trees, taken out while being evaluated.
    entries: Vec<Option<Entry>>,
    /// Correlation slots per pattern: every slot it mentions, nested patterns included.
    vars: Vec<Vec<Slot>>,
    memo: FxHashMap<MemoKey, bool>,
    charge: Charge,
}

impl ExistsState {
    pub fn new(ops: Vec<BoxOp>, plans: &[Plan], width: usize) -> Self {
        let vars = plan_vars(plans);
        let entries = ops
            .into_iter()
            .map(|op| {
                let mut batch = Batch::new(width);
                for _ in 0..BATCH_ROWS - 1 {
                    batch.push_unbound();
                }
                Some(Entry { op, batch, seed: vec![TermId::UNBOUND; width] })
            })
            .collect();
        ExistsState { entries, vars, memo: FxHashMap::default(), charge: Charge::default() }
    }

    /// Closes every pattern's operator tree and releases the memo.
    pub fn close(&mut self, cx: &mut ExecCtx) {
        for e in self.entries.iter_mut().flatten() {
            e.op.close(cx);
        }
        self.memo = FxHashMap::default();
        cx.sh.release(&mut self.charge);
    }
}

/// The `ExistsEval` handed to expression evaluation by [`ExecCtx::with_eval`].
pub(crate) struct ExistsHook<'a> {
    pub sh: &'a Rc<Shared>,
    pub graph: Option<TermId>,
}

impl ExistsEval for ExistsHook<'_> {
    fn exists(&mut self, id: ExistsId, row: &[TermId]) -> Result<bool> {
        evaluate(self.sh, self.graph, id, row)
    }
}

fn evaluate(sh: &Rc<Shared>, graph: Option<TermId>, id: ExistsId, row: &[TermId]) -> Result<bool> {
    let ix = id.0 as usize;
    let present = |s: &Slot| row.get(*s as usize).copied().filter(|v| is_present(*v)).unwrap_or(TermId::UNBOUND);
    let key: MemoKey = {
        let st = sh.exists.borrow();
        let vars = st.vars.get(ix).ok_or_else(|| Error::internal(format!("unknown EXISTS #{ix}")))?;
        (id.0, graph.unwrap_or(TermId::UNBOUND), vars.iter().map(present).collect())
    };
    if let Some(b) = sh.exists.borrow().memo.get(&key) {
        return Ok(*b);
    }
    let mut entry = sh.exists.borrow_mut().entries[ix]
        .take()
        .ok_or_else(|| Error::internal(format!("EXISTS #{ix} evaluated recursively")))?;
    let mut subst = SlotSet::EMPTY;
    entry.seed.fill(TermId::UNBOUND);
    for (s, v) in sh.exists.borrow().vars[ix].iter().zip(&key.2) {
        if v.is_bound() {
            entry.seed[*s as usize] = *v;
            subst.insert(*s);
        }
    }
    let mut cx = ExecCtx::new(Rc::clone(sh), sh.take_values(), graph.into_iter().collect());
    let result = run(&mut cx, &mut entry, subst);
    entry.op.close(&mut cx);
    sh.return_values(std::mem::take(&mut cx.values));
    let mut st = sh.exists.borrow_mut();
    st.entries[ix] = Some(entry);
    let found = result?;
    let bytes = 64 + 8 * key.2.len();
    st.memo.insert(key, found);
    let mut charge = std::mem::take(&mut st.charge);
    let r = sh.charge(&mut charge, bytes);
    st.charge = charge;
    r?;
    Ok(found)
}

/// Opens the pattern with the substitution and pulls until a row agrees with it.
fn run(cx: &mut ExecCtx, e: &mut Entry, subst: SlotSet) -> Result<bool> {
    let Entry { op, batch, seed } = e;
    op.open(cx, Some(Seed { row: seed, subst }))?;
    let start = batch.len();
    loop {
        let more = op.next(cx, batch)?;
        let found = (start..batch.len()).any(|i| {
            let r = batch.row(i);
            seed.iter().zip(r).all(|(s, v)| !s.is_bound() || !v.is_bound() || s == v)
        });
        batch.truncate(start);
        if found {
            return Ok(true);
        }
        if !more {
            return Ok(false);
        }
    }
}

/// Correlation slots of each EXISTS plan: every slot it mentions, including the slots of the
/// EXISTS patterns nested in its expressions.
fn plan_vars(plans: &[Plan]) -> Vec<Vec<Slot>> {
    let direct: Vec<(SlotSet, Vec<u32>)> = plans
        .iter()
        .map(|p| {
            let mut slots = SlotSet::EMPTY;
            let mut nested = Vec::new();
            collect(p, &mut slots, &mut nested);
            (slots, nested)
        })
        .collect();
    (0..plans.len())
        .map(|i| {
            let mut all = SlotSet::EMPTY;
            let mut seen = vec![false; plans.len()];
            let mut stack = vec![i];
            while let Some(j) = stack.pop() {
                if std::mem::replace(&mut seen[j], true) {
                    continue;
                }
                all = all.union(direct[j].0);
                stack.extend(direct[j].1.iter().map(|n| *n as usize).filter(|n| *n < plans.len()));
            }
            all.iter().collect()
        })
        .collect()
}

fn expr_slots(e: &Expr, slots: &mut SlotSet, nested: &mut Vec<u32>) {
    let mut v = Vec::new();
    e.slots(&mut v);
    v.into_iter().for_each(|s| slots.insert(s));
    exists_ids(e, nested);
}

/// EXISTS ids referenced by an expression.
pub(crate) fn exists_ids(e: &Expr, out: &mut Vec<u32>) {
    match e {
        Expr::Exists(id) => out.push(id.0),
        Expr::Const(_) | Expr::Slot(_) | Expr::Bound(_) => {}
        Expr::Not(a) | Expr::UnaryPlus(a) | Expr::UnaryMinus(a) => exists_ids(a, out),
        Expr::Or(a, b)
        | Expr::And(a, b)
        | Expr::Equal(a, b)
        | Expr::SameTerm(a, b)
        | Expr::Greater(a, b)
        | Expr::GreaterOrEqual(a, b)
        | Expr::Less(a, b)
        | Expr::LessOrEqual(a, b)
        | Expr::Add(a, b)
        | Expr::Subtract(a, b)
        | Expr::Multiply(a, b)
        | Expr::Divide(a, b) => {
            exists_ids(a, out);
            exists_ids(b, out);
        }
        Expr::In(a, l) => {
            exists_ids(a, out);
            l.iter().for_each(|x| exists_ids(x, out));
        }
        Expr::If(a, b, c) => {
            exists_ids(a, out);
            exists_ids(b, out);
            exists_ids(c, out);
        }
        Expr::Coalesce(l) | Expr::Call(_, l) => l.iter().for_each(|x| exists_ids(x, out)),
    }
}

fn term_slot(t: PatTerm, slots: &mut SlotSet) {
    if let PatTerm::Slot(s) = t {
        slots.insert(s);
    }
}

fn graph_slot(g: GraphSel, slots: &mut SlotSet) {
    if let GraphSel::Slot(s) = g {
        slots.insert(s);
    }
}

/// Every slot a plan subtree mentions, and the EXISTS ids of its expressions.
pub(crate) fn collect(p: &Plan, slots: &mut SlotSet, nested: &mut Vec<u32>) {
    let put = |list: &[Slot], slots: &mut SlotSet| list.iter().for_each(|s| slots.insert(*s));
    match &p.op {
        Op::Unit | Op::Empty => {}
        Op::Scan(s) => {
            graph_slot(s.pat.g, slots);
            for t in [s.pat.s, s.pat.p, s.pat.o] {
                term_slot(t, slots);
            }
        }
        Op::Values { slots: v, .. } => put(v, slots),
        Op::BindJoin { seed, kind, .. } => {
            put(seed, slots);
            if let JoinKind::Left { filter: Some(f) } = kind {
                expr_slots(f, slots, nested);
            }
        }
        Op::HashJoin { keys, check, kind, .. } => {
            put(keys, slots);
            put(check, slots);
            if let JoinKind::Left { filter: Some(f) } = kind {
                expr_slots(f, slots, nested);
            }
        }
        Op::Minus { keys, check, .. } => {
            put(keys, slots);
            put(check, slots);
        }
        Op::MergeJoin { key, check, .. } => {
            slots.insert(*key);
            put(check, slots);
        }
        Op::StarMerge { key, check, .. } => {
            slots.insert(*key);
            put(check, slots);
        }
        Op::NestedLoop { check, kind, .. } => {
            put(check, slots);
            if let JoinKind::Left { filter: Some(f) } = kind {
                expr_slots(f, slots, nested);
            }
        }
        Op::Union(_) => {}
        Op::Filter { expr, .. } => expr_slots(expr, slots, nested),
        Op::Extend { slot, expr, .. } => {
            slots.insert(*slot);
            expr_slots(expr, slots, nested);
        }
        Op::Path { g, s, o, .. } => {
            graph_slot(*g, slots);
            term_slot(*s, slots);
            term_slot(*o, slots);
        }
        Op::GraphLoop { slot, .. } | Op::GraphNames { slot } => slots.insert(*slot),
        Op::Group { keys, aggs, .. } => {
            put(keys, slots);
            for (s, a) in aggs {
                slots.insert(*s);
                if let Some(e) = &a.arg {
                    expr_slots(e, slots, nested);
                }
            }
        }
        Op::Count { out, .. } => slots.insert(*out),
        Op::Sort { keys, .. } | Op::TopK { keys, .. } => {
            for k in keys {
                expr_slots(&k.expr, slots, nested);
            }
        }
        Op::SortBySlot { key, .. } => slots.insert(*key),
        Op::Distinct { slots: v, .. } | Op::Reduced { slots: v, .. } | Op::Project { slots: v, .. } => put(v, slots),
        Op::Slice { .. } => {}
        Op::SqlJoin { out, .. } => put(out, slots),
    }
    for c in p.children() {
        collect(c, slots, nested);
    }
}
