//! Empty propagation. `GRAPH <c>` with c outside the dataset's named graphs resolves to
//! `Empty` (spec §7.2); this pass folds every such `Empty` into its ancestors so that no
//! operator keeps an `Empty` input. Without it the planner would emit `Op::Empty` (which binds
//! nothing) for a subtree whose LOGICAL props still claim bound slots, and derive join keys and
//! BindJoin seeds from slots its input never binds.
//!
//! Rules (bottom-up; each is an identity of the SPARQL algebra over an empty multiset):
//! * Join(_, Empty), Join(Empty, _) -> Empty;
//! * LeftJoin(Empty, _, f) -> Empty; LeftJoin(L, Empty, f) -> L (every row of L survives);
//! * Minus(Empty, _) -> Empty; Minus(L, Empty) -> L;
//! * Union drops Empty branches;
//! * Filter / Extend / Project / Distinct / Reduced / OrderBy / Slice / Graph over Empty -> Empty;
//! * Group with keys over Empty -> Empty; Group without keys stays (it yields one row);
//! * an EXISTS pattern that is Empty makes a top-level filter conjunct `EXISTS {..}` false and
//!   `NOT EXISTS {..}` true: Filter(X, false ∧ ..) -> Empty, LeftJoin(L, R, false ∧ ..) -> L, and
//!   true conjuncts are dropped. (`e ∧ false` is false and `e ∧ true` has the effective boolean
//!   value of `e` even when `e` is an error.) Elsewhere such an EXISTS stays; the correlated
//!   evaluation of an `Op::Empty` pattern is false.
use sq_core::ir::{ExistsId, Expr, Logical};

use crate::expr::{and_all, conjuncts};
use crate::logical::map_children;

/// Fold `Empty` in `root` and in every EXISTS pattern (see module docs).
pub(crate) fn prune_empty(root: Logical, exists: &mut [Logical]) -> Logical {
    let mut pruner = Pruner { state: vec![State::Pending; exists.len()], exists };
    for id in 0..pruner.exists.len() {
        pruner.exists_is_empty(ExistsId(id as u32));
    }
    pruner.prune(root)
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum State {
    Pending,
    Running,
    Done,
}

struct Pruner<'e> {
    exists: &'e mut [Logical],
    state: Vec<State>,
}

/// A filter expression after folding its constant conjuncts.
enum Folded {
    /// Some conjunct is always false.
    False,
    /// Every conjunct was always true.
    True,
    Expr(Expr),
}

fn is_empty(l: &Logical) -> bool {
    matches!(l, Logical::Empty)
}

impl Pruner<'_> {
    /// Prune `exists[id]` (once) and report whether it is Empty.
    fn exists_is_empty(&mut self, id: ExistsId) -> bool {
        let i = id.0 as usize;
        match self.state.get(i) {
            Some(State::Pending) => {
                self.state[i] = State::Running;
                let pat = std::mem::replace(&mut self.exists[i], Logical::Unit);
                self.exists[i] = self.prune(pat);
                self.state[i] = State::Done;
            }
            Some(State::Done) => {}
            // EXISTS nesting is acyclic by construction and ids are checked by validate():
            // never fold what cannot be decided.
            Some(State::Running) | None => return false,
        }
        is_empty(&self.exists[i])
    }

    fn fold(&mut self, e: Expr) -> Folded {
        let mut cs = Vec::new();
        conjuncts(e, &mut cs);
        let mut kept = Vec::with_capacity(cs.len());
        for c in cs {
            let constant = match &c {
                Expr::Exists(id) => self.exists_is_empty(*id).then_some(false),
                Expr::Not(inner) => match &**inner {
                    Expr::Exists(id) => self.exists_is_empty(*id).then_some(true),
                    _ => None,
                },
                _ => None,
            };
            match constant {
                Some(false) => return Folded::False,
                Some(true) => {}
                None => kept.push(c),
            }
        }
        and_all(kept).map_or(Folded::True, Folded::Expr)
    }

    fn prune(&mut self, l: Logical) -> Logical {
        let l = map_children(l, &mut |c| self.prune(c));
        match l {
            Logical::Join(a, b) if is_empty(&a) || is_empty(&b) => Logical::Empty,
            Logical::LeftJoin { left, .. } if is_empty(&left) => Logical::Empty,
            Logical::LeftJoin { left, right, .. } if is_empty(&right) => *left,
            Logical::LeftJoin { left, right, filter: Some(f) } => match self.fold(f) {
                Folded::False => *left,
                Folded::True => Logical::LeftJoin { left, right, filter: None },
                Folded::Expr(f) => Logical::LeftJoin { left, right, filter: Some(f) },
            },
            Logical::Minus { left, .. } if is_empty(&left) => Logical::Empty,
            Logical::Minus { left, right } if is_empty(&right) => *left,
            Logical::Union(a, b) if is_empty(&a) => *b,
            Logical::Union(a, b) if is_empty(&b) => *a,
            Logical::Filter { input, .. } if is_empty(&input) => Logical::Empty,
            Logical::Filter { input, expr } => match self.fold(expr) {
                Folded::False => Logical::Empty,
                Folded::True => *input,
                Folded::Expr(expr) => Logical::Filter { input, expr },
            },
            Logical::Extend { input, .. }
            | Logical::Project { input, .. }
            | Logical::Distinct(input)
            | Logical::Reduced(input)
            | Logical::OrderBy { input, .. }
            | Logical::Slice { input, .. }
            | Logical::Graph { input, .. }
                if is_empty(&input) =>
            {
                Logical::Empty
            }
            Logical::Group { input, keys, .. } if !keys.is_empty() && is_empty(&input) => Logical::Empty,
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sq_core::batch::Slot;
    use sq_core::id::Tag;
    use sq_core::ir::{AggFunc, AggSpec, GraphSel, PatTerm, QuadPat};
    use sq_core::TermId;

    fn bgp(s: Slot, o: Slot) -> Logical {
        let p = PatTerm::Const(TermId::from_dict(2000, Tag::Iri));
        Logical::Bgp(vec![QuadPat { g: GraphSel::Default, s: PatTerm::Slot(s), p, o: PatTerm::Slot(o) }])
    }
    fn b(l: Logical) -> Box<Logical> {
        Box::new(l)
    }
    fn prune(l: Logical) -> Logical {
        prune_empty(l, &mut [])
    }

    #[test]
    fn joins_and_unions() {
        let e = || b(Logical::Empty);
        assert_eq!(prune(Logical::Join(b(bgp(0, 1)), e())), Logical::Empty);
        assert_eq!(prune(Logical::Join(e(), b(bgp(0, 1)))), Logical::Empty);
        assert_eq!(prune(Logical::Union(b(bgp(0, 1)), e())), bgp(0, 1));
        assert_eq!(prune(Logical::Union(e(), e())), Logical::Empty);
        let lj = Logical::LeftJoin { left: b(bgp(0, 1)), right: e(), filter: Some(Expr::Bound(2)) };
        assert_eq!(prune(lj), bgp(0, 1));
        assert_eq!(prune(Logical::LeftJoin { left: e(), right: b(bgp(0, 1)), filter: None }), Logical::Empty);
        assert_eq!(prune(Logical::Minus { left: b(bgp(0, 1)), right: e() }), bgp(0, 1));
        assert_eq!(prune(Logical::Minus { left: e(), right: b(bgp(0, 1)) }), Logical::Empty);
        // Propagates through nesting.
        let nested = Logical::Project {
            input: b(Logical::Filter { input: b(Logical::Join(b(bgp(0, 1)), e())), expr: Expr::Bound(0) }),
            slots: vec![0],
        };
        assert_eq!(prune(nested), Logical::Empty);
    }

    #[test]
    fn groups() {
        let count = AggSpec { func: AggFunc::CountAll, distinct: false, arg: None };
        let implicit = Logical::Group { input: b(Logical::Empty), keys: vec![], aggs: vec![(3, count.clone())] };
        assert_eq!(prune(implicit.clone()), implicit, "the implicit group yields one row");
        let keyed = Logical::Group { input: b(Logical::Empty), keys: vec![0], aggs: vec![(3, count)] };
        assert_eq!(prune(keyed), Logical::Empty);
    }

    #[test]
    fn empty_exists_patterns_fold() {
        let mut exists = vec![Logical::Join(b(bgp(0, 1)), b(Logical::Empty)), bgp(0, 2)];
        let not_exists = |id| Expr::Not(Box::new(Expr::Exists(ExistsId(id))));
        // NOT EXISTS { Empty } is true: the conjunct goes, the other one stays.
        let f = Logical::Filter {
            input: b(bgp(0, 1)),
            expr: Expr::And(Box::new(not_exists(0)), Box::new(Expr::Exists(ExistsId(1)))),
        };
        let out = prune_empty(f, &mut exists);
        assert_eq!(exists[0], Logical::Empty);
        assert_eq!(out, Logical::Filter { input: b(bgp(0, 1)), expr: Expr::Exists(ExistsId(1)) });
        // EXISTS { Empty } is false.
        let f = Logical::Filter { input: b(bgp(0, 1)), expr: Expr::Exists(ExistsId(0)) };
        assert_eq!(prune_empty(f, &mut exists), Logical::Empty);
        let lj = Logical::LeftJoin { left: b(bgp(0, 1)), right: b(bgp(1, 2)), filter: Some(Expr::Exists(ExistsId(0))) };
        assert_eq!(prune_empty(lj, &mut exists), bgp(0, 1));
        let lj = Logical::LeftJoin { left: b(bgp(0, 1)), right: b(bgp(1, 2)), filter: Some(not_exists(0)) };
        assert_eq!(
            prune_empty(lj, &mut exists),
            Logical::LeftJoin { left: b(bgp(0, 1)), right: b(bgp(1, 2)), filter: None }
        );
        // A nested EXISTS pattern is pruned before the pattern that references it.
        let mut exists = vec![
            Logical::Filter { input: b(bgp(0, 1)), expr: Expr::Exists(ExistsId(1)) },
            Logical::Join(b(Logical::Empty), b(bgp(0, 1))),
        ];
        assert_eq!(prune_empty(Logical::Unit, &mut exists), Logical::Unit);
        assert_eq!(exists, vec![Logical::Empty, Logical::Empty]);
    }
}
