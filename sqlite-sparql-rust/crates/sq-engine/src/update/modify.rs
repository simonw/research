//! `DELETE { .. } INSERT { .. } [USING ..] WHERE { .. }` (SPARQL 1.1 Update §3.1.3).
//!
//! 1. The WHERE pattern is lowered with the USING / WITH dataset (`sq_plan::lower_pattern`),
//!    planned against the current store and run to completion: every solution is
//!    materialized before the store changes (no Halloween problem).
//! 2. DELETE templates are instantiated lookup-only: a position that is unbound, or whose
//!    term the store does not know, cannot match a stored quad and drops that quad.
//! 3. INSERT templates are instantiated with a fresh blank node per (solution, template
//!    label); TEMP values (computed terms, query constants absent from the store) are decoded
//!    and interned; a quad with an unbound position, a literal subject, a non-IRI predicate or
//!    a non-IRI graph is skipped.
//! 4. `remove(deletes)`, then `insert(inserts)`.
//!
//! Only the columns the templates read are kept, and the materialized solutions, their
//! decoded TEMP terms and the instantiated quads are charged against `max_memory_mb` (on top
//! of what the WHERE execution itself holds while it runs): an update over a large WHERE
//! fails with `Error::MemoryBudget` like a query that buffers as much.
use std::mem::size_of;
use std::rc::Rc;
use std::sync::Arc;

use oxrdf::Variable;
use rustc_hash::FxHashMap;
use spargebra::algebra::{GraphPattern, QueryDataset};
use spargebra::term::{GraphNamePattern, GroundQuadPattern, GroundTermPattern, NamedNodePattern, QuadPattern, TermPattern};
use sq_core::eval_api::SolutionSource;
use sq_core::store::{Dictionary, QuadStore, QuadStoreWriter};
use sq_core::{Error, Kind, Result, Term, TermId, BATCH_ROWS};
use sq_exec::{ExecCaches, Execution};

use super::Updater;

impl Updater<'_> {
    pub(super) fn delete_insert(
        &mut self,
        delete: &[GroundQuadPattern],
        insert: &[QuadPattern],
        using: Option<&QueryDataset>,
        pattern: &GraphPattern,
    ) -> Result<()> {
        let template_vars = delete.iter().flat_map(ground_vars).chain(insert.iter().flat_map(quad_vars));
        let mut budget = Budget::new(self.opts.max_memory_mb);
        let solutions = self.solutions(pattern, using, template_vars, &mut budget)?;
        if solutions.rows == 0 {
            return Ok(());
        }
        let dict = self.store.dict();
        let mut deletes = DeleteTemplates::compile(dict, delete, &solutions)?.instantiate(dict, &solutions, &mut budget)?;
        let mut inserts =
            InsertTemplates::compile(insert, &solutions).instantiate(&*self.store, &solutions, &mut budget)?;
        self.report.deleted += self.store.remove(&mut deletes)?;
        self.report.inserted += self.store.insert(&mut inserts)?;
        Ok(())
    }

    /// Runs the WHERE pattern to completion, keeping the columns of the template variables;
    /// decodes their TEMP ids while the execution (which owns them) is still alive.
    fn solutions<'v>(
        &self,
        pattern: &GraphPattern,
        using: Option<&QueryDataset>,
        template_vars: impl Iterator<Item = &'v Variable>,
        budget: &mut Budget,
    ) -> Result<Solutions> {
        let store: Rc<dyn QuadStore> = self.store.clone();
        let (lowered, _) = sq_plan::lower_pattern(pattern, using, self.base.as_deref(), &*store, self.opts)?;
        let plan = sq_plan::optimize(lowered, &*store, self.opts)?;
        let mut exec = Execution::new(store, plan, self.opts, ExecCaches::default())?;
        exec.set_now(self.now);
        let all: FxHashMap<&str, usize> = exec.variables().iter().enumerate().map(|(i, v)| (v.as_str(), i)).collect();
        // The execution columns the templates read (a template variable the WHERE pattern
        // never binds has none); `columns` maps each to its position in a kept row.
        let mut used: Vec<(usize, String)> = Vec::new();
        for v in template_vars {
            if let Some(&c) = all.get(v.as_str())
                && !used.iter().any(|(u, _)| *u == c)
            {
                used.push((c, v.as_str().to_owned()));
            }
        }
        let full_width = all.len();
        drop(all);
        let width = used.len();
        let columns: FxHashMap<String, usize> = used.iter().enumerate().map(|(i, (_, v))| (v.clone(), i)).collect();
        let mut batch: Vec<TermId> = Vec::new();
        let mut ids: Vec<TermId> = Vec::new();
        let mut charged = 0;
        let mut rows = 0;
        loop {
            batch.clear();
            let n = exec.next_rows(&mut batch, BATCH_ROWS)?;
            if n == 0 {
                break;
            }
            rows += n;
            if width > 0 {
                for row in batch.chunks_exact(full_width) {
                    ids.extend(used.iter().map(|(c, _)| row[*c]));
                }
                budget.charge_vec(&mut charged, &ids)?;
            }
            budget.check(exec.memory_used())?;
        }
        drop(batch);
        let mut temp_ids: Vec<TermId> = ids.iter().copied().filter(|id| id.is_temp()).collect();
        temp_ids.sort_unstable();
        temp_ids.dedup();
        let mut terms = Vec::with_capacity(temp_ids.len());
        exec.decode(&temp_ids, &mut terms)?;
        drop(exec);
        let mut temps = FxHashMap::default();
        for (id, t) in temp_ids.into_iter().zip(terms) {
            if let Some(t) = t {
                budget.charge(TEMP_ENTRY_BYTES + term_bytes(&t))?;
                temps.insert(id, t);
            }
        }
        Ok(Solutions { columns, width, rows, ids, temps })
    }
}

/// The update's share of `max_memory_mb`: everything it materializes for one operation.
struct Budget {
    limit: u64,
    mb: u64,
    used: u64,
}

impl Budget {
    fn new(mb: u64) -> Self {
        Budget { limit: mb.saturating_mul(1 << 20), mb, used: 0 }
    }

    /// Fails when what is charged plus `elsewhere` bytes (held by a running execution)
    /// exceeds the budget.
    fn check(&self, elsewhere: u64) -> Result<()> {
        if self.used.saturating_add(elsewhere) > self.limit {
            return Err(Error::MemoryBudget(self.mb));
        }
        Ok(())
    }

    fn charge(&mut self, bytes: usize) -> Result<()> {
        self.used += bytes as u64;
        self.check(0)
    }

    /// Charges the growth of `v`'s allocation since the last call (`charged` = the capacity
    /// already charged).
    fn charge_vec<T>(&mut self, charged: &mut usize, v: &Vec<T>) -> Result<()> {
        let cap = v.capacity();
        if cap <= *charged {
            return Ok(());
        }
        let grown = (cap - *charged) * size_of::<T>();
        *charged = cap;
        self.charge(grown)
    }
}

/// Bytes of a decoded TEMP map entry beyond its strings (key, `Arc`, term header, table).
const TEMP_ENTRY_BYTES: usize = 96;
/// Bytes of a lookup / intern memo entry (`TermId` key and value, table overhead).
const MEMO_ENTRY_BYTES: usize = 32;

fn term_bytes(t: &Term) -> usize {
    match t {
        Term::Iri(s) | Term::BNode(s) => s.len(),
        Term::Literal(l) => l.lex.len() + l.datatype.len() + l.lang.as_ref().map_or(0, |g| g.len()),
    }
}

/// The materialized solutions of a WHERE pattern, restricted to the template variables.
struct Solutions {
    /// Position in a row of every template variable the WHERE pattern may bind.
    columns: FxHashMap<String, usize>,
    width: usize,
    rows: usize,
    /// `rows * width` ids, row-major (UNBOUND = 0).
    ids: Vec<TermId>,
    /// The decoded TEMP ids of `ids`.
    temps: FxHashMap<TermId, Arc<Term>>,
}

impl Solutions {
    fn row(&self, i: usize) -> &[TermId] {
        &self.ids[i * self.width..(i + 1) * self.width]
    }

    /// Column of a template variable; None when the WHERE pattern never binds it.
    fn column(&self, v: &Variable) -> Option<usize> {
        self.columns.get(v.as_str()).copied()
    }

    fn temp(&self, id: TermId) -> Result<&Arc<Term>> {
        self.temps.get(&id).ok_or_else(|| sq_core::Error::internal(format!("TEMP id {id:?} was not decoded")))
    }
}

/// Role of a template position: which kinds of terms are legal there.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Role {
    Subject,
    Predicate,
    Object,
    Graph,
}

impl Role {
    fn accepts(self, id: TermId) -> bool {
        match self {
            Role::Subject => matches!(id.kind(), Kind::Iri | Kind::BNode),
            Role::Predicate => id.kind() == Kind::Iri,
            Role::Object => true,
            // The default graph is g = 0; a variable must be bound to an IRI.
            Role::Graph => id == TermId::DEFAULT_GRAPH || id.kind() == Kind::Iri,
        }
    }
}

const ROLES: [Role; 4] = [Role::Graph, Role::Subject, Role::Predicate, Role::Object];

/// A DELETE template position.
#[derive(Copy, Clone)]
enum DelPos {
    /// A constant known to the store (or the default graph).
    Fixed(TermId),
    /// A solution column.
    Column(usize),
}

/// DELETE templates with their constants looked up. Template quads that can never match (a
/// constant absent from the store, a variable the WHERE pattern never binds, a constant in an
/// illegal role) are dropped at compile time.
struct DeleteTemplates(Vec<[DelPos; 4]>);

impl DeleteTemplates {
    fn compile(dict: &dyn Dictionary, templates: &[GroundQuadPattern], sol: &Solutions) -> Result<Self> {
        let mut out = Vec::with_capacity(templates.len());
        'templates: for t in templates {
            let mut quad = [DelPos::Fixed(TermId::DEFAULT_GRAPH); 4];
            let positions = [graph_pattern(&t.graph_name), ground_term(&t.subject), named_node(&t.predicate), ground_term(&t.object)];
            for (i, pos) in positions.into_iter().enumerate() {
                quad[i] = match pos {
                    Tpl::DefaultGraph => DelPos::Fixed(TermId::DEFAULT_GRAPH),
                    Tpl::Term(term) => match dict.lookup(term)? {
                        Some(id) if ROLES[i].accepts(id) => DelPos::Fixed(id),
                        _ => continue 'templates,
                    },
                    Tpl::Var(v) => match sol.column(v) {
                        Some(c) => DelPos::Column(c),
                        None => continue 'templates,
                    },
                    Tpl::BNode(_) => unreachable!("DELETE templates have no blank nodes"),
                };
            }
            out.push(quad);
        }
        Ok(DeleteTemplates(out))
    }

    fn instantiate(&self, dict: &dyn Dictionary, sol: &Solutions, budget: &mut Budget) -> Result<Vec<[TermId; 4]>> {
        let mut out = Vec::new();
        if self.0.is_empty() {
            return Ok(out);
        }
        // TEMP values resolve to stored ids by lookup, at most once each.
        let mut resolved: FxHashMap<TermId, Option<TermId>> = FxHashMap::default();
        let (mut charged, mut memo) = (0, 0);
        for r in 0..sol.rows {
            budget.charge_vec(&mut charged, &out)?;
            if resolved.len() > memo {
                budget.charge((resolved.len() - memo) * MEMO_ENTRY_BYTES)?;
                memo = resolved.len();
            }
            let row = sol.row(r);
            'templates: for t in &self.0 {
                let mut quad = [TermId::UNBOUND; 4];
                for (i, pos) in t.iter().enumerate() {
                    let id = match *pos {
                        DelPos::Fixed(id) => id,
                        DelPos::Column(c) => {
                            let v = row[c];
                            if !v.is_bound() {
                                continue 'templates;
                            }
                            let v = if v.is_temp() { lookup_temp(dict, sol, &mut resolved, v)? } else { Some(v) };
                            match v {
                                Some(id) if ROLES[i].accepts(id) => id,
                                _ => continue 'templates,
                            }
                        }
                    };
                    quad[i] = id;
                }
                out.push(quad);
            }
        }
        budget.charge_vec(&mut charged, &out)?;
        Ok(out)
    }
}

/// The stored id of a TEMP value, if the store holds that term (TEMP blank nodes never are).
fn lookup_temp(
    dict: &dyn Dictionary,
    sol: &Solutions,
    resolved: &mut FxHashMap<TermId, Option<TermId>>,
    id: TermId,
) -> Result<Option<TermId>> {
    if let Some(r) = resolved.get(&id) {
        return Ok(*r);
    }
    let r = match &**sol.temp(id)? {
        Term::BNode(_) => None,
        t => dict.lookup(t.to_oxrdf().as_ref())?,
    };
    resolved.insert(id, r);
    Ok(r)
}

/// An INSERT template position.
#[derive(Copy, Clone)]
enum InsPos {
    DefaultGraph,
    /// Index into [`InsertTemplates::consts`].
    Const(usize),
    Column(usize),
    /// Index of the template blank-node label.
    BNode(usize),
}

/// INSERT templates. Constants are interned lazily, so a template that never produces a
/// quad leaves the dictionary untouched.
struct InsertTemplates {
    quads: Vec<[InsPos; 4]>,
    consts: Vec<(oxrdf::Term, Option<TermId>)>,
    labels: usize,
}

impl InsertTemplates {
    fn compile(templates: &[QuadPattern], sol: &Solutions) -> Self {
        let mut consts: Vec<(oxrdf::Term, Option<TermId>)> = Vec::new();
        let mut labels: FxHashMap<&str, usize> = FxHashMap::default();
        let mut quads = Vec::with_capacity(templates.len());
        'templates: for t in templates {
            let mut quad = [InsPos::DefaultGraph; 4];
            let positions = [graph_pattern(&t.graph_name), term_pattern(&t.subject), named_node(&t.predicate), term_pattern(&t.object)];
            for (i, pos) in positions.into_iter().enumerate() {
                quad[i] = match pos {
                    Tpl::DefaultGraph => InsPos::DefaultGraph,
                    Tpl::Term(term) => {
                        let legal = match ROLES[i] {
                            Role::Subject => !matches!(term, oxrdf::TermRef::Literal(_)),
                            Role::Predicate | Role::Graph => matches!(term, oxrdf::TermRef::NamedNode(_)),
                            Role::Object => true,
                        };
                        if !legal {
                            continue 'templates;
                        }
                        let owned = term.into_owned();
                        let k = match consts.iter().position(|(c, _)| *c == owned) {
                            Some(k) => k,
                            None => {
                                consts.push((owned, None));
                                consts.len() - 1
                            }
                        };
                        InsPos::Const(k)
                    }
                    Tpl::Var(v) => match sol.column(v) {
                        Some(c) => InsPos::Column(c),
                        None => continue 'templates,
                    },
                    Tpl::BNode(label) => {
                        if matches!(ROLES[i], Role::Predicate | Role::Graph) {
                            continue 'templates;
                        }
                        let n = labels.len();
                        InsPos::BNode(*labels.entry(label).or_insert(n))
                    }
                };
            }
            quads.push(quad);
        }
        InsertTemplates { quads, consts, labels: labels.len() }
    }

    fn instantiate(
        mut self,
        store: &dyn QuadStoreWriter,
        sol: &Solutions,
        budget: &mut Budget,
    ) -> Result<Vec<[TermId; 4]>> {
        let mut out = Vec::new();
        if self.quads.is_empty() {
            return Ok(out);
        }
        // TEMP values interned once per operation; a TEMP blank node becomes one fresh store
        // blank node for the whole operation (it is the same node in every solution).
        let mut interned: FxHashMap<TermId, TermId> = FxHashMap::default();
        let mut bnodes: Vec<Option<TermId>> = vec![None; self.labels];
        let (mut charged, mut memo) = (0, 0);
        for r in 0..sol.rows {
            budget.charge_vec(&mut charged, &out)?;
            if interned.len() > memo {
                budget.charge((interned.len() - memo) * MEMO_ENTRY_BYTES)?;
                memo = interned.len();
            }
            let row = sol.row(r);
            bnodes.iter_mut().for_each(|b| *b = None);
            'templates: for t in &self.quads {
                // Check unbound / illegal positions before allocating anything for this quad.
                for (i, pos) in t.iter().enumerate() {
                    if let InsPos::Column(c) = *pos {
                        let v = row[c];
                        if !v.is_bound() || !ROLES[i].accepts(v) {
                            continue 'templates;
                        }
                    }
                }
                let mut quad = [TermId::UNBOUND; 4];
                for (i, pos) in t.iter().enumerate() {
                    quad[i] = match *pos {
                        InsPos::DefaultGraph => TermId::DEFAULT_GRAPH,
                        InsPos::Const(k) => match self.consts[k].1 {
                            Some(id) => id,
                            None => {
                                let id = store.intern(self.consts[k].0.as_ref())?;
                                self.consts[k].1 = Some(id);
                                id
                            }
                        },
                        InsPos::Column(c) => {
                            let v = row[c];
                            if v.is_temp() { intern_temp(store, sol, &mut interned, v)? } else { v }
                        }
                        InsPos::BNode(l) => match bnodes[l] {
                            Some(id) => id,
                            None => {
                                let id = store.fresh_bnode()?;
                                bnodes[l] = Some(id);
                                id
                            }
                        },
                    };
                }
                out.push(quad);
            }
        }
        budget.charge_vec(&mut charged, &out)?;
        Ok(out)
    }
}

/// The stored id of a TEMP value, interning it (or a fresh blank node for a TEMP blank node).
fn intern_temp(
    store: &dyn QuadStoreWriter,
    sol: &Solutions,
    interned: &mut FxHashMap<TermId, TermId>,
    id: TermId,
) -> Result<TermId> {
    if let Some(r) = interned.get(&id) {
        return Ok(*r);
    }
    let r = match &**sol.temp(id)? {
        Term::BNode(_) => store.fresh_bnode()?,
        t => store.intern(t.to_oxrdf().as_ref())?,
    };
    interned.insert(id, r);
    Ok(r)
}

/// A template position, independent of the template kind.
enum Tpl<'a> {
    DefaultGraph,
    Term(oxrdf::TermRef<'a>),
    Var(&'a Variable),
    BNode(&'a str),
}

fn ground_term(t: &GroundTermPattern) -> Tpl<'_> {
    match t {
        GroundTermPattern::NamedNode(n) => Tpl::Term(n.as_ref().into()),
        GroundTermPattern::Literal(l) => Tpl::Term(l.as_ref().into()),
        GroundTermPattern::Variable(v) => Tpl::Var(v),
    }
}

fn term_pattern(t: &TermPattern) -> Tpl<'_> {
    match t {
        TermPattern::NamedNode(n) => Tpl::Term(n.as_ref().into()),
        TermPattern::Literal(l) => Tpl::Term(l.as_ref().into()),
        TermPattern::BlankNode(b) => Tpl::BNode(b.as_str()),
        TermPattern::Variable(v) => Tpl::Var(v),
    }
}

fn named_node(t: &NamedNodePattern) -> Tpl<'_> {
    match t {
        NamedNodePattern::NamedNode(n) => Tpl::Term(n.as_ref().into()),
        NamedNodePattern::Variable(v) => Tpl::Var(v),
    }
}

fn graph_pattern(g: &GraphNamePattern) -> Tpl<'_> {
    match g {
        GraphNamePattern::NamedNode(n) => Tpl::Term(n.as_ref().into()),
        GraphNamePattern::DefaultGraph => Tpl::DefaultGraph,
        GraphNamePattern::Variable(v) => Tpl::Var(v),
    }
}

fn ground_vars(t: &GroundQuadPattern) -> impl Iterator<Item = &Variable> {
    let g = match &t.graph_name {
        GraphNamePattern::Variable(v) => Some(v),
        _ => None,
    };
    let s = match &t.subject {
        GroundTermPattern::Variable(v) => Some(v),
        _ => None,
    };
    let p = match &t.predicate {
        NamedNodePattern::Variable(v) => Some(v),
        NamedNodePattern::NamedNode(_) => None,
    };
    let o = match &t.object {
        GroundTermPattern::Variable(v) => Some(v),
        _ => None,
    };
    [g, s, p, o].into_iter().flatten()
}

fn quad_vars(t: &QuadPattern) -> impl Iterator<Item = &Variable> {
    let g = match &t.graph_name {
        GraphNamePattern::Variable(v) => Some(v),
        _ => None,
    };
    let s = match &t.subject {
        TermPattern::Variable(v) => Some(v),
        _ => None,
    };
    let p = match &t.predicate {
        NamedNodePattern::Variable(v) => Some(v),
        NamedNodePattern::NamedNode(_) => None,
    };
    let o = match &t.object {
        TermPattern::Variable(v) => Some(v),
        _ => None,
    };
    [g, s, p, o].into_iter().flatten()
}
