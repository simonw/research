//! Join groups (spec §7.3.1, §7.5): flatten Join / Bgp into units (quad patterns, paths,
//! Values and opaque subtrees), order them, choose a join operator per step and apply the
//! group's filter conjuncts as soon as the joined prefix binds their evars.
//!
//! Ordering: left-deep dynamic programming over connected subsets for up to
//! [`MAX_DP_UNITS`] units, greedy (cheapest connected next unit, cross products last)
//! beyond, syntactic order when `reorder` is off. Every decision depends only on the
//! estimated [`Shape`] of the joined prefix, so the DP runs on shapes alone and the winning
//! order is then replayed through the same code to build the plan.
use std::cmp::Ordering;

use sq_core::batch::SlotSet;
use sq_core::ir::{Expr, Logical, QuadPat};
use sq_core::plan::{JoinKind, JoinPolicy, Op, Plan};
use sq_core::Result;

use super::{mk, semi, Ctx, Planner, EXPR_EVAL_NS};
use crate::card::selectivity;
use crate::expr::{and_all, conjuncts};
use crate::props::{self, slot_list, Props};

/// Groups up to this many units are ordered by dynamic programming over connected subsets.
const MAX_DP_UNITS: usize = 12;
/// `sort_left` threshold (spec §7.5).
const SORT_LEFT_ROWS: f64 = 64.0;

#[derive(Clone, Copy)]
enum UnitRef<'l> {
    Pat(QuadPat),
    Node(&'l Logical),
}

#[derive(Clone, Copy)]
struct Unit<'l> {
    r: UnitRef<'l>,
    props: Props,
}

/// A filter conjunct of a group with its evars (relative to the group's rows).
struct Conj {
    expr: Expr,
    evars: SlotSet,
    sel: f64,
}

/// Estimated properties of a joined prefix: everything join and filter decisions depend on.
/// `certain` / `maybe` include the seeded slots; `pats` are the quad patterns joined so far
/// (characteristic-set estimates for subject stars).
#[derive(Clone, Debug)]
pub(super) struct Shape {
    pub(super) rows: f64,
    pub(super) cost: f64,
    pub(super) certain: SlotSet,
    pub(super) maybe: SlotSet,
    pub(super) pats: Vec<QuadPat>,
}

/// A joined prefix: its plan and shape (`shape.rows / cost` = the plan's estimates).
struct Side {
    plan: Plan,
    shape: Shape,
}

/// A join operator decision (slot lists become vectors only when the plan is built).
pub(super) enum Algo {
    Bind { seed: SlotSet, sort_left: bool },
    Hash { keys: SlotSet, check: SlotSet, build_is_left: bool },
    Nested { check: SlotSet },
}

/// A join step: operator, planned right input and estimates of the joined result.
pub(super) struct Choice {
    pub(super) algo: Algo,
    pub(super) right: Plan,
    pub(super) rows: f64,
    pub(super) cost: f64,
}

/// How newly complete filter conjuncts are applied.
enum Step {
    Filter { expr: Expr, rows: f64, cost: f64 },
    Join { choice: Choice, kind: JoinKind },
}

/// A DP state: the cheapest shape found for a subset and how it was reached.
struct DpState {
    shape: Shape,
    applied: Vec<bool>,
    /// (subset before the last unit, last unit); None for a single-unit subset.
    from: Option<(usize, usize)>,
}

fn cheaper(a: &Choice, b: &Choice) -> bool {
    a.cost.total_cmp(&b.cost).then(a.rows.total_cmp(&b.rows)) == Ordering::Less
}

/// The plan of `left ⋈ right` for a decided join.
fn join_plan(left: Plan, ch: Choice, kind: JoinKind) -> Plan {
    let (left, right) = (Box::new(left), Box::new(ch.right));
    let op = match ch.algo {
        Algo::Bind { seed, sort_left } => {
            Op::BindJoin { left, right, seed: slot_list(seed), kind, sort_left, adaptive: true }
        }
        Algo::Hash { keys, check, build_is_left } => {
            let (build, probe) = if build_is_left { (left, right) } else { (right, left) };
            Op::HashJoin { build, probe, keys: slot_list(keys), check: slot_list(check), kind, build_is_left }
        }
        Algo::Nested { check } => Op::NestedLoop { left, right, check: slot_list(check), kind },
    };
    mk(op, ch.rows, ch.cost, None)
}

/// Shape after joining a unit with properties `right` (and quad pattern `pat`, if any).
fn joined_shape(left: &Shape, right: &Props, pat: Option<QuadPat>, ch: &Choice, kind: &JoinKind) -> Shape {
    let inner = *kind == JoinKind::Inner;
    let semi = matches!(kind, JoinKind::Semi | JoinKind::Anti);
    let mut pats = left.pats.clone();
    if inner {
        pats.extend(pat);
    }
    Shape {
        rows: ch.rows,
        cost: ch.cost,
        certain: if inner { left.certain.union(right.certain) } else { left.certain },
        maybe: if semi { left.maybe } else { left.maybe.union(right.maybe) },
        pats,
    }
}

impl<'a> Planner<'a> {
    /// A Bgp / Join tree, optionally under Filters: one join group.
    pub(crate) fn plan_group(&mut self, l: &Logical, ctx: Ctx) -> Result<Plan> {
        let mut exprs = Vec::new();
        let mut body = l;
        while let Logical::Filter { input, expr } = body {
            conjuncts(expr.clone(), &mut exprs);
            body = input;
        }
        let mut units = Vec::new();
        if !self.collect_units(body, &mut units) {
            return Ok(mk(Op::Empty, 0.0, 0.0, None));
        }
        let group_maybe = self.lp.props(body).maybe;
        let conj: Vec<Conj> = exprs
            .into_iter()
            .map(|e| Conj { evars: self.lp.evars(&e, group_maybe), sel: selectivity(&e), expr: e })
            .collect();
        if units.is_empty() {
            let shape = Shape { rows: 1.0, cost: 0.0, certain: ctx.seeded, maybe: ctx.seeded, pats: Vec::new() };
            let side = Side { plan: mk(Op::Unit, 1.0, 0.0, None), shape };
            let mut applied = vec![false; conj.len()];
            let side = self.apply_filters(side, &conj, &mut applied, ctx, false)?;
            return Ok(self.apply_filters(side, &conj, &mut applied, ctx, true)?.plan);
        }
        let order = if !self.opts.reorder {
            (0..units.len()).collect()
        } else if units.len() <= MAX_DP_UNITS {
            self.dp_order(&units, &conj, ctx)?
        } else {
            self.greedy_order(&units, &conj, ctx)?
        };
        self.build_order(&units, &conj, ctx, &order)
    }

    /// Flatten Join / Bgp into units; Unit is dropped. Returns false if a unit is Empty.
    fn collect_units<'l>(&self, l: &'l Logical, out: &mut Vec<Unit<'l>>) -> bool {
        match l {
            Logical::Join(a, b) => self.collect_units(a, out) && self.collect_units(b, out),
            Logical::Bgp(pats) => {
                out.extend(pats.iter().map(|p| Unit { r: UnitRef::Pat(*p), props: props::pattern(p) }));
                true
            }
            Logical::Unit => true,
            Logical::Empty => false,
            other => {
                out.push(Unit { r: UnitRef::Node(other), props: self.lp.props(other) });
                true
            }
        }
    }

    fn plan_unit(&mut self, r: UnitRef<'_>, ctx: Ctx) -> Result<Plan> {
        match r {
            UnitRef::Pat(p) => self.plan_scan(&p, ctx),
            UnitRef::Node(n) => self.plan(n, ctx),
        }
    }

    /// Shape of a single unit opened in `ctx`.
    fn start_shape(&mut self, unit: &Unit<'_>, ctx: Ctx) -> Result<(Plan, Shape)> {
        let plan = self.plan_unit(unit.r, ctx)?;
        let shape = Shape {
            rows: plan.est_rows,
            cost: plan.est_cost_ns,
            certain: unit.props.certain.union(ctx.seeded),
            maybe: unit.props.maybe.union(ctx.seeded),
            pats: match unit.r {
                UnitRef::Pat(p) => vec![p],
                UnitRef::Node(_) => Vec::new(),
            },
        };
        Ok((plan, shape))
    }

    fn pat_of(unit: &Unit<'_>) -> Option<QuadPat> {
        match unit.r {
            UnitRef::Pat(p) => Some(p),
            UnitRef::Node(_) => None,
        }
    }

    /// Units among `outside` that share a slot with the prefix; all of them when none does
    /// (cross products last).
    fn next_candidates(units: &[Unit<'_>], shape: &Shape, outside: impl Iterator<Item = usize>) -> Vec<usize> {
        let outside: Vec<usize> = outside.collect();
        let connected: Vec<usize> =
            outside.iter().copied().filter(|&i| !shape.maybe.intersect(units[i].props.maybe).is_empty()).collect();
        if connected.is_empty() { outside } else { connected }
    }

    /// Left-deep dynamic programming over connected subsets (DPsub) on shapes only: the
    /// cheapest order of all units.
    fn dp_order(&mut self, units: &[Unit<'_>], conj: &[Conj], ctx: Ctx) -> Result<Vec<usize>> {
        let n = units.len();
        let full = (1usize << n) - 1;
        let mut best: Vec<Option<DpState>> = (0..=full).map(|_| None).collect();
        for (i, u) in units.iter().enumerate() {
            let (_, shape) = self.start_shape(u, ctx)?;
            let mut applied = vec![false; conj.len()];
            let (_, shape) = self.filter_steps(&shape, conj, &mut applied, ctx, false)?;
            best[1 << i] = Some(DpState { shape, applied, from: None });
        }
        // Every superset of a subset is numerically larger: ascending order sees final states.
        for mask in 1..full {
            let Some(state) = best[mask].take() else { continue };
            let outside = (0..n).filter(|i| mask & (1 << i) == 0);
            for i in Self::next_candidates(units, &state.shape, outside) {
                let ch = self.join_choice(&state.shape, &units[i], None, ctx)?;
                let next = mask | (1 << i);
                let bound = best[next].as_ref().map_or(f64::INFINITY, |s| s.shape.cost);
                // Filters only add cost: a join step at least as expensive cannot win.
                if ch.cost >= bound {
                    continue;
                }
                let shape = joined_shape(&state.shape, &units[i].props, Self::pat_of(&units[i]), &ch, &JoinKind::Inner);
                let mut applied = state.applied.clone();
                let (_, shape) = self.filter_steps(&shape, conj, &mut applied, ctx, false)?;
                if shape.cost < bound {
                    best[next] = Some(DpState { shape, applied, from: Some((mask, i)) });
                }
            }
            best[mask] = Some(state);
        }
        let mut order = Vec::with_capacity(n);
        let mut mask = full;
        loop {
            let state = best[mask].as_ref().expect("every subset on the path was reached");
            match state.from {
                Some((prev, unit)) => {
                    order.push(unit);
                    mask = prev;
                }
                None => {
                    order.push(mask.trailing_zeros() as usize);
                    break;
                }
            }
        }
        order.reverse();
        Ok(order)
    }

    /// Greedy order: the unit with the fewest rows first, then the cheapest connected next unit.
    fn greedy_order(&mut self, units: &[Unit<'_>], conj: &[Conj], ctx: Ctx) -> Result<Vec<usize>> {
        let mut start = (0, f64::INFINITY);
        for (i, u) in units.iter().enumerate() {
            let rows = self.plan_unit(u.r, ctx)?.est_rows;
            if rows < start.1 {
                start = (i, rows);
            }
        }
        let (_, shape) = self.start_shape(&units[start.0], ctx)?;
        let mut applied = vec![false; conj.len()];
        let (_, mut shape) = self.filter_steps(&shape, conj, &mut applied, ctx, false)?;
        let mut order = vec![start.0];
        let mut remaining: Vec<usize> = (0..units.len()).filter(|&i| i != start.0).collect();
        while !remaining.is_empty() {
            let mut best: Option<(usize, Choice)> = None;
            for i in Self::next_candidates(units, &shape, remaining.iter().copied()) {
                let ch = self.join_choice(&shape, &units[i], None, ctx)?;
                if best.as_ref().is_none_or(|(_, b)| cheaper(&ch, b)) {
                    best = Some((i, ch));
                }
            }
            let (i, ch) = best.expect("at least one candidate");
            remaining.retain(|&j| j != i);
            order.push(i);
            let joined = joined_shape(&shape, &units[i].props, Self::pat_of(&units[i]), &ch, &JoinKind::Inner);
            shape = self.filter_steps(&joined, conj, &mut applied, ctx, false)?.1;
        }
        Ok(order)
    }

    /// Build the plan joining the units in `order` (left-deep), choosing each operator and
    /// placing each filter exactly as the ordering search estimated it.
    fn build_order(&mut self, units: &[Unit<'_>], conj: &[Conj], ctx: Ctx, order: &[usize]) -> Result<Plan> {
        let mut applied = vec![false; conj.len()];
        let (plan, shape) = self.start_shape(&units[order[0]], ctx)?;
        let mut side = self.apply_filters(Side { plan, shape }, conj, &mut applied, ctx, false)?;
        for &i in &order[1..] {
            let unit = &units[i];
            let ch = self.join_choice(&side.shape, unit, None, ctx)?;
            let shape = joined_shape(&side.shape, &unit.props, Self::pat_of(unit), &ch, &JoinKind::Inner);
            let plan = join_plan(side.plan, ch, JoinKind::Inner);
            side = self.apply_filters(Side { plan, shape }, conj, &mut applied, ctx, false)?;
        }
        Ok(self.apply_filters(side, conj, &mut applied, ctx, true)?.plan)
    }

    /// Decide how every not-yet-applied conjunct whose evars are certain on `shape` (all
    /// remaining ones when `rest` is set: their original position, the top of the group) is
    /// applied; marks them applied and returns the steps and the resulting shape. Whole
    /// `[NOT] EXISTS` conjuncts may become Semi / Anti joins (see `semi`).
    fn filter_steps(
        &mut self,
        shape: &Shape,
        conj: &[Conj],
        applied: &mut [bool],
        ctx: Ctx,
        rest: bool,
    ) -> Result<(Vec<Step>, Shape)> {
        let mut plain = Vec::new();
        let mut sel = 1.0;
        let mut exists = Vec::new();
        for (i, c) in conj.iter().enumerate() {
            if applied[i] || !(rest || c.evars.is_subset(&shape.certain)) {
                continue;
            }
            applied[i] = true;
            match semi::exists_conjunct(&c.expr) {
                Some((id, anti)) if !rest => exists.push((i, id, anti)),
                _ => {
                    self.note_exists(&c.expr, shape.certain.union(ctx.subst));
                    sel *= c.sel;
                    plain.push(c.expr.clone());
                }
            }
        }
        let mut steps = Vec::new();
        let mut shape = shape.clone();
        let n = plain.len() as f64;
        if let Some(expr) = and_all(plain) {
            let (rows, cost) = (shape.rows * sel, shape.cost + shape.rows * n * EXPR_EVAL_NS);
            steps.push(Step::Filter { expr, rows, cost });
            (shape.rows, shape.cost) = (rows, cost);
        }
        for (i, id, anti) in exists {
            match self.semi_join_choice(&shape, id, conj[i].evars, ctx)? {
                Some(choice) => {
                    let kind = semi::semi_kind(anti);
                    shape = joined_shape(&shape, &Props::default(), None, &choice, &kind);
                    steps.push(Step::Join { choice, kind });
                }
                None => {
                    self.note_exists(&conj[i].expr, shape.certain.union(ctx.subst));
                    let (rows, cost) = (shape.rows * conj[i].sel, shape.cost + shape.rows * EXPR_EVAL_NS);
                    steps.push(Step::Filter { expr: conj[i].expr.clone(), rows, cost });
                    (shape.rows, shape.cost) = (rows, cost);
                }
            }
        }
        Ok((steps, shape))
    }

    /// `filter_steps` materialized on a side.
    fn apply_filters(&mut self, side: Side, conj: &[Conj], applied: &mut [bool], ctx: Ctx, rest: bool) -> Result<Side> {
        let (steps, shape) = self.filter_steps(&side.shape, conj, applied, ctx, rest)?;
        let mut plan = side.plan;
        for step in steps {
            plan = match step {
                Step::Filter { expr, rows, cost } => {
                    let sorted = plan.sorted_by;
                    mk(Op::Filter { input: Box::new(plan), expr }, rows, cost, sorted)
                }
                Step::Join { choice, kind } => join_plan(plan, choice, kind),
            };
        }
        Ok(Side { plan, shape })
    }

    /// Choose the operator joining `left` with `unit`. `left_join` = Some(filter selectivity)
    /// for an OPTIONAL (kind Left), None for an inner join.
    fn join_choice(&mut self, left: &Shape, unit: &Unit<'_>, left_join: Option<f64>, ctx: Ctx) -> Result<Choice> {
        let pr = &unit.props;
        let shared = left.maybe.intersect(pr.maybe);
        let seed = left.certain.intersect(pr.maybe);
        let keys = left.certain.intersect(pr.certain);
        let check = shared.minus(keys);
        let bind_ok = !seed.is_empty() && shared.is_subset(&left.certain) && pr.accepts_seed(seed);
        let hash_ok = !keys.is_empty();
        let policy = self.opts.join_policy;
        let try_bind = bind_ok && policy != JoinPolicy::PreferHash;
        let try_hash = hash_ok && !(policy == JoinPolicy::PreferBind && bind_ok);
        // Inputs other than a BindJoin's right side receive the outer seed only for inner joins.
        let rctx = if left_join.is_some() { ctx.unseeded() } else { ctx };

        let lrows = left.rows;
        // Rows per left row when the unit extends a subject star (characteristic sets).
        let star = match (unit.r, left_join) {
            (UnitRef::Pat(q), None) => self.card.star_ratio(&left.pats, &q),
            _ => None,
        };
        let finish_rows = |rows: f64| match left_join {
            Some(sel) => (rows * sel).max(lrows),
            None => rows,
        };
        let mut best: Option<Choice> = None;
        if try_bind {
            let right = self.plan_unit(unit.r, Ctx { seeded: seed, ..ctx })?;
            let sort_left = lrows > SORT_LEFT_ROWS;
            let mut per_probe = right.est_cost_ns;
            if sort_left && matches!(right.op, Op::Scan(_)) {
                per_probe -= self.costs.seek_random - self.costs.seek_sorted;
            }
            let rows = finish_rows(lrows * star.unwrap_or(right.est_rows));
            let cost = left.cost + lrows * per_probe.max(0.0) + rows * self.row_copy();
            let algo = Algo::Bind { seed, sort_left };
            best = Some(Choice { algo, right, rows, cost });
        }
        if try_hash {
            let right = self.plan_unit(unit.r, rctx)?;
            let per_left = match star {
                Some(r) => r,
                None => self.rows_given(unit, keys, ctx, lrows, &right)?,
            };
            let rows = finish_rows(lrows * per_left);
            let rrows = right.est_rows;
            let build_is_left = left_join.is_none() && lrows < rrows;
            let (b, p) = if build_is_left { (lrows, rrows) } else { (rrows, lrows) };
            let (hb, hp) = if b < 32_768.0 {
                (self.costs.hash_build_small, self.costs.hash_probe_small)
            } else {
                (self.costs.hash_build_large, self.costs.hash_probe_large)
            };
            let cost = left.cost + right.est_cost_ns + b * hb + p * hp + rows * self.row_copy();
            let algo = Algo::Hash { keys, check, build_is_left };
            let ch = Choice { algo, right, rows, cost };
            if best.as_ref().is_none_or(|b| cheaper(&ch, b)) {
                best = Some(ch);
            }
        }
        if let Some(ch) = best {
            return Ok(ch);
        }
        let right = self.plan_unit(unit.r, rctx)?;
        let rrows = right.est_rows;
        let match_rate = if shared.is_empty() { 1.0 } else { 0.5 };
        let rows = finish_rows(lrows * rrows * match_rate);
        let cost =
            left.cost + right.est_cost_ns + lrows * rrows * 2.0 * self.costs.row_copy_per_slot + rows * self.row_copy();
        Ok(Choice { algo: Algo::Nested { check: shared }, right, rows, cost })
    }

    /// Rows of `unit` per left row when joined on `keys` (estimate only).
    fn rows_given(&mut self, unit: &Unit<'_>, keys: SlotSet, ctx: Ctx, lrows: f64, standalone: &Plan) -> Result<f64> {
        match unit.r {
            UnitRef::Pat(p) => self.card.pattern_rows(&p, keys.union(ctx.bound())),
            UnitRef::Node(n) if unit.props.accepts_seed(keys) => {
                Ok(self.plan(n, Ctx { seeded: keys.union(ctx.seeded), ..ctx })?.est_rows)
            }
            UnitRef::Node(_) => Ok(standalone.est_rows / standalone.est_rows.max(lrows).max(1.0)),
        }
    }

    /// OPTIONAL: BindJoin{Left} when the seeds are safe for the right side, else
    /// HashJoin{Left} (build right) or NestedLoop{Left}. The outer seed goes to the left only.
    pub(crate) fn plan_left_join(
        &mut self,
        left: &Logical,
        right: &Logical,
        filter: Option<&Expr>,
        ctx: Ctx,
    ) -> Result<Plan> {
        let (pl, pr) = (self.lp.props(left), self.lp.props(right));
        let plan = self.plan(left, ctx)?;
        let shape = Shape {
            rows: plan.est_rows,
            cost: plan.est_cost_ns,
            certain: pl.certain.union(ctx.seeded),
            maybe: pl.maybe.union(ctx.seeded),
            pats: Vec::new(),
        };
        if let Some(f) = filter {
            self.note_exists(f, shape.certain.union(pr.certain).union(ctx.subst));
        }
        let unit = Unit { r: UnitRef::Node(right), props: pr };
        let ch = self.join_choice(&shape, &unit, Some(filter.map_or(1.0, selectivity)), ctx)?;
        Ok(join_plan(plan, ch, JoinKind::Left { filter: filter.cloned() }))
    }

    /// MINUS: no statically shared slot means no row can be removed -> just the left plan.
    pub(crate) fn plan_minus(&mut self, left: &Logical, right: &Logical, ctx: Ctx) -> Result<Plan> {
        let (pl, pr) = (self.lp.props(left), self.lp.props(right));
        let lplan = self.plan(left, ctx)?;
        let shared = pl.maybe.union(ctx.seeded).intersect(pr.maybe);
        if shared.is_empty() {
            return Ok(lplan);
        }
        let rplan = self.plan(right, ctx.unseeded())?;
        let keys = pl.certain.union(ctx.seeded).intersect(pr.certain);
        let check = shared.minus(keys);
        let (l, r) = (lplan.est_rows, rplan.est_rows);
        let (hb, hp) = if r < 32_768.0 {
            (self.costs.hash_build_small, self.costs.hash_probe_small)
        } else {
            (self.costs.hash_build_large, self.costs.hash_probe_large)
        };
        let cost = lplan.est_cost_ns + rplan.est_cost_ns + r * hb + l * hp;
        let rows = if r > 0.0 { l * 0.5 } else { l };
        let op = Op::Minus {
            left: Box::new(lplan),
            right: Box::new(rplan),
            keys: slot_list(keys),
            check: slot_list(check),
        };
        Ok(mk(op, rows, cost, None))
    }
}
