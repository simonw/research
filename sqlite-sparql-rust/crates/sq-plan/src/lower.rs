//! Lowering: spargebra algebra -> `sq_core::ir::Logical` (spec §5.4, §5.6, §7.1 step 2).
//!
//! * One global slot layout for the whole query, including EXISTS patterns.
//! * A non-root `Project` (a sub-select) opens a variable scope: projected variables keep the
//!   enclosing scope's slots, every other variable of the sub-select gets a fresh slot
//!   (alpha renaming).
//! * Pattern blank nodes become hidden slots, one per label per scope (spargebra already
//!   rejects a label shared by two blocks, so labels only repeat where spargebra itself shares
//!   them, e.g. the middle node of a split property path).
//! * Constants are resolved with `Dictionary::lookup`; a miss becomes a `TempDict` TEMP id.
//! * `GRAPH` becomes `Logical::Graph` and the patterns inside read `GraphSel::Active`.
use std::sync::Arc;

use rustc_hash::FxHashMap;
use spargebra::algebra::{
    AggregateExpression, AggregateFunction, Expression, GraphPattern, OrderExpression, PropertyPathExpression,
    QueryDataset,
};
use spargebra::term::{GroundTerm, NamedNodePattern, TermPattern, TriplePattern, Variable};
use spargebra::Query;
use sq_core::batch::{Slot, SlotSet};
use sq_core::eval_api::TempDict;
use sq_core::ir::{
    AggFunc, AggSpec, DatasetSpec, DefaultGraph, ExistsId, Expr, GraphSel, Logical, LoweredQuery, NamedGraphs,
    OrderKey, PatTerm, PathPlan, QuadPat, QueryForm, TemplateTerm, VarOrigin, VarTable,
};
use sq_core::plan::QueryOptions;
use sq_core::store::QuadStore;
use sq_core::term::Term;
use sq_core::{Error, Result, TermId};

/// Lower a parsed query against `store` (constants are resolved against its dictionary).
pub fn lower_query(q: &Query, store: &dyn QuadStore, opts: &QueryOptions) -> Result<LoweredQuery> {
    let (dataset, pattern, base) = match q {
        Query::Select { dataset, pattern, base_iri }
        | Query::Ask { dataset, pattern, base_iri }
        | Query::Construct { dataset, pattern, base_iri, .. }
        | Query::Describe { dataset, pattern, base_iri } => (dataset, pattern, base_iri),
    };
    let mut lw = Lowerer::new(store);
    let root_project = find_root_project(pattern);
    lw.root_project = root_project.map(|p| p as *const GraphPattern);
    let root = lw.pattern(pattern)?;
    let projected = |lw: &mut Lowerer<'_>| -> Result<Vec<Slot>> {
        match root_project {
            Some(GraphPattern::Project { variables, .. }) => variables.iter().map(|v| lw.var_slot(v)).collect(),
            _ => Ok(Vec::new()),
        }
    };
    let form = match q {
        Query::Select { .. } => QueryForm::Select(projected(&mut lw)?),
        Query::Ask { .. } => QueryForm::Ask,
        Query::Describe { .. } => QueryForm::Describe(projected(&mut lw)?),
        Query::Construct { template, .. } => QueryForm::Construct(lw.template(template)?),
    };
    let dataset = dataset_spec(dataset.as_ref(), opts, &mut lw)?;
    let base_iri = base.as_ref().map(|b| b.as_str().to_owned()).or_else(|| opts.base_iri.clone());
    lw.finish(form, root, dataset, base_iri)
}

/// Lower the WHERE pattern of an update operation (spec §5.7) with its USING / WITH dataset.
/// The form is `Select(in-scope slots)`; the map gives the slot of every variable of the
/// pattern's top-level scope (DELETE / INSERT templates are instantiated through it).
pub fn lower_pattern(
    p: &GraphPattern,
    using: Option<&QueryDataset>,
    base: Option<&str>,
    store: &dyn QuadStore,
    opts: &QueryOptions,
) -> Result<(LoweredQuery, FxHashMap<Variable, Slot>)> {
    let mut lw = Lowerer::new(store);
    let root = lw.pattern(p)?;
    let mut in_scope: Vec<Slot> = Vec::new();
    let mut seen = SlotSet::EMPTY;
    let mut names: Vec<&Variable> = Vec::new();
    p.on_in_scope_variable(|v| names.push(v));
    for v in names {
        let s = lw.var_slot(v)?;
        if !seen.contains(s) {
            seen.insert(s);
            in_scope.push(s);
        }
    }
    let dataset = dataset_spec(using, opts, &mut lw)?;
    let map: FxHashMap<Variable, Slot> =
        lw.scopes[0].vars.iter().map(|(name, slot)| (Variable::new_unchecked(name.clone()), *slot)).collect();
    let base_iri = base.map(str::to_owned).or_else(|| opts.base_iri.clone());
    let q = lw.finish(QueryForm::Select(in_scope), root, dataset, base_iri)?;
    Ok((q, map))
}

/// The top-level `Project` of a query (below Slice / Distinct / Reduced), which does NOT open
/// a sub-select scope.
fn find_root_project(p: &GraphPattern) -> Option<&GraphPattern> {
    match p {
        GraphPattern::Slice { inner, .. } | GraphPattern::Distinct { inner } | GraphPattern::Reduced { inner } => {
            find_root_project(inner)
        }
        GraphPattern::Project { .. } => Some(p),
        _ => None,
    }
}

/// Dataset per spec §5.6. `named: None` (update `WITH`) means every named graph of the store.
fn dataset_spec(ds: Option<&QueryDataset>, opts: &QueryOptions, lw: &mut Lowerer<'_>) -> Result<DatasetSpec> {
    let Some(ds) = ds else {
        let default = if opts.union_default_graph { DefaultGraph::UnionAll } else { DefaultGraph::StoreDefault };
        return Ok(DatasetSpec { default, named: NamedGraphs::All });
    };
    let mut ids = |list: &[spargebra::term::NamedNode]| -> Result<Arc<[TermId]>> {
        let mut v = Vec::with_capacity(list.len());
        for g in list {
            v.push(lw.constant(g.as_ref().into())?);
        }
        v.sort();
        v.dedup();
        Ok(v.into())
    };
    let default = DefaultGraph::Graphs(ids(&ds.default)?);
    let named = match &ds.named {
        None => NamedGraphs::All,
        Some(list) => NamedGraphs::Only(ids(list)?),
    };
    Ok(DatasetSpec { default, named })
}

/// Variable names spargebra generates (`format!("{:x}", random::<u128>())`). Only used to
/// classify `VarOrigin` for display; never to rename anything.
fn looks_generated(name: &str) -> bool {
    name.len() >= 20 && name.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[derive(Default)]
struct Scope {
    vars: FxHashMap<String, Slot>,
}

struct Lowerer<'a> {
    store: &'a dyn QuadStore,
    vars: Vec<(String, VarOrigin)>,
    temps: TempDict,
    exists: Vec<Logical>,
    /// Variable scopes: `scopes[0]` is the query scope, one more per enclosing sub-select.
    scopes: Vec<Scope>,
    /// Blank-node label maps: one per variable scope and one per EXISTS pattern.
    bnodes: Vec<FxHashMap<String, Slot>>,
    /// Inside `GRAPH`: quad patterns read the active graph.
    in_graph: bool,
    root_project: Option<*const GraphPattern>,
    consts: FxHashMap<oxrdf::Term, TermId>,
}

impl<'a> Lowerer<'a> {
    fn new(store: &'a dyn QuadStore) -> Self {
        Lowerer {
            store,
            vars: Vec::new(),
            temps: TempDict::new(),
            exists: Vec::new(),
            scopes: vec![Scope::default()],
            bnodes: vec![FxHashMap::default()],
            in_graph: false,
            root_project: None,
            consts: FxHashMap::default(),
        }
    }

    fn finish(self, form: QueryForm, root: Logical, dataset: DatasetSpec, base_iri: Option<String>) -> Result<LoweredQuery> {
        Ok(LoweredQuery {
            form,
            root,
            vars: VarTable { vars: self.vars },
            dataset,
            exists: self.exists,
            temps: self.temps,
            base_iri,
        })
    }

    fn alloc(&mut self, name: String, origin: VarOrigin) -> Result<Slot> {
        if self.vars.len() >= SlotSet::MAX_SLOTS {
            return Err(Error::Unsupported(format!(
                "query needs more than {} variables (including hidden ones)",
                SlotSet::MAX_SLOTS
            )));
        }
        self.vars.push((name, origin));
        Ok((self.vars.len() - 1) as Slot)
    }

    /// Slot of a variable in the innermost scope (allocated on first use).
    fn var_slot(&mut self, v: &Variable) -> Result<Slot> {
        self.named_slot(v.as_str(), VarOrigin::User)
    }

    fn named_slot(&mut self, name: &str, origin: VarOrigin) -> Result<Slot> {
        if let Some(s) = self.scopes.last().and_then(|sc| sc.vars.get(name)) {
            return Ok(*s);
        }
        // Inner-only variables of a sub-select are alpha-renamed: they get their own slot
        // even when an enclosing scope uses the same name.
        let origin = if self.scopes.len() > 1 && origin == VarOrigin::User { VarOrigin::Renamed } else { origin };
        let s = self.alloc(name.to_owned(), origin)?;
        self.scopes.last_mut().expect("scope stack is never empty").vars.insert(name.to_owned(), s);
        Ok(s)
    }

    /// Variable introduced by spargebra (aggregate outputs, GROUP BY / DESCRIBE helpers).
    fn generated_slot(&mut self, v: &Variable) -> Result<Slot> {
        self.named_slot(v.as_str(), VarOrigin::Generated)
    }

    fn bnode_slot(&mut self, label: &str) -> Result<Slot> {
        if let Some(s) = self.bnodes.last().and_then(|m| m.get(label)) {
            return Ok(*s);
        }
        let s = self.alloc(format!("_:{label}"), VarOrigin::BlankNode)?;
        self.bnodes.last_mut().expect("bnode scope stack is never empty").insert(label.to_owned(), s);
        Ok(s)
    }

    fn constant(&mut self, t: oxrdf::TermRef<'_>) -> Result<TermId> {
        let key = t.into_owned();
        if let Some(id) = self.consts.get(&key) {
            return Ok(*id);
        }
        let id = match self.store.dict().lookup(t)? {
            Some(id) => id,
            None => self.temps.intern(Term::from_oxrdf(t)),
        };
        self.consts.insert(key, id);
        Ok(id)
    }

    fn graph_sel(&self) -> GraphSel {
        if self.in_graph { GraphSel::Active } else { GraphSel::Default }
    }

    // ------------------------------------------------------------------ patterns

    fn pattern(&mut self, p: &GraphPattern) -> Result<Logical> {
        Ok(match p {
            GraphPattern::Bgp { patterns } => {
                if patterns.is_empty() {
                    Logical::Unit
                } else {
                    let g = self.graph_sel();
                    let quads = patterns.iter().map(|t| self.quad(t, g)).collect::<Result<Vec<_>>>()?;
                    Logical::Bgp(quads)
                }
            }
            GraphPattern::Path { subject, path, object } => {
                let s = self.term_pattern(subject)?;
                let path = self.path(path)?;
                let o = self.term_pattern(object)?;
                Logical::Path { g: self.graph_sel(), s, path, o }
            }
            GraphPattern::Join { left, right } => {
                Logical::Join(Box::new(self.pattern(left)?), Box::new(self.pattern(right)?))
            }
            GraphPattern::LeftJoin { left, right, expression } => {
                let left = Box::new(self.pattern(left)?);
                let right = Box::new(self.pattern(right)?);
                let filter = expression.as_ref().map(|e| self.expr(e)).transpose()?;
                Logical::LeftJoin { left, right, filter }
            }
            GraphPattern::Filter { expr, inner } => {
                let input = Box::new(self.pattern(inner)?);
                Logical::Filter { input, expr: self.expr(expr)? }
            }
            GraphPattern::Union { left, right } => {
                Logical::Union(Box::new(self.pattern(left)?), Box::new(self.pattern(right)?))
            }
            GraphPattern::Graph { name, inner } => {
                let name = match name {
                    NamedNodePattern::NamedNode(n) => PatTerm::Const(self.constant(n.as_ref().into())?),
                    NamedNodePattern::Variable(v) => PatTerm::Slot(self.var_slot(v)?),
                };
                let saved = std::mem::replace(&mut self.in_graph, true);
                let input = self.pattern(inner);
                self.in_graph = saved;
                Logical::Graph { name, input: Box::new(input?) }
            }
            GraphPattern::Extend { inner, variable, expression } => {
                let input = self.pattern(inner)?;
                let expr = self.expr(expression)?;
                // DESCRIBE <iri> binds the IRI to a spargebra-generated variable.
                let generated =
                    looks_generated(variable.as_str()) && matches!(expression, Expression::NamedNode(_));
                let slot = if generated { self.generated_slot(variable)? } else { self.var_slot(variable)? };
                Logical::Extend { input: Box::new(input), slot, expr }
            }
            GraphPattern::Minus { left, right } => {
                Logical::Minus { left: Box::new(self.pattern(left)?), right: Box::new(self.pattern(right)?) }
            }
            GraphPattern::Values { variables, bindings } => {
                let slots = variables.iter().map(|v| self.var_slot(v)).collect::<Result<Vec<_>>>()?;
                let mut rows = Vec::with_capacity(bindings.len());
                for b in bindings {
                    let mut row = Vec::with_capacity(b.len());
                    for t in b {
                        row.push(match t {
                            None => TermId::UNBOUND,
                            Some(t) => self.ground(t)?,
                        });
                    }
                    rows.push(row);
                }
                Logical::Values { slots, rows }
            }
            GraphPattern::OrderBy { inner, expression } => {
                let input = Box::new(self.pattern(inner)?);
                let keys = expression
                    .iter()
                    .map(|o| {
                        Ok(match o {
                            OrderExpression::Asc(e) => OrderKey { expr: self.expr(e)?, desc: false },
                            OrderExpression::Desc(e) => OrderKey { expr: self.expr(e)?, desc: true },
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                Logical::OrderBy { input, keys }
            }
            GraphPattern::Project { inner, variables } => {
                if self.root_project == Some(p as *const GraphPattern) {
                    let input = Box::new(self.pattern(inner)?);
                    let slots = variables.iter().map(|v| self.var_slot(v)).collect::<Result<Vec<_>>>()?;
                    Logical::Project { input, slots }
                } else {
                    self.sub_select(inner, variables)?
                }
            }
            GraphPattern::Distinct { inner } => Logical::Distinct(Box::new(self.pattern(inner)?)),
            GraphPattern::Reduced { inner } => Logical::Reduced(Box::new(self.pattern(inner)?)),
            GraphPattern::Slice { inner, start, length } => Logical::Slice {
                input: Box::new(self.pattern(inner)?),
                offset: *start as u64,
                limit: length.map(|l| l as u64),
            },
            GraphPattern::Group { inner, variables, aggregates } => {
                let input = Box::new(self.pattern(inner)?);
                let keys = variables.iter().map(|v| self.var_slot(v)).collect::<Result<Vec<_>>>()?;
                // `GROUP BY (expr)` without AS binds a spargebra-generated variable.
                for (v, s) in variables.iter().zip(&keys) {
                    if looks_generated(v.as_str()) {
                        self.vars[*s as usize].1 = VarOrigin::Generated;
                    }
                }
                let mut aggs = Vec::with_capacity(aggregates.len());
                for (v, a) in aggregates {
                    let spec = self.aggregate(a)?;
                    aggs.push((self.generated_slot(v)?, spec));
                }
                Logical::Group { input, keys, aggs }
            }
            GraphPattern::Service { silent, .. } => Logical::Service { silent: *silent },
        })
    }

    /// A non-root `Project`: projected variables keep the enclosing scope's slots, all other
    /// variables (and blank nodes) of the sub-select live in a fresh scope.
    fn sub_select(&mut self, inner: &GraphPattern, variables: &[Variable]) -> Result<Logical> {
        let slots = variables.iter().map(|v| self.var_slot(v)).collect::<Result<Vec<_>>>()?;
        let mut scope = Scope::default();
        for (v, s) in variables.iter().zip(&slots) {
            scope.vars.insert(v.as_str().to_owned(), *s);
        }
        self.scopes.push(scope);
        self.bnodes.push(FxHashMap::default());
        let input = self.pattern(inner);
        self.bnodes.pop();
        self.scopes.pop();
        Ok(Logical::Project { input: Box::new(input?), slots })
    }

    fn quad(&mut self, t: &TriplePattern, g: GraphSel) -> Result<QuadPat> {
        Ok(QuadPat {
            g,
            s: self.term_pattern(&t.subject)?,
            p: match &t.predicate {
                NamedNodePattern::NamedNode(n) => PatTerm::Const(self.constant(n.as_ref().into())?),
                NamedNodePattern::Variable(v) => PatTerm::Slot(self.var_slot(v)?),
            },
            o: self.term_pattern(&t.object)?,
        })
    }

    fn term_pattern(&mut self, t: &TermPattern) -> Result<PatTerm> {
        #[allow(unreachable_patterns)]
        Ok(match t {
            TermPattern::NamedNode(n) => PatTerm::Const(self.constant(n.as_ref().into())?),
            TermPattern::Literal(l) => PatTerm::Const(self.constant(l.as_ref().into())?),
            TermPattern::BlankNode(b) => PatTerm::Slot(self.bnode_slot(b.as_str())?),
            TermPattern::Variable(v) => PatTerm::Slot(self.var_slot(v)?),
            _ => return Err(Error::Unsupported("RDF 1.2 triple terms".into())),
        })
    }

    fn ground(&mut self, t: &GroundTerm) -> Result<TermId> {
        #[allow(unreachable_patterns)]
        match t {
            GroundTerm::NamedNode(n) => self.constant(n.as_ref().into()),
            GroundTerm::Literal(l) => self.constant(l.as_ref().into()),
            _ => Err(Error::Unsupported("RDF 1.2 triple terms".into())),
        }
    }

    fn path(&mut self, p: &PropertyPathExpression) -> Result<PathPlan> {
        Ok(match p {
            PropertyPathExpression::NamedNode(n) => PathPlan::Pred(self.constant(n.as_ref().into())?),
            PropertyPathExpression::Reverse(p) => PathPlan::Inv(Box::new(self.path(p)?)),
            PropertyPathExpression::Sequence(a, b) => PathPlan::Seq(Box::new(self.path(a)?), Box::new(self.path(b)?)),
            PropertyPathExpression::Alternative(a, b) => {
                PathPlan::Alt(Box::new(self.path(a)?), Box::new(self.path(b)?))
            }
            PropertyPathExpression::ZeroOrMore(p) => PathPlan::ZeroOrMore(Box::new(self.path(p)?)),
            PropertyPathExpression::OneOrMore(p) => PathPlan::OneOrMore(Box::new(self.path(p)?)),
            PropertyPathExpression::ZeroOrOne(p) => PathPlan::ZeroOrOne(Box::new(self.path(p)?)),
            PropertyPathExpression::NegatedPropertySet(list) => {
                let mut ids = Vec::with_capacity(list.len());
                for n in list {
                    ids.push(self.constant(n.as_ref().into())?);
                }
                PathPlan::Nps(ids)
            }
        })
    }

    // ------------------------------------------------------------------ expressions

    fn expr(&mut self, e: &Expression) -> Result<Expr> {
        let b = |x: Expr| Box::new(x);
        Ok(match e {
            Expression::NamedNode(n) => Expr::Const(self.constant(n.as_ref().into())?),
            Expression::Literal(l) => Expr::Const(self.constant(l.as_ref().into())?),
            Expression::Variable(v) => Expr::Slot(self.var_slot(v)?),
            Expression::Or(x, y) => Expr::Or(b(self.expr(x)?), b(self.expr(y)?)),
            Expression::And(x, y) => Expr::And(b(self.expr(x)?), b(self.expr(y)?)),
            Expression::Equal(x, y) => Expr::Equal(b(self.expr(x)?), b(self.expr(y)?)),
            Expression::SameTerm(x, y) => Expr::SameTerm(b(self.expr(x)?), b(self.expr(y)?)),
            Expression::Greater(x, y) => Expr::Greater(b(self.expr(x)?), b(self.expr(y)?)),
            Expression::GreaterOrEqual(x, y) => Expr::GreaterOrEqual(b(self.expr(x)?), b(self.expr(y)?)),
            Expression::Less(x, y) => Expr::Less(b(self.expr(x)?), b(self.expr(y)?)),
            Expression::LessOrEqual(x, y) => Expr::LessOrEqual(b(self.expr(x)?), b(self.expr(y)?)),
            Expression::In(x, list) => {
                let x = b(self.expr(x)?);
                Expr::In(x, list.iter().map(|e| self.expr(e)).collect::<Result<_>>()?)
            }
            Expression::Add(x, y) => Expr::Add(b(self.expr(x)?), b(self.expr(y)?)),
            Expression::Subtract(x, y) => Expr::Subtract(b(self.expr(x)?), b(self.expr(y)?)),
            Expression::Multiply(x, y) => Expr::Multiply(b(self.expr(x)?), b(self.expr(y)?)),
            Expression::Divide(x, y) => Expr::Divide(b(self.expr(x)?), b(self.expr(y)?)),
            Expression::UnaryPlus(x) => Expr::UnaryPlus(b(self.expr(x)?)),
            Expression::UnaryMinus(x) => Expr::UnaryMinus(b(self.expr(x)?)),
            Expression::Not(x) => Expr::Not(b(self.expr(x)?)),
            Expression::Exists(p) => {
                // Same variable scope (substitution semantics), own blank-node labels.
                self.bnodes.push(FxHashMap::default());
                let pat = self.pattern(p);
                self.bnodes.pop();
                let id = ExistsId(self.exists.len() as u32);
                self.exists.push(pat?);
                Expr::Exists(id)
            }
            Expression::Bound(v) => Expr::Bound(self.var_slot(v)?),
            Expression::If(x, y, z) => Expr::If(b(self.expr(x)?), b(self.expr(y)?), b(self.expr(z)?)),
            Expression::Coalesce(list) => Expr::Coalesce(list.iter().map(|e| self.expr(e)).collect::<Result<_>>()?),
            Expression::FunctionCall(f, args) => {
                Expr::Call(f.clone(), args.iter().map(|e| self.expr(e)).collect::<Result<_>>()?)
            }
        })
    }

    fn aggregate(&mut self, a: &AggregateExpression) -> Result<AggSpec> {
        Ok(match a {
            AggregateExpression::CountSolutions { distinct } => {
                AggSpec { func: AggFunc::CountAll, distinct: *distinct, arg: None }
            }
            AggregateExpression::FunctionCall { name, expr, distinct } => {
                let func = match name {
                    AggregateFunction::Count => AggFunc::Count,
                    AggregateFunction::Sum => AggFunc::Sum,
                    AggregateFunction::Avg => AggFunc::Avg,
                    AggregateFunction::Min => AggFunc::Min,
                    AggregateFunction::Max => AggFunc::Max,
                    AggregateFunction::Sample => AggFunc::Sample,
                    AggregateFunction::GroupConcat { separator } => {
                        AggFunc::GroupConcat { separator: separator.clone().unwrap_or_else(|| " ".to_owned()) }
                    }
                    AggregateFunction::Custom(n) => AggFunc::Custom(n.as_str().to_owned()),
                };
                AggSpec { func, distinct: *distinct, arg: Some(self.expr(expr)?) }
            }
        })
    }

    /// CONSTRUCT template: variables resolve in the query scope; blank nodes are numbered by
    /// first occurrence (fresh per solution at execution time).
    fn template(&mut self, template: &[TriplePattern]) -> Result<Vec<[TemplateTerm; 3]>> {
        let mut labels: FxHashMap<String, u32> = FxHashMap::default();
        let mut out = Vec::with_capacity(template.len());
        for t in template {
            let mut term = |lw: &mut Self, t: &TermPattern| -> Result<TemplateTerm> {
                #[allow(unreachable_patterns)]
                Ok(match t {
                    TermPattern::NamedNode(n) => TemplateTerm::Const(lw.constant(n.as_ref().into())?),
                    TermPattern::Literal(l) => TemplateTerm::Const(lw.constant(l.as_ref().into())?),
                    TermPattern::Variable(v) => TemplateTerm::Slot(lw.var_slot(v)?),
                    TermPattern::BlankNode(b) => {
                        let n = labels.len() as u32;
                        TemplateTerm::BNode(*labels.entry(b.as_str().to_owned()).or_insert(n))
                    }
                    _ => return Err(Error::Unsupported("RDF 1.2 triple terms".into())),
                })
            };
            let s = term(self, &t.subject)?;
            let p = match &t.predicate {
                NamedNodePattern::NamedNode(n) => TemplateTerm::Const(self.constant(n.as_ref().into())?),
                NamedNodePattern::Variable(v) => TemplateTerm::Slot(self.var_slot(v)?),
            };
            let o = term(self, &t.object)?;
            out.push([s, p, o]);
        }
        Ok(out)
    }
}
