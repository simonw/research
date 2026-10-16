//! Structural helpers over `sq_core::ir::Logical`.
use spargebra::algebra::Function;
use sq_core::ir::{Expr, Logical};

/// Rebuild `l` with `f` applied to each direct child (expressions untouched).
pub(crate) fn map_children(l: Logical, f: &mut impl FnMut(Logical) -> Logical) -> Logical {
    let mut b = |x: Box<Logical>| Box::new(f(*x));
    match l {
        Logical::Unit
        | Logical::Empty
        | Logical::Bgp(_)
        | Logical::Path { .. }
        | Logical::Values { .. }
        | Logical::Service { .. } => l,
        Logical::Join(a, c) => {
            let a = b(a);
            Logical::Join(a, b(c))
        }
        Logical::LeftJoin { left, right, filter } => {
            let left = b(left);
            Logical::LeftJoin { left, right: b(right), filter }
        }
        Logical::Minus { left, right } => {
            let left = b(left);
            Logical::Minus { left, right: b(right) }
        }
        Logical::Union(a, c) => {
            let a = b(a);
            Logical::Union(a, b(c))
        }
        Logical::Filter { input, expr } => Logical::Filter { input: b(input), expr },
        Logical::Extend { input, slot, expr } => Logical::Extend { input: b(input), slot, expr },
        Logical::Graph { name, input } => Logical::Graph { name, input: b(input) },
        Logical::Group { input, keys, aggs } => Logical::Group { input: b(input), keys, aggs },
        Logical::OrderBy { input, keys } => Logical::OrderBy { input: b(input), keys },
        Logical::Project { input, slots } => Logical::Project { input: b(input), slots },
        Logical::Distinct(input) => Logical::Distinct(b(input)),
        Logical::Reduced(input) => Logical::Reduced(b(input)),
        Logical::Slice { input, offset, limit } => Logical::Slice { input: b(input), offset, limit },
    }
}

/// Direct children, left to right.
pub(crate) fn children(l: &Logical) -> Vec<&Logical> {
    match l {
        Logical::Unit
        | Logical::Empty
        | Logical::Bgp(_)
        | Logical::Path { .. }
        | Logical::Values { .. }
        | Logical::Service { .. } => vec![],
        Logical::Join(a, b) | Logical::Union(a, b) => vec![a, b],
        Logical::LeftJoin { left, right, .. } | Logical::Minus { left, right } => vec![left, right],
        Logical::Filter { input, .. }
        | Logical::Extend { input, .. }
        | Logical::Graph { input, .. }
        | Logical::Group { input, .. }
        | Logical::OrderBy { input, .. }
        | Logical::Project { input, .. }
        | Logical::Distinct(input)
        | Logical::Reduced(input)
        | Logical::Slice { input, .. } => vec![input],
    }
}

/// Expressions held directly by this node (not by its children).
pub(crate) fn exprs(l: &Logical) -> Vec<&Expr> {
    match l {
        Logical::LeftJoin { filter, .. } => filter.iter().collect(),
        Logical::Filter { expr, .. } | Logical::Extend { expr, .. } => vec![expr],
        Logical::Group { aggs, .. } => aggs.iter().filter_map(|(_, a)| a.arg.as_ref()).collect(),
        Logical::OrderBy { keys, .. } => keys.iter().map(|k| &k.expr).collect(),
        _ => vec![],
    }
}

/// Does evaluating `l` call a function that returns a new value on every call (BNODE, RAND,
/// UUID, STRUUID)? Each evaluation of such a subtree gives its solutions different values, so
/// the planner never re-opens it per probe (BindJoin) where its values reach the output; it is
/// evaluated once instead (HashJoin / NestedLoop), as sq-exec's `build::mints_values` also
/// enforces at run time. EXISTS patterns are not inspected: they only contribute a boolean.
pub(crate) fn mints_values(l: &Logical) -> bool {
    let mut mints = false;
    for e in exprs(l) {
        crate::expr::visit(e, &mut |x| {
            mints |= matches!(
                x,
                Expr::Call(Function::BNode | Function::Rand | Function::Uuid | Function::StrUuid, _)
            );
        });
    }
    mints || children(l).into_iter().any(mints_values)
}

/// Is this node a leaf of a join group that the DP orders directly?
pub(crate) fn is_group_leaf(l: &Logical) -> bool {
    matches!(l, Logical::Bgp(_) | Logical::Path { .. } | Logical::Values { .. } | Logical::Unit | Logical::Empty)
}
