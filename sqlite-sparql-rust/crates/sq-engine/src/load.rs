//! The generic RDF loader over any [`QuadStoreWriter`] (MemStore, `INSERT`-sized loads and
//! the `LOAD` update operation). sq-sqlite has its own bulk loader for large inputs.
//!
//! Input is streamed: quads are encoded and inserted in batches of [`INSERT_BATCH`], so memory
//! does not grow with the document. IRIs are interned once per load through a bounded cache
//! (predicates, graph names, classes and subjects repeat; on SQLite every `intern` is a SQL
//! lookup).
use std::fs::File;
use std::path::Path;

use oxrdf::{GraphName, NamedOrBlankNode, Quad, TermRef};
use oxrdfio::{RdfFormat, RdfParseError, RdfParser};
use rustc_hash::FxHashMap;
use sq_core::store::QuadStoreWriter;
use sq_core::{Error, Result, TermId};

/// Quads per `QuadStoreWriter::insert` call.
const INSERT_BATCH: usize = 16_384;
/// IRIs remembered per load; the cache starts over when it is full.
const IRI_CACHE_MAX: usize = 65_536;

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
    let quads = parser(format, base, graph)?.for_slice(data).map(|q| q.map_err(|e| Error::Parse(e.to_string())));
    insert_all(store, quads)
}

/// Streams the local file `path` into the store (the `LOAD` operation). A document that fails
/// part-way leaves the batches inserted before the failure: the caller's write scope undoes
/// them.
pub(crate) fn load_file(
    store: &dyn QuadStoreWriter,
    path: &Path,
    format: RdfFormat,
    base: Option<&str>,
    graph: Option<&str>,
) -> Result<LoadStats> {
    insert_all(store, read_file(path, format, base, graph)?)
}

/// Parses the local file `path` without touching the store; returns its number of quads
/// (`LOAD SILENT` checks the whole document before it writes anything).
pub(crate) fn check_file(path: &Path, format: RdfFormat, base: Option<&str>, graph: Option<&str>) -> Result<u64> {
    let mut n = 0;
    for q in read_file(path, format, base, graph)? {
        q?;
        n += 1;
    }
    Ok(n)
}

fn read_file(
    path: &Path,
    format: RdfFormat,
    base: Option<&str>,
    graph: Option<&str>,
) -> Result<impl Iterator<Item = Result<Quad>>> {
    let parser = parser(format, base, graph)?;
    let file = File::open(path).map_err(|e| Error::Invalid(format!("{}: {e}", path.display())))?;
    Ok(parser.for_reader(file).map(|q| {
        q.map_err(|e| match e {
            RdfParseError::Io(e) => Error::Invalid(format!("read error: {e}")),
            RdfParseError::Syntax(e) => Error::Parse(e.to_string()),
        })
    }))
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

/// Encodes and inserts parsed quads in batches (one blank-node mapping for the whole input).
fn insert_all(store: &dyn QuadStoreWriter, quads: impl Iterator<Item = Result<Quad>>) -> Result<LoadStats> {
    let mut enc = QuadEncoder::new(store);
    for q in quads {
        enc.push(&q?)?;
        if enc.batch.len() >= INSERT_BATCH {
            enc.flush()?;
        }
    }
    enc.flush()?;
    Ok(enc.stats)
}

/// Turns parsed quads into `[g, s, p, o]` store ids. Blank-node labels map to fresh store
/// blank nodes, one mapping per encoder (that is, per load call).
struct QuadEncoder<'a> {
    store: &'a dyn QuadStoreWriter,
    bnodes: FxHashMap<String, TermId>,
    iris: IriCache,
    batch: Vec<[TermId; 4]>,
    stats: LoadStats,
}

impl<'a> QuadEncoder<'a> {
    fn new(store: &'a dyn QuadStoreWriter) -> Self {
        QuadEncoder {
            store,
            bnodes: FxHashMap::default(),
            iris: IriCache::new(IRI_CACHE_MAX),
            batch: Vec::new(),
            stats: LoadStats::default(),
        }
    }

    fn push(&mut self, q: &Quad) -> Result<()> {
        let g = match &q.graph_name {
            GraphName::DefaultGraph => TermId::DEFAULT_GRAPH,
            GraphName::NamedNode(n) => self.iris.intern(self.store, n.as_ref())?,
            GraphName::BlankNode(b) => self.bnode(b.as_str())?,
        };
        let s = match &q.subject {
            NamedOrBlankNode::NamedNode(n) => self.iris.intern(self.store, n.as_ref())?,
            NamedOrBlankNode::BlankNode(b) => self.bnode(b.as_str())?,
        };
        let p = self.iris.intern(self.store, q.predicate.as_ref())?;
        let o = match q.object.as_ref() {
            TermRef::NamedNode(n) => self.iris.intern(self.store, n)?,
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

/// IRI -> interned id, for the duration of one load (ids never change within a write). At
/// most `max` entries: a full cache starts over, which keeps memory bounded on documents
/// with millions of distinct subjects while the frequent IRIs come back at once.
struct IriCache {
    ids: FxHashMap<Box<str>, TermId>,
    max: usize,
}

impl IriCache {
    fn new(max: usize) -> Self {
        IriCache { ids: FxHashMap::default(), max }
    }

    fn intern(&mut self, store: &dyn QuadStoreWriter, iri: oxrdf::NamedNodeRef<'_>) -> Result<TermId> {
        if let Some(id) = self.ids.get(iri.as_str()) {
            return Ok(*id);
        }
        let id = store.intern(iri.into())?;
        if self.ids.len() >= self.max {
            self.ids.clear();
        }
        self.ids.insert(iri.as_str().into(), id);
        Ok(id)
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
    fn files_are_checked_without_writing_and_streamed_in() {
        let dir = std::env::temp_dir().join(format!("sq-engine-load-unit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let good = dir.join("good.ttl");
        std::fs::write(&good, "<a> <p> _:b . <a> <q> _:b .").unwrap();
        let bad = dir.join("bad.nt");
        std::fs::write(&bad, "<http://ex/a> <http://ex/p> _:b . <a>").unwrap();
        let store = MemStore::new();
        assert_eq!(check_file(&good, RdfFormat::Turtle, Some("http://ex/"), None).unwrap(), 2);
        let e = check_file(&bad, RdfFormat::NTriples, None, None).unwrap_err();
        assert!(matches!(e, Error::Parse(_)), "{e}");
        let e = check_file(&dir.join("missing.ttl"), RdfFormat::Turtle, None, None).unwrap_err();
        assert!(matches!(e, Error::Invalid(_)), "{e}");
        assert_eq!(count(&store), 0, "checking writes nothing");
        let st = load_file(&store, &good, RdfFormat::Turtle, Some("http://ex/"), None).unwrap();
        assert_eq!(st, LoadStats { quads_read: 2, quads_new: 2 });
        let quads = store.quads();
        assert_eq!(quads[0][3], quads[1][3], "one blank-node mapping per call");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_iri_cache_is_bounded_and_returns_interned_ids() {
        let store = MemStore::new();
        let mut cache = IriCache::new(3);
        let iri = |i: usize| oxrdf::NamedNode::new_unchecked(format!("http://ex/i{i}"));
        let mut ids = Vec::new();
        for round in 0..3 {
            for i in 0..5 {
                let id = cache.intern(&store, iri(i).as_ref()).unwrap();
                assert!(cache.ids.len() <= 3);
                if round == 0 {
                    ids.push(id);
                } else {
                    assert_eq!(id, ids[i], "the same IRI always gets the same id");
                }
                assert_eq!(store.dict().lookup(iri(i).as_ref().into()).unwrap(), Some(id));
            }
        }
    }
}
