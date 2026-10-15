//! sq-engine: the facade over `Rc<dyn QuadStore>` used by sq-api and the tools (docs/SPEC.md
//! §11 sq-engine, §5.7).
//!
//! * [`prepare`] / [`query_to_string`]: parse, lower, optimize and execute SPARQL queries with
//!   the native planner (sq-plan) and executor (sq-exec), and serialize results (sq-results);
//! * [`explain`]: the four EXPLAIN views (spargebra algebra, logical IR, physical plan, and the
//!   physical plan annotated with runtime counters);
//! * [`update`] / [`update_with`]: the SPARQL 1.1 Update executor of §5.7 (operations in
//!   order, each seeing the previous ones' effects; WHERE solutions materialized before any
//!   change);
//! * [`load`]: the generic RDF loader over any [`QuadStoreWriter`] (MemStore, small inserts).
//!
//! Atomicity is the caller's job: sq-api wraps every request in a SQLite write scope. The
//! facade always runs the native engine; sq-api routes `QueryOptions::engine == Sql` (the
//! baseline SPARQL-to-SQL translator) itself.
#![forbid(unsafe_code)]

mod explain;
mod load;
mod query;
mod update;

use std::rc::Rc;

use sq_core::plan::QueryOptions;
use sq_core::store::{QuadStore, QuadStoreWriter};
use sq_core::Result;
use sq_exec::{ExecCaches, Execution};
use sq_plan::ExplainMode;
use sq_results::ResultFormat;

pub use load::{load_quads, LoadStats};
pub use query::write_results;
pub use update::{UpdateOptions, UpdateReport};

/// Lowers `query` against `store`, optimizes it (the plan is validated in debug builds) and
/// creates its [`Execution`].
pub fn prepare(store: Rc<dyn QuadStore>, query: &spargebra::Query, opts: &QueryOptions, caches: ExecCaches) -> Result<Execution> {
    query::prepare(store, query, opts, caches)
}

/// Parses `query` (base IRI from `opts.base_iri`), runs it and serializes the result:
/// SELECT and ASK to the solution formats (JSON, XML, CSV, TSV), CONSTRUCT and DESCRIBE to the
/// graph formats (N-Triples, N-Quads, Turtle). A format that does not fit the query form is
/// `Error::Invalid`.
pub fn query_to_string(store: Rc<dyn QuadStore>, query: &str, fmt: ResultFormat, opts: &QueryOptions) -> Result<String> {
    query::query_to_string(store, query, fmt, opts)
}

/// EXPLAIN `query`: spargebra's algebra (SSE), the lowered logical IR, the physical plan with
/// estimates, or (`Analyze`) the physical plan after running the query to completion, with
/// per-operator rows, opens, seeks and times.
pub fn explain(store: Rc<dyn QuadStore>, query: &str, mode: ExplainMode, opts: &QueryOptions) -> Result<String> {
    explain::explain(store, query, mode, opts)
}

/// Runs a SPARQL Update request with the default [`UpdateOptions`] (no `LOAD` of local
/// files). The caller provides atomicity (a write scope).
pub fn update(store: Rc<dyn QuadStoreWriter>, update: &spargebra::Update, opts: &QueryOptions) -> Result<UpdateReport> {
    update::run(store, update, opts, &UpdateOptions::default())
}

/// [`update`] with explicit [`UpdateOptions`].
pub fn update_with(
    store: Rc<dyn QuadStoreWriter>,
    update: &spargebra::Update,
    opts: &QueryOptions,
    uopts: &UpdateOptions,
) -> Result<UpdateReport> {
    update::run(store, update, opts, uopts)
}

/// Generic loader (MemStore, small inserts): parses `data` in `format` (relative IRIs against
/// `base`), puts default-graph triples into `graph` (None = the default graph), maps blank-node
/// labels to fresh store blank nodes (one mapping per call) and inserts in batches. Returns the
/// number of quads that were new to the store.
pub fn load(
    store: &dyn QuadStoreWriter,
    data: &[u8],
    format: oxrdfio::RdfFormat,
    base: Option<&str>,
    graph: Option<&str>,
) -> Result<u64> {
    Ok(load::load_quads(store, data, format, base, graph)?.quads_new)
}
