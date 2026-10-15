//! `sqlite` backend: a fresh in-memory SQLite connection with `sq_api::register`, driven
//! black-box through the SQL functions of §8.2 — exactly what an application sees, minus the
//! cdylib. The base IRI is passed by prefixing the request with a `BASE` declaration.
use std::str::FromStr;
use std::sync::{Arc, Mutex, PoisonError};

use oxrdf::{Dataset, GraphName, NamedNodeRef, NamedOrBlankNode, Quad, Term};
use oxrdfio::RdfFormat;
use rusqlite::{params, Connection, InterruptHandle, OptionalExtension};
use sparesults::QueryResultsFormat;

use super::{failed, Backend, BackendError, BackendResult, Interrupter, QueryRequest, UpdateRequest};
use crate::results::{parse_graph, parse_results, QueryOutcome, TextFormat};

pub struct SqliteBackend {
    conn: Connection,
    /// Interrupt handle of the CURRENT connection (reset swaps connections).
    interrupt: Arc<Mutex<InterruptHandle>>,
}

fn open() -> Result<Connection, String> {
    let conn = Connection::open_in_memory().map_err(|e| format!("cannot open SQLite: {e}"))?;
    sq_api::register(&conn).map_err(|e| format!("sq_api::register failed: {e}"))?;
    Ok(conn)
}

/// The request with its base IRI made explicit (a leading `BASE` is valid in every SPARQL
/// query and update; a later BASE in the text still applies, resolved against it).
fn with_base(text: &str, base: &str) -> String {
    format!("BASE <{base}>\n{text}")
}

fn format_name(format: RdfFormat) -> &'static str {
    match format {
        RdfFormat::Turtle => "turtle",
        RdfFormat::NTriples => "ntriples",
        RdfFormat::NQuads => "nquads",
        RdfFormat::TriG => "trig",
        RdfFormat::RdfXml => "rdfxml",
        RdfFormat::N3 => "n3",
        _ => "jsonld",
    }
}

fn parse_term(text: &str) -> BackendResult<Term> {
    Term::from_str(text).map_err(|e| failed(format!("sparql_term returned invalid N-Triples term {text:?}: {e}")))
}

impl SqliteBackend {
    pub fn new() -> Result<Self, String> {
        let conn = open()?;
        let interrupt = Arc::new(Mutex::new(conn.get_interrupt_handle()));
        Ok(SqliteBackend { conn, interrupt })
    }

    fn sparql_query(&self, text: &str, base: &str, format: &str) -> BackendResult<String> {
        self.conn
            .query_row("SELECT sparql_query(?1, ?2)", params![with_base(text, base), format], |r| {
                r.get::<_, Option<String>>(0)
            })
            .map_err(failed)?
            .ok_or_else(|| failed("sparql_query returned NULL"))
    }
}

impl Backend for SqliteBackend {
    fn reset(&mut self) -> BackendResult<()> {
        self.conn = open().map_err(BackendError::Failed)?;
        *self.interrupt.lock().unwrap_or_else(PoisonError::into_inner) = self.conn.get_interrupt_handle();
        Ok(())
    }

    fn load(
        &mut self,
        data: &[u8],
        format: RdfFormat,
        base: &str,
        graph: Option<NamedNodeRef<'_>>,
    ) -> BackendResult<()> {
        let sql = "SELECT sparql_load(?1, ?2, ?3, ?4)";
        let (format, graph) = (format_name(format), graph.map(|g| g.as_str()));
        let loaded = match std::str::from_utf8(data) {
            Ok(text) => self.conn.query_row(sql, params![text, format, base, graph], |r| r.get::<_, Option<i64>>(0)),
            Err(_) => self.conn.query_row(sql, params![data, format, base, graph], |r| r.get::<_, Option<i64>>(0)),
        };
        loaded.map_err(failed)?;
        Ok(())
    }

    fn query(&mut self, request: &QueryRequest<'_>) -> BackendResult<QueryOutcome> {
        if request.is_graph_query() {
            let text = self.sparql_query(request.text, request.base, "ntriples")?;
            parse_graph(text.as_bytes(), RdfFormat::NTriples).map_err(BackendError::Failed)
        } else {
            let text = self.sparql_query(request.text, request.base, "json")?;
            parse_results(text.as_bytes(), QueryResultsFormat::Json).map_err(BackendError::Failed)
        }
    }

    fn query_text(&mut self, request: &QueryRequest<'_>, format: TextFormat) -> BackendResult<String> {
        self.sparql_query(request.text, request.base, format.name())
    }

    fn update(&mut self, request: &UpdateRequest<'_>) -> BackendResult<()> {
        self.conn
            .query_row("SELECT sparql_update(?1)", [with_base(request.text, request.base)], |r| {
                r.get::<_, rusqlite::types::Value>(0)
            })
            .map_err(failed)?;
        Ok(())
    }

    fn dump(&mut self) -> BackendResult<Dataset> {
        let has_store = self
            .conn
            .query_row("SELECT 1 FROM main.sqlite_master WHERE type = 'table' AND name = 'rdf_quad'", [], |_| Ok(()))
            .optional()
            .map_err(failed)?
            .is_some();
        let mut dataset = Dataset::new();
        if !has_store {
            return Ok(dataset);
        }
        let mut stmt = self
            .conn
            .prepare("SELECT sparql_term(g), sparql_term(s), sparql_term(p), sparql_term(o) FROM main.rdf_quad")
            .map_err(failed)?;
        let mut rows = stmt.query([]).map_err(failed)?;
        while let Some(row) = rows.next().map_err(failed)? {
            let text = |i: usize| -> BackendResult<Option<String>> { row.get::<_, Option<String>>(i).map_err(failed) };
            let required = |i: usize| -> BackendResult<Term> {
                parse_term(&text(i)?.ok_or_else(|| failed(format!("sparql_term returned NULL for column {i}")))?)
            };
            let graph = match text(0)? {
                None => GraphName::DefaultGraph,
                Some(g) => match parse_term(&g)? {
                    Term::NamedNode(n) => n.into(),
                    Term::BlankNode(b) => b.into(),
                    Term::Literal(l) => return Err(failed(format!("literal graph name {l}"))),
                },
            };
            let subject = match required(1)? {
                Term::NamedNode(n) => NamedOrBlankNode::from(n),
                Term::BlankNode(b) => b.into(),
                Term::Literal(l) => return Err(failed(format!("literal subject {l}"))),
            };
            let Term::NamedNode(predicate) = required(2)? else {
                return Err(failed("non-IRI predicate in rdf_quad"));
            };
            dataset.insert(&Quad::new(subject, predicate, required(3)?, graph));
        }
        Ok(dataset)
    }

    fn interrupter(&self) -> Option<Interrupter> {
        let handle = Arc::clone(&self.interrupt);
        Some(Box::new(move || handle.lock().unwrap_or_else(PoisonError::into_inner).interrupt()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_is_prepended() {
        assert_eq!(with_base("SELECT * {}", "http://ex/q.rq"), "BASE <http://ex/q.rq>\nSELECT * {}");
        let q = spargebra::SparqlParser::new()
            .parse_query(&with_base("BASE <sub/> SELECT * { <a> ?p ?o }", "http://ex/dir/q.rq"))
            .unwrap();
        assert!(q.to_string().contains("<http://ex/dir/sub/a>"), "{q}");
    }

    #[test]
    fn dump_of_a_store_without_schema_is_empty() {
        // A bare connection (no sq-api): the dump must not depend on sparql_term existing.
        let conn = Connection::open_in_memory().unwrap();
        let interrupt = Arc::new(Mutex::new(conn.get_interrupt_handle()));
        let mut backend = SqliteBackend { conn, interrupt };
        assert_eq!(backend.dump().unwrap().len(), 0);
        // Missing SQL functions surface as clean errors.
        let err = backend.update(&UpdateRequest {
            text: "CLEAR DEFAULT",
            base: "http://ex/",
            parsed: &spargebra::SparqlParser::new().parse_update("CLEAR DEFAULT").unwrap(),
        });
        assert!(matches!(err, Err(BackendError::Failed(m)) if m.contains("sparql_update")));
    }

    #[test]
    fn parses_ntriples_terms() {
        assert_eq!(parse_term("<http://ex/a>").unwrap().to_string(), "<http://ex/a>");
        assert!(matches!(parse_term("_:b1").unwrap(), Term::BlankNode(_)));
        assert!(matches!(parse_term("\"x\"@en").unwrap(), Term::Literal(_)));
        assert!(parse_term("not a term").is_err());
    }
}
