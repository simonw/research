//! `oracle` backend: the spec-transcribed sq-oracle evaluator over a query lowered by
//! sq-plan, on MemStore. Query evaluation only: update tests are reported as skipped.
use std::collections::HashMap;

use oxrdf::{Dataset, NamedNode, NamedNodeRef, NamedOrBlankNode, Term, Triple, Variable};
use oxrdfio::{RdfFormat, RdfParser};
use sq_core::plan::QueryOptions;
use sq_core::store::QuadStoreWriter;
use sq_core::TermId;
use sq_memstore::MemStore;
use sq_oracle::OracleOutput;

use super::{failed, memstore, Backend, BackendError, BackendResult, QueryRequest, UpdateRequest};
use crate::results::{QueryOutcome, Solutions};

pub struct OracleBackend {
    store: MemStore,
}

impl OracleBackend {
    pub fn new() -> Self {
        OracleBackend { store: MemStore::new() }
    }
}

impl Backend for OracleBackend {
    fn reset(&mut self) -> BackendResult<()> {
        self.store = MemStore::new();
        Ok(())
    }

    fn load(
        &mut self,
        data: &[u8],
        format: RdfFormat,
        base: &str,
        graph: Option<NamedNodeRef<'_>>,
    ) -> BackendResult<()> {
        load_document(&self.store, data, format, base, graph).map_err(BackendError::Failed)?;
        Ok(())
    }

    fn query(&mut self, request: &QueryRequest<'_>) -> BackendResult<QueryOutcome> {
        let options = QueryOptions { base_iri: Some(request.base.to_owned()), ..QueryOptions::default() };
        let lowered = sq_plan::lower_query(request.parsed, &self.store, &options).map_err(failed)?;
        Ok(match sq_oracle::evaluate(&lowered, &self.store).map_err(failed)? {
            OracleOutput::Boolean(b) => QueryOutcome::Boolean(b),
            OracleOutput::Solutions { vars, rows } => {
                let variables = vars
                    .iter()
                    .map(|v| Variable::new(v.as_str()).map_err(failed))
                    .collect::<BackendResult<Vec<_>>>()?;
                let rows = rows
                    .iter()
                    .map(|row| row.iter().map(|t| t.as_ref().map(|t| t.to_oxrdf())).collect())
                    .collect();
                QueryOutcome::Solutions(Solutions { variables, rows, ordered: true })
            }
            OracleOutput::Graph(triples) => QueryOutcome::Graph(
                triples
                    .iter()
                    .map(|[s, p, o]| {
                        let subject = match s.to_oxrdf() {
                            Term::NamedNode(n) => NamedOrBlankNode::from(n),
                            Term::BlankNode(b) => b.into(),
                            Term::Literal(l) => return Err(failed(format!("literal subject {l} in a graph result"))),
                        };
                        let Term::NamedNode(predicate) = p.to_oxrdf() else {
                            return Err(failed("non-IRI predicate in a graph result"));
                        };
                        Ok(Triple::new(subject, predicate, o.to_oxrdf()))
                    })
                    .collect::<BackendResult<Vec<_>>>()?,
            ),
        })
    }

    fn update(&mut self, _request: &UpdateRequest<'_>) -> BackendResult<()> {
        Err(BackendError::Unsupported("the oracle evaluates queries only".into()))
    }

    fn dump(&mut self) -> BackendResult<Dataset> {
        memstore::dump(&self.store).map_err(BackendError::Failed)
    }
}

/// A minimal loader straight onto the `QuadStoreWriter` API (the oracle backend must not
/// depend on sq-engine's loader). Blank nodes get fresh store ids per document.
fn load_document(
    store: &MemStore,
    data: &[u8],
    format: RdfFormat,
    base: &str,
    graph: Option<NamedNodeRef<'_>>,
) -> Result<u64, String> {
    let mut parser = RdfParser::from_format(format).with_base_iri(base).map_err(|e| e.to_string())?;
    if let Some(g) = graph {
        parser = parser.with_default_graph(NamedNode::from(g));
    }
    let mut blank_nodes: HashMap<String, TermId> = HashMap::new();
    let mut id_of = |t: oxrdf::TermRef<'_>| -> Result<TermId, String> {
        match t {
            oxrdf::TermRef::BlankNode(b) => match blank_nodes.get(b.as_str()) {
                Some(id) => Ok(*id),
                None => {
                    let id = store.fresh_bnode().map_err(|e| e.to_string())?;
                    blank_nodes.insert(b.as_str().to_owned(), id);
                    Ok(id)
                }
            },
            other => store.intern(other).map_err(|e| e.to_string()),
        }
    };
    let mut quads = Vec::new();
    for quad in parser.for_slice(data) {
        let quad = quad.map_err(|e| format!("cannot parse {base}: {e}"))?;
        let g = match quad.graph_name.as_ref() {
            oxrdf::GraphNameRef::DefaultGraph => TermId::DEFAULT_GRAPH,
            oxrdf::GraphNameRef::NamedNode(n) => id_of(n.into())?,
            oxrdf::GraphNameRef::BlankNode(b) => id_of(b.into())?,
        };
        let s = id_of(quad.subject.as_ref().into())?;
        let p = id_of(quad.predicate.as_ref().into())?;
        let o = id_of(quad.object.as_ref())?;
        quads.push([g, s, p, o]);
    }
    store.insert(&mut quads).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_and_dump_round_trip() {
        let store = MemStore::new();
        let data = b"@prefix : <http://ex/> . _:a :p 1, \"x\"@en ; :q _:a . :s :p 2.5 .";
        let g = NamedNodeRef::new_unchecked("http://ex/g");
        assert_eq!(load_document(&store, data, RdfFormat::Turtle, "http://ex/", None).unwrap(), 4);
        assert_eq!(load_document(&store, data, RdfFormat::Turtle, "http://ex/", Some(g)).unwrap(), 4);
        let dumped = memstore::dump(&store).unwrap();
        assert_eq!(dumped.len(), 8);
        let mut expected = Dataset::new();
        crate::suite::parse_rdf_into(&mut expected, data, RdfFormat::Turtle, "http://ex/", None).unwrap();
        crate::suite::parse_rdf_into(&mut expected, data, RdfFormat::Turtle, "http://ex/", Some(g)).unwrap();
        crate::compare::compare_datasets(&expected, &dumped, crate::compare::Numeric::Exact).unwrap();
    }
}
