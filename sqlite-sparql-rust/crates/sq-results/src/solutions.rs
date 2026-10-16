//! SELECT / ASK serialization in the SPARQL 1.1 result formats (JSON, XML, CSV, TSV).
use std::io::{BufWriter, Write};
use std::sync::Arc;

use oxrdf::Variable;
use sparesults::QueryResultsSerializer;
use sq_core::eval_api::SolutionSource;
use sq_core::term::Term;
use sq_core::{Error, Result, TermId, BATCH_ROWS};

use crate::convert::{check_ntriples_term, term_ref};
use crate::format::ResultFormat;
use crate::{io_error, OUTPUT_BUFFER_BYTES};

/// Streams every solution of `src` to `w` in `fmt` (JSON, XML, CSV or TSV).
///
/// Rows are pulled [`BATCH_ROWS`] at a time with [`SolutionSource::next_rows`]; each batch is
/// decoded with ONE [`SolutionSource::decode`] call and serialized straight from the decoded
/// terms, so no per-row allocation happens here. UNBOUND (and the internal ERROR group-key
/// sentinel) are written as unbound.
pub fn write_solutions(src: &mut dyn SolutionSource, fmt: ResultFormat, w: &mut dyn Write) -> Result<()> {
    let format = fmt
        .solutions_format()
        .ok_or_else(|| Error::Invalid(format!("'{fmt}' is not a SELECT result format (use json, xml, csv or tsv)")))?;
    let variables = src
        .variables()
        .iter()
        .map(|name| Variable::new(name.as_str()))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::internal(format!("invalid projected variable name: {e}")))?;
    let width = variables.len();

    let mut out = BufWriter::with_capacity(OUTPUT_BUFFER_BYTES, w);
    let mut serializer = QueryResultsSerializer::from_format(format)
        .serialize_solutions_to_writer(&mut out, variables.clone())
        .map_err(io_error)?;

    let xml = matches!(fmt, ResultFormat::Xml);
    let tsv = matches!(fmt, ResultFormat::Tsv);
    let mut ids: Vec<TermId> = Vec::with_capacity(BATCH_ROWS * width);
    let mut terms: Vec<Option<Arc<Term>>> = Vec::with_capacity(BATCH_ROWS * width);
    loop {
        ids.clear();
        let rows = src.next_rows(&mut ids, BATCH_ROWS)?;
        if rows == 0 {
            break;
        }
        if ids.len() != rows * width {
            return Err(Error::internal(format!(
                "solution source returned {} ids for {rows} rows of width {width}",
                ids.len()
            )));
        }
        if width == 0 {
            // Solutions with no projected variable (e.g. `SELECT * {}`): one empty row each.
            for _ in 0..rows {
                serializer.serialize(std::iter::empty::<(&Variable, &oxrdf::Term)>()).map_err(io_error)?;
            }
            continue;
        }
        for id in &mut ids {
            if *id == TermId::ERROR {
                *id = TermId::UNBOUND;
            }
        }
        terms.clear();
        src.decode(&ids, &mut terms)?;
        if terms.len() != ids.len() {
            return Err(Error::internal(format!(
                "solution source decoded {} terms for {} ids",
                terms.len(),
                ids.len()
            )));
        }
        if xml {
            if let Some(c) = terms.iter().flatten().find_map(|t| first_non_xml_char(t)) {
                return Err(Error::Invalid(format!(
                    "a result value contains U+{:04X}, which XML 1.0 cannot represent; use the json, csv or tsv format",
                    c as u32
                )));
            }
        }
        if tsv {
            // TSV writes terms in N-Triples syntax (CSV writes bare values, quoted as CSV).
            for t in terms.iter().flatten() {
                check_ntriples_term(t)?;
            }
        }
        for row in terms.chunks_exact(width) {
            let bindings =
                variables.iter().zip(row).filter_map(|(var, term)| Some((var.as_ref(), term_ref(term.as_deref()?))));
            serializer.serialize(bindings).map_err(io_error)?;
        }
    }
    serializer.finish().map_err(io_error)?;
    out.flush().map_err(io_error)
}

/// First character of `t` outside the XML 1.0 `Char` production (C0 controls other than tab, LF
/// and CR, and U+FFFE/U+FFFF). Such characters cannot appear in an XML 1.0 document at all, not even
/// as character references, so the XML writer refuses the result instead of emitting broken XML.
fn first_non_xml_char(t: &Term) -> Option<char> {
    let bad = |s: &str| {
        s.chars().find(|c| {
            !matches!(c, '\u{9}' | '\u{A}' | '\u{D}' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..)
        })
    };
    match t {
        Term::Iri(s) | Term::BNode(s) => bad(s),
        Term::Literal(l) => bad(&l.lex).or_else(|| bad(&l.datatype)),
    }
}

/// Writes an ASK result. CSV and TSV carry the bare `true` / `false` (no header), as
/// sparesults does.
pub fn write_boolean(b: bool, fmt: ResultFormat, w: &mut dyn Write) -> Result<()> {
    let format = fmt
        .solutions_format()
        .ok_or_else(|| Error::Invalid(format!("'{fmt}' is not an ASK result format (use json, xml, csv or tsv)")))?;
    QueryResultsSerializer::from_format(format).serialize_boolean_to_writer(&mut *w, b).map_err(io_error)?;
    w.flush().map_err(io_error)
}
