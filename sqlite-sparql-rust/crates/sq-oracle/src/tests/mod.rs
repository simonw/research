//! Unit tests over hand-built Logical IR on MemStore (no planner involved).
//!
//! IRIs are written `:local` (namespace `http://ex/`); results are compared as rendered
//! terms, `-` for unbound.
mod aggregates;
mod exists;
mod forms;
mod graphs;
mod paths;
mod patterns;

use std::str::FromStr;
use std::sync::Arc;

use sq_core::eval_api::TempDict;
use sq_core::spargebra::algebra::Function;
use sq_core::ir::{
    DatasetSpec, DefaultGraph, Expr, GraphSel, Logical, LoweredQuery, NamedGraphs, OrderKey, PatTerm, QuadPat,
    QueryForm, VarOrigin, VarTable,
};
use sq_core::store::QuadStoreWriter;
use sq_core::{Literal, Slot, Term, TermId};
use sq_memstore::MemStore;

use crate::{OracleOptions, OracleOutput};

pub(crate) const NS: &str = "http://ex/";

/// A store, a variable table and a TempDict to build queries against.
pub(crate) struct Fx {
    pub(crate) store: MemStore,
    vars: Vec<(String, VarOrigin)>,
    temps: TempDict,
    exists: Vec<Logical>,
    dataset: DatasetSpec,
}

impl Fx {
    pub(crate) fn new() -> Self {
        Fx {
            store: MemStore::new(),
            vars: Vec::new(),
            temps: TempDict::new(),
            exists: Vec::new(),
            dataset: DatasetSpec { default: DefaultGraph::StoreDefault, named: NamedGraphs::All },
        }
    }

    // ------------------------------------------------------------------ terms and data

    /// `:local`, interned in the store dictionary.
    pub(crate) fn iri(&self, local: &str) -> TermId {
        self.store.intern(oxrdf::NamedNodeRef::new_unchecked(&format!("{NS}{local}")).into()).unwrap()
    }

    pub(crate) fn lit(&self, lex: &str) -> TermId {
        self.store.intern(oxrdf::LiteralRef::new_simple_literal(lex).into()).unwrap()
    }

    pub(crate) fn int(&self, v: i64) -> TermId {
        TermId::from_int(v).unwrap()
    }

    pub(crate) fn bnode(&self) -> TermId {
        self.store.fresh_bnode().unwrap()
    }

    /// `:local` as a query constant that is absent from the store (a TEMP id).
    pub(crate) fn absent(&mut self, local: &str) -> TermId {
        self.temps.intern(Term::Iri(format!("{NS}{local}").into()))
    }

    /// A simple-literal query constant absent from the store (longer than 7 bytes).
    pub(crate) fn absent_lit(&mut self, lex: &str) -> TermId {
        self.temps.intern(Term::Literal(Literal::simple(lex)))
    }

    /// Adds a triple to the store's default graph.
    pub(crate) fn add(&self, s: TermId, p: TermId, o: TermId) {
        self.add_in(TermId::DEFAULT_GRAPH, s, p, o);
    }

    pub(crate) fn add_in(&self, g: TermId, s: TermId, p: TermId, o: TermId) {
        self.store.insert(&mut vec![[g, s, p, o]]).unwrap();
    }

    // ------------------------------------------------------------------ query building

    pub(crate) fn var(&mut self, name: &str) -> Slot {
        self.slot(name, VarOrigin::User)
    }

    /// A blank-node pattern variable (hidden from `SELECT *` and `COUNT(DISTINCT *)`).
    pub(crate) fn hidden(&mut self, name: &str) -> Slot {
        self.slot(&format!("_:{name}"), VarOrigin::BlankNode)
    }

    fn slot(&mut self, name: &str, origin: VarOrigin) -> Slot {
        self.vars.push((name.to_owned(), origin));
        (self.vars.len() - 1) as Slot
    }

    /// Registers an EXISTS pattern and returns the expression referencing it.
    pub(crate) fn exists(&mut self, pattern: Logical) -> Expr {
        self.exists.push(pattern);
        Expr::Exists(sq_core::ir::ExistsId(self.exists.len() as u32 - 1))
    }

    pub(crate) fn set_dataset(&mut self, default: DefaultGraph, named: NamedGraphs) {
        self.dataset = DatasetSpec { default, named };
    }

    pub(crate) fn query(&self, form: QueryForm, root: Logical) -> LoweredQuery {
        LoweredQuery {
            form,
            root,
            vars: VarTable { vars: self.vars.clone() },
            dataset: self.dataset.clone(),
            exists: self.exists.clone(),
            temps: self.temps.clone(),
            base_iri: Some(NS.to_owned()),
        }
    }

    // ------------------------------------------------------------------ running

    pub(crate) fn run(&self, form: QueryForm, root: Logical) -> OracleOutput {
        self.try_run(form, root).expect("evaluation succeeds")
    }

    pub(crate) fn try_run(&self, form: QueryForm, root: Logical) -> sq_core::Result<OracleOutput> {
        let opts = OracleOptions {
            now: Some(oxsdatatypes::DateTime::from_str("2026-10-15T12:00:00Z").unwrap()),
            rng_seed: Some(7),
        };
        crate::evaluate_with(&self.query(form, root), &self.store, &opts)
    }

    /// SELECT `slots`: rendered rows in solution order.
    pub(crate) fn select(&self, slots: &[Slot], root: Logical) -> Vec<Vec<String>> {
        match self.run(QueryForm::Select(slots.to_vec()), root) {
            OracleOutput::Solutions { vars, rows } => {
                assert_eq!(vars.len(), slots.len());
                rows.iter().map(|r| r.iter().map(|t| show(t.as_deref())).collect()).collect()
            }
            other => panic!("expected solutions, got {other:?}"),
        }
    }

    /// SELECT `slots`: rendered rows, sorted (bag comparison).
    pub(crate) fn bag(&self, slots: &[Slot], root: Logical) -> Vec<Vec<String>> {
        let mut rows = self.select(slots, root);
        rows.sort();
        rows
    }

    pub(crate) fn ask(&self, root: Logical) -> bool {
        match self.run(QueryForm::Ask, root) {
            OracleOutput::Boolean(b) => b,
            other => panic!("expected a boolean, got {other:?}"),
        }
    }
}

/// `:local` for IRIs in the test namespace, N-Triples otherwise, `-` for unbound.
pub(crate) fn show(t: Option<&Term>) -> String {
    match t {
        None => "-".into(),
        Some(Term::Iri(i)) if i.starts_with(NS) => format!(":{}", &i[NS.len()..]),
        Some(t) => t.to_ntriples(),
    }
}

/// `"n"^^xsd:integer`, rendered.
pub(crate) fn int(n: i64) -> String {
    format!("\"{n}\"^^<http://www.w3.org/2001/XMLSchema#integer>")
}

/// Expected rows from string literals.
pub(crate) fn rows(expected: &[&[&str]]) -> Vec<Vec<String>> {
    expected.iter().map(|r| r.iter().map(|s| s.to_string()).collect()).collect()
}

/// Sorted expected rows.
pub(crate) fn bag(expected: &[&[&str]]) -> Vec<Vec<String>> {
    let mut r = rows(expected);
    r.sort();
    r
}

// ---------------------------------------------------------------------- IR shorthands

pub(crate) fn v(s: Slot) -> PatTerm {
    PatTerm::Slot(s)
}

pub(crate) fn c(id: TermId) -> PatTerm {
    PatTerm::Const(id)
}

pub(crate) fn tp(s: PatTerm, p: PatTerm, o: PatTerm) -> QuadPat {
    QuadPat { g: GraphSel::Default, s, p, o }
}

/// A triple pattern reading the active graph (inside `Logical::Graph`).
pub(crate) fn tpa(s: PatTerm, p: PatTerm, o: PatTerm) -> QuadPat {
    QuadPat { g: GraphSel::Active, s, p, o }
}

pub(crate) fn bgp(patterns: Vec<QuadPat>) -> Logical {
    Logical::Bgp(patterns)
}

pub(crate) fn join(a: Logical, b: Logical) -> Logical {
    Logical::Join(Box::new(a), Box::new(b))
}

pub(crate) fn filter(input: Logical, expr: Expr) -> Logical {
    Logical::Filter { input: Box::new(input), expr }
}

pub(crate) fn extend(input: Logical, slot: Slot, expr: Expr) -> Logical {
    Logical::Extend { input: Box::new(input), slot, expr }
}

pub(crate) fn project(input: Logical, slots: &[Slot]) -> Logical {
    Logical::Project { input: Box::new(input), slots: slots.to_vec() }
}

pub(crate) fn values(slots: &[Slot], rows: &[&[TermId]]) -> Logical {
    Logical::Values { slots: slots.to_vec(), rows: rows.iter().map(|r| r.to_vec()).collect() }
}

pub(crate) fn order_by(input: Logical, keys: &[(Expr, bool)]) -> Logical {
    Logical::OrderBy {
        input: Box::new(input),
        keys: keys.iter().map(|(expr, desc)| OrderKey { expr: expr.clone(), desc: *desc }).collect(),
    }
}

pub(crate) fn es(s: Slot) -> Expr {
    Expr::Slot(s)
}

pub(crate) fn ec(id: TermId) -> Expr {
    Expr::Const(id)
}

pub(crate) fn not(e: Expr) -> Expr {
    Expr::Not(Box::new(e))
}

pub(crate) fn eq(a: Expr, b: Expr) -> Expr {
    Expr::Equal(Box::new(a), Box::new(b))
}

pub(crate) fn gt(a: Expr, b: Expr) -> Expr {
    Expr::Greater(Box::new(a), Box::new(b))
}

pub(crate) fn add(a: Expr, b: Expr) -> Expr {
    Expr::Add(Box::new(a), Box::new(b))
}

pub(crate) fn call(f: Function, args: Vec<Expr>) -> Expr {
    Expr::Call(f, args)
}

pub(crate) fn arc_dataset(ids: &[TermId]) -> Arc<[TermId]> {
    Arc::from(ids.to_vec())
}
