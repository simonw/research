//! Engines under test, behind one trait. Each backend owns a store that the runner resets,
//! loads, queries, updates and dumps; the comparison logic never sees engine types.
#[cfg(any(feature = "mem", feature = "oracle"))]
mod memstore;
#[cfg(feature = "mem")]
mod mem;
#[cfg(feature = "oracle")]
mod oracle;
#[cfg(feature = "reference")]
mod reference;
#[cfg(feature = "sqlite")]
mod sqlite;

use std::collections::HashSet;
use std::fmt;

use oxrdf::{Dataset, NamedNodeRef};
use oxrdfio::RdfFormat;
use sparesults::{QueryResultsFormat, QueryResultsSerializer};

use crate::report::parse_known_failures;
use crate::results::{QueryOutcome, TextFormat};

/// spareval / spargebra divergences found while validating the harness.
const REFERENCE_DIVERGENCES: &str = include_str!("../../reference_divergences.txt");

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackendError {
    /// The backend cannot run this kind of test (reported as skipped).
    Unsupported(String),
    /// The operation failed (reported as an error).
    Failed(String),
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendError::Unsupported(m) => write!(f, "unsupported: {m}"),
            BackendError::Failed(m) => f.write_str(m),
        }
    }
}

pub type BackendResult<T> = Result<T, BackendError>;

#[cfg(any(feature = "reference", feature = "mem", feature = "sqlite", feature = "oracle"))]
pub(crate) fn failed(e: impl fmt::Display) -> BackendError {
    BackendError::Failed(e.to_string())
}

/// A query to run: its text, the base IRI to resolve it against, and the harness's own
/// spargebra parse (used for the query form and by backends that take an algebra).
pub struct QueryRequest<'a> {
    pub text: &'a str,
    pub base: &'a str,
    pub parsed: &'a spargebra::Query,
}

impl QueryRequest<'_> {
    /// SELECT and ASK produce SPARQL results; CONSTRUCT and DESCRIBE produce graphs.
    pub fn is_graph_query(&self) -> bool {
        matches!(self.parsed, spargebra::Query::Construct { .. } | spargebra::Query::Describe { .. })
    }
}

pub struct UpdateRequest<'a> {
    pub text: &'a str,
    pub base: &'a str,
    pub parsed: &'a spargebra::Update,
}

/// Cancels the operation currently running on a backend (called from another thread).
pub type Interrupter = Box<dyn Fn() + Send>;

pub trait Backend {
    /// Empties the store.
    fn reset(&mut self) -> BackendResult<()>;
    /// Loads an RDF document; triples go to `graph` (None = default graph).
    fn load(
        &mut self,
        data: &[u8],
        format: RdfFormat,
        base: &str,
        graph: Option<NamedNodeRef<'_>>,
    ) -> BackendResult<()>;
    fn query(&mut self, request: &QueryRequest<'_>) -> BackendResult<QueryOutcome>;
    /// The query's result document in `format` (CSV / TSV result-format tests). By default
    /// the structured outcome is serialized with sparesults; engines with their own writers
    /// override this.
    fn query_text(&mut self, request: &QueryRequest<'_>, format: TextFormat) -> BackendResult<String> {
        let outcome = self.query(request)?;
        serialize_outcome(&outcome, format.sparesults()).map_err(BackendError::Failed)
    }
    fn update(&mut self, request: &UpdateRequest<'_>) -> BackendResult<()>;
    /// The whole store as a dataset.
    fn dump(&mut self) -> BackendResult<Dataset>;
    /// A thread-safe handle that interrupts a running operation, if the backend has one.
    fn interrupter(&self) -> Option<Interrupter> {
        None
    }
}

/// Serializes a SELECT / ASK outcome with sparesults.
pub fn serialize_outcome(outcome: &QueryOutcome, format: QueryResultsFormat) -> Result<String, String> {
    let serializer = QueryResultsSerializer::from_format(format);
    let bytes = match outcome {
        QueryOutcome::Boolean(b) => serializer.serialize_boolean_to_writer(Vec::new(), *b).map_err(|e| e.to_string())?,
        QueryOutcome::Solutions(s) => {
            let mut writer =
                serializer.serialize_solutions_to_writer(Vec::new(), s.variables.clone()).map_err(|e| e.to_string())?;
            for row in &s.rows {
                writer
                    .serialize(s.variables.iter().zip(row).filter_map(|(v, t)| Some((v, t.as_ref()?))))
                    .map_err(|e| e.to_string())?;
            }
            writer.finish().map_err(|e| e.to_string())?
        }
        QueryOutcome::Graph(_) => return Err("a graph result has no SPARQL results serialization".into()),
    };
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

/// The engines the runner can drive.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BackendKind {
    /// spareval over an oxrdf::Dataset: validates the harness itself.
    Reference,
    /// MemStore + sq-engine.
    Mem,
    /// In-memory SQLite + sq-api, black-box through SQL functions.
    Sqlite,
    /// sq-oracle over MemStore (queries only).
    Oracle,
}

impl BackendKind {
    pub const ALL: [BackendKind; 4] =
        [BackendKind::Reference, BackendKind::Mem, BackendKind::Sqlite, BackendKind::Oracle];

    pub fn parse(s: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|k| k.name().eq_ignore_ascii_case(s))
            .ok_or_else(|| format!("unknown backend '{s}' (expected reference, mem, sqlite or oracle)"))
    }

    pub const fn name(self) -> &'static str {
        match self {
            BackendKind::Reference => "reference",
            BackendKind::Mem => "mem",
            BackendKind::Sqlite => "sqlite",
            BackendKind::Oracle => "oracle",
        }
    }

    /// Cargo feature that compiles this backend in.
    pub const fn feature(self) -> &'static str {
        self.name()
    }

    pub const fn is_available(self) -> bool {
        match self {
            BackendKind::Reference => cfg!(feature = "reference"),
            BackendKind::Mem => cfg!(feature = "mem"),
            BackendKind::Sqlite => cfg!(feature = "sqlite"),
            BackendKind::Oracle => cfg!(feature = "oracle"),
        }
    }

    /// Tests this backend is known to fail for reasons outside sparqlite and the harness:
    /// the reference engine's documented divergences (`reference_divergences.txt`).
    pub fn builtin_known_failures(self) -> HashSet<String> {
        match self {
            BackendKind::Reference => parse_known_failures(REFERENCE_DIVERGENCES),
            _ => HashSet::new(),
        }
    }

    /// Creates a backend with an empty store.
    pub fn create(self) -> Result<Box<dyn Backend>, String> {
        let unavailable = || {
            Err(format!(
                "backend '{}' is not compiled in (rebuild sq-w3c with --features {})",
                self.name(),
                self.feature()
            ))
        };
        match self {
            #[cfg(feature = "reference")]
            BackendKind::Reference => Ok(Box::new(reference::ReferenceBackend::new())),
            #[cfg(feature = "mem")]
            BackendKind::Mem => Ok(Box::new(mem::MemBackend::new())),
            #[cfg(feature = "sqlite")]
            BackendKind::Sqlite => Ok(Box::new(sqlite::SqliteBackend::new()?)),
            #[cfg(feature = "oracle")]
            BackendKind::Oracle => Ok(Box::new(oracle::OracleBackend::new())),
            #[allow(unreachable_patterns)]
            _ => unavailable(),
        }
    }
}

impl fmt::Display for BackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::results::Solutions;
    use oxrdf::{Literal, Variable};

    #[test]
    fn kinds_parse_and_report_availability() {
        for k in BackendKind::ALL {
            assert_eq!(BackendKind::parse(&k.name().to_uppercase()).unwrap(), k);
            if !k.is_available() {
                assert!(k.create().err().is_some_and(|e| e.contains("--features")), "{k}");
            }
        }
        assert!(BackendKind::parse("postgres").is_err());
        let divergences = BackendKind::Reference.builtin_known_failures();
        assert!(divergences.len() > 20 && divergences.iter().all(|t| t.starts_with("http://www.w3.org/")));
        assert!(BackendKind::Sqlite.builtin_known_failures().is_empty());
    }

    #[test]
    fn serializes_outcomes_for_text_comparison() {
        let outcome = QueryOutcome::Solutions(Solutions {
            variables: vec![Variable::new_unchecked("x")],
            rows: vec![vec![Some(Literal::from(1).into())], vec![None]],
            ordered: true,
        });
        assert_eq!(serialize_outcome(&outcome, QueryResultsFormat::Csv).unwrap(), "x\r\n1\r\n\r\n");
        assert_eq!(serialize_outcome(&outcome, QueryResultsFormat::Tsv).unwrap(), "?x\n1\n\n");
        assert_eq!(serialize_outcome(&QueryOutcome::Boolean(true), QueryResultsFormat::Csv).unwrap(), "true");
        assert!(serialize_outcome(&QueryOutcome::Graph(vec![]), QueryResultsFormat::Csv).is_err());
    }
}
