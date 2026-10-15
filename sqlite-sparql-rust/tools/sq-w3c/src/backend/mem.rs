//! `mem` backend: MemStore driven through the sq-engine facade (load, query_to_string,
//! update). Query output goes through sq-results (SPARQL JSON / N-Triples) and is parsed back,
//! so the result writers are exercised too.
use std::rc::Rc;

use oxrdf::{Dataset, NamedNodeRef};
use oxrdfio::RdfFormat;
use sparesults::QueryResultsFormat;
use sq_core::plan::QueryOptions;
use sq_core::store::{QuadStore, QuadStoreWriter};
use sq_memstore::MemStore;
use sq_results::ResultFormat;

use super::{failed, memstore, Backend, BackendError, BackendResult, QueryRequest, UpdateRequest};
use crate::results::{parse_graph, parse_results, QueryOutcome, TextFormat};

pub struct MemBackend {
    store: MemStore,
}

impl MemBackend {
    pub fn new() -> Self {
        MemBackend { store: MemStore::new() }
    }

    fn options(base: &str) -> QueryOptions {
        QueryOptions { base_iri: Some(base.to_owned()), ..QueryOptions::default() }
    }

    fn run(&self, request: &QueryRequest<'_>, format: ResultFormat) -> BackendResult<String> {
        let store: Rc<dyn QuadStore> = Rc::new(self.store.clone());
        sq_engine::query_to_string(store, request.text, format, &Self::options(request.base)).map_err(failed)
    }
}

impl Backend for MemBackend {
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
        sq_engine::load(&self.store, data, format, Some(base), graph.map(|g| g.as_str())).map_err(failed)?;
        Ok(())
    }

    fn query(&mut self, request: &QueryRequest<'_>) -> BackendResult<QueryOutcome> {
        if request.is_graph_query() {
            let text = self.run(request, ResultFormat::NTriples)?;
            parse_graph(text.as_bytes(), RdfFormat::NTriples).map_err(BackendError::Failed)
        } else {
            let text = self.run(request, ResultFormat::Json)?;
            parse_results(text.as_bytes(), QueryResultsFormat::Json).map_err(BackendError::Failed)
        }
    }

    fn query_text(&mut self, request: &QueryRequest<'_>, format: TextFormat) -> BackendResult<String> {
        let format = match format {
            TextFormat::Csv => ResultFormat::Csv,
            TextFormat::Tsv => ResultFormat::Tsv,
        };
        self.run(request, format)
    }

    fn update(&mut self, request: &UpdateRequest<'_>) -> BackendResult<()> {
        let store: Rc<dyn QuadStoreWriter> = Rc::new(self.store.clone());
        sq_engine::update(store, request.parsed, &Self::options(request.base)).map_err(failed)?;
        Ok(())
    }

    fn dump(&mut self) -> BackendResult<Dataset> {
        memstore::dump(&self.store).map_err(BackendError::Failed)
    }
}
