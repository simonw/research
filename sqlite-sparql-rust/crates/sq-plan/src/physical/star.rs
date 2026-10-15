//! StarMerge (spec §7.5): a left-deep chain of k ≥ 3 inputs merge-joined on one slot (a subject
//! star read through s-sorted streams) may instead be one leapfrog intersection. It is taken
//! only when its cost beats the chained MergeJoins: a badly used leapfrog measured 4.3 µs per
//! output row, so every leapfrog step is charged [`STAR_STEP_NS`].
use sq_core::batch::{Slot, SlotSet};
use sq_core::plan::{Op, Plan};

use super::{mk, Planner};
use crate::props::slot_list;

/// Cost of one leapfrog step (one input positioned on a candidate key).
const STAR_STEP_NS: f64 = 300.0;

/// Number of inputs of the chain of MergeJoins on `key` along the left spine of `p`.
fn chain_inputs(p: &Plan, key: Slot) -> usize {
    match &p.op {
        Op::MergeJoin { left, key: k, .. } if *k == key => 1 + chain_inputs(left, key),
        _ => 1,
    }
}

/// The inputs of the chain (left to right) and the union of its checks.
fn flatten(p: Plan, key: Slot, inputs: &mut Vec<Plan>, check: &mut SlotSet) {
    match p.op {
        Op::MergeJoin { left, right, key: k, check: c, .. } if k == key => {
            flatten(*left, key, inputs, check);
            *check = check.union(SlotSet::from_slots(c));
            inputs.push(*right);
        }
        op => inputs.push(Plan { op, ..p }),
    }
}

impl Planner<'_> {
    /// Replace chains of at least three MergeJoins' inputs on one key along the left spine of a
    /// group's plan by a StarMerge where that is cheaper. Units planned as their own groups
    /// were already rewritten.
    pub(super) fn star_merges(&self, mut p: Plan) -> Plan {
        if let Op::MergeJoin { key, .. } = p.op {
            if chain_inputs(&p, key) >= 3 {
                return self.star_merge(p, key);
            }
        }
        // Continue down the left spine (the group's own operators).
        let spine = match &mut p.op {
            Op::MergeJoin { left, .. } | Op::BindJoin { left, .. } | Op::NestedLoop { left, .. } => Some(left),
            Op::HashJoin { build, probe, build_is_left, .. } => Some(if *build_is_left { build } else { probe }),
            Op::Filter { input, .. } | Op::SortBySlot { input, .. } => Some(input),
            _ => None,
        };
        if let Some(child) = spine {
            let c = std::mem::replace(&mut **child, mk(Op::Empty, 0.0, 0.0, None));
            **child = self.star_merges(c);
        }
        p
    }

    /// `p` (a MergeJoin chain on `key`) as a StarMerge when cheaper, else unchanged.
    fn star_merge(&self, p: Plan, key: Slot) -> Plan {
        let (rows, chain_cost, id) = (p.est_rows, p.est_cost_ns, p.id);
        let star_cost = {
            let mut inputs = Vec::new();
            let mut probe = |q: &Plan| inputs.push((q.est_rows, q.est_cost_ns, matches!(q.op, Op::Scan(_))));
            let mut q = &p;
            while let Op::MergeJoin { left, right, key: k, .. } = &q.op {
                if *k != key {
                    break;
                }
                probe(right);
                q = left;
            }
            probe(q);
            self.star_cost(&inputs, rows)
        };
        if star_cost >= chain_cost {
            return p;
        }
        let mut inputs = Vec::new();
        let mut check = SlotSet::EMPTY;
        flatten(p, key, &mut inputs, &mut check);
        let mut star = mk(Op::StarMerge { key, inputs, check: slot_list(check) }, rows, star_cost, Some(key));
        star.id = id;
        star
    }

    /// `Σ_i min(|R_i| scanned, |driver| × seek_sorted + matched_i) + steps × 300 ns + out`,
    /// where the driver is the smallest input and every step positions one input on a
    /// candidate key. `inputs` are (rows, cost, is a seekable scan).
    fn star_cost(&self, inputs: &[(f64, f64, bool)], out_rows: f64) -> f64 {
        let driver = inputs.iter().map(|i| i.0).fold(f64::INFINITY, f64::min).max(0.0);
        let mut cost = 0.0;
        for &(rows, c, scan) in inputs {
            cost += if scan {
                let per_row = (c - self.costs.seek_random).max(0.0) / rows.max(1.0);
                c.min(self.costs.seek_random + driver * self.costs.seek_sorted + rows.min(out_rows) * per_row)
            } else {
                c
            };
        }
        cost + inputs.len() as f64 * driver * STAR_STEP_NS + out_rows * self.row_copy()
    }
}
