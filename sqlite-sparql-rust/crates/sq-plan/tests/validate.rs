//! `validate()` rejects corrupted plans: every check is exercised by mutating a valid plan.
mod common;

use common::*;
use sq_core::ir::{GraphSel, PatTerm};
use sq_core::plan::{JoinKind, Op, Plan, QueryPlan};
use sq_core::store::IndexOrder;

fn children_mut(p: &mut Plan) -> Vec<&mut Plan> {
    match &mut p.op {
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
        _ => vec![],
    }
}

/// Apply `f` to the first node (pre-order) for which it returns true.
fn mutate(p: &mut Plan, f: &mut impl FnMut(&mut Plan) -> bool) -> bool {
    if f(p) {
        return true;
    }
    for c in children_mut(p) {
        if mutate(c, f) {
            return true;
        }
    }
    false
}

fn renumber(q: &mut QueryPlan) {
    fn go(p: &mut Plan, next: &mut u32) {
        p.id = *next;
        *next += 1;
        for c in children_mut(p) {
            go(c, next);
        }
    }
    let mut n = 0;
    go(&mut q.root, &mut n);
    for e in &mut q.exists {
        go(e, &mut n);
    }
}

fn rejects(mut q: QueryPlan, what: &str, f: impl FnMut(&mut Plan) -> bool) {
    let mut f = f;
    assert!(mutate(&mut q.root, &mut f), "{what}: nothing to mutate");
    renumber(&mut q);
    let err = sq_plan::validate(&q).expect_err(what);
    assert!(matches!(err, sq_core::Error::Internal(_)), "{what}: {err}");
}

#[test]
fn rejects_corrupted_plans() {
    let st = social(30);

    // Unsafe BindJoin seed: a LIMIT sub-select on the optional side.
    let q = plan("SELECT * { ?x a :C1 OPTIONAL { SELECT ?x ?y { ?x :knows ?y } LIMIT 1 } }", &st);
    let x = slot(&q, "x");
    rejects(q, "unsafe seed", |p| {
        let Op::HashJoin { build, probe, kind, .. } = &p.op else { return false };
        let (left, right) = (probe.clone(), build.clone());
        p.op = Op::BindJoin { left, right, seed: vec![x], kind: kind.clone(), sort_left: false, adaptive: true };
        true
    });

    // A maybe-bound shared slot that is neither key nor checked.
    let q = plan("SELECT * { ?x :knows ?y OPTIONAL { ?x :age ?z } ?w :age ?z }", &st);
    assert!(nodes(&q).iter().any(|n| matches!(&n.op, Op::NestedLoop { check, .. } | Op::HashJoin { check, .. } if !check.is_empty())));
    rejects(q, "missing check", |p| match &mut p.op {
        Op::NestedLoop { check, kind: JoinKind::Inner, .. } | Op::HashJoin { check, kind: JoinKind::Inner, .. }
            if !check.is_empty() =>
        {
            check.clear();
            true
        }
        _ => false,
    });

    // BindJoin seed that is not certain on the left.
    let q = plan("SELECT * { ?x a :C1 . ?x :age ?a }", &st);
    let a = slot(&q, "a");
    rejects(q, "seed not certain on the left", |p| match &mut p.op {
        Op::BindJoin { seed, .. } => {
            seed.push(a + 100);
            true
        }
        _ => false,
    });

    // Extend overwriting a bound slot.
    let q = plan("SELECT * { ?x :age ?a BIND(?a + 1 AS ?b) }", &st);
    let a = slot(&q, "a");
    rejects(q, "Extend overwrites", |p| match &mut p.op {
        Op::Extend { slot, .. } => {
            *slot = a;
            true
        }
        _ => false,
    });

    // Non-canonical id feeding DISTINCT.
    let q = plan("SELECT DISTINCT ?b { ?x :age ?a BIND(?a + 1 AS ?b) }", &st);
    rejects(q, "canonical flag", |p| match &mut p.op {
        Op::Extend { canonical, .. } => {
            *canonical = false;
            true
        }
        _ => false,
    });

    // Distinct that ignores a slot its input may bind.
    let q = plan("SELECT DISTINCT ?x ?a { ?x :age ?a }", &st);
    rejects(q, "Distinct slots", |p| match &mut p.op {
        Op::Distinct { slots, .. } => {
            slots.pop();
            true
        }
        _ => false,
    });

    // Active graph outside a GraphLoop.
    let q = plan("SELECT * { ?x :age ?a }", &st);
    rejects(q, "active outside loop", |p| match &mut p.op {
        Op::Scan(s) => {
            s.pat.g = GraphSel::Active;
            true
        }
        _ => false,
    });

    // Scan order that does not start with the bound columns.
    let q = plan("SELECT * { ?x :age ?a }", &st);
    rejects(q, "scan order", |p| match &mut p.op {
        Op::Scan(s) => {
            s.order = IndexOrder::Gosp;
            true
        }
        _ => false,
    });

    // A sortedness claim that is not guaranteed.
    let q = plan("SELECT * { ?x :age ?a }", &st);
    let a = slot(&q, "a");
    rejects(q, "sorted_by", |p| match &p.op {
        Op::Scan(_) => {
            p.sorted_by = Some(a);
            true
        }
        _ => false,
    });

    // Slots outside the layout (never a panic, even where slot sets are built).
    let q = plan("SELECT * { ?x :age ?a FILTER(?a > 3) }", &st);
    rejects(q, "slot range", |p| match &mut p.op {
        Op::Filter { expr, .. } => {
            *expr = sq_core::ir::Expr::SameTerm(
                Box::new(sq_core::ir::Expr::Slot(4000)),
                Box::new(sq_core::ir::Expr::Slot(300)),
            );
            true
        }
        _ => false,
    });
    let q = plan("SELECT * { ?x :age ?a }", &st);
    rejects(q, "scan slot range", |p| match &mut p.op {
        Op::Scan(s) => {
            s.pat.o = PatTerm::Slot(999);
            true
        }
        _ => false,
    });
}

#[test]
fn rejects_seed_slots_read_from_rows_that_do_not_carry_them() {
    // OPTIONAL { { SELECT ?a ... LIMIT 1 } ?a :knows ?v } under a BindJoin seeded with ?v: the
    // LIMIT sub-select never receives the seed, so its rows have no ?v to pass on as a seed.
    let st = social(30);
    let text = "SELECT * { ?v :name \"Person 5\" \
                OPTIONAL { { SELECT ?a { ?a :name \"Person 1\" } LIMIT 1 } ?a :knows ?v } }";
    // PreferHash keeps the inner join a HashJoin with the sub-select as its left input.
    let hash = sq_core::plan::QueryOptions { join_policy: sq_core::plan::JoinPolicy::PreferHash, ..Default::default() };
    let q = plan_with(text, &st, &hash);
    let (a, v) = (slot(&q, "a"), slot(&q, "v"));
    let as_bind_join = |p: &mut Plan, seed: Vec<u16>| {
        let Op::HashJoin { build, probe, kind: JoinKind::Inner, .. } = &p.op else { return false };
        // The sub-select side becomes the BindJoin's left input, whichever side it was built on.
        let (left, right) = match (&build.op, &probe.op) {
            (Op::Slice { .. }, _) => (build.clone(), probe.clone()),
            (_, Op::Slice { .. }) => (probe.clone(), build.clone()),
            _ => return false,
        };
        p.op = Op::BindJoin { left, right, seed, kind: JoinKind::Inner, sort_left: false, adaptive: true };
        true
    };
    rejects(q.clone(), "seed read from a LIMIT sub-select", |p| as_bind_join(p, vec![a, v]));
    // Without ?v in the seed list the right side would bind ?v freely: rejected as well.
    rejects(q, "seeded slot missing from the seed list", |p| as_bind_join(p, vec![a]));
}

#[test]
fn rejects_wrong_ids() {
    let st = social(10);
    let mut q = plan("SELECT * { ?x :age ?a FILTER(?a > 3) }", &st);
    q.root.id = 7;
    assert!(sq_plan::validate(&q).is_err());
}
