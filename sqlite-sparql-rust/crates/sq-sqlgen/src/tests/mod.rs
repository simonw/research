//! Unit tests: golden SQL, SQL building blocks, GRAPH analysis and the helper vocabulary.
//! Result equivalence against a reference evaluator lives in `tests/` (integration tests over
//! a real SQLite store).
use sq_core::ir::LoweredQuery;
use sq_core::plan::QueryOptions;
use sq_core::store::QuadStoreWriter;
use sq_memstore::MemStore;

use crate::{SqlQuery, translate};

mod golden;
mod graph;
mod helpers;
mod limits;
mod numeric;
mod sql;
mod shapes;

pub(crate) const PREFIXES: &str = "PREFIX : <http://ex.org/> PREFIX xsd: <http://www.w3.org/2001/XMLSchema#> ";

/// A MemStore whose dictionary holds the vocabulary of the unit-test queries, so their
/// constants lower to dictionary ids (an absent constant is a TEMP id, which empties a BGP).
pub(crate) fn store() -> MemStore {
    let store = MemStore::new();
    for local in [
        "p", "q", "r", "name", "age", "knows", "likes", "score", "sub", "Person", "a", "b", "c", "g1", "g2", "tea",
    ] {
        store.intern(oxrdf::NamedNode::new_unchecked(format!("http://ex.org/{local}")).as_ref().into()).unwrap();
    }
    for lex in ["Alice", "Bob the Builder"] {
        store.intern(oxrdf::Literal::new_simple_literal(lex).as_ref().into()).unwrap();
    }
    store
}

pub(crate) fn lower_with(query: &str, opts: &QueryOptions) -> LoweredQuery {
    let q = sq_plan::parse_query(&format!("{PREFIXES}{query}"), None).unwrap();
    sq_plan::lower_query(&q, &store(), opts).unwrap()
}

pub(crate) fn lower(query: &str) -> LoweredQuery {
    lower_with(query, &QueryOptions::default())
}

pub(crate) fn sql(query: &str) -> SqlQuery {
    translate(&lower(query)).unwrap_or_else(|e| panic!("{query}: {e}"))
}
