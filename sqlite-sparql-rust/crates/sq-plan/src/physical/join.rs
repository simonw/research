//! Join groups (spec §7.3.1, §7.5): flatten Join / Bgp into units (quad patterns, paths,
//! Values and opaque subtrees), order them, choose a join operator per step and apply the
//! group's filter conjuncts as soon as the joined prefix binds their evars.
//!
//! Ordering: left-deep dynamic programming over connected subsets for groups of up to
//! [`MAX_DP_UNITS`] units whose search takes at most [`MAX_DP_STEPS`] join steps, greedy
//! (cheapest connected next unit, cross products last) otherwise, syntactic order when
//! `reorder` is off. Every decision depends only on the estimated [`Shape`] of the joined
//! prefix, so the search runs on shapes alone ([`Mode::Probe`]: opaque units are estimated, not
//! planned, see `estimate`; quad patterns are costed from a per-group cache) and the winning
//! order is then replayed through the same code with real plans ([`Mode::Build`]).
use std::cmp::Ordering;

use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use sq_core::batch::SlotSet;
use sq_core::ir::{ExistsId, Expr, Logical, QuadPat};
use sq_core::plan::{JoinKind, JoinPolicy, Op, Plan};
use sq_core::{Error, Result};

use super::bits::{mask_iter, Bits};
use super::scan::bound_columns;
use super::{carried, clamp_est, mk, semi, Ctx, Planner, EXPR_EVAL_NS};
use crate::card::{same_star, selectivity, Card};
use crate::expr::{and_all, conjuncts};
use crate::props::{self, slot_list, Props};

/// Groups of up to this many units may be ordered by dynamic programming (spec §7.5) ...
const MAX_DP_UNITS: usize = 12;
/// ... when the search takes at most this many join steps (pairs of a reachable subset and a
/// candidate next unit, see [`dp_steps`]), which bounds planning time by work rather than by
/// unit count. A dense group (every unit shares a slot with every other, e.g. a subject star)
/// of n units takes n·2^(n-1) - n steps: 2 295 for 9 units, 5 110 for 10, 24 564 for 12, while
/// a chain of n takes n(n-1). Larger searches use the greedy order.
const MAX_DP_STEPS: usize = 6_000;
/// `sort_left` threshold (spec §7.5).
pub(super) const SORT_LEFT_ROWS: f64 = 64.0;

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

/// One join group: its units and filter conjuncts, the context it is planned in, and caches
/// of the ordering search (which costs every unit in many contexts).
struct Group<'l> {
    units: Vec<Unit<'l>>,
    conj: Vec<Conj>,
    ctx: Ctx,
    /// Per unit that can extend a subject star (`Card::star_extends`): the other quad pattern
    /// units of that star (`same_star`); empty for every other unit.
    star_mates: Vec<Bits>,
    /// Characteristic-set estimates per star (a set of quad pattern units).
    star_memo: StarMemo,
    /// Per quad pattern unit: rows and cost of one open by bound columns (bit c set = column c
    /// of [g, s, p, o] bound, see `bound_columns`), which determine the estimate.
    scan_est: Vec<[Option<(f64, f64)>; 16]>,
}

impl<'l> Group<'l> {
    fn new(units: Vec<Unit<'l>>, conj: Vec<Conj>, ctx: Ctx, card: &Card<'_>) -> Self {
        let star_mates = units
            .iter()
            .enumerate()
            .map(|(i, u)| match u.r {
                UnitRef::Pat(next) if card.star_extends(&next) => {
                    let mut mates = Bits::default();
                    for (j, v) in units.iter().enumerate() {
                        if let UnitRef::Pat(p) = v.r {
                            if j != i && same_star(&p, &next) {
                                mates.insert(j);
                            }
                        }
                    }
                    mates
                }
                _ => Bits::default(),
            })
            .collect();
        let scan_est = vec![[None; 16]; units.len()];
        Group { units, conj, ctx, star_mates, star_memo: StarMemo::default(), scan_est }
    }

    /// Per unit, the units sharing a maybe-bound slot with it, and the units that share one
    /// with the group's seed (connected to every prefix): bit masks for groups of at most
    /// [`MAX_DP_UNITS`] units.
    fn adjacency(&self) -> (Vec<usize>, usize) {
        // The units other than `skip` that may bind one of `slots`.
        let touching = |slots: SlotSet, skip: Option<usize>| {
            (0..self.units.len())
                .filter(|&j| Some(j) != skip && !slots.intersect(self.units[j].props.maybe).is_empty())
                .fold(0usize, |m, j| m | 1 << j)
        };
        let adj = (0..self.units.len()).map(|i| touching(self.units[i].props.maybe, Some(i))).collect();
        (adj, touching(self.ctx.seeded, None))
    }
}

/// Characteristic-set estimates per star, keyed by its set of quad pattern units: one word
/// for stars among the first 64 units (every DP group), a `Bits` otherwise.
#[derive(Default)]
struct StarMemo {
    small: FxHashMap<u64, Option<f64>>,
    large: FxHashMap<Bits, Option<f64>>,
}

impl StarMemo {
    fn get(&self, star: &Bits) -> Option<Option<f64>> {
        match star.as_word() {
            Some(w) => self.small.get(&w).copied(),
            None => self.large.get(star).copied(),
        }
    }

    fn insert(&mut self, star: &Bits, rows: Option<f64>) {
        match star.as_word() {
            Some(w) => self.small.insert(w, rows),
            None => self.large.insert(star.clone(), rows),
        };
    }
}

/// The units a DP prefix `mask` may be joined with next: those sharing a slot with it (a
/// prefix's maybe-bound slots are its units' and the seed's), or every other unit when none
/// does (cross products last).
fn dp_candidates(adj: &[usize], seeded: usize, mask: usize) -> usize {
    let outside = ((1usize << adj.len()) - 1) & !mask;
    let near = mask_iter(mask).fold(seeded, |m, i| m | adj[i]) & outside;
    if near == 0 { outside } else { near }
}

/// The number of join steps `dp_order` takes (one per reachable subset and candidate next
/// unit), or None once it exceeds `limit`.
fn dp_steps(adj: &[usize], seeded: usize, limit: usize) -> Option<usize> {
    let full = (1usize << adj.len()) - 1;
    let mut reached = vec![false; full + 1];
    for i in 0..adj.len() {
        reached[1 << i] = true;
    }
    let mut steps = 0;
    for mask in 1..full {
        if !reached[mask] {
            continue;
        }
        let next = dp_candidates(adj, seeded, mask);
        steps += next.count_ones() as usize;
        if steps > limit {
            return None;
        }
        for i in mask_iter(next) {
            reached[mask | 1 << i] = true;
        }
    }
    Some(steps)
}

/// Whether a join decision only needs estimates (the ordering search) or builds the plan.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(super) enum Mode {
    Probe,
    Build,
}

/// A join input opened in some context: its estimates, the seeded slots (of the context it
/// was opened with) its rows carry, and its plan (only when building; boxed so that the many
/// probe-mode choices of the ordering search stay small).
pub(super) struct Opened {
    pub(super) plan: Option<Box<Plan>>,
    pub(super) rows: f64,
    pub(super) cost: f64,
    pub(super) carried: SlotSet,
}

impl Opened {
    fn planned(plan: Plan, seeded: SlotSet) -> Opened {
        let (rows, cost, carried) = (plan.est_rows, plan.est_cost_ns, carried(&plan, seeded));
        Opened { plan: Some(Box::new(plan)), rows, cost, carried }
    }

    fn into_plan(self) -> Result<Box<Plan>> {
        self.plan.ok_or_else(|| Error::internal("a plan was built from an ordering-search estimate"))
    }
}

/// Estimated properties of a joined prefix: everything join and filter decisions depend on.
#[derive(Clone, Debug)]
pub(super) struct Shape {
    pub(super) rows: f64,
    pub(super) cost: f64,
    /// Slots the joined units bind in every row themselves. Filter conjuncts wait for these:
    /// a conjunct applied to rows that hold a slot only through the group's seed would make
    /// that seed unsafe for the group (`safe_inject` of a Filter).
    pub(super) own: SlotSet,
    /// `own` plus the seeded slots the prefix's rows carry (`carried`): what a join may read.
    pub(super) certain: SlotSet,
    /// Slots the prefix may bind, every seeded slot included.
    pub(super) maybe: SlotSet,
    /// The group's units joined so far (subject stars among them are estimated by
    /// characteristic sets).
    units: Bits,
}

impl Shape {
    /// The shape of an input outside any join group (the left side of an OPTIONAL).
    fn of_input(rows: f64, cost: f64, own: SlotSet, certain: SlotSet, maybe: SlotSet) -> Shape {
        Shape { rows, cost, own, certain, maybe, units: Bits::default() }
    }
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

/// A join step: operator, right input and estimates of the joined result.
pub(super) struct Choice {
    pub(super) algo: Algo,
    pub(super) right: Opened,
    pub(super) rows: f64,
    pub(super) cost: f64,
}

impl Choice {
    /// A join step with its estimates kept finite (`clamp_est`).
    pub(super) fn new(algo: Algo, right: Opened, rows: f64, cost: f64) -> Choice {
        Choice { algo, right, rows: clamp_est(rows), cost: clamp_est(cost) }
    }
}

/// How newly complete filter conjuncts are applied.
enum Step {
    Filter { expr: Expr, rows: f64, cost: f64 },
    Join { choice: Choice, kind: JoinKind },
}

/// A DP state: the cheapest shape found for a subset and how it was reached.
struct DpState {
    shape: Shape,
    /// The conjuncts applied so far.
    applied: Bits,
    /// (subset before the last unit, last unit); None for a single-unit subset.
    from: Option<(usize, usize)>,
}

fn cheaper(a: &Choice, b: &Choice) -> bool {
    a.cost.total_cmp(&b.cost).then(a.rows.total_cmp(&b.rows)) == Ordering::Less
}

/// The plan of `left ⋈ right` for a decided join.
fn join_plan(left: Plan, ch: Choice, kind: JoinKind) -> Result<Plan> {
    let (left, right) = (Box::new(left), ch.right.into_plan()?);
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
    Ok(mk(op, ch.rows, ch.cost, None))
}

/// Shape after joining unit `unit` (None for an EXISTS pattern) with properties `right`.
fn joined_shape(left: &Shape, right: &Props, unit: Option<usize>, ch: &Choice, kind: &JoinKind) -> Shape {
    let inner = *kind == JoinKind::Inner;
    let semi = matches!(kind, JoinKind::Semi | JoinKind::Anti);
    let mut units = left.units.clone();
    if let (true, Some(i)) = (inner, unit) {
        units.insert(i);
    }
    Shape {
        rows: ch.rows,
        cost: ch.cost,
        own: if inner { left.own.union(right.certain) } else { left.own },
        // A BindJoin's right side carries seed-list slots only, which the left rows hold.
        certain: if inner { left.certain.union(right.certain).union(ch.right.carried) } else { left.certain },
        maybe: if semi { left.maybe } else { left.maybe.union(right.maybe) },
        units,
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
            // Not reached for rewritten queries: Empty propagation (`rewrite::empty`) leaves no
            // Empty inside a join.
            return Ok(mk(Op::Empty, 0.0, 0.0, None));
        }
        let group_maybe = self.lp.props(body).maybe;
        let conj: Vec<Conj> = exprs
            .into_iter()
            .map(|e| Conj { evars: self.lp.evars(&e, group_maybe), sel: selectivity(&e), expr: e })
            .collect();
        let mut g = Group::new(units, conj, ctx, &self.card);
        if g.units.is_empty() {
            let (own, certain, maybe) = (SlotSet::EMPTY, ctx.seeded, ctx.seeded);
            let shape = Shape::of_input(1.0, 0.0, own, certain, maybe);
            let side = Side { plan: mk(Op::Unit, 1.0, 0.0, None), shape };
            let mut applied = Bits::default();
            let side = self.apply_filters(&mut g, side, &mut applied, false)?;
            return Ok(self.apply_filters(&mut g, side, &mut applied, true)?.plan);
        }
        let order = if self.opts.reorder { self.join_order(&mut g)? } else { (0..g.units.len()).collect() };
        self.build_order(&mut g, &order)
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

    /// The join order of a group: DP when its search is small enough, greedy otherwise.
    fn join_order(&mut self, g: &mut Group<'_>) -> Result<Vec<usize>> {
        if g.units.len() <= MAX_DP_UNITS {
            let (adj, seeded) = g.adjacency();
            if dp_steps(&adj, seeded, MAX_DP_STEPS).is_some() {
                return self.dp_order(g, &adj, seeded);
            }
        }
        self.greedy_order(g)
    }

    /// Open unit `i` in `ctx` (see [`Opened`]).
    fn open_unit(&mut self, g: &mut Group<'_>, i: usize, ctx: Ctx, mode: Mode) -> Result<Opened> {
        match (g.units[i].r, mode) {
            (UnitRef::Pat(p), Mode::Build) => Ok(Opened::planned(self.plan_scan(&p, ctx)?, ctx.seeded)),
            // A scan's rows carry the whole seed (`carried`).
            (UnitRef::Pat(p), Mode::Probe) => {
                let (rows, cost) = self.group_scan_estimate(g, i, &p, ctx.bound())?;
                Ok(Opened { plan: None, rows, cost, carried: ctx.seeded })
            }
            (UnitRef::Node(n), _) => self.open_node(n, ctx, mode),
        }
    }

    /// `scan_estimate` of quad pattern unit `i` with `bound` slots bound, cached per group.
    fn group_scan_estimate(
        &mut self,
        g: &mut Group<'_>,
        i: usize,
        pat: &QuadPat,
        bound: SlotSet,
    ) -> Result<(f64, f64)> {
        let key = bound_columns(pat, bound).iter().enumerate().fold(0, |k, (c, b)| k | (*b as usize) << c);
        if let Some(e) = g.scan_est[i][key] {
            return Ok(e);
        }
        let (rows, cost) = self.scan_estimate(pat, bound)?;
        let e = (clamp_est(rows), clamp_est(cost));
        g.scan_est[i][key] = Some(e);
        Ok(e)
    }

    /// Open an opaque subtree in `ctx`: planned when building, estimated when probing.
    pub(super) fn open_node(&mut self, n: &Logical, ctx: Ctx, mode: Mode) -> Result<Opened> {
        match mode {
            Mode::Build => Ok(Opened::planned(self.plan(n, ctx)?, ctx.seeded)),
            Mode::Probe => {
                let e = self.estimate(n, ctx)?;
                Ok(Opened { plan: None, rows: e.rows, cost: e.cost, carried: ctx.seeded.intersect(e.carries) })
            }
        }
    }

    /// Unit `i` opened in the group's context and its shape.
    fn start_shape(&mut self, g: &mut Group<'_>, i: usize, mode: Mode) -> Result<(Opened, Shape)> {
        let ctx = g.ctx;
        let opened = self.open_unit(g, i, ctx, mode)?;
        let props = g.units[i].props;
        let shape = Shape {
            rows: opened.rows,
            cost: opened.cost,
            own: props.certain,
            certain: props.certain.union(opened.carried),
            maybe: props.maybe.union(ctx.seeded),
            units: Bits::single(i),
        };
        Ok((opened, shape))
    }

    /// Rows per left row when unit `i` extends a subject star of the prefix `joined`
    /// (characteristic sets, §7.4.4); None when no star applies.
    fn star_ratio(&self, g: &mut Group<'_>, joined: &Bits, i: usize) -> Option<f64> {
        if g.star_mates[i].is_empty() {
            return None;
        }
        let mut star = joined.intersect(&g.star_mates[i]);
        if star.is_empty() {
            return None;
        }
        let before = self.star_rows(g, &star)?;
        star.insert(i);
        let after = self.star_rows(g, &star)?;
        (before > 0.0).then(|| after / before)
    }

    /// `Card::star_rows` of a star given as quad pattern units (memoized per group).
    fn star_rows(&self, g: &mut Group<'_>, star: &Bits) -> Option<f64> {
        if let Some(r) = g.star_memo.get(star) {
            return r;
        }
        let pats = star.iter().filter_map(|j| match &g.units[j].r {
            UnitRef::Pat(p) => Some(p),
            UnitRef::Node(_) => None,
        });
        let r = self.card.star_rows(pats);
        g.star_memo.insert(star, r);
        r
    }

    /// Units among `outside` that share a slot with the prefix; all of them when none does
    /// (cross products last).
    fn next_candidates(g: &Group<'_>, shape: &Shape, outside: impl Iterator<Item = usize>) -> Vec<usize> {
        let outside: Vec<usize> = outside.collect();
        let connected: Vec<usize> =
            outside.iter().copied().filter(|&i| !shape.maybe.intersect(g.units[i].props.maybe).is_empty()).collect();
        if connected.is_empty() { outside } else { connected }
    }

    /// Left-deep dynamic programming over connected subsets (DPsub) on shapes only: the
    /// cheapest order of all units. `adj` / `seeded` are the group's [`Group::adjacency`].
    fn dp_order(&mut self, g: &mut Group<'_>, adj: &[usize], seeded: usize) -> Result<Vec<usize>> {
        let n = g.units.len();
        let full = (1usize << n) - 1;
        let mut best: Vec<Option<DpState>> = (0..=full).map(|_| None).collect();
        for i in 0..n {
            let (_, shape) = self.start_shape(g, i, Mode::Probe)?;
            let mut applied = Bits::default();
            let (_, shape) = self.filter_steps(g, &shape, &mut applied, false, Mode::Probe)?;
            best[1 << i] = Some(DpState { shape, applied, from: None });
        }
        // Every superset of a subset is numerically larger: ascending order sees final states.
        for mask in 1..full {
            let Some(state) = best[mask].take() else { continue };
            for i in mask_iter(dp_candidates(adj, seeded, mask)) {
                let ch = self.join_choice(g, &state.shape, i, None, Mode::Probe)?;
                let next = mask | (1 << i);
                let bound = best[next].as_ref().map_or(f64::INFINITY, |s| s.shape.cost);
                // Filters only add cost: a join step at least as expensive cannot win.
                if ch.cost >= bound {
                    continue;
                }
                let shape = joined_shape(&state.shape, &g.units[i].props, Some(i), &ch, &JoinKind::Inner);
                let mut applied = state.applied.clone();
                let (_, shape) = self.filter_steps(g, &shape, &mut applied, false, Mode::Probe)?;
                if shape.cost < bound {
                    best[next] = Some(DpState { shape, applied, from: Some((mask, i)) });
                }
            }
            best[mask] = Some(state);
        }
        let mut order = Vec::with_capacity(n);
        let mut mask = full;
        loop {
            let state = best[mask].as_ref().ok_or_else(|| Error::internal("join DP left a subset unreached"))?;
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
    fn greedy_order(&mut self, g: &mut Group<'_>) -> Result<Vec<usize>> {
        let ctx = g.ctx;
        let mut start = (0, f64::INFINITY);
        for i in 0..g.units.len() {
            let rows = self.open_unit(g, i, ctx, Mode::Probe)?.rows;
            if rows < start.1 {
                start = (i, rows);
            }
        }
        let (_, shape) = self.start_shape(g, start.0, Mode::Probe)?;
        let mut applied = Bits::default();
        let (_, mut shape) = self.filter_steps(g, &shape, &mut applied, false, Mode::Probe)?;
        let mut order = vec![start.0];
        let mut remaining: Vec<usize> = (0..g.units.len()).filter(|&i| i != start.0).collect();
        while !remaining.is_empty() {
            let mut best: Option<(usize, Choice)> = None;
            for i in Self::next_candidates(g, &shape, remaining.iter().copied()) {
                let ch = self.join_choice(g, &shape, i, None, Mode::Probe)?;
                if best.as_ref().is_none_or(|(_, b)| cheaper(&ch, b)) {
                    best = Some((i, ch));
                }
            }
            let (i, ch) = best.ok_or_else(|| Error::internal("greedy join order without a candidate"))?;
            remaining.retain(|&j| j != i);
            order.push(i);
            let joined = joined_shape(&shape, &g.units[i].props, Some(i), &ch, &JoinKind::Inner);
            shape = self.filter_steps(g, &joined, &mut applied, false, Mode::Probe)?.1;
        }
        Ok(order)
    }

    /// Build the plan joining the units in `order` (left-deep), choosing each operator and
    /// placing each filter with real plans of the inputs.
    fn build_order(&mut self, g: &mut Group<'_>, order: &[usize]) -> Result<Plan> {
        let mut applied = Bits::default();
        let (opened, shape) = self.start_shape(g, order[0], Mode::Build)?;
        let side = Side { plan: *opened.into_plan()?, shape };
        let mut side = self.apply_filters(g, side, &mut applied, false)?;
        for &i in &order[1..] {
            let ch = self.join_choice(g, &side.shape, i, None, Mode::Build)?;
            let shape = joined_shape(&side.shape, &g.units[i].props, Some(i), &ch, &JoinKind::Inner);
            let plan = join_plan(side.plan, ch, JoinKind::Inner)?;
            side = self.apply_filters(g, Side { plan, shape }, &mut applied, false)?;
        }
        Ok(self.apply_filters(g, side, &mut applied, true)?.plan)
    }

    /// Decide how every not-yet-applied conjunct whose evars the joined units bind (`own`; all
    /// remaining ones when `rest` is set: their original position, the top of the group) is
    /// applied; marks them applied and returns the resulting shape and, when building, the
    /// steps. Whole `[NOT] EXISTS` conjuncts may become Semi / Anti joins (see `semi`).
    fn filter_steps(
        &mut self,
        g: &mut Group<'_>,
        shape: &Shape,
        applied: &mut Bits,
        rest: bool,
        mode: Mode,
    ) -> Result<(Vec<Step>, Shape)> {
        let mut plain: SmallVec<[usize; 4]> = SmallVec::new();
        let mut sel = 1.0;
        let mut exists: SmallVec<[(usize, ExistsId, bool); 2]> = SmallVec::new();
        for (i, c) in g.conj.iter().enumerate() {
            if applied.contains(i) || !(rest || c.evars.is_subset(&shape.own)) {
                continue;
            }
            applied.insert(i);
            match semi::exists_conjunct(&c.expr) {
                Some((id, anti)) if !rest => exists.push((i, id, anti)),
                _ => {
                    sel *= c.sel;
                    plain.push(i);
                }
            }
        }
        let mut steps = Vec::new();
        let mut shape = shape.clone();
        if !plain.is_empty() {
            let n = plain.len() as f64;
            let (rows, cost) = (clamp_est(shape.rows * sel), clamp_est(shape.cost + shape.rows * n * EXPR_EVAL_NS));
            if mode == Mode::Build {
                if let Some(expr) = and_all(plain.iter().map(|&i| g.conj[i].expr.clone()).collect()) {
                    steps.push(Step::Filter { expr, rows, cost });
                }
            }
            (shape.rows, shape.cost) = (rows, cost);
        }
        for (i, id, anti) in exists {
            match self.semi_join_choice(&shape, id, g.conj[i].evars, g.ctx, mode)? {
                Some(choice) => {
                    let kind = semi::semi_kind(anti);
                    shape = joined_shape(&shape, &Props::default(), None, &choice, &kind);
                    if mode == Mode::Build {
                        steps.push(Step::Join { choice, kind });
                    }
                }
                None => {
                    let c = &g.conj[i];
                    let rows = clamp_est(shape.rows * c.sel);
                    let cost = clamp_est(shape.cost + shape.rows * EXPR_EVAL_NS);
                    if mode == Mode::Build {
                        steps.push(Step::Filter { expr: c.expr.clone(), rows, cost });
                    }
                    (shape.rows, shape.cost) = (rows, cost);
                }
            }
        }
        Ok((steps, shape))
    }

    /// `filter_steps` materialized on a side.
    fn apply_filters(&mut self, g: &mut Group<'_>, side: Side, applied: &mut Bits, rest: bool) -> Result<Side> {
        let (steps, shape) = self.filter_steps(g, &side.shape, applied, rest, Mode::Build)?;
        let mut plan = side.plan;
        for step in steps {
            plan = match step {
                Step::Filter { expr, rows, cost } => {
                    let sorted = plan.sorted_by;
                    mk(Op::Filter { input: Box::new(plan), expr }, rows, cost, sorted)
                }
                Step::Join { choice, kind } => join_plan(plan, choice, kind)?,
            };
        }
        Ok(Side { plan, shape })
    }

    /// Choose the operator joining `left` with unit `i`. `left_join` = Some(filter
    /// selectivity) for an OPTIONAL (kind Left), None for an inner join.
    ///
    /// Inside a group opened with seed S (`ctx.seeded`): a BindJoin re-opens the unit with its
    /// seed list only, so every slot both sides may bind, S included, must be carried by the
    /// left rows; HashJoin / NestedLoop open the unit with S itself when the join is inner, and
    /// slots of S are then neither keys nor checked (see the module contract).
    fn join_choice(
        &mut self,
        g: &mut Group<'_>,
        left: &Shape,
        i: usize,
        left_join: Option<f64>,
        mode: Mode,
    ) -> Result<Choice> {
        let (unit, ctx) = (g.units[i], g.ctx);
        let pr = &unit.props;
        let inner = left_join.is_none();
        let both_seeded = if inner { ctx.seeded } else { SlotSet::EMPTY };
        let shared = left.maybe.intersect(pr.maybe);
        let seed = left.certain.intersect(pr.maybe);
        let keys = left.certain.intersect(pr.certain).minus(both_seeded);
        let check = shared.minus(keys).minus(both_seeded);
        let bind_ok = !seed.is_empty() && shared.is_subset(&left.certain) && pr.accepts_seed(seed);
        let hash_ok = !keys.is_empty();
        let policy = self.opts.join_policy;
        let try_bind = bind_ok && policy != JoinPolicy::PreferHash;
        let try_hash = hash_ok && !(policy == JoinPolicy::PreferBind && bind_ok);
        // Inputs other than a BindJoin's right side receive the outer seed only for inner joins.
        let rctx = if inner { ctx } else { ctx.unseeded() };

        let lrows = left.rows;
        // Rows per left row when the unit extends a subject star (characteristic sets).
        let star = if inner { self.star_ratio(g, &left.units, i) } else { None };
        let finish_rows = |rows: f64| match left_join {
            Some(sel) => (rows * sel).max(lrows),
            None => rows,
        };
        let mut best: Option<Choice> = None;
        if try_bind {
            let right = self.open_unit(g, i, Ctx { seeded: seed, ..ctx }, mode)?;
            let sort_left = lrows > SORT_LEFT_ROWS;
            let mut per_probe = right.cost;
            if sort_left && matches!(unit.r, UnitRef::Pat(_)) {
                per_probe -= self.costs.seek_random - self.costs.seek_sorted;
            }
            let rows = finish_rows(lrows * star.unwrap_or(right.rows));
            let cost = left.cost + lrows * per_probe.max(0.0) + rows * self.row_copy();
            best = Some(Choice::new(Algo::Bind { seed, sort_left }, right, rows, cost));
        }
        if try_hash {
            let right = self.open_unit(g, i, rctx, mode)?;
            let per_left = match star {
                Some(r) => r,
                None => self.rows_given(g, i, keys, lrows, right.rows)?,
            };
            let rows = finish_rows(lrows * per_left);
            let rrows = right.rows;
            let build_is_left = inner && lrows < rrows;
            let (b, p) = if build_is_left { (lrows, rrows) } else { (rrows, lrows) };
            let (hb, hp) = if b < 32_768.0 {
                (self.costs.hash_build_small, self.costs.hash_probe_small)
            } else {
                (self.costs.hash_build_large, self.costs.hash_probe_large)
            };
            let cost = left.cost + right.cost + b * hb + p * hp + rows * self.row_copy();
            let ch = Choice::new(Algo::Hash { keys, check, build_is_left }, right, rows, cost);
            if best.as_ref().is_none_or(|b| cheaper(&ch, b)) {
                best = Some(ch);
            }
        }
        if let Some(ch) = best {
            return Ok(ch);
        }
        // No usable key: a cross product per seed value, checking the maybe-bound shared slots.
        let check = shared.minus(both_seeded);
        let right = self.open_unit(g, i, rctx, mode)?;
        let rrows = right.rows;
        let match_rate = if check.is_empty() { 1.0 } else { 0.5 };
        let rows = finish_rows(lrows * rrows * match_rate);
        let cost = left.cost + right.cost + lrows * rrows * 2.0 * self.costs.row_copy_per_slot + rows * self.row_copy();
        Ok(Choice::new(Algo::Nested { check }, right, rows, cost))
    }

    /// Rows of unit `i` per left row when joined on `keys` (estimate only); `standalone` = its
    /// rows without them.
    fn rows_given(&mut self, g: &mut Group<'_>, i: usize, keys: SlotSet, lrows: f64, standalone: f64) -> Result<f64> {
        let (unit, ctx) = (g.units[i], g.ctx);
        match unit.r {
            UnitRef::Pat(p) => Ok(self.group_scan_estimate(g, i, &p, keys.union(ctx.bound()))?.0),
            UnitRef::Node(n) if unit.props.accepts_seed(keys) => {
                Ok(self.estimate(n, Ctx { seeded: keys.union(ctx.seeded), ..ctx })?.rows)
            }
            UnitRef::Node(_) => Ok(standalone / standalone.max(lrows).max(1.0)),
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
        let certain = pl.certain.union(carried(&plan, ctx.seeded));
        let shape = Shape::of_input(plan.est_rows, plan.est_cost_ns, pl.certain, certain, pl.maybe.union(ctx.seeded));
        let mut g = Group::new(vec![Unit { r: UnitRef::Node(right), props: pr }], Vec::new(), ctx, &self.card);
        let ch = self.join_choice(&mut g, &shape, 0, Some(filter.map_or(1.0, selectivity)), Mode::Build)?;
        join_plan(plan, ch, JoinKind::Left { filter: filter.cloned() })
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
        // The right side is evaluated without the seed: seeded slots it binds are compared,
        // which needs them in the left rows (they are certain there: safe_inject of Minus).
        let keys = pl.certain.union(carried(&lplan, ctx.seeded)).intersect(pr.certain);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dp_step_counts() {
        // Dense groups of n units take n·2^(n-1) - n steps.
        for n in [3usize, 9, 10] {
            let all = (1usize << n) - 1;
            let adj: Vec<usize> = (0..n).map(|i| all & !(1 << i)).collect();
            assert_eq!(dp_steps(&adj, 0, usize::MAX), Some((n << (n - 1)) - n), "dense {n}");
        }
        // A chain of n units: every connected subset is an interval with at most two
        // neighbours; n(n-1) steps.
        let n = 12;
        let adj: Vec<usize> = (0..n).map(|i| ((1 << i) >> 1 | (1 << i) << 1) & ((1 << n) - 1)).collect();
        assert_eq!(dp_steps(&adj, 0, usize::MAX), Some(n * (n - 1)));
        // Over the limit: None. A dense 12-unit group is not searched by DP.
        let all = (1usize << 12) - 1;
        let dense: Vec<usize> = (0..12).map(|i| all & !(1 << i)).collect();
        assert_eq!(dp_steps(&dense, 0, MAX_DP_STEPS), None);
        // Disconnected units: every subset is reachable through cross products.
        assert_eq!(dp_steps(&[0, 0, 0], 0, usize::MAX), Some(9));
        // A unit touching the seed is connected to every prefix.
        assert_eq!(dp_candidates(&[0, 0, 0], 0b100, 0b001), 0b100);
        assert_eq!(dp_candidates(&[0b010, 0b001, 0], 0, 0b001), 0b010);
        assert_eq!(dp_candidates(&[0, 0, 0], 0, 0b001), 0b110);
    }
}
