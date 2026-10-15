//! Static properties (spec §7.2): `vars`, `certain`, `maybe` and `safe_inject` per node, for
//! the logical IR and for physical plans (the latter is what `validate()` checks against).
//!
//! `safe_inject` is stored restricted to `vars`; a slot the subtree never mentions is always
//! safe ("P ignores it and the rows are merged"), see [`Props::safe_hat`].
//!
//! `evars(e) = slots(e) ∪ ⋃ { vars(exists[id]) ∩ maybe(context) | Exists(id) in e }`.
//! Filter placement uses exactly that. For `safe_inject` the planner is deliberately more
//! conservative and uses all of `vars(exists[id])` (not intersected with maybe(context)): a
//! Join-mode seed on a slot that only an EXISTS pattern mentions would otherwise be visible to
//! the correlated EXISTS if the executor merges seeds into intermediate rows.
use std::cell::RefCell;

use sq_core::batch::{Slot, SlotSet};
use sq_core::ir::{AggFunc, AggSpec, ExistsId, Expr, GraphSel, Logical, PatTerm, QuadPat};
use sq_core::plan::{JoinKind, Op, Plan};
use sq_core::TermId;

use crate::expr;

/// Every slot.
pub(crate) const ALL: SlotSet = SlotSet([u64::MAX; 4]);

fn not(s: SlotSet) -> SlotSet {
    ALL.minus(s)
}

/// The slots of `s` in ascending order, word by word (`SlotSet::iter` tests all 256 bits).
pub(crate) fn slot_list(s: SlotSet) -> Vec<Slot> {
    slots(s).collect()
}

/// The slots of `s` in ascending order, without allocating.
pub(crate) fn slots(s: SlotSet) -> impl Iterator<Item = Slot> {
    s.0.into_iter().enumerate().flat_map(|(w, mut bits)| {
        std::iter::from_fn(move || {
            (bits != 0).then(|| {
                let slot = (w as u32 * 64 + bits.trailing_zeros()) as Slot;
                bits &= bits - 1;
                slot
            })
        })
    })
}

/// Static properties of a (logical or physical) subtree.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Props {
    /// Every slot mentioned (including slots of expressions and EXISTS patterns).
    pub vars: SlotSet,
    /// Slots bound in every solution.
    pub certain: SlotSet,
    /// Slots bound in some solution.
    pub maybe: SlotSet,
    /// Slots (⊆ vars) on which a Join-mode seed is exact.
    pub safe_inject: SlotSet,
}

impl Props {
    /// Every slot a Join-mode seed may carry: `safe_inject` plus all slots outside `vars`.
    pub fn safe_hat(&self) -> SlotSet {
        self.safe_inject.union(not(self.vars))
    }
    /// Is a Join-mode seed on all of `seed` exact?
    pub fn accepts_seed(&self, seed: SlotSet) -> bool {
        seed.is_subset(&self.safe_hat())
    }
    fn with_safe_hat(mut self, safe_hat: SlotSet) -> Props {
        self.safe_inject = safe_hat.intersect(self.vars);
        self
    }
}

/// Properties of a logical subtree; `exists` are the query's EXISTS patterns (`LoweredQuery.exists`).
pub fn props(l: &Logical, exists: &[Logical]) -> Props {
    LogicalProps::new(exists).props(l)
}

// ---------------------------------------------------------------- per-operator formulas

pub(crate) fn pattern(p: &QuadPat) -> Props {
    let v = SlotSet::from_slots(pattern_slots(p));
    Props { vars: v, certain: v, maybe: v, safe_inject: v }
}

/// Slots of a quad pattern (the graph slot included).
pub(crate) fn pattern_slots(p: &QuadPat) -> impl Iterator<Item = Slot> {
    let g = match p.g {
        GraphSel::Slot(s) => Some(s),
        _ => None,
    };
    let term = |t: PatTerm| match t {
        PatTerm::Slot(s) => Some(s),
        PatTerm::Const(_) => None,
    };
    [g, term(p.s), term(p.p), term(p.o)].into_iter().flatten()
}

pub(crate) fn path(g: GraphSel, s: PatTerm, o: PatTerm) -> Props {
    pattern(&QuadPat { g, s, p: PatTerm::Const(TermId::UNBOUND), o })
}

pub(crate) fn values(slots: &[Slot], rows: &[Vec<TermId>]) -> Props {
    let vars = SlotSet::from_slots(slots.iter().copied());
    let mut certain = vars;
    for row in rows {
        for (s, v) in slots.iter().zip(row) {
            if !v.is_bound() {
                certain.remove(*s);
            }
        }
    }
    Props { vars, certain, maybe: vars, safe_inject: vars }
}

pub(crate) fn join(a: &Props, b: &Props) -> Props {
    Props {
        vars: a.vars.union(b.vars),
        certain: a.certain.union(b.certain),
        maybe: a.maybe.union(b.maybe),
        safe_inject: SlotSet::EMPTY,
    }
    .with_safe_hat(a.safe_hat().intersect(b.safe_hat()))
}

/// LeftJoin(L, R, f): safe(L) ∩ (certain(L) ∪ ¬(vars(R) ∪ evars(f))); the seed goes to L only.
pub(crate) fn left_join(l: &Props, r: &Props, filter_vars: SlotSet) -> Props {
    Props {
        vars: l.vars.union(r.vars).union(filter_vars),
        certain: l.certain,
        maybe: l.maybe.union(r.maybe),
        safe_inject: SlotSet::EMPTY,
    }
    .with_safe_hat(l.safe_hat().intersect(l.certain.union(not(r.vars.union(filter_vars)))))
}

/// Minus(L, R) (and Semi / Anti joins): rows of L only; seed goes to L only.
pub(crate) fn minus(l: &Props, r: &Props) -> Props {
    Props { vars: l.vars.union(r.vars), certain: l.certain, maybe: l.maybe, safe_inject: SlotSet::EMPTY }
        .with_safe_hat(l.safe_hat().intersect(l.certain.union(not(r.vars))))
}

pub(crate) fn union<'a>(branches: impl IntoIterator<Item = &'a Props>) -> Props {
    let mut it = branches.into_iter();
    let Some(first) = it.next() else { return Props::default() };
    let mut out = *first;
    let mut hat = first.safe_hat();
    for b in it {
        out.vars = out.vars.union(b.vars);
        out.certain = out.certain.intersect(b.certain);
        out.maybe = out.maybe.union(b.maybe);
        hat = hat.intersect(b.safe_hat());
    }
    out.with_safe_hat(hat)
}

pub(crate) fn filter(p: &Props, expr_vars: SlotSet) -> Props {
    Props { vars: p.vars.union(expr_vars), ..*p }.with_safe_hat(p.safe_hat().intersect(p.certain.union(not(expr_vars))))
}

pub(crate) fn extend(p: &Props, slot: Slot, expr_vars: SlotSet, is_const: bool) -> Props {
    let mut vars = p.vars.union(expr_vars);
    vars.insert(slot);
    let mut certain = p.certain;
    if is_const {
        certain.insert(slot);
    }
    let mut maybe = p.maybe;
    maybe.insert(slot);
    let mut hat = p.safe_hat().intersect(p.certain.union(not(expr_vars)));
    hat.remove(slot);
    Props { vars, certain, maybe, safe_inject: SlotSet::EMPTY }.with_safe_hat(hat)
}

/// GRAPH ?g evaluated per named graph (GraphLoop): never seeded.
pub(crate) fn graph_loop(p: &Props, g: Slot) -> Props {
    let mut out = *p;
    out.vars.insert(g);
    out.certain.insert(g);
    out.maybe.insert(g);
    out.safe_inject = SlotSet::EMPTY;
    out
}

pub(crate) fn project(p: &Props, slots: &[Slot]) -> Props {
    let s = SlotSet::from_slots(slots.iter().copied());
    Props {
        vars: p.vars.union(s),
        certain: p.certain.intersect(s),
        maybe: p.maybe.intersect(s),
        safe_inject: SlotSet::EMPTY,
    }
    .with_safe_hat(s.intersect(p.safe_hat()))
}

pub(crate) fn distinct(p: &Props) -> Props {
    Props { safe_inject: p.safe_inject.intersect(p.certain), ..*p }
}

pub(crate) fn order_by(p: &Props, key_vars: SlotSet) -> Props {
    Props { vars: p.vars.union(key_vars), ..*p }.with_safe_hat(p.safe_hat())
}

/// Slice / TopK: a limit is not commutable with a seed filter.
pub(crate) fn slice(p: &Props, extra_vars: SlotSet) -> Props {
    Props { vars: p.vars.union(extra_vars), safe_inject: SlotSet::EMPTY, ..*p }
}

pub(crate) fn group(p: &Props, keys: &[Slot], aggs: &[(Slot, AggSpec)], agg_vars: SlotSet) -> Props {
    let keys = SlotSet::from_slots(keys.iter().copied());
    let outs = SlotSet::from_slots(aggs.iter().map(|(s, _)| *s));
    let counts = SlotSet::from_slots(
        aggs.iter().filter(|(_, a)| matches!(a.func, AggFunc::Count | AggFunc::CountAll)).map(|(s, _)| *s),
    );
    Props {
        vars: p.vars.union(keys).union(outs).union(agg_vars),
        certain: counts,
        maybe: keys.union(outs),
        safe_inject: SlotSet::EMPTY,
    }
    .with_safe_hat(keys.intersect(p.safe_hat()).intersect(p.certain))
}

/// Slots that only produce rows (GraphNames, Count, SqlJoin): never seeded.
pub(crate) fn opaque_leaf(out: SlotSet, extra_vars: SlotSet) -> Props {
    Props { vars: out.union(extra_vars), certain: out, maybe: out, safe_inject: SlotSet::EMPTY }
}

// ---------------------------------------------------------------- logical traversal

/// Logical property computation with a per-call cache of EXISTS pattern variables.
pub(crate) struct LogicalProps<'a> {
    exists: &'a [Logical],
    exists_vars: RefCell<Vec<Option<SlotSet>>>,
}

impl<'a> LogicalProps<'a> {
    pub(crate) fn new(exists: &'a [Logical]) -> Self {
        LogicalProps { exists, exists_vars: RefCell::new(vec![None; exists.len()]) }
    }

    /// All slots of `exists[id]` (nested EXISTS included).
    pub(crate) fn exists_vars(&self, id: ExistsId) -> SlotSet {
        let i = id.0 as usize;
        if let Some(Some(v)) = self.exists_vars.borrow().get(i) {
            return *v;
        }
        let Some(pat) = self.exists.get(i) else { return SlotSet::EMPTY };
        // Mark in progress (EXISTS ids are acyclic by construction; this only guards recursion).
        self.exists_vars.borrow_mut()[i] = Some(SlotSet::EMPTY);
        let v = self.props(pat).vars;
        self.exists_vars.borrow_mut()[i] = Some(v);
        v
    }

    /// slots(e) ∪ vars of every EXISTS pattern in e.
    pub(crate) fn expr_vars(&self, e: &Expr) -> SlotSet {
        expr::exists_ids(e).into_iter().fold(expr::slots(e), |acc, id| acc.union(self.exists_vars(id)))
    }

    /// evars(e) relative to a context whose maybe-bound slots are `maybe` (spec §7.2).
    pub(crate) fn evars(&self, e: &Expr, maybe: SlotSet) -> SlotSet {
        expr::exists_ids(e)
            .into_iter()
            .fold(expr::slots(e), |acc, id| acc.union(self.exists_vars(id).intersect(maybe)))
    }

    pub(crate) fn props(&self, l: &Logical) -> Props {
        match l {
            Logical::Unit | Logical::Empty | Logical::Service { .. } => Props::default(),
            Logical::Bgp(pats) => {
                let v = SlotSet::from_slots(pats.iter().flat_map(pattern_slots));
                Props { vars: v, certain: v, maybe: v, safe_inject: v }
            }
            Logical::Path { g, s, o, .. } => path(*g, *s, *o),
            Logical::Values { slots, rows } => values(slots, rows),
            Logical::Join(a, b) => join(&self.props(a), &self.props(b)),
            Logical::LeftJoin { left, right, filter } => {
                let fv = filter.as_ref().map_or(SlotSet::EMPTY, |f| self.expr_vars(f));
                left_join(&self.props(left), &self.props(right), fv)
            }
            Logical::Minus { left, right } => minus(&self.props(left), &self.props(right)),
            Logical::Union(a, b) => union([&self.props(a), &self.props(b)]),
            Logical::Filter { input, expr } => filter(&self.props(input), self.expr_vars(expr)),
            Logical::Extend { input, slot, expr } => {
                extend(&self.props(input), *slot, self.expr_vars(expr), matches!(expr, Expr::Const(_)))
            }
            Logical::Graph { name: PatTerm::Slot(g), input } => graph_loop(&self.props(input), *g),
            Logical::Graph { name: PatTerm::Const(_), input } => self.props(input),
            Logical::Group { input, keys, aggs } => {
                let av = aggs
                    .iter()
                    .filter_map(|(_, a)| a.arg.as_ref())
                    .fold(SlotSet::EMPTY, |acc, e| acc.union(self.expr_vars(e)));
                group(&self.props(input), keys, aggs, av)
            }
            Logical::OrderBy { input, keys } => {
                let kv = keys.iter().fold(SlotSet::EMPTY, |acc, k| acc.union(self.expr_vars(&k.expr)));
                order_by(&self.props(input), kv)
            }
            Logical::Project { input, slots } => project(&self.props(input), slots),
            Logical::Distinct(input) | Logical::Reduced(input) => distinct(&self.props(input)),
            Logical::Slice { input, .. } => slice(&self.props(input), SlotSet::EMPTY),
        }
    }
}

// ---------------------------------------------------------------- physical traversal

/// Properties of a physical plan subtree; `exists` are the QueryPlan's EXISTS plans.
pub fn plan_props(p: &Plan, exists: &[Plan]) -> Props {
    PlanProps::new(exists).props(p)
}

pub(crate) struct PlanProps<'a> {
    exists: &'a [Plan],
    exists_vars: RefCell<Vec<Option<SlotSet>>>,
}

impl<'a> PlanProps<'a> {
    pub(crate) fn new(exists: &'a [Plan]) -> Self {
        PlanProps { exists, exists_vars: RefCell::new(vec![None; exists.len()]) }
    }

    pub(crate) fn exists_vars(&self, id: ExistsId) -> SlotSet {
        let i = id.0 as usize;
        if let Some(Some(v)) = self.exists_vars.borrow().get(i) {
            return *v;
        }
        let Some(plan) = self.exists.get(i) else { return SlotSet::EMPTY };
        self.exists_vars.borrow_mut()[i] = Some(SlotSet::EMPTY);
        let v = self.props(plan).vars;
        self.exists_vars.borrow_mut()[i] = Some(v);
        v
    }

    pub(crate) fn expr_vars(&self, e: &Expr) -> SlotSet {
        expr::exists_ids(e).into_iter().fold(expr::slots(e), |acc, id| acc.union(self.exists_vars(id)))
    }

    fn kind_props(&self, l: &Props, r: &Props, kind: &JoinKind) -> Props {
        match kind {
            JoinKind::Inner => join(l, r),
            JoinKind::Left { filter } => {
                let fv = filter.as_ref().map_or(SlotSet::EMPTY, |f| self.expr_vars(f));
                left_join(l, r, fv)
            }
            JoinKind::Semi | JoinKind::Anti => minus(l, r),
        }
    }

    pub(crate) fn props(&self, p: &Plan) -> Props {
        match &p.op {
            Op::Unit | Op::Empty => Props::default(),
            Op::Scan(s) => pattern(&s.pat),
            Op::Values { slots, rows } => values(slots, rows),
            Op::BindJoin { left, right, seed, kind, .. } => {
                let (l, r) = (self.props(left), self.props(right));
                let out = self.kind_props(&l, &r, kind);
                // The right side only sees the seed list: an outer seed on a slot the right
                // side can bind must travel in it.
                let unseeded = r.maybe.minus(SlotSet::from_slots(seed.iter().copied()));
                let hat = out.safe_hat().intersect(r.safe_hat()).intersect(not(unseeded));
                out.with_safe_hat(match kind {
                    JoinKind::Inner => hat,
                    _ => out.safe_hat().intersect(not(unseeded)),
                })
            }
            Op::HashJoin { build, probe, kind, build_is_left, .. } => {
                let (b, pr) = (self.props(build), self.props(probe));
                let (l, r) = if *build_is_left { (b, pr) } else { (pr, b) };
                self.kind_props(&l, &r, kind)
            }
            Op::NestedLoop { left, right, kind, .. } => self.kind_props(&self.props(left), &self.props(right), kind),
            Op::MergeJoin { left, right, .. } => join(&self.props(left), &self.props(right)),
            Op::StarMerge { inputs, .. } => {
                let ps: Vec<Props> = inputs.iter().map(|i| self.props(i)).collect();
                ps.iter().skip(1).fold(ps.first().copied().unwrap_or_default(), |acc, b| join(&acc, b))
            }
            Op::Minus { left, right, .. } => minus(&self.props(left), &self.props(right)),
            Op::Union(branches) => {
                let ps: Vec<Props> = branches.iter().map(|b| self.props(b)).collect();
                union(ps.iter())
            }
            Op::Filter { input, expr } => filter(&self.props(input), self.expr_vars(expr)),
            Op::Extend { input, slot, expr, .. } => {
                extend(&self.props(input), *slot, self.expr_vars(expr), matches!(expr, Expr::Const(_)))
            }
            Op::Path { g, s, o, .. } => path(*g, *s, *o),
            Op::GraphLoop { slot, input } => graph_loop(&self.props(input), *slot),
            Op::GraphNames { slot } => opaque_leaf(SlotSet::from_slots([*slot]), SlotSet::EMPTY),
            Op::Group { input, keys, aggs, .. } => {
                let av = aggs
                    .iter()
                    .filter_map(|(_, a)| a.arg.as_ref())
                    .fold(SlotSet::EMPTY, |acc, e| acc.union(self.expr_vars(e)));
                group(&self.props(input), keys, aggs, av)
            }
            Op::Count { input, out } => opaque_leaf(SlotSet::from_slots([*out]), self.props(input).vars),
            Op::Sort { input, keys } => {
                let kv = keys.iter().fold(SlotSet::EMPTY, |acc, k| acc.union(self.expr_vars(&k.expr)));
                order_by(&self.props(input), kv)
            }
            Op::SortBySlot { input, key } => order_by(&self.props(input), SlotSet::from_slots([*key])),
            Op::TopK { input, keys, .. } => {
                let kv = keys.iter().fold(SlotSet::EMPTY, |acc, k| acc.union(self.expr_vars(&k.expr)));
                slice(&self.props(input), kv)
            }
            Op::Distinct { input, .. } | Op::Reduced { input, .. } => distinct(&self.props(input)),
            Op::Slice { input, .. } => slice(&self.props(input), SlotSet::EMPTY),
            Op::Project { input, slots } => project(&self.props(input), slots),
            Op::SqlJoin { out, .. } => opaque_leaf(SlotSet::from_slots(out.iter().copied()), SlotSet::EMPTY),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sq_core::id::Tag;

    fn c(n: i64) -> PatTerm {
        PatTerm::Const(TermId::from_dict(2000 + n, Tag::Iri))
    }
    fn v(s: Slot) -> PatTerm {
        PatTerm::Slot(s)
    }
    fn bgp(pats: &[(PatTerm, PatTerm, PatTerm)]) -> Logical {
        Logical::Bgp(pats.iter().map(|(s, p, o)| QuadPat { g: GraphSel::Default, s: *s, p: *p, o: *o }).collect())
    }
    fn set(s: &[Slot]) -> SlotSet {
        SlotSet::from_slots(s.iter().copied())
    }

    #[test]
    fn slot_list_matches_iter() {
        let s = set(&[0, 1, 63, 64, 130, 255]);
        assert_eq!(slot_list(s), s.iter().collect::<Vec<_>>());
        assert!(slot_list(SlotSet::EMPTY).is_empty());
    }

    #[test]
    fn left_join_formula() {
        // ?0 :p ?1 OPTIONAL { ?1 :q ?2 }  — seed on ?1 is safe (certain on the left),
        // seed on ?2 is not (only bound by the optional side).
        let l = Logical::LeftJoin {
            left: Box::new(bgp(&[(v(0), c(1), v(1))])),
            right: Box::new(bgp(&[(v(1), c(2), v(2))])),
            filter: None,
        };
        let p = props(&l, &[]);
        assert_eq!(p.certain, set(&[0, 1]));
        assert_eq!(p.maybe, set(&[0, 1, 2]));
        assert_eq!(p.safe_inject, set(&[0, 1]));
        assert!(!p.accepts_seed(set(&[2])));
        assert!(p.accepts_seed(set(&[0, 7])));
    }

    #[test]
    fn left_join_filter_vars_are_unsafe() {
        // OPTIONAL { ?1 :q ?2 FILTER(?3) } : ?3 is neither certain on the left nor ignored.
        let l = Logical::LeftJoin {
            left: Box::new(bgp(&[(v(0), c(1), v(1))])),
            right: Box::new(bgp(&[(v(1), c(2), v(2))])),
            filter: Some(Expr::Bound(3)),
        };
        let p = props(&l, &[]);
        assert!(p.vars.contains(3));
        assert!(!p.safe_inject.contains(3));
    }

    #[test]
    fn union_and_values() {
        let u = Logical::Union(
            Box::new(bgp(&[(v(0), c(1), v(1))])),
            Box::new(Logical::Values {
                slots: vec![0, 2],
                rows: vec![vec![TermId::from_int(1).unwrap(), TermId::UNBOUND]],
            }),
        );
        let p = props(&u, &[]);
        assert_eq!(p.certain, set(&[0]));
        assert_eq!(p.maybe, set(&[0, 1, 2]));
        assert_eq!(p.safe_inject, set(&[0, 1, 2]));
    }

    #[test]
    fn extend_group_project_distinct_slice() {
        let base = bgp(&[(v(0), c(1), v(1))]);
        let e = Logical::Extend { input: Box::new(base.clone()), slot: 2, expr: Expr::Slot(1) };
        let p = props(&e, &[]);
        assert_eq!(p.certain, set(&[0, 1]));
        assert_eq!(p.maybe, set(&[0, 1, 2]));
        assert!(!p.safe_inject.contains(2));
        let e = Logical::Extend { input: Box::new(base.clone()), slot: 2, expr: Expr::Const(TermId::from_int(1).unwrap()) };
        assert!(props(&e, &[]).certain.contains(2));

        let g = Logical::Group {
            input: Box::new(base.clone()),
            keys: vec![0],
            aggs: vec![
                (3, AggSpec { func: AggFunc::CountAll, distinct: false, arg: None }),
                (4, AggSpec { func: AggFunc::Sum, distinct: false, arg: Some(Expr::Slot(1)) }),
            ],
        };
        let p = props(&g, &[]);
        assert_eq!(p.certain, set(&[3]));
        assert_eq!(p.maybe, set(&[0, 3, 4]));
        assert_eq!(p.safe_inject, set(&[0]));

        let pr = Logical::Project { input: Box::new(base.clone()), slots: vec![1, 5] };
        let p = props(&pr, &[]);
        assert_eq!(p.certain, set(&[1]));
        assert_eq!(p.safe_inject, set(&[1, 5]));
        assert!(!p.accepts_seed(set(&[0])));

        let d = Logical::Distinct(Box::new(Logical::LeftJoin {
            left: Box::new(base.clone()),
            right: Box::new(bgp(&[(v(1), c(2), v(2))])),
            filter: None,
        }));
        assert_eq!(props(&d, &[]).safe_inject, set(&[0, 1]));
        let s = Logical::Slice { input: Box::new(base), offset: 0, limit: Some(1) };
        assert!(props(&s, &[]).safe_inject.is_empty());
    }

    #[test]
    fn exists_vars_make_filter_slots_unsafe() {
        // FILTER EXISTS { ?3 :p ?0 } over a pattern binding ?0 only: ?3 lives in the EXISTS.
        let exists = vec![bgp(&[(v(3), c(1), v(0))])];
        let f = Logical::Filter { input: Box::new(bgp(&[(v(0), c(2), v(1))])), expr: Expr::Exists(ExistsId(0)) };
        let lp = LogicalProps::new(&exists);
        let p = lp.props(&f);
        assert!(p.vars.contains(3));
        assert!(!p.accepts_seed(set(&[3])));
        assert!(p.accepts_seed(set(&[0, 1])));
        // evars relative to the filter's context only see EXISTS slots that can be bound there.
        assert_eq!(lp.evars(&Expr::Exists(ExistsId(0)), set(&[0, 1])), set(&[0]));
    }

    #[test]
    fn graph_minus_and_slot_graphs() {
        let g = Logical::Graph { name: PatTerm::Slot(9), input: Box::new(bgp(&[(v(0), c(1), v(1))])) };
        let p = props(&g, &[]);
        assert_eq!(p.certain, set(&[0, 1, 9]));
        assert!(p.safe_inject.is_empty());
        let m = Logical::Minus { left: Box::new(bgp(&[(v(0), c(1), v(1))])), right: Box::new(bgp(&[(v(1), c(2), v(2))])) };
        let p = props(&m, &[]);
        assert_eq!(p.maybe, set(&[0, 1]));
        assert_eq!(p.safe_inject, set(&[0, 1]));
        let pushed = Logical::Bgp(vec![QuadPat { g: GraphSel::Slot(5), s: v(0), p: c(1), o: v(1) }]);
        assert_eq!(props(&pushed, &[]).certain, set(&[0, 1, 5]));
    }
}
