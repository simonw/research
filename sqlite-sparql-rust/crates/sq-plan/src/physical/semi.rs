//! EXISTS decorrelation (spec §7.3.4): a whole filter conjunct `[NOT] EXISTS { Q }` may become
//! a Semi (Anti) join on S = vars(Q) ∩ maybe(input) iff S ⊆ certain(input),
//! S ⊆ safe_inject(Q) and no zero-length-capable path in Q has an endpoint in S. Otherwise it
//! stays a correlated `Expr::Exists` (evaluated by substitution, memoized per S-tuple).
//!
//! One more condition makes the join equal to that substitution: no MINUS in Q may mention a
//! slot of S on its right side. Substitution (`SeedMode::Substitute`) turns S into constants in
//! the MINUS right side too and leaves them out of its domain test, while a join (independent
//! Q, or Q opened with a Join-mode seed, which reaches the MINUS left side only) keeps them in
//! it: for `?x :knows ?y MINUS { ?x :age ?a }` and ?x = :a with `:a :knows :b . :a :age 3`
//! the substituted EXISTS is true but the Semi join drops :a.
//!
//! Inside EXISTS plans nothing is decorrelated: there the outer substitution would also have to
//! reach the independently evaluated Q.
use sq_core::batch::SlotSet;
use sq_core::ir::{ExistsId, Expr, Logical, PatTerm, PathPlan};
use sq_core::plan::{JoinKind, JoinPolicy};
use sq_core::Result;

use super::join::{Algo, Choice, Mode, Shape, SORT_LEFT_ROWS};
use super::{Ctx, Planner};
use crate::expr;
use crate::logical::{children, exprs};
use crate::props::LogicalProps;

/// `EXISTS { Q }` -> (id, false); `NOT EXISTS { Q }` -> (id, true).
pub(super) fn exists_conjunct(e: &Expr) -> Option<(ExistsId, bool)> {
    match e {
        Expr::Exists(id) => Some((*id, false)),
        Expr::Not(inner) => match &**inner {
            Expr::Exists(id) => Some((*id, true)),
            _ => None,
        },
        _ => None,
    }
}

/// Can `p` match a zero-length path?
fn zero_length(p: &PathPlan) -> bool {
    match p {
        PathPlan::Pred(_) | PathPlan::Nps(_) => false,
        PathPlan::ZeroOrMore(_) | PathPlan::ZeroOrOne(_) => true,
        PathPlan::Inv(p) | PathPlan::OneOrMore(p) => zero_length(p),
        PathPlan::Seq(a, b) => zero_length(a) && zero_length(b),
        PathPlan::Alt(a, b) => zero_length(a) || zero_length(b),
    }
}

/// Would evaluating `l` joined on `s` differ from substituting `s` into it? True when `l` (or
/// an EXISTS pattern nested in it) has a zero-length-capable path with an endpoint in `s`, or
/// a MINUS whose right side mentions a slot of `s`.
fn substitution_differs(l: &Logical, s: SlotSet, lp: &LogicalProps<'_>, all_exists: &[Logical]) -> bool {
    let local = match l {
        Logical::Path { s: from, path, o: to, .. } => {
            let hits = |t: &PatTerm| matches!(t, PatTerm::Slot(x) if s.contains(*x));
            (hits(from) || hits(to)) && zero_length(path)
        }
        Logical::Minus { right, .. } => !lp.props(right).vars.intersect(s).is_empty(),
        _ => false,
    };
    local
        || exprs(l).into_iter().flat_map(expr::exists_ids).any(|ExistsId(id)| {
            all_exists.get(id as usize).is_some_and(|q| substitution_differs(q, s, lp, all_exists))
        })
        || children(l).into_iter().any(|c| substitution_differs(c, s, lp, all_exists))
}

impl Planner<'_> {
    /// Decide how to apply `[NOT] EXISTS { exists[id] }` on `side`, whose evars (relative to
    /// the group) are `s`. Returns the join to build, or None to keep the correlated filter.
    pub(super) fn semi_join_choice(
        &mut self,
        side: &Shape,
        id: ExistsId,
        s: SlotSet,
        ctx: Ctx,
        mode: Mode,
    ) -> Result<Option<Choice>> {
        let Some(q) = self.exists_pats.get(id.0 as usize) else { return Ok(None) };
        let pq = self.lp.props(q);
        if ctx.in_exists
            || s.is_empty()
            || !s.is_subset(&side.own)
            || !pq.accepts_seed(s)
            || substitution_differs(q, s, &self.lp, self.exists_pats)
        {
            return Ok(None);
        }
        let lrows = side.rows;
        let rows = lrows * 0.5;
        let keys = s.intersect(pq.certain);
        let check = s.minus(keys);

        let hash = |this: &mut Self| -> Result<Choice> {
            // The outer seed goes to the left side only.
            let right = this.open_node(q, ctx.unseeded(), mode)?;
            let b = right.rows;
            let (hb, hp) = if b < 32_768.0 {
                (this.costs.hash_build_small, this.costs.hash_probe_small)
            } else {
                (this.costs.hash_build_large, this.costs.hash_probe_large)
            };
            let cost = side.cost + right.cost + b * hb + lrows * hp;
            let algo = if keys.is_empty() {
                Algo::Nested { check: s }
            } else {
                Algo::Hash { keys, check, build_is_left: false }
            };
            Ok(Choice::new(algo, right, rows, cost))
        };
        let bind = |this: &mut Self| -> Result<Choice> {
            let right = this.open_node(q, Ctx { seeded: s, ..ctx.unseeded() }, mode)?;
            let cost = side.cost + lrows * right.cost;
            Ok(Choice::new(Algo::Bind { seed: s, sort_left: lrows > SORT_LEFT_ROWS }, right, rows, cost))
        };
        Ok(Some(match self.opts.join_policy {
            JoinPolicy::PreferHash => hash(self)?,
            JoinPolicy::PreferBind => bind(self)?,
            JoinPolicy::Auto | JoinPolicy::PreferMerge | JoinPolicy::NoSql => {
                let h = hash(self)?;
                // Correlated: one substituted evaluation per left row (memoized per S-tuple).
                let subst = side.certain.union(ctx.subst);
                let correlated = self.estimate(q, Ctx { seeded: SlotSet::EMPTY, subst, in_exists: true })?;
                if side.cost + lrows * correlated.cost <= h.cost {
                    return Ok(None);
                }
                h
            }
        }))
    }
}

/// Join kind of a decorrelated conjunct.
pub(super) fn semi_kind(anti: bool) -> JoinKind {
    if anti { JoinKind::Anti } else { JoinKind::Semi }
}
