//! StarMerge (spec §7.5): a left-deep chain of k ≥ 3 inputs merge-joined on one slot (a subject
//! star read through s-sorted streams) may instead be one leapfrog intersection. The post-pass
//! takes it only when it is cheaper than the chained MergeJoins for what the group's rows are
//! used for ([`Goal`]):
//!
//! * the leapfrog positions every input once per candidate key, and there are about as many
//!   candidate keys as the input with the fewest distinct keys has: `k × keys` steps of
//!   [`STAR_STEP_NS`], kept conservative (a badly used leapfrog measured 4.3 µs per output row;
//!   a step measured 35-50 ns in release);
//! * every input is scanned, or, read through one cursor, gallops from key to key
//!   (`min(cost, seek_random + keys × seek_sorted + matched rows)`);
//! * an output row combines k input rows ([`Planner::star_row_cost`]), against one row copy
//!   per output row of the chain, which however also produces the intermediate rows of its
//!   inner joins;
//! * under COUNT(*) the top of the group produces no row when it is factorized: a check-free
//!   StarMerge multiplies its runs per key, a check-free top MergeJoin of the chain sums the
//!   products of its runs but still produces the rows of its inner joins.
//!
//! Measured (release, MemStore, 300 subjects × 20 values of three predicates, COUNT(*)): the
//! chain 2.6 ms with its 120 000 intermediate rows, the StarMerge 0.6 ms. Without the count, the
//! 2.4M output rows make the StarMerge slower (70 ms against 46 ms), as the costs say.
use smallvec::SmallVec;
use sq_core::batch::{Slot, SlotSet};
use sq_core::ir::{GraphSel, PatTerm};
use sq_core::plan::{JoinPolicy, Op, Plan};
use sq_core::store::Col;

use super::join::Goal;
use super::scan::single_cursor;
use super::{clamp_est, mk, Planner};
use crate::props::slot_list;

/// Cost of one leapfrog step (one input positioned on a candidate key).
const STAR_STEP_NS: f64 = 300.0;

/// The inputs of the chain of MergeJoins on `key` along the left spine of `p`, left to right.
fn chain_inputs(p: &Plan, key: Slot) -> SmallVec<[&Plan; 8]> {
    let mut inputs = SmallVec::new();
    let mut q = p;
    while let Op::MergeJoin { left, right, key: k, .. } = &q.op {
        if *k != key {
            break;
        }
        inputs.push(&**right);
        q = left;
    }
    inputs.push(q);
    inputs.reverse();
    inputs
}

/// The union of the checks of the chain of MergeJoins on `key` along the left spine of `p`.
fn chain_check(p: &Plan, key: Slot) -> SlotSet {
    match &p.op {
        Op::MergeJoin { left, key: k, check, .. } if *k == key => {
            chain_check(left, key).union(SlotSet::from_slots(check.iter().copied()))
        }
        _ => SlotSet::EMPTY,
    }
}

/// The inputs of the chain (left to right), consumed.
fn flatten(p: Plan, key: Slot, inputs: &mut Vec<Plan>) {
    match p.op {
        Op::MergeJoin { left, right, key: k, .. } if k == key => {
            flatten(*left, key, inputs);
            inputs.push(*right);
        }
        op => inputs.push(Plan { op, ..p }),
    }
}

impl Planner<'_> {
    /// Cost of producing one StarMerge output row from `k` input rows (one row copy each).
    pub(super) fn star_row_cost(&self, k: usize) -> f64 {
        k as f64 * self.row_copy()
    }

    /// Replace chains of at least three MergeJoins' inputs on one key along the left spine of a
    /// group's plan `p` by a StarMerge where that is cheaper. `bound`: the slots bound when the
    /// group opens. Under `Goal::Count`, the top of `p` is compared as counted. Units planned
    /// as their own groups were already rewritten. `PreferMerge` forces binary MergeJoins
    /// (a fixed shape for differential testing), so only the cost-based policies take it.
    pub(super) fn star_merges(&self, p: Plan, goal: Goal, bound: SlotSet) -> Plan {
        if self.opts.join_policy == JoinPolicy::PreferMerge {
            return p;
        }
        self.star_merges_at(p, goal == Goal::Count, bound).0
    }

    /// [`Self::star_merges`] of a node of the spine (`counted`: the group's top under COUNT(*)),
    /// with the decrease of its estimated cost, which its ancestors on the spine also get.
    fn star_merges_at(&self, mut p: Plan, counted: bool, bound: SlotSet) -> (Plan, f64) {
        if let Op::MergeJoin { key, .. } = p.op
            && chain_inputs(&p, key).len() >= 3
        {
            // The spine continues below the chain's first input (a chain on another key may
            // be there, under an enforcer).
            let below = self.chain_head(&mut p, key, bound);
            let (p, saved) = self.star_merge(p, key, counted, bound);
            return (p, below + saved);
        }
        // Continue down the left spine (the group's own operators).
        let spine = match &mut p.op {
            Op::MergeJoin { left, .. } | Op::BindJoin { left, .. } | Op::NestedLoop { left, .. } => Some(left),
            Op::HashJoin { build, probe, build_is_left, .. } => Some(if *build_is_left { build } else { probe }),
            Op::Filter { input, .. } | Op::SortBySlot { input, .. } => Some(input),
            _ => None,
        };
        let Some(child) = spine else { return (p, 0.0) };
        let c = std::mem::replace(&mut **child, mk(Op::Empty, 0.0, 0.0, None));
        let (c, saved) = self.star_merges_at(c, false, bound);
        **child = c;
        p.est_cost_ns = clamp_est(p.est_cost_ns - saved);
        (p, saved)
    }

    /// Rewrite the first input of the chain of MergeJoins on `key` that `p` heads, and lower the
    /// estimated costs of the chain's joins by what that saved (returned).
    fn chain_head(&self, p: &mut Plan, key: Slot, bound: SlotSet) -> f64 {
        let saved = match &mut p.op {
            Op::MergeJoin { left, key: k, .. } if *k == key => self.chain_head(left, key, bound),
            _ => {
                let head = std::mem::replace(p, mk(Op::Empty, 0.0, 0.0, None));
                let (head, saved) = self.star_merges_at(head, false, bound);
                *p = head;
                return saved;
            }
        };
        p.est_cost_ns = clamp_est(p.est_cost_ns - saved);
        saved
    }

    /// `p` (a MergeJoin chain of at least three inputs on `key`) as a StarMerge when cheaper,
    /// else unchanged; with the decrease of the estimated cost.
    fn star_merge(&self, p: Plan, key: Slot, counted: bool, bound: SlotSet) -> (Plan, f64) {
        let (rows, chain_cost, id) = (p.est_rows, p.est_cost_ns, p.id);
        let (k, star_cost) = {
            let inputs = chain_inputs(&p, key);
            (inputs.len(), self.star_cost(&inputs, key, rows, bound))
        };
        let check = chain_check(&p, key);
        // What each produces in the end: a counted, check-free top produces no row.
        let (chain_rank, star_rank) = if counted {
            let top_free = matches!(&p.op, Op::MergeJoin { check, .. } if check.is_empty());
            let chain = if top_free { chain_cost - rows * self.row_copy() } else { chain_cost };
            let star = if check.is_empty() { star_cost - rows * self.star_row_cost(k) } else { star_cost };
            (chain, star)
        } else {
            (chain_cost, star_cost)
        };
        if star_rank >= chain_rank {
            return (p, 0.0);
        }
        let mut inputs = Vec::with_capacity(k);
        flatten(p, key, &mut inputs);
        let mut star = mk(Op::StarMerge { key, inputs, check: slot_list(check) }, rows, star_cost, Some(key));
        star.id = id;
        // A counted top has no ancestor in the group; elsewhere `star_cost < chain_cost`.
        let saved = if counted { 0.0 } else { chain_cost - star.est_cost_ns };
        (star, saved)
    }

    /// `Σ_i input_i + k × keys × STAR_STEP_NS + out × star_row_cost(k)`, where `keys` is the
    /// smallest number of distinct keys of an input and an input read through one cursor costs
    /// `min(its cost, seek_random + keys × seek_sorted + matched rows)` (see the module docs).
    fn star_cost(&self, inputs: &[&Plan], key: Slot, out_rows: f64, bound: SlotSet) -> f64 {
        let keys: SmallVec<[f64; 8]> = inputs.iter().map(|q| self.input_keys(q, key)).collect();
        let driver = keys.iter().copied().fold(f64::INFINITY, f64::min).max(0.0);
        let mut cost = 0.0;
        for (q, &dk) in inputs.iter().zip(&keys) {
            cost += match &q.op {
                Op::Scan(s) if single_cursor(&s.pat, bound, self.dataset) => {
                    let per_row = (q.est_cost_ns - self.costs.seek_random).max(0.0) / q.est_rows.max(1.0);
                    // Rows of this input under the candidate keys.
                    let matched = q.est_rows * (driver / dk.max(1.0)).min(1.0);
                    q.est_cost_ns.min(self.costs.seek_random + driver * self.costs.seek_sorted + matched * per_row)
                }
                _ => q.est_cost_ns,
            };
        }
        clamp_est(cost + inputs.len() as f64 * driver * STAR_STEP_NS + out_rows * self.star_row_cost(inputs.len()))
    }

    /// Distinct values of `key` in the rows of a StarMerge input (at most its rows): the column
    /// statistics of a scan, the input's rows otherwise.
    fn input_keys(&self, q: &Plan, key: Slot) -> f64 {
        let rows = q.est_rows;
        match &q.op {
            Op::Scan(s) => {
                let pat = &s.pat;
                let col = [(Col::S, pat.s), (Col::P, pat.p), (Col::O, pat.o)]
                    .into_iter()
                    .find_map(|(c, t)| (t == PatTerm::Slot(key)).then_some(c))
                    .or_else(|| (pat.g == GraphSel::Slot(key)).then_some(Col::G));
                match col {
                    Some(c) => self.card.distinct_values(pat, c, rows).min(rows),
                    None => rows,
                }
            }
            Op::SortBySlot { input, .. } => self.input_keys(input, key).min(rows),
            _ => rows,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sq_core::eval_api::TempDict;
    use sq_core::ir::{DatasetSpec, DefaultGraph, NamedGraphs};
    use sq_core::plan::{QueryOptions, ScanOp};
    use sq_core::store::IndexOrder;

    fn scan(s: Slot, o: Slot) -> Plan {
        let pat = sq_core::ir::QuadPat {
            g: GraphSel::Default,
            s: PatTerm::Slot(s),
            p: PatTerm::Const(sq_core::TermId::from_dict(2000 + o as i64, sq_core::id::Tag::Iri)),
            o: PatTerm::Slot(o),
        };
        mk(Op::Scan(ScanOp { pat, order: IndexOrder::Gpso, range: None }), 10.0, 100.0, Some(s))
    }

    fn merge(left: Plan, right: Plan, key: Slot, check: &[Slot]) -> Plan {
        let op = Op::MergeJoin { left: Box::new(left), right: Box::new(right), key, check: check.to_vec(), gallop: 6 };
        mk(op, 10.0, 300.0, Some(key))
    }

    /// A merge join with the estimates the planner gives it: the inputs' costs plus a row copy
    /// per output row (`row_copy` = 8 for the planner below).
    fn costed_merge(left: Plan, right: Plan, key: Slot, rows: f64) -> Plan {
        let cost = left.est_cost_ns + right.est_cost_ns + rows * 8.0;
        Plan { est_rows: rows, est_cost_ns: cost, ..merge(left, right, key, &[]) }
    }

    /// A scan of 1 000 rows sorted by `s` (no statistics: 500 distinct subjects).
    fn big_scan(s: Slot, o: Slot) -> Plan {
        Plan { est_rows: 1_000.0, est_cost_ns: 24_300.0, ..scan(s, o) }
    }

    #[test]
    fn chains_become_star_merges_where_cheaper_down_the_spine() {
        let store = sq_memstore::MemStore::new();
        let opts = QueryOptions::default();
        let ds = DatasetSpec { default: DefaultGraph::StoreDefault, named: NamedGraphs::All };
        let temps = TempDict::default();
        let planner = Planner::new(&store, &opts, &ds, 0, &[], &temps, 8);
        // A star on ?y (1M intermediate rows, 10 results) re-sorted by ?x heads a star on ?x
        // (1M intermediate rows, 10 results).
        let star_y = costed_merge(costed_merge(big_scan(1, 2), big_scan(1, 3), 1, 1e6), big_scan(1, 4), 1, 10.0);
        let (rows, cost) = (star_y.est_rows, star_y.est_cost_ns + 100.0);
        let head = mk(Op::SortBySlot { input: Box::new(star_y), key: 0 }, rows, cost, Some(0));
        let p = costed_merge(costed_merge(head, big_scan(0, 5), 0, 1e6), big_scan(0, 6), 0, 10.0);
        let before = p.est_cost_ns;
        let out = planner.star_merges(p, Goal::Rows, SlotSet::EMPTY);
        let Op::StarMerge { key: 0, inputs, check } = &out.op else { panic!("{out:?}") };
        assert!(check.is_empty() && inputs.len() == 3);
        let Op::SortBySlot { input, .. } = &inputs[0].op else { panic!("{:?}", inputs[0]) };
        assert!(matches!(&input.op, Op::StarMerge { key: 1, inputs, .. } if inputs.len() == 3), "{input:?}");
        // Both leapfrogs cost far less than 2M intermediate row copies.
        assert!(out.est_cost_ns < before - 15e6, "{} -> {}", before, out.est_cost_ns);
        assert_eq!(out.sorted_by, Some(0));
        // Few intermediate rows: the chain stays; PreferMerge keeps binary merge joins.
        let chain = costed_merge(costed_merge(big_scan(0, 1), big_scan(0, 2), 0, 1_000.0), big_scan(0, 3), 0, 1_000.0);
        let kept = planner.star_merges(chain.clone(), Goal::Rows, SlotSet::EMPTY);
        assert!(matches!(kept.op, Op::MergeJoin { .. }) && kept.est_cost_ns == chain.est_cost_ns);
        let forced = QueryOptions { join_policy: JoinPolicy::PreferMerge, ..QueryOptions::default() };
        let planner = Planner::new(&store, &forced, &ds, 0, &[], &temps, 8);
        let p = costed_merge(costed_merge(big_scan(0, 1), big_scan(0, 2), 0, 1e6), big_scan(0, 3), 0, 10.0);
        assert!(matches!(planner.star_merges(p, Goal::Rows, SlotSet::EMPTY).op, Op::MergeJoin { .. }));
    }

    #[test]
    fn chains_are_read_along_the_left_spine() {
        let chain = merge(merge(merge(scan(0, 1), scan(0, 2), 0, &[]), scan(0, 3), 0, &[5]), scan(0, 4), 0, &[6]);
        let objects: Vec<Option<Slot>> = chain_inputs(&chain, 0)
            .iter()
            .map(|q| match &q.op {
                Op::Scan(s) => match s.pat.o {
                    PatTerm::Slot(o) => Some(o),
                    PatTerm::Const(_) => None,
                },
                _ => None,
            })
            .collect();
        assert_eq!(objects, vec![Some(1), Some(2), Some(3), Some(4)]);
        assert_eq!(chain_check(&chain, 0), SlotSet::from_slots([5, 6]));
        // A merge on another key ends the chain.
        let other = merge(merge(scan(0, 1), scan(0, 2), 1, &[]), scan(0, 3), 0, &[]);
        assert_eq!(chain_inputs(&other, 0).len(), 2);
        assert_eq!(chain_check(&other, 0), SlotSet::EMPTY);
        let mut inputs = Vec::new();
        flatten(chain, 0, &mut inputs);
        assert_eq!(inputs.len(), 4);
        assert!(inputs.iter().all(|i| matches!(i.op, Op::Scan(_))));
    }
}
