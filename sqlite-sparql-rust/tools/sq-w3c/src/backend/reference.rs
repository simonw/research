//! Reference backend: spareval over an `oxrdf::Dataset`. It exists only to validate the
//! harness (manifest reading, data naming, result parsing and comparison), so its update
//! support is a direct transcription of SPARQL 1.1 Update §3 over the in-memory dataset.
use std::collections::{BTreeSet, HashMap};

use oxrdf::{
    BlankNode, Dataset, GraphName, GraphNameRef, NamedNode, NamedNodeRef, NamedOrBlankNode, Quad, Term,
};
use oxiri::Iri;
use oxrdfio::RdfFormat;
use spareval::{DeleteInsertQuad, QueryEvaluator, QueryResults};
use spargebra::algebra::GraphTarget;
use spargebra::term::{GroundQuad, GroundTerm};
use spargebra::GraphUpdateOperation;

use super::{failed, Backend, BackendError, BackendResult, QueryRequest, UpdateRequest};
use crate::compare::select_modifiers;
use crate::results::{QueryOutcome, Solutions};
use crate::suite::parse_rdf_into;

pub struct ReferenceBackend {
    dataset: Dataset,
    /// Named graphs that exist even when empty (CREATE GRAPH, loaded files).
    graphs: BTreeSet<NamedNode>,
    evaluator: QueryEvaluator,
}

impl ReferenceBackend {
    pub fn new() -> Self {
        ReferenceBackend { dataset: Dataset::new(), graphs: BTreeSet::new(), evaluator: QueryEvaluator::new() }
    }

    fn graph_exists(&self, g: NamedNodeRef<'_>) -> bool {
        self.graphs.contains(&g.into_owned()) || self.dataset.quads_for_graph_name(g).next().is_some()
    }

    fn remove_where(&mut self, keep: impl Fn(GraphNameRef<'_>) -> bool) {
        let doomed: Vec<Quad> = self.dataset.iter().filter(|q| !keep(q.graph_name)).map(Quad::from).collect();
        for q in &doomed {
            self.dataset.remove(q);
        }
    }

    fn clear(&mut self, target: &GraphTarget) {
        match target {
            GraphTarget::NamedNode(g) => self.remove_where(|name| name != g.as_ref().into()),
            GraphTarget::DefaultGraph => self.remove_where(|name| !name.is_default_graph()),
            GraphTarget::NamedGraphs => self.remove_where(|name| name.is_default_graph()),
            GraphTarget::AllGraphs => self.dataset.clear(),
        }
    }

    fn insert(&mut self, quad: Quad) {
        if let GraphName::NamedNode(g) = &quad.graph_name {
            self.graphs.insert(g.clone());
        }
        self.dataset.insert(&quad);
    }

    fn apply(&mut self, op: &GraphUpdateOperation, base: Option<&Iri<String>>) -> Result<(), String> {
        match op {
            GraphUpdateOperation::InsertData { data } => {
                // Blank nodes of INSERT DATA are fresh for this operation.
                let mut fresh: HashMap<BlankNode, BlankNode> = HashMap::new();
                let mut rename = |b: &BlankNode| fresh.entry(b.clone()).or_default().clone();
                for q in data {
                    let subject = match &q.subject {
                        NamedOrBlankNode::BlankNode(b) => rename(b).into(),
                        s => s.clone(),
                    };
                    let object = match &q.object {
                        Term::BlankNode(b) => rename(b).into(),
                        o => o.clone(),
                    };
                    self.insert(Quad::new(subject, q.predicate.clone(), object, graph_name(&q.graph_name)));
                }
            }
            GraphUpdateOperation::DeleteData { data } => {
                for q in data {
                    self.dataset.remove(&ground_quad(q));
                }
            }
            GraphUpdateOperation::DeleteInsert { delete, insert, using, pattern } => {
                let changes = self
                    .evaluator
                    .prepare_delete_insert(delete.clone(), insert.clone(), base.cloned(), using.clone(), pattern)
                    .execute(&self.dataset)
                    .map_err(|e| e.to_string())?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| e.to_string())?;
                // All deletions happen before all insertions (§3.1.3).
                let (mut deletes, mut inserts) = (Vec::new(), Vec::new());
                for change in changes {
                    match change {
                        DeleteInsertQuad::Delete(q) => deletes.push(q),
                        DeleteInsertQuad::Insert(q) => inserts.push(q),
                    }
                }
                for q in &deletes {
                    self.dataset.remove(q);
                }
                for q in inserts {
                    self.insert(q);
                }
            }
            GraphUpdateOperation::Load { silent, source, .. } => {
                // The reference backend never dereferences IRIs (http(s) LOAD tests are
                // skipped by the runner); any other LOAD fails, which SILENT absorbs.
                if !silent {
                    return Err(format!("cannot LOAD {source}: dereferencing is not supported"));
                }
            }
            GraphUpdateOperation::Clear { silent, graph } => {
                if let GraphTarget::NamedNode(g) = graph {
                    if !self.graph_exists(g.as_ref()) && !silent {
                        return Err(format!("CLEAR: graph {g} does not exist"));
                    }
                }
                self.clear(graph);
            }
            GraphUpdateOperation::Create { silent, graph } => {
                if self.graph_exists(graph.as_ref()) {
                    if !silent {
                        return Err(format!("CREATE: graph {graph} already exists"));
                    }
                } else {
                    self.graphs.insert(graph.clone());
                }
            }
            GraphUpdateOperation::Drop { silent, graph } => {
                match graph {
                    GraphTarget::NamedNode(g) => {
                        if !self.graph_exists(g.as_ref()) && !silent {
                            return Err(format!("DROP: graph {g} does not exist"));
                        }
                        self.graphs.remove(g);
                    }
                    GraphTarget::NamedGraphs | GraphTarget::AllGraphs => self.graphs.clear(),
                    GraphTarget::DefaultGraph => {}
                }
                self.clear(graph);
            }
        }
        Ok(())
    }
}

fn ground_quad(q: &GroundQuad) -> Quad {
    Quad::new(
        q.subject.clone(),
        q.predicate.clone(),
        match &q.object {
            GroundTerm::NamedNode(n) => Term::from(n.clone()),
            GroundTerm::Literal(l) => Term::from(l.clone()),
        },
        graph_name(&q.graph_name),
    )
}

fn graph_name(g: &spargebra::term::GraphName) -> GraphName {
    match g {
        spargebra::term::GraphName::NamedNode(n) => n.clone().into(),
        spargebra::term::GraphName::DefaultGraph => GraphName::DefaultGraph,
    }
}

impl Backend for ReferenceBackend {
    fn reset(&mut self) -> BackendResult<()> {
        self.dataset.clear();
        self.graphs.clear();
        Ok(())
    }

    fn load(
        &mut self,
        data: &[u8],
        format: RdfFormat,
        base: &str,
        graph: Option<NamedNodeRef<'_>>,
    ) -> BackendResult<()> {
        parse_rdf_into(&mut self.dataset, data, format, base, graph).map_err(BackendError::Failed)?;
        if let Some(g) = graph {
            self.graphs.insert(g.into_owned());
        }
        Ok(())
    }

    fn query(&mut self, request: &QueryRequest<'_>) -> BackendResult<QueryOutcome> {
        let results = self.evaluator.prepare(request.parsed).execute(&self.dataset).map_err(failed)?;
        Ok(match results {
            QueryResults::Boolean(b) => QueryOutcome::Boolean(b),
            QueryResults::Solutions(solutions) => {
                // Report columns in the query's projection order, like the engines under test
                // (spareval's own order need not match it).
                let mut variables = solutions.variables().to_vec();
                if let Some((Some(projection), _)) = select_modifiers(request.parsed) {
                    if projection.len() == variables.len() && projection.iter().all(|v| variables.contains(v)) {
                        variables = projection.to_vec();
                    }
                }
                let mut rows = Vec::new();
                for solution in solutions {
                    let solution = solution.map_err(failed)?;
                    rows.push(variables.iter().map(|v| solution.get(v).cloned()).collect());
                }
                QueryOutcome::Solutions(Solutions { variables, rows, ordered: true })
            }
            QueryResults::Graph(triples) => {
                QueryOutcome::Graph(triples.collect::<Result<Vec<_>, _>>().map_err(failed)?)
            }
        })
    }

    fn update(&mut self, request: &UpdateRequest<'_>) -> BackendResult<()> {
        // Operations apply in sequence; a failing operation aborts the rest of the request.
        let base = request.parsed.base_iri.as_ref();
        for op in &request.parsed.operations {
            self.apply(op, base).map_err(BackendError::Failed)?;
        }
        Ok(())
    }

    fn dump(&mut self) -> BackendResult<Dataset> {
        Ok(self.dataset.clone())
    }
}
