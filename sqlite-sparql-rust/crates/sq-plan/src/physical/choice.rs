//! Join operator candidates for one join step (spec §7.5) and their costs.
//!
//! For a joined prefix (left [`Shape`]) and a next unit, every legal operator that `JoinPolicy`
//! allows becomes a [`Choice`] with its estimated rows, cost and output order:
//! * **BindJoin** (index nested loop): `probes × cost(R | seeded) + out × row_copy`, where a
//!   left input sorted by a seed slot, or sorted per batch (`sort_left`), probes in key order
//!   and only once per distinct key. A probe in key order costs `seek_sorted` when the keys
//!   are dense in the unit's index range and up to `seek_random` when they are more than
//!   `LOCAL_SEEK_ROWS` rows apart (`ordered_seek_cost`). Keeps the left order unless
//!   `sort_left`.
//! * **SQL step**: the same probe, run by SQLite's VDBE inside one SqlJoin statement
//!   (`vdbe_probe` per left row, `vdbe_row` per produced column); only while the prefix is an
//!   open SQL chain of fewer than `sqljoin::MAX_ALIASES` patterns. Unordered.
//! * **HashJoin** (build the smaller side, `_small` / `_large` constants around 32K entries).
//!   Keeps the probe side's order.
//! * **MergeJoin** on each certainly-bound shared slot: both sides sorted by it, either already
//!   (a quad pattern read through an index order whose first free column is the key) or through
//!   a SortBySlot enforcer (`n·log2(n)·sort_row_log`); a right side read through one cursor
//!   gallops (`min(|R| scanned, distinct(L) × (ordered seek + rows read before it) + matched
//!   rows)`, where each seek follows up to `gallop` rows read past the previous key and is a
//!   random seek when the keys are sparse), merged multi-graph streams cannot seek and are
//!   scanned. Every left row meets the buffered right run of its key, so a merge whose other
//!   shared slots are checked visits `pairs = Σ |L_run| × |R_run| ≈ |L| × rows(R | key)` pairs
//!   before its check keeps the output rows ([`Planner::pair_cost`] each); the matched right
//!   rows are copied into the run buffer once (neither the pairs nor the buffer when a COUNT
//!   is factorized over a check-free merge: [`Choice::count_free`]). A low-cardinality key
//!   with checks therefore costs its run products, and a merge on the most selective key, or a
//!   HashJoin on all the keys, wins. Sorted by the key.
//! * **NestedLoop** when nothing else is legal. Keeps the left order.
//!
//! Memory: a SortBySlot enforcer or a hash table estimated larger than the memory budget
//! (`max_memory_mb`) would stop the query, and costs [`OVER_BUDGET_ROW_NS`] more per buffered
//! row ([`Planner::over_budget`]), which only a far slower plan outweighs.
//!
//! Rows (spec §7.4 joins): every operator of a step gets the same estimate. For a quad pattern
//! unit it is its rows in the group's context times the combined selectivity of the join keys
//! ([`Planner::pattern_join`]): per key `1 / max(dv_L, dv_R)` (distinct values in the
//! prefix and in the unit), combined by [`combine_keys`]: the most selective key in full,
//! further entity keys multiplied (independence), further attribute keys (values the unit
//! repeats on [`ATTRIBUTE_ROWS`] rows or more) backed off exponentially, as attributes shared
//! along a join are often correlated with its other keys; a unit that extends a subject star
//! of the prefix uses the characteristic-set ratio for its subject.
//! Opaque units are estimated with the keys bound (`estimate`).
//!
//! Policies: `PreferHash` and `PreferBind` force their operator wherever legal (WP5);
//! `PreferMerge` takes only (binary) merge joins wherever a key exists (enforcers on both sides
//! allowed); `Auto` and `NoSql` compare everything (at most one enforcer per merge) and may turn
//! merge chains into StarMerges (`star`); SQL steps only under `Auto` with a store that runs SQL
//! (`Group::sql`).
//!
//! Callers may ask for a subset of the candidates ([`Wanted`]): one family when a recorded
//! decision is replayed, or only those that keep the left input's order or open SQL chain when
//! the ordering search extends a prefix that it keeps for that property alone.
use smallvec::SmallVec;
use sq_core::batch::{Slot, SlotSet};
use sq_core::ir::{Logical, PatTerm, QuadPat};
use sq_core::plan::JoinPolicy;
use sq_core::{Result, BATCH_ROWS};

use super::join::{Algo, Choice, Family, Group, Mode, Opened, Shape, UnitRef, NO_DVS, SORT_LEFT_ROWS};
use super::scan::{bound_columns, range_access, single_cursor};
use super::sqljoin::MAX_ALIASES;
use super::{clamp_est, Ctx, Planner};
use crate::props::slots;

/// Cost of sorting a left batch by its seed tuple for `sort_left`, per row (about 20 µs per
/// 1024 rows, spec §5.3).
const SORT_LEFT_NS: f64 = 20.0;
/// Hash tables up to this many entries use the `_small` constants (spec §7.5).
const SMALL_HASH: f64 = 32_768.0;
/// Bytes a buffered row takes besides its slots (8 bytes each): a SortBySlot charges 20 per
/// row to the memory budget, a hash table entry 24 (`sq-exec`).
const BUFFER_ROW_BYTES: f64 = 24.0;
/// Cost per row of a buffer (SortBySlot enforcer, hash table) estimated larger than the
/// query's memory budget (`QueryOptions::max_memory_mb`): the executor would stop the plan
/// with "memory budget exceeded", so it is taken only when every alternative buffers as much,
/// or costs more than this per buffered row (about 20,000 row copies; 1,000 s for 1e7 rows).
/// It is a cost rather than a ban because the row estimates may be too high.
const OVER_BUDGET_ROW_NS: f64 = 100_000.0;
/// Index rows within which a forward seek to the next key stays local (about a leaf page of
/// index entries). Seeks in key order that are further apart than this cost a random seek:
/// 9 keys spread over a 600K-row range are 9 descents, not 9 cheap forward steps.
const LOCAL_SEEK_ROWS: f64 = 256.0;

/// Which join candidates a caller asks for.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(super) enum Wanted {
    /// Every legal candidate.
    All,
    /// The candidates of one operator family (replaying a recorded decision).
    Family(Family),
    /// Only candidates whose output keeps the left rows' order by this slot: BindJoin without
    /// re-sorting, HashJoin probing with the left rows, MergeJoin on the slot, NestedLoop.
    KeepOrder(Slot),
    /// Only the SQL step, which extends the left's open SQL chain.
    ExtendSql,
}

impl Wanted {
    fn family(self, f: Family) -> bool {
        match self {
            Wanted::All => true,
            Wanted::Family(only) => f == only,
            Wanted::KeepOrder(_) => f != Family::Sql,
            Wanted::ExtendSql => f == Family::Sql,
        }
    }

    /// Is a candidate whose output is sorted by `sorted` wanted (given its family is)?
    fn order(self, sorted: Option<Slot>) -> bool {
        match self {
            Wanted::KeepOrder(v) => sorted == Some(v),
            _ => true,
        }
    }
}

/// Output order of a BindJoin whose left input is sorted by `left`: the left order, unless
/// batches are re-sorted by a seed tuple that does not start with that slot.
pub(super) fn bind_sorted(left: Option<Slot>, seed: SlotSet, sort_left: bool) -> Option<Slot> {
    if !sort_left || slots(seed).next() == left { left } else { None }
}

/// The number of slots in `s`.
fn slot_count(s: SlotSet) -> usize {
    s.0.iter().map(|w| w.count_ones() as usize).sum()
}

/// Rows per value from which a join key is an attribute ([`combine_keys`]): a value the unit
/// repeats on this many rows or more (a city, an age, a type, a vendor's country) rather than
/// one naming a few rows (a person, a product, the end of a `:knows` edge, a year-day of
/// 365).
pub(super) const ATTRIBUTE_ROWS: f64 = 100.0;

/// One join key of a step for [`combine_keys`]: its selectivity in [0, 1], and whether it is an
/// attribute key (its values repeat on at least [`ATTRIBUTE_ROWS`] rows of the unit).
#[derive(Copy, Clone, Debug, PartialEq)]
pub(super) struct KeySel {
    pub(super) sel: f64,
    pub(super) attribute: bool,
}

/// The combined selectivity of several join keys. The most selective key counts in full; of the
/// others, entity keys multiply (independent keys, spec §7.4.5) and attribute keys back off
/// exponentially: the first square-rooted, the next fourth-rooted, and so on.
///
/// Attributes shared along a join are often correlated with the join's other keys (friends
/// of friends tend to live in one city; products sharing a feature tend to share a type), and
/// the product then underestimates the joined rows by up to the attribute's distinct values
/// (the tutorial's 100K-person network: 4.5e4 against 292,595 rows; the 1.6M-triple social
/// graph's 1,000 cities: 1.8e3 against 52,949). Backing off one attribute key splits that range
/// geometrically, and costs at most `sqrt(dv)` when the keys are independent, which an
/// attribute's few values keep small. An entity key closing a cycle (a triangle's last edge,
/// a mutual `:knows`) has as many values as the graph has nodes: backing it off would
/// overestimate a random graph's cycles by `sqrt(nodes)` (378 against 26 triangles on 200
/// people, ~500x on 200K), so it multiplies. Sorts `keys`.
pub(super) fn combine_keys(keys: &mut [KeySel]) -> f64 {
    keys.sort_by(|a, b| a.sel.total_cmp(&b.sel));
    let mut combined = 1.0;
    let mut backed_off = 0;
    for (k, key) in keys.iter().enumerate() {
        let s = key.sel.clamp(0.0, 1.0);
        combined *= if k > 0 && key.attribute {
            backed_off += 1;
            s.powf(0.5f64.powi(backed_off))
        } else {
            s
        };
    }
    combined
}

/// How a quad pattern unit joins a prefix (`Planner::pattern_join`): its rows and each join
/// key (a quad pattern has at most four slots).
pub(super) struct PatJoin {
    rows: f64,
    keys: usize,
    sels: [(Slot, KeySel); 4],
}

/// An unused [`PatJoin`] key.
const NO_KEY: (Slot, KeySel) = (0, KeySel { sel: 1.0, attribute: false });

impl PatJoin {
    /// Rows of the unit per left row, joined on every key ([`combine_keys`]).
    pub(super) fn per_left(&self) -> f64 {
        let sel = match &self.sels[..self.keys] {
            [] => 1.0,
            [(_, k)] => k.sel,
            many => {
                let mut keys = [NO_KEY.1; 4];
                for (to, (_, k)) in keys.iter_mut().zip(many) {
                    *to = *k;
                }
                combine_keys(&mut keys[..many.len()])
            }
        };
        clamp_est(self.rows * sel)
    }

    /// Rows of the unit per left row, joined on `key` alone (a merge join on `key` checks
    /// the other keys afterwards).
    pub(super) fn per_left_on(&self, key: Slot) -> f64 {
        let sel = self.sels[..self.keys].iter().find(|&&(v, _)| v == key).map_or(1.0, |&(_, k)| k.sel);
        clamp_est(self.rows * sel)
    }
}

impl Planner<'_> {
    /// Hash build and probe cost per row for a table of `build_rows` entries.
    pub(super) fn hash_costs(&self, build_rows: f64) -> (f64, f64) {
        if build_rows < SMALL_HASH {
            (self.costs.hash_build_small, self.costs.hash_probe_small)
        } else {
            (self.costs.hash_build_large, self.costs.hash_probe_large)
        }
    }

    /// Cost of each of `keys` seeks made in key order into a sorted input of `rows` rows:
    /// `seek_sorted` when consecutive keys are close, up to `seek_random` when they are more
    /// than [`LOCAL_SEEK_ROWS`] rows apart on average.
    pub(super) fn ordered_seek_cost(&self, keys: f64, rows: f64) -> f64 {
        let gap = rows / keys.max(1.0);
        let far = (gap / LOCAL_SEEK_ROWS).clamp(0.0, 1.0);
        self.costs.seek_sorted + (self.costs.seek_random - self.costs.seek_sorted).max(0.0) * far
    }

    /// Cost of a SortBySlot enforcer over `rows` rows (buffer and sort, and
    /// [`Self::over_budget`]).
    pub(super) fn enforce_cost(&self, rows: f64) -> f64 {
        let n = rows.max(1.0);
        n * n.log2().max(1.0) * self.costs.sort_row_log + n * self.row_copy() + self.over_budget(rows)
    }

    /// The penalty of buffering `rows` rows of the plan's width (a SortBySlot enforcer, a hash
    /// table): 0 while their estimated bytes fit the memory budget, else
    /// [`OVER_BUDGET_ROW_NS`] per row, so that a plan that would exceed the budget loses to
    /// any that fits unless that one is far slower.
    pub(super) fn over_budget(&self, rows: f64) -> f64 {
        let bytes = rows * (self.width * 8.0 + BUFFER_ROW_BYTES);
        let budget = self.opts.max_memory_mb as f64 * (1u64 << 20) as f64;
        if bytes > budget { rows * OVER_BUDGET_ROW_NS } else { 0.0 }
    }

    /// Append to `out` every candidate operator joining the prefix `left` with unit `i` (see
    /// the module docs) that `wanted` asks for. `left_join` = Some(filter selectivity) for an
    /// OPTIONAL (kind Left), None for an inner join.
    ///
    /// Inside a group opened with seed S (`ctx.seeded`): a BindJoin re-opens the unit with its
    /// seed list only, so every slot both sides may bind, S included, must be carried by the
    /// left rows; HashJoin / MergeJoin / NestedLoop open the unit with S itself when the join
    /// is inner, and slots of S are then neither keys nor checked (see `physical`).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn join_choices(
        &mut self,
        g: &mut Group<'_>,
        left: &Shape,
        i: usize,
        left_join: Option<f64>,
        mode: Mode,
        wanted: Wanted,
        out: &mut Vec<Choice>,
    ) -> Result<()> {
        let (unit, ctx) = (g.units[i], g.ctx);
        let pr = &unit.props;
        let inner = left_join.is_none();
        let both_seeded = if inner { ctx.seeded } else { SlotSet::EMPTY };
        let shared = left.maybe.intersect(pr.maybe);
        let seed = left.certain.intersect(pr.maybe);
        let keys = left.certain.intersect(pr.certain).minus(both_seeded);
        let check = shared.minus(keys).minus(both_seeded);
        // A unit that mints values (BNODE, RAND, UUID, STRUUID) is evaluated once: probing it
        // per seed tuple would give one of its solutions several values (sq-exec hashes such
        // a BindJoin anyway; planning it as hash keeps costs and EXPLAIN truthful).
        let mints = match unit.r {
            UnitRef::Node(n) => self.mints(n),
            UnitRef::Pat(_) => false,
        };
        let bind_ok = !seed.is_empty() && shared.is_subset(&left.certain) && pr.accepts_seed(seed) && !mints;
        let hash_ok = !keys.is_empty();
        let merge_ok = inner && g.merge && hash_ok;
        let policy = self.opts.join_policy;
        let prefer_merge = policy == JoinPolicy::PreferMerge && merge_ok;
        let try_bind = bind_ok && policy != JoinPolicy::PreferHash && !prefer_merge;
        // An SQL step adds one alias to the prefix's open chain (SQLite's join limit caps it).
        let try_sql = try_bind && inner && g.sql && (1..MAX_ALIASES).contains(&left.chain) && unit.sqlable;
        let try_hash = hash_ok && !(policy == JoinPolicy::PreferBind && bind_ok) && !prefer_merge;
        let wants = |f: Family| wanted.family(f);
        // Inputs other than a BindJoin's right side receive the outer seed only for inner joins.
        let rctx = if inner { ctx } else { ctx.unseeded() };
        let lrows = left.rows;
        // Rows per left row when the unit extends a subject star (characteristic sets).
        let star = if inner { self.star_ratio(g, &left.units, i) } else { None };
        // How a quad pattern unit joins: one estimate for every operator.
        let pat_join = match unit.r {
            UnitRef::Pat(p) => Some(self.pattern_join(g, left, i, &p, rctx.bound(), keys, star)?),
            UnitRef::Node(_) => None,
        };
        let per_left = pat_join.as_ref().map(PatJoin::per_left);
        let finish_rows = |rows: f64| match left_join {
            Some(sel) => (rows * sel).max(lrows),
            None => rows,
        };

        if (try_bind && wants(Family::Bind)) || (try_sql && wants(Family::Sql)) {
            let probe_ctx = Ctx { seeded: seed, ..ctx };
            let right = self.open_unit(g, i, probe_ctx, mode, None)?;
            let rows = finish_rows(lrows * per_left.unwrap_or(right.rows));
            if try_sql && wants(Family::Sql) {
                let UnitRef::Pat(p) = unit.r else { unreachable!("sqlable units are quad patterns") };
                let free = bound_columns(&p, probe_ctx.bound()).iter().filter(|b| !**b).count() as f64;
                let per_row = free * self.costs.vdbe_row + self.row_copy();
                let cost = left.cost + lrows * self.costs.vdbe_probe + rows * per_row;
                out.push(Choice::new(Algo::Sql { seed }, right.clone(), rows, cost, None));
            }
            if try_bind && wants(Family::Bind) {
                self.bind_choices(g, left, i, seed, right, rows, wanted, out)?;
            }
        }

        let try_merge = merge_ok && wants(Family::Merge);
        if (try_hash && wants(Family::Hash)) || try_merge {
            let base = self.open_unit(g, i, rctx, mode, None)?;
            let per_left = match (per_left, unit.r) {
                (Some(r), _) => r,
                (None, UnitRef::Node(n)) => self.node_rows_given(g, i, n, keys, lrows, base.rows)?,
                (None, UnitRef::Pat(_)) => unreachable!("quad pattern units are estimated above"),
            };
            let joined = lrows * per_left;
            let rows = finish_rows(joined);
            if try_hash && wants(Family::Hash) {
                // An IntRange scan of the unit shrinks the build side and the joined rows (its
                // range conjuncts then count as applied).
                let ranged = match unit.r {
                    UnitRef::Pat(p) => match range_access(&p, rctx.bound(), unit.range, self.dataset) {
                        Some(a) => Some(self.open_unit(g, i, rctx, mode, Some(a))?),
                        None => None,
                    },
                    UnitRef::Node(_) => None,
                };
                let (right, rows) = match ranged {
                    Some(r) if r.cost < base.cost => {
                        let kept = (r.rows / base.rows.max(1e-9)).min(1.0);
                        (r, finish_rows(joined * kept))
                    }
                    _ => (base.clone(), rows),
                };
                let rrows = right.rows;
                let build_is_left = inner && lrows < rrows;
                let (b, p) = if build_is_left { (lrows, rrows) } else { (rrows, lrows) };
                let (hb, hp) = self.hash_costs(b);
                let cost = left.cost + right.cost + b * hb + self.over_budget(b) + p * hp + rows * self.row_copy();
                // Probing with the left rows keeps their order; building them, the unit's.
                let sorted = if build_is_left { right.sorted } else { left.sorted };
                if wanted.order(sorted) {
                    out.push(Choice::new(Algo::Hash { keys, check, build_is_left }, right, rows, cost, sorted));
                }
            }
            if try_merge {
                let check = shared.minus(both_seeded);
                self.merge_choices(g, left, i, mode, keys, check, &base, rows, pat_join.as_ref(), wanted, out)?;
            }
        }
        if !(try_bind || try_hash || merge_ok) && wants(Family::Nested) {
            // No usable key: a cross product per seed value, checking the maybe-bound shared slots.
            let check = shared.minus(both_seeded);
            let right = self.open_unit(g, i, rctx, mode, None)?;
            let rrows = right.rows;
            let match_rate = if check.is_empty() { 1.0 } else { 0.5 };
            let rows = finish_rows(lrows * rrows * match_rate);
            let cost =
                left.cost + right.cost + lrows * rrows * 2.0 * self.costs.row_copy_per_slot + rows * self.row_copy();
            out.push(Choice::new(Algo::Nested { check }, right, rows, cost, left.sorted));
        }
        Ok(())
    }

    /// BindJoin candidates: with `sort_left` (cheaper sorted probes, the left order is lost)
    /// and, when merge joins may still use it, without (the left order is kept).
    #[allow(clippy::too_many_arguments)]
    fn bind_choices(
        &mut self,
        g: &mut Group<'_>,
        left: &Shape,
        i: usize,
        seed: SlotSet,
        right: Opened,
        rows: f64,
        wanted: Wanted,
        out: &mut Vec<Choice>,
    ) -> Result<()> {
        let lrows = left.rows;
        // Left rows already in seed order probe in index order without re-sorting.
        let in_order = left.sorted.is_some_and(|v| seed.contains(v));
        let variants: SmallVec<[bool; 2]> = if in_order || lrows <= SORT_LEFT_ROWS {
            smallvec::smallvec![false]
        } else if left.sorted.is_some() && g.merge {
            smallvec::smallvec![true, false]
        } else {
            smallvec::smallvec![true]
        };
        for sort_left in variants {
            let sorted = bind_sorted(left.sorted, seed, sort_left);
            if !wanted.order(sorted) {
                continue;
            }
            let sorted_probes = sort_left || in_order;
            let probes = if sorted_probes { self.distinct_probes(g, seed, lrows, in_order)? } else { lrows };
            let mut per_probe = right.cost;
            if sorted_probes && let UnitRef::Pat(p) = g.units[i].r {
                // Probes in key order seek forward: cheap when the keys are dense in the
                // unit's index range, as costly as random probes when they are sparse. A
                // `sort_left` join sorts each batch on its own, so its runs are per batch.
                let range_rows = self.group_scan_estimate(g, i, &p, g.ctx.bound())?.0;
                let runs = if in_order { 1.0 } else { (lrows / BATCH_ROWS as f64).ceil().max(1.0) };
                per_probe -= self.costs.seek_random - self.ordered_seek_cost(probes / runs, range_rows);
            }
            let sort = if sort_left { lrows * SORT_LEFT_NS } else { 0.0 };
            let cost = left.cost + sort + probes * per_probe.max(0.0) + rows * self.row_copy();
            out.push(Choice::new(Algo::Bind { seed, sort_left }, right.clone(), rows, cost, sorted));
        }
        Ok(())
    }

    /// MergeJoin candidates, one per key slot (spec §7.5): the left side through an enforcer
    /// unless sorted by the key, the right side read in key order when an index order gives
    /// it, else enforced. Outside `PreferMerge` at most one side is enforced. A merge on one key
    /// that checks the other shared slots pays for every pair of its key runs (see the module
    /// docs); `pat_join` is how a quad pattern unit joins (`join_choices`).
    #[allow(clippy::too_many_arguments)]
    fn merge_choices(
        &mut self,
        g: &mut Group<'_>,
        left: &Shape,
        i: usize,
        mode: Mode,
        keys: SlotSet,
        shared: SlotSet,
        base: &Opened,
        rows: f64,
        pat_join: Option<&PatJoin>,
        wanted: Wanted,
        out: &mut Vec<Choice>,
    ) -> Result<()> {
        // Merge joins are inner: the unit opens in the group's own context.
        let bound = g.ctx.bound();
        let lrows = left.rows;
        let both_enforced_ok = self.opts.join_policy == JoinPolicy::PreferMerge;
        for key in slots(keys) {
            if !wanted.order(Some(key)) {
                continue;
            }
            let mut check = shared;
            check.remove(key);
            // The pairs of left rows and right rows of equal keys: the output rows unless other
            // shared slots are checked, else the rows of a join on the key alone.
            let pairs = if check.is_empty() {
                rows
            } else {
                let per_left = match (pat_join, g.units[i].r) {
                    (Some(j), _) => j.per_left_on(key),
                    (None, UnitRef::Node(n)) => {
                        self.node_rows_given(g, i, n, SlotSet::from_slots([key]), lrows, base.rows)?
                    }
                    (None, UnitRef::Pat(_)) => unreachable!("quad pattern units come with their join estimate"),
                };
                clamp_est(lrows * per_left).max(rows)
            };
            let enforce_left = left.sorted != Some(key);
            let (right, enforce_right, seekable) = match g.units[i].r {
                UnitRef::Pat(p) => {
                    // Galloping seeks need the scan read through one cursor.
                    let seekable = single_cursor(&p, bound, self.dataset);
                    match (self.sorted_access(g, i, &p, key), mode) {
                        // A plain scan's estimates depend on its bound columns only, not on the
                        // index order that reads it.
                        (Some(_), Mode::Probe) => (Opened { sorted: Some(key), ..base.clone() }, false, seekable),
                        (Some(a), Mode::Build) => (self.open_unit(g, i, g.ctx, mode, Some(a))?, false, seekable),
                        (None, _) => (base.clone(), true, false),
                    }
                }
                UnitRef::Node(_) => (base.clone(), base.sorted != Some(key), false),
            };
            if enforce_left && enforce_right && !both_enforced_ok {
                continue;
            }
            let left_cost = left.cost + if enforce_left { self.enforce_cost(lrows) } else { 0.0 };
            // Right rows under keys the left side has: read, and buffered run by run.
            let matched = pairs.min(right.rows);
            let right_cost = if enforce_right {
                right.cost + self.enforce_cost(right.rows)
            } else if seekable {
                // Galloping: after a few misses the right scan seeks to the next left key. Each
                // seek follows up to `gallop` rows read past the previous key, and is a random
                // seek when the keys are sparse in the right input.
                let distinct = lrows.min(self.slot_dv(g, key)?);
                let per_row = (right.cost - self.costs.seek_random).max(0.0) / right.rows.max(1.0);
                let gap = right.rows / distinct.max(1.0);
                let per_seek = self.ordered_seek_cost(distinct, right.rows) + gap.min(f64::from(self.gallop)) * per_row;
                let gallop = self.costs.seek_random + distinct * per_seek + matched * per_row;
                right.cost.min(gallop)
            } else {
                right.cost
            };
            let checked = if check.is_empty() { 0.0 } else { pairs * self.pair_cost(slot_count(check)) };
            let cost = left_cost + right_cost + matched * self.row_copy() + checked + rows * self.row_copy();
            let algo = Algo::Merge { key, check };
            let mut ch = Choice::new(algo, right, rows, cost, Some(key));
            if check.is_empty() {
                // Counted, it multiplies run lengths: no run buffered, no row produced.
                ch.count_free = self.merge_count_free(rows, matched);
            }
            out.push(ch);
        }
        Ok(())
    }

    /// Of a check-free MergeJoin's cost, what counting its `rows` skips (`fast_count`): the
    /// output rows and the `matched` right rows it would buffer, one row copy each.
    pub(super) fn merge_count_free(&self, rows: f64, matched: f64) -> f64 {
        (rows + matched) * self.row_copy()
    }

    /// Cost of one pair of rows a MergeJoin compares before merging (a left row and a row of
    /// the buffered right run of its key) when it checks `checked` slots: about a row copy
    /// (reading the candidate row), plus the comparisons. Measured: 5.7 ns per pair for 4-slot
    /// rows with one checked slot (release, 9.4e9 pairs in 54 s), modelled as 5 ns.
    pub(super) fn pair_cost(&self, checked: usize) -> f64 {
        self.row_copy() + checked as f64 * self.costs.row_copy_per_slot
    }

    /// How quad pattern unit `i` (opened with `bound`) joins the prefix `left` on `keys` (spec
    /// §7.4 joins): its rows and the selectivity of each key ([`PatJoin`]). A key's selectivity
    /// is `1 / max(dv_L, dv_R)`: the unit's rows per value of the key over its rows
    /// (`1 / dv_R`), at most one over the distinct values of the key in the prefix (`dv_L`,
    /// [`Self::key_dvs`]): a prefix with more values than the unit has finds a match for only
    /// some of its rows (containment of value sets). `star` (the characteristic-set ratio of a
    /// unit that extends a subject star of the prefix: rows per subject) gives the subject's
    /// selectivity (`star / rows`) when the subject is a key, and the rows when the group's
    /// seed binds it.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn pattern_join(
        &mut self,
        g: &mut Group<'_>,
        left: &Shape,
        i: usize,
        p: &QuadPat,
        bound: SlotSet,
        keys: SlotSet,
        star: Option<f64>,
    ) -> Result<PatJoin> {
        let keys = keys.minus(bound);
        let base = self.group_scan_estimate(g, i, p, bound)?.0;
        let mut join = PatJoin { rows: base, keys: 0, sels: [NO_KEY; 4] };
        let subject = match (star, p.s) {
            (Some(r), PatTerm::Slot(s)) if bound.contains(s) => {
                join.rows = r; // a seeded star
                return Ok(join);
            }
            (Some(r), PatTerm::Slot(s)) if keys.contains(s) => Some((s, r)),
            _ => None,
        };
        if base <= 0.0 {
            return Ok(join);
        }
        // The cached distinct values are those of the units opened in the group's context.
        let own = bound == g.ctx.bound();
        for v in slots(keys) {
            // The selectivity, and the unit's rows per value of the key.
            let (sel, per_value) = match subject {
                Some((s, r)) if s == v => (r / base, r),
                _ => {
                    let (dv_r, dv_l) = self.key_dvs(g, left, i, v)?;
                    let sel_r = match dv_r {
                        Some(dv) if own => 1.0 / dv.max(1.0),
                        _ => self.group_scan_estimate(g, i, p, bound.union(SlotSet::from_slots([v])))?.0 / base,
                    };
                    (dv_l.map_or(sel_r, |dv| sel_r.min(1.0 / dv)), base * sel_r)
                }
            };
            // The keys are slots of the pattern: at most four.
            if let Some(to) = join.sels.get_mut(join.keys) {
                *to = (v, KeySel { sel, attribute: per_value >= ATTRIBUTE_ROWS });
                join.keys += 1;
            }
        }
        Ok(join)
    }

    /// Distinct values of slot `v` in quad pattern unit `i` and among the rows of the prefix
    /// `left`. A unit's are its rows over its rows per value (in the group's context); the
    /// prefix's are the fewest of its quad pattern units that bind `v`, at most its rows, and
    /// None when none does (an opaque unit or the group's seed binds `v`).
    fn key_dvs(&mut self, g: &mut Group<'_>, left: &Shape, i: usize, v: Slot) -> Result<(Option<f64>, Option<f64>)> {
        if g.unit_dv_index[usize::from(v)] == NO_DVS {
            // Once per slot and group: the ordering search asks for every prefix.
            let bound = g.ctx.bound();
            let only = bound.union(SlotSet::from_slots([v]));
            let mut dvs: SmallVec<[(f64, usize); 4]> = SmallVec::new();
            // A slot of the group's seed has one value per open: no join key.
            for j in (0..g.units.len()).filter(|_| !bound.contains(v)) {
                let UnitRef::Pat(p) = g.units[j].r else { continue };
                if !g.units[j].props.certain.contains(v) {
                    continue;
                }
                let all = self.group_scan_estimate(g, j, &p, bound)?.0;
                let per_value = self.group_scan_estimate(g, j, &p, only)?.0;
                dvs.push((if per_value > 0.0 { all / per_value } else { all }, j));
            }
            dvs.sort_by(|a, b| a.0.total_cmp(&b.0));
            // At most one list per slot: fewer than `NO_DVS`.
            g.unit_dv_index[usize::from(v)] = g.unit_dvs.len() as u16;
            g.unit_dvs.push(dvs);
        }
        let dvs = &g.unit_dvs[usize::from(g.unit_dv_index[usize::from(v)])];
        let unit = dvs.iter().find(|&&(_, j)| j == i).map(|&(dv, _)| dv);
        let prefix = dvs.iter().find(|&&(_, j)| left.units.contains(j)).map(|&(dv, _)| dv.min(left.rows).max(1.0));
        Ok((unit, prefix))
    }

    /// Probes of a BindJoin whose probes run in key order: one per distinct seed tuple (runs of
    /// equal tuples reuse the previous probe), per 1024-row batch unless the whole left input is
    /// sorted by a seed slot (`globally`). Slots of the group's seed are constant.
    fn distinct_probes(&mut self, g: &mut Group<'_>, seed: SlotSet, lrows: f64, globally: bool) -> Result<f64> {
        let mut distinct = 1.0;
        for s in slots(seed.minus(g.ctx.seeded)) {
            distinct *= self.slot_dv(g, s)?;
        }
        if !distinct.is_finite() {
            return Ok(lrows);
        }
        let batches = if globally { 1.0 } else { (lrows / BATCH_ROWS as f64).ceil().max(1.0) };
        Ok(lrows.min(distinct * batches))
    }

    /// Rows of opaque unit `i` (`n`) per left row when joined on `keys` (estimate only): `n`
    /// estimated with the keys seeded when it accepts them, else its rows without them
    /// (`standalone`) spread over the larger side.
    fn node_rows_given(
        &mut self,
        g: &Group<'_>,
        i: usize,
        n: &Logical,
        keys: SlotSet,
        lrows: f64,
        standalone: f64,
    ) -> Result<f64> {
        let (props, ctx) = (g.units[i].props, g.ctx);
        if props.accepts_seed(keys) {
            Ok(self.estimate(n, Ctx { seeded: keys.union(ctx.seeded), ..ctx })?.rows)
        } else {
            Ok(standalone / standalone.max(lrows).max(1.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sq_core::eval_api::TempDict;
    use sq_core::ir::{DatasetSpec, DefaultGraph, NamedGraphs};
    use sq_core::plan::QueryOptions;

    #[test]
    fn buffers_over_the_memory_budget_are_penalized() {
        let store = sq_memstore::MemStore::new();
        let opts = QueryOptions { max_memory_mb: 1, ..QueryOptions::default() };
        let ds = DatasetSpec { default: DefaultGraph::StoreDefault, named: NamedGraphs::All };
        let temps = TempDict::default();
        // 4 slots: 56 bytes per buffered row, 18,724 rows per MB.
        let planner = Planner::new(&store, &opts, &ds, 0, &[], &temps, 4);
        assert_eq!(planner.over_budget(18_724.0), 0.0);
        assert_eq!(planner.over_budget(18_725.0), 18_725.0 * OVER_BUDGET_ROW_NS);
        let sort = |rows: f64| {
            let n: f64 = rows;
            n * n.log2() * planner.costs.sort_row_log + n * planner.row_copy()
        };
        assert!((planner.enforce_cost(10_000.0) - sort(10_000.0)).abs() < 1e-6);
        assert!((planner.enforce_cost(20_000.0) - sort(20_000.0) - 2e9).abs() < 1e-3);
        // No budget: every buffer is over it.
        let none = QueryOptions { max_memory_mb: 0, ..QueryOptions::default() };
        let planner = Planner::new(&store, &none, &ds, 0, &[], &temps, 4);
        assert_eq!(planner.over_budget(1.0), OVER_BUDGET_ROW_NS);
    }

    #[test]
    fn bind_join_output_order() {
        let seed = SlotSet::from_slots([2, 5]);
        assert_eq!(bind_sorted(Some(7), seed, false), Some(7));
        assert_eq!(bind_sorted(Some(7), seed, true), None);
        // Sorting batches by a seed tuple that starts with the sorted slot keeps the order.
        assert_eq!(bind_sorted(Some(2), seed, true), Some(2));
        assert_eq!(bind_sorted(None, seed, true), None);
    }

    fn entity(sel: f64) -> KeySel {
        KeySel { sel, attribute: false }
    }

    fn attr(sel: f64) -> KeySel {
        KeySel { sel, attribute: true }
    }

    #[test]
    fn attribute_keys_back_off_and_entity_keys_multiply() {
        assert_eq!(combine_keys(&mut []), 1.0);
        assert_eq!(combine_keys(&mut [attr(0.05)]), 0.05);
        // The most selective key in full, then an attribute key square-rooted, in any order.
        let two = 1e-5 * 0.05f64.sqrt();
        assert!((combine_keys(&mut [attr(0.05), entity(1e-5)]) - two).abs() < 1e-18);
        assert!((combine_keys(&mut [entity(1e-5), attr(0.05)]) - two).abs() < 1e-18);
        // Further attribute keys fourth-rooted, eighth-rooted...; entity keys in full.
        let three = 1e-5 * 0.05f64.sqrt() * 0.5f64.powf(0.25);
        assert!((combine_keys(&mut [attr(0.5), attr(0.05), entity(1e-5)]) - three).abs() < 1e-18);
        let mixed = 1e-5 * 0.05f64.sqrt() * 1e-3;
        assert!((combine_keys(&mut [attr(0.05), entity(1e-3), entity(1e-5)]) - mixed).abs() < 1e-24);
        // Closing a cycle on entity keys: independent keys (spec §7.4.5), the product.
        assert_eq!(combine_keys(&mut [entity(1e-3), entity(2e-3)]), 2e-6);
        // Attribute keys: between independence (the product) and the most selective key.
        let sels = [0.3, 0.01, 0.2, 0.001];
        let combined = combine_keys(&mut sels.map(attr));
        assert!(combined > sels.iter().product::<f64>() && combined < 0.001, "{combined}");
        // Out-of-range inputs are clamped.
        assert_eq!(combine_keys(&mut [attr(2.0)]), 1.0);
        assert_eq!(combine_keys(&mut [entity(-1.0), attr(0.5)]), 0.0);
    }

    #[test]
    fn pattern_joins_estimate_all_keys_or_one() {
        let j = PatJoin { rows: 1e5, keys: 2, sels: [(3, entity(1e-5)), (7, attr(0.05)), NO_KEY, NO_KEY] };
        // ?c :livesIn ?city joined on ?c and ?city: ~0.22 rows per left row (the city backed
        // off), against 1 on ?c alone (the city checked afterwards) and 5 000 on ?city alone.
        assert!((j.per_left() - 0.05f64.sqrt()).abs() < 1e-12, "{}", j.per_left());
        assert!((j.per_left_on(3) - 1.0).abs() < 1e-9);
        assert!((j.per_left_on(7) - 5_000.0).abs() < 1e-6);
        // A slot that is not a key does not restrict.
        assert_eq!(j.per_left_on(9), 1e5);
        // ?z :knows ?x closing a triangle: two entity keys multiply.
        let sels = [(1, entity(1.0 / 200.0)), (2, entity(1.0 / 200.0)), NO_KEY, NO_KEY];
        let cycle = PatJoin { rows: 600.0, keys: 2, sels };
        assert!((cycle.per_left() - 0.015).abs() < 1e-12, "{}", cycle.per_left());
        let none = PatJoin { rows: 42.0, keys: 0, sels: [NO_KEY; 4] };
        assert_eq!(none.per_left(), 42.0);
    }
}
