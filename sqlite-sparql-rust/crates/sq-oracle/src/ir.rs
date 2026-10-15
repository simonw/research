//! Static helpers over the Logical IR: the well-formedness check run before evaluation,
//! in-scope variables (§18.2.1) and the EXISTS patterns an expression references.
use sq_core::batch::SlotSet;
use sq_core::ir::{ExistsId, Expr, GraphSel, Logical, LoweredQuery, PatTerm, QueryForm, TemplateTerm};
use sq_core::{Error, Result, Slot};

/// Rejects IR the evaluator could not index safely: slots outside the variable table, VALUES
/// rows of the wrong length and references to missing EXISTS patterns.
pub(crate) fn validate(q: &LoweredQuery) -> Result<()> {
    let check = Check { width: q.vars.width(), exists: q.exists.len() };
    check.logical(&q.root)?;
    for p in &q.exists {
        check.logical(p)?;
    }
    match &q.form {
        QueryForm::Select(slots) | QueryForm::Describe(slots) => slots.iter().try_for_each(|s| check.slot(*s)),
        QueryForm::Ask => Ok(()),
        QueryForm::Construct(template) => template.iter().flatten().try_for_each(|t| match t {
            TemplateTerm::Slot(s) => check.slot(*s),
            TemplateTerm::Const(_) | TemplateTerm::BNode(_) => Ok(()),
        }),
    }
}

struct Check {
    width: usize,
    exists: usize,
}

impl Check {
    fn slot(&self, s: Slot) -> Result<()> {
        if (s as usize) < self.width {
            Ok(())
        } else {
            Err(Error::Invalid(format!("sq-oracle: slot {s} is outside the variable table (width {})", self.width)))
        }
    }

    fn term(&self, t: &PatTerm) -> Result<()> {
        match t {
            PatTerm::Slot(s) => self.slot(*s),
            PatTerm::Const(_) => Ok(()),
        }
    }

    fn graph(&self, g: &GraphSel) -> Result<()> {
        match g {
            GraphSel::Slot(s) => self.slot(*s),
            GraphSel::Default | GraphSel::Active | GraphSel::Const(_) => Ok(()),
        }
    }

    fn expr(&self, e: &Expr) -> Result<()> {
        let mut slots = Vec::new();
        e.slots(&mut slots);
        slots.into_iter().try_for_each(|s| self.slot(s))?;
        let mut ids = Vec::new();
        exists_ids(e, &mut ids);
        match ids.into_iter().find(|id| id.0 as usize >= self.exists) {
            Some(id) => Err(Error::Invalid(format!("sq-oracle: EXISTS #{} is not in the query", id.0))),
            None => Ok(()),
        }
    }

    fn logical(&self, l: &Logical) -> Result<()> {
        match l {
            Logical::Unit | Logical::Empty | Logical::Service { .. } => Ok(()),
            Logical::Bgp(patterns) => patterns.iter().try_for_each(|p| {
                self.graph(&p.g)?;
                self.term(&p.s)?;
                self.term(&p.p)?;
                self.term(&p.o)
            }),
            Logical::Path { g, s, o, .. } => {
                self.graph(g)?;
                self.term(s)?;
                self.term(o)
            }
            Logical::Join(a, b) | Logical::Union(a, b) | Logical::Minus { left: a, right: b } => {
                self.logical(a)?;
                self.logical(b)
            }
            Logical::LeftJoin { left, right, filter } => {
                self.logical(left)?;
                self.logical(right)?;
                filter.iter().try_for_each(|f| self.expr(f))
            }
            Logical::Filter { input, expr } => {
                self.logical(input)?;
                self.expr(expr)
            }
            Logical::Extend { input, slot, expr } => {
                self.logical(input)?;
                self.slot(*slot)?;
                self.expr(expr)
            }
            Logical::Values { slots, rows } => {
                slots.iter().try_for_each(|s| self.slot(*s))?;
                match rows.iter().find(|r| r.len() != slots.len()) {
                    Some(r) => Err(Error::Invalid(format!(
                        "sq-oracle: VALUES row of {} terms for {} variables",
                        r.len(),
                        slots.len()
                    ))),
                    None => Ok(()),
                }
            }
            Logical::Graph { name, input } => {
                self.term(name)?;
                self.logical(input)
            }
            Logical::Group { input, keys, aggs } => {
                self.logical(input)?;
                keys.iter().try_for_each(|s| self.slot(*s))?;
                aggs.iter().try_for_each(|(s, a)| {
                    self.slot(*s)?;
                    a.arg.iter().try_for_each(|e| self.expr(e))
                })
            }
            Logical::OrderBy { input, keys } => {
                self.logical(input)?;
                keys.iter().try_for_each(|k| self.expr(&k.expr))
            }
            Logical::Project { input, slots } => {
                self.logical(input)?;
                slots.iter().try_for_each(|s| self.slot(*s))
            }
            Logical::Distinct(input) | Logical::Reduced(input) | Logical::Slice { input, .. } => self.logical(input),
        }
    }
}

/// The in-scope variables of a pattern (§18.2.1): what `SELECT *` over it would project,
/// including blank-node pattern variables (callers filter those by `VarOrigin`).
pub(crate) fn in_scope(l: &Logical) -> SlotSet {
    fn term(t: &PatTerm, out: &mut SlotSet) {
        if let PatTerm::Slot(s) = t {
            out.insert(*s);
        }
    }
    fn graph(g: &GraphSel, out: &mut SlotSet) {
        if let GraphSel::Slot(s) = g {
            out.insert(*s);
        }
    }
    let mut out = SlotSet::EMPTY;
    match l {
        Logical::Unit | Logical::Empty | Logical::Service { .. } => {}
        Logical::Bgp(patterns) => {
            for p in patterns {
                graph(&p.g, &mut out);
                term(&p.s, &mut out);
                term(&p.p, &mut out);
                term(&p.o, &mut out);
            }
        }
        Logical::Path { g, s, o, .. } => {
            graph(g, &mut out);
            term(s, &mut out);
            term(o, &mut out);
        }
        Logical::Join(a, b) | Logical::Union(a, b) | Logical::LeftJoin { left: a, right: b, .. } => {
            out = in_scope(a).union(in_scope(b));
        }
        Logical::Minus { left, .. } => out = in_scope(left),
        Logical::Filter { input, .. }
        | Logical::OrderBy { input, .. }
        | Logical::Distinct(input)
        | Logical::Reduced(input)
        | Logical::Slice { input, .. } => out = in_scope(input),
        Logical::Extend { input, slot, .. } => {
            out = in_scope(input);
            out.insert(*slot);
        }
        Logical::Values { slots, .. } | Logical::Project { slots, .. } => {
            out = SlotSet::from_slots(slots.iter().copied());
        }
        Logical::Graph { name, input } => {
            out = in_scope(input);
            term(name, &mut out);
        }
        Logical::Group { keys, aggs, .. } => {
            out = SlotSet::from_slots(keys.iter().copied().chain(aggs.iter().map(|(s, _)| *s)));
        }
    }
    out
}

/// Appends the ids of the EXISTS patterns `e` references (not those nested inside them).
pub(crate) fn exists_ids(e: &Expr, out: &mut Vec<ExistsId>) {
    match e {
        Expr::Exists(id) => out.push(*id),
        Expr::Const(_) | Expr::Slot(_) | Expr::Bound(_) => {}
        Expr::Not(a) | Expr::UnaryPlus(a) | Expr::UnaryMinus(a) => exists_ids(a, out),
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
            exists_ids(a, out);
            exists_ids(b, out);
        }
        Expr::In(a, list) => {
            exists_ids(a, out);
            list.iter().for_each(|x| exists_ids(x, out));
        }
        Expr::If(a, b, c) => {
            exists_ids(a, out);
            exists_ids(b, out);
            exists_ids(c, out);
        }
        Expr::Coalesce(list) | Expr::Call(_, list) => list.iter().for_each(|x| exists_ids(x, out)),
    }
}
