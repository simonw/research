//! Estimates without planning, for the join ordering search (spec §7.5).
//!
//! The ordering search probes every opaque unit of a group (UNION, OPTIONAL, sub-select,
//! nested group, ...) with many seed sets, one per joined prefix. Planning the unit for each of
//! them re-ran its own ordering search per seed set, and at every nesting level, so planning
//! time grew with the product of the seed sets explored per level. Instead a probed unit is
//! costed from ONE plan per node, made for its context-free base context, and re-costed for
//! the slots bound at open time with the planner's own formulas: leaves are re-estimated, the
//! operators above them scale with their inputs, operators and order stay as they are. Real
//! plans are only built when the winning order is replayed (`Mode::Build`), so every node is
//! planned in at most one context per planned context of its parent, plus its base context.
use sq_core::batch::SlotSet;
use sq_core::ir::Logical;
use sq_core::plan::{Op, Plan};
use sq_core::Result;

use super::{carried, clamp_est, input_seeds, Ctx, Planner};
use crate::props::ALL;

/// Estimated rows and cost of one open of a subtree, and which seeded slots its rows carry.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Est {
    pub(crate) rows: f64,
    pub(crate) cost: f64,
    /// `carried(plan, ALL)`: the rows of the estimated plan carry `seed ∩ carries` of any seed.
    pub(crate) carries: SlotSet,
}

/// new / old, or 1 when there was nothing to scale.
fn ratio(new: f64, old: f64) -> f64 {
    if old > 0.0 { new / old } else { 1.0 }
}

impl Planner<'_> {
    /// Estimate opening `n` in `ctx` (memoized): the real plan's numbers when it was already
    /// planned for `ctx`, otherwise its base plan re-costed for `ctx`'s bound slots.
    pub(crate) fn estimate(&mut self, n: &Logical, ctx: Ctx) -> Result<Est> {
        let key = self.key(n, ctx);
        if let Some(e) = self.est_memo.get(&key) {
            return Ok(*e);
        }
        let est = match self.memo.get(&key) {
            Some(p) => Est { rows: p.est_rows, cost: p.est_cost_ns, carries: carried(p, ALL) },
            None => {
                let base = self.plan(n, Ctx { in_exists: ctx.in_exists, ..Ctx::default() })?;
                let (rows, cost) = self.recost(&base, SlotSet::EMPTY, key.1.bound())?;
                Est { rows, cost, carries: carried(&base, ALL) }
            }
        };
        self.est_memo.insert(key, est);
        Ok(est)
    }

    /// Rows and cost of one open of `p`, planned with the slots `bound` bound at open time,
    /// when the slots `k` are bound as well. A seed reaches the inputs that receive the
    /// subtree's seed (`input_seeds`); the right input of a BindJoin, planned for its seed
    /// list, sees `k` too (a real plan would carry those slots in that list).
    pub(crate) fn recost(&mut self, p: &Plan, bound: SlotSet, k: SlotSet) -> Result<(f64, f64)> {
        let k = k.minus(bound);
        if k.is_empty() {
            return Ok((p.est_rows, p.est_cost_ns));
        }
        let b = bound.union(k);
        let (rows, cost) = match &p.op {
            Op::Scan(s) => self.scan_estimate(&s.pat, b)?,
            Op::Path { g, s, path, o } => self.path_estimate(*g, *s, path, *o, b),
            Op::Values { slots, rows } => self.values_estimate(slots, rows, b),
            Op::Unit | Op::Empty | Op::GraphNames { .. } | Op::SqlJoin { .. } => (p.est_rows, p.est_cost_ns),
            Op::Union(branches) => {
                let (mut rows, mut cost) = (0.0, 0.0);
                for br in branches {
                    let (r, c) = self.recost(br, bound, k)?;
                    rows += r;
                    cost += c;
                }
                (rows, cost)
            }
            Op::BindJoin { left, right, seed, .. } => {
                let (lr, lc) = self.recost(left, bound, k)?;
                let (rr, rc) = self.recost(right, SlotSet::from_slots(seed.iter().copied()), k)?;
                let rows = p.est_rows * ratio(lr, left.est_rows) * ratio(rr, right.est_rows);
                (rows, lc + lr * rc + rows * self.row_copy())
            }
            _ => {
                // The inputs that receive the seed: all of them, the left one only (outer, semi
                // and anti joins, Minus) or none (never-seeded operators keep their estimates).
                let seeded: Vec<&Plan> =
                    input_seeds(p, b).into_iter().filter(|(_, s)| !s.is_empty()).map(|(c, _)| c).collect();
                self.scale_with_inputs(p, &seeded, bound, k)?
            }
        };
        Ok((clamp_est(rows), clamp_est(cost)))
    }

    /// `p` re-costed when its `seeded` inputs see `k` more bound slots: rows scale with the
    /// product of the inputs' row ratios, the operator's own work with the largest ratio.
    fn scale_with_inputs(&mut self, p: &Plan, seeded: &[&Plan], bound: SlotSet, k: SlotSet) -> Result<(f64, f64)> {
        if seeded.is_empty() {
            return Ok((p.est_rows, p.est_cost_ns));
        }
        let children: f64 = p.children().iter().map(|c| c.est_cost_ns).sum();
        let own = (p.est_cost_ns - children).max(0.0);
        let (mut rows_f, mut work_f, mut cost) = (1.0, 0.0f64, children);
        for c in seeded {
            let (r, cc) = self.recost(c, bound, k)?;
            let f = ratio(r, c.est_rows);
            rows_f *= f;
            work_f = work_f.max(f);
            cost += cc - c.est_cost_ns;
        }
        Ok((p.est_rows * rows_f, cost + own * work_f))
    }
}
