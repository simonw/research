//! §2.3 bulk loader: parse with oxrdfio (base IRI, target graph), map blank-node labels to
//! fresh counter ids per load, de-duplicate terms through a load-local map plus hash probes of
//! the existing dictionary, append new dictionary rows in row order, then insert the sorted,
//! deduplicated quads — in bulk mode with the secondary indexes dropped and rebuilt.
//!
//! Large inputs are parsed on a background thread feeding bounded batches, so parsing
//! overlaps encoding and inserting (SQLite is only ever called from the calling thread).
use std::borrow::Cow;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::mpsc;
use std::time::Instant;

use rustc_hash::FxHashMap;
use sq_core::codec::{self, FIRST_USER_ROW, WELL_KNOWN};
use sq_core::{Error, Result, Tag, TermId};

use crate::analyze::LOADED_SINCE_ANALYZE;
use crate::dict::row_matches;
use crate::keys::{KeyLike, KeyView, OwnedKey};
use crate::raw::{RawDb, RawStmt};
use crate::schema::{QUAD_INDEXES, TERM_HASH_INDEX_DDL};
use crate::shared::StorageShared;
use crate::store::SqliteStore;
use crate::template::{Fixed, TERM_INSERT_BATCH};
use crate::writer::bind_term_row;

pub enum LoadSource<'a> {
    Bytes(&'a [u8]),
    File(&'a Path),
}

#[derive(Clone, Debug)]
pub struct LoadOptions {
    pub format: oxrdfio::RdfFormat,
    pub base_iri: Option<String>,
    /// Target graph IRI for triples in the default graph (None = default graph).
    pub graph: Option<String>,
    pub bulk: BulkMode,
}

impl LoadOptions {
    pub fn new(format: oxrdfio::RdfFormat) -> Self {
        LoadOptions { format, base_iri: None, graph: None, bulk: BulkMode::Auto }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum BulkMode {
    /// Bulk mode iff the load has >= 100K quads and is > 0.3x the current `rdf_quad` size.
    #[default]
    Auto,
    Never,
    Always,
}

#[derive(Clone, Debug, Default)]
pub struct LoadReport {
    /// Quads produced by the parser (before de-duplication).
    pub quads_read: u64,
    /// Quads that were not already stored.
    pub quads_new: u64,
    /// New dictionary rows.
    pub terms_new: u64,
    /// Secondary indexes were dropped and rebuilt.
    pub bulk: bool,
    pub ms: u64,
}

const CHUNK_QUADS: usize = 100_000;
const TERM_MAP_CAP: usize = 8_000_000;
const BULK_MIN_QUADS: usize = 100_000;
const BULK_MIN_RATIO: f64 = 0.3;
/// Re-analyze when the store grew by more than this fraction since `analyzed_quads`.
const REANALYZE_GROWTH: f64 = 0.25;
const BULK_SORT_THREADS: i64 = 4;
/// Page-cache (= sorter run) size while bulk mode rebuilds the indexes.
const BULK_SORT_RUN_BYTES: i64 = 16 << 20;
/// Inputs at least this large are parsed on a background thread.
const PIPELINE_MIN_BYTES: usize = 1 << 20;
/// Quads per batch handed over by the parser thread, and batches in flight.
const PIPELINE_BATCH: usize = 4096;
const PIPELINE_DEPTH: usize = 8;

/// Chunking limits of the encoder (tests shrink them to exercise every path).
#[derive(Copy, Clone, Debug)]
pub(crate) struct Limits {
    /// Quads per chunk: dictionary misses are probed / allocated once per chunk.
    pub chunk_quads: usize,
    /// The load-local term map is cleared (and probing switched on) beyond this size.
    pub term_map_cap: usize,
}

impl Limits {
    pub const DEFAULT: Limits = Limits { chunk_quads: CHUNK_QUADS, term_map_cap: TERM_MAP_CAP };
}

pub(crate) fn bulk_load(store: &SqliteStore, src: LoadSource<'_>, opts: &LoadOptions) -> Result<LoadReport> {
    load_with(store, src, opts, Limits::DEFAULT)
}

pub(crate) fn load_with(store: &SqliteStore, src: LoadSource<'_>, opts: &LoadOptions, limits: Limits) -> Result<LoadReport> {
    let start = Instant::now();
    store.ensure_schema()?;
    let shared = store.shared();
    let scope = store.write_scope(0)?;
    let parser = parser_for(opts)?;
    let mut enc = Encoder::new(shared, limits)?;
    parse_into(&mut enc, parser, src)?;
    enc.finish()?;
    let mut report = LoadReport { quads_read: enc.quads_read, terms_new: enc.terms_new, ..LoadReport::default() };
    if enc.next_bnode != enc.first_bnode {
        shared.meta_set("next_bnode", enc.next_bnode)?;
    }
    let mut quads = std::mem::take(&mut enc.quads);
    drop(enc);
    quads.sort_unstable();
    quads.dedup();
    debug_assert!(quads.iter().all(crate::writer::storable_quad));
    let bulk_insert = if use_bulk(shared, opts.bulk, quads.len())? { insert_bulk(shared, &quads)? } else { None };
    report.bulk = bulk_insert.is_some();
    report.quads_new = match bulk_insert {
        Some(n) => {
            shared.register_graphs(&quads)?;
            n
        }
        None => shared.insert_quads(&mut quads)?,
    };
    drop(quads);
    let analyzed = if should_analyze(shared, report.quads_new)? {
        Some(crate::analyze::analyze_in_scope(shared)?)
    } else {
        None
    };
    scope.commit()?;
    if let Some((generation, stats)) = analyzed {
        shared.set_stats(generation, stats);
    }
    report.ms = start.elapsed().as_millis() as u64;
    Ok(report)
}

fn parser_for(opts: &LoadOptions) -> Result<oxrdfio::RdfParser> {
    let mut parser = oxrdfio::RdfParser::from_format(opts.format);
    if let Some(base) = &opts.base_iri {
        parser = parser.with_base_iri(base.as_str()).map_err(|e| Error::Invalid(format!("base IRI <{base}>: {e}")))?;
    }
    if let Some(g) = &opts.graph {
        let g = oxrdf::NamedNode::new(g.as_str()).map_err(|e| Error::Invalid(format!("graph IRI <{g}>: {e}")))?;
        parser = parser.with_default_graph(g);
    }
    Ok(parser)
}

fn open_file(path: &Path) -> Result<BufReader<File>> {
    let f = File::open(path).map_err(|e| Error::Invalid(format!("{}: {e}", path.display())))?;
    Ok(BufReader::with_capacity(1 << 20, f))
}

fn parse_error(e: impl std::fmt::Display) -> Error {
    Error::Parse(e.to_string())
}

type Batch = Result<Vec<oxrdf::Quad>>;

/// Feeds every parsed quad to the encoder, through a parser thread for large inputs.
fn parse_into(enc: &mut Encoder<'_>, parser: oxrdfio::RdfParser, src: LoadSource<'_>) -> Result<()> {
    let pipelined = match &src {
        LoadSource::Bytes(b) => b.len() >= PIPELINE_MIN_BYTES,
        LoadSource::File(p) => std::fs::metadata(p).is_ok_and(|m| m.len() >= PIPELINE_MIN_BYTES as u64),
    };
    if !pipelined {
        return match src {
            LoadSource::Bytes(bytes) => parser.for_slice(bytes).try_for_each(|q| enc.push(&q.map_err(parse_error)?)),
            LoadSource::File(path) => {
                parser.for_reader(open_file(path)?).try_for_each(|q| enc.push(&q.map_err(parse_error)?))
            }
        };
    }
    std::thread::scope(|scope| {
        let (tx, rx) = mpsc::sync_channel::<Batch>(PIPELINE_DEPTH);
        // Processed batches go back to the parser thread, which frees them: deallocating in
        // the allocating thread is cheaper and keeps that work off the encoding thread.
        let (back_tx, back_rx) = mpsc::channel::<Vec<oxrdf::Quad>>();
        let producer = scope.spawn(move || produce(parser, src, tx, &back_rx));
        let mut result = Ok(());
        for batch in rx.iter() {
            result = batch.and_then(|quads| {
                quads.iter().try_for_each(|q| enc.push(q))?;
                let _ = back_tx.send(quads);
                Ok(())
            });
            if result.is_err() {
                break;
            }
        }
        // Dropping the receiver makes a still-running producer stop at its next send.
        drop(rx);
        drop(back_tx);
        match producer.join() {
            Ok(()) => result,
            Err(_) => result.and(Err(Error::internal("RDF parser thread panicked"))),
        }
    })
}

/// Parser thread body: sends batches until the input ends, an error occurs (sent as the last
/// item) or the receiver hangs up; frees the batches the encoder hands back. `tx` is dropped
/// before the final drain, which ends the encoder's loop and with it the return channel.
fn produce(
    parser: oxrdfio::RdfParser,
    src: LoadSource<'_>,
    tx: mpsc::SyncSender<Batch>,
    back: &mpsc::Receiver<Vec<oxrdf::Quad>>,
) {
    fn pump<E: std::fmt::Display>(
        quads: impl Iterator<Item = std::result::Result<oxrdf::Quad, E>>,
        tx: &mpsc::SyncSender<Batch>,
        back: &mpsc::Receiver<Vec<oxrdf::Quad>>,
    ) {
        let mut batch = Vec::with_capacity(PIPELINE_BATCH);
        for q in quads {
            match q {
                Ok(q) => batch.push(q),
                Err(e) => {
                    let _ = tx.send(Err(parse_error(e)));
                    return;
                }
            }
            if batch.len() == PIPELINE_BATCH {
                if tx.send(Ok(std::mem::replace(&mut batch, Vec::with_capacity(PIPELINE_BATCH)))).is_err() {
                    return;
                }
                back.try_iter().for_each(drop);
            }
        }
        let _ = tx.send(Ok(batch));
    }
    match src {
        LoadSource::Bytes(bytes) => pump(parser.for_slice(bytes), &tx, back),
        LoadSource::File(path) => match open_file(path) {
            Ok(reader) => pump(parser.for_reader(reader), &tx, back),
            Err(e) => {
                let _ = tx.send(Err(e));
            }
        },
    }
    drop(tx);
    back.iter().for_each(drop);
}

/// §2.3 step 5: bulk mode for large loads relative to the current store size.
pub(crate) fn use_bulk(shared: &StorageShared, mode: BulkMode, n: usize) -> Result<bool> {
    match mode {
        BulkMode::Always => Ok(n > 0),
        BulkMode::Never => Ok(false),
        BulkMode::Auto if n < BULK_MIN_QUADS => Ok(false),
        BulkMode::Auto => {
            // Counting stops at the size beyond which bulk mode cannot apply.
            let limit = (n as f64 / BULK_MIN_RATIO).ceil() as i64;
            let existing = count_quads_capped(shared, limit)?;
            Ok(existing < limit && n as f64 > BULK_MIN_RATIO * existing as f64)
        }
    }
}

fn count_quads_capped(shared: &StorageShared, limit: i64) -> Result<i64> {
    let mut st = shared.pool.lease_fixed(Fixed::QuadCountCapped)?;
    st.bind_i64(1, limit)?;
    st.step()?;
    Ok(st.col_i64(0))
}

/// `DROP INDEX`; Ok(false) when SQLite refuses with SQLITE_LOCKED because another statement
/// of this connection is reading (e.g. `SELECT sparql_load(x) FROM t`).
fn try_drop_index(db: RawDb, name: &str) -> Result<bool> {
    match db.exec(&format!("DROP INDEX IF EXISTS main.{name}")) {
        Ok(()) => Ok(true),
        Err(Error::Sqlite { code, .. }) if code & 0xff == rusqlite::ffi::SQLITE_LOCKED => Ok(false),
        Err(e) => Err(e),
    }
}

fn pragma_i64(db: RawDb, pragma: &str) -> Result<i64> {
    let mut st = RawStmt::prepare(db, &format!("PRAGMA {pragma}"), false)?;
    Ok(if st.step()? { st.col_i64(0) } else { 0 })
}

/// Drops the 8 secondary indexes, inserts in primary-key order and rebuilds them; returns
/// the number of new quads, or `None` (nothing inserted: the caller inserts incrementally)
/// when the indexes cannot be dropped right now.
///
/// During the rebuild, `PRAGMA threads` lets the sorter use worker threads and
/// `cache_size` is capped at 16 MB, which is also the sorter's run size: a single huge
/// in-memory run (e.g. `cache_size=-524288`) sorts markedly slower than 16 MB runs merged in
/// parallel (measured 4.9 s vs 3.0 s for three 1M-row indexes). Both pragmas are restored.
fn insert_bulk(shared: &StorageShared, quads: &[[TermId; 4]]) -> Result<Option<u64>> {
    let db = shared.db();
    for (k, (name, _)) in QUAD_INDEXES.iter().enumerate() {
        if !try_drop_index(db, name)? {
            // Normally the first DROP already fails (the condition is connection-wide).
            for (_, ddl) in &QUAD_INDEXES[..k] {
                db.exec(ddl)?;
            }
            shared.refresh_abort()?;
            return Ok(None);
        }
    }
    shared.forget_schema();
    let threads = pragma_i64(db, "threads")?;
    let cache = pragma_i64(db, "main.cache_size")?;
    let page = pragma_i64(db, "main.page_size")?;
    let cache_bytes = if cache < 0 { -cache * 1024 } else { cache * page };
    let capped = cache_bytes > BULK_SORT_RUN_BYTES;
    if capped {
        db.exec(&format!("PRAGMA main.cache_size=-{}", BULK_SORT_RUN_BYTES / 1024))?;
    }
    db.exec(&format!("PRAGMA threads={BULK_SORT_THREADS}"))?;
    let built = shared
        .apply_quads(Fixed::QuadInsert, quads)
        .and_then(|n| QUAD_INDEXES.iter().try_for_each(|(_, ddl)| db.exec(ddl)).map(|()| n));
    let restored = db
        .exec(&format!("PRAGMA threads={threads}"))
        .and_then(|()| if capped { db.exec(&format!("PRAGMA main.cache_size={cache}")) } else { Ok(()) });
    let n = built?;
    restored?;
    // CREATE INDEX expired every prepared statement (scope::Abort).
    shared.refresh_abort()?;
    shared.forget_schema();
    Ok(Some(n))
}

/// §2.3 step 6: analyze on the first load, or when loads added more than 25% of
/// `analyzed_quads` since the last analyze. The running counter (`LOADED_SINCE_ANALYZE`,
/// reset by analyze) makes the check O(1) however small each load is.
fn should_analyze(shared: &StorageShared, quads_new: u64) -> Result<bool> {
    if quads_new == 0 {
        return Ok(false);
    }
    let analyzed = shared.meta_get("analyzed_quads")?.unwrap_or(0).max(0);
    if analyzed == 0 {
        return Ok(true);
    }
    let quads_new = i64::try_from(quads_new).map_err(|_| Error::internal("load size overflows i64"))?;
    shared.meta_add(LOADED_SINCE_ANALYZE, quads_new)?;
    let loaded = shared.meta_get(LOADED_SINCE_ANALYZE)?.unwrap_or(0);
    Ok(loaded as f64 > REANALYZE_GROWTH * analyzed as f64)
}

/// A load-local term map pre-seeded with the `codec::WELL_KNOWN` IRIs (fixed rows), so every
/// term costs one map probe.
fn well_known_term_map() -> FxHashMap<OwnedKey, TermId> {
    WELL_KNOWN
        .iter()
        .map(|(row, iri)| (KeyView::iri(iri).to_owned_key(), TermId::from_dict(*row, Tag::Iri)))
        .collect()
}

/// A dictionary miss of the current chunk.
struct Pending {
    key: OwnedKey,
    h: i64,
    /// TYPED: datatype IRI id (possibly a placeholder of an earlier pending entry).
    dt: Option<TermId>,
    /// Final id (UNBOUND until probed / allocated).
    id: TermId,
}

impl Pending {
    fn view(&self) -> KeyView<'_> {
        KeyView { tag: self.key.tag, lex: &self.key.lex, extra: Cow::Borrowed(&self.key.extra) }
    }
}

/// Term encoder for one load call. Unresolved terms of the current chunk are represented in
/// `chunk` by TEMP ids whose index points into `pending` (TEMP ids are never storable, so
/// they cannot collide with real ids).
struct Encoder<'a> {
    shared: &'a StorageShared,
    limits: Limits,
    next_row: i64,
    first_bnode: i64,
    next_bnode: i64,
    /// Probe `rdf_term_h` for misses: the dictionary had user rows at load start, or the
    /// load-local map was cleared.
    probe: bool,
    /// `rdf_term_h` was dropped for the load (nothing to probe); rebuilt by `finish` or before
    /// the first probe.
    hash_index_dropped: bool,
    terms: FxHashMap<OwnedKey, TermId>,
    bnodes: FxHashMap<String, TermId>,
    pending: Vec<Pending>,
    chunk: Vec<[TermId; 4]>,
    quads: Vec<[TermId; 4]>,
    quads_read: u64,
    terms_new: u64,
}

impl<'a> Encoder<'a> {
    fn new(shared: &'a StorageShared, limits: Limits) -> Result<Self> {
        let next_row = shared.next_dict_row()?;
        let next_bnode = shared.meta_get("next_bnode")?.unwrap_or(1).max(1);
        let probe = next_row > FIRST_USER_ROW;
        // Appending to a dictionary that needs no probing: build the random-keyed hash index
        // once at the end instead of maintaining it per row.
        let hash_index_dropped = !probe && try_drop_index(shared.db(), "rdf_term_h")?;
        Ok(Encoder {
            shared,
            limits,
            next_row,
            first_bnode: next_bnode,
            next_bnode,
            probe,
            hash_index_dropped,
            terms: well_known_term_map(),
            bnodes: FxHashMap::default(),
            pending: Vec::new(),
            chunk: Vec::with_capacity(limits.chunk_quads.min(CHUNK_QUADS)),
            quads: Vec::new(),
            quads_read: 0,
            terms_new: 0,
        })
    }

    fn push(&mut self, q: &oxrdf::Quad) -> Result<()> {
        let g = match &q.graph_name {
            oxrdf::GraphName::DefaultGraph => TermId::DEFAULT_GRAPH,
            oxrdf::GraphName::NamedNode(n) => self.encode(n.as_ref().into())?,
            oxrdf::GraphName::BlankNode(b) => self.bnode(b.as_str()),
        };
        let s = match &q.subject {
            oxrdf::NamedOrBlankNode::NamedNode(n) => self.encode(n.as_ref().into())?,
            oxrdf::NamedOrBlankNode::BlankNode(b) => self.bnode(b.as_str()),
        };
        let p = self.encode(q.predicate.as_ref().into())?;
        let o = self.encode(q.object.as_ref())?;
        self.chunk.push([g, s, p, o]);
        self.quads_read += 1;
        if self.chunk.len() >= self.limits.chunk_quads {
            self.flush_chunk()?;
        }
        Ok(())
    }

    /// Resolves the last chunk and restores the hash index.
    fn finish(&mut self) -> Result<()> {
        self.flush_chunk()?;
        self.restore_hash_index()
    }

    fn restore_hash_index(&mut self) -> Result<()> {
        if self.hash_index_dropped {
            self.shared.db().exec(TERM_HASH_INDEX_DDL)?;
            self.hash_index_dropped = false;
            self.shared.refresh_abort()?;
        }
        Ok(())
    }

    fn bnode(&mut self, label: &str) -> TermId {
        if let Some(id) = self.bnodes.get(label) {
            return *id;
        }
        let id = TermId::bnode(self.next_bnode);
        self.next_bnode += 1;
        self.bnodes.insert(label.to_string(), id);
        id
    }

    fn encode(&mut self, t: oxrdf::TermRef<'_>) -> Result<TermId> {
        if let Some(id) = codec::encode_inline(t) {
            return Ok(id);
        }
        if let oxrdf::TermRef::BlankNode(b) = t {
            return Ok(self.bnode(b.as_str()));
        }
        let view = KeyView::of(t).ok_or_else(|| Error::Unsupported(format!("cannot store term {t}")))?;
        self.encode_key(&view)
    }

    fn encode_key(&mut self, view: &KeyView<'_>) -> Result<TermId> {
        if let Some(id) = self.terms.get(view.as_dyn()) {
            return Ok(*id);
        }
        // The datatype IRI is encoded first, so it gets the lower row.
        let dt = if view.tag == Tag::Typed { Some(self.encode_key(&KeyView::iri(&view.extra))?) } else { None };
        let idx = u32::try_from(self.pending.len()).map_err(|_| Error::internal("too many pending terms"))?;
        let placeholder = TermId::temp(idx, view.tag);
        let key = view.to_owned_key();
        self.pending.push(Pending { key: key.clone(), h: view.hash64(), dt, id: TermId::UNBOUND });
        self.terms.insert(key, placeholder);
        Ok(placeholder)
    }

    #[inline]
    fn resolve(pending: &[Pending], id: TermId) -> TermId {
        match id.temp_index() {
            Some(i) => pending[i as usize].id,
            None => id,
        }
    }

    /// Resolves the chunk's misses (probe, then allocate), rewrites its quads and appends
    /// them to `quads`.
    fn flush_chunk(&mut self) -> Result<()> {
        if !self.pending.is_empty() {
            if self.probe {
                self.restore_hash_index()?;
                self.probe_pending()?;
            }
            self.allocate_pending()?;
            for p in &self.pending {
                if let Some(v) = self.terms.get_mut(&p.key as &dyn KeyLike) {
                    *v = p.id;
                }
            }
        }
        let pending = &self.pending;
        self.quads.extend(self.chunk.drain(..).map(|q| q.map(|t| Self::resolve(pending, t))));
        self.pending.clear();
        if self.terms.len() >= self.limits.term_map_cap {
            self.terms = well_known_term_map();
            self.probe = true;
        }
        Ok(())
    }

    /// Hash probes in hash order: non-TYPED terms first, then TYPED literals whose datatype
    /// is known to be stored (a new datatype IRI implies a new literal).
    fn probe_pending(&mut self) -> Result<()> {
        let mut st = self.shared.pool.lease_fixed(Fixed::TermByHash)?;
        for typed in [false, true] {
            let mut order: Vec<usize> =
                (0..self.pending.len()).filter(|&i| (self.pending[i].key.tag == Tag::Typed) == typed).collect();
            order.sort_unstable_by_key(|&i| self.pending[i].h);
            for i in order {
                let dt = match self.pending[i].dt {
                    Some(d) => {
                        let d = Self::resolve(&self.pending, d);
                        if !d.is_bound() {
                            continue;
                        }
                        d
                    }
                    None => TermId::UNBOUND,
                };
                let p = &self.pending[i];
                let view = p.view();
                st.reset();
                st.bind_i64(1, p.h)?;
                self.shared.count(|c| c.probes += 1);
                let mut found = TermId::UNBOUND;
                while st.step()? {
                    if row_matches(&st, 1, &view, dt) {
                        found = TermId::from_dict(st.col_i64(0), view.tag);
                        break;
                    }
                }
                self.pending[i].id = found;
            }
        }
        Ok(())
    }

    /// New rows for the remaining misses, in first-occurrence order (datatype IRIs precede
    /// their literals), appended in row order, `TERM_INSERT_BATCH` rows per statement.
    fn allocate_pending(&mut self) -> Result<()> {
        let mut fresh: Vec<usize> = Vec::new();
        for i in 0..self.pending.len() {
            if self.pending[i].id.is_bound() {
                continue;
            }
            if let Some(d) = self.pending[i].dt {
                let d = Self::resolve(&self.pending, d);
                if !d.is_bound() {
                    return Err(Error::internal("datatype IRI allocated after its literal"));
                }
                self.pending[i].dt = Some(d);
            }
            self.pending[i].id = TermId::from_dict(self.next_row, self.pending[i].key.tag);
            self.next_row += 1;
            fresh.push(i);
        }
        let mut chunks = fresh.chunks_exact(TERM_INSERT_BATCH);
        if chunks.len() > 0 {
            let mut many = self.shared.pool.lease_fixed(Fixed::TermInsertMany)?;
            for chunk in &mut chunks {
                many.reset();
                for (k, &i) in chunk.iter().enumerate() {
                    let p = &self.pending[i];
                    bind_term_row(&mut many, 1 + 6 * k as i32, p.id.dict_row(), &p.view(), p.dt, p.h)?;
                }
                many.run()?;
            }
            let batched = fresh.len() - chunks.remainder().len();
            self.shared.count(|c| c.terms_inserted += batched as u64);
        }
        for &i in chunks.remainder() {
            let p = &self.pending[i];
            self.shared.insert_term_row(p.id.dict_row(), &p.view(), p.dt, p.h)?;
        }
        self.terms_new += fresh.len() as u64;
        Ok(())
    }
}
