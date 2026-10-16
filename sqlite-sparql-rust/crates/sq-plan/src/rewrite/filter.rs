//! Filter placement (spec §7.3.2) and the sameTerm seed rewrite (§7.3.3).
//!
//! Conjuncts only move DOWN, only to a node where every slot they can observe (`evars`) is
//! certainly bound, are copied into every UNION branch, and never enter the right side of a
//! LeftJoin (except that LeftJoin's own filter conjuncts whose evars are certain in the right
//! side) or of a Minus, never cross a Slice, and are never hoisted out of their group. Inside a
//! join group a conjunct is pushed into an opaque unit when that unit alone binds all its
//! evars; otherwise it stays at the group, where the join planner applies it as soon as the
//! joined prefix binds its evars.
//!
//! A conjunct `sameTerm(?x, c)` (or `?x = c` for an IRI / simple literal c) resting on a node
//! where ?x is certain becomes `Join(Values{?x: [c]}, node)`, i.e. a seed constant.
use sq_core::batch::SlotSet;
use sq_core::ir::{Expr, Logical, PatTerm};
use sq_core::{Error, Result};

use crate::expr::{and_all, conjuncts, seed_constant};
use crate::logical::{children, is_group_leaf};
use crate::props::LogicalProps;

pub(crate) struct FilterPlacer<'p, 'a> {
    lp: &'p LogicalProps<'a>,
}

impl<'p, 'a> FilterPlacer<'p, 'a> {
    pub(crate) fn new(lp: &'p LogicalProps<'a>) -> Self {
        FilterPlacer { lp }
    }

    pub(crate) fn place(&self, l: Logical) -> Logical {
        self.push(l, Vec::new())
    }

    /// `cs` are conjuncts that may legally be evaluated on `l`'s output rows.
    fn push(&self, l: Logical, mut cs: Vec<Expr>) -> Logical {
        match l {
            Logical::Filter { input, expr } => {
                let mut own = Vec::new();
                conjuncts(expr, &mut own);
                own.append(&mut cs);
                self.push(*input, own)
            }
            Logical::Join(..) | Logical::Bgp(_) => self.push_group(l, cs),
            Logical::Union(a, b) => {
                let a = self.push(*a, cs.clone());
                Logical::Union(Box::new(a), Box::new(self.push(*b, cs)))
            }
            Logical::LeftJoin { left, right, filter } => {
                let (pl, pr) = (self.lp.props(&left), self.lp.props(&right));
                // Context of both the outer conjuncts and the LeftJoin's own filter: merged rows.
                let ctx = pl.maybe.union(pr.maybe);
                let (to_left, keep) = self.split(cs, ctx, |ev| ev.is_subset(&pl.certain));
                let mut own = Vec::new();
                if let Some(f) = filter {
                    conjuncts(f, &mut own);
                }
                let (to_right, own) = self.split(own, ctx, |ev| ev.is_subset(&pr.certain));
                let node = Logical::LeftJoin {
                    left: Box::new(self.push(*left, to_left)),
                    right: Box::new(self.push(*right, to_right)),
                    filter: and_all(own),
                };
                self.wrap(node, keep)
            }
            Logical::Minus { left, right } => {
                let pl = self.lp.props(&left);
                let (to_left, keep) = self.split(cs, pl.maybe, |ev| ev.is_subset(&pl.certain));
                let node = Logical::Minus {
                    left: Box::new(self.push(*left, to_left)),
                    right: Box::new(self.push(*right, Vec::new())),
                };
                self.wrap(node, keep)
            }
            Logical::Extend { input, slot, expr } => {
                let ip = self.lp.props(&input);
                let mut ctx = ip.maybe;
                ctx.insert(slot);
                let (down, keep) = self.split(cs, ctx, |ev| !ev.contains(slot) && ev.is_subset(&ip.certain));
                self.wrap(Logical::Extend { input: Box::new(self.push(*input, down)), slot, expr }, keep)
            }
            Logical::Project { input, slots } => {
                let ip = self.lp.props(&input);
                let projected = SlotSet::from_slots(slots.iter().copied());
                // evars relative to the sub-select's own rows must all be projected.
                let (down, keep) = self.split(cs, ip.maybe, |ev| ev.is_subset(&projected) && ev.is_subset(&ip.certain));
                self.wrap(Logical::Project { input: Box::new(self.push(*input, down)), slots }, keep)
            }
            Logical::Distinct(input) => {
                let (down, keep) = self.split_through(&input, cs);
                self.wrap(Logical::Distinct(Box::new(self.push(*input, down))), keep)
            }
            Logical::Reduced(input) => {
                let (down, keep) = self.split_through(&input, cs);
                self.wrap(Logical::Reduced(Box::new(self.push(*input, down))), keep)
            }
            Logical::OrderBy { input, keys } => {
                let (down, keep) = self.split_through(&input, cs);
                self.wrap(Logical::OrderBy { input: Box::new(self.push(*input, down)), keys }, keep)
            }
            Logical::Graph { name, input } => {
                let ip = self.lp.props(&input);
                let g = match name {
                    PatTerm::Slot(g) => Some(g),
                    PatTerm::Const(_) => None,
                };
                let mut ctx = ip.maybe;
                if let Some(g) = g {
                    ctx.insert(g);
                }
                // Never move EXISTS into a GRAPH: it would read a different active graph.
                let (down, keep) = self.split_exprs(cs, |c| {
                    let ev = self.lp.evars(c, ctx);
                    !c.contains_exists() && g.is_none_or(|g| !ev.contains(g)) && ev.is_subset(&ip.certain)
                });
                self.wrap(Logical::Graph { name, input: Box::new(self.push(*input, down)) }, keep)
            }
            Logical::Group { input, keys, aggs } => {
                let ip = self.lp.props(&input);
                let key_set = SlotSet::from_slots(keys.iter().copied());
                // Not for the implicit group (keys = []): it yields a row even for no input.
                let (down, keep) = self.split_exprs(cs, |c| {
                    let ev = self.lp.evars(c, key_set);
                    !keys.is_empty() && !c.contains_exists() && ev.is_subset(&key_set) && ev.is_subset(&ip.certain)
                });
                self.wrap(Logical::Group { input: Box::new(self.push(*input, down)), keys, aggs }, keep)
            }
            Logical::Slice { input, offset, limit } => {
                let node = Logical::Slice { input: Box::new(self.push(*input, Vec::new())), offset, limit };
                self.wrap(node, cs)
            }
            leaf @ (Logical::Unit
            | Logical::Empty
            | Logical::Path { .. }
            | Logical::Values { .. }
            | Logical::Service { .. }) => self.wrap(leaf, cs),
        }
    }

    /// Distinct / Reduced / OrderBy keep rows unchanged: conjuncts pass when certain below.
    fn split_through(&self, input: &Logical, cs: Vec<Expr>) -> (Vec<Expr>, Vec<Expr>) {
        let ip = self.lp.props(input);
        self.split(cs, ip.maybe, |ev| ev.is_subset(&ip.certain))
    }

    fn split(&self, cs: Vec<Expr>, ctx: SlotSet, ok: impl Fn(SlotSet) -> bool) -> (Vec<Expr>, Vec<Expr>) {
        self.split_exprs(cs, |c| ok(self.lp.evars(c, ctx)))
    }

    fn split_exprs(&self, cs: Vec<Expr>, ok: impl Fn(&Expr) -> bool) -> (Vec<Expr>, Vec<Expr>) {
        cs.into_iter().partition(|c| ok(c))
    }

    /// A join group: push each conjunct into the first opaque unit that binds all its evars.
    fn push_group(&self, l: Logical, cs: Vec<Expr>) -> Logical {
        let ctx = self.lp.props(&l).maybe;
        let mut units = Vec::new();
        flatten_join(l, &mut units);
        let unit_certain: Vec<SlotSet> = units.iter().map(|u| self.lp.props(u).certain).collect();
        let mut assigned: Vec<Vec<Expr>> = vec![Vec::new(); units.len()];
        let mut keep = Vec::new();
        for c in cs {
            let ev = self.lp.evars(&c, ctx);
            match units.iter().zip(&unit_certain).position(|(u, cert)| !is_group_leaf(u) && ev.is_subset(cert)) {
                Some(i) => assigned[i].push(c),
                None => keep.push(c),
            }
        }
        let rebuilt = units
            .into_iter()
            .zip(assigned)
            .map(|(u, cs)| if is_group_leaf(&u) { u } else { self.push(u, cs) })
            .reduce(|a, b| Logical::Join(Box::new(a), Box::new(b)))
            .unwrap_or(Logical::Unit);
        self.wrap(rebuilt, keep)
    }

    /// Attach `keep` at `node`: seed constants first, then one Filter with the rest.
    fn wrap(&self, mut node: Logical, keep: Vec<Expr>) -> Logical {
        if keep.is_empty() {
            return node;
        }
        let certain = self.lp.props(&node).certain;
        let mut rest = Vec::new();
        for c in keep {
            match seed_constant(&c) {
                Some((x, k)) if certain.contains(x) => {
                    let seed = Logical::Values { slots: vec![x], rows: vec![vec![k]] };
                    node = Logical::Join(Box::new(seed), Box::new(node));
                }
                _ => rest.push(c),
            }
        }
        match and_all(rest) {
            Some(expr) => Logical::Filter { input: Box::new(node), expr },
            None => node,
        }
    }
}

/// Check the placement invariant that no physical plan can show (spec §7.1 step 6): no
/// conjunct entered the right side of a LeftJoin (other than one of that LeftJoin's own
/// filter) or of a Minus. A plan cannot tell a moved conjunct from a nested group's own filter
/// that reads an outer variable, so `validate()` cannot check this; `optimize` runs this check
/// on every pattern before and after placement in debug builds. Placement never creates, drops
/// or reorders LeftJoin / Minus nodes, so their pre-order sequences align; the conjuncts in a
/// right side may only lose members (seed constants) or be copied within it (UNION branches).
pub(crate) fn check_barriers(before: &Logical, after: &Logical) -> Result<()> {
    let (mut b, mut a) = (Vec::new(), Vec::new());
    barrier_conjuncts(before, &mut b);
    barrier_conjuncts(after, &mut a);
    if a.len() != b.len() {
        return Err(Error::internal("filter placement changed the OPTIONAL / MINUS structure"));
    }
    match b.iter().zip(&a).find(|(b, a)| !a.iter().all(|c| b.contains(c))) {
        Some(_) => Err(Error::internal("filter placement moved a conjunct into an OPTIONAL / MINUS right side")),
        None => Ok(()),
    }
}

/// For each LeftJoin / Minus of `l` in pre-order: every conjunct inside its right side (Filter
/// and LeftJoin filters at any depth), plus a LeftJoin's own filter conjuncts.
fn barrier_conjuncts<'l>(l: &'l Logical, out: &mut Vec<Vec<&'l Expr>>) {
    fn split<'l>(e: &'l Expr, out: &mut Vec<&'l Expr>) {
        match e {
            Expr::And(a, b) => {
                split(a, out);
                split(b, out);
            }
            other => out.push(other),
        }
    }
    fn all_in<'l>(l: &'l Logical, out: &mut Vec<&'l Expr>) {
        match l {
            Logical::Filter { expr, .. } | Logical::LeftJoin { filter: Some(expr), .. } => split(expr, out),
            _ => {}
        }
        children(l).into_iter().for_each(|c| all_in(c, out));
    }
    match l {
        Logical::LeftJoin { right, filter, .. } => {
            let mut cs = Vec::new();
            all_in(right, &mut cs);
            filter.iter().for_each(|f| split(f, &mut cs));
            out.push(cs);
        }
        Logical::Minus { right, .. } => {
            let mut cs = Vec::new();
            all_in(right, &mut cs);
            out.push(cs);
        }
        _ => {}
    }
    children(l).into_iter().for_each(|c| barrier_conjuncts(c, out));
}

/// Units of a join group: the leaves of a Join tree (Bgp nodes stay whole here).
fn flatten_join(l: Logical, out: &mut Vec<Logical>) {
    match l {
        Logical::Join(a, b) => {
            flatten_join(*a, out);
            flatten_join(*b, out);
        }
        other => out.push(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sq_core::batch::Slot;
    use sq_core::id::Tag;
    use sq_core::ir::{ExistsId, GraphSel, QuadPat};
    use sq_core::TermId;

    fn c(n: i64) -> PatTerm {
        PatTerm::Const(TermId::from_dict(2000 + n, Tag::Iri))
    }
    fn tp(s: Slot, p: i64, o: Slot) -> Logical {
        Logical::Bgp(vec![QuadPat { g: GraphSel::Default, s: PatTerm::Slot(s), p: c(p), o: PatTerm::Slot(o) }])
    }
    fn gt(s: Slot) -> Expr {
        Expr::Greater(Box::new(Expr::Slot(s)), Box::new(Expr::Const(TermId::from_int(3).unwrap())))
    }
    fn place(l: Logical) -> Logical {
        place_with(l, &[])
    }
    fn place_with(l: Logical, exists: &[Logical]) -> Logical {
        let lp = LogicalProps::new(exists);
        FilterPlacer::new(&lp).place(l)
    }

    #[test]
    fn not_bound_stays_above_optional() {
        let lj = Logical::LeftJoin { left: Box::new(tp(0, 1, 1)), right: Box::new(tp(1, 2, 2)), filter: None };
        let f = Logical::Filter { input: Box::new(lj.clone()), expr: Expr::Not(Box::new(Expr::Bound(2))) };
        assert_eq!(place(f.clone()), f);
        // A conjunct on the certain left slot moves into the left side.
        let f = Logical::Filter { input: Box::new(lj), expr: gt(0) };
        let Logical::LeftJoin { left, .. } = place(f) else { panic!("filter should have moved") };
        assert!(matches!(*left, Logical::Filter { .. }));
    }

    #[test]
    fn optional_filter_moves_into_right_only_when_certain_there() {
        let lj = Logical::LeftJoin {
            left: Box::new(tp(0, 1, 1)),
            right: Box::new(tp(1, 2, 2)),
            filter: Some(Expr::And(Box::new(gt(2)), Box::new(gt(0)))),
        };
        let Logical::LeftJoin { right, filter, .. } = place(lj) else { panic!() };
        assert!(matches!(*right, Logical::Filter { expr, .. } if expr == gt(2)));
        assert_eq!(filter, Some(gt(0)));
    }

    #[test]
    fn copied_into_union_branches_and_not_across_slice() {
        let u = Logical::Union(Box::new(tp(0, 1, 1)), Box::new(tp(0, 2, 1)));
        let f = Logical::Filter { input: Box::new(u), expr: gt(1) };
        let Logical::Union(a, b) = place(f) else { panic!() };
        assert!(matches!(*a, Logical::Filter { .. }) && matches!(*b, Logical::Filter { .. }));
        let s = Logical::Slice { input: Box::new(tp(0, 1, 1)), offset: 0, limit: Some(1) };
        let f = Logical::Filter { input: Box::new(s), expr: gt(1) };
        assert!(matches!(place(f), Logical::Filter { input, .. } if matches!(*input, Logical::Slice { .. })));
    }

    #[test]
    fn group_filter_scope_is_kept() {
        // { ?0 :1 ?1 { ?1 :2 ?2 FILTER(?0 > 3) } }: the inner filter sees ?0 unbound and must
        // stay inside its group.
        let inner = Logical::Filter { input: Box::new(tp(1, 2, 2)), expr: gt(0) };
        let j = Logical::Join(Box::new(tp(0, 1, 1)), Box::new(inner.clone()));
        assert_eq!(place(j.clone()), j);
        // A group-level filter on a slot of an opaque unit is pushed into it.
        let u = Logical::Union(Box::new(tp(1, 3, 2)), Box::new(tp(1, 4, 2)));
        let f = Logical::Filter { input: Box::new(Logical::Join(Box::new(tp(0, 1, 1)), Box::new(u))), expr: gt(2) };
        let Logical::Join(_, b) = place(f) else { panic!() };
        assert!(matches!(*b, Logical::Union(..)));
    }

    #[test]
    fn same_term_becomes_seed() {
        let k = TermId::from_dict(3000, Tag::Iri);
        let f = Logical::Filter {
            input: Box::new(tp(0, 1, 1)),
            expr: Expr::SameTerm(Box::new(Expr::Slot(1)), Box::new(Expr::Const(k))),
        };
        assert_eq!(
            place(f),
            Logical::Join(Box::new(Logical::Values { slots: vec![1], rows: vec![vec![k]] }), Box::new(tp(0, 1, 1)))
        );
        // ?x maybe-bound only: stays a filter.
        let lj = Logical::LeftJoin { left: Box::new(tp(0, 1, 1)), right: Box::new(tp(1, 2, 2)), filter: None };
        let f = Logical::Filter {
            input: Box::new(lj),
            expr: Expr::Equal(Box::new(Expr::Slot(2)), Box::new(Expr::Const(k))),
        };
        assert!(matches!(place(f), Logical::Filter { .. }));
    }

    #[test]
    fn barrier_check_accepts_placement_and_rejects_moved_conjuncts() {
        let lj = |right: Logical, filter| {
            Logical::LeftJoin { left: Box::new(tp(0, 1, 1)), right: Box::new(right), filter }
        };
        let not_bound = Expr::Not(Box::new(Expr::Bound(2)));
        // Placement keeps FILTER(!bound(?2)) above its OPTIONAL and moves the OPTIONAL's own
        // certain-right conjunct into the right side: both pass.
        let before = Logical::Filter { input: Box::new(lj(tp(1, 2, 2), Some(gt(2)))), expr: not_bound.clone() };
        let after = place(before.clone());
        assert!(check_barriers(&before, &after).is_ok());
        // The outer conjunct moved into the right side is rejected, for OPTIONAL and MINUS.
        let moved = lj(Logical::Filter { input: Box::new(tp(1, 2, 2)), expr: not_bound.clone() }, None);
        assert!(check_barriers(&before, &moved).is_err());
        let minus = |right: Logical| Logical::Minus { left: Box::new(tp(0, 1, 1)), right: Box::new(right) };
        let before = Logical::Filter { input: Box::new(minus(tp(1, 2, 2))), expr: gt(2) };
        let moved = minus(Logical::Filter { input: Box::new(tp(1, 2, 2)), expr: gt(2) });
        assert!(check_barriers(&before, &place(before.clone())).is_ok());
        assert!(check_barriers(&before, &moved).is_err());
        // A changed OPTIONAL / MINUS structure is rejected as well.
        assert!(check_barriers(&before, &tp(0, 1, 1)).is_err());
    }

    #[test]
    fn exists_conjuncts_respect_context() {
        // FILTER EXISTS { ?2 :5 ?0 } over Join(?0 :1 ?1, Union(..binds ?2..)): the EXISTS can
        // observe ?2, which only the union binds, so it may not move into the first unit.
        let exists = vec![tp(2, 5, 0)];
        let u = Logical::Union(Box::new(tp(1, 3, 2)), Box::new(tp(1, 4, 2)));
        let j = Logical::Join(Box::new(tp(0, 1, 1)), Box::new(u));
        let f = Logical::Filter { input: Box::new(j), expr: Expr::Exists(ExistsId(0)) };
        assert!(matches!(place_with(f, &exists), Logical::Filter { .. }));
    }
}
