//! Bottom-up evaluation of the Logical IR, transcribed from SPARQL 1.1 §18.6
//! ("Evaluation Semantics"): every operator consumes and produces a whole bag of solutions.
//!
//! # EXISTS by substitution
//!
//! `exists(P)` with the current solution μ is true iff `eval(D(G), substitute(P, μ))` is not
//! empty (§18.6). `substitute` replaces every variable of μ by its value; the oracle evaluates
//! P with μ as the *seed* ([`Ctx::seed`]) instead of rewriting the tree, which is the same
//! thing operator by operator:
//!
//! * every row produced under a seed carries the seed's bindings, and a seeded slot is a
//!   constant wherever it is read: in triple and path patterns (including zero-length path
//!   endpoints), expressions, `GRAPH ?g`, VALUES (rows incompatible with the seed are dropped)
//!   and `BIND` (a row survives iff the value is the seeded term or an error);
//! * MINUS leaves seeded slots out of its domain test (a substituted variable is no longer a
//!   variable of either side);
//! * Project and Group re-apply the seed to their output, like the constants they stand for.
//!
//! With an all-UNBOUND seed (the query itself) these rules are the plain algebra.
//!
//! Expressions are evaluated by sq-expr. The EXISTS answers an expression needs for a row are
//! computed before the expression runs (they depend on the row and the active graph only)
//! and handed to sq-expr through [`ExistsEval`].
use std::rc::Rc;

use rand::rngs::StdRng;
use rustc_hash::{FxHashMap, FxHashSet};
use sq_core::eval_api::ExistsEval;
use sq_core::ir::{ExistsId, Expr, GraphSel, Logical, LoweredQuery, OrderKey, PatTerm, PathPlan, QuadPat};
use sq_core::store::QuadStore;
use sq_core::{Error, Result, Slot, TermId};
use sq_expr::{CompiledExpr, EvalCtx, SortKey, ValueCache};

use crate::bag::{bind, compatible, merge_into, Bag};
use crate::dataset::{ActiveGraph, Dataset, Graph, GraphReader};
use crate::ir::exists_ids;
use crate::path::{End, PathEval};
use crate::resolver::OracleResolver;

/// Where an evaluation happens: the substituted solution and the active graph.
#[derive(Copy, Clone)]
pub(crate) struct Ctx<'a> {
    /// Full-width row; bound slots are substituted constants (all UNBOUND outside EXISTS).
    pub(crate) seed: &'a [TermId],
    pub(crate) active: ActiveGraph,
}

/// A compiled expression and the EXISTS patterns it references.
struct Compiled {
    expr: CompiledExpr,
    exists: Vec<ExistsId>,
}

pub(crate) struct Evaluator<'q> {
    pub(crate) q: &'q LoweredQuery,
    pub(crate) reader: GraphReader<'q>,
    pub(crate) dataset: Dataset,
    pub(crate) width: usize,
    pub(crate) resolver: OracleResolver<'q>,
    /// Compiled expressions keyed by address: every expression evaluated lives in `q`, which
    /// is borrowed (hence neither moved nor freed) for the whole evaluation.
    compiled: FxHashMap<*const Expr, Rc<Compiled>>,
    values: ValueCache,
    rng: StdRng,
    now: oxsdatatypes::DateTime,
}

impl<'q> Evaluator<'q> {
    pub(crate) fn new(
        q: &'q LoweredQuery,
        store: &'q dyn QuadStore,
        now: oxsdatatypes::DateTime,
        rng: StdRng,
    ) -> Result<Self> {
        Ok(Evaluator {
            q,
            reader: GraphReader { store },
            dataset: Dataset::resolve(&q.dataset, store)?,
            width: q.vars.width(),
            resolver: OracleResolver::new(store.dict(), q.temps.clone()),
            compiled: FxHashMap::default(),
            values: ValueCache::new(),
            rng,
            now,
        })
    }

    /// eval(D(G), node) as a bag of full-width rows.
    pub(crate) fn eval(&mut self, node: &'q Logical, cx: Ctx<'_>) -> Result<Bag> {
        match node {
            Logical::Unit => Ok(Bag::single(cx.seed)),
            Logical::Empty => Ok(Bag::new(self.width)),
            Logical::Bgp(patterns) => self.bgp(patterns, cx),
            Logical::Path { g, s, path, o } => self.path(*g, *s, path, *o, cx),
            Logical::Join(left, right) => {
                let left = self.eval(left, cx)?;
                let right = self.eval(right, cx)?;
                Ok(join(&left, &right))
            }
            Logical::LeftJoin { left, right, filter } => {
                let left = self.eval(left, cx)?;
                let right = self.eval(right, cx)?;
                self.left_join(&left, &right, filter.as_ref(), cx.active)
            }
            Logical::Minus { left, right } => {
                let left = self.eval(left, cx)?;
                let right = self.eval(right, cx)?;
                Ok(minus(&left, &right, cx.seed))
            }
            Logical::Union(left, right) => {
                let mut out = self.eval(left, cx)?;
                out.append(self.eval(right, cx)?);
                Ok(out)
            }
            Logical::Filter { input, expr } => {
                let input = self.eval(input, cx)?;
                let mut out = Bag::new(self.width);
                for row in input.rows() {
                    if self.ebv(expr, row, cx.active)? {
                        out.push(row);
                    }
                }
                Ok(out)
            }
            Logical::Extend { input, slot, expr } => {
                let input = self.eval(input, cx)?;
                self.extend(&input, *slot, expr, cx.active)
            }
            Logical::Values { slots, rows } => Ok(self.values(slots, rows, cx.seed)),
            Logical::Graph { name, input } => self.graph(*name, input, cx),
            Logical::Group { input, keys, aggs } => self.group(input, keys, aggs, cx),
            Logical::OrderBy { input, keys } => {
                let input = self.eval(input, cx)?;
                self.order_by(&input, keys, cx.active)
            }
            Logical::Project { input, slots } => {
                let input = self.eval(input, cx)?;
                let mut out = Bag::new(self.width);
                let mut row = Vec::with_capacity(self.width);
                for r in input.rows() {
                    row.clear();
                    row.extend_from_slice(cx.seed);
                    for &s in slots {
                        if r[s as usize].is_bound() {
                            row[s as usize] = r[s as usize];
                        }
                    }
                    out.push(&row);
                }
                Ok(out)
            }
            Logical::Distinct(input) => {
                let input = self.eval(input, cx)?;
                let mut seen: FxHashSet<&[TermId]> = FxHashSet::default();
                let mut out = Bag::new(self.width);
                for row in input.rows() {
                    if seen.insert(row) {
                        out.push(row);
                    }
                }
                Ok(out)
            }
            // REDUCED may drop any number of duplicates; keeping all of them is a valid answer.
            Logical::Reduced(input) => self.eval(input, cx),
            Logical::Slice { input, offset, limit } => {
                let input = self.eval(input, cx)?;
                let start = usize::try_from(*offset).unwrap_or(usize::MAX).min(input.len());
                let end = match limit {
                    Some(l) => start.saturating_add(usize::try_from(*l).unwrap_or(usize::MAX)).min(input.len()),
                    None => input.len(),
                };
                Ok(input.select(start..end))
            }
            Logical::Service { silent: true } => Ok(Bag::single(cx.seed)),
            Logical::Service { silent: false } => Err(Error::Unsupported("SERVICE".into())),
        }
    }

    // ------------------------------------------------------------------ graph patterns

    /// Basic graph pattern: nested loops, one triple pattern at a time, each partial solution
    /// extended by the matching triples of its graph (bound slots act as constants).
    fn bgp(&mut self, patterns: &[QuadPat], cx: Ctx<'_>) -> Result<Bag> {
        let mut current = Bag::single(cx.seed);
        let mut triples = Vec::new();
        let mut scratch = Vec::with_capacity(self.width);
        for pat in patterns {
            let mut next = Bag::new(self.width);
            for row in current.rows() {
                let fixed = |t: PatTerm| match t {
                    PatTerm::Const(c) => Some(c),
                    PatTerm::Slot(s) => Some(row[s as usize]).filter(|v| v.is_bound()),
                };
                let (s, p, o) = (fixed(pat.s), fixed(pat.p), fixed(pat.o));
                for (graph, graph_slot) in self.graphs_of(pat.g, row, cx.active) {
                    triples.clear();
                    self.reader.triples(graph, s, p, o, &mut triples)?;
                    for &[ts, tp, to] in &triples {
                        scratch.clear();
                        scratch.extend_from_slice(row);
                        let ok = graph_slot.is_none_or(|(x, g)| bind(&mut scratch, x, g))
                            && bind_term(&mut scratch, pat.s, ts)
                            && bind_term(&mut scratch, pat.p, tp)
                            && bind_term(&mut scratch, pat.o, to);
                        if ok {
                            next.push(&scratch);
                        }
                    }
                }
            }
            current = next;
        }
        Ok(current)
    }

    /// The graphs a pattern with selector `sel` reads for `row`. `GraphSel::Slot` (a pushed
    /// `GRAPH ?g`) ranges over the named graphs and also binds the slot.
    fn graphs_of(&self, sel: GraphSel, row: &[TermId], active: ActiveGraph) -> Vec<(Graph<'_>, Option<(Slot, TermId)>)> {
        match sel {
            GraphSel::Slot(x) => {
                let bound = row[x as usize];
                self.dataset
                    .named()
                    .iter()
                    .filter(|g| !bound.is_bound() || **g == bound)
                    .map(|&g| (Graph::One(g), Some((x, g))))
                    .collect()
            }
            sel => vec![(self.dataset.graph(sel, active), None)],
        }
    }

    /// Property path pattern (§18.4): the (x, y) pairs of the path, bound to its endpoints.
    fn path(&mut self, g: GraphSel, s: PatTerm, path: &PathPlan, o: PatTerm, cx: Ctx<'_>) -> Result<Bag> {
        let end = |t: PatTerm| match t {
            PatTerm::Const(c) => End::Fixed(c),
            PatTerm::Slot(x) if cx.seed[x as usize].is_bound() => End::Fixed(cx.seed[x as usize]),
            PatTerm::Slot(_) => End::Free,
        };
        let (from, to) = (end(s), end(o));
        let mut out = Bag::new(self.width);
        let mut scratch = Vec::with_capacity(self.width);
        for (graph, graph_slot) in self.graphs_of(g, cx.seed, cx.active) {
            let pairs = PathEval { reader: self.reader, graph }.pairs(path, from, to)?;
            for (x, y) in pairs {
                scratch.clear();
                scratch.extend_from_slice(cx.seed);
                let ok = graph_slot.is_none_or(|(slot, g)| bind(&mut scratch, slot, g))
                    && bind_term(&mut scratch, s, x)
                    && bind_term(&mut scratch, o, y);
                if ok {
                    out.push(&scratch);
                }
            }
        }
        Ok(out)
    }

    /// LeftJoin(Ω1, Ω2, expr) = Filter(expr, Join(Ω1, Ω2)) ∪ Diff(Ω1, Ω2, expr): the filter
    /// sees the merged solution, and a left solution survives alone iff no compatible right
    /// solution passes it (an error counts as false).
    fn left_join(&mut self, left: &Bag, right: &Bag, filter: Option<&'q Expr>, active: ActiveGraph) -> Result<Bag> {
        let mut out = Bag::new(self.width);
        let mut merged = Vec::with_capacity(self.width);
        for l in left.rows() {
            let mut matched = false;
            for r in right.rows() {
                if !compatible(l, r) {
                    continue;
                }
                merged.clear();
                merged.extend_from_slice(l);
                merge_into(&mut merged, r);
                let pass = match filter {
                    Some(f) => self.ebv(f, &merged, active)?,
                    None => true,
                };
                if pass {
                    out.push(&merged);
                    matched = true;
                }
            }
            if !matched {
                out.push(l);
            }
        }
        Ok(out)
    }

    /// Extend(μ, var, expr): binds the value, or leaves the slot unbound on an error. A slot
    /// that is already bound (a substituted variable) acts as a constant: the solution survives
    /// iff the value is that same term or an error.
    fn extend(&mut self, input: &Bag, slot: Slot, expr: &'q Expr, active: ActiveGraph) -> Result<Bag> {
        let mut out = Bag::new(self.width);
        let mut row = Vec::with_capacity(self.width);
        for r in input.rows() {
            let value = self.value(expr, r, active)?;
            row.clear();
            row.extend_from_slice(r);
            if value.is_none_or(|v| bind(&mut row, slot, v)) {
                out.push(&row);
            }
        }
        Ok(out)
    }

    /// VALUES: one solution per data row (UNDEF = unbound), merged with the seed when
    /// compatible.
    fn values(&self, slots: &[Slot], rows: &[Vec<TermId>], seed: &[TermId]) -> Bag {
        let mut out = Bag::new(self.width);
        let mut row = Vec::with_capacity(self.width);
        for data in rows {
            row.clear();
            row.extend_from_slice(seed);
            if slots.iter().zip(data).all(|(&s, &v)| !v.is_bound() || bind(&mut row, s, v)) {
                out.push(&row);
            }
        }
        out
    }

    /// GRAPH: `GRAPH <c> { P }` is `eval(D(D[c]), P)` when c names a graph of the dataset and
    /// empty otherwise; `GRAPH ?g { P }` is the union over the named graphs g of
    /// `Join(eval(D(D[g]), P), {?g → g})`. The default graph is never a named graph.
    fn graph(&mut self, name: PatTerm, input: &'q Logical, cx: Ctx<'_>) -> Result<Bag> {
        let (graphs, slot): (Vec<TermId>, Option<Slot>) = match name {
            PatTerm::Const(g) => (vec![g], None),
            PatTerm::Slot(s) if cx.seed[s as usize].is_bound() => (vec![cx.seed[s as usize]], Some(s)),
            PatTerm::Slot(s) => (self.dataset.named().to_vec(), Some(s)),
        };
        let mut out = Bag::new(self.width);
        let mut row = Vec::with_capacity(self.width);
        for g in graphs {
            if !self.dataset.is_named(g) {
                continue;
            }
            let inner = self.eval(input, Ctx { seed: cx.seed, active: ActiveGraph::Named(g) })?;
            match slot {
                None => out.append(inner),
                Some(s) => {
                    for r in inner.rows() {
                        row.clear();
                        row.extend_from_slice(r);
                        if bind(&mut row, s, g) {
                            out.push(&row);
                        }
                    }
                }
            }
        }
        Ok(out)
    }

    /// ORDER BY: a stable sort on the keys' sort keys (unbound and errors lowest, DESC
    /// reversed), so solutions that tie keep their input order.
    fn order_by(&mut self, input: &Bag, keys: &'q [OrderKey], active: ActiveGraph) -> Result<Bag> {
        let mut sort_keys: Vec<Vec<SortKey>> = Vec::with_capacity(input.len());
        for row in input.rows() {
            let mut k = Vec::with_capacity(keys.len());
            for key in keys {
                k.push(self.sort_key(&key.expr, row, active)?);
            }
            sort_keys.push(k);
        }
        let mut order: Vec<usize> = (0..input.len()).collect();
        order.sort_by(|&a, &b| {
            keys.iter()
                .zip(sort_keys[a].iter().zip(&sort_keys[b]))
                .map(|(key, (x, y))| if key.desc { y.cmp(x) } else { x.cmp(y) })
                .find(|o| o.is_ne())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(input.select(order))
    }

    // ------------------------------------------------------------------ expressions

    /// FILTER semantics: true iff the effective boolean value is true (errors are false).
    pub(crate) fn ebv(&mut self, e: &'q Expr, row: &[TermId], active: ActiveGraph) -> Result<bool> {
        Ok(self.with_expr(e, row, active, |c, cx| c.ebv(row, cx))? == Some(true))
    }

    /// The canonical id of the value of `e`, or None for an error / unbound result.
    pub(crate) fn value(&mut self, e: &'q Expr, row: &[TermId], active: ActiveGraph) -> Result<Option<TermId>> {
        self.with_expr(e, row, active, |c, cx| c.eval(row, cx, true))
    }

    fn sort_key(&mut self, e: &'q Expr, row: &[TermId], active: ActiveGraph) -> Result<SortKey> {
        self.with_expr(e, row, active, |c, cx| c.sort_key(row, cx))
    }

    /// Runs `f` on the compiled `e` with an `EvalCtx` whose EXISTS answers were computed for
    /// `row` in the active graph.
    fn with_expr<R>(
        &mut self,
        e: &'q Expr,
        row: &[TermId],
        active: ActiveGraph,
        f: impl FnOnce(&CompiledExpr, &mut EvalCtx<'_>) -> Result<R>,
    ) -> Result<R> {
        let compiled = self.compile(e)?;
        let mut answers = Vec::with_capacity(compiled.exists.len());
        for &id in &compiled.exists {
            answers.push(self.exists(id, row, active)?);
        }
        let mut hook = Answers { ids: &compiled.exists, answers: &answers, row };
        self.run(&mut hook, |cx| f(&compiled.expr, cx))
    }

    /// Runs `f` with an `EvalCtx` that cannot evaluate EXISTS (accumulators).
    pub(crate) fn with_cx<R>(&mut self, f: impl FnOnce(&mut EvalCtx<'_>) -> Result<R>) -> Result<R> {
        let mut hook = Answers { ids: &[], answers: &[], row: &[] };
        self.run(&mut hook, f)
    }

    fn run<R>(&mut self, exists: &mut dyn ExistsEval, f: impl FnOnce(&mut EvalCtx<'_>) -> Result<R>) -> Result<R> {
        let mut cx = EvalCtx {
            resolver: &mut self.resolver,
            exists,
            values: &mut self.values,
            now: self.now,
            base_iri: self.q.base_iri.as_deref(),
            rng: &mut self.rng,
        };
        f(&mut cx)
    }

    fn compile(&mut self, e: &'q Expr) -> Result<Rc<Compiled>> {
        let key: *const Expr = e;
        if let Some(c) = self.compiled.get(&key) {
            return Ok(c.clone());
        }
        let mut exists = Vec::new();
        exists_ids(e, &mut exists);
        exists.sort_unstable_by_key(|id| id.0);
        exists.dedup();
        let c = Rc::new(Compiled { expr: sq_expr::compile(e, &self.q.temps)?, exists });
        self.compiled.insert(key, c.clone());
        Ok(c)
    }

    /// exists(P) for the solution `row`: eval(D(G), substitute(P, row)) is not empty.
    fn exists(&mut self, id: ExistsId, row: &[TermId], active: ActiveGraph) -> Result<bool> {
        let q = self.q;
        let pattern = q
            .exists
            .get(id.0 as usize)
            .ok_or_else(|| Error::Invalid(format!("sq-oracle: EXISTS #{} is not in the query", id.0)))?;
        Ok(!self.eval(pattern, Ctx { seed: row, active })?.is_empty())
    }
}

/// Precomputed EXISTS answers for one expression evaluation.
struct Answers<'a> {
    ids: &'a [ExistsId],
    answers: &'a [bool],
    row: &'a [TermId],
}

impl ExistsEval for Answers<'_> {
    fn exists(&mut self, id: ExistsId, row: &[TermId]) -> Result<bool> {
        debug_assert_eq!(row, self.row, "EXISTS asked for another solution than the one it was computed for");
        self.ids
            .iter()
            .position(|i| *i == id)
            .map(|k| self.answers[k])
            .ok_or_else(|| Error::internal(format!("sq-oracle: EXISTS #{} was not precomputed", id.0)))
    }
}

/// Binds a pattern position: a constant must equal the term, a slot is bound (or checked).
fn bind_term(row: &mut [TermId], t: PatTerm, value: TermId) -> bool {
    match t {
        PatTerm::Const(c) => c == value,
        PatTerm::Slot(s) => bind(row, s, value),
    }
}

/// Join(Ω1, Ω2) = { merge(μ1, μ2) | μ1 ∈ Ω1, μ2 ∈ Ω2, μ1 and μ2 compatible }, by nested loops.
pub(crate) fn join(left: &Bag, right: &Bag) -> Bag {
    let mut out = Bag::new(left.width());
    let mut merged = Vec::with_capacity(left.width());
    for l in left.rows() {
        for r in right.rows() {
            if compatible(l, r) {
                merged.clear();
                merged.extend_from_slice(l);
                merge_into(&mut merged, r);
                out.push(&merged);
            }
        }
    }
    out
}

/// Minus(Ω1, Ω2) = { μ ∈ Ω1 | ∀ μ' ∈ Ω2: μ and μ' are not compatible, or dom(μ) and dom(μ')
/// are disjoint }. Seeded slots are constants, so they belong to neither domain.
pub(crate) fn minus(left: &Bag, right: &Bag, seed: &[TermId]) -> Bag {
    let shares_variable = |l: &[TermId], r: &[TermId]| {
        l.iter().zip(r).zip(seed).any(|((a, b), s)| a.is_bound() && b.is_bound() && !s.is_bound())
    };
    let mut out = Bag::new(left.width());
    for l in left.rows() {
        if right.rows().all(|r| !compatible(l, r) || !shares_variable(l, r)) {
            out.push(l);
        }
    }
    out
}
