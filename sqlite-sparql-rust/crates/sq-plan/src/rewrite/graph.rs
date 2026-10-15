//! GRAPH rewrites (spec §7.2 "GRAPH ?g push rule", "GRAPH \<c\>").
//!
//! * `GRAPH <c> { P }`: Empty when c is not a named graph of the dataset (even for an empty P);
//!   otherwise P with every `Active` of its immediate scope (EXISTS patterns included) replaced
//!   by `Const(c)`.
//! * `GRAPH ?g { P }`: P′ = P with `Active` -> `Slot(g)` when that is provably equivalent to
//!   evaluating P per named graph; otherwise the `Graph` node stays and is planned as a
//!   `GraphLoop` (or `GraphNames` for an empty P).
//!
//! The push condition is the spec's (only Bgp / Path / Join / LeftJoin / Union / Filter /
//! Extend / Values / nested Graph; no expression mentions ?g or contains EXISTS; no Extend
//! targets ?g; ?g certain in P′), tightened to close three holes where "?g certain in P′" holds
//! but the pushed form is not equivalent: `VALUES ?g` inside the graph (binds ?g without
//! ranging over named graphs), a UNION branch or nested `GRAPH <c>` binding ?g through a
//! non-graph position, and an OPTIONAL whose left side does not read the graph while its right
//! side does. Each node is classified as
//!   * `Indep`: does not read the active graph and never binds ?g;
//!   * `GForm`: equivalent to ⋃_G eval_G(X) ⋈ {?g ↦ G} once pushed;
//!   * `Bad`: anything else,
//! and P is pushed iff it is `GForm` (which implies ?g ∈ certain(P′)).
use sq_core::batch::Slot;
use sq_core::ir::{ExistsId, Expr, GraphSel, Logical, PatTerm};
use sq_core::TermId;

use crate::expr;
use crate::logical::{exprs, map_children};
use crate::props::LogicalProps;

/// Replace `GRAPH <c>` nodes (see module docs). `named` is the dataset's sorted named graphs.
pub(crate) fn resolve_const_graphs(l: Logical, exists: &mut Vec<Logical>, named: &[TermId]) -> Logical {
    match l {
        Logical::Graph { name: PatTerm::Const(c), input } => {
            let mut input = *input;
            // Substitute even when the graph is absent so that no EXISTS pattern keeps an
            // `Active` reference without an enclosing graph.
            subst_active(&mut input, GraphSel::Const(c), exists);
            if named.binary_search(&c).is_err() {
                return Logical::Empty;
            }
            resolve_const_graphs(input, exists, named)
        }
        other => map_children(other, &mut |c| resolve_const_graphs(c, exists, named)),
    }
}

/// Bottom-up GRAPH ?g push-or-loop decision.
pub(crate) fn push_graph_vars(l: Logical, exists: &mut Vec<Logical>, lp: &LogicalProps<'_>) -> Logical {
    match l {
        Logical::Graph { name: PatTerm::Slot(g), input } => {
            let mut input = push_graph_vars(*input, exists, lp);
            if classify(&input, g, lp) == Class::GForm {
                subst_active(&mut input, GraphSel::Slot(g), exists);
                input
            } else {
                Logical::Graph { name: PatTerm::Slot(g), input: Box::new(input) }
            }
        }
        other => map_children(other, &mut |c| push_graph_vars(c, exists, lp)),
    }
}

/// Replace `GraphSel::Active` by `to` in the immediate scope of `l` (not inside nested GRAPH
/// nodes), including the EXISTS patterns referenced from that scope.
pub(crate) fn subst_active(l: &mut Logical, to: GraphSel, exists: &mut Vec<Logical>) {
    let mut pending: Vec<ExistsId> = Vec::new();
    subst_node(l, to, &mut pending);
    while let Some(id) = pending.pop() {
        let Some(slot) = exists.get_mut(id.0 as usize) else { continue };
        let mut pat = std::mem::replace(slot, Logical::Unit);
        subst_node(&mut pat, to, &mut pending);
        exists[id.0 as usize] = pat;
    }
}

fn subst_node(l: &mut Logical, to: GraphSel, pending: &mut Vec<ExistsId>) {
    for e in exprs(l) {
        pending.extend(expr::exists_ids(e));
    }
    match l {
        Logical::Bgp(pats) => {
            for p in pats {
                if p.g == GraphSel::Active {
                    p.g = to;
                }
            }
        }
        Logical::Path { g, .. } => {
            if *g == GraphSel::Active {
                *g = to;
            }
        }
        Logical::Graph { .. } => {}
        Logical::Unit | Logical::Empty | Logical::Values { .. } | Logical::Service { .. } => {}
        Logical::Join(a, b) | Logical::Union(a, b) => {
            subst_node(a, to, pending);
            subst_node(b, to, pending);
        }
        Logical::LeftJoin { left, right, .. } | Logical::Minus { left, right } => {
            subst_node(left, to, pending);
            subst_node(right, to, pending);
        }
        Logical::Filter { input, .. }
        | Logical::Extend { input, .. }
        | Logical::Group { input, .. }
        | Logical::OrderBy { input, .. }
        | Logical::Project { input, .. }
        | Logical::Distinct(input)
        | Logical::Reduced(input)
        | Logical::Slice { input, .. } => subst_node(input, to, pending),
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Class {
    Indep,
    GForm,
    Bad,
}

fn join_class(a: Class, b: Class) -> Class {
    match (a, b) {
        (Class::Bad, _) | (_, Class::Bad) => Class::Bad,
        (Class::GForm, _) | (_, Class::GForm) => Class::GForm,
        _ => Class::Indep,
    }
}

/// Class of a quad pattern / path whose graph is `gs` and whose other positions are `terms`.
fn pattern_class(gs: GraphSel, terms: &[PatTerm], g: Slot) -> Class {
    if gs == GraphSel::Active || gs == GraphSel::Slot(g) {
        Class::GForm
    } else if terms.contains(&PatTerm::Slot(g)) {
        Class::Bad
    } else {
        Class::Indep
    }
}

fn expr_blocks(e: &Expr, g: Slot) -> bool {
    e.contains_exists() || expr::slots(e).contains(g)
}

/// Classify the body of `GRAPH ?g { l }` (see module docs).
pub(crate) fn classify(l: &Logical, g: Slot, lp: &LogicalProps<'_>) -> Class {
    match l {
        Logical::Unit | Logical::Empty => Class::Indep,
        Logical::Bgp(pats) => pats
            .iter()
            .map(|p| pattern_class(p.g, &[p.s, p.p, p.o], g))
            .fold(Class::Indep, join_class),
        Logical::Path { g: gs, s, o, .. } => pattern_class(*gs, &[*s, *o], g),
        Logical::Values { slots, .. } => {
            if slots.contains(&g) {
                Class::Bad
            } else {
                Class::Indep
            }
        }
        Logical::Join(a, b) => join_class(classify(a, g, lp), classify(b, g, lp)),
        Logical::LeftJoin { left, right, filter } => {
            if filter.as_ref().is_some_and(|f| expr_blocks(f, g)) {
                return Class::Bad;
            }
            match (classify(left, g, lp), classify(right, g, lp)) {
                (Class::Bad, _) | (_, Class::Bad) | (Class::Indep, Class::GForm) => Class::Bad,
                (Class::GForm, _) => Class::GForm,
                (Class::Indep, Class::Indep) => Class::Indep,
            }
        }
        Logical::Union(a, b) => match (classify(a, g, lp), classify(b, g, lp)) {
            (Class::GForm, Class::GForm) => Class::GForm,
            (Class::Indep, Class::Indep) => Class::Indep,
            _ => Class::Bad,
        },
        Logical::Filter { input, expr } => {
            if expr_blocks(expr, g) {
                Class::Bad
            } else {
                classify(input, g, lp)
            }
        }
        Logical::Extend { input, slot, expr } => {
            if *slot == g || expr_blocks(expr, g) {
                Class::Bad
            } else {
                classify(input, g, lp)
            }
        }
        Logical::Graph { name, input } => match name {
            // A nested GRAPH ?g ranges over the named graphs and binds ?g itself.
            PatTerm::Slot(h) if *h == g => Class::GForm,
            _ => {
                if lp.props(input).vars.contains(g) {
                    Class::Bad
                } else {
                    Class::Indep
                }
            }
        },
        Logical::Minus { .. }
        | Logical::Group { .. }
        | Logical::OrderBy { .. }
        | Logical::Project { .. }
        | Logical::Distinct(_)
        | Logical::Reduced(_)
        | Logical::Slice { .. }
        | Logical::Service { .. } => Class::Bad,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sq_core::id::Tag;
    use sq_core::ir::QuadPat;

    const G: Slot = 9;

    fn c(n: i64) -> PatTerm {
        PatTerm::Const(TermId::from_dict(2000 + n, Tag::Iri))
    }
    fn active(s: PatTerm, p: PatTerm, o: PatTerm) -> Logical {
        Logical::Bgp(vec![QuadPat { g: GraphSel::Active, s, p, o }])
    }
    fn class(l: &Logical) -> Class {
        classify(l, G, &LogicalProps::new(&[]))
    }

    #[test]
    fn classification() {
        let a = active(PatTerm::Slot(0), c(1), PatTerm::Slot(1));
        assert_eq!(class(&a), Class::GForm);
        assert_eq!(class(&active(PatTerm::Slot(G), c(1), PatTerm::Slot(1))), Class::GForm);
        let values = Logical::Values { slots: vec![0], rows: vec![vec![TermId::from_int(1).unwrap()]] };
        assert_eq!(class(&values), Class::Indep);
        assert_eq!(class(&Logical::Values { slots: vec![G], rows: vec![] }), Class::Bad);
        assert_eq!(class(&Logical::Join(Box::new(a.clone()), Box::new(values.clone()))), Class::GForm);
        assert_eq!(class(&Logical::Union(Box::new(a.clone()), Box::new(values.clone()))), Class::Bad);
        let lj = |l: &Logical, r: &Logical| Logical::LeftJoin {
            left: Box::new(l.clone()),
            right: Box::new(r.clone()),
            filter: None,
        };
        assert_eq!(class(&lj(&a, &values)), Class::GForm);
        assert_eq!(class(&lj(&values, &a)), Class::Bad);
        let minus = Logical::Minus { left: Box::new(a.clone()), right: Box::new(a.clone()) };
        assert_eq!(class(&minus), Class::Bad);
        let bind_g = Logical::Extend { input: Box::new(a.clone()), slot: G, expr: Expr::Slot(0) };
        assert_eq!(class(&bind_g), Class::Bad);
        let filter_g = Logical::Filter { input: Box::new(a.clone()), expr: Expr::Bound(G) };
        assert_eq!(class(&filter_g), Class::Bad);
        let filter_ok = Logical::Filter { input: Box::new(a.clone()), expr: Expr::Bound(0) };
        assert_eq!(class(&filter_ok), Class::GForm);
        assert_eq!(class(&Logical::Unit), Class::Indep);
    }

    #[test]
    fn push_and_loop() {
        let lp = LogicalProps::new(&[]);
        let mut ex = Vec::new();
        let pushed = push_graph_vars(
            Logical::Graph { name: PatTerm::Slot(G), input: Box::new(active(PatTerm::Slot(0), c(1), PatTerm::Slot(1))) },
            &mut ex,
            &lp,
        );
        assert_eq!(
            pushed,
            Logical::Bgp(vec![QuadPat { g: GraphSel::Slot(G), s: PatTerm::Slot(0), p: c(1), o: PatTerm::Slot(1) }])
        );
        let minus = Logical::Graph {
            name: PatTerm::Slot(G),
            input: Box::new(Logical::Minus {
                left: Box::new(active(PatTerm::Slot(0), c(1), PatTerm::Slot(1))),
                right: Box::new(active(PatTerm::Slot(0), c(2), PatTerm::Slot(1))),
            }),
        };
        assert_eq!(push_graph_vars(minus.clone(), &mut ex, &lp), minus);
    }

    #[test]
    fn const_graphs() {
        let named = [TermId::from_dict(2001, Tag::Iri)];
        let mut ex = vec![active(PatTerm::Slot(3), c(5), PatTerm::Slot(4))];
        let inner = Logical::Filter {
            input: Box::new(active(PatTerm::Slot(0), c(1), PatTerm::Slot(1))),
            expr: Expr::Exists(ExistsId(0)),
        };
        let present = Logical::Graph { name: c(1), input: Box::new(inner.clone()) };
        let out = resolve_const_graphs(present, &mut ex, &named);
        let Logical::Filter { input, .. } = out else { panic!("expected a filter") };
        let Logical::Bgp(p) = *input else { panic!("expected a bgp") };
        assert_eq!(p[0].g, GraphSel::Const(named[0]));
        let Logical::Bgp(p) = &ex[0] else { panic!("expected a bgp") };
        assert_eq!(p[0].g, GraphSel::Const(named[0]), "EXISTS patterns follow their graph");
        let absent = Logical::Graph { name: c(7), input: Box::new(Logical::Unit) };
        assert_eq!(resolve_const_graphs(absent, &mut ex, &named), Logical::Empty);
    }
}
