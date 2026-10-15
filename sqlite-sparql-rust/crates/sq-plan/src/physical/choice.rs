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
//!   scanned. Sorted by the key.
//! * **NestedLoop** when nothing else is legal. Keeps the left order.
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
use sq_core::plan::JoinPolicy;
use sq_core::{Result, BATCH_ROWS};

use super::join::{Algo, Choice, Family, Group, Mode, Opened, Shape, UnitRef, SORT_LEFT_ROWS};
use super::scan::{bound_columns, range_access, single_cursor};
use super::sqljoin::MAX_ALIASES;
use super::{Ctx, Planner};
use crate::props::slots;

/// Cost of sorting a left batch by its seed tuple for `sort_left`, per row (about 20 µs per
/// 1024 rows, spec §5.3).
const SORT_LEFT_NS: f64 = 20.0;
/// Hash tables up to this many entries use the `_small` constants (spec §7.5).
const SMALL_HASH: f64 = 32_768.0;
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

    /// Cost of a SortBySlot enforcer over `rows` rows (buffer and sort).
    pub(super) fn enforce_cost(&self, rows: f64) -> f64 {
        let n = rows.max(1.0);
        n * n.log2().max(1.0) * self.costs.sort_row_log + n * self.row_copy()
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
        let finish_rows = |rows: f64| match left_join {
            Some(sel) => (rows * sel).max(lrows),
            None => rows,
        };

        if (try_bind && wants(Family::Bind)) || (try_sql && wants(Family::Sql)) {
            let probe_ctx = Ctx { seeded: seed, ..ctx };
            let right = self.open_unit(g, i, probe_ctx, mode, None)?;
            let rows = finish_rows(lrows * star.unwrap_or(right.rows));
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
            let per_left = match star {
                Some(r) => r,
                None => self.rows_given(g, i, keys, lrows, base.rows)?,
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
                let cost = left.cost + right.cost + b * hb + p * hp + rows * self.row_copy();
                // Probing with the left rows keeps their order; building them, the unit's.
                let sorted = if build_is_left { right.sorted } else { left.sorted };
                if wanted.order(sorted) {
                    out.push(Choice::new(Algo::Hash { keys, check, build_is_left }, right, rows, cost, sorted));
                }
            }
            if try_merge {
                let check = shared.minus(both_seeded);
                self.merge_choices(g, left, i, rctx, mode, keys, check, &base, rows, wanted, out)?;
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
    /// it, else enforced. Outside `PreferMerge` at most one side is enforced.
    #[allow(clippy::too_many_arguments)]
    fn merge_choices(
        &mut self,
        g: &mut Group<'_>,
        left: &Shape,
        i: usize,
        rctx: Ctx,
        mode: Mode,
        keys: SlotSet,
        shared: SlotSet,
        base: &Opened,
        rows: f64,
        wanted: Wanted,
        out: &mut Vec<Choice>,
    ) -> Result<()> {
        let lrows = left.rows;
        let both_enforced_ok = self.opts.join_policy == JoinPolicy::PreferMerge;
        for key in slots(keys) {
            if !wanted.order(Some(key)) {
                continue;
            }
            let mut check = shared;
            check.remove(key);
            let enforce_left = left.sorted != Some(key);
            let (right, enforce_right, seekable) = match g.units[i].r {
                UnitRef::Pat(p) => {
                    // Merge joins are inner: the unit opens in the group's own context.
                    debug_assert_eq!(rctx, g.ctx);
                    // Galloping seeks need the scan read through one cursor.
                    let seekable = single_cursor(&p, rctx.bound(), self.dataset);
                    match (self.sorted_access(g, i, &p, key), mode) {
                        // A plain scan's estimates depend on its bound columns only, not on the
                        // index order that reads it.
                        (Some(_), Mode::Probe) => (Opened { sorted: Some(key), ..base.clone() }, false, seekable),
                        (Some(a), Mode::Build) => (self.open_unit(g, i, rctx, mode, Some(a))?, false, seekable),
                        (None, _) => (base.clone(), true, false),
                    }
                }
                UnitRef::Node(_) => (base.clone(), base.sorted != Some(key), false),
            };
            if enforce_left && enforce_right && !both_enforced_ok {
                continue;
            }
            let left_cost = left.cost + if enforce_left { self.enforce_cost(lrows) } else { 0.0 };
            let right_cost = if enforce_right {
                right.cost + self.enforce_cost(right.rows)
            } else if seekable {
                // Galloping: after a few misses the right scan seeks to the next left key. Each
                // seek follows up to `gallop` rows read past the previous key, and is a random
                // seek when the keys are sparse in the right input.
                let distinct = lrows.min(self.slot_dv(g, key)?);
                let matched = rows.min(right.rows);
                let per_row = (right.cost - self.costs.seek_random).max(0.0) / right.rows.max(1.0);
                let gap = right.rows / distinct.max(1.0);
                let per_seek = self.ordered_seek_cost(distinct, right.rows) + gap.min(f64::from(self.gallop)) * per_row;
                let gallop = self.costs.seek_random + distinct * per_seek + matched * per_row;
                right.cost.min(gallop)
            } else {
                right.cost
            };
            let cost = left_cost + right_cost + rows * self.row_copy();
            let algo = Algo::Merge { key, check };
            out.push(Choice::new(algo, right, rows, cost, Some(key)));
        }
        Ok(())
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_join_output_order() {
        let seed = SlotSet::from_slots([2, 5]);
        assert_eq!(bind_sorted(Some(7), seed, false), Some(7));
        assert_eq!(bind_sorted(Some(7), seed, true), None);
        // Sorting batches by a seed tuple that starts with the sorted slot keeps the order.
        assert_eq!(bind_sorted(Some(2), seed, true), Some(2));
        assert_eq!(bind_sorted(None, seed, true), None);
    }
}
