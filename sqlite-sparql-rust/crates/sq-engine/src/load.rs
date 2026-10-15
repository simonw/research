//! The generic RDF loader over any [`QuadStoreWriter`] (MemStore, `INSERT`-sized loads and
//! the `LOAD` update operation). sq-sqlite has its own bulk loader for large inputs.
use oxrdf::{GraphName, NamedOrBlankNode, Quad, TermRef};
use oxrdfio::{RdfFormat, RdfParser};
use rustc_hash::FxHashMap;
use sq_core::store::QuadStoreWriter;
use sq_core::{Error, Result, TermId};

/// Quads per `QuadStoreWriter::insert` call.
const INSERT_BATCH: usize = 16_384;

/// What a load did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LoadStats {
    /// Quads parsed from the input.
    pub quads_read: u64,
    /// Quads that were not in the store before.
    pub quads_new: u64,
}

/// Parses `data` and inserts every quad (see [`crate::load`]).
pub fn load_quads(
    store: &dyn QuadStoreWriter,
    data: &[u8],
    format: RdfFormat,
    base: Option<&str>,
    graph: Option<&str>,
) -> Result<LoadStats> {
    let parser = parser(format, base, graph)?;
    let mut enc = QuadEncoder::new(store);
    for quad in parser.for_slice(data) {
        let quad = quad.map_err(|e| Error::Parse(e.to_string()))?;
        enc.push(&quad)?;
        if enc.batch.len() >= INSERT_BATCH {
            enc.flush()?;
        }
    }
    enc.flush()?;
    Ok(enc.stats)
}

/// Parses the whole document without touching the store (the `LOAD` operation parses before
/// it writes, so a malformed document leaves the store unchanged).
pub(crate) fn parse_document(
    data: &[u8],
    format: RdfFormat,
    base: Option<&str>,
    graph: Option<&str>,
) -> Result<Vec<Quad>> {
    parser(format, base, graph)?
        .for_slice(data)
        .collect::<Result<Vec<Quad>, _>>()
        .map_err(|e| Error::Parse(e.to_string()))
}

/// Inserts parsed quads (one blank-node mapping for the whole slice).
pub(crate) fn insert_quads(store: &dyn QuadStoreWriter, quads: &[Quad]) -> Result<LoadStats> {
    let mut enc = QuadEncoder::new(store);
    for q in quads {
        enc.push(q)?;
        if enc.batch.len() >= INSERT_BATCH {
            enc.flush()?;
        }
    }
    enc.flush()?;
    Ok(enc.stats)
}

fn parser(format: RdfFormat, base: Option<&str>, graph: Option<&str>) -> Result<RdfParser> {
    let mut parser = RdfParser::from_format(format);
    if let Some(base) = base {
        parser = parser.with_base_iri(base).map_err(|e| Error::Invalid(format!("base IRI <{base}>: {e}")))?;
    }
    if let Some(g) = graph {
        let g = oxrdf::NamedNode::new(g).map_err(|e| Error::Invalid(format!("graph IRI <{g}>: {e}")))?;
        parser = parser.with_default_graph(g);
    }
    Ok(parser)
}

/// Turns parsed quads into `[g, s, p, o]` store ids. Blank-node labels map to fresh store
/// blank nodes, one mapping per encoder (that is, per load call).
struct QuadEncoder<'a> {
    store: &'a dyn QuadStoreWriter,
    bnodes: FxHashMap<String, TermId>,
    batch: Vec<[TermId; 4]>,
    stats: LoadStats,
}

impl<'a> QuadEncoder<'a> {
    fn new(store: &'a dyn QuadStoreWriter) -> Self {
        QuadEncoder { store, bnodes: FxHashMap::default(), batch: Vec::new(), stats: LoadStats::default() }
    }

    fn push(&mut self, q: &Quad) -> Result<()> {
        let g = match &q.graph_name {
            GraphName::DefaultGraph => TermId::DEFAULT_GRAPH,
            GraphName::NamedNode(n) => self.store.intern(n.as_ref().into())?,
            GraphName::BlankNode(b) => self.bnode(b.as_str())?,
        };
        let s = match &q.subject {
            NamedOrBlankNode::NamedNode(n) => self.store.intern(n.as_ref().into())?,
            NamedOrBlankNode::BlankNode(b) => self.bnode(b.as_str())?,
        };
        let p = self.store.intern(q.predicate.as_ref().into())?;
        let o = match q.object.as_ref() {
            TermRef::BlankNode(b) => self.bnode(b.as_str())?,
            other => self.store.intern(other)?,
        };
        self.batch.push([g, s, p, o]);
        self.stats.quads_read += 1;
        Ok(())
    }

    fn bnode(&mut self, label: &str) -> Result<TermId> {
        if let Some(id) = self.bnodes.get(label) {
            return Ok(*id);
        }
        let id = self.store.fresh_bnode()?;
        self.bnodes.insert(label.to_owned(), id);
        Ok(id)
    }

    fn flush(&mut self) -> Result<()> {
        if !self.batch.is_empty() {
            self.stats.quads_new += self.store.insert(&mut self.batch)?;
            self.batch.clear();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use sq_core::store::{IndexOrder, QuadStore, ScanSpec};
    use sq_memstore::MemStore;

    use super::*;

    fn count(store: &MemStore) -> usize {
        store.quads().len()
    }

    #[test]
    fn loads_turtle_into_the_default_graph_and_counts_new_quads() {
        let store = MemStore::new();
        let ttl = b"@prefix : <http://ex/> . :a :p 1, 2 ; :q \"x\" .";
        let st = load_quads(&store, ttl, RdfFormat::Turtle, None, None).unwrap();
        assert_eq!(st, LoadStats { quads_read: 3, quads_new: 3 });
        assert!(store.quads().iter().all(|q| q[0] == TermId::DEFAULT_GRAPH));
        // Loading the same document again adds nothing.
        let st = load_quads(&store, ttl, RdfFormat::Turtle, None, None).unwrap();
        assert_eq!(st, LoadStats { quads_read: 3, quads_new: 0 });
        assert_eq!(count(&store), 3);
    }

    #[test]
    fn target_graph_and_base_iri_apply() {
        let store = MemStore::new();
        let st = load_quads(&store, b"<a> <p> <b> .", RdfFormat::Turtle, Some("http://base/"), Some("http://g/")).unwrap();
        assert_eq!(st.quads_new, 1);
        let g = store.dict().lookup(oxrdf::NamedNodeRef::new_unchecked("http://g/").into()).unwrap().unwrap();
        let s = store.dict().lookup(oxrdf::NamedNodeRef::new_unchecked("http://base/a").into()).unwrap().unwrap();
        assert_eq!(store.named_graphs().unwrap(), vec![g]);
        assert_eq!(store.count(&ScanSpec::new(IndexOrder::Gspo, &[g, s]), None).unwrap(), 1);
    }

    #[test]
    fn quad_formats_keep_their_graphs_and_default_triples_go_to_the_target() {
        let store = MemStore::new();
        let trig = b"@prefix : <http://ex/> . :a :p :b . :g1 { :a :p :c } ";
        load_quads(&store, trig, RdfFormat::TriG, None, Some("http://ex/t")).unwrap();
        let mut graphs: Vec<String> = store
            .named_graphs()
            .unwrap()
            .into_iter()
            .map(|g| store.dict().decode_one(g).unwrap().to_ntriples())
            .collect();
        graphs.sort();
        assert_eq!(graphs, ["<http://ex/g1>", "<http://ex/t>"]);
        assert!(store.quads().iter().all(|q| q[0] != TermId::DEFAULT_GRAPH));
    }

    #[test]
    fn blank_nodes_are_fresh_per_call_and_shared_within_one() {
        let store = MemStore::new();
        let ttl = b"_:x <http://ex/p> _:x . _:y <http://ex/p> _:x .";
        load_quads(&store, ttl, RdfFormat::Turtle, None, None).unwrap();
        load_quads(&store, ttl, RdfFormat::Turtle, None, None).unwrap();
        let quads = store.quads();
        assert_eq!(quads.len(), 4, "the second load must not reuse the first load's blank nodes");
        let mut bnodes: Vec<TermId> = quads.iter().flat_map(|q| [q[1], q[3]]).collect();
        bnodes.sort();
        bnodes.dedup();
        assert_eq!(bnodes.len(), 4);
    }

    #[test]
    fn batches_larger_than_one_insert() {
        let store = MemStore::new();
        let mut nt = String::new();
        for i in 0..(INSERT_BATCH + 10) {
            nt.push_str(&format!("<http://ex/s{i}> <http://ex/p> \"{i}\" .\n"));
        }
        let st = load_quads(&store, nt.as_bytes(), RdfFormat::NTriples, None, None).unwrap();
        assert_eq!(st.quads_new as usize, INSERT_BATCH + 10);
        assert_eq!(count(&store), INSERT_BATCH + 10);
    }

    #[test]
    fn errors_are_reported() {
        let store = MemStore::new();
        let e = load_quads(&store, b"<a> <b> .", RdfFormat::NTriples, None, None).unwrap_err();
        assert!(matches!(e, Error::Parse(_)), "{e}");
        let e = load_quads(&store, b"", RdfFormat::Turtle, Some("not an iri"), None).unwrap_err();
        assert!(matches!(e, Error::Invalid(_)), "{e}");
        let e = load_quads(&store, b"", RdfFormat::Turtle, None, Some("bad graph")).unwrap_err();
        assert!(matches!(e, Error::Invalid(_)), "{e}");
    }

    #[test]
    fn parse_document_then_insert() {
        let store = MemStore::new();
        let e = parse_document(b"<http://ex/a> <http://ex/p> _:b . <a>", RdfFormat::NTriples, None, None);
        assert!(matches!(e, Err(Error::Parse(_))));
        let quads = parse_document(b"<a> <p> _:b . <a> <q> _:b .", RdfFormat::Turtle, Some("http://ex/"), None).unwrap();
        assert_eq!(quads.len(), 2);
        assert_eq!(count(&store), 0, "parsing alone writes nothing");
        let st = insert_quads(&store, &quads).unwrap();
        assert_eq!(st, LoadStats { quads_read: 2, quads_new: 2 });
        let quads = store.quads();
        assert_eq!(quads[0][3], quads[1][3], "one blank-node mapping per call");
    }
}
