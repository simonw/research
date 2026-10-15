//! Plan -> operator tree. Every planned operator is wrapped in [`Instrumented`] (profile
//! counters per `Plan::id`, interrupt polling); expressions are compiled once here.
use sq_core::eval_api::TempDict;
use sq_core::plan::{Op, Plan};
use sq_core::{Error, Result, Slot, SlotSet, TermId};

use crate::ops::bind_join::BindJoin;
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

impl Builder<'_> {
    /// Checks that every slot of a plan tree is inside the row layout (once, at the root).
    pub fn check_layout(&self, p: &Plan) -> Result<()> {
        let mut slots = SlotSet::EMPTY;
        crate::exists::collect(p, &mut slots, &mut Vec::new());
        match slots.iter().find(|s| *s as usize >= self.width) {
            Some(s) => Err(Error::internal(format!("plan #{}: slot {s} outside the row layout of width {}", p.id, self.width))),
            None => Ok(()),
        }
    }

    fn expr(&self, e: &sq_core::ir::Expr) -> Result<sq_expr::CompiledExpr> {
        sq_expr::compile(e, self.temps)
    }

    pub fn build(&self, p: &Plan) -> Result<BoxOp> {
        let w = self.width;
        let op: BoxOp = match &p.op {
            Op::Unit => Box::new(Unit::new(w)),
            Op::Empty => Box::new(Empty),
            Op::Scan(s) => Box::new(Scan::new(Some(p.id), s.pat, s.order, s.range, w)),
            Op::Values { slots, rows } => Box::new(Values::new(slots.clone(), rows.clone(), w)?),
            Op::BindJoin { left, right, seed, kind, sort_left, adaptive } => Box::new(BindJoin::new(
                self.build(left)?,
                self.build(right)?,
                seed.clone(),
                JoinMode::compile(kind, self.temps)?,
                *sort_left,
                adaptive.then_some(left.est_rows),
                w,
            )),
            Op::HashJoin { build, probe, keys, check, kind, build_is_left } => Box::new(HashJoin::new(
                self.build(build)?,
                self.build(probe)?,
                keys.clone(),
                check.clone(),
                JoinMode::compile(kind, self.temps)?,
                *build_is_left,
                w,
            )),
            Op::MergeJoin { left, right, key, check, gallop } => {
                Box::new(MergeJoin::new(self.build(left)?, self.build(right)?, *key, check.clone(), *gallop, w))
            }
            Op::StarMerge { key, inputs, check } => {
                if inputs.is_empty() {
                    return Err(Error::internal(format!("plan node #{}: StarMerge without inputs", p.id)));
                }
                let ops = inputs.iter().map(|i| self.build(i)).collect::<Result<Vec<_>>>()?;
                Box::new(StarMerge::new(ops, *key, check.clone(), w))
            }
            // A nested loop is a hash join without keys: every right row is a candidate.
            Op::NestedLoop { left, right, check, kind } => Box::new(HashJoin::new(
                self.build(right)?,
                self.build(left)?,
                Vec::new(),
                check.clone(),
                JoinMode::compile(kind, self.temps)?,
                false,
                w,
            )),
            Op::Minus { left, right, keys, check } => {
                Box::new(Minus::new(self.build(left)?, self.build(right)?, keys.clone(), check.clone(), w))
            }
            Op::Union(branches) => {
                Box::new(Union::new(branches.iter().map(|b| self.build(b)).collect::<Result<Vec<_>>>()?, w))
            }
            Op::Filter { input, expr } => Box::new(Filter::new(self.build(input)?, self.expr(expr)?, w)),
            Op::Extend { input, slot, expr, canonical } => {
                Box::new(Extend::new(self.build(input)?, *slot, self.expr(expr)?, *canonical, TermId::UNBOUND, w))
            }
            Op::Path { g, s, path, o } => Box::new(PathOp::new(*g, *s, path, *o, w)),
            Op::GraphLoop { slot, input } => Box::new(GraphLoop::new(*slot, self.build(input)?, w)),
            Op::GraphNames { slot } => Box::new(GraphNames::new(*slot, w)),
            Op::Group { input, keys, aggs, streaming } => {
                let compiled = aggs
                    .iter()
                    .map(|(slot, spec)| {
                        Ok(Agg { slot: *slot, spec: spec.clone(), arg: spec.arg.as_ref().map(|e| self.expr(e)).transpose()? })
                    })
                    .collect::<Result<Vec<_>>>()?;
                Box::new(Group::new(self.group_input(input, keys)?, keys.clone(), compiled, *streaming, w))
            }
            Op::Count { input, out } => Box::new(Count::new(self.build(input)?, *out, w)),
            Op::Sort { input, keys } => {
                let keys = keys.iter().map(|k| SortSpec::compile(k, self.temps)).collect::<Result<Vec<_>>>()?;
                Box::new(Sort::new(self.build(input)?, keys, w))
            }
            Op::SortBySlot { input, key } => Box::new(SortBySlot::new(self.build(input)?, *key, w)),
            Op::TopK { input, keys, k } => {
                let keys = keys.iter().map(|k| SortSpec::compile(k, self.temps)).collect::<Result<Vec<_>>>()?;
                Box::new(TopK::new(self.build(input)?, keys, *k, w))
            }
            Op::Distinct { input, slots } => Box::new(Distinct::new(self.build(input)?, slots.clone(), w)),
            Op::Reduced { input, slots } => Box::new(Reduced::new(self.build(input)?, slots.clone(), w)),
            Op::Slice { input, offset, limit } => Box::new(Slice::new(self.build(input)?, *offset, *limit, w)),
            Op::Project { input, slots } => Box::new(Project::new(self.build(input)?, slots, w)),
            Op::SqlJoin { sql, params, out } => Box::new(SqlJoin::new(sql.clone(), params.clone(), out.clone(), w)),
        };
        Ok(Box::new(Instrumented { id: p.id, inner: op }))
    }

    /// The input of a Group: the Extends directly below it that compute GROUP BY keys write
    /// `TermId::ERROR` on an error, so that an erroring key forms its own group.
    fn group_input(&self, input: &Plan, keys: &[Slot]) -> Result<BoxOp> {
        match &input.op {
            Op::Extend { input: inner, slot, expr, canonical } if keys.contains(slot) => {
                let op = Extend::new(self.group_input(inner, keys)?, *slot, self.expr(expr)?, *canonical, TermId::ERROR, self.width);
                Ok(Box::new(Instrumented { id: input.id, inner: Box::new(op) }))
            }
            _ => self.build(input),
        }
    }
}
