//! CONSTRUCT / DESCRIBE serialization (N-Triples, N-Quads in the default graph, Turtle).
use std::io::{BufWriter, Write};
use std::sync::Arc;

use oxrdf::TripleRef;
use oxrdfio::RdfSerializer;
use sq_core::eval_api::TripleSource;
use sq_core::term::Term;
use sq_core::{Error, Result, TermId, BATCH_ROWS};

use crate::convert::{check_ntriples_term, predicate_ref, subject_ref, term_ref};
use crate::format::ResultFormat;
use crate::{io_error, OUTPUT_BUFFER_BYTES};

/// Prefixes declared in Turtle output (they only shorten the text; the graph is unchanged).
const TURTLE_PREFIXES: [(&str, &str); 3] = [
    ("rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#"),
    ("rdfs", "http://www.w3.org/2000/01/rdf-schema#"),
    ("xsd", "http://www.w3.org/2001/XMLSchema#"),
];

/// Streams every triple of `src` to `w` in `fmt` (N-Triples, N-Quads or Turtle). With N-Quads
/// the triples are written in the default graph.
///
/// Triples are pulled [`BATCH_ROWS`] at a time and each batch is decoded with one
/// [`TripleSource::decode`] call. A triple with an unbound position, a literal subject or a
/// non-IRI predicate violates the `TripleSource` contract and is reported as
/// `Error::Internal`.
pub fn write_triples(src: &mut dyn TripleSource, fmt: ResultFormat, w: &mut dyn Write) -> Result<()> {
    let format = fmt.graph_format().ok_or_else(|| {
        Error::Invalid(format!("'{fmt}' is not a CONSTRUCT/DESCRIBE result format (use ntriples, nquads or turtle)"))
    })?;
    let mut rdf_serializer = RdfSerializer::from_format(format);
    if fmt == ResultFormat::Turtle {
        for (prefix, iri) in TURTLE_PREFIXES {
            rdf_serializer = rdf_serializer
                .with_prefix(prefix, iri)
                .map_err(|e| Error::internal(format!("invalid Turtle prefix {prefix}: {e}")))?;
        }
    }

    let mut out = BufWriter::with_capacity(OUTPUT_BUFFER_BYTES, w);
    let mut serializer = rdf_serializer.for_writer(&mut out);
    let mut triples: Vec<[TermId; 3]> = Vec::with_capacity(BATCH_ROWS);
    let mut ids: Vec<TermId> = Vec::with_capacity(BATCH_ROWS * 3);
    let mut terms: Vec<Option<Arc<Term>>> = Vec::with_capacity(BATCH_ROWS * 3);
    loop {
        triples.clear();
        let n = src.next_triples(&mut triples, BATCH_ROWS)?;
        if n == 0 {
            break;
        }
        if triples.len() != n {
            return Err(Error::internal(format!(
                "triple source reported {n} triples but appended {}",
                triples.len()
            )));
        }
        ids.clear();
        ids.extend(triples.iter().flatten());
        terms.clear();
        src.decode(&ids, &mut terms)?;
        if terms.len() != ids.len() {
            return Err(Error::internal(format!(
                "triple source decoded {} terms for {} ids",
                terms.len(),
                ids.len()
            )));
        }
        for spo in terms.chunks_exact(3) {
            let [Some(s), Some(p), Some(o)] = spo else {
                return Err(Error::internal("graph result contains a triple with an unbound position"));
            };
            for t in [s, p, o] {
                check_ntriples_term(t)?;
            }
            let triple = TripleRef::new(subject_ref(s)?, predicate_ref(p)?, term_ref(o));
            serializer.serialize_triple(triple).map_err(io_error)?;
        }
    }
    serializer.finish().map_err(io_error)?;
    out.flush().map_err(io_error)
}
