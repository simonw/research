//! Physical planning (spec §5.4, §7.5): Logical (after rewrites) -> `sq_core::plan::Plan`.
//!
//! Correctness-first planner: join groups are ordered by dynamic programming over connected
//! subsets when the search is small, greedily otherwise (cheapest connected next unit, cross
//! products last; see `join`), and each step picks BindJoin / HashJoin / NestedLoop by cost and
//! `JoinPolicy`. Estimates are clamped to [`MAX_ESTIMATE`].
//!
//! Seed propagation contract (what `validate()` checks and the executor must implement):
//! a subtree opened with a Join-mode seed S passes S to every input of an inner join (and the
//! left input of a BindJoin, whose `seed` list then contains S ∩ maybe(right)), to every UNION
//! branch, to the LEFT input only of Left / Semi / Anti joins and Minus, and to the input of
//! Filter / Extend / Project / Distinct / Reduced / Sort / Group. Slice, TopK, Count, GraphLoop
//! and GraphNames are never seeded. A BindJoin re-opens its right input with exactly its
//! `seed` slots; every slot both sides may bind is in that list, so merging needs no check.
//!
//! Which seeded slots a row actually holds is [`carried`]: leaves (Unit, Scan, Values, Path)
//! merge the seed into their rows, Project / Group keep it on their output slots only, and
//! the never-seeded operators drop it. The planner reads a seeded slot from a join input (as a
//! BindJoin seed or a HashJoin key) only where that input carries it, so no other operator has
//! to re-merge the seed: the BindJoin that opened the subtree merges its output into the left
//! row, which holds every seed value. Slots seeded on both inputs of an inner join hold the seed
//! value or nothing on each side and are never compared.
//!
//! Planning effort stays near-linear in the query size: a node's plan depends only on the
//! bound slots it mentions (the memo key is normalized to them), and the join ordering search
//! costs opaque units from estimates (`estimate`) instead of planning them for every seed set
//! it probes. EXISTS substitution hints are read off the emitted plans (`exists_hints`).
//!
//! Deferred to the optimizer package (structured so they slot in as extra join choices /
//! post-passes): MergeJoin, StarMerge, SortBySlot, SqlJoin rendering, IntRange pushdown,
//! interesting-order DP and constant calibration.
mod bits;
mod estimate;
mod finish;
mod join;
mod scan;
mod semi;

pub(crate) use finish::{canonical_needed, node_exprs, walk};
pub(crate) use scan::{bound_columns, scan_sorted_by};

use rustc_hash::FxHashMap;
use sq_core::batch::{Slot, SlotSet};
use sq_core::ir::{
    AggFunc, DatasetSpec, ExistsId, GraphSel, Logical, LoweredQuery, NamedGraphs, OrderKey, PatTerm, PathPlan,
    QuadPat, QueryForm,
};
use sq_core::plan::{JoinKind, Op, Plan, QueryOptions, QueryPlan};
use sq_core::stats::CostConstants;
use sq_core::store::QuadStore;
use sq_core::{Error, Result, TermId};

use crate::card::Card;
use crate::expr;
use crate::props::{slot_list, LogicalProps, PlanProps};
use crate::rewrite::Rewritten;
use estimate::Est;

/// Per-row cost of evaluating a scalar expression (ns).
const EXPR_EVAL_NS: f64 = 25.0;

/// Planning context of a subtree.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub(crate) struct Ctx {
    /// Slots bound by a Join-mode seed whenever this subtree is opened.
    pub seeded: SlotSet,
    /// Slots an EXISTS evaluation substitutes (hints for index choice only).
    pub subst: SlotSet,
    /// Planning an EXISTS pattern: its scans may re-pick orders at run time, so no sortedness
    /// is ever claimed.
    pub in_exists: bool,
}

impl Ctx {
    pub(crate) fn bound(&self) -> SlotSet {
        self.seeded.union(self.subst)
    }
    /// The same context without Join-mode seeds (inputs that never receive the seed).
    pub(crate) fn unseeded(&self) -> Ctx {
        Ctx { seeded: SlotSet::EMPTY, ..*self }
    }
}

/// Upper bound of row and cost estimates. Products of large estimates (cross products of many
/// patterns) saturate here instead of overflowing to infinity: plans must carry finite
/// estimates (`validate()`), and the ordering search compares and adds them.
pub(crate) const MAX_ESTIMATE: f64 = 1e300;

/// `x` as a usable estimate: within [0, MAX_ESTIMATE]; NaN (from inf - inf or 0 * inf of
/// unclamped intermediates) counts as the maximum.
pub(crate) fn clamp_est(x: f64) -> f64 {
    if x.is_nan() { MAX_ESTIMATE } else { x.clamp(0.0, MAX_ESTIMATE) }
}

pub(crate) fn mk(op: Op, est_rows: f64, est_cost_ns: f64, sorted_by: Option<Slot>) -> Plan {
    Plan { id: 0, est_rows: clamp_est(est_rows), est_cost_ns: clamp_est(est_cost_ns), sorted_by, op }
}

/// The slots of a Join-mode seed `s` that are bound in every output row of `p` when `p` is
/// opened with `s`, under the seed propagation contract above. Always a subset of `s`.
pub(crate) fn carried(p: &Plan, s: SlotSet) -> SlotSet {
    if s.is_empty() {
        return s;
    }
    let set = |slots: &[Slot]| SlotSet::from_slots(slots.iter().copied());
    // Inner joins keep what either seeded input keeps; the other kinds keep the left rows only.
    let join = |left: &Plan, right: &Plan, right_seed: SlotSet, kind: &JoinKind| {
        let l = carried(left, s);
        if *kind == JoinKind::Inner { l.union(carried(right, right_seed).intersect(s)) } else { l }
    };
    match &p.op {
        Op::Unit | Op::Empty | Op::Scan(_) | Op::Values { .. } | Op::Path { .. } => s,
        Op::Slice { .. }
        | Op::TopK { .. }
        | Op::Count { .. }
        | Op::GraphLoop { .. }
        | Op::GraphNames { .. }
        | Op::SqlJoin { .. } => SlotSet::EMPTY,
        Op::Filter { input, .. }
        | Op::Extend { input, .. }
        | Op::Distinct { input, .. }
        | Op::Reduced { input, .. }
        | Op::Sort { input, .. }
        | Op::SortBySlot { input, .. } => carried(input, s),
        Op::Project { input, slots } => carried(input, s).intersect(set(slots)),
        Op::Group { input, keys, .. } => carried(input, s).intersect(set(keys)),
        Op::Union(branches) => branches.iter().fold(s, |acc, b| acc.intersect(carried(b, s))),
        Op::BindJoin { left, right, seed, kind, .. } => join(left, right, set(seed), kind),
        Op::HashJoin { build, probe, kind, build_is_left, .. } => {
            let (l, r) = if *build_is_left { (build, probe) } else { (probe, build) };
            join(l, r, s, kind)
        }
        Op::NestedLoop { left, right, kind, .. } => join(left, right, s, kind),
        Op::MergeJoin { left, right, .. } => carried(left, s).union(carried(right, s)),
        Op::StarMerge { inputs, .. } => inputs.iter().fold(SlotSet::EMPTY, |acc, i| acc.union(carried(i, s))),
        Op::Minus { left, .. } => carried(left, s),
    }
}

/// Every input of `p` with the Join-mode seed it is opened with when `p` is opened with `s`
/// (the seed propagation contract above).
pub(crate) fn input_seeds(p: &Plan, s: SlotSet) -> Vec<(&Plan, SlotSet)> {
    let inner = |kind: &JoinKind| if *kind == JoinKind::Inner { s } else { SlotSet::EMPTY };
    match &p.op {
        Op::BindJoin { left, right, seed, .. } => vec![(left, s), (right, SlotSet::from_slots(seed.iter().copied()))],
        Op::HashJoin { build, probe, kind, build_is_left, .. } => {
            let (l, r) = if *build_is_left { (build, probe) } else { (probe, build) };
            vec![(l, s), (r, inner(kind))]
        }
        Op::NestedLoop { left, right, kind, .. } => vec![(left, s), (right, inner(kind))],
        Op::Minus { left, right, .. } => vec![(left, s), (right, SlotSet::EMPTY)],
        Op::Slice { input, .. } | Op::TopK { input, .. } | Op::Count { input, .. } | Op::GraphLoop { input, .. } => {
            vec![(input, SlotSet::EMPTY)]
        }
        _ => p.children().into_iter().map(|c| (c, s)).collect(),
    }
}

pub(crate) struct Planner<'a> {
    opts: &'a QueryOptions,
    dataset: &'a DatasetSpec,
    costs: &'a CostConstants,
    card: Card<'a>,
    lp: LogicalProps<'a>,
    /// The rewritten EXISTS patterns (decorrelation plans them as join inputs).
    exists_pats: &'a [Logical],
    /// Row width (number of slots), for row-copy costs.
    width: f64,
    /// Plans per (node address, context normalized to the node's slots).
    memo: FxHashMap<(usize, Ctx), Plan>,
    scan_memo: FxHashMap<(QuadPat, Ctx), Plan>,
    /// Ordering-search estimates per (node address, normalized context).
    est_memo: FxHashMap<(usize, Ctx), Est>,
    /// `vars` per node address (the logical trees outlive the planner and never move).
    vars: FxHashMap<usize, SlotSet>,
}

impl<'a> Planner<'a> {
    fn new(
        store: &'a dyn QuadStore,
        opts: &'a QueryOptions,
        dataset: &'a DatasetSpec,
        named_graphs: usize,
        exists: &'a [Logical],
        width: usize,
    ) -> Self {
        Planner {
            opts,
            dataset,
            costs: store.costs(),
            card: Card::new(store, dataset, named_graphs, opts),
            lp: LogicalProps::new(exists),
            exists_pats: exists,
            width: width.max(1) as f64,
            memo: FxHashMap::default(),
            scan_memo: FxHashMap::default(),
            est_memo: FxHashMap::default(),
            vars: FxHashMap::default(),
        }
    }

    fn row_copy(&self) -> f64 {
        self.costs.row_copy_per_slot * self.width
    }

    /// The memo key of `l` in `ctx`: bound slots `l` never mentions cannot change its plan
    /// (seeds outside `vars` are merged, not read), so they are dropped from the context.
    fn key(&mut self, l: &Logical, ctx: Ctx) -> (usize, Ctx) {
        let addr = l as *const Logical as usize;
        let vars = match self.vars.get(&addr) {
            Some(v) => *v,
            None => {
                let v = self.lp.props(l).vars;
                self.vars.insert(addr, v);
                v
            }
        };
        (addr, Ctx { seeded: ctx.seeded.intersect(vars), subst: ctx.subst.intersect(vars), ..ctx })
    }

    /// Plan `l` for context `ctx` (memoized per node and normalized context).
    pub(crate) fn plan(&mut self, l: &Logical, ctx: Ctx) -> Result<Plan> {
        let key = self.key(l, ctx);
        if let Some(p) = self.memo.get(&key) {
            return Ok(p.clone());
        }
        let p = self.plan_node(l, key.1)?;
        self.memo.insert(key, p.clone());
        Ok(p)
    }

    fn plan_node(&mut self, l: &Logical, ctx: Ctx) -> Result<Plan> {
        Ok(match l {
            Logical::Unit => mk(Op::Unit, 1.0, 0.0, None),
            Logical::Empty => mk(Op::Empty, 0.0, 0.0, None),
            Logical::Service { silent: true } => mk(Op::Unit, 1.0, 0.0, None),
            Logical::Service { silent: false } => {
                return Err(Error::Unsupported("SERVICE (federated query); use SERVICE SILENT".into()));
            }
            Logical::Bgp(_) | Logical::Join(..) | Logical::Filter { .. } => self.plan_group(l, ctx)?,
            Logical::Path { g, s, path, o } => self.plan_path(*g, *s, path, *o, ctx),
            Logical::Values { slots, rows } => self.plan_values(slots, rows, ctx),
            Logical::LeftJoin { left, right, filter } => self.plan_left_join(left, right, filter.as_ref(), ctx)?,
            Logical::Minus { left, right } => self.plan_minus(left, right, ctx)?,
            Logical::Union(..) => {
                let mut branches = Vec::new();
                flatten_union(l, &mut branches);
                let plans = branches.into_iter().map(|b| self.plan(b, ctx)).collect::<Result<Vec<_>>>()?;
                let rows = plans.iter().map(|p| p.est_rows).sum();
                let cost = plans.iter().map(|p| p.est_cost_ns).sum();
                mk(Op::Union(plans), rows, cost, None)
            }
            Logical::Extend { input, slot, expr } => {
                let input = self.plan(input, ctx)?;
                let (rows, cost) = (input.est_rows, input.est_cost_ns + input.est_rows * EXPR_EVAL_NS);
                let sorted = input.sorted_by.filter(|s| s != slot);
                let op = Op::Extend { input: Box::new(input), slot: *slot, expr: expr.clone(), canonical: true };
                mk(op, rows, cost, sorted)
            }
            Logical::Graph { name: PatTerm::Slot(g), input } => self.plan_graph(*g, input, ctx)?,
            Logical::Graph { name: PatTerm::Const(_), .. } => {
                return Err(Error::internal("GRAPH <iri> must be resolved before physical planning"));
            }
            Logical::Group { input, keys, aggs } => {
                // COUNT(*) without DISTINCT / GROUP BY: the Count operator (never seeded).
                let count_out = match aggs.as_slice() {
                    [(out, a)] if keys.is_empty() && a.func == AggFunc::CountAll && !a.distinct => Some(*out),
                    _ => None,
                };
                if let Some(out) = count_out {
                    let p = self.plan(input, ctx.unseeded())?;
                    let cost = p.est_cost_ns + p.est_rows * self.costs.vdbe_count_row;
                    mk(Op::Count { input: Box::new(p), out }, 1.0, cost, None)
                } else {
                    let p = self.plan(input, ctx)?;
                    let cost = p.est_cost_ns + p.est_rows * (self.costs.hash_probe_small + EXPR_EVAL_NS);
                    let rows = if keys.is_empty() { 1.0 } else { (p.est_rows / 2.0).max(1.0) };
                    let op = Op::Group { input: Box::new(p), keys: keys.clone(), aggs: aggs.clone(), streaming: false };
                    mk(op, rows, cost, None)
                }
            }
            Logical::OrderBy { input, keys } => {
                let p = self.plan(input, ctx)?;
                let (rows, n) = (p.est_rows, p.est_rows.max(1.0));
                let cost = p.est_cost_ns + n * n.log2().max(1.0) * self.costs.sort_row_log + n * self.costs.decode_hit;
                mk(Op::Sort { input: Box::new(p), keys: keys.clone() }, rows, cost, None)
            }
            Logical::Project { input, slots } => {
                let p = self.plan(input, ctx)?;
                let sorted = p.sorted_by.filter(|s| slots.contains(s));
                let (rows, cost) = (p.est_rows, p.est_cost_ns);
                mk(Op::Project { input: Box::new(p), slots: slots.clone() }, rows, cost, sorted)
            }
            Logical::Distinct(input) | Logical::Reduced(input) => {
                let p = self.plan(input, ctx)?;
                let slots = slot_list(self.lp.props(input).maybe);
                let hash = if p.est_rows < 32_768.0 { self.costs.hash_build_small } else { self.costs.hash_build_large };
                let (rows, cost, sorted) = (p.est_rows, p.est_cost_ns + p.est_rows * hash, p.sorted_by);
                let op = if matches!(l, Logical::Distinct(_)) {
                    Op::Distinct { input: Box::new(p), slots }
                } else {
                    Op::Reduced { input: Box::new(p), slots }
                };
                mk(op, rows, cost, sorted)
            }
            Logical::Slice { input, offset, limit } => self.plan_slice(input, *offset, *limit, ctx)?,
        })
    }

    fn plan_values(&mut self, slots: &[Slot], rows: &[Vec<TermId>], ctx: Ctx) -> Plan {
        let (est, cost) = self.values_estimate(slots, rows, ctx.bound());
        mk(Op::Values { slots: slots.to_vec(), rows: rows.to_vec() }, est, cost, None)
    }

    /// Rows and cost of one open of a VALUES block with `bound` slots bound: rows per distinct
    /// value of the most selective bound slot.
    fn values_estimate(&self, slots: &[Slot], rows: &[Vec<TermId>], bound: SlotSet) -> (f64, f64) {
        let n = rows.len() as f64;
        let mut est = n;
        for (i, s) in slots.iter().enumerate() {
            if bound.contains(*s) {
                let mut vals: Vec<TermId> = rows.iter().map(|r| r[i]).collect();
                vals.sort();
                vals.dedup();
                est = est.min(n / vals.len().max(1) as f64);
            }
        }
        (est, n * self.row_copy())
    }

    fn plan_path(&mut self, g: GraphSel, s: PatTerm, path: &PathPlan, o: PatTerm, ctx: Ctx) -> Plan {
        let (rows, cost) = self.path_estimate(g, s, path, o, ctx.bound());
        mk(Op::Path { g, s, path: path.clone(), o }, rows, cost, None)
    }

    /// Rows and cost of one open of a property path with `bound` slots bound.
    fn path_estimate(&self, g: GraphSel, s: PatTerm, path: &PathPlan, o: PatTerm, bound: SlotSet) -> (f64, f64) {
        let is_bound = |t: PatTerm| match t {
            PatTerm::Const(_) => true,
            PatTerm::Slot(x) => bound.contains(x),
        };
        let mut rows = self.card.path_rows(path, is_bound(s), is_bound(o));
        if let GraphSel::Slot(gs) = g {
            if !bound.contains(gs) {
                rows *= self.card.named_count();
            }
        }
        (rows, 2.0 * self.costs.seek_random + rows * 3.0 * self.costs.scan_row)
    }

    /// `GRAPH ?g { P }` that was not pushed: GraphNames for an empty P, otherwise GraphLoop.
    fn plan_graph(&mut self, g: Slot, input: &Logical, ctx: Ctx) -> Result<Plan> {
        let named = self.card.named_count();
        Ok(match input {
            Logical::Unit => mk(Op::GraphNames { slot: g }, named, named * self.row_copy(), None),
            Logical::Empty => mk(Op::Empty, 0.0, 0.0, None),
            _ => {
                let p = self.plan(input, ctx.unseeded())?;
                let (rows, cost) = (named * p.est_rows, named * p.est_cost_ns);
                mk(Op::GraphLoop { slot: g, input: Box::new(p) }, rows, cost, None)
            }
        })
    }

    /// Slice; `Slice(OrderBy(X))` and `Slice(Project(OrderBy(X)))` with a LIMIT become a TopK
    /// (k = offset + limit) under a Slice for the offset. Never across Distinct / Reduced.
    fn plan_slice(&mut self, input: &Logical, offset: u64, limit: Option<u64>, ctx: Ctx) -> Result<Plan> {
        let ctx = ctx.unseeded();
        let topk = |this: &mut Self, x: &Logical, keys: &[OrderKey], k: u64| -> Result<Plan> {
            let p = this.plan(x, ctx)?;
            let n = p.est_rows;
            let cost = p.est_cost_ns + n * (k.max(2) as f64).log2() * this.costs.sort_row_log + n * this.costs.decode_hit;
            Ok(mk(Op::TopK { input: Box::new(p), keys: keys.to_vec(), k }, n.min(k as f64), cost, None))
        };
        let inner = match (input, limit) {
            (Logical::OrderBy { input: x, keys }, Some(lim)) => Some(topk(self, x, keys, offset.saturating_add(lim))?),
            (Logical::Project { input: proj_in, slots }, Some(lim)) => match &**proj_in {
                Logical::OrderBy { input: x, keys } => {
                    let t = topk(self, x, keys, offset.saturating_add(lim))?;
                    let (rows, cost) = (t.est_rows, t.est_cost_ns);
                    Some(mk(Op::Project { input: Box::new(t), slots: slots.clone() }, rows, cost, None))
                }
                _ => None,
            },
            _ => None,
        };
        let (p, needs_slice) = match inner {
            Some(t) => (t, offset > 0),
            None => (self.plan(input, ctx)?, true),
        };
        if !needs_slice {
            return Ok(p);
        }
        let rows = (p.est_rows - offset as f64).max(0.0).min(limit.map_or(f64::INFINITY, |l| l as f64));
        let (cost, sorted) = (p.est_cost_ns, p.sorted_by);
        Ok(mk(Op::Slice { input: Box::new(p), offset, limit }, rows, cost, sorted))
    }
}

/// Narrow `hints[id]` to the slots bound wherever plan `p` (opened with seed `seeded`, inside an
/// EXISTS evaluation substituting `subst`) evaluates `EXISTS #id`: the certain slots of the rows
/// the expression sees, the seeded slots they carry and the substituted ones. The EXISTS plan is
/// then planned with those slots as substitution hints (index choice only).
fn exists_hints(p: &Plan, seeded: SlotSet, subst: SlotSet, hints: &mut [Option<SlotSet>]) {
    let exprs = node_exprs(p);
    if exprs.iter().any(|e| e.contains_exists()) {
        let pp = PlanProps::new(&[]);
        // The rows an expression of `p` is evaluated on: the input's, or the merged rows of an
        // OPTIONAL (whose filter is the only expression a join holds).
        let bound = input_seeds(p, seeded)
            .into_iter()
            .map(|(c, s)| pp.props(c).certain.union(carried(c, s)))
            .fold(subst, |acc, b| acc.union(b));
        for ExistsId(id) in exprs.into_iter().flat_map(expr::exists_ids) {
            if let Some(h) = hints.get_mut(id as usize) {
                *h = Some(h.map_or(bound, |h| h.intersect(bound)));
            }
        }
    }
    for (c, s) in input_seeds(p, seeded) {
        exists_hints(c, s, subst, hints);
    }
}

fn flatten_union<'l>(l: &'l Logical, out: &mut Vec<&'l Logical>) {
    match l {
        Logical::Union(a, b) => {
            flatten_union(a, out);
            flatten_union(b, out);
        }
        other => out.push(other),
    }
}

/// The dataset's named graphs, sorted (`QueryPlan.named_graphs`).
pub(crate) fn named_graphs(q: &LoweredQuery, store: &dyn QuadStore) -> Result<Vec<TermId>> {
    let mut v = match &q.dataset.named {
        NamedGraphs::All => store.named_graphs()?,
        NamedGraphs::Only(list) => list.to_vec(),
    };
    v.sort();
    v.dedup();
    Ok(v)
}

/// Plan a rewritten query. `named` are the dataset's sorted named graphs.
pub(crate) fn plan_query(
    q: &LoweredQuery,
    rw: Rewritten,
    named: Vec<TermId>,
    store: &dyn QuadStore,
    opts: &QueryOptions,
) -> Result<QueryPlan> {
    let mut planner = Planner::new(store, opts, &q.dataset, named.len(), &rw.exists, q.vars.width());
    let mut root = planner.plan(&rw.root, Ctx::default())?;
    if q.form == QueryForm::Ask {
        let (rows, cost) = (root.est_rows.min(1.0), root.est_cost_ns);
        root = mk(Op::Slice { input: Box::new(root), offset: 0, limit: Some(1) }, rows, cost, None);
    }
    // EXISTS patterns: parents (higher ids) before the patterns nested in them, so every
    // evaluation context is known when a pattern is planned.
    let mut hints = vec![None; rw.exists.len()];
    exists_hints(&root, SlotSet::EMPTY, SlotSet::EMPTY, &mut hints);
    let mut exists: Vec<Plan> = vec![mk(Op::Empty, 0.0, 0.0, None); rw.exists.len()];
    for id in (0..rw.exists.len()).rev() {
        let subst = hints[id].unwrap_or(SlotSet::EMPTY);
        let ctx = Ctx { seeded: SlotSet::EMPTY, subst, in_exists: true };
        exists[id] = planner.plan(&rw.exists[id], ctx)?;
        exists_hints(&exists[id], SlotSet::EMPTY, subst, &mut hints);
    }
    drop(planner);
    let mut plan = QueryPlan {
        form: q.form.clone(),
        root,
        vars: q.vars.clone(),
        dataset: q.dataset.clone(),
        named_graphs: named,
        exists,
        temps: q.temps.clone(),
        base_iri: q.base_iri.clone(),
    };
    finish::assign_ids(&mut plan);
    finish::set_canonical_flags(&mut plan);
    Ok(plan)
}

#[cfg(test)]
mod tests;
