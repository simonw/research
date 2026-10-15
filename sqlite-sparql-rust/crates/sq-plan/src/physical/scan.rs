//! Quad pattern -> `Op::Scan`: access paths (index order, optional IntRange), estimates and
//! the order the output is sorted by.
use smallvec::SmallVec;
use sq_core::batch::{Slot, SlotSet};
use sq_core::ir::{DatasetSpec, DefaultGraph, GraphSel, NamedGraphs, PatTerm, QuadPat};
use sq_core::plan::{IntRange, Op, Plan, ScanOp};
use sq_core::store::{Col, IndexOrder};
use sq_core::{Result, TermId};

use super::{mk, Ctx, Planner};

/// How a quad pattern is read: an index order and, for IntRange scans, the integer range of
/// the object (spec §7.3.7; then `order` is `Gpos` with g and p bound).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) struct Access {
    pub order: IndexOrder,
    pub range: Option<IntRange>,
}

impl Access {
    pub(crate) fn plain(order: IndexOrder) -> Access {
        Access { order, range: None }
    }
}

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

/// The default index order for the bound columns: the first of `IndexOrder::candidates`
/// (s-sorted streams are preferred where there is a choice), except that point probes with g,
/// s and p bound read GPSO: it clusters each predicate's entries, measured ~1.4 µs per random
/// probe against ~2.1 µs for GSPO, with the same output order.
pub(crate) fn choose_order(bound: [bool; 4]) -> (IndexOrder, usize) {
    let cands = IndexOrder::candidates(bound);
    let k = bound.iter().filter(|b| **b).count();
    if k >= 3 && bound[0] && cands.iter().any(|c| c.0 == IndexOrder::Gpso) {
        return (IndexOrder::Gpso, k);
    }
    let (order, k) = cands.first().copied().expect("every bound-column set has a candidate order");
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

/// Slot the scan output is sorted by, if the executor guarantees it: the first free column of
/// the order, when the graph is read as one sorted stream (a single graph, a merged multi-graph
/// default graph, or the named-only partial index over every named graph). IntRange scans read
/// two ranges one after the other and are never sorted; FROM NAMED lists may be read graph by
/// graph and are conservatively unsorted.
pub(crate) fn scan_sorted_by(
    pat: &QuadPat,
    order: IndexOrder,
    k: usize,
    range: bool,
    dataset: &DatasetSpec,
) -> Option<Slot> {
    if range {
        return None;
    }
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

/// The one graph the default graph of `dataset` is, when it is a single stored graph (the
/// store's default graph, or one stored FROM graph): `Some(0)` for the store default graph.
pub(crate) fn fixed_default_graph(dataset: &DatasetSpec) -> Option<TermId> {
    match &dataset.default {
        DefaultGraph::StoreDefault => Some(TermId::DEFAULT_GRAPH),
        DefaultGraph::UnionAll => None,
        DefaultGraph::Graphs(list) => {
            let mut stored = list.iter().copied().filter(|g| g.is_storable());
            let first = stored.next()?;
            stored.all(|g| g == first).then_some(first)
        }
    }
}

/// The graph id a pattern reads when it is one fixed, stored graph known at plan time (SQL
/// chains and graph-specific counts need it). The GraphLoop register is fixed per open but
/// unknown here.
pub(crate) fn fixed_graph_id(pat: &QuadPat, dataset: &DatasetSpec) -> Option<TermId> {
    match pat.g {
        GraphSel::Default => fixed_default_graph(dataset),
        GraphSel::Const(g) => g.is_storable().then_some(g),
        GraphSel::Active | GraphSel::Slot(_) => None,
    }
}

/// Does the executor read `pat` as one g-first cursor of a single graph (where IntRange scans
/// apply)? The default graph when it is one stored graph, a constant graph, or the GraphLoop
/// register.
pub(crate) fn single_fixed_graph(pat: &QuadPat, dataset: &DatasetSpec) -> bool {
    pat.g == GraphSel::Active || fixed_graph_id(pat, dataset).is_some()
}

/// Can `pat` be one alias of a SqlJoin statement (spec §7.5 "sqlable")? Its graph is one fixed,
/// stored graph and every constant is a stored id (a TEMP constant matches nothing).
pub(crate) fn sqlable(pat: &QuadPat, dataset: &DatasetSpec) -> bool {
    let stored = |t: PatTerm| match t {
        PatTerm::Const(c) => c.is_storable(),
        PatTerm::Slot(_) => true,
    };
    fixed_graph_id(pat, dataset).is_some() && stored(pat.s) && stored(pat.p) && stored(pat.o)
}

/// Does `pat` repeat a slot (`?x :p ?x`, `GRAPH ?g { ?g ?p ?o }`)? The scan then post-checks
/// rows, so a COUNT cannot run in the VDBE.
pub(crate) fn repeats_slot(pat: &QuadPat) -> bool {
    let slots: SmallVec<[Slot; 4]> = crate::props::pattern_slots(pat).collect();
    slots.iter().enumerate().any(|(i, s)| slots[..i].contains(s))
}

/// The IntRange access applicable to `pat` with `bound` slots bound, given the group's range on
/// its object slot: a fixed graph, a constant predicate, subject and object free (Gpos with
/// exactly g and p as its prefix).
pub(crate) fn range_access(
    pat: &QuadPat,
    bound: SlotSet,
    range: Option<IntRange>,
    dataset: &DatasetSpec,
) -> Option<Access> {
    let range = range?;
    let ok = matches!(pat.o, PatTerm::Slot(_))
        && matches!(pat.p, PatTerm::Const(c) if c.is_storable())
        && bound_columns(pat, bound) == [true, false, true, false]
        && single_fixed_graph(pat, dataset);
    ok.then_some(Access { order: IndexOrder::Gpos, range: Some(range) })
}

/// The plain access paths of `pat` with `bound` slots bound, one per distinct output order
/// (the default order first), with the slot each is sorted by.
pub(crate) fn sorted_accesses(
    pat: &QuadPat,
    bound: SlotSet,
    dataset: &DatasetSpec,
) -> SmallVec<[(Access, Option<Slot>); 4]> {
    let cols = bound_columns(pat, bound);
    let (first, k) = choose_order(cols);
    let mut out: SmallVec<[(Access, Option<Slot>); 4]> = SmallVec::new();
    out.push((Access::plain(first), scan_sorted_by(pat, first, k, false, dataset)));
    for (order, k) in IndexOrder::candidates(cols) {
        let sorted = scan_sorted_by(pat, order, k as usize, false, dataset);
        if !out.iter().any(|(_, s)| *s == sorted) {
            out.push((Access::plain(order), sorted));
        }
    }
    out
}

impl Planner<'_> {
    /// Plan a quad pattern unit read through `access` (None: the default order for the bound
    /// columns). Memoized per pattern, context and access path.
    pub(crate) fn plan_scan_with(&mut self, pat: &QuadPat, ctx: Ctx, access: Option<Access>) -> Result<Plan> {
        let slots = SlotSet::from_slots(crate::props::pattern_slots(pat));
        let ctx = Ctx { seeded: ctx.seeded.intersect(slots), subst: ctx.subst.intersect(slots), ..ctx };
        if let Some(p) = self.scan_memo.get(&(*pat, ctx, access)) {
            return Ok(p.clone());
        }
        let p = self.plan_scan_uncached(pat, ctx, access)?;
        self.scan_memo.insert((*pat, ctx, access), p.clone());
        Ok(p)
    }

    fn plan_scan_uncached(&mut self, pat: &QuadPat, ctx: Ctx, access: Option<Access>) -> Result<Plan> {
        let bound = ctx.bound();
        let cols = bound_columns(pat, bound);
        let k = cols.iter().filter(|b| **b).count();
        let access = access.unwrap_or_else(|| Access::plain(choose_order(cols).0));
        let (rows, cost) = match access.range {
            Some(r) => self.range_scan_estimate(pat, r)?,
            None => self.scan_estimate(pat, bound)?,
        };
        let ranged = access.range.is_some();
        let sorted = if ctx.in_exists { None } else { scan_sorted_by(pat, access.order, k, ranged, self.dataset) };
        let op = Op::Scan(ScanOp { pat: *pat, order: access.order, range: access.range });
        Ok(mk(op, rows, cost, sorted))
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

    /// Rows and cost of an IntRange scan of `pat` (g and p bound): the integers of the range,
    /// uniform over the predicate's [imin, imax] (1/3 without statistics), plus the numeric
    /// exceptions of (g, p); two seeks.
    pub(crate) fn range_scan_estimate(&mut self, pat: &QuadPat, range: IntRange) -> Result<(f64, f64)> {
        let all = self.card.pattern_rows(pat, SlotSet::EMPTY)?;
        let frac = self.card.int_range_fraction(pat, range).unwrap_or(1.0 / 3.0);
        let exceptions = match (fixed_graph_id(pat, self.dataset), pat.p) {
            (Some(g), PatTerm::Const(p)) => self.card.numeric_exceptions(g, p)?,
            _ => 0.0,
        };
        let rows = (all * frac + exceptions).min(all);
        let per_row = self.costs.scan_row + 2.0 * self.costs.scan_col;
        Ok((rows, 2.0 * self.costs.seek_random + rows * per_row))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sq_core::id::Tag;
    use sq_core::TermId;

    fn c() -> PatTerm {
        PatTerm::Const(TermId::from_dict(2000, Tag::Iri))
    }

    fn ds() -> DatasetSpec {
        DatasetSpec { default: DefaultGraph::StoreDefault, named: NamedGraphs::All }
    }

    #[test]
    fn order_choice() {
        let p = QuadPat { g: GraphSel::Default, s: PatTerm::Slot(0), p: c(), o: PatTerm::Slot(1) };
        let cols = bound_columns(&p, SlotSet::EMPTY);
        assert_eq!(choose_order(cols), (IndexOrder::Gpso, 2));
        assert_eq!(choose_order(bound_columns(&p, SlotSet::from_slots([1]))), (IndexOrder::Gpos, 3));
        // (g, s, p) probes read GPSO, as do fully bound ones.
        assert_eq!(choose_order(bound_columns(&p, SlotSet::from_slots([0]))), (IndexOrder::Gpso, 3));
        assert_eq!(choose_order(bound_columns(&p, SlotSet::from_slots([0, 1]))), (IndexOrder::Gpso, 4));
        let all_free = QuadPat { p: PatTerm::Slot(2), ..p };
        assert_eq!(choose_order(bound_columns(&all_free, SlotSet::EMPTY)), (IndexOrder::Gspo, 1));
        let named = QuadPat { g: GraphSel::Slot(5), ..p };
        assert_eq!(choose_order(bound_columns(&named, SlotSet::EMPTY)), (IndexOrder::Psog, 1));
        assert_eq!(choose_order(bound_columns(&named, SlotSet::from_slots([5]))), (IndexOrder::Gpso, 2));
        assert_eq!(choose_order(bound_columns(&named, SlotSet::from_slots([0]))), (IndexOrder::Spog, 2));
    }

    #[test]
    fn sortedness() {
        let ds = ds();
        let p = QuadPat { g: GraphSel::Default, s: PatTerm::Slot(0), p: c(), o: PatTerm::Slot(1) };
        assert_eq!(scan_sorted_by(&p, IndexOrder::Gpso, 2, false, &ds), Some(0));
        assert_eq!(scan_sorted_by(&p, IndexOrder::Gpos, 2, false, &ds), Some(1));
        assert_eq!(scan_sorted_by(&p, IndexOrder::Gpos, 2, true, &ds), None, "IntRange scans are unsorted");
        let all_bound = QuadPat { s: c(), o: c(), ..p };
        assert_eq!(scan_sorted_by(&all_bound, IndexOrder::Gspo, 4, false, &ds), None);
        let named = QuadPat { g: GraphSel::Slot(5), ..p };
        assert_eq!(scan_sorted_by(&named, IndexOrder::Psog, 1, false, &ds), Some(0));
        let only = DatasetSpec { named: NamedGraphs::Only(vec![TermId::from_dict(9, Tag::Iri)].into()), ..ds.clone() };
        assert_eq!(scan_sorted_by(&named, IndexOrder::Psog, 1, false, &only), None);
        // One access path per output order, the default first.
        let acc = sorted_accesses(&p, SlotSet::EMPTY, &ds);
        let got: Vec<(IndexOrder, Option<Slot>)> = acc.iter().map(|(a, s)| (a.order, *s)).collect();
        assert_eq!(got, vec![(IndexOrder::Gpso, Some(0)), (IndexOrder::Gpos, Some(1))]);
    }

    #[test]
    fn graphs_and_ranges() {
        let g1 = TermId::from_dict(9, Tag::Iri);
        let temp = TermId::temp(0, Tag::Iri);
        let from = |l: Vec<TermId>| DatasetSpec { default: DefaultGraph::Graphs(l.into()), named: NamedGraphs::All };
        assert_eq!(fixed_default_graph(&ds()), Some(TermId::DEFAULT_GRAPH));
        assert_eq!(fixed_default_graph(&from(vec![g1, temp])), Some(g1));
        assert_eq!(fixed_default_graph(&from(vec![g1, TermId::from_dict(10, Tag::Iri)])), None);
        assert_eq!(fixed_default_graph(&from(vec![])), None);
        let p = QuadPat { g: GraphSel::Default, s: PatTerm::Slot(0), p: c(), o: PatTerm::Slot(1) };
        let r = Some(IntRange { lo: 1, hi: 5 });
        assert!(range_access(&p, SlotSet::EMPTY, r, &ds()).is_some());
        assert!(range_access(&p, SlotSet::from_slots([0]), r, &ds()).is_none(), "subject bound");
        assert!(range_access(&p, SlotSet::EMPTY, None, &ds()).is_none());
        let union = DatasetSpec { default: DefaultGraph::UnionAll, named: NamedGraphs::All };
        assert!(range_access(&p, SlotSet::EMPTY, r, &union).is_none(), "merged graphs");
        let active = QuadPat { g: GraphSel::Active, ..p };
        assert!(range_access(&active, SlotSet::EMPTY, r, &union).is_some());
        assert!(sqlable(&p, &ds()) && !sqlable(&active, &ds()));
        assert!(!sqlable(&QuadPat { o: PatTerm::Const(temp), ..p }, &ds()));
        assert!(repeats_slot(&QuadPat { o: PatTerm::Slot(0), ..p }) && !repeats_slot(&p));
    }
}
