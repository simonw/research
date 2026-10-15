//! Plan validation (spec §7.1 step 6), run in debug builds and in every test.
//!
//! Checks, per node, under the seed propagation contract documented in `physical`:
//! * ids are the pre-order numbering of the root followed by the EXISTS plans;
//! * every slot is inside the layout, every EXISTS id exists;
//! * slots are bound below their uses where operators require it: BindJoin seeds are certain
//!   on the left, HashJoin / Minus keys certain on both sides, MergeJoin keys certain and both
//!   inputs sorted by them, check lists cover every other slot both sides may bind (slots
//!   seeded on both inputs excepted), BindJoin seed lists cover every slot both sides may bind,
//!   Extend never overwrites a bound slot, Distinct / Reduced cover every slot their input may
//!   bind; "certain" counts a seeded slot only where the input's rows carry it
//!   (`physical::carried`), never merely because the subtree was opened with it;
//!   `seed ⊆ safe_inject(right)` for every BindJoin;
//! * filters inside a LeftJoin-right / Minus-right side observe exactly their own scope: those
//!   sides only ever receive seeds that are in `safe_inject` and that they bind themselves
//!   (checked above). That no outer filter was moved INTO such a side cannot be read off a
//!   physical plan (a nested group's own filter may legally read an outer variable, which is
//!   unbound there, and look exactly like a moved one); `optimize` checks it on the filter
//!   placement itself in debug builds (`rewrite::filter::check_barriers`);
//! * `GraphSel::Active` only under a GraphLoop (for EXISTS plans: every evaluation of the
//!   pattern is under one);
//! * `sorted_by` claims are derivable from what the executor guarantees (see `physical`): a
//!   Scan's first free column, merge keys, SortBySlot keys, a BindJoin's left order (unless
//!   batches are re-sorted by another slot), a HashJoin's probe order, a NestedLoop's / Minus'
//!   left order, pass-through operators; a slot carried over from an input must be certain
//!   there (a merge could not overwrite it);
//! * MergeJoin / StarMerge inputs are sorted by the key, which is certain on both sides and
//!   never seeded on both; SortBySlot sorts by a slot its input may bind; a streaming Group has
//!   one key and an input sorted by it;
//! * Scan orders match their bound columns; an IntRange scan reads Gpos with exactly g and p
//!   bound over one fixed graph, its object a slot, `lo <= hi` within the inline range;
//! * a SqlJoin statement has the rendered shape (`physical::sqljoin`): one result column per
//!   distinct `out` slot, parameters `?1..?n` with `n = params.len()`, g-first full indexes;
//! * `Extend.canonical = false` only for slots no id-comparing consumer reads.
use sq_core::batch::{Slot, SlotSet};
use sq_core::ir::{ExistsId, GraphSel, PatTerm, QueryForm, TemplateTerm};
use sq_core::plan::{JoinKind, Op, Plan, QueryPlan};
use sq_core::store::IndexOrder;
use sq_core::{Error, Result};

use crate::expr;
use crate::physical::{
    bound_columns, canonical_needed, carried, node_exprs, scan_sorted_by, single_fixed_graph, sqljoin, walk,
};
use crate::props::{slot_list, PlanProps, Props};
use sq_core::TermId;

/// Validate a physical plan; `Err(Error::Internal)` names the offending node.
pub fn validate(p: &QueryPlan) -> Result<()> {
    Validator::new(p).run()
}

fn fail<T>(p: &Plan, msg: impl std::fmt::Display) -> Result<T> {
    Err(Error::Internal(format!("invalid plan: node #{}: {msg}", p.id)))
}

fn set(slots: &[Slot]) -> SlotSet {
    SlotSet::from_slots(slots.iter().copied())
}

/// A validated input as its parent sees it: static properties, the seed it was opened with and
/// the seeded slots its rows carry.
struct Input {
    props: Props,
    seed: SlotSet,
    carried: SlotSet,
}

impl Input {
    /// Slots bound in every row.
    fn certain(&self) -> SlotSet {
        self.props.certain.union(self.carried)
    }
    /// Slots bound in some row (a seeded slot holds the seed value or nothing).
    fn maybe(&self) -> SlotSet {
        self.props.maybe.union(self.seed)
    }
    /// The order of plan `p` (this input) as it survives a merge into joined rows: only a
    /// slot bound in every row.
    fn sorted(&self, p: &Plan) -> Option<Slot> {
        p.sorted_by.filter(|s| self.certain().contains(*s))
    }
}

struct Validator<'q> {
    q: &'q QueryPlan,
    pp: PlanProps<'q>,
    width: usize,
    /// Slots that must have canonical ids (computed once every slot is known to be in range).
    canonical: SlotSet,
    /// For each EXISTS id: whether every reference seen so far is under a GraphLoop.
    refs_in_loop: Vec<Option<bool>>,
}

impl<'q> Validator<'q> {
    fn new(q: &'q QueryPlan) -> Self {
        Validator {
            q,
            pp: PlanProps::new(&q.exists),
            width: q.width(),
            canonical: SlotSet::EMPTY,
            refs_in_loop: vec![None; q.exists.len()],
        }
    }

    fn run(mut self) -> Result<()> {
        self.check_ids()?;
        self.check_form()?;
        self.check_slot_range()?;
        self.canonical = canonical_needed(self.q);
        self.node(&self.q.root, SlotSet::EMPTY, false, false)?;
        // EXISTS plans: a parent always has a higher id than the patterns nested in it.
        for id in (0..self.q.exists.len()).rev() {
            let in_loop = self.refs_in_loop[id].unwrap_or(true);
            self.node(&self.q.exists[id], SlotSet::EMPTY, in_loop, true)?;
        }
        Ok(())
    }

    fn check_ids(&self) -> Result<()> {
        let mut next = 0u32;
        let mut bad: Option<&Plan> = None;
        let mut visit = |p: &'q Plan| {
            if p.id != next && bad.is_none() {
                bad = Some(p);
            }
            next += 1;
        };
        walk(&self.q.root, &mut visit);
        for e in &self.q.exists {
            walk(e, &mut visit);
        }
        match bad {
            Some(p) => fail(p, "plan ids are not the pre-order numbering"),
            None => Ok(()),
        }
    }

    fn check_form(&self) -> Result<()> {
        let w = self.width;
        let bad = |s: &Slot| *s as usize >= w;
        let ok = match &self.q.form {
            QueryForm::Select(slots) | QueryForm::Describe(slots) => !slots.iter().any(bad),
            QueryForm::Ask => true,
            QueryForm::Construct(t) => !t.iter().flatten().any(|x| matches!(x, TemplateTerm::Slot(s) if bad(s))),
        };
        if ok {
            Ok(())
        } else {
            Err(Error::Internal("invalid plan: query form references a slot outside the layout".into()))
        }
    }

    /// Every slot reference is inside the layout (checked on raw slots, before any `SlotSet`
    /// is built from them).
    fn check_slot_range(&self) -> Result<()> {
        let limit = self.width.min(SlotSet::MAX_SLOTS);
        let mut bad: Option<(&Plan, Slot)> = None;
        let mut visit = |p: &'q Plan| {
            if bad.is_some() {
                return;
            }
            let mut v = Vec::new();
            node_slots(p, &mut v);
            if let Some(s) = v.into_iter().find(|s| *s as usize >= limit) {
                bad = Some((p, s));
            }
        };
        walk(&self.q.root, &mut visit);
        for e in &self.q.exists {
            walk(e, &mut visit);
        }
        match bad {
            Some((p, s)) => fail(p, format!("slot {s} outside the layout (width {})", self.width)),
            None => Ok(()),
        }
    }

    fn check_slots(&self, p: &Plan, slots: SlotSet) -> Result<()> {
        match slot_list(slots).into_iter().find(|s| *s as usize >= self.width) {
            Some(s) => fail(p, format!("slot {s} outside the layout (width {})", self.width)),
            None => Ok(()),
        }
    }

    fn check_exprs(&mut self, p: &Plan, in_loop: bool) -> Result<()> {
        for e in node_exprs(p) {
            self.check_slots(p, expr::slots(e))?;
            for ExistsId(id) in expr::exists_ids(e) {
                let Some(r) = self.refs_in_loop.get_mut(id as usize) else {
                    return fail(p, format!("unknown EXISTS #{id}"));
                };
                *r = Some(r.unwrap_or(true) && in_loop);
            }
        }
        Ok(())
    }

    /// Validate `p` opened with Join-mode seed `seeded`.
    fn input(&mut self, p: &Plan, seeded: SlotSet, in_loop: bool, in_exists: bool) -> Result<Input> {
        let props = self.node(p, seeded, in_loop, in_exists)?;
        Ok(Input { props, seed: seeded, carried: carried(p, seeded) })
    }

    /// Validate `p` opened with Join-mode seed `seeded`; returns its properties.
    fn node(&mut self, p: &Plan, seeded: SlotSet, in_loop: bool, in_exists: bool) -> Result<Props> {
        let props = self.pp.props(p);
        self.check_slots(p, props.vars)?;
        self.check_exprs(p, in_loop)?;
        let derived_sorted = match &p.op {
            Op::Unit | Op::Empty | Op::GraphNames { .. } => None,
            Op::SqlJoin { sql, params, out } => {
                check_sql_join(p, sql, params, out)?;
                None
            }
            Op::Scan(s) => {
                if s.pat.g == GraphSel::Active && !in_loop {
                    return fail(p, "Scan reads the active graph outside a GraphLoop");
                }
                if s.order == IndexOrder::Gpox {
                    return fail(p, "Gpox is only read as part of an IntRange scan");
                }
                let cols = bound_columns(&s.pat, seeded);
                if let Some(r) = s.range {
                    let shape = s.order == IndexOrder::Gpos
                        && matches!(s.pat.o, PatTerm::Slot(_))
                        && matches!(s.pat.p, PatTerm::Const(_))
                        && cols == [true, false, true, false]
                        && single_fixed_graph(&s.pat, &self.q.dataset);
                    if !shape {
                        let msg = "IntRange needs a Gpos scan of one fixed graph with g and p bound and an object slot";
                        return fail(p, msg);
                    }
                    if r.lo > r.hi || r.lo < TermId::INT_MIN || r.hi > TermId::INT_MAX {
                        let (lo, hi) = (r.lo, r.hi);
                        return fail(p, format!("IntRange [{lo}, {hi}] is empty or outside the inline integers"));
                    }
                }
                let k = cols.iter().filter(|b| **b).count();
                if !in_exists && !IndexOrder::candidates(cols).iter().any(|(o, _)| *o == s.order) {
                    return fail(p, format!("order {:?} does not start with the bound columns {cols:?}", s.order));
                }
                if in_exists { None } else { scan_sorted_by(&s.pat, s.order, k, s.range.is_some(), &self.q.dataset) }
            }
            Op::Path { g, .. } => {
                if *g == GraphSel::Active && !in_loop {
                    return fail(p, "Path reads the active graph outside a GraphLoop");
                }
                None
            }
            Op::Values { slots, rows } => {
                if rows.iter().any(|r| r.len() != slots.len()) {
                    return fail(p, "Values row width differs from its slot list");
                }
                None
            }
            Op::BindJoin { left, right, seed, sort_left, .. } => {
                let seed_set = set(seed);
                let l = self.input(left, seeded, in_loop, in_exists)?;
                let rp = self.node(right, seed_set, in_loop, in_exists)?;
                if seed.is_empty() {
                    return fail(p, "BindJoin without seed slots");
                }
                if !seed_set.is_subset(&l.certain()) {
                    return fail(p, "BindJoin seed slot not certainly bound in the left rows");
                }
                if !rp.accepts_seed(seed_set) {
                    return fail(p, "BindJoin seed not in safe_inject(right)");
                }
                if !l.maybe().intersect(rp.maybe).is_subset(&seed_set) {
                    return fail(p, "BindJoin: a slot both sides may bind is not in the seed list");
                }
                // Left rows are emitted in order, per batch re-sorted by the seed tuple when
                // `sort_left`.
                let kept = !*sort_left || seed.first().copied() == left.sorted_by;
                kept.then_some(l.sorted(left)).flatten()
            }
            Op::HashJoin { build, probe, keys, check, kind, build_is_left } => {
                let inner = matches!(kind, JoinKind::Inner);
                if !inner && *build_is_left {
                    return fail(p, "outer / semi / anti HashJoin must build the right side");
                }
                let rseed = if inner { seeded } else { SlotSet::EMPTY };
                let (lp, rp) = if *build_is_left { (build, probe) } else { (probe, build) };
                let l = self.input(lp, seeded, in_loop, in_exists)?;
                let r = self.input(rp, rseed, in_loop, in_exists)?;
                check_keys(p, &l, &r, keys, check, true)?;
                // Probe rows stream through in order (inner joins always probe-major).
                if *build_is_left { r.sorted(rp) } else { l.sorted(lp) }
            }
            Op::NestedLoop { left, right, check, kind } => {
                let inner = matches!(kind, JoinKind::Inner);
                let rseed = if inner { seeded } else { SlotSet::EMPTY };
                let l = self.input(left, seeded, in_loop, in_exists)?;
                let r = self.input(right, rseed, in_loop, in_exists)?;
                check_keys(p, &l, &r, &[], check, false)?;
                l.sorted(left)
            }
            Op::MergeJoin { left, right, key, check, gallop } => {
                let l = self.input(left, seeded, in_loop, in_exists)?;
                let r = self.input(right, seeded, in_loop, in_exists)?;
                if left.sorted_by != Some(*key) || right.sorted_by != Some(*key) {
                    return fail(p, "MergeJoin input not sorted by the key");
                }
                if *gallop == 0 {
                    return fail(p, "MergeJoin galloping threshold must be positive");
                }
                check_keys(p, &l, &r, &[*key], check, true)?;
                Some(*key)
            }
            Op::StarMerge { key, inputs, check } => {
                if inputs.len() < 2 {
                    return fail(p, "StarMerge needs at least two inputs");
                }
                let mut acc: Option<Input> = None;
                for i in inputs {
                    let next = self.input(i, seeded, in_loop, in_exists)?;
                    if i.sorted_by != Some(*key) {
                        return fail(p, "StarMerge input not sorted by the key");
                    }
                    acc = Some(match acc {
                        None => next,
                        Some(a) => {
                            check_keys(p, &a, &next, &[*key], check, true)?;
                            let props = crate::props::join(&a.props, &next.props);
                            Input { props, seed: seeded, carried: a.carried.union(next.carried) }
                        }
                    });
                }
                Some(*key)
            }
            Op::Minus { left, right, keys, check } => {
                let l = self.input(left, seeded, in_loop, in_exists)?;
                let r = self.input(right, SlotSet::EMPTY, in_loop, in_exists)?;
                check_keys(p, &l, &r, keys, check, false)?;
                l.sorted(left)
            }
            Op::Union(branches) => {
                if branches.is_empty() {
                    return fail(p, "empty Union");
                }
                for b in branches {
                    self.node(b, seeded, in_loop, in_exists)?;
                }
                None
            }
            Op::Filter { input, .. } => self.node(input, seeded, in_loop, in_exists).map(|_| input.sorted_by)?,
            Op::Extend { input, slot, canonical, .. } => {
                let ip = self.node(input, seeded, in_loop, in_exists)?;
                if ip.maybe.union(seeded).contains(*slot) {
                    return fail(p, format!("Extend overwrites slot {slot}, which its input may bind"));
                }
                if !*canonical && self.canonical.contains(*slot) {
                    return fail(p, format!("Extend of slot {slot} must be canonical"));
                }
                input.sorted_by.filter(|s| s != slot)
            }
            Op::GraphLoop { input, .. } => {
                self.node(input, SlotSet::EMPTY, true, in_exists)?;
                None
            }
            Op::Group { input, keys, aggs, streaming } => {
                self.node(input, seeded, in_loop, in_exists)?;
                if aggs.iter().any(|(o, _)| keys.contains(o)) {
                    return fail(p, "aggregate output slot is also a group key");
                }
                if *streaming && (keys.len() != 1 || input.sorted_by != keys.first().copied()) {
                    return fail(p, "streaming Group needs one key and an input sorted by it");
                }
                None
            }
            Op::Count { input, .. } => {
                self.node(input, SlotSet::EMPTY, in_loop, in_exists)?;
                None
            }
            Op::Sort { input, .. } => {
                self.node(input, seeded, in_loop, in_exists)?;
                None
            }
            Op::SortBySlot { input, key } => {
                let ip = self.node(input, seeded, in_loop, in_exists)?;
                if !ip.maybe.union(seeded).contains(*key) {
                    return fail(p, format!("SortBySlot key {key} is never bound by its input"));
                }
                Some(*key)
            }
            Op::TopK { input, .. } => {
                self.node(input, SlotSet::EMPTY, in_loop, in_exists)?;
                None
            }
            Op::Slice { input, .. } => self.node(input, SlotSet::EMPTY, in_loop, in_exists).map(|_| input.sorted_by)?,
            Op::Distinct { input, slots } | Op::Reduced { input, slots } => {
                let ip = self.node(input, seeded, in_loop, in_exists)?;
                if !ip.maybe.is_subset(&set(slots)) {
                    return fail(p, "Distinct / Reduced does not cover every slot its input may bind");
                }
                input.sorted_by
            }
            Op::Project { input, slots } => {
                self.node(input, seeded, in_loop, in_exists)?;
                input.sorted_by.filter(|s| slots.contains(s))
            }
        };
        if p.sorted_by.is_some() && p.sorted_by != derived_sorted {
            return fail(p, format!("sorted_by {:?} is not guaranteed (derivable: {derived_sorted:?})", p.sorted_by));
        }
        if !p.est_rows.is_finite() || !p.est_cost_ns.is_finite() || p.est_rows < 0.0 || p.est_cost_ns < 0.0 {
            return fail(p, "estimates must be finite and non-negative");
        }
        Ok(props)
    }
}

/// A SqlJoin statement in the shape `physical::sqljoin` renders.
fn check_sql_join(p: &Plan, sql: &str, params: &[i64], out: &[Slot]) -> Result<()> {
    let Some(parsed) = sqljoin::parse(sql) else { return fail(p, "SqlJoin text is not a rendered SQL chain") };
    if out.is_empty() || parsed.select.len() != out.len() {
        return fail(p, format!("SqlJoin returns {} columns for {} slots", parsed.select.len(), out.len()));
    }
    if set(out).0.iter().map(|w| w.count_ones() as usize).sum::<usize>() != out.len() {
        return fail(p, "SqlJoin binds a slot twice");
    }
    if parsed.params != params.len() {
        return fail(p, format!("SqlJoin uses {} parameters, {} given", parsed.params, params.len()));
    }
    Ok(())
}

/// keys ⊆ certain on both sides; every other slot both sides may bind is in `check`, except
/// slots seeded on both inputs: each side holds the seed value there or nothing.
fn check_keys(p: &Plan, l: &Input, r: &Input, keys: &[Slot], check: &[Slot], need_keys: bool) -> Result<()> {
    let keys = set(keys);
    if need_keys && keys.is_empty() {
        return fail(p, "join without key slots");
    }
    if !keys.is_subset(&l.certain().intersect(r.certain())) {
        return fail(p, "join key not certainly bound on both sides");
    }
    let shared = l.maybe().intersect(r.maybe()).minus(l.seed.intersect(r.seed));
    if !shared.minus(keys).is_subset(&set(check)) {
        return fail(p, "a slot both sides may bind is neither a key nor checked");
    }
    Ok(())
}

/// Every slot a node references directly (patterns, expressions, keys, lists).
fn node_slots(p: &Plan, out: &mut Vec<Slot>) {
    let term = |t: PatTerm, out: &mut Vec<Slot>| {
        if let PatTerm::Slot(s) = t {
            out.push(s);
        }
    };
    let graph = |g: GraphSel, out: &mut Vec<Slot>| {
        if let GraphSel::Slot(s) = g {
            out.push(s);
        }
    };
    for e in node_exprs(p) {
        e.slots(out);
    }
    match &p.op {
        Op::Scan(s) => {
            graph(s.pat.g, out);
            [s.pat.s, s.pat.p, s.pat.o].into_iter().for_each(|t| term(t, out));
        }
        Op::Path { g, s, o, .. } => {
            graph(*g, out);
            term(*s, out);
            term(*o, out);
        }
        Op::Values { slots, .. }
        | Op::Distinct { slots, .. }
        | Op::Reduced { slots, .. }
        | Op::Project { slots, .. }
        | Op::SqlJoin { out: slots, .. } => out.extend(slots),
        Op::BindJoin { seed, .. } => out.extend(seed),
        Op::HashJoin { keys, check, .. } | Op::Minus { keys, check, .. } => {
            out.extend(keys);
            out.extend(check);
        }
        Op::MergeJoin { key, check, .. } | Op::StarMerge { key, check, .. } => {
            out.push(*key);
            out.extend(check);
        }
        Op::NestedLoop { check, .. } => out.extend(check),
        Op::Extend { slot, .. } | Op::GraphLoop { slot, .. } | Op::GraphNames { slot } => out.push(*slot),
        Op::Group { keys, aggs, .. } => {
            out.extend(keys);
            out.extend(aggs.iter().map(|(s, _)| *s));
        }
        Op::Count { out: o, .. } | Op::SortBySlot { key: o, .. } => out.push(*o),
        Op::Unit | Op::Empty | Op::Union(_) | Op::Filter { .. } | Op::Sort { .. } | Op::TopK { .. } | Op::Slice { .. } => {}
    }
    if let Some(s) = p.sorted_by {
        out.push(s);
    }
}
