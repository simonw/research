//! Physical post-passes: pre-order plan ids and canonical flags (spec §7.3.8).
use sq_core::batch::SlotSet;
use sq_core::ir::{Expr, PatTerm, QueryForm, TemplateTerm};
use sq_core::plan::{JoinKind, Op, Plan, QueryPlan};

use crate::expr;
use crate::props::{pattern_slots, PlanProps};

/// Mutable direct children of a plan node.
pub(crate) fn children_mut(p: &mut Plan) -> Vec<&mut Plan> {
    match &mut p.op {
        Op::Unit | Op::Empty | Op::Scan(_) | Op::Values { .. } | Op::Path { .. } | Op::GraphNames { .. } | Op::SqlJoin { .. } => {
            vec![]
        }
        Op::BindJoin { left, right, .. }
        | Op::MergeJoin { left, right, .. }
        | Op::NestedLoop { left, right, .. }
        | Op::Minus { left, right, .. } => vec![&mut **left, &mut **right],
        Op::HashJoin { build, probe, .. } => vec![&mut **build, &mut **probe],
        Op::StarMerge { inputs, .. } | Op::Union(inputs) => inputs.iter_mut().collect(),
        Op::Filter { input, .. }
        | Op::Extend { input, .. }
        | Op::GraphLoop { input, .. }
        | Op::Group { input, .. }
        | Op::Count { input, .. }
        | Op::Sort { input, .. }
        | Op::SortBySlot { input, .. }
        | Op::TopK { input, .. }
        | Op::Distinct { input, .. }
        | Op::Reduced { input, .. }
        | Op::Slice { input, .. }
        | Op::Project { input, .. } => vec![&mut **input],
    }
}

/// Number the root plan, then each EXISTS plan, in pre-order: ids are unique in the QueryPlan.
pub(crate) fn assign_ids(q: &mut QueryPlan) {
    fn number(p: &mut Plan, next: &mut u32) {
        p.id = *next;
        *next += 1;
        for c in children_mut(p) {
            number(c, next);
        }
    }
    let mut next = 0;
    number(&mut q.root, &mut next);
    for e in &mut q.exists {
        number(e, &mut next);
    }
}

/// Expressions held directly by a plan node.
pub(crate) fn node_exprs(p: &Plan) -> Vec<&Expr> {
    match &p.op {
        Op::Filter { expr, .. } | Op::Extend { expr, .. } => vec![expr],
        Op::BindJoin { kind: JoinKind::Left { filter: Some(f) }, .. }
        | Op::HashJoin { kind: JoinKind::Left { filter: Some(f) }, .. }
        | Op::NestedLoop { kind: JoinKind::Left { filter: Some(f) }, .. } => vec![f],
        Op::Group { aggs, .. } => aggs.iter().filter_map(|(_, a)| a.arg.as_ref()).collect(),
        Op::Sort { keys, .. } | Op::TopK { keys, .. } => keys.iter().map(|k| &k.expr).collect(),
        _ => vec![],
    }
}

/// Pre-order walk over a plan tree.
pub(crate) fn walk<'p>(p: &'p Plan, f: &mut impl FnMut(&'p Plan)) {
    f(p);
    for c in p.children() {
        walk(c, f);
    }
}

/// Slots whose ids must be canonical: every consumer that compares ids (§7.3.8) — scans, paths,
/// Values, join keys / checks / seeds, SortBySlot keys, Minus, Distinct / Reduced, group keys,
/// aggregate arguments, GraphLoop / GraphNames, sameTerm, EXISTS correlation, CONSTRUCT /
/// DESCRIBE — plus every slot an Extend of such a slot reads (pass-through expressions keep ids
/// as they are).
pub(crate) fn canonical_needed(q: &QueryPlan) -> SlotSet {
    let pp = PlanProps::new(&q.exists);
    let mut need = SlotSet::EMPTY;
    let mut deps: Vec<(sq_core::batch::Slot, SlotSet)> = Vec::new();
    let add = |need: &mut SlotSet, slots: &[sq_core::batch::Slot]| {
        for s in slots {
            need.insert(*s);
        }
    };
    let mut visit = |p: &Plan| {
        match &p.op {
            Op::Scan(s) => pattern_slots(&s.pat).for_each(|x| need.insert(x)),
            Op::Path { g, s, o, .. } => {
                let pat = sq_core::ir::QuadPat { g: *g, s: *s, p: PatTerm::Const(sq_core::TermId::UNBOUND), o: *o };
                pattern_slots(&pat).for_each(|x| need.insert(x));
            }
            Op::Values { slots, .. }
            | Op::Distinct { slots, .. }
            | Op::Reduced { slots, .. }
            | Op::SqlJoin { out: slots, .. } => add(&mut need, slots),
            Op::BindJoin { seed, .. } => add(&mut need, seed),
            Op::HashJoin { keys, check, .. } | Op::Minus { keys, check, .. } => {
                add(&mut need, keys);
                add(&mut need, check);
            }
            Op::MergeJoin { key, check, .. } | Op::StarMerge { key, check, .. } => {
                need.insert(*key);
                add(&mut need, check);
            }
            Op::NestedLoop { check, .. } => add(&mut need, check),
            Op::Group { keys, aggs, .. } => {
                add(&mut need, keys);
                for (_, a) in aggs {
                    if let Some(e) = &a.arg {
                        need = need.union(expr::slots(e));
                    }
                }
            }
            Op::GraphLoop { slot, .. } | Op::GraphNames { slot } | Op::SortBySlot { key: slot, .. } => {
                need.insert(*slot)
            }
            Op::Extend { slot, expr, .. } => deps.push((*slot, expr::slots(expr))),
            _ => {}
        }
        for e in node_exprs(p) {
            expr::visit(e, &mut |x| match x {
                Expr::SameTerm(a, b) => need = need.union(expr::slots(a)).union(expr::slots(b)),
                Expr::Exists(id) => need = need.union(pp.exists_vars(*id)),
                _ => {}
            });
        }
    };
    walk(&q.root, &mut visit);
    for e in &q.exists {
        walk(e, &mut visit);
    }
    match &q.form {
        QueryForm::Construct(template) => {
            for t in template.iter().flatten() {
                if let TemplateTerm::Slot(s) = t {
                    need.insert(*s);
                }
            }
        }
        QueryForm::Describe(slots) => add(&mut need, slots),
        QueryForm::Select(_) | QueryForm::Ask => {}
    }
    loop {
        let before = need;
        for (slot, reads) in &deps {
            if need.contains(*slot) {
                need = need.union(*reads);
            }
        }
        if need == before {
            return need;
        }
    }
}

/// `Extend.canonical = false` only where the slot provably flows to output / expressions only.
pub(crate) fn set_canonical_flags(q: &mut QueryPlan) {
    let need = canonical_needed(q);
    fn set(p: &mut Plan, need: SlotSet) {
        if let Op::Extend { slot, canonical, .. } = &mut p.op {
            *canonical = need.contains(*slot);
        }
        for c in children_mut(p) {
            set(c, need);
        }
    }
    set(&mut q.root, need);
    for e in &mut q.exists {
        set(e, need);
    }
}
