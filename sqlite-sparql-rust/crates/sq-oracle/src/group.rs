//! GROUP BY and aggregates (SPARQL 1.1 §18.5.1 Group / Aggregation, docs/SPEC.md §6.7).
//!
//! The input is partitioned by the values of the key slots (unbound is a value of its own),
//! groups in order of first appearance. Without keys the whole input is one group, even when
//! it is empty: `SELECT (COUNT(*) AS ?c) {}` over no solutions gives one row with 0. With keys
//! and no input there are no groups. Each aggregate is computed by sq-expr's accumulators;
//! an aggregate whose value is an error leaves its slot unbound but the group row is kept.
//!
//! `GROUP BY (expr)` arrives as an `Extend` binding a key slot, which leaves the slot unbound
//! when `expr` is an error. An error key and an unbound key therefore share a group, as
//! §18.5.1 requires: Group compares ListEval results, and ListEval gives `(error)` both for an
//! expression whose evaluation is an error and for an unbound one ("the evaluation of an
//! unbound expression is an error"). A separate group for errors would deviate from it.
use rustc_hash::FxHashMap;
use sq_core::ir::{AggFunc, AggSpec, Logical, VarOrigin};
use sq_core::{Result, Slot, TermId};

use crate::bag::Bag;
use crate::dataset::ActiveGraph;
use crate::eval::{Ctx, Evaluator};
use crate::ir::in_scope;

impl<'q> Evaluator<'q> {
    pub(crate) fn group(
        &mut self,
        input: &'q Logical,
        keys: &[Slot],
        aggs: &'q [(Slot, AggSpec)],
        cx: Ctx<'_>,
    ) -> Result<Bag> {
        let rows = self.eval(input, cx)?;
        let mut groups: Vec<(Vec<TermId>, Vec<usize>)> = Vec::new();
        if keys.is_empty() {
            groups.push((Vec::new(), (0..rows.len()).collect()));
        } else {
            let mut index: FxHashMap<Vec<TermId>, usize> = FxHashMap::default();
            for (i, row) in rows.rows().enumerate() {
                let key: Vec<TermId> = keys.iter().map(|k| row[*k as usize]).collect();
                let g = match index.get(&key) {
                    Some(&g) => g,
                    None => {
                        index.insert(key.clone(), groups.len());
                        groups.push((key, Vec::new()));
                        groups.len() - 1
                    }
                };
                groups[g].1.push(i);
            }
        }
        let needs_visible = aggs.iter().any(|(_, a)| a.func == AggFunc::CountAll && a.distinct);
        let visible = if needs_visible { self.visible_slots(input) } else { Vec::new() };

        let mut out = Bag::new(self.width);
        for (key, members) in groups {
            // Seeded slots stay bound, like the constants they stand for.
            let mut row = cx.seed.to_vec();
            for (&k, &v) in keys.iter().zip(&key) {
                if v.is_bound() {
                    row[k as usize] = v;
                }
            }
            for (slot, spec) in aggs {
                if let Some(v) = self.aggregate(spec, &rows, &members, &visible, cx.active)? {
                    row[*slot as usize] = v;
                }
            }
            out.push(&row);
        }
        Ok(out)
    }

    /// One aggregate over the members of a group. COUNT(*) sees whole solutions; every other
    /// aggregate sees its argument's canonical value, or None for an unbound / error value.
    fn aggregate(
        &mut self,
        spec: &'q AggSpec,
        rows: &Bag,
        members: &[usize],
        visible: &[Slot],
        active: ActiveGraph,
    ) -> Result<Option<TermId>> {
        let mut acc = sq_expr::accumulator(spec);
        for &i in members {
            let row = rows.row(i);
            match (&spec.func, &spec.arg) {
                (AggFunc::CountAll, _) => self.with_cx(|cx| acc.add_row(row, visible, cx))?,
                (_, Some(arg)) => {
                    let v = self.value(arg, row, active)?;
                    self.with_cx(|cx| acc.add(v, cx))?;
                }
                (_, None) => self.with_cx(|cx| acc.add(None, cx))?,
            }
        }
        self.with_cx(|cx| acc.finish(cx))
    }

    /// The variables `COUNT(DISTINCT *)` compares: the in-scope variables of the group input
    /// that are part of a SPARQL solution (not blank-node pattern variables), sorted.
    fn visible_slots(&self, input: &Logical) -> Vec<Slot> {
        in_scope(input)
            .iter()
            .filter(|s| self.q.vars.vars.get(*s as usize).is_some_and(|(_, origin)| *origin != VarOrigin::BlankNode))
            .collect()
    }
}
