//! `GRAPH ?g` support analysis.
//!
//! The baseline evaluates `GRAPH ?g { P }` by *pushing* the graph variable into P's quad
//! patterns (every pattern reading the active graph binds `g`, restricted to named graphs).
//! That equals SPARQL's per-graph evaluation only when every solution of P reads the graph
//! through a quad ([`GClass::GForm`]), and ?g is not otherwise used inside P. A body that never
//! reads the active graph ([`GClass::Indep`]) is joined with the list of named graphs instead.
//! Anything mixed ([`GClass::Bad`]) — a UNION whose branches disagree, an OPTIONAL whose left
//! side does not read the graph while its right side does, a sub-select over the graph, a
//! zero-length-capable path — is not supported by the baseline.
//!
//! The same classification decides whether `GRAPH <c> { P }` needs its dataset-membership
//! check: only a [`GClass::GForm`] body proves that c is a graph of the store by itself.
use sq_core::Slot;
use sq_core::ir::{Expr, GraphSel, Logical, PatTerm};

use crate::path::zero_length;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum GClass {
    /// Never reads the active graph.
    Indep,
    /// Every solution binds the active graph through a quad pattern.
    GForm,
    /// Reads the active graph in some solutions only, or may produce solutions that read no
    /// quad of it (a zero-length path pairs a constant endpoint with itself).
    Bad,
}

fn join_class(a: GClass, b: GClass) -> GClass {
    use GClass::*;
    match (a, b) {
        (Bad, _) | (_, Bad) => Bad,
        (GForm, _) | (_, GForm) => GForm,
        _ => Indep,
    }
}

/// OPTIONAL / MINUS: the left side decides which rows exist.
fn left_class(left: GClass, right: GClass, filter_reads: bool) -> GClass {
    use GClass::*;
    match (left, right) {
        (Bad, _) | (_, Bad) => Bad,
        (GForm, _) => GForm,
        (Indep, Indep) if !filter_reads => Indep,
        (Indep, _) => Bad,
    }
}

/// Classifies a pattern with respect to the active graph of its innermost GRAPH.
pub(crate) fn classify(node: &Logical, exists: &[Logical]) -> GClass {
    use GClass::*;
    match node {
        // A nested GRAPH has its own active graph.
        Logical::Unit | Logical::Empty | Logical::Values { .. } | Logical::Service { .. } | Logical::Graph { .. } => {
            Indep
        }
        Logical::Bgp(pats) => {
            if pats.iter().any(|p| p.g == GraphSel::Active) {
                GForm
            } else {
                Indep
            }
        }
        Logical::Path { g, path, .. } => match (*g == GraphSel::Active, zero_length(path)) {
            (false, _) => Indep,
            (true, false) => GForm,
            (true, true) => Bad,
        },
        Logical::Join(a, b) => join_class(classify(a, exists), classify(b, exists)),
        Logical::LeftJoin { left, right, filter } => left_class(
            classify(left, exists),
            classify(right, exists),
            filter.as_ref().is_some_and(|f| expr_reads_active(f, exists)),
        ),
        Logical::Minus { left, right } => left_class(classify(left, exists), classify(right, exists), false),
        Logical::Union(a, b) => match (classify(a, exists), classify(b, exists)) {
            (x, y) if x == y => x,
            _ => Bad,
        },
        Logical::Filter { input, expr } | Logical::Extend { input, expr, .. } => {
            match classify(input, exists) {
                Indep if expr_reads_active(expr, exists) => Bad,
                c => c,
            }
        }
        Logical::Group { .. }
        | Logical::OrderBy { .. }
        | Logical::Project { .. }
        | Logical::Distinct(_)
        | Logical::Reduced(_)
        | Logical::Slice { .. } => {
            if reads_active(node, exists) {
                Bad
            } else {
                Indep
            }
        }
    }
}

/// EXISTS patterns referenced by an expression.
fn exists_patterns<'a>(e: &Expr, exists: &'a [Logical], out: &mut Vec<&'a Logical>) {
    match e {
        Expr::Exists(id) => out.extend(exists.get(id.0 as usize)),
        Expr::Const(_) | Expr::Slot(_) | Expr::Bound(_) => {}
        Expr::Not(a) | Expr::UnaryPlus(a) | Expr::UnaryMinus(a) => exists_patterns(a, exists, out),
        Expr::Or(a, b)
        | Expr::And(a, b)
        | Expr::Equal(a, b)
        | Expr::SameTerm(a, b)
        | Expr::Greater(a, b)
        | Expr::GreaterOrEqual(a, b)
        | Expr::Less(a, b)
        | Expr::LessOrEqual(a, b)
        | Expr::Add(a, b)
        | Expr::Subtract(a, b)
        | Expr::Multiply(a, b)
        | Expr::Divide(a, b) => {
            exists_patterns(a, exists, out);
            exists_patterns(b, exists, out);
        }
        Expr::In(a, l) => {
            exists_patterns(a, exists, out);
            l.iter().for_each(|x| exists_patterns(x, exists, out));
        }
        Expr::If(a, b, c) => {
            exists_patterns(a, exists, out);
            exists_patterns(b, exists, out);
            exists_patterns(c, exists, out);
        }
        Expr::Coalesce(l) | Expr::Call(_, l) => l.iter().for_each(|x| exists_patterns(x, exists, out)),
    }
}

fn expr_reads_active(e: &Expr, exists: &[Logical]) -> bool {
    let mut pats = Vec::new();
    exists_patterns(e, exists, &mut pats);
    pats.into_iter().any(|p| reads_active(p, exists))
}

/// Does any quad pattern or path (outside nested GRAPHs, including EXISTS patterns) read the
/// active graph?
pub(crate) fn reads_active(node: &Logical, exists: &[Logical]) -> bool {
    let r = |n: &Logical| reads_active(n, exists);
    match node {
        Logical::Unit | Logical::Empty | Logical::Values { .. } | Logical::Service { .. } | Logical::Graph { .. } => {
            false
        }
        Logical::Bgp(pats) => pats.iter().any(|p| p.g == GraphSel::Active),
        Logical::Path { g, .. } => *g == GraphSel::Active,
        Logical::Join(a, b) | Logical::Minus { left: a, right: b } | Logical::Union(a, b) => r(a) || r(b),
        Logical::LeftJoin { left, right, filter } => {
            r(left) || r(right) || filter.as_ref().is_some_and(|f| expr_reads_active(f, exists))
        }
        Logical::Filter { input, expr } | Logical::Extend { input, expr, .. } => {
            r(input) || expr_reads_active(expr, exists)
        }
        Logical::Group { input, aggs, .. } => {
            r(input) || aggs.iter().any(|(_, a)| a.arg.as_ref().is_some_and(|e| expr_reads_active(e, exists)))
        }
        Logical::OrderBy { input, keys } => r(input) || keys.iter().any(|k| expr_reads_active(&k.expr, exists)),
        Logical::Project { input, .. }
        | Logical::Distinct(input)
        | Logical::Reduced(input)
        | Logical::Slice { input, .. } => r(input),
    }
}

/// Does the expression read the slot, directly or in one of its EXISTS patterns?
pub(crate) fn expr_mentions(e: &Expr, s: Slot, exists: &[Logical]) -> bool {
    let mut slots = Vec::new();
    e.slots(&mut slots);
    if slots.contains(&s) {
        return true;
    }
    let mut pats = Vec::new();
    exists_patterns(e, exists, &mut pats);
    pats.into_iter().any(|p| mentions(p, s, exists))
}

/// Does the pattern use the slot anywhere (patterns, expressions, EXISTS, VALUES, GRAPH
/// names, projections, group keys)?
pub(crate) fn mentions(node: &Logical, s: Slot, exists: &[Logical]) -> bool {
    let m = |n: &Logical| mentions(n, s, exists);
    let term = |t: &PatTerm| *t == PatTerm::Slot(s);
    match node {
        Logical::Unit | Logical::Empty | Logical::Service { .. } => false,
        Logical::Bgp(pats) => {
            pats.iter().any(|p| term(&p.s) || term(&p.p) || term(&p.o) || p.g == GraphSel::Slot(s))
        }
        Logical::Path { g, s: a, o: b, .. } => term(a) || term(b) || *g == GraphSel::Slot(s),
        Logical::Join(a, b) | Logical::Minus { left: a, right: b } | Logical::Union(a, b) => m(a) || m(b),
        Logical::LeftJoin { left, right, filter } => {
            m(left) || m(right) || filter.as_ref().is_some_and(|f| expr_mentions(f, s, exists))
        }
        Logical::Filter { input, expr } => m(input) || expr_mentions(expr, s, exists),
        Logical::Extend { input, slot, expr } => *slot == s || m(input) || expr_mentions(expr, s, exists),
        Logical::Values { slots, .. } => slots.contains(&s),
        Logical::Graph { name, input } => term(name) || m(input),
        Logical::Group { input, keys, aggs } => {
            keys.contains(&s)
                || m(input)
                || aggs.iter().any(|(out, a)| *out == s || a.arg.as_ref().is_some_and(|e| expr_mentions(e, s, exists)))
        }
        Logical::OrderBy { input, keys } => m(input) || keys.iter().any(|k| expr_mentions(&k.expr, s, exists)),
        Logical::Project { input, slots } => slots.contains(&s) || m(input),
        Logical::Distinct(input) | Logical::Reduced(input) | Logical::Slice { input, .. } => m(input),
    }
}
