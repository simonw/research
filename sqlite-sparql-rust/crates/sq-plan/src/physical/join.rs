//! Join groups (spec §7.3.1, §7.5): flatten Join / Bgp into units (quad patterns, paths,
//! Values and opaque subtrees), order them, choose a join operator per step and apply the
//! group's filter conjuncts as soon as the joined prefix binds their evars.
//!
//! Ordering: left-deep dynamic programming over connected subsets for groups of up to
//! [`MAX_DP_UNITS`] units whose search does at most [`MAX_DP_WORK`] units of work (see
//! [`dp_work`]), greedy (cheapest connected next unit, cross products last) otherwise, and a DP
//! over the prefixes of the syntactic order when `reorder` is off. The DPs keep the cheapest
//! prefix per *interesting property* ([`Key`]): unordered, sorted by a slot that a remaining
//! unit binds (a later merge join key), or an open SQL chain that a remaining unit may extend.
//! Only the cheapest prefix of a subset is extended with every operator; one kept for its
//! property alone only with the operators that keep it (`choice::Wanted`). When keeping every
//! property would exceed the budget, the DP keeps fewer of them ([`Keep::TIERS`]), down to the
//! cheapest prefix per subset, before it falls back to greedy. Every decision depends only
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
use super::choice::Wanted;
use super::range;
use super::scan::{bound_columns, choose_order, range_access, scan_sorted_by, sorted_accesses, sqlable, Access};
use super::sqljoin;
use super::{carried, clamp_est, mk, semi, Ctx, Planner, EXPR_EVAL_NS};
use crate::card::{same_star, selectivity_with, Card};
use crate::expr::{and_all, conjuncts};
use crate::props::{self, slot_list, Props};

/// Groups of up to this many units may be ordered by dynamic programming (spec §7.5) ...
const MAX_DP_UNITS: usize = 12;
/// ... when the search extends at most this many DP states by a next unit ([`dp_work`], with
/// the states a subset keeps bounded by [`Group::extra_states`]), which bounds planning time by
/// work rather than by unit count. With one state per subset the work is the number of join
/// steps: a dense group (every unit shares a slot with every other, e.g. a subject star) of n
/// units takes n·2^(n-1) - n steps, 2 295 for 9 units, 5 110 for 10, 24 564 for 12, while a
/// chain of n takes n(n-1). Interesting orders multiply it by up to 1 + the number of join slots
/// (+1 for an open SQL chain): 6 for a 5-clique, whose search then keeps fewer properties
/// ([`Keep::TIERS`]). Larger searches use the greedy order. One extension costs about 2.0K-2.5K
/// instructions with a forced operator and 3.9K-4.6K when bind, hash and merge joins are
/// compared (`Auto`, `NoSql`), so a search takes at most about 15M (forced) or 28M
/// instructions.
const MAX_DP_WORK: usize = 6_000;
/// `sort_left` threshold (spec §7.5).
pub(super) const SORT_LEFT_ROWS: f64 = 64.0;
/// [`Group::unit_dv_index`] of a slot whose distinct values are not computed yet.
pub(super) const NO_DVS: u16 = u16::MAX;

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

/// The plain access paths of a quad pattern with the slot each is sorted by (`sorted_accesses`).
type SortedAccesses = SmallVec<[(Access, Option<Slot>); 4]>;

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
    /// The interesting properties ([`Key`]) the ordering search keeps prefixes for, besides
    /// each subset's cheapest one: fewer when keeping all would exceed the planning budget.
    keep: Keep,
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
    accesses: Vec<Option<SortedAccesses>>,
    /// Distinct values per slot (min over the patterns binding it; infinite when unknown).
    pub(super) dv: FxHashMap<Slot, f64>,
    /// Per slot (an index into `unit_dvs`, `NO_DVS` until computed): the distinct values of
    /// the slot in each quad pattern unit that binds it (in the group's context), ascending,
    /// for join estimates (`choice::Planner::key_dvs`).
    pub(super) unit_dv_index: [u16; SlotSet::MAX_SLOTS],
    pub(super) unit_dvs: Vec<SmallVec<[(f64, usize); 4]>>,
    /// Join candidates of the step being decided: one buffer for the whole ordering search.
    choices: Vec<Choice>,
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
                        if let UnitRef::Pat(p) = v.r
                            && j != i
                            && same_star(&p, &next)
                        {
                            mates.insert(j);
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
            keep: Keep::TIERS[0],
            star_mates,
            star_memo: StarMemo::default(),
            scan_est: vec![[None; 16]; n],
            range_est: vec![None; n],
            accesses: vec![None; n],
            dv: FxHashMap::default(),
            unit_dv_index: [NO_DVS; SlotSet::MAX_SLOTS],
            unit_dvs: Vec::new(),
            choices: Vec::new(),
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

    /// Upper bounds on the states the ordering search may keep for the subset `mask` of a
    /// group of at most [`MAX_DP_UNITS`] units besides the unordered one (one per [`Key`]):
    /// sorted ones, by a slot that both a unit of the subset and one outside it bind certainly
    /// (merge joins on: a prefix is sorted by a slot its units bind, and kept for it while a
    /// remaining unit binds it too); and an open SQL chain (SQL chains on, at least two units,
    /// all sqlable, and an sqlable unit outside).
    fn extra_states(&self, mask: usize) -> (usize, usize) {
        let rest = ((1usize << self.units.len()) - 1) & !mask;
        let mut sorted = 0;
        if self.merge {
            let certain = |m: usize| mask_iter(m).fold(SlotSet::EMPTY, |acc, j| acc.union(self.units[j].props.certain));
            let join_slots = certain(mask).intersect(certain(rest));
            sorted = join_slots.0.iter().map(|w| w.count_ones() as usize).sum();
        }
        let sqlable = |j: usize| self.units[j].sqlable;
        let chain = self.sql && mask.count_ones() >= 2 && mask_iter(mask).all(sqlable) && mask_iter(rest).any(sqlable);
        (sorted, usize::from(chain))
    }
}

/// Which interesting properties ([`Key`]) the ordering search keeps a prefix for, besides the
/// cheapest prefix of each subset.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(super) struct Keep {
    /// Sorted by a slot a remaining unit binds (a later merge join key).
    sorted: bool,
    /// An open SQL chain a remaining unit may extend.
    sql: bool,
}

impl Keep {
    /// The searches from the richest to the plainest; a group takes the first that fits the
    /// budget. Sorted prefixes are kept before open SQL chains: they feed merge joins and
    /// in-order BindJoin probes, while a chain that is the cheapest prefix is still extended
    /// (a 9-pattern star on SQLite: the full search's plan when keeping sorted prefixes, 1.4x
    /// its estimated cost when keeping chains).
    pub(super) const TIERS: [Keep; 4] = [
        Keep { sorted: true, sql: true },
        Keep { sorted: true, sql: false },
        Keep { sorted: false, sql: true },
        Keep { sorted: false, sql: false },
    ];

    /// The states a subset keeps at most, given its [`Group::extra_states`].
    fn states(self, (sorted, sql): (usize, usize)) -> usize {
        1 + usize::from(self.sorted) * sorted + usize::from(self.sql) * sql
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

/// The work `dp_path` does at most under each of [`Keep::TIERS`]: per reachable subset, its
/// candidate next units times the states it keeps (each is extended by each candidate), from
/// `extra(subset)` ([`Group::extra_states`]); None once even the plainest search exceeds
/// `limit`.
fn dp_work(adj: &[usize], seeded: usize, extra: impl Fn(usize) -> (usize, usize), limit: usize) -> Option<[usize; 4]> {
    let full = (1usize << adj.len()) - 1;
    let mut reached = vec![false; full + 1];
    for i in 0..adj.len() {
        reached[1 << i] = true;
    }
    let mut work = [0; 4];
    for mask in 1..full {
        if !reached[mask] {
            continue;
        }
        let next = dp_candidates(adj, seeded, mask);
        let (n, extra) = (next.count_ones() as usize, extra(mask));
        for (w, keep) in work.iter_mut().zip(Keep::TIERS) {
            *w += n * keep.states(extra);
        }
        if work[3] > limit {
            return None;
        }
        for i in mask_iter(next) {
            reached[mask | 1 << i] = true;
        }
    }
    Some(work)
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
    /// or such scans joined by SQL steps only, with no filter applied yet. At most
    /// `sqljoin::MAX_ALIASES`.
    pub(super) chain: usize,
    /// The cost a COUNT of the prefix does not pay (0 unless it is factorized): when the last
    /// operator is a merge join without checks, the count is the sum of its run products
    /// (`fast_count`), which produces no row and buffers no run ([`Choice::count_free`]).
    pub(super) count_free: f64,
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
            count_free: 0.0,
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
    /// Of `cost`, what a COUNT of the join's rows does not pay: a merge join without checks
    /// counts the products of its runs (`fast_count`) without producing its rows or buffering
    /// its right runs. 0 for every other step.
    pub(super) count_free: f64,
}

impl Choice {
    /// A join step with its estimates kept finite (`clamp_est`).
    pub(super) fn new(algo: Algo, right: Opened, rows: f64, cost: f64, sorted: Option<Slot>) -> Choice {
        Choice { algo, right, rows: clamp_est(rows), cost: clamp_est(cost), sorted, count_free: 0.0 }
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
    /// An open SQL chain of at least two units that a remaining unit may extend (it has fewer
    /// than `sqljoin::MAX_ALIASES` units).
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
/// SQL chain can always be ignored), and dropped when such a state arrives. Only buckets that
/// no back pointer refers to yet are changed.
fn insert(bucket: &mut Bucket, st: DpState) {
    if st.key == Key::Any && bucket.iter().any(|s| s.rank <= st.rank) {
        return;
    }
    if st.key != Key::Any {
        bucket.retain(|s| s.key != Key::Any || s.rank < st.rank);
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
    key_for(g, shape.chain, shape.sorted, rest)
}

/// [`key_of`] a prefix that is an SQL chain of `chain` units (0: none) sorted by `sorted`.
fn key_for(g: &Group<'_>, chain: usize, sorted: Option<Slot>, rest: (SlotSet, bool)) -> Key {
    let (interesting, sql_rest) = rest;
    if g.keep.sql && (2..sqljoin::MAX_ALIASES).contains(&chain) && sql_rest {
        return Key::Sql;
    }
    match sorted {
        Some(v) if g.keep.sorted && g.merge && interesting.contains(v) => Key::Sorted(v),
        _ => Key::Any,
    }
}

/// Can a state of `key` and `rank` not improve `bucket`? A state of the same key, or of any
/// key for an unordered one, that is at most as expensive already dominates it.
fn dominated(bucket: &Bucket, key: Key, rank: f64) -> bool {
    bucket.iter().any(|s| (key == Key::Any || s.key == key) && s.rank <= rank)
}

/// The length of the open SQL chain after join step `ch` on a prefix with an open chain of
/// `left_chain` units: SQL steps extend it, every other operator closes it.
fn step_chain(left_chain: usize, ch: &Choice) -> usize {
    if matches!(ch.algo, Algo::Sql { .. }) { left_chain + 1 } else { 0 }
}

/// The cheapest state of a complete bucket.
fn cheapest(bucket: &Bucket) -> Result<usize> {
    (0..bucket.len())
        .min_by(|&a, &b| bucket[a].rank.total_cmp(&bucket[b].rank))
        .ok_or_else(|| Error::internal("join DP left a subset unreached"))
}

/// The join candidates a state of a finished bucket is extended with: every one for the
/// bucket's cheapest state; for a state kept only for its [`Key`], the candidates that keep it
/// (any other candidate costs at least as much as from the cheapest state, and its output
/// competes for the same keys); none for an unordered state that is not the cheapest.
fn extension(state: &DpState, is_cheapest: bool) -> Option<Wanted> {
    match state.key {
        _ if is_cheapest => Some(Wanted::All),
        Key::Sorted(v) => Some(Wanted::KeepOrder(v)),
        Key::Sql => Some(Wanted::ExtendSql),
        Key::Any => None,
    }
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
        chain: step_chain(left.chain, ch),
        count_free: ch.count_free,
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
                count_free: 0.0,
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
            // Adjacency masks exist for DP-sized groups only. The richest search that fits the
            // budget, else greedy.
            let mut dp = None;
            if g.units.len() <= MAX_DP_UNITS {
                let (adj, seeded) = g.adjacency();
                let work = dp_work(&adj, seeded, |m| g.extra_states(m), MAX_DP_WORK);
                if let Some(tier) = work.and_then(|w| w.iter().position(|&w| w <= MAX_DP_WORK)) {
                    g.keep = Keep::TIERS[tier];
                    dp = Some((adj, seeded));
                }
            }
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
    ) -> SortedAccesses {
        if let Some(a) = &g.accesses[i] {
            return a.clone();
        }
        let a = sorted_accesses(p, g.ctx.bound(), self.dataset);
        g.accesses[i] = Some(a.clone());
        a
    }

    /// The plain access path of quad pattern unit `i` (in the group's context) whose output is
    /// sorted by `key`, if any.
    pub(super) fn sorted_access(&self, g: &mut Group<'_>, i: usize, p: &QuadPat, key: Slot) -> Option<Access> {
        if g.accesses[i].is_none() {
            g.accesses[i] = Some(sorted_accesses(p, g.ctx.bound(), self.dataset));
        }
        g.accesses[i].as_ref()?.iter().find(|(_, s)| *s == Some(key)).map(|(a, _)| *a)
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
            count_free: 0.0,
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
    /// produces no row and buffers no run (`Shape::count_free`).
    fn final_cost(&self, g: &Group<'_>, shape: &Shape) -> f64 {
        self.final_cost_of(g, shape.cost, shape.count_free)
    }

    /// [`Self::final_cost`] of a complete group with these estimates.
    fn final_cost_of(&self, g: &Group<'_>, cost: f64, count_free: f64) -> f64 {
        match g.goal {
            Goal::Count => (cost - count_free).max(0.0),
            _ => cost,
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
        // and back pointers only point into finished buckets. Unreachable subsets stay empty.
        for mask in 1..full {
            if best[mask].is_empty() {
                continue;
            }
            let bucket = std::mem::take(&mut best[mask]);
            // The planning budget relies on this bound (`dp_work`).
            debug_assert!(bucket.len() <= g.keep.states(g.extra_states(mask)), "subset {mask:#b} over its state bound");
            let top = cheapest(&bucket)?;
            for i in mask_iter(dp_candidates(adj, seeded, mask)) {
                let next = mask | 1 << i;
                let rest = g.interest(mask_iter(full & !next));
                self.extend_bucket(g, (&bucket, mask, top), i, &mut best[next], rest, next == full)?;
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
            let top = cheapest(&prev)?;
            let rest = g.interest(k + 1..n);
            self.extend_bucket(g, (&prev, k - 1, top), k, &mut buckets[k], rest, k == n - 1)?;
            buckets[k - 1] = prev;
        }
        let idx = cheapest(&buckets[n - 1])?;
        Ok(reconstruct(&buckets, n - 1, idx))
    }

    /// One DP transition: join unit `i` to the states of the finished bucket `from` = (bucket,
    /// its index, its cheapest state), each with the candidates [`extension`] allows, into
    /// `into`. `rest` is what the units still to join afterwards make interesting; `complete`:
    /// the transition completes the group.
    fn extend_bucket(
        &mut self,
        g: &mut Group<'_>,
        from: (&Bucket, usize, usize),
        i: usize,
        into: &mut Bucket,
        rest: (SlotSet, bool),
        complete: bool,
    ) -> Result<()> {
        let (bucket, prev, top) = from;
        let mut chs = std::mem::take(&mut g.choices);
        for (idx, state) in bucket.iter().enumerate() {
            let Some(wanted) = extension(state, idx == top) else { continue };
            chs.clear();
            self.join_choices(g, &state.shape, i, None, Mode::Probe, wanted, &mut chs)?;
            for ch in &chs {
                // A step that cannot improve `into` before its filters cannot after them either:
                // filters only add cost, keep the order and may close an SQL chain (whose rows
                // are unordered, so the state then competes as `Key::Any`).
                let key = key_for(g, step_chain(state.shape.chain, ch), ch.sorted, rest);
                let rank = if complete { self.final_cost_of(g, ch.cost, ch.count_free) } else { ch.cost };
                if dominated(into, key, rank) {
                    continue;
                }
                let shape = joined_shape(&state.shape, &g.units[i].props, Some(i), ch, &JoinKind::Inner);
                let from = From::Step { prev, idx, unit: i, algo: ch.algo };
                let st = self.settle(g, &shape, state.applied.clone(), from, rest, complete)?;
                insert(into, st);
            }
        }
        g.choices = chs;
        Ok(())
    }

    /// Greedy order: first the unit with the fewest rows once the conjuncts it completes are
    /// applied, read through its cheapest access path (an IntRange scan, a sorted order or the
    /// default one, see [`Self::start_accesses`]); then the cheapest connected next unit and
    /// operator.
    fn greedy_path(&mut self, g: &mut Group<'_>) -> Result<Path> {
        // (unit, access, settled shape, conjuncts it applied)
        let mut start: Option<(usize, Option<Access>, Shape, Bits)> = None;
        for i in 0..g.units.len() {
            // The access paths of one unit return the same rows: the cheapest one represents it.
            let mut best: Option<(Option<Access>, Shape, Bits)> = None;
            for access in self.start_accesses(g, i) {
                let (_, shape) = self.start_shape(g, i, Mode::Probe, access)?;
                let mut applied = Bits::default();
                let (_, shape) = self.filter_steps(g, &shape, &mut applied, false, Mode::Probe)?;
                if best.as_ref().is_none_or(|(_, b, _)| shape.cost < b.cost) {
                    best = Some((access, shape, applied));
                }
            }
            let Some((access, shape, applied)) = best else { continue };
            let fewer = |s: &(usize, Option<Access>, Shape, Bits)| {
                shape.rows.total_cmp(&s.2.rows).then(shape.cost.total_cmp(&s.2.cost)) == Ordering::Less
            };
            if start.as_ref().is_none_or(fewer) {
                start = Some((i, access, shape, applied));
            }
        }
        let (first, access, mut shape, mut applied) =
            start.ok_or_else(|| Error::internal("greedy join order of an empty group"))?;
        let mut path = Path { start: (first, access), steps: Vec::new() };
        let mut remaining: Vec<usize> = (0..g.units.len()).filter(|&i| i != first).collect();
        let mut chs = std::mem::take(&mut g.choices);
        while !remaining.is_empty() {
            let mut best: Option<(usize, Choice)> = None;
            for i in Self::next_candidates(g, &shape, remaining.iter().copied()) {
                chs.clear();
                self.join_choices(g, &shape, i, None, Mode::Probe, Wanted::All, &mut chs)?;
                for ch in chs.drain(..) {
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
        g.choices = chs;
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
        Ok(self.star_merges(side.plan, g.goal, g.ctx.bound()))
    }

    /// The real-plan choice for a recorded step: the same decision when it is still legal,
    /// else the cheapest legal one of the same family, else the cheapest legal one.
    fn replay_choice(&mut self, g: &mut Group<'_>, left: &Shape, i: usize, algo: &Algo) -> Result<Choice> {
        let family = algo.family();
        let mut chs = Vec::new();
        self.join_choices(g, left, i, None, Mode::Build, Wanted::Family(family), &mut chs)?;
        if chs.is_empty() {
            self.join_choices(g, left, i, None, Mode::Build, Wanted::All, &mut chs)?;
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
        shape.count_free = 0.0;
        if !plain.is_empty() {
            let n = plain.len() as f64;
            let (rows, cost) = (clamp_est(shape.rows * sel), clamp_est(shape.cost + shape.rows * n * EXPR_EVAL_NS));
            if mode == Mode::Build
                && let Some(expr) = and_all(plain.iter().map(|&i| g.conj[i].expr.clone()).collect())
            {
                steps.push(Step::Filter { expr, rows, cost });
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
        let mut chs = Vec::new();
        self.join_choices(&mut g, &shape, 0, Some(sel), Mode::Build, Wanted::All, &mut chs)?;
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

    fn steps(adj: &[usize], limit: usize) -> Option<usize> {
        dp_work(adj, 0, |_| (0, 0), limit).map(|w| w[3])
    }

    #[test]
    fn dp_step_counts() {
        // Dense groups of n units take n·2^(n-1) - n steps.
        for n in [3usize, 9, 10] {
            let all = (1usize << n) - 1;
            let adj: Vec<usize> = (0..n).map(|i| all & !(1 << i)).collect();
            assert_eq!(steps(&adj, usize::MAX), Some((n << (n - 1)) - n), "dense {n}");
        }
        // A chain of n units: every connected subset is an interval with at most two
        // neighbours; n(n-1) steps.
        let n = 12;
        let adj: Vec<usize> = (0..n).map(|i| ((1 << i) >> 1 | (1 << i) << 1) & ((1 << n) - 1)).collect();
        assert_eq!(steps(&adj, usize::MAX), Some(n * (n - 1)));
        // Over the limit: None. A dense 12-unit group is not searched by DP.
        let all = (1usize << 12) - 1;
        let dense: Vec<usize> = (0..12).map(|i| all & !(1 << i)).collect();
        assert_eq!(steps(&dense, MAX_DP_WORK), None);
        // Disconnected units: every subset is reachable through cross products.
        assert_eq!(steps(&[0, 0, 0], usize::MAX), Some(9));
        // Every state a subset keeps is extended by every candidate next unit: per tier, with up
        // to one sorted state and one open SQL chain per subset.
        let all = (1usize << 10) - 1;
        let dense: Vec<usize> = (0..10).map(|i| all & !(1 << i)).collect();
        assert_eq!(dp_work(&dense, 0, |_| (1, 1), usize::MAX), Some([3 * 5_110, 2 * 5_110, 2 * 5_110, 5_110]));
        // A dense group of 10 units fits the budget with one state per subset only.
        let w = dp_work(&dense, 0, |_| (1, 1), MAX_DP_WORK).unwrap();
        assert_eq!(w.iter().position(|&w| w <= MAX_DP_WORK), Some(3));
        // None once even one state per subset is over the limit.
        assert_eq!(dp_work(&dense, 0, |_| (1, 1), 5_000), None);
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
            count_free: 0.0,
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
        // An unordered prefix is dominated by any state that is at most as expensive, and
        // dropped when one arrives.
        insert(&mut b, state(Key::Any, 11.5));
        assert_eq!(b.iter().find(|s| s.key == Key::Any).unwrap().rank, 10.0);
        insert(&mut b, state(Key::Sql, 9.0));
        assert!(b.iter().all(|s| s.key != Key::Any));
        insert(&mut b, state(Key::Any, 9.5));
        assert!(b.iter().all(|s| s.key != Key::Any));
        assert_eq!(b[cheapest(&b).unwrap()].key, Key::Sql);
        insert(&mut b, state(Key::Any, 8.0));
        assert_eq!(b.len(), 3, "a cheaper unordered prefix joins the ordered ones");
        assert_eq!(b[cheapest(&b).unwrap()].key, Key::Any);
    }

    #[test]
    fn only_the_cheapest_state_takes_every_operator() {
        let mut b = Bucket::new();
        insert(&mut b, state(Key::Sorted(3), 12.0));
        insert(&mut b, state(Key::Sql, 11.0));
        insert(&mut b, state(Key::Any, 10.0));
        let top = cheapest(&b).unwrap();
        let wanted: Vec<Option<Wanted>> = b.iter().enumerate().map(|(i, s)| extension(s, i == top)).collect();
        assert_eq!(wanted, vec![Some(Wanted::KeepOrder(3)), Some(Wanted::ExtendSql), Some(Wanted::All)]);
        // A sorted prefix that is the cheapest is extended with every operator.
        assert_eq!(extension(&state(Key::Sorted(3), 1.0), true), Some(Wanted::All));
        assert_eq!(extension(&state(Key::Any, 1.0), false), None);
    }

    /// A group of quad patterns `(s, p, o)` (slots, predicate number) over the default graph,
    /// with merge joins on, the way `plan_group` sets it up under `Auto` on a MemStore.
    fn group_of<'c>(pats: &[(Slot, i64, Slot)], card: &Card<'c>) -> Group<'static> {
        use sq_core::id::Tag;
        use sq_core::ir::GraphSel;
        use sq_core::TermId;
        let units = pats
            .iter()
            .map(|&(s, pred, o)| {
                let q = QuadPat {
                    g: GraphSel::Default,
                    s: PatTerm::Slot(s),
                    p: PatTerm::Const(TermId::from_dict(2000 + pred, Tag::Iri)),
                    o: PatTerm::Slot(o),
                };
                Unit { r: UnitRef::Pat(q), props: props::pattern(&q), sqlable: true, range: None }
            })
            .collect();
        let mut g = Group::new(units, Ctx::default(), Goal::Rows, card);
        g.merge = true;
        g
    }

    /// The tier of [`Keep::TIERS`] `plan_group` searches `g` with (None: greedy).
    fn tier(g: &Group<'_>) -> Option<usize> {
        let (adj, seeded) = g.adjacency();
        let work = dp_work(&adj, seeded, |m| g.extra_states(m), MAX_DP_WORK)?;
        work.iter().position(|&w| w <= MAX_DP_WORK)
    }

    #[test]
    fn dense_groups_keep_fewer_properties() {
        let store = sq_memstore::MemStore::new();
        let ds = sq_core::ir::DatasetSpec {
            default: sq_core::ir::DefaultGraph::StoreDefault,
            named: sq_core::ir::NamedGraphs::All,
        };
        let opts = sq_core::plan::QueryOptions::default();
        let card = Card::new(&store, &ds, 0, &opts);
        // Every edge of a k-clique of slots is a :knows pattern.
        let clique = |k: Slot| -> Vec<(Slot, i64, Slot)> {
            (0..k).flat_map(|a| (a + 1..k).map(move |b| (a, 1, b))).collect()
        };
        assert_eq!(tier(&group_of(&clique(4), &card)), Some(0), "a 4-clique keeps sorted prefixes");
        let g = group_of(&clique(5), &card);
        let (adj, seeded) = g.adjacency();
        let work = dp_work(&adj, seeded, |m| g.extra_states(m), usize::MAX).unwrap();
        assert!(work[0] > 4 * work[3], "interesting orders multiply the work: {work:?}");
        // The sorted search would exceed the budget; without SQL chains the next tiers are the
        // plain search.
        assert!(work[1] > MAX_DP_WORK && work[3] <= MAX_DP_WORK, "{work:?}");
        assert_eq!(tier(&g), Some(2));
        // A subject star of 9 patterns keeps its sorted prefixes, but not its open SQL chains
        // as well.
        let star: Vec<(Slot, i64, Slot)> = (1..=9).map(|o| (0, o as i64, o)).collect();
        let mut g = group_of(&star, &card);
        assert_eq!(tier(&g), Some(0));
        g.sql = true;
        assert_eq!(tier(&g), Some(1));
        // 12 dense units: greedy.
        let star: Vec<(Slot, i64, Slot)> = (1..=12).map(|o| (0, o as i64, o)).collect();
        assert_eq!(tier(&group_of(&star, &card)), None);
    }

    #[test]
    fn kept_states_follow_join_slots_and_sql_chains() {
        // ?x :p ?a . ?x :q ?b . ?b :r ?c over the store default graph: join slots ?x and ?b.
        use sq_core::id::Tag;
        use sq_core::ir::GraphSel;
        use sq_core::TermId;
        let p = |s: Slot, pred: i64, o: Slot| QuadPat {
            g: GraphSel::Default,
            s: PatTerm::Slot(s),
            p: PatTerm::Const(TermId::from_dict(2000 + pred, Tag::Iri)),
            o: PatTerm::Slot(o),
        };
        let unit = |q: QuadPat| Unit { r: UnitRef::Pat(q), props: props::pattern(&q), sqlable: true, range: None };
        let units = vec![unit(p(0, 1, 1)), unit(p(0, 2, 2)), unit(p(2, 3, 3))];
        let store = sq_memstore::MemStore::new();
        let ds = sq_core::ir::DatasetSpec {
            default: sq_core::ir::DefaultGraph::StoreDefault,
            named: sq_core::ir::NamedGraphs::All,
        };
        let opts = sq_core::plan::QueryOptions::default();
        let card = Card::new(&store, &ds, 0, &opts);
        let mut g = Group::new(units, Ctx::default(), Goal::Rows, &card);
        assert_eq!(g.extra_states(0b001), (0, 0), "no merge joins, no SQL chains");
        g.merge = true;
        assert_eq!(g.extra_states(0b001), (1, 0), "{{0}} binds ?x, which unit 1 binds too");
        assert_eq!(g.extra_states(0b010), (2, 0), "{{1}} binds ?x and ?b, which units 0 and 2 bind");
        assert_eq!(g.extra_states(0b011), (1, 0), "{{0, 1}}: only ?b is left to join on");
        g.sql = true;
        assert_eq!(g.extra_states(0b011), (1, 1), "an open SQL chain that unit 2 may extend");
        assert_eq!(g.extra_states(0b001), (1, 0), "one unit is no SQL chain state yet");
        assert_eq!(g.extra_states(0b111), (0, 0), "nothing left to join");
        // Keys follow the kept properties.
        let rest = (SlotSet::from_slots([2]), true);
        assert_eq!(key_for(&g, 2, Some(2), rest), Key::Sql);
        assert_eq!(key_for(&g, 0, Some(2), rest), Key::Sorted(2));
        let (sorted_only, sql_only) = (Keep { sorted: true, sql: false }, Keep { sorted: false, sql: true });
        g.keep = sorted_only;
        assert_eq!(key_for(&g, 2, Some(2), rest), Key::Sorted(2), "open chains not kept");
        g.keep = sql_only;
        assert_eq!(key_for(&g, 0, Some(2), rest), Key::Any, "sorted prefixes not kept");
        g.keep = Keep::TIERS[3];
        assert_eq!(key_for(&g, 2, Some(2), rest), Key::Any, "nothing but the cheapest prefix kept");
        assert_eq!((sorted_only.states((3, 1)), sql_only.states((3, 1)), Keep::TIERS[0].states((3, 1))), (4, 2, 5));
        // From the richest search to the plainest.
        let (all, none) = (Keep { sorted: true, sql: true }, Keep { sorted: false, sql: false });
        assert_eq!(Keep::TIERS, [all, sorted_only, sql_only, none]);
    }
}

