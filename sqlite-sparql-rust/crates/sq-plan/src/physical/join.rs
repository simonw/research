//! Join groups (spec §7.3.1, §7.5): flatten Join / Bgp into units (quad patterns, paths,
//! Values and opaque subtrees), order them, choose a join operator per step and apply the
//! group's filter conjuncts as soon as the joined prefix binds their evars.
//!
//! Ordering: left-deep dynamic programming over connected subsets for groups of up to
//! [`MAX_DP_UNITS`] units whose search takes at most [`MAX_DP_STEPS`] join steps, greedy
//! (cheapest connected next unit, cross products last) otherwise, and a DP over the prefixes of
//! the syntactic order when `reorder` is off. The DPs keep the cheapest prefix per *interesting
//! property* ([`Key`]): unordered, sorted by a slot that a remaining unit binds (a later merge
//! join key), or an open SQL chain that a remaining unit may extend. Every decision depends only
//! on the estimated [`Shape`] of the joined prefix, so the search runs on shapes alone
//! ([`Mode::Probe`]: opaque units are estimated, not planned, see `estimate`; quad patterns are
//! costed from a per-group cache). The winning [`Path`] (first unit and access path, then the
//! operator of each step) is replayed with real plans ([`Mode::Build`]); a decision that the real
//! plans make illegal falls back to the cheapest legal operator of the same family, then to the
//! cheapest legal one.
//!
//! Operator choice per step lives in `choice`; SQL chains are rendered by `sqljoin` when they
//! close (a filter, a non-SQL step or the end of the group), StarMerge is a post-pass (`star`).
use std::cmp::Ordering;

use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use sq_core::batch::{Slot, SlotSet};
use sq_core::ir::{ExistsId, Expr, Logical, PatTerm, QuadPat};
use sq_core::plan::{IntRange, JoinKind, JoinPolicy, Op, Plan};
use sq_core::store::Col;
use sq_core::{Error, Result};

use super::bits::{mask_iter, Bits};
use super::range;
use super::scan::{bound_columns, choose_order, range_access, scan_sorted_by, sorted_accesses, sqlable, Access};
use super::sqljoin;
use super::{carried, clamp_est, mk, semi, Ctx, Planner, EXPR_EVAL_NS};
use crate::card::{same_star, selectivity_with, Card};
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
pub(super) enum UnitRef<'l> {
    Pat(QuadPat),
    Node(&'l Logical),
}

#[derive(Clone, Copy)]
pub(super) struct Unit<'l> {
    pub(super) r: UnitRef<'l>,
    pub(super) props: Props,
    /// A quad pattern that can be one alias of a SqlJoin statement.
    pub(super) sqlable: bool,
    /// The integer range that the group's conjuncts put on this quad pattern's object slot.
    pub(super) range: Option<IntRange>,
}

/// A filter conjunct of a group with its evars (relative to the group's rows).
struct Conj {
    expr: Expr,
    evars: SlotSet,
    sel: f64,
    /// The slot of an integer range conjunct (`?o > 3`): an IntRange scan of that object has
    /// already applied its selectivity.
    range_slot: Option<Slot>,
}

/// What the rows of a group are used for.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Goal {
    Rows,
    /// Only counted (the input of `Op::Count`): a merge join on top is factorized.
    Count,
}

/// One join group: its units and filter conjuncts, the context it is planned in, and caches
/// of the ordering search (which costs every unit in many contexts).
pub(super) struct Group<'l> {
    pub(super) units: Vec<Unit<'l>>,
    conj: Vec<Conj>,
    pub(super) ctx: Ctx,
    goal: Goal,
    /// Merge joins are allowed (and so sorted prefixes are interesting).
    pub(super) merge: bool,
    /// SQL chains (SqlJoin) are allowed.
    pub(super) sql: bool,
    /// Per unit that can extend a subject star (`Card::star_extends`): the other quad pattern
    /// units of that star (`same_star`); empty for every other unit.
    star_mates: Vec<Bits>,
    /// Characteristic-set estimates per star (a set of quad pattern units).
    star_memo: StarMemo,
    /// Per quad pattern unit: rows and cost of one open by bound columns (bit c set = column c
    /// of [g, s, p, o] bound, see `bound_columns`), which determine the estimate.
    scan_est: Vec<[Option<(f64, f64)>; 16]>,
    /// Per quad pattern unit: rows and cost of its IntRange scan.
    range_est: Vec<Option<(f64, f64)>>,
    /// Per quad pattern unit: its plain access paths in the group's context, one per output
    /// order (`sorted_accesses`).
    accesses: Vec<Option<SmallVec<[(Access, Option<Slot>); 4]>>>,
    /// Distinct values per slot (min over the patterns binding it; infinite when unknown).
    pub(super) dv: FxHashMap<Slot, f64>,
}

impl<'l> Group<'l> {
    fn new(units: Vec<Unit<'l>>, ctx: Ctx, goal: Goal, card: &Card<'_>) -> Self {
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
        let n = units.len();
        Group {
            units,
            conj: Vec::new(),
            ctx,
            goal,
            merge: false,
            sql: false,
            star_mates,
            star_memo: StarMemo::default(),
            scan_est: vec![[None; 16]; n],
            range_est: vec![None; n],
            accesses: vec![None; n],
            dv: FxHashMap::default(),
        }
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

    /// What the units in `rest` (still to be joined) make interesting: the slots they bind
    /// certainly (future merge keys) and whether one can extend an SQL chain.
    fn interest(&self, rest: impl Iterator<Item = usize>) -> (SlotSet, bool) {
        rest.fold((SlotSet::EMPTY, false), |(slots, sql), j| {
            let u = &self.units[j];
            (slots.union(u.props.certain), sql || u.sqlable)
        })
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

/// The number of join steps `dp_path` takes (one per reachable subset and candidate next
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
/// was opened with) its rows carry, the slot its output is sorted by, and its plan (only when
/// building; boxed so that the many probe-mode choices of the ordering search stay small).
#[derive(Clone)]
pub(super) struct Opened {
    pub(super) plan: Option<Box<Plan>>,
    pub(super) rows: f64,
    pub(super) cost: f64,
    pub(super) carried: SlotSet,
    pub(super) sorted: Option<Slot>,
    /// The object slot of an IntRange scan (its range conjuncts are already applied).
    pub(super) ranged: SlotSet,
}

impl Opened {
    pub(super) fn planned(plan: Plan, seeded: SlotSet) -> Opened {
        let ranged = match &plan.op {
            Op::Scan(s) if s.range.is_some() => match s.pat.o {
                PatTerm::Slot(o) => SlotSet::from_slots([o]),
                PatTerm::Const(_) => SlotSet::EMPTY,
            },
            _ => SlotSet::EMPTY,
        };
        let (rows, cost, carried, sorted) = (plan.est_rows, plan.est_cost_ns, carried(&plan, seeded), plan.sorted_by);
        Opened { plan: Some(Box::new(plan)), rows, cost, carried, sorted, ranged }
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
    pub(super) units: Bits,
    /// The slot the prefix's rows are sorted by (as the executor guarantees it).
    pub(super) sorted: Option<Slot>,
    /// Number of units of the open SQL chain the prefix is (0: none): one plain sqlable scan,
    /// or such scans joined by SQL steps only, with no filter applied yet.
    pub(super) chain: usize,
    /// The last operator is a merge join without checks: a COUNT of the prefix is the sum of
    /// run products (`fast_count`), no row is produced.
    pub(super) factorized: bool,
    /// Object slots read by IntRange scans: their range conjuncts' selectivity is already in
    /// `rows`.
    pub(super) ranged: SlotSet,
}

impl Shape {
    /// The shape of an input outside any join group (the left side of an OPTIONAL).
    fn of_input(plan: &Plan, own: SlotSet, certain: SlotSet, maybe: SlotSet) -> Shape {
        Shape {
            rows: plan.est_rows,
            cost: plan.est_cost_ns,
            own,
            certain,
            maybe,
            units: Bits::default(),
            sorted: plan.sorted_by,
            chain: 0,
            factorized: false,
            ranged: SlotSet::EMPTY,
        }
    }
}

/// A joined prefix: its plan and shape (`shape.rows / cost` = the plan's estimates).
struct Side {
    plan: Plan,
    shape: Shape,
}

/// A join operator decision (slot lists become vectors only when the plan is built).
#[derive(Copy, Clone, Debug)]
pub(super) enum Algo {
    Bind { seed: SlotSet, sort_left: bool },
    Hash { keys: SlotSet, check: SlotSet, build_is_left: bool },
    Nested { check: SlotSet },
    /// Both inputs sorted by `key`, through a SortBySlot enforcer where the input is not.
    Merge { key: Slot, check: SlotSet },
    /// A BindJoin step of an SQL chain: the chain is rendered as one SqlJoin when it closes.
    Sql { seed: SlotSet },
}

/// Operator families (what a replayed decision falls back to first).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(super) enum Family {
    Bind,
    Hash,
    Nested,
    Merge,
    Sql,
}

impl Algo {
    pub(super) fn family(&self) -> Family {
        match self {
            Algo::Bind { .. } => Family::Bind,
            Algo::Hash { .. } => Family::Hash,
            Algo::Nested { .. } => Family::Nested,
            Algo::Merge { .. } => Family::Merge,
            Algo::Sql { .. } => Family::Sql,
        }
    }

    /// The same decision (slot sets aside, which the replay recomputes).
    fn same_as(&self, o: &Algo) -> bool {
        match (self, o) {
            (Algo::Bind { sort_left: a, .. }, Algo::Bind { sort_left: b, .. }) => a == b,
            (Algo::Hash { build_is_left: a, .. }, Algo::Hash { build_is_left: b, .. }) => a == b,
            (Algo::Merge { key: a, .. }, Algo::Merge { key: b, .. }) => a == b,
            _ => self.family() == o.family(),
        }
    }
}

/// A join step: operator, right input, estimates of the joined result and its order.
pub(super) struct Choice {
    pub(super) algo: Algo,
    pub(super) right: Opened,
    pub(super) rows: f64,
    pub(super) cost: f64,
    pub(super) sorted: Option<Slot>,
}

impl Choice {
    /// A join step with its estimates kept finite (`clamp_est`).
    pub(super) fn new(algo: Algo, right: Opened, rows: f64, cost: f64, sorted: Option<Slot>) -> Choice {
        Choice { algo, right, rows: clamp_est(rows), cost: clamp_est(cost), sorted }
    }

    /// A merge join without checks: COUNT is factorized over it.
    fn factorized(&self) -> bool {
        matches!(self.algo, Algo::Merge { check, .. } if check.is_empty())
    }

}

/// A join order with its decisions: the first unit and its access path, then the operator
/// joining each further unit (the operator decides how that unit is read).
struct Path {
    start: (usize, Option<Access>),
    steps: Vec<(usize, Algo)>,
}

/// How newly complete filter conjuncts are applied.
enum Step {
    Filter { expr: Expr, rows: f64, cost: f64 },
    Join { choice: Choice, kind: JoinKind },
}

/// The interesting property a DP keeps one cheapest prefix for (spec §7.5 interesting orders).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Key {
    Any,
    /// Sorted by a slot a remaining unit binds certainly (a possible merge key).
    Sorted(Slot),
    /// An open SQL chain of at least two units that a remaining unit may extend.
    Sql,
}

/// Where a DP state comes from.
#[derive(Copy, Clone, Debug)]
enum From {
    Start { unit: usize, access: Option<Access> },
    /// `prev` / `idx`: the bucket and index of the state extended by `unit`.
    Step { prev: usize, idx: usize, unit: usize, algo: Algo },
}

/// A DP state: the cheapest shape found for a (subset, key) and how it was reached.
struct DpState {
    key: Key,
    /// What states compete on: the cost, or the final cost of a complete group.
    rank: f64,
    shape: Shape,
    /// The conjuncts applied so far.
    applied: Bits,
    from: From,
}

type Bucket = SmallVec<[DpState; 2]>;

/// Keep `st` unless the bucket has a state of the same key that is at most as expensive; an
/// unordered state is also dominated by any state at most as expensive (an order or an open
/// SQL chain can always be ignored).
fn insert(bucket: &mut Bucket, st: DpState) {
    if st.key == Key::Any && bucket.iter().any(|s| s.rank <= st.rank) {
        return;
    }
    match bucket.iter_mut().find(|s| s.key == st.key) {
        Some(s) => {
            if st.rank < s.rank {
                *s = st;
            }
        }
        None => bucket.push(st),
    }
}

/// The key a prefix competes under; `rest` is what the units still to join make interesting
/// ([`Group::interest`]).
fn key_of(g: &Group<'_>, shape: &Shape, rest: (SlotSet, bool)) -> Key {
    let (interesting, sql_rest) = rest;
    if shape.chain >= 2 && sql_rest {
        return Key::Sql;
    }
    match shape.sorted {
        Some(v) if g.merge && interesting.contains(v) => Key::Sorted(v),
        _ => Key::Any,
    }
}

/// The cheapest state of a complete bucket.
fn cheapest(bucket: &Bucket) -> Result<usize> {
    (0..bucket.len())
        .min_by(|&a, &b| bucket[a].rank.total_cmp(&bucket[b].rank))
        .ok_or_else(|| Error::internal("join DP left a subset unreached"))
}

/// Walk the back pointers from `buckets[at][idx]` to the first unit.
fn reconstruct(buckets: &[Bucket], mut at: usize, mut idx: usize) -> Path {
    let mut steps = Vec::new();
    loop {
        match buckets[at][idx].from {
            From::Step { prev, idx: i, unit, algo } => {
                steps.push((unit, algo));
                (at, idx) = (prev, i);
            }
            From::Start { unit, access } => {
                steps.reverse();
                return Path { start: (unit, access), steps };
            }
        }
    }
}

fn cheaper(a: &Choice, b: &Choice) -> bool {
    a.cost.total_cmp(&b.cost).then(a.rows.total_cmp(&b.rows)) == Ordering::Less
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
        sorted: ch.sorted,
        chain: if matches!(ch.algo, Algo::Sql { .. }) { left.chain + 1 } else { 0 },
        factorized: ch.factorized(),
        ranged: if inner { left.ranged.union(ch.right.ranged) } else { left.ranged },
    }
}

impl<'a> Planner<'a> {
    /// A Bgp / Join tree, optionally under Filters: one join group.
    pub(crate) fn plan_group(&mut self, l: &Logical, ctx: Ctx, goal: Goal) -> Result<Plan> {
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
        let mut g = Group::new(units, ctx, goal, &self.card);
        let policy = self.opts.join_policy;
        g.merge = !ctx.in_exists && matches!(policy, JoinPolicy::Auto | JoinPolicy::NoSql | JoinPolicy::PreferMerge);
        g.sql = policy == JoinPolicy::Auto && self.caps.sql && ctx.seeded.is_empty() && !ctx.in_exists;
        let group_maybe = self.lp.props(body).maybe;
        self.add_conjuncts(&mut g, exprs, group_maybe)?;
        if g.units.is_empty() {
            let shape = Shape {
                rows: 1.0,
                cost: 0.0,
                own: SlotSet::EMPTY,
                certain: ctx.seeded,
                maybe: ctx.seeded,
                units: Bits::default(),
                sorted: None,
                chain: 0,
                factorized: false,
                ranged: SlotSet::EMPTY,
            };
            let side = Side { plan: mk(Op::Unit, 1.0, 0.0, None), shape };
            let mut applied = Bits::default();
            let side = self.apply_filters(&mut g, side, &mut applied, false)?;
            return Ok(self.apply_filters(&mut g, side, &mut applied, true)?.plan);
        }
        let path = if !self.opts.reorder {
            self.fixed_path(&mut g)?
        } else {
            // Adjacency masks exist for DP-sized groups only.
            let dp = (g.units.len() <= MAX_DP_UNITS)
                .then(|| g.adjacency())
                .filter(|(adj, seeded)| dp_steps(adj, *seeded, MAX_DP_STEPS).is_some());
            match dp {
                Some((adj, seeded)) => self.dp_path(&mut g, &adj, seeded)?,
                None => self.greedy_path(&mut g)?,
            }
        };
        self.build_path(&mut g, &path)
    }

    /// Flatten Join / Bgp into units; Unit is dropped. Returns false if a unit is Empty.
    fn collect_units<'l>(&self, l: &'l Logical, out: &mut Vec<Unit<'l>>) -> bool {
        match l {
            Logical::Join(a, b) => self.collect_units(a, out) && self.collect_units(b, out),
            Logical::Bgp(pats) => {
                out.extend(pats.iter().map(|p| Unit {
                    r: UnitRef::Pat(*p),
                    props: props::pattern(p),
                    sqlable: sqlable(p, self.dataset),
                    range: None,
                }));
                true
            }
            Logical::Unit => true,
            Logical::Empty => false,
            other => {
                out.push(Unit { r: UnitRef::Node(other), props: self.lp.props(other), sqlable: false, range: None });
                true
            }
        }
    }

    /// The group's filter conjuncts: evars, integer ranges (put on the quad patterns whose
    /// object they restrict) and selectivities.
    fn add_conjuncts(&mut self, g: &mut Group<'_>, exprs: Vec<Expr>, group_maybe: SlotSet) -> Result<()> {
        let mut ranges: SmallVec<[(Slot, Option<IntRange>); 2]> = SmallVec::new();
        for e in exprs {
            let bounds = range::int_bounds(&e, &mut |id| self.numeric(id));
            if let Some((s, r)) = bounds {
                match ranges.iter_mut().find(|(x, _)| *x == s) {
                    Some((_, acc)) => *acc = acc.and_then(|a| range::intersect(a, r)),
                    None => ranges.push((s, Some(r))),
                }
            }
            let evars = self.lp.evars(&e, group_maybe);
            g.conj.push(Conj { evars, sel: 1.0, range_slot: bounds.map(|b| b.0), expr: e });
        }
        for u in &mut g.units {
            if let UnitRef::Pat(QuadPat { o: PatTerm::Slot(o), .. }) = u.r {
                u.range = ranges.iter().find(|(s, _)| *s == o).and_then(|(_, r)| *r);
            }
        }
        for i in 0..g.conj.len() {
            let e = g.conj[i].expr.clone();
            g.conj[i].sel = self.conj_selectivity(g, &e)?;
        }
        Ok(())
    }

    /// §7.4.5: `=` against a constant (or another slot) is 1/dv, an integer range on a
    /// predicate's object is uniform over its [imin, imax], other shapes use fixed factors.
    fn conj_selectivity(&mut self, g: &mut Group<'_>, e: &Expr) -> Result<f64> {
        let mut eq_slots: SmallVec<[Slot; 4]> = SmallVec::new();
        crate::expr::visit(e, &mut |x| {
            if let Expr::Equal(a, b) = x {
                for side in [a, b] {
                    if let Expr::Slot(s) = **side {
                        eq_slots.push(s);
                    }
                }
            }
        });
        for s in eq_slots {
            self.slot_dv(g, s)?;
        }
        let dv = |s: Slot| g.dv.get(&s).copied().filter(|d| d.is_finite());
        let units = &g.units;
        let card = &self.card;
        let mut num = |id| self.numeric(id);
        Ok(selectivity_with(e, &mut |x| match x {
            Expr::Equal(a, b) => match (&**a, &**b) {
                (Expr::Slot(s), Expr::Const(_)) | (Expr::Const(_), Expr::Slot(s)) => dv(*s).map(|d| 1.0 / d.max(1.0)),
                (Expr::Slot(s), Expr::Slot(t)) => Some(1.0 / dv(*s)?.max(dv(*t)?).max(1.0)),
                _ => None,
            },
            Expr::Greater(..) | Expr::GreaterOrEqual(..) | Expr::Less(..) | Expr::LessOrEqual(..) => {
                let (s, r) = range::int_bounds(x, &mut num)?;
                units.iter().find_map(|u| match u.r {
                    UnitRef::Pat(p) if p.o == PatTerm::Slot(s) && matches!(p.p, PatTerm::Const(_)) => {
                        card.int_range_fraction(&p, r)
                    }
                    _ => None,
                })
            }
            _ => None,
        }))
    }

    /// Distinct values of slot `s` among the group's rows: the smallest distinct count of a
    /// column binding it in one of the group's quad patterns (statistics, else a guess from the
    /// pattern's rows); infinite when no quad pattern binds it. Cached per group.
    pub(super) fn slot_dv(&mut self, g: &mut Group<'_>, s: Slot) -> Result<f64> {
        if let Some(d) = g.dv.get(&s) {
            return Ok(*d);
        }
        let mut best = f64::INFINITY;
        for u in &g.units {
            let UnitRef::Pat(p) = u.r else { continue };
            for (col, t) in [(Col::S, p.s), (Col::P, p.p), (Col::O, p.o)] {
                if t == PatTerm::Slot(s) {
                    let rows = self.card.pattern_rows(&p, SlotSet::EMPTY)?;
                    best = best.min(self.card.distinct_values(&p, col, rows).min(rows.max(1.0)));
                }
            }
        }
        g.dv.insert(s, best);
        Ok(best)
    }

    /// The access paths a DP tries for unit `i` opened in the group's context: the default
    /// one (None), the other sorted orders when merge joins are possible, and the IntRange scan.
    fn start_accesses(&self, g: &mut Group<'_>, i: usize) -> SmallVec<[Option<Access>; 4]> {
        let mut out: SmallVec<[Option<Access>; 4]> = SmallVec::new();
        out.push(None);
        if let UnitRef::Pat(p) = g.units[i].r {
            if g.merge {
                out.extend(self.unit_accesses(g, i, &p).into_iter().skip(1).map(|(a, _)| Some(a)));
            }
            if let Some(a) = range_access(&p, g.ctx.bound(), g.units[i].range, self.dataset) {
                out.push(Some(a));
            }
        }
        out
    }

    /// `sorted_accesses` of quad pattern unit `i` in the group's context (cached per group).
    pub(super) fn unit_accesses(
        &self,
        g: &mut Group<'_>,
        i: usize,
        p: &QuadPat,
    ) -> SmallVec<[(Access, Option<Slot>); 4]> {
        if let Some(a) = &g.accesses[i] {
            return a.clone();
        }
        let a = sorted_accesses(p, g.ctx.bound(), self.dataset);
        g.accesses[i] = Some(a.clone());
        a
    }

    /// Open unit `i` in `ctx` (see [`Opened`]), a quad pattern through `access` (None: the
    /// default order for its bound columns).
    pub(super) fn open_unit(
        &mut self,
        g: &mut Group<'_>,
        i: usize,
        ctx: Ctx,
        mode: Mode,
        access: Option<Access>,
    ) -> Result<Opened> {
        match (g.units[i].r, mode) {
            (UnitRef::Pat(p), Mode::Build) => Ok(Opened::planned(self.plan_scan_with(&p, ctx, access)?, ctx.seeded)),
            // A scan's rows carry the whole seed (`carried`).
            (UnitRef::Pat(p), Mode::Probe) => {
                let bound = ctx.bound();
                let range = access.and_then(|a| a.range);
                let (rows, cost) = match range {
                    Some(r) => self.group_range_estimate(g, i, &p, r)?,
                    None => self.group_scan_estimate(g, i, &p, bound)?,
                };
                let cols = bound_columns(&p, bound);
                let k = cols.iter().filter(|b| **b).count();
                let order = access.map_or_else(|| choose_order(cols).0, |a| a.order);
                let sorted =
                    if ctx.in_exists { None } else { scan_sorted_by(&p, order, k, range.is_some(), self.dataset) };
                let ranged = match (range, p.o) {
                    (Some(_), PatTerm::Slot(o)) => SlotSet::from_slots([o]),
                    _ => SlotSet::EMPTY,
                };
                Ok(Opened { plan: None, rows, cost, carried: ctx.seeded, sorted, ranged })
            }
            (UnitRef::Node(n), _) => self.open_node(n, ctx, mode),
        }
    }

    /// `scan_estimate` of quad pattern unit `i` with `bound` slots bound, cached per group.
    pub(super) fn group_scan_estimate(
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

    /// `range_scan_estimate` of quad pattern unit `i`, cached per group.
    fn group_range_estimate(&mut self, g: &mut Group<'_>, i: usize, pat: &QuadPat, r: IntRange) -> Result<(f64, f64)> {
        if let Some(e) = g.range_est[i] {
            return Ok(e);
        }
        let (rows, cost) = self.range_scan_estimate(pat, r)?;
        let e = (clamp_est(rows), clamp_est(cost));
        g.range_est[i] = Some(e);
        Ok(e)
    }

    /// Open an opaque subtree in `ctx`: planned when building, estimated when probing (no
    /// order is assumed for an estimate).
    pub(super) fn open_node(&mut self, n: &Logical, ctx: Ctx, mode: Mode) -> Result<Opened> {
        match mode {
            Mode::Build => Ok(Opened::planned(self.plan(n, ctx)?, ctx.seeded)),
            Mode::Probe => {
                let e = self.estimate(n, ctx)?;
                let carried = ctx.seeded.intersect(e.carries);
                Ok(Opened { plan: None, rows: e.rows, cost: e.cost, carried, sorted: None, ranged: SlotSet::EMPTY })
            }
        }
    }

    /// Unit `i` opened in the group's context through `access`, and its shape.
    fn start_shape(
        &mut self,
        g: &mut Group<'_>,
        i: usize,
        mode: Mode,
        access: Option<Access>,
    ) -> Result<(Opened, Shape)> {
        let ctx = g.ctx;
        let opened = self.open_unit(g, i, ctx, mode, access)?;
        let unit = &g.units[i];
        let props = unit.props;
        let chain = usize::from(g.sql && unit.sqlable && opened.ranged.is_empty());
        let shape = Shape {
            rows: opened.rows,
            cost: opened.cost,
            own: props.certain,
            certain: props.certain.union(opened.carried),
            maybe: props.maybe.union(ctx.seeded),
            units: Bits::single(i),
            sorted: opened.sorted,
            chain,
            factorized: false,
            ranged: opened.ranged,
        };
        Ok((opened, shape))
    }

    /// Rows per left row when unit `i` extends a subject star of the prefix `joined`
    /// (characteristic sets, §7.4.4); None when no star applies.
    pub(super) fn star_ratio(&self, g: &mut Group<'_>, joined: &Bits, i: usize) -> Option<f64> {
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

    /// What a complete group costs for its goal: under a COUNT a factorized merge join on top
    /// produces no row.
    fn final_cost(&self, g: &Group<'_>, shape: &Shape) -> f64 {
        match g.goal {
            Goal::Count if shape.factorized => (shape.cost - shape.rows * self.row_copy()).max(0.0),
            _ => shape.cost,
        }
    }

    /// The DP state of `shape` (before the conjuncts it completes are applied): filters
    /// applied, key and rank computed. `rest` is what the units still to join make interesting.
    fn settle(
        &mut self,
        g: &mut Group<'_>,
        shape: &Shape,
        mut applied: Bits,
        from: From,
        rest: (SlotSet, bool),
        complete: bool,
    ) -> Result<DpState> {
        let (_, shape) = self.filter_steps(g, shape, &mut applied, false, Mode::Probe)?;
        let key = key_of(g, &shape, rest);
        let rank = if complete { self.final_cost(g, &shape) } else { shape.cost };
        Ok(DpState { key, rank, shape, applied, from })
    }

    /// Can the join step to `shape` (before its filters) not improve `bucket`? Filters only add
    /// cost, keep the order and may close an SQL chain (whose rows are unordered, so the state
    /// then competes as `Key::Any`): a state of the same key, or of any key for an unordered
    /// one, that is at most as expensive already dominates it.
    fn dominated(&self, g: &Group<'_>, bucket: &Bucket, shape: &Shape, rest: (SlotSet, bool), complete: bool) -> bool {
        let key = key_of(g, shape, rest);
        let rank = if complete { self.final_cost(g, shape) } else { shape.cost };
        bucket.iter().any(|s| (key == Key::Any || s.key == key) && s.rank <= rank)
    }

    /// Left-deep dynamic programming over connected subsets (DPsub) on shapes only, keeping the
    /// cheapest prefix per (subset, [`Key`]). `adj` / `seeded` are the group's
    /// [`Group::adjacency`].
    fn dp_path(&mut self, g: &mut Group<'_>, adj: &[usize], seeded: usize) -> Result<Path> {
        let n = g.units.len();
        let full = (1usize << n) - 1;
        let mut best: Vec<Bucket> = (0..=full).map(|_| Bucket::new()).collect();
        for i in 0..n {
            let rest = g.interest(mask_iter(full & !(1 << i)));
            for access in self.start_accesses(g, i) {
                let (_, shape) = self.start_shape(g, i, Mode::Probe, access)?;
                let st = self.settle(g, &shape, Bits::default(), From::Start { unit: i, access }, rest, n == 1)?;
                insert(&mut best[1 << i], st);
            }
        }
        // Every superset of a subset is numerically larger: ascending order sees final states,
        // and back pointers only point into finished buckets.
        for mask in 1..full {
            let bucket = std::mem::take(&mut best[mask]);
            for i in mask_iter(dp_candidates(adj, seeded, mask)) {
                let next = mask | 1 << i;
                let rest = g.interest(mask_iter(full & !next));
                for (idx, state) in bucket.iter().enumerate() {
                    for ch in self.join_choices(g, &state.shape, i, None, Mode::Probe, None)? {
                        let shape = joined_shape(&state.shape, &g.units[i].props, Some(i), &ch, &JoinKind::Inner);
                        if self.dominated(g, &best[next], &shape, rest, next == full) {
                            continue;
                        }
                        let from = From::Step { prev: mask, idx, unit: i, algo: ch.algo };
                        let st = self.settle(g, &shape, state.applied.clone(), from, rest, next == full)?;
                        insert(&mut best[next], st);
                    }
                }
            }
            best[mask] = bucket;
        }
        let idx = cheapest(&best[full])?;
        Ok(reconstruct(&best, full, idx))
    }

    /// `reorder = false`: the syntactic order, with operators and access paths chosen by a DP
    /// over its prefixes (cheapest per [`Key`]).
    fn fixed_path(&mut self, g: &mut Group<'_>) -> Result<Path> {
        let n = g.units.len();
        let mut buckets: Vec<Bucket> = (0..n).map(|_| Bucket::new()).collect();
        let rest = g.interest(1..n);
        for access in self.start_accesses(g, 0) {
            let (_, shape) = self.start_shape(g, 0, Mode::Probe, access)?;
            let st = self.settle(g, &shape, Bits::default(), From::Start { unit: 0, access }, rest, n == 1)?;
            insert(&mut buckets[0], st);
        }
        for k in 1..n {
            let prev = std::mem::take(&mut buckets[k - 1]);
            let rest = g.interest(k + 1..n);
            for (idx, state) in prev.iter().enumerate() {
                for ch in self.join_choices(g, &state.shape, k, None, Mode::Probe, None)? {
                    let shape = joined_shape(&state.shape, &g.units[k].props, Some(k), &ch, &JoinKind::Inner);
                    if self.dominated(g, &buckets[k], &shape, rest, k == n - 1) {
                        continue;
                    }
                    let from = From::Step { prev: k - 1, idx, unit: k, algo: ch.algo };
                    let st = self.settle(g, &shape, state.applied.clone(), from, rest, k == n - 1)?;
                    insert(&mut buckets[k], st);
                }
            }
            buckets[k - 1] = prev;
        }
        let idx = cheapest(&buckets[n - 1])?;
        Ok(reconstruct(&buckets, n - 1, idx))
    }

    /// Greedy order: the unit with the fewest rows first, then the cheapest connected next
    /// unit and operator.
    fn greedy_path(&mut self, g: &mut Group<'_>) -> Result<Path> {
        let ctx = g.ctx;
        let mut start = (0, f64::INFINITY);
        for i in 0..g.units.len() {
            let rows = self.open_unit(g, i, ctx, Mode::Probe, None)?.rows;
            if rows < start.1 {
                start = (i, rows);
            }
        }
        let (_, shape) = self.start_shape(g, start.0, Mode::Probe, None)?;
        let mut applied = Bits::default();
        let (_, mut shape) = self.filter_steps(g, &shape, &mut applied, false, Mode::Probe)?;
        let mut path = Path { start: (start.0, None), steps: Vec::new() };
        let mut remaining: Vec<usize> = (0..g.units.len()).filter(|&i| i != start.0).collect();
        while !remaining.is_empty() {
            let mut best: Option<(usize, Choice)> = None;
            for i in Self::next_candidates(g, &shape, remaining.iter().copied()) {
                for ch in self.join_choices(g, &shape, i, None, Mode::Probe, None)? {
                    if best.as_ref().is_none_or(|(_, b)| cheaper(&ch, b)) {
                        best = Some((i, ch));
                    }
                }
            }
            let (i, ch) = best.ok_or_else(|| Error::internal("greedy join order without a candidate"))?;
            remaining.retain(|&j| j != i);
            path.steps.push((i, ch.algo));
            let joined = joined_shape(&shape, &g.units[i].props, Some(i), &ch, &JoinKind::Inner);
            shape = self.filter_steps(g, &joined, &mut applied, false, Mode::Probe)?.1;
        }
        Ok(path)
    }

    /// Build the plan of `path` (left-deep), replaying each recorded decision with real plans
    /// and placing each filter as during the search.
    fn build_path(&mut self, g: &mut Group<'_>, path: &Path) -> Result<Plan> {
        let mut applied = Bits::default();
        let (first, access) = path.start;
        let (opened, shape) = self.start_shape(g, first, Mode::Build, access)?;
        let side = Side { plan: *opened.into_plan()?, shape };
        let mut side = self.apply_filters(g, side, &mut applied, false)?;
        for &(i, algo) in &path.steps {
            let ch = self.replay_choice(g, &side.shape, i, &algo)?;
            if !matches!(ch.algo, Algo::Sql { .. }) {
                side = self.close_sql(side)?;
            }
            let shape = joined_shape(&side.shape, &g.units[i].props, Some(i), &ch, &JoinKind::Inner);
            let plan = self.join_plan(side.plan, ch, JoinKind::Inner)?;
            side = self.apply_filters(g, Side { plan, shape }, &mut applied, false)?;
        }
        let side = self.apply_filters(g, side, &mut applied, true)?;
        let side = self.close_sql(side)?;
        Ok(self.star_merges(side.plan))
    }

    /// The real-plan choice for a recorded step: the same decision when it is still legal,
    /// else the cheapest legal one of the same family, else the cheapest legal one.
    fn replay_choice(&mut self, g: &mut Group<'_>, left: &Shape, i: usize, algo: &Algo) -> Result<Choice> {
        let family = algo.family();
        let mut chs = self.join_choices(g, left, i, None, Mode::Build, Some(family))?;
        if chs.is_empty() {
            chs = self.join_choices(g, left, i, None, Mode::Build, None)?;
        }
        let exact = chs.iter().position(|c| c.algo.same_as(algo));
        let pick = match exact {
            Some(p) => p,
            None => {
                let same: Vec<usize> = (0..chs.len()).filter(|&j| chs[j].algo.family() == family).collect();
                let pool: Vec<usize> = if same.is_empty() { (0..chs.len()).collect() } else { same };
                pool.into_iter()
                    .reduce(|a, b| if cheaper(&chs[b], &chs[a]) { b } else { a })
                    .ok_or_else(|| Error::internal("no legal join operator to replay"))?
            }
        };
        Ok(chs.swap_remove(pick))
    }

    /// Render an open SQL chain of at least two units as one SqlJoin; the chain is closed
    /// either way.
    fn close_sql(&mut self, mut side: Side) -> Result<Side> {
        if side.shape.chain >= 2 {
            let scans = sqljoin::chain_scans(&side.plan)
                .ok_or_else(|| Error::internal("an SQL chain is not a BindJoin chain of scans"))?;
            let r = sqljoin::render(&scans, self.dataset).ok_or_else(|| Error::internal("unrenderable SQL chain"))?;
            let op = Op::SqlJoin { sql: r.sql, params: r.params, out: r.out };
            side.plan = mk(op, side.shape.rows, side.shape.cost, None);
        }
        side.shape.chain = 0;
        Ok(side)
    }

    /// The plan of `left ⋈ right` for a decided join (merge inputs that are not sorted by the
    /// key get a SortBySlot enforcer).
    pub(super) fn join_plan(&self, left: Plan, ch: Choice, kind: JoinKind) -> Result<Plan> {
        let (rows, cost, sorted) = (ch.rows, ch.cost, ch.sorted);
        let right = ch.right.into_plan()?;
        let left = Box::new(left);
        let op = match ch.algo {
            Algo::Bind { seed, sort_left } => {
                Op::BindJoin { left, right, seed: slot_list(seed), kind, sort_left, adaptive: true }
            }
            // Rendered into a SqlJoin when the chain closes; never adaptive meanwhile.
            Algo::Sql { seed } => {
                Op::BindJoin { left, right, seed: slot_list(seed), kind, sort_left: false, adaptive: false }
            }
            Algo::Hash { keys, check, build_is_left } => {
                let (build, probe) = if build_is_left { (left, right) } else { (right, left) };
                Op::HashJoin { build, probe, keys: slot_list(keys), check: slot_list(check), kind, build_is_left }
            }
            Algo::Nested { check } => Op::NestedLoop { left, right, check: slot_list(check), kind },
            Algo::Merge { key, check, .. } => Op::MergeJoin {
                left: Box::new(self.sorted_input(*left, key)),
                right: Box::new(self.sorted_input(*right, key)),
                key,
                check: slot_list(check),
                gallop: self.gallop,
            },
        };
        Ok(mk(op, rows, cost, sorted))
    }

    /// `p`, or `p` under a SortBySlot enforcer when it is not sorted by `key`.
    fn sorted_input(&self, p: Plan, key: Slot) -> Plan {
        if p.sorted_by == Some(key) {
            return p;
        }
        let (rows, cost) = (p.est_rows, p.est_cost_ns + self.enforce_cost(p.est_rows));
        mk(Op::SortBySlot { input: Box::new(p), key }, rows, cost, Some(key))
    }

    /// Decide how every not-yet-applied conjunct whose evars the joined units bind (`own`; all
    /// remaining ones when `rest` is set: their original position, the top of the group) is
    /// applied; marks them applied and returns the resulting shape and, when building, the
    /// steps. Whole `[NOT] EXISTS` conjuncts may become Semi / Anti joins (see `semi`).
    /// Applying a conjunct closes an SQL chain and ends a factorized count.
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
                    if !c.range_slot.is_some_and(|s| shape.ranged.contains(s)) {
                        sel *= c.sel;
                    }
                    plain.push(i);
                }
            }
        }
        let mut steps = Vec::new();
        let mut shape = shape.clone();
        if plain.is_empty() && exists.is_empty() {
            return Ok((steps, shape));
        }
        shape.chain = 0;
        shape.factorized = false;
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

    /// `filter_steps` materialized on a side (an open SQL chain is rendered first).
    fn apply_filters(&mut self, g: &mut Group<'_>, side: Side, applied: &mut Bits, rest: bool) -> Result<Side> {
        let (steps, shape) = self.filter_steps(g, &side.shape, applied, rest, Mode::Build)?;
        if steps.is_empty() {
            return Ok(Side { plan: side.plan, shape });
        }
        let mut plan = self.close_sql(side)?.plan;
        for step in steps {
            plan = match step {
                Step::Filter { expr, rows, cost } => {
                    let sorted = plan.sorted_by;
                    mk(Op::Filter { input: Box::new(plan), expr }, rows, cost, sorted)
                }
                Step::Join { choice, kind } => self.join_plan(plan, choice, kind)?,
            };
        }
        Ok(Side { plan, shape })
    }

    /// OPTIONAL: BindJoin{Left} when the seeds are safe for the right side, else
    /// HashJoin{Left} (build right) or NestedLoop{Left}, the cheapest. The outer seed goes to
    /// the left only.
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
        let shape = Shape::of_input(&plan, pl.certain, certain, pl.maybe.union(ctx.seeded));
        let unit = Unit { r: UnitRef::Node(right), props: pr, sqlable: false, range: None };
        let mut g = Group::new(vec![unit], ctx, Goal::Rows, &self.card);
        let sel = filter.map_or(1.0, crate::card::selectivity);
        let chs = self.join_choices(&mut g, &shape, 0, Some(sel), Mode::Build, None)?;
        let ch = chs
            .into_iter()
            .reduce(|a, b| if cheaper(&b, &a) { b } else { a })
            .ok_or_else(|| Error::internal("OPTIONAL without a join operator"))?;
        self.join_plan(plan, ch, JoinKind::Left { filter: filter.cloned() })
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
        let (hb, hp) = self.hash_costs(r);
        let cost = lplan.est_cost_ns + rplan.est_cost_ns + r * hb + l * hp;
        let rows = if r > 0.0 { l * 0.5 } else { l };
        // Left rows stream through in order.
        let sorted = lplan.sorted_by.filter(|s| pl.certain.contains(*s));
        let op = Op::Minus {
            left: Box::new(lplan),
            right: Box::new(rplan),
            keys: slot_list(keys),
            check: slot_list(check),
        };
        Ok(mk(op, rows, cost, sorted))
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

    fn shape(rows: f64) -> Shape {
        Shape {
            rows,
            cost: rows,
            own: SlotSet::EMPTY,
            certain: SlotSet::EMPTY,
            maybe: SlotSet::EMPTY,
            units: Bits::default(),
            sorted: None,
            chain: 0,
            factorized: false,
            ranged: SlotSet::EMPTY,
        }
    }

    fn state(key: Key, rank: f64) -> DpState {
        DpState { key, rank, shape: shape(rank), applied: Bits::default(), from: From::Start { unit: 0, access: None } }
    }

    #[test]
    fn buckets_keep_the_cheapest_state_per_key() {
        let mut b = Bucket::new();
        insert(&mut b, state(Key::Any, 10.0));
        insert(&mut b, state(Key::Sorted(3), 12.0));
        assert_eq!(b.len(), 2, "a dearer sorted prefix may still win through a merge join");
        insert(&mut b, state(Key::Sorted(3), 11.0));
        assert_eq!(b.iter().find(|s| s.key == Key::Sorted(3)).unwrap().rank, 11.0);
        insert(&mut b, state(Key::Sorted(3), 13.0));
        assert_eq!(b.iter().find(|s| s.key == Key::Sorted(3)).unwrap().rank, 11.0);
        // An unordered prefix is dominated by any state that is at most as expensive.
        insert(&mut b, state(Key::Any, 11.5));
        assert_eq!(b.iter().find(|s| s.key == Key::Any).unwrap().rank, 10.0);
        insert(&mut b, state(Key::Sql, 9.0));
        insert(&mut b, state(Key::Any, 9.5));
        assert_eq!(b.iter().find(|s| s.key == Key::Any).unwrap().rank, 10.0);
        assert_eq!(b[cheapest(&b).unwrap()].key, Key::Sql);
    }
}
