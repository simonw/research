//! Plan -> operator tree. Every planned operator is wrapped in [`Instrumented`] (profile
//! counters per `Plan::id`, interrupt polling); expressions are compiled once here.
use sq_core::eval_api::TempDict;
use sq_core::ir::Expr;
use sq_core::plan::{JoinKind, Op, Plan};
use sq_core::spargebra::algebra::Function;
use sq_core::{Error, Result, Slot, SlotSet, TermId};

use crate::ops::bind_join::{BindJoin, HashSwitch};
use crate::ops::filter::{Extend, Filter};
use crate::ops::graph::GraphLoop;
use crate::ops::group::{Agg, Count, Group};
use crate::ops::hash_join::{HashJoin, JoinMode};
use crate::ops::leaf::{Empty, GraphNames, SqlJoin, Unit, Values};
use crate::ops::merge_join::{MergeJoin, StarMerge};
use crate::ops::minus::Minus;
use crate::ops::modifiers::{Distinct, Project, Reduced, Slice};
use crate::ops::path::PathOp;
use crate::ops::scan::Scan;
use crate::ops::sort::{Sort, SortBySlot, SortSpec, TopK};
use crate::ops::union::Union;
use crate::ops::{BoxOp, Instrumented};

pub(crate) struct Builder<'a> {
    pub temps: &'a TempDict,
    pub width: usize,
}

/// Whether the planner's sortedness claims (`Plan::sorted_by`) hold in the subtree being built.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Sortedness {
    /// The subtree runs with the seeds it was planned for.
    Planned,
    /// The subtree runs without them: the right side of a BindJoin evaluated once for a hash
    /// join. A scan whose index prefix held a seeded slot then yields another order, so every
    /// operator that needs sorted input gets a SortBySlot enforcer (merge joins) or does not
    /// rely on the order (hash aggregation instead of streaming).
    Unknown,
}

impl Builder<'_> {
    /// Checks that every slot of a plan tree is inside the row layout (once, at the root).
    pub fn check_layout(&self, p: &Plan) -> Result<()> {
        let mut slots = SlotSet::EMPTY;
        crate::exists::collect(p, &mut slots, &mut Vec::new());
        match slots.iter().find(|s| *s as usize >= self.width) {
            Some(s) => {
                Err(Error::internal(format!("plan #{}: slot {s} outside the row layout of width {}", p.id, self.width)))
            }
            None => Ok(()),
        }
    }

    fn expr(&self, e: &sq_core::ir::Expr) -> Result<sq_expr::CompiledExpr> {
        sq_expr::compile(e, self.temps)
    }

    pub fn build(&self, p: &Plan) -> Result<BoxOp> {
        self.build_in(p, Sortedness::Planned)
    }

    /// An input that must be sorted by `key`: as planned, or behind an enforcer.
    fn sorted_input(&self, p: &Plan, key: Slot, mode: Sortedness) -> Result<BoxOp> {
        let op = self.build_in(p, mode)?;
        Ok(match mode {
            Sortedness::Planned => op,
            Sortedness::Unknown => Box::new(SortBySlot::new(op, key, self.width)),
        })
    }

    fn build_in(&self, p: &Plan, mode: Sortedness) -> Result<BoxOp> {
        let w = self.width;
        let build = |p: &Plan| self.build_in(p, mode);
        let op: BoxOp = match &p.op {
            Op::Unit => Box::new(Unit::new(w)),
            Op::Empty => Box::new(Empty),
            Op::Scan(s) => Box::new(Scan::new(Some(p.id), s.pat, s.order, s.range, w)),
            Op::Values { slots, rows } => Box::new(Values::new(slots.clone(), rows.clone(), w)?),
            Op::BindJoin { left, right, seed, kind, sort_left, adaptive } => {
                // Right-side values reach the output of inner and left joins only. The switch
                // to hashing evaluates `right` without seeds, which a right side planned as
                // seeded and relying on sorted inputs only survives when built for it.
                let (switch, right_mode) =
                    if matches!(kind, JoinKind::Inner | JoinKind::Left { .. }) && mints_values(right) {
                        (HashSwitch::Immediate, Sortedness::Unknown)
                    } else if *adaptive && (mode == Sortedness::Unknown || !needs_sorted_input(right)) {
                        (HashSwitch::Adaptive(left.est_rows), mode)
                    } else {
                        (HashSwitch::Never, mode)
                    };
                Box::new(BindJoin::new(
                    build(left)?,
                    self.build_in(right, right_mode)?,
                    seed.clone(),
                    JoinMode::compile(kind, self.temps)?,
                    *sort_left,
                    switch,
                    w,
                ))
            }
            Op::HashJoin { build: b, probe, keys, check, kind, build_is_left } => Box::new(HashJoin::new(
                build(b)?,
                build(probe)?,
                keys.clone(),
                check.clone(),
                JoinMode::compile(kind, self.temps)?,
                *build_is_left,
                w,
            )),
            Op::MergeJoin { left, right, key, check, gallop } => Box::new(MergeJoin::new(
                self.sorted_input(left, *key, mode)?,
                self.sorted_input(right, *key, mode)?,
                *key,
                check.clone(),
                *gallop,
                w,
            )),
            Op::StarMerge { key, inputs, check } => {
                if inputs.is_empty() {
                    return Err(Error::internal(format!("plan node #{}: StarMerge without inputs", p.id)));
                }
                let ops = inputs.iter().map(|i| self.sorted_input(i, *key, mode)).collect::<Result<Vec<_>>>()?;
                Box::new(StarMerge::new(ops, *key, check.clone(), w))
            }
            // A nested loop is a hash join without keys: every right row is a candidate.
            Op::NestedLoop { left, right, check, kind } => Box::new(HashJoin::new(
                build(right)?,
                build(left)?,
                Vec::new(),
                check.clone(),
                JoinMode::compile(kind, self.temps)?,
                false,
                w,
            )),
            Op::Minus { left, right, keys, check } => {
                Box::new(Minus::new(build(left)?, build(right)?, keys.clone(), check.clone(), w))
            }
            Op::Union(branches) => Box::new(Union::new(branches.iter().map(build).collect::<Result<Vec<_>>>()?, w)),
            Op::Filter { input, expr } => Box::new(Filter::new(build(input)?, self.expr(expr)?, w)),
            Op::Extend { input, slot, expr, canonical } => {
                Box::new(Extend::new(build(input)?, *slot, self.expr(expr)?, *canonical, TermId::UNBOUND, w))
            }
            Op::Path { g, s, path, o } => Box::new(PathOp::new(*g, *s, path, *o, w)),
            Op::GraphLoop { slot, input } => Box::new(GraphLoop::new(*slot, build(input)?, w)),
            Op::GraphNames { slot } => Box::new(GraphNames::new(*slot, w)),
            Op::Group { input, keys, aggs, streaming } => {
                let compiled = aggs
                    .iter()
                    .map(|(slot, spec)| Ok(Agg::new(*slot, spec, spec.arg.as_ref().map(|e| self.expr(e)).transpose()?)))
                    .collect::<Result<Vec<_>>>()?;
                let streaming = *streaming && mode == Sortedness::Planned;
                Box::new(Group::new(self.group_input(input, keys, mode)?, keys.clone(), compiled, streaming, w))
            }
            Op::Count { input, out } => Box::new(Count::new(build(input)?, *out, w)),
            Op::Sort { input, keys } => {
                let keys = keys.iter().map(|k| SortSpec::compile(k, self.temps)).collect::<Result<Vec<_>>>()?;
                Box::new(Sort::new(build(input)?, keys, w))
            }
            Op::SortBySlot { input, key } => Box::new(SortBySlot::new(build(input)?, *key, w)),
            Op::TopK { input, keys, k } => {
                let keys = keys.iter().map(|k| SortSpec::compile(k, self.temps)).collect::<Result<Vec<_>>>()?;
                Box::new(TopK::new(build(input)?, keys, *k, w))
            }
            Op::Distinct { input, slots } => Box::new(Distinct::new(build(input)?, slots.clone(), w)),
            Op::Reduced { input, slots } => Box::new(Reduced::new(build(input)?, slots.clone(), w)),
            Op::Slice { input, offset, limit } => Box::new(Slice::new(build(input)?, *offset, *limit, w)),
            Op::Project { input, slots } => Box::new(Project::new(build(input)?, slots, w)),
            Op::SqlJoin { sql, params, out } => Box::new(SqlJoin::new(sql.clone(), params.clone(), out.clone(), w)),
        };
        Ok(Box::new(Instrumented { id: p.id, inner: op }))
    }

    /// The input of a Group: the Extends directly below it that compute GROUP BY keys write
    /// `TermId::ERROR` on an error, so that an erroring key forms its own group.
    fn group_input(&self, input: &Plan, keys: &[Slot], mode: Sortedness) -> Result<BoxOp> {
        match &input.op {
            Op::Extend { input: inner, slot, expr, canonical } if keys.contains(slot) => {
                let op = Extend::new(
                    self.group_input(inner, keys, mode)?,
                    *slot,
                    self.expr(expr)?,
                    *canonical,
                    TermId::ERROR,
                    self.width,
                );
                Ok(Box::new(Instrumented { id: input.id, inner: Box::new(op) }))
            }
            _ => self.build_in(input, mode),
        }
    }
}

/// Does the subtree contain an operator that relies on a sorted input (merge joins, streaming
/// aggregation)?
fn needs_sorted_input(p: &Plan) -> bool {
    matches!(p.op, Op::MergeJoin { .. } | Op::StarMerge { .. } | Op::Group { streaming: true, .. })
        || p.children().into_iter().any(needs_sorted_input)
}

/// Does evaluating `p` call a function that returns a new value on every call (BNODE, RAND,
/// UUID, STRUUID)? Each evaluation of such a subtree gives its solutions different values, so
/// one evaluation must not be replaced by several (a BindJoin re-opening it per seed tuple).
/// EXISTS patterns are not inspected: they only contribute a boolean per row.
pub(crate) fn mints_values(p: &Plan) -> bool {
    let here = match &p.op {
        Op::Filter { expr, .. } | Op::Extend { expr, .. } => expr_mints(expr),
        Op::BindJoin { kind, .. } | Op::HashJoin { kind, .. } | Op::NestedLoop { kind, .. } => {
            matches!(kind, JoinKind::Left { filter: Some(f) } if expr_mints(f))
        }
        Op::Group { aggs, .. } => aggs.iter().any(|(_, a)| a.arg.as_ref().is_some_and(expr_mints)),
        Op::Sort { keys, .. } | Op::TopK { keys, .. } => keys.iter().any(|k| expr_mints(&k.expr)),
        Op::Unit
        | Op::Empty
        | Op::Scan(_)
        | Op::Values { .. }
        | Op::MergeJoin { .. }
        | Op::StarMerge { .. }
        | Op::Minus { .. }
        | Op::Union(_)
        | Op::Path { .. }
        | Op::GraphLoop { .. }
        | Op::GraphNames { .. }
        | Op::Count { .. }
        | Op::SortBySlot { .. }
        | Op::Distinct { .. }
        | Op::Reduced { .. }
        | Op::Slice { .. }
        | Op::Project { .. }
        | Op::SqlJoin { .. } => false,
    };
    here || p.children().into_iter().any(mints_values)
}

fn expr_mints(e: &Expr) -> bool {
    match e {
        Expr::Call(Function::BNode | Function::Rand | Function::Uuid | Function::StrUuid, _) => true,
        Expr::Const(_) | Expr::Slot(_) | Expr::Bound(_) | Expr::Exists(_) => false,
        Expr::Not(a) | Expr::UnaryPlus(a) | Expr::UnaryMinus(a) => expr_mints(a),
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
        | Expr::Divide(a, b) => expr_mints(a) || expr_mints(b),
        Expr::In(a, l) => expr_mints(a) || l.iter().any(expr_mints),
        Expr::If(a, b, c) => expr_mints(a) || expr_mints(b) || expr_mints(c),
        Expr::Coalesce(l) | Expr::Call(_, l) => l.iter().any(expr_mints),
    }
}
