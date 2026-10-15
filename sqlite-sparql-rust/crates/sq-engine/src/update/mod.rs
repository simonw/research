//! The SPARQL 1.1 Update executor (docs/SPEC.md §5.7).
//!
//! Operations run in request order and each one sees the effects of the previous ones: every
//! `DELETE/INSERT ... WHERE` is lowered and planned against the store as it is when the
//! operation starts, so query constants inserted by an earlier operation resolve to their
//! dictionary ids. `NOW()` has one value for the whole request. Atomicity is the caller's job
//! (sq-api runs the request inside one write scope).
//!
//! spargebra already rewrites `WITH` (into the templates and `USING`), `DELETE WHERE`, and
//! `ADD` / `MOVE` / `COPY` (into `DROP SILENT` + `INSERT { GRAPH .. } WHERE { GRAPH .. }`).
//!
//! `union_default_graph` is a query-only view (as in Oxigraph): every WHERE clause without
//! `USING` reads the stored default graph (g = 0), the graph that templates, `DELETE DATA`,
//! `CLEAR DEFAULT` and `DROP DEFAULT` write. Otherwise `COPY DEFAULT TO <g>` (lowered to an
//! `INSERT .. WHERE { ?s ?p ?o }` without `USING`) would copy every named graph's triples, and
//! `DELETE WHERE { ?s ?p ?o }` would match triples that its g = 0 template cannot delete.
mod graphs;
mod modify;

use std::rc::Rc;

use oxrdf::{NamedOrBlankNode, TermRef};
use rustc_hash::FxHashMap;
use spargebra::term::{GraphName, GroundQuad, GroundTerm, Quad};
use spargebra::{GraphUpdateOperation, Update};
use sq_core::plan::QueryOptions;
use sq_core::store::QuadStoreWriter;
use sq_core::{Error, Result, TermId};

/// Settings of an update request beyond the query options.
#[derive(Clone, Debug, Default)]
pub struct UpdateOptions {
    /// Allow `LOAD <file://...>` (the `allow_files` connection setting). Other IRI schemes are
    /// never fetched.
    pub allow_files: bool,
    /// The value of `NOW()` for every operation of the request (default: the current time).
    pub now: Option<oxsdatatypes::DateTime>,
}

/// Quads actually added and removed by an update request.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UpdateReport {
    pub inserted: u64,
    pub deleted: u64,
}

pub(crate) fn run(
    store: Rc<dyn QuadStoreWriter>,
    update: &Update,
    opts: &QueryOptions,
    uopts: &UpdateOptions,
) -> Result<UpdateReport> {
    let base = update.base_iri.as_ref().map(|b| b.as_str().to_owned()).or_else(|| opts.base_iri.clone());
    let opts = QueryOptions { union_default_graph: false, ..opts.clone() };
    let mut ex = Updater {
        store,
        opts: &opts,
        allow_files: uopts.allow_files,
        now: uopts.now.unwrap_or_else(oxsdatatypes::DateTime::now),
        base,
        report: UpdateReport::default(),
    };
    for op in &update.operations {
        ex.operation(op)?;
    }
    Ok(ex.report)
}

/// State of one update request.
struct Updater<'a> {
    store: Rc<dyn QuadStoreWriter>,
    opts: &'a QueryOptions,
    allow_files: bool,
    now: oxsdatatypes::DateTime,
    base: Option<String>,
    report: UpdateReport,
}

impl Updater<'_> {
    fn operation(&mut self, op: &GraphUpdateOperation) -> Result<()> {
        match op {
            GraphUpdateOperation::InsertData { data } => self.insert_data(data),
            GraphUpdateOperation::DeleteData { data } => self.delete_data(data),
            GraphUpdateOperation::DeleteInsert { delete, insert, using, pattern } => {
                self.delete_insert(delete, insert, using.as_ref(), pattern)
            }
            GraphUpdateOperation::Load { silent, source, destination } => self.load(*silent, source, destination),
            GraphUpdateOperation::Clear { silent, graph } => self.clear(*silent, graph),
            GraphUpdateOperation::Create { silent, graph } => self.create(*silent, graph),
            GraphUpdateOperation::Drop { silent, graph } => self.drop_graphs(*silent, graph),
        }
    }

    /// `INSERT DATA`: every term is interned; blank-node labels map to fresh blank nodes for
    /// this operation; new named graphs are registered by the insert.
    fn insert_data(&mut self, data: &[Quad]) -> Result<()> {
        let store = &*self.store;
        let mut bnodes = BNodes::default();
        let mut quads = Vec::with_capacity(data.len());
        for q in data {
            let g = match &q.graph_name {
                GraphName::DefaultGraph => TermId::DEFAULT_GRAPH,
                GraphName::NamedNode(n) => store.intern(n.as_ref().into())?,
            };
            let s = match &q.subject {
                NamedOrBlankNode::NamedNode(n) => store.intern(n.as_ref().into())?,
                NamedOrBlankNode::BlankNode(b) => bnodes.get(store, b.as_str())?,
            };
            let p = store.intern(q.predicate.as_ref().into())?;
            let o = match q.object.as_ref() {
                TermRef::BlankNode(b) => bnodes.get(store, b.as_str())?,
                other => store.intern(other)?,
            };
            quads.push([g, s, p, o]);
        }
        self.report.inserted += store.insert(&mut quads)?;
        Ok(())
    }

    /// `DELETE DATA`: lookup only; a quad with a term the store does not know cannot be in
    /// the store and is skipped.
    fn delete_data(&mut self, data: &[GroundQuad]) -> Result<()> {
        let dict = self.store.dict();
        let mut quads = Vec::with_capacity(data.len());
        for q in data {
            let g = match &q.graph_name {
                GraphName::DefaultGraph => Some(TermId::DEFAULT_GRAPH),
                GraphName::NamedNode(n) => dict.lookup(n.as_ref().into())?,
            };
            let s = dict.lookup(q.subject.as_ref().into())?;
            let p = dict.lookup(q.predicate.as_ref().into())?;
            let o = match &q.object {
                GroundTerm::NamedNode(n) => dict.lookup(n.as_ref().into())?,
                GroundTerm::Literal(l) => dict.lookup(l.as_ref().into())?,
            };
            if let (Some(g), Some(s), Some(p), Some(o)) = (g, s, p, o) {
                quads.push([g, s, p, o]);
            }
        }
        self.report.deleted += self.store.remove(&mut quads)?;
        Ok(())
    }
}

/// Blank-node labels of one operation's data, mapped to fresh store blank nodes.
#[derive(Default)]
struct BNodes<'d>(FxHashMap<&'d str, TermId>);

impl<'d> BNodes<'d> {
    fn get(&mut self, store: &dyn QuadStoreWriter, label: &'d str) -> Result<TermId> {
        if let Some(id) = self.0.get(label) {
            return Ok(*id);
        }
        let id = store.fresh_bnode()?;
        self.0.insert(label, id);
        Ok(id)
    }
}

/// `SILENT`: the operation's own failures (a missing or already existing graph, a document
/// that cannot be read or parsed, a source that may not be fetched) become success.
/// Interrupts, timeouts and storage failures are never silenced: they may have left a partial
/// write that only the caller's rollback can undo.
fn silenced(silent: bool, r: Result<()>) -> Result<()> {
    match r {
        Err(Error::Invalid(_) | Error::Parse(_) | Error::Unsupported(_)) if silent => Ok(()),
        r => r,
    }
}
