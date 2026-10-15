//! Cardinality estimation (spec §7.4). Statistics and counts are HINTS: nothing here affects
//! correctness, only join order and operator choice.
//!
//! A quad pattern is first estimated standalone (constants only): a runtime bounded count
//! (`store.count(spec, Some(cap))`, at most `max_probe_counts` per query) when the pattern has
//! a constant besides the predicate (or when there are no statistics at all), otherwise
//! heavy hitters / per-predicate statistics, otherwise fixed defaults. Slots bound at open time
//! (BindJoin seeds) then divide the estimate by the number of distinct values of their column.
use std::sync::Arc;

use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use sq_core::batch::SlotSet;
use sq_core::ir::{DatasetSpec, DefaultGraph, Expr, GraphSel, NamedGraphs, PatTerm, PathPlan, QuadPat};
use sq_core::plan::QueryOptions;
use sq_core::stats::{ScopeStats, Stats};
use sq_core::store::{Col, IndexOrder, QuadStore, ScanSpec};
use sq_core::{Result, TermId};
use spargebra::algebra::Function;

/// Without statistics bounded counts use this cap and extrapolate 10x beyond it.
const NO_STATS_CAP: u64 = 10_000;

/// One graph a pattern reads: a fixed graph id (0 = store default graph) or "every named
/// graph" (g free, partial g-last indexes).
#[derive(Copy, Clone, PartialEq, Debug)]
enum GraphPart {
    Fixed(TermId),
    AllNamed,
}

pub(crate) struct Card<'a> {
    store: &'a dyn QuadStore,
    stats: Option<Arc<Stats>>,
    dataset: &'a DatasetSpec,
    named_count: f64,
    probes_left: u32,
    cap: u64,
    counts: FxHashMap<ScanSpec, u64>,
    /// Pattern estimates per (pattern, bound slots of the pattern).
    rows: FxHashMap<(QuadPat, SlotSet), f64>,
}

impl<'a> Card<'a> {
    pub(crate) fn new(store: &'a dyn QuadStore, dataset: &'a DatasetSpec, named_count: usize, opts: &QueryOptions) -> Self {
        let stats = store.stats();
        let cap = if stats.is_some() { opts.probe_count_cap } else { opts.probe_count_cap.max(NO_STATS_CAP) };
        Card {
            store,
            stats,
            dataset,
            named_count: named_count.max(1) as f64,
            probes_left: opts.max_probe_counts,
            cap,
            counts: FxHashMap::default(),
            rows: FxHashMap::default(),
        }
    }

    pub(crate) fn named_count(&self) -> f64 {
        self.named_count
    }

    fn named_parts(&self) -> Vec<GraphPart> {
        match &self.dataset.named {
            NamedGraphs::All => vec![GraphPart::AllNamed],
            NamedGraphs::Only(list) => list.iter().map(|g| GraphPart::Fixed(*g)).collect(),
        }
    }

    /// Graphs read by a pattern with graph selector `g` and the per-part scale factor.
    fn graph_parts(&self, g: GraphSel, bound: SlotSet) -> Vec<(GraphPart, f64)> {
        let per_graph = 1.0 / self.named_count;
        match g {
            GraphSel::Default => match &self.dataset.default {
                DefaultGraph::StoreDefault => vec![(GraphPart::Fixed(TermId::DEFAULT_GRAPH), 1.0)],
                DefaultGraph::UnionAll => {
                    vec![(GraphPart::Fixed(TermId::DEFAULT_GRAPH), 1.0), (GraphPart::AllNamed, 1.0)]
                }
                DefaultGraph::Graphs(list) => list.iter().map(|g| (GraphPart::Fixed(*g), 1.0)).collect(),
            },
            GraphSel::Const(c) => vec![(GraphPart::Fixed(c), 1.0)],
            GraphSel::Active => self.named_parts().into_iter().map(|p| (p, per_graph)).collect(),
            GraphSel::Slot(s) => {
                let scale = if bound.contains(s) { per_graph } else { 1.0 };
                self.named_parts().into_iter().map(|p| (p, scale)).collect()
            }
        }
    }

    /// Estimated rows of one open of `pat` when the slots in `bound` are already bound.
    pub(crate) fn pattern_rows(&mut self, pat: &QuadPat, bound: SlotSet) -> Result<f64> {
        let key = (*pat, bound.intersect(SlotSet::from_slots(crate::props::pattern_slots(pat))));
        if let Some(r) = self.rows.get(&key) {
            return Ok(*r);
        }
        let r = self.pattern_rows_uncached(pat, key.1)?;
        self.rows.insert(key, r);
        Ok(r)
    }

    fn pattern_rows_uncached(&mut self, pat: &QuadPat, bound: SlotSet) -> Result<f64> {
        let consts = [pat.s, pat.p, pat.o].map(|t| match t {
            PatTerm::Const(c) => Some(c),
            PatTerm::Slot(_) => None,
        });
        if consts.iter().flatten().any(|c| c.is_temp()) {
            return Ok(0.0); // a constant absent from the store matches nothing
        }
        let mut total = 0.0;
        for (part, scale) in self.graph_parts(pat.g, bound) {
            if let GraphPart::Fixed(g) = part {
                if g.is_temp() {
                    continue;
                }
            }
            let c0 = self.standalone(part, consts)?;
            total += c0 * scale;
        }
        // Per-probe reduction for slots bound at open time.
        let mut seen = SlotSet::EMPTY;
        for (col, t) in [(Col::S, pat.s), (Col::P, pat.p), (Col::O, pat.o)] {
            let PatTerm::Slot(s) = t else { continue };
            if bound.contains(s) || seen.contains(s) {
                total /= self.distinct_values(pat, col, total);
            }
            seen.insert(s);
        }
        Ok(total.max(0.0))
    }

    /// Standalone estimate of one graph part with constants `consts` = [s, p, o].
    fn standalone(&mut self, part: GraphPart, consts: [Option<TermId>; 3]) -> Result<f64> {
        let [s, p, o] = consts;
        let wants_count = s.is_some() || o.is_some() || self.stats.is_none();
        if wants_count {
            if let Some(n) = self.bounded_count(part, consts)? {
                let exceeded = n > self.cap;
                if !exceeded {
                    return Ok(n as f64);
                }
                return Ok(match self.stats_estimate(part, consts) {
                    Some(est) => est.max((self.cap + 1) as f64),
                    None => 10.0 * self.cap as f64,
                });
            }
        }
        if let Some(est) = self.stats_estimate(part, consts) {
            return Ok(est);
        }
        Ok(default_rows(s.is_some(), p.is_some(), o.is_some()))
    }

    /// `store.count(spec, Some(cap))` within the per-query budget (cached per spec).
    fn bounded_count(&mut self, part: GraphPart, [s, p, o]: [Option<TermId>; 3]) -> Result<Option<u64>> {
        let mut bound = [false; 4];
        let mut vals = [TermId::UNBOUND; 4];
        if let GraphPart::Fixed(g) = part {
            bound[0] = true;
            vals[0] = g;
        }
        for (i, v) in [(1, s), (2, p), (3, o)] {
            if let Some(v) = v {
                bound[i] = true;
                vals[i] = v;
            }
        }
        let Some((order, k)) = IndexOrder::candidates(bound).first().copied() else { return Ok(None) };
        let prefix: Vec<TermId> = order.cols()[..k as usize].iter().map(|c| vals[*c as usize]).collect();
        let spec = ScanSpec::new(order, &prefix);
        if let Some(n) = self.counts.get(&spec) {
            return Ok(Some(*n));
        }
        if self.probes_left == 0 {
            return Ok(None);
        }
        self.probes_left -= 1;
        let n = self.store.count(&spec, Some(self.cap))?;
        self.counts.insert(spec, n);
        Ok(Some(n))
    }

    fn scope(&self, part: GraphPart) -> Option<(&ScopeStats, f64)> {
        let stats = self.stats.as_ref()?;
        Some(match part {
            GraphPart::Fixed(g) if g == TermId::DEFAULT_GRAPH => (&stats.scope[0], 1.0),
            GraphPart::Fixed(_) => (&stats.scope[1], 1.0 / self.named_count),
            GraphPart::AllNamed => (&stats.scope[1], 1.0),
        })
    }

    /// Heavy hitters and per-predicate statistics (§7.4.2-3).
    fn stats_estimate(&self, part: GraphPart, [s, p, o]: [Option<TermId>; 3]) -> Option<f64> {
        let (sc, scale) = self.scope(part)?;
        let est = match p {
            Some(p) => {
                let Some(ps) = sc.pred.get(&p) else { return Some(1.0 * scale) };
                let (n, ds, dobj) = (ps.n as f64, ps.ds.max(1) as f64, ps.dobj.max(1) as f64);
                match (s.is_some(), o) {
                    (false, None) => n,
                    (false, Some(o)) => sc.po.get(&(p, o)).map_or(n / dobj, |c| *c as f64),
                    (true, None) => n / ds,
                    (true, Some(_)) => n / ds / dobj,
                }
            }
            None => {
                let (n, ds, dobj) = (sc.n as f64, sc.ds.max(1) as f64, sc.dobj.max(1) as f64);
                match (s.is_some(), o.is_some()) {
                    (false, false) => n,
                    (false, true) => n / dobj,
                    (true, false) => n / ds,
                    (true, true) => n / ds / dobj,
                }
            }
        };
        Some(est * scale)
    }

    /// Distinct values of column `col` among the rows of `pat` (≥ 1), used to turn a
    /// standalone estimate into a per-probe one.
    fn distinct_values(&self, pat: &QuadPat, col: Col, rows: f64) -> f64 {
        let part = match pat.g {
            GraphSel::Default if self.dataset.default == DefaultGraph::StoreDefault => {
                GraphPart::Fixed(TermId::DEFAULT_GRAPH)
            }
            _ => GraphPart::AllNamed,
        };
        let pred = match pat.p {
            PatTerm::Const(p) => Some(p),
            PatTerm::Slot(_) => None,
        };
        if let Some((sc, _)) = self.scope(part) {
            let ps = pred.and_then(|p| sc.pred.get(&p));
            let dv = match (col, ps) {
                (Col::S, Some(ps)) => ps.ds,
                (Col::O, Some(ps)) => ps.dobj,
                (Col::S, None) => sc.ds,
                (Col::O, None) => sc.dobj,
                (Col::P, _) => sc.dp,
                (Col::G, _) => self.named_count as u64,
            };
            return (dv as f64).max(1.0);
        }
        let dv = match col {
            Col::S => rows / 2.0,
            Col::O => rows / 4.0,
            Col::P => rows.min(50.0),
            Col::G => self.named_count,
        };
        dv.max(1.0)
    }

    /// Characteristic sets (§7.4.4): expected rows per left row when quad pattern `next`
    /// (subject slot, constant predicate) extends the subject star formed by the `joined`
    /// patterns with the same subject slot, graph and a constant predicate (default graph
    /// only). None when no characteristic sets apply.
    pub(crate) fn star_ratio(&self, joined: &[QuadPat], next: &QuadPat) -> Option<f64> {
        if !matches!((next.s, next.p), (PatTerm::Slot(_), PatTerm::Const(_)))
            || next.g != GraphSel::Default
            || self.dataset.default != DefaultGraph::StoreDefault
        {
            return None;
        }
        let in_star = |p: &&QuadPat| p.s == next.s && p.g == next.g && matches!(p.p, PatTerm::Const(_));
        if !joined.iter().any(|p| in_star(&p)) {
            return None;
        }
        let sc = &self.stats.as_ref()?.scope[0];
        if sc.cs.is_empty() {
            return None;
        }
        let before = star_estimate(sc, joined.iter().filter(in_star))?;
        let after = star_estimate(sc, joined.iter().filter(in_star).chain(std::iter::once(next)))?;
        (before > 0.0).then(|| after / before)
    }

    /// Estimated rows of a property path pattern (§5.5); `s_bound` / `o_bound` include
    /// constants and seeded slots.
    pub(crate) fn path_rows(&self, path: &PathPlan, s_bound: bool, o_bound: bool) -> f64 {
        let base = self.path_base(path).max(1.0);
        match (s_bound, o_bound) {
            (false, false) => base,
            (true, false) | (false, true) => (base / 100.0).max(3.0),
            (true, true) => 1.0,
        }
    }

    fn path_base(&self, path: &PathPlan) -> f64 {
        let sc = self.stats.as_ref().map(|s| &s.scope[0]);
        match path {
            PathPlan::Pred(p) => match sc {
                Some(sc) => sc.pred.get(p).map_or(1.0, |ps| ps.n as f64),
                None => default_rows(false, true, false),
            },
            PathPlan::Nps(_) => sc.map_or(default_rows(false, false, false), |sc| sc.n as f64),
            PathPlan::Inv(p) => self.path_base(p),
            PathPlan::Seq(a, b) => self.path_base(a).max(self.path_base(b)),
            PathPlan::Alt(a, b) => self.path_base(a) + self.path_base(b),
            PathPlan::OneOrMore(p) => 2.0 * self.path_base(p),
            PathPlan::ZeroOrMore(p) | PathPlan::ZeroOrOne(p) => {
                let nodes = sc.map_or(1_000.0, |sc| (sc.ds + sc.dobj) as f64);
                2.0 * self.path_base(p) + nodes
            }
        }
    }
}

/// Σ_{cs ⊇ P} subjects(cs) × Π_{p∈P} occ(cs,p)/subjects(cs), a constant object scaling its
/// pattern by n(p,o)/n(p); subjects outside the stored sets fall back to independence.
fn star_estimate<'p>(sc: &ScopeStats, star: impl Iterator<Item = &'p QuadPat>) -> Option<f64> {
    let mut preds: SmallVec<[(TermId, f64, f64); 8]> = SmallVec::new();
    for p in star {
        let PatTerm::Const(pred) = p.p else { return None };
        let ps = sc.pred.get(&pred)?;
        let obj_scale = match p.o {
            PatTerm::Const(o) => sc.po.get(&(pred, o)).map_or(1.0 / ps.dobj.max(1) as f64, |n| *n as f64 / ps.n.max(1) as f64),
            PatTerm::Slot(_) => 1.0,
        };
        preds.push((pred, ps.n as f64, obj_scale));
    }
    let mut total = 0.0;
    let mut covered = 0u64;
    for cs in &sc.cs {
        covered += cs.subjects;
        let subjects = cs.subjects.max(1) as f64;
        let mut est = subjects;
        for (pred, _, scale) in &preds {
            let Ok(i) = cs.preds.binary_search_by_key(pred, |x| x.0) else {
                est = 0.0;
                break;
            };
            est *= cs.preds[i].1 as f64 / subjects * scale;
        }
        total += est;
    }
    let missing = sc.ds.saturating_sub(covered) as f64;
    if missing > 0.0 {
        let ds = sc.ds.max(1) as f64;
        total += preds.iter().fold(missing, |acc, (_, n, scale)| acc * n / ds * scale);
    }
    Some(total)
}

/// Rows of a pattern when nothing better is known, by which of s / p / o are constant.
fn default_rows(s: bool, p: bool, o: bool) -> f64 {
    match (s, p, o) {
        (false, false, false) => 1_000_000.0,
        (false, true, false) => 10_000.0,
        (true, false, false) => 20.0,
        (false, false, true) => 100.0,
        (true, true, false) => 3.0,
        (false, true, true) => 50.0,
        (true, false, true) => 2.0,
        (true, true, true) => 1.0,
    }
}

/// Selectivity of a filter conjunct (§7.4.5).
pub(crate) fn selectivity(e: &Expr) -> f64 {
    match e {
        Expr::Equal(..) | Expr::SameTerm(..) => 0.1,
        Expr::Greater(..) | Expr::GreaterOrEqual(..) | Expr::Less(..) | Expr::LessOrEqual(..) => 1.0 / 3.0,
        Expr::Bound(_) => 0.9,
        Expr::Exists(_) => 0.5,
        Expr::Not(inner) => match &**inner {
            Expr::Exists(_) => 0.5,
            Expr::Bound(_) => 0.1,
            other => (1.0 - selectivity(other)).clamp(0.05, 1.0),
        },
        Expr::And(a, b) => selectivity(a) * selectivity(b),
        Expr::Or(a, b) => {
            let (x, y) = (selectivity(a), selectivity(b));
            x + y - x * y
        }
        Expr::In(_, list) => (0.1 * list.len() as f64).min(1.0),
        Expr::Call(Function::Regex | Function::Contains | Function::StrStarts | Function::StrEnds, _) => 0.25,
        Expr::Const(_) => 1.0,
        _ => 1.0 / 3.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sq_core::id::Tag;
    use sq_core::ir::NamedGraphs;
    use sq_core::store::QuadStoreWriter;
    use sq_memstore::MemStore;

    fn iri(n: i64) -> TermId {
        TermId::from_dict(2000 + n, Tag::Iri)
    }

    fn store() -> MemStore {
        let st = MemStore::new();
        let mut quads = Vec::new();
        for s in 0..50 {
            quads.push([TermId::DEFAULT_GRAPH, iri(s), iri(100), iri(200 + s % 5)]);
            quads.push([TermId::DEFAULT_GRAPH, iri(s), iri(101), TermId::from_int(s).unwrap()]);
        }
        quads.push([iri(900), iri(1), iri(100), iri(2)]);
        st.insert(&mut quads).unwrap();
        st
    }

    fn ds() -> DatasetSpec {
        DatasetSpec { default: DefaultGraph::StoreDefault, named: NamedGraphs::All }
    }

    fn pat(s: PatTerm, p: PatTerm, o: PatTerm) -> QuadPat {
        QuadPat { g: GraphSel::Default, s, p, o }
    }

    #[test]
    fn counts_without_stats() {
        let st = store();
        let ds = ds();
        let mut card = Card::new(&st, &ds, 1, &QueryOptions::default());
        let p = pat(PatTerm::Slot(0), PatTerm::Const(iri(100)), PatTerm::Slot(1));
        assert_eq!(card.pattern_rows(&p, SlotSet::EMPTY).unwrap(), 50.0);
        let p = pat(PatTerm::Slot(0), PatTerm::Const(iri(100)), PatTerm::Const(iri(201)));
        assert_eq!(card.pattern_rows(&p, SlotSet::EMPTY).unwrap(), 10.0);
        let temp = pat(PatTerm::Slot(0), PatTerm::Const(TermId::temp(0, Tag::Iri)), PatTerm::Slot(1));
        assert_eq!(card.pattern_rows(&temp, SlotSet::EMPTY).unwrap(), 0.0);
        // Seeded subject: per-probe estimate.
        let p = pat(PatTerm::Slot(0), PatTerm::Const(iri(100)), PatTerm::Slot(1));
        let per_probe = card.pattern_rows(&p, SlotSet::from_slots([0])).unwrap();
        assert!(per_probe < 50.0 && per_probe > 0.0);
    }

    #[test]
    fn probe_budget_is_respected() {
        let st = store();
        let ds = ds();
        let opts = QueryOptions { max_probe_counts: 1, ..QueryOptions::default() };
        let mut card = Card::new(&st, &ds, 1, &opts);
        let a = pat(PatTerm::Slot(0), PatTerm::Const(iri(100)), PatTerm::Slot(1));
        let b = pat(PatTerm::Slot(0), PatTerm::Const(iri(101)), PatTerm::Slot(1));
        assert_eq!(card.pattern_rows(&a, SlotSet::EMPTY).unwrap(), 50.0);
        assert_eq!(card.pattern_rows(&b, SlotSet::EMPTY).unwrap(), default_rows(false, true, false));
        // Cached counts do not consume budget.
        assert_eq!(card.pattern_rows(&a, SlotSet::EMPTY).unwrap(), 50.0);
    }

    #[test]
    fn stats_estimates() {
        let st = store();
        st.analyze();
        let ds = ds();
        let mut card = Card::new(&st, &ds, 1, &QueryOptions::default());
        let p = pat(PatTerm::Slot(0), PatTerm::Const(iri(101)), PatTerm::Slot(1));
        assert_eq!(card.pattern_rows(&p, SlotSet::EMPTY).unwrap(), 50.0);
        // ?s :101 ?o with s seeded: n / ds = 1 per probe
        assert_eq!(card.pattern_rows(&p, SlotSet::from_slots([0])).unwrap(), 1.0);
        let named = QuadPat { g: GraphSel::Slot(5), ..p };
        assert!(card.pattern_rows(&named, SlotSet::EMPTY).unwrap() <= 1.0);
    }

    #[test]
    fn characteristic_sets() {
        // 50 subjects with :100 and :101, plus 10 subjects with :102 only.
        let st = store();
        let mut extra: Vec<[TermId; 4]> =
            (0..10).map(|s| [TermId::DEFAULT_GRAPH, iri(500 + s), iri(102), iri(1)]).collect();
        st.insert(&mut extra).unwrap();
        st.analyze();
        let ds = ds();
        let card = Card::new(&st, &ds, 1, &QueryOptions::default());
        let p = |pred: i64| pat(PatTerm::Slot(0), PatTerm::Const(iri(pred)), PatTerm::Slot(1));
        // Every subject with :100 also has exactly one :101.
        assert_eq!(card.star_ratio(&[p(100)], &p(101)), Some(1.0));
        // No subject has both :100 and :102.
        assert_eq!(card.star_ratio(&[p(100)], &p(102)), Some(0.0));
        // A constant object scales by its share of the predicate.
        let typed = pat(PatTerm::Slot(0), PatTerm::Const(iri(100)), PatTerm::Const(iri(201)));
        let r = card.star_ratio(&[p(101)], &typed).unwrap();
        assert!((r - 0.2).abs() < 1e-9, "{r}");
        assert_eq!(card.star_ratio(&[], &p(101)), None);
    }

    #[test]
    fn selectivities() {
        let gt = Expr::Greater(Box::new(Expr::Slot(0)), Box::new(Expr::Slot(1)));
        assert!((selectivity(&gt) - 1.0 / 3.0).abs() < 1e-9);
        assert_eq!(selectivity(&Expr::Bound(0)), 0.9);
        let and = Expr::And(Box::new(Expr::Bound(0)), Box::new(Expr::Bound(1)));
        assert!((selectivity(&and) - 0.81).abs() < 1e-9);
    }
}
