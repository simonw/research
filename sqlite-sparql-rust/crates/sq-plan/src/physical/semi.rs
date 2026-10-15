//! EXISTS decorrelation (spec §7.3.4): a whole filter conjunct `[NOT] EXISTS { Q }` may become
//! a Semi (Anti) join on S = vars(Q) ∩ maybe(input) iff S ⊆ certain(input),
//! S ⊆ safe_inject(Q) and no zero-length-capable path in Q has an endpoint in S. Otherwise it
//! stays a correlated `Expr::Exists` (evaluated by substitution, memoized per S-tuple).
//!
//! Inside EXISTS plans nothing is decorrelated: there the outer substitution would also have to
//! reach the independently evaluated Q.
use sq_core::batch::SlotSet;
use sq_core::ir::{ExistsId, Expr, Logical, PatTerm, PathPlan};
use sq_core::plan::{JoinKind, JoinPolicy};
use sq_core::Result;

use super::join::{Algo, Choice, Shape};
use super::{Ctx, Planner};
use crate::expr;
use crate::logical::{children, exprs};

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

/// Does `l` (or an EXISTS pattern nested in it) contain a zero-length-capable path with an
/// endpoint slot in `s`?
fn zero_length_endpoint(l: &Logical, s: SlotSet, all_exists: &[Logical]) -> bool {
    if let Logical::Path { s: from, path, o: to, .. } = l {
        let hits = |t: &PatTerm| matches!(t, PatTerm::Slot(x) if s.contains(*x));
        if (hits(from) || hits(to)) && zero_length(path) {
            return true;
        }
    }
    let nested = exprs(l).into_iter().flat_map(expr::exists_ids).any(|ExistsId(id)| {
        all_exists.get(id as usize).is_some_and(|q| zero_length_endpoint(q, s, all_exists))
    });
    nested || children(l).into_iter().any(|c| zero_length_endpoint(c, s, all_exists))
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
    ) -> Result<Option<Choice>> {
        let Some(q) = self.exists_pats.get(id.0 as usize) else { return Ok(None) };
        let pq = self.lp.props(q);
        if ctx.in_exists
            || s.is_empty()
            || !s.is_subset(&side.certain)
            || !pq.accepts_seed(s)
            || zero_length_endpoint(q, s, self.exists_pats)
        {
            return Ok(None);
        }
        let lrows = side.rows;
        let rows = lrows * 0.5;
        let keys = s.intersect(pq.certain);
        let check = s.minus(keys);

        let hash = |this: &mut Self| -> Result<Choice> {
            // The outer seed goes to the left side only.
            let right = this.plan(q, ctx.unseeded())?;
            let b = right.est_rows;
            let (hb, hp) = if b < 32_768.0 {
                (this.costs.hash_build_small, this.costs.hash_probe_small)
            } else {
                (this.costs.hash_build_large, this.costs.hash_probe_large)
            };
            let cost = side.cost + right.est_cost_ns + b * hb + lrows * hp;
            let algo = if keys.is_empty() {
                Algo::Nested { check: s }
            } else {
                Algo::Hash { keys, check, build_is_left: false }
            };
            Ok(Choice { algo, right, rows, cost })
        };
        let bind = |this: &mut Self| -> Result<Choice> {
            let right = this.plan(q, Ctx { seeded: s, ..ctx.unseeded() })?;
            let cost = side.cost + lrows * right.est_cost_ns;
            Ok(Choice { algo: Algo::Bind { seed: s, sort_left: lrows > 64.0 }, right, rows, cost })
        };
        Ok(Some(match self.opts.join_policy {
            JoinPolicy::PreferHash => hash(self)?,
            JoinPolicy::PreferBind => bind(self)?,
            JoinPolicy::Auto | JoinPolicy::PreferMerge | JoinPolicy::NoSql => {
                let h = hash(self)?;
                // Correlated: one substituted evaluation per left row (memoized per S-tuple).
                let correlated = self.plan(q, Ctx { seeded: SlotSet::EMPTY, subst: side.certain.union(ctx.subst), in_exists: true })?;
                if side.cost + lrows * correlated.est_cost_ns <= h.cost {
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
