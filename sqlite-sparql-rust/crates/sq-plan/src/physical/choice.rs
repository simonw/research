//! Join operator candidates for one join step (spec §7.5) and their costs.
//!
//! For a joined prefix (left [`Shape`]) and a next unit, every legal operator that `JoinPolicy`
//! allows becomes a [`Choice`] with its estimated rows, cost and output order:
//! * **BindJoin** (index nested loop): `probes × cost(R | seeded) + out × row_copy`, where a
//!   left input sorted by a seed slot, or sorted per batch (`sort_left`), probes in key order
//!   (`seek_sorted`) and only once per distinct key. Keeps the left order unless `sort_left`.
//! * **SQL step**: the same probe, run by SQLite's VDBE inside one SqlJoin statement
//!   (`vdbe_probe` per left row, `vdbe_row` per produced column); only while the prefix is an
//!   open SQL chain. Unordered.
//! * **HashJoin** (build the smaller side, `_small` / `_large` constants around 32K entries).
//!   Keeps the probe side's order.
//! * **MergeJoin** on each certainly-bound shared slot: both sides sorted by it, either already
//!   (a quad pattern read through an index order whose first free column is the key) or through
//!   a SortBySlot enforcer (`n·log2(n)·sort_row_log`); the right side gallops
//!   (`min(|R| scanned, distinct(L) × seek_sorted + matched rows)`). Sorted by the key.
//! * **NestedLoop** when nothing else is legal. Keeps the left order.
//!
//! Policies: `PreferHash` and `PreferBind` force their operator wherever legal (WP5);
//! `PreferMerge` takes only merge joins wherever a key exists (enforcers on both sides allowed);
//! `Auto` and `NoSql` compare everything (at most one enforcer per merge); SQL steps only under
//! `Auto` with a store that runs SQL (`Group::sql`).
use smallvec::SmallVec;
use sq_core::batch::{Slot, SlotSet};
use sq_core::plan::JoinPolicy;
use sq_core::{Result, BATCH_ROWS};

use super::join::{Algo, Choice, Family, Group, Mode, Opened, Shape, UnitRef, SORT_LEFT_ROWS};
use super::scan::{bound_columns, range_access};
use super::{Ctx, Planner};
use crate::props::slot_list;

/// Cost of sorting a left batch by its seed tuple for `sort_left`, per row (about 20 µs per
/// 1024 rows, spec §5.3).
const SORT_LEFT_NS: f64 = 20.0;
/// Hash tables up to this many entries use the `_small` constants (spec §7.5).
const SMALL_HASH: f64 = 32_768.0;

/// Output order of a BindJoin whose left input is sorted by `left`: the left order, unless
/// batches are re-sorted by a seed tuple that does not start with that slot.
pub(super) fn bind_sorted(left: Option<Slot>, seed: SlotSet, sort_left: bool) -> Option<Slot> {
    if !sort_left || slot_list(seed).first().copied() == left { left } else { None }
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

    /// Cost of a SortBySlot enforcer over `rows` rows (buffer and sort).
    pub(super) fn enforce_cost(&self, rows: f64) -> f64 {
        let n = rows.max(1.0);
        n * n.log2().max(1.0) * self.costs.sort_row_log + n * self.row_copy()
    }

    /// Every candidate operator joining the prefix `left` with unit `i` (see the module docs).
    /// `left_join` = Some(filter selectivity) for an OPTIONAL (kind Left), None for an inner
    /// join; `only` restricts the candidates to one family (replaying a recorded decision).
    ///
    /// Inside a group opened with seed S (`ctx.seeded`): a BindJoin re-opens the unit with its
    /// seed list only, so every slot both sides may bind, S included, must be carried by the
    /// left rows; HashJoin / MergeJoin / NestedLoop open the unit with S itself when the join
    /// is inner, and slots of S are then neither keys nor checked (see `physical`).
    pub(super) fn join_choices(
        &mut self,
        g: &mut Group<'_>,
        left: &Shape,
        i: usize,
        left_join: Option<f64>,
        mode: Mode,
        only: Option<Family>,
    ) -> Result<SmallVec<[Choice; 4]>> {
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
        let merge_ok = inner && g.merge && hash_ok;
        let policy = self.opts.join_policy;
        let prefer_merge = policy == JoinPolicy::PreferMerge && merge_ok;
        let try_bind = bind_ok && policy != JoinPolicy::PreferHash && !prefer_merge;
        let try_sql = try_bind && inner && g.sql && left.chain >= 1 && unit.sqlable;
        let try_hash = hash_ok && !(policy == JoinPolicy::PreferBind && bind_ok) && !prefer_merge;
        let wants = |f: Family| only.is_none_or(|o| o == f);
        // Inputs other than a BindJoin's right side receive the outer seed only for inner joins.
        let rctx = if inner { ctx } else { ctx.unseeded() };
        let lrows = left.rows;
        // Rows per left row when the unit extends a subject star (characteristic sets).
        let star = if inner { self.star_ratio(g, &left.units, i) } else { None };
        let finish_rows = |rows: f64| match left_join {
            Some(sel) => (rows * sel).max(lrows),
            None => rows,
        };
        let mut out: SmallVec<[Choice; 4]> = SmallVec::new();

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
                self.bind_choices(g, left, i, seed, right, rows, &mut out)?;
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
                let sorted = if build_is_left { right.sorted } else { left.sorted };
                out.push(Choice::new(Algo::Hash { keys, check, build_is_left }, right, rows, cost, sorted));
            }
            if try_merge {
                self.merge_choices(g, left, i, rctx, mode, keys, shared.minus(both_seeded), &base, rows, &mut out)?;
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
        Ok(out)
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
        out: &mut SmallVec<[Choice; 4]>,
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
        let is_scan = matches!(g.units[i].r, UnitRef::Pat(_));
        for sort_left in variants {
            let sorted_probes = sort_left || in_order;
            let probes = if sorted_probes { self.distinct_probes(g, seed, lrows, in_order)? } else { lrows };
            let mut per_probe = right.cost;
            if sorted_probes && is_scan {
                per_probe -= self.costs.seek_random - self.costs.seek_sorted;
            }
            let sort = if sort_left { lrows * SORT_LEFT_NS } else { 0.0 };
            let cost = left.cost + sort + probes * per_probe.max(0.0) + rows * self.row_copy();
            let sorted = bind_sorted(left.sorted, seed, sort_left);
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
        out: &mut SmallVec<[Choice; 4]>,
    ) -> Result<()> {
        let lrows = left.rows;
        let both_enforced_ok = self.opts.join_policy == JoinPolicy::PreferMerge;
        for key in slot_list(keys) {
            let mut check = shared;
            check.remove(key);
            let enforce_left = left.sorted != Some(key);
            let (right, enforce_right, seekable) = match g.units[i].r {
                UnitRef::Pat(p) => {
                    // Merge joins are inner: the unit opens in the group's own context.
                    debug_assert_eq!(rctx, g.ctx);
                    let accesses = self.unit_accesses(g, i, &p);
                    let access = accesses.into_iter().find(|(_, s)| *s == Some(key)).map(|(a, _)| a);
                    match access {
                        Some(a) => (self.open_unit(g, i, rctx, mode, Some(a))?, false, true),
                        None => (base.clone(), true, false),
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
                // Galloping: after a few misses the right scan seeks to the next left key.
                let distinct = lrows.min(self.slot_dv(g, key)?);
                let matched = rows.min(right.rows);
                let per_row = (right.cost - self.costs.seek_random).max(0.0) / right.rows.max(1.0);
                let gallop = self.costs.seek_random + distinct * self.costs.seek_sorted + matched * per_row;
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
        for s in slot_list(seed.minus(g.ctx.seeded)) {
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
