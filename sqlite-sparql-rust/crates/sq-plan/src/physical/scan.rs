//! Quad pattern -> `Op::Scan`: index order choice, estimates and sortedness.
use sq_core::batch::{Slot, SlotSet};
use sq_core::ir::{DatasetSpec, GraphSel, NamedGraphs, PatTerm, QuadPat};
use sq_core::plan::{Op, Plan, ScanOp};
use sq_core::store::{Col, IndexOrder};
use sq_core::Result;

use super::{mk, Ctx, Planner};

/// Which of [g, s, p, o] are bound when the scan opens: constants, the graph (default graph,
/// constant graph or the GraphLoop register) and slots bound at open time.
pub(crate) fn bound_columns(pat: &QuadPat, bound: SlotSet) -> [bool; 4] {
    let term = |t: PatTerm| match t {
        PatTerm::Const(_) => true,
        PatTerm::Slot(s) => bound.contains(s),
    };
    let g = match pat.g {
        GraphSel::Default | GraphSel::Const(_) | GraphSel::Active => true,
        GraphSel::Slot(s) => bound.contains(s),
    };
    [g, term(pat.s), term(pat.p), term(pat.o)]
}

/// The planned index order: the first of `IndexOrder::candidates(bound)` (index order of
/// `IndexOrder::ALL`, so s-sorted streams are preferred where there is a choice).
pub(crate) fn choose_order(bound: [bool; 4]) -> (IndexOrder, usize) {
    let (order, k) = IndexOrder::candidates(bound)
        .first()
        .copied()
        .expect("every bound-column set has a candidate order");
    (order, k as usize)
}

/// Term of `pat` at column `c`.
pub(crate) fn column_term(pat: &QuadPat, c: Col) -> Option<PatTerm> {
    match c {
        Col::G => match pat.g {
            GraphSel::Slot(s) => Some(PatTerm::Slot(s)),
            GraphSel::Const(g) => Some(PatTerm::Const(g)),
            GraphSel::Default | GraphSel::Active => None,
        },
        Col::S => Some(pat.s),
        Col::P => Some(pat.p),
        Col::O => Some(pat.o),
    }
}

/// Slot the scan output is sorted by, if guaranteed: the first free column of the order, when
/// the graph is read as one sorted stream (single graph, or a merged multi-graph default graph,
/// or the named-only partial index with every named graph).
pub(crate) fn scan_sorted_by(pat: &QuadPat, order: IndexOrder, k: usize, dataset: &DatasetSpec) -> Option<Slot> {
    if let GraphSel::Slot(_) = pat.g {
        if order.named_only() && !matches!(dataset.named, NamedGraphs::All) {
            return None;
        }
    }
    let col = *order.cols().get(k)?;
    match column_term(pat, col)? {
        PatTerm::Slot(s) => Some(s),
        PatTerm::Const(_) => None,
    }
}

impl Planner<'_> {
    /// Plan a quad pattern unit (memoized per pattern and context).
    pub(crate) fn plan_scan(&mut self, pat: &QuadPat, ctx: Ctx) -> Result<Plan> {
        let slots = SlotSet::from_slots(crate::props::pattern_slots(pat));
        let ctx = Ctx { seeded: ctx.seeded.intersect(slots), subst: ctx.subst.intersect(slots), ..ctx };
        if let Some(p) = self.scan_memo.get(&(*pat, ctx)) {
            return Ok(p.clone());
        }
        let p = self.plan_scan_uncached(pat, ctx)?;
        self.scan_memo.insert((*pat, ctx), p.clone());
        Ok(p)
    }

    fn plan_scan_uncached(&mut self, pat: &QuadPat, ctx: Ctx) -> Result<Plan> {
        let bound = ctx.bound();
        let (order, k) = choose_order(bound_columns(pat, bound));
        let (rows, cost) = self.scan_estimate(pat, bound)?;
        let sorted = if ctx.in_exists { None } else { scan_sorted_by(pat, order, k, self.dataset) };
        Ok(mk(Op::Scan(ScanOp { pat: *pat, order, range: None }), rows, cost, sorted))
    }

    /// Rows and cost of one open of `pat` with `bound` slots bound: one seek, then the rows
    /// with their free columns.
    pub(crate) fn scan_estimate(&mut self, pat: &QuadPat, bound: SlotSet) -> Result<(f64, f64)> {
        // Every candidate order has the bound columns first (`choose_order`).
        let k = bound_columns(pat, bound).iter().filter(|b| **b).count();
        let rows = self.card.pattern_rows(pat, bound)?;
        let free = (4 - k) as f64;
        Ok((rows, self.costs.seek_random + rows * (self.costs.scan_row + free * self.costs.scan_col)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sq_core::id::Tag;
    use sq_core::ir::DefaultGraph;
    use sq_core::TermId;

    fn c() -> PatTerm {
        PatTerm::Const(TermId::from_dict(2000, Tag::Iri))
    }

    #[test]
    fn order_choice() {
        let p = QuadPat { g: GraphSel::Default, s: PatTerm::Slot(0), p: c(), o: PatTerm::Slot(1) };
        let cols = bound_columns(&p, SlotSet::EMPTY);
        assert_eq!(choose_order(cols), (IndexOrder::Gpso, 2));
        assert_eq!(choose_order(bound_columns(&p, SlotSet::from_slots([1]))), (IndexOrder::Gpos, 3));
        assert_eq!(choose_order(bound_columns(&p, SlotSet::from_slots([0]))), (IndexOrder::Gspo, 3));
        let named = QuadPat { g: GraphSel::Slot(5), ..p };
        assert_eq!(choose_order(bound_columns(&named, SlotSet::EMPTY)), (IndexOrder::Psog, 1));
        assert_eq!(choose_order(bound_columns(&named, SlotSet::from_slots([5]))), (IndexOrder::Gpso, 2));
    }

    #[test]
    fn sortedness() {
        let ds = DatasetSpec { default: DefaultGraph::StoreDefault, named: NamedGraphs::All };
        let p = QuadPat { g: GraphSel::Default, s: PatTerm::Slot(0), p: c(), o: PatTerm::Slot(1) };
        assert_eq!(scan_sorted_by(&p, IndexOrder::Gpso, 2, &ds), Some(0));
        assert_eq!(scan_sorted_by(&p, IndexOrder::Gpos, 2, &ds), Some(1));
        let all_bound = QuadPat { s: c(), o: c(), ..p };
        assert_eq!(scan_sorted_by(&all_bound, IndexOrder::Gspo, 4, &ds), None);
        let named = QuadPat { g: GraphSel::Slot(5), ..p };
        assert_eq!(scan_sorted_by(&named, IndexOrder::Psog, 1, &ds), Some(0));
        let only = DatasetSpec { named: NamedGraphs::Only(vec![TermId::from_dict(9, Tag::Iri)].into()), ..ds };
        assert_eq!(scan_sorted_by(&named, IndexOrder::Psog, 1, &only), None);
    }
}
