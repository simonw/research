//! IRIs of the W3C test-manifest vocabularies.
use oxrdf::NamedNodeRef;

const fn iri(s: &'static str) -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(s)
}

pub mod rdf {
    use super::*;
    pub const TYPE: NamedNodeRef<'static> = iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#type");
    pub const FIRST: NamedNodeRef<'static> = iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#first");
    pub const REST: NamedNodeRef<'static> = iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#rest");
    pub const NIL: NamedNodeRef<'static> = iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#nil");
}

pub mod rdfs {
    use super::*;
    pub const LABEL: NamedNodeRef<'static> = iri("http://www.w3.org/2000/01/rdf-schema#label");
    pub const COMMENT: NamedNodeRef<'static> = iri("http://www.w3.org/2000/01/rdf-schema#comment");
}

/// Test manifest vocabulary.
pub mod mf {
    use super::*;
    pub const MANIFEST: NamedNodeRef<'static> = iri("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#Manifest");
    pub const INCLUDE: NamedNodeRef<'static> = iri("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#include");
    pub const ENTRIES: NamedNodeRef<'static> = iri("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#entries");
    pub const NAME: NamedNodeRef<'static> = iri("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#name");
    pub const ACTION: NamedNodeRef<'static> = iri("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#action");
    pub const RESULT: NamedNodeRef<'static> = iri("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#result");
    pub const RESULT_CARDINALITY: NamedNodeRef<'static> =
        iri("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#resultCardinality");
    pub const LAX_CARDINALITY: NamedNodeRef<'static> =
        iri("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#LaxCardinality");
    pub const QUERY_EVALUATION_TEST: NamedNodeRef<'static> =
        iri("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#QueryEvaluationTest");
    pub const UPDATE_EVALUATION_TEST: NamedNodeRef<'static> =
        iri("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#UpdateEvaluationTest");
    pub const CSV_RESULT_FORMAT_TEST: NamedNodeRef<'static> =
        iri("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#CSVResultFormatTest");
}

/// Query test vocabulary.
pub mod qt {
    use super::*;
    pub const QUERY: NamedNodeRef<'static> = iri("http://www.w3.org/2001/sw/DataAccess/tests/test-query#query");
    pub const DATA: NamedNodeRef<'static> = iri("http://www.w3.org/2001/sw/DataAccess/tests/test-query#data");
    pub const GRAPH_DATA: NamedNodeRef<'static> = iri("http://www.w3.org/2001/sw/DataAccess/tests/test-query#graphData");
    pub const SERVICE_DATA: NamedNodeRef<'static> =
        iri("http://www.w3.org/2001/sw/DataAccess/tests/test-query#serviceData");
}

/// Update test vocabulary.
pub mod ut {
    use super::*;
    pub const REQUEST: NamedNodeRef<'static> = iri("http://www.w3.org/2009/sparql/tests/test-update#request");
    pub const DATA: NamedNodeRef<'static> = iri("http://www.w3.org/2009/sparql/tests/test-update#data");
    pub const GRAPH_DATA: NamedNodeRef<'static> = iri("http://www.w3.org/2009/sparql/tests/test-update#graphData");
    pub const GRAPH: NamedNodeRef<'static> = iri("http://www.w3.org/2009/sparql/tests/test-update#graph");
}

/// DAWG approval vocabulary.
pub mod dawgt {
    use super::*;
    pub const APPROVAL: NamedNodeRef<'static> = iri("http://www.w3.org/2001/sw/DataAccess/tests/test-dawg#approval");
}

/// Service description vocabulary (entailment regimes).
pub mod sd {
    use super::*;
    pub const ENTAILMENT_REGIME: NamedNodeRef<'static> =
        iri("http://www.w3.org/ns/sparql-service-description#entailmentRegime");
}

/// DAWG result-set vocabulary (`.ttl` / `.rdf` expected results).
pub mod rs {
    use super::*;
    pub const RESULT_SET: NamedNodeRef<'static> =
        iri("http://www.w3.org/2001/sw/DataAccess/tests/result-set#ResultSet");
    pub const RESULT_VARIABLE: NamedNodeRef<'static> =
        iri("http://www.w3.org/2001/sw/DataAccess/tests/result-set#resultVariable");
    pub const SOLUTION: NamedNodeRef<'static> = iri("http://www.w3.org/2001/sw/DataAccess/tests/result-set#solution");
    pub const BINDING: NamedNodeRef<'static> = iri("http://www.w3.org/2001/sw/DataAccess/tests/result-set#binding");
    pub const VARIABLE: NamedNodeRef<'static> = iri("http://www.w3.org/2001/sw/DataAccess/tests/result-set#variable");
    pub const VALUE: NamedNodeRef<'static> = iri("http://www.w3.org/2001/sw/DataAccess/tests/result-set#value");
    pub const INDEX: NamedNodeRef<'static> = iri("http://www.w3.org/2001/sw/DataAccess/tests/result-set#index");
    pub const BOOLEAN: NamedNodeRef<'static> = iri("http://www.w3.org/2001/sw/DataAccess/tests/result-set#boolean");
}
