//! Connection-wide storage state (§8.6 `StorageShared`): statement pool, DecodeCache,
//! LookupCache, cached statistics, schema state and the §4.4 epoch / admission rule.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use rusqlite::ffi;
use sq_core::codec::{ENCODING_VERSION, RDF_LANG_STRING_IRI, SCHEMA_VERSION, WELL_KNOWN, XSD_STRING_IRI};
use sq_core::stats::Stats;
use sq_core::term::Term;
use sq_core::{Error, Result, Tag, TermId};

use crate::cache::ClockCache;
use crate::keys::OwnedKey;
use crate::pool::{PoolCounters, StmtPool};
use crate::raw::RawDb;
use crate::schema::{self, Detected, SchemaState};
use crate::scope::Abort;
use crate::template::Fixed;

/// Dictionary rows 1..=63 are reserved for `codec::WELL_KNOWN` IRIs (§2.1) and never change.
const WELL_KNOWN_ROWS: i64 = 63;

#[derive(Clone, Debug)]
pub struct StorageConfig {
    /// DecodeCache budget (approximate bytes of cached terms), default 64 MB.
    pub decode_cache_mb: u64,
    /// LookupCache capacity (term -> id entries), default 64K.
    pub lookup_cache_entries: usize,
}

impl Default for StorageConfig {
    fn default() -> Self {
        StorageConfig { decode_cache_mb: 64, lookup_cache_entries: 65536 }
    }
}

/// Dictionary counters (`sparql_status`, and tests asserting "no decode").
#[derive(Copy, Clone, Default, Debug)]
pub struct DictCounters {
    /// `Dictionary::decode` calls.
    pub decode_calls: u64,
    /// Dictionary rows read from SQLite by decode (cache misses).
    pub decode_db_rows: u64,
    /// Decode batches served by one merge scan instead of point lookups.
    pub decode_merge_scans: u64,
    /// Hash probes of `rdf_term_h` (lookup / intern misses).
    pub probes: u64,
    /// Dictionary rows inserted through this connection.
    pub terms_inserted: u64,
}

#[derive(Default)]
struct StatsCache {
    /// `rdf_meta.stats_gen` the cached value belongs to (None = unknown).
    generation: Option<i64>,
    /// Loaded inside a write transaction: re-read once outside it (it may be rolled back).
    provisional: bool,
    loaded: bool,
    stats: Option<Arc<Stats>>,
}

/// Shared IRI strings so dictionary literals do not each allocate their datatype.
pub(crate) struct CommonIris {
    pub xsd_string: Arc<str>,
    pub lang_string: Arc<str>,
}

/// `codec::WELL_KNOWN` as decoded terms, indexed by dictionary row.
fn well_known_terms() -> Box<[Option<Arc<Term>>]> {
    let mut terms = vec![None; WELL_KNOWN_ROWS as usize + 1];
    for (row, iri) in WELL_KNOWN {
        terms[*row as usize] = Some(Arc::new(Term::Iri(Arc::from(*iri))));
    }
    terms.into_boxed_slice()
}

pub struct StorageShared {
    db: RawDb,
    cfg: RefCell<StorageConfig>,
    pub(crate) pool: Rc<StmtPool>,
    pub(crate) decode_cache: RefCell<ClockCache<Arc<Term>>>,
    /// term hash -> (exact key, id).
    pub(crate) lookup_cache: RefCell<ClockCache<(OwnedKey, TermId)>>,
    schema_cookie: Cell<Option<i64>>,
    schema: RefCell<Option<Detected>>,
    /// `rdf_meta.dict_gen` the dictionary caches belong to (None = flush at next validation).
    dict_gen: Cell<Option<i64>>,
    /// Admission bound: only dictionary rows <= this (known committed) enter the caches.
    committed_max_row: Cell<i64>,
    stats: RefCell<StatsCache>,
    /// Scopes currently open through this store (savepoint name depth, rule R3).
    open_scopes: Cell<u32>,
    /// Forced-rollback statement, held while a scope savepoint is open (`scope`).
    pub(crate) abort: Abort,
    counters: Cell<DictCounters>,
    pub(crate) iris: CommonIris,
    /// Terms of the reserved well-known rows, indexed by row (§2.1): decoded without I/O,
    /// whether or not the schema exists.
    well_known: Box<[Option<Arc<Term>>]>,
    /// Placeholder pushed for decode misses before they are fetched.
    pub(crate) placeholder: Arc<Term>,
}

impl StorageShared {
    pub fn new(db: RawDb, cfg: StorageConfig) -> Rc<Self> {
        let decode_budget = (cfg.decode_cache_mb as usize).saturating_mul(1 << 20);
        let lookup_budget = cfg.lookup_cache_entries;
        Rc::new(StorageShared {
            db,
            cfg: RefCell::new(cfg),
            pool: StmtPool::new(db),
            decode_cache: RefCell::new(ClockCache::new(decode_budget)),
            lookup_cache: RefCell::new(ClockCache::new(lookup_budget)),
            schema_cookie: Cell::new(None),
            schema: RefCell::new(None),
            dict_gen: Cell::new(None),
            committed_max_row: Cell::new(0),
            stats: RefCell::new(StatsCache::default()),
            open_scopes: Cell::new(0),
            abort: Abort::default(),
            counters: Cell::new(DictCounters::default()),
            iris: CommonIris { xsd_string: Arc::from(XSD_STRING_IRI), lang_string: Arc::from(RDF_LANG_STRING_IRI) },
            well_known: well_known_terms(),
            placeholder: Arc::new(Term::Iri(Arc::from(""))),
        })
    }

    pub fn db(&self) -> RawDb {
        self.db
    }

    pub fn config(&self) -> StorageConfig {
        self.cfg.borrow().clone()
    }

    /// Applies new cache budgets (`sparql_config('decode_cache_mb', …)`).
    pub fn set_config(&self, cfg: StorageConfig) -> Result<()> {
        self.decode_cache
            .try_borrow_mut()
            .map_err(|_| Error::Reentrant)?
            .set_budget((cfg.decode_cache_mb as usize).saturating_mul(1 << 20));
        self.lookup_cache.try_borrow_mut().map_err(|_| Error::Reentrant)?.set_budget(cfg.lookup_cache_entries);
        *self.cfg.try_borrow_mut().map_err(|_| Error::Reentrant)? = cfg;
        Ok(())
    }

    // ------------------------------------------------------------------ epoch (§4.4)

    /// Once per API entry, inside the read scope. Re-detects the schema when the schema cookie
    /// changed; flushes the dictionary caches when `dict_gen` or the schema changed; refreshes
    /// `committed_max_row` only outside a write transaction (rollback safety); marks the
    /// statistics for reload when `stats_gen` changed.
    pub fn validate_epoch(&self) -> Result<SchemaState> {
        let cookie = {
            let mut st = self.pool.lease_fixed(Fixed::SchemaCookie)?;
            if !st.step()? {
                return Err(Error::internal("pragma_schema_version returned no row"));
            }
            st.col_i64(0)
        };
        let known = self.schema.try_borrow().map_err(|_| Error::Reentrant)?.is_some();
        if !known || self.schema_cookie.get() != Some(cookie) {
            let detected = schema::detect(self.db)?;
            self.flush_dictionary_caches()?;
            self.dict_gen.set(None);
            self.invalidate_stats()?;
            *self.schema.try_borrow_mut().map_err(|_| Error::Reentrant)? = Some(detected);
            self.schema_cookie.set(Some(cookie));
        }
        let state = self.schema_state_cached().ok_or(Error::Reentrant)?;
        if !matches!(state, SchemaState::Ready { .. }) {
            self.committed_max_row.set(0);
            return Ok(state);
        }
        let (dict_gen, stats_gen, max_row, versions) = {
            let mut st = self.pool.lease_fixed(Fixed::Epoch)?;
            if !st.step()? {
                return Err(Error::internal("epoch query returned no row"));
            }
            (st.col_i64(0), st.col_i64(1), st.col_opt_i64(2).unwrap_or(0), (st.col_opt_i64(3), st.col_opt_i64(4)))
        };
        if versions != (Some(SCHEMA_VERSION), Some(ENCODING_VERSION)) {
            // Version rows edited behind our back: re-detect (-> Incompatible).
            self.forget_schema();
            self.flush_dictionary_caches()?;
            self.committed_max_row.set(0);
            return self.schema_state();
        }
        if self.dict_gen.get() != Some(dict_gen) {
            self.flush_dictionary_caches()?;
            self.dict_gen.set(Some(dict_gen));
        }
        let writing = self.db.txn_state() == ffi::SQLITE_TXN_WRITE;
        if !writing {
            self.committed_max_row.set(max_row);
        }
        {
            let mut s = self.stats.try_borrow_mut().map_err(|_| Error::Reentrant)?;
            if s.generation != Some(stats_gen) || (s.provisional && !writing) {
                *s = StatsCache { generation: Some(stats_gen), ..StatsCache::default() };
            }
        }
        Ok(state)
    }

    /// Current schema state (detected on first use; refreshed by `validate_epoch`).
    pub fn schema_state(&self) -> Result<SchemaState> {
        if let Some(s) = self.schema_state_cached() {
            return Ok(s);
        }
        let detected = schema::detect(self.db)?;
        let state = detected.state.clone();
        *self.schema.try_borrow_mut().map_err(|_| Error::Reentrant)? = Some(detected);
        Ok(state)
    }

    fn schema_state_cached(&self) -> Option<SchemaState> {
        self.schema.try_borrow().ok()?.as_ref().map(|d| d.state.clone())
    }

    /// Ok(true) when the RDF tables exist, Ok(false) when absent, Error::Schema when
    /// incompatible.
    pub(crate) fn tables_present(&self) -> Result<bool> {
        match self.schema_state()? {
            SchemaState::Ready { .. } => Ok(true),
            SchemaState::Absent => Ok(false),
            SchemaState::Incompatible(m) => Err(Error::Schema(m)),
        }
    }

    pub(crate) fn require_ready(&self) -> Result<()> {
        if self.tables_present()? {
            Ok(())
        } else {
            Err(Error::Schema("the RDF store is not initialized (run sparql_init())".into()))
        }
    }

    /// Forgets the detected schema (after DDL or a rolled-back write scope).
    pub(crate) fn forget_schema(&self) {
        if let Ok(mut s) = self.schema.try_borrow_mut() {
            *s = None;
        }
        self.schema_cookie.set(None);
    }

    pub(crate) fn committed_max_row(&self) -> i64 {
        self.committed_max_row.get()
    }

    /// The fixed term of a reserved well-known dictionary id, if `id` is one.
    #[inline]
    pub(crate) fn well_known_term(&self, id: TermId) -> Option<&Arc<Term>> {
        if id.tag() != Tag::Iri {
            return None;
        }
        self.well_known.get(usize::try_from(id.dict_row()).ok()?)?.as_ref()
    }

    /// Admission rule: the reserved well-known rows are fixed; other rows only when known
    /// committed.
    #[inline]
    pub(crate) fn admissible(&self, row: i64) -> bool {
        row <= WELL_KNOWN_ROWS || row <= self.committed_max_row.get()
    }

    pub(crate) fn flush_dictionary_caches(&self) -> Result<()> {
        self.decode_cache.try_borrow_mut().map_err(|_| Error::Reentrant)?.clear();
        self.lookup_cache.try_borrow_mut().map_err(|_| Error::Reentrant)?.clear();
        Ok(())
    }

    /// Dictionary rows were deleted in the current transaction (`vacuum_terms`): flush, admit
    /// nothing until `committed_max_row` is refreshed outside a write transaction, and flush
    /// again at the next validation whatever `dict_gen` then says (the deletion may roll back).
    pub(crate) fn note_dictionary_rows_deleted(&self) -> Result<()> {
        self.flush_dictionary_caches()?;
        self.dict_gen.set(None);
        self.committed_max_row.set(0);
        Ok(())
    }

    /// A write scope rolled back: forget everything derived from possibly undone writes.
    pub(crate) fn after_rollback(&self) {
        self.forget_schema();
        self.dict_gen.set(None);
        let _ = self.invalidate_stats();
    }

    // ------------------------------------------------------------------ statistics

    fn invalidate_stats(&self) -> Result<()> {
        *self.stats.try_borrow_mut().map_err(|_| Error::Reentrant)? = StatsCache::default();
        Ok(())
    }

    /// Cached statistics, loaded from the `rdf_stat_*` tables on first use after a change.
    pub(crate) fn stats(&self) -> Option<Arc<Stats>> {
        {
            let s = self.stats.try_borrow().ok()?;
            if s.loaded {
                return s.stats.clone();
            }
        }
        if !matches!(self.schema_state(), Ok(SchemaState::Ready { .. })) {
            return None;
        }
        let (generation, stats) = crate::analyze::load_stats(self).ok()?;
        let provisional = self.db.txn_state() == ffi::SQLITE_TXN_WRITE;
        let stats = stats.map(Arc::new);
        if let Ok(mut s) = self.stats.try_borrow_mut() {
            *s = StatsCache { generation: Some(generation), provisional, loaded: true, stats: stats.clone() };
        }
        stats
    }

    /// Installs freshly computed statistics (analyze).
    pub(crate) fn set_stats(&self, generation: i64, stats: Arc<Stats>) {
        let provisional = self.db.txn_state() == ffi::SQLITE_TXN_WRITE;
        if let Ok(mut s) = self.stats.try_borrow_mut() {
            *s = StatsCache { generation: Some(generation), provisional, loaded: true, stats: Some(stats) };
        }
    }

    // ------------------------------------------------------------------ pool / close hook

    pub fn arm_pool(&self) {
        self.pool.arm();
    }

    pub fn is_armed(&self) -> bool {
        self.pool.is_armed()
    }

    /// Close hook (§8.7): disarm and finalize every idle statement.
    pub fn disarm_and_finalize(&self) {
        self.pool.disarm_and_finalize();
    }

    /// R6: steps a pooled `SELECT 1`; after `sqlite3_interrupt` it fails with
    /// `Error::Interrupted` while any statement of the connection is running.
    pub fn check_interrupt(&self) -> Result<()> {
        let mut st = self.pool.lease_fixed(Fixed::Select1)?;
        st.step().map(|_| ())
    }

    // ------------------------------------------------------------------ scopes / counters

    /// Savepoint depth for a new scope: at least the caller's depth, above every open one.
    pub(crate) fn enter_scope(&self, depth: u32) -> u32 {
        let d = depth.max(self.open_scopes.get());
        self.open_scopes.set(self.open_scopes.get() + 1);
        d
    }

    pub(crate) fn leave_scope(&self) {
        self.open_scopes.set(self.open_scopes.get().saturating_sub(1));
    }

    pub fn dict_counters(&self) -> DictCounters {
        self.counters.get()
    }

    /// Statement-pool counters (prepared / reused / finalized statements).
    pub fn pool_counters(&self) -> PoolCounters {
        self.pool.counters()
    }

    pub(crate) fn count(&self, f: impl FnOnce(&mut DictCounters)) {
        let mut c = self.counters.get();
        f(&mut c);
        self.counters.set(c);
    }

    /// `sparql_status` rows owned by the storage layer.
    pub fn status(&self) -> Vec<(String, String)> {
        let schema = match self.schema_state_cached() {
            None => "unknown".to_string(),
            Some(SchemaState::Absent) => "absent".to_string(),
            Some(SchemaState::Ready { schema_version }) => format!("ready (schema_version {schema_version})"),
            Some(SchemaState::Incompatible(m)) => format!("incompatible: {m}"),
        };
        let (dc_entries, dc_bytes, dc_hits, dc_misses) = self
            .decode_cache
            .try_borrow()
            .map(|c| (c.len(), c.cost(), c.hits(), c.misses()))
            .unwrap_or_default();
        let lc_entries = self.lookup_cache.try_borrow().map(|c| c.len()).unwrap_or_default();
        let (stats_gen, stats_loaded) =
            self.stats.try_borrow().map(|s| (s.generation, s.loaded && s.stats.is_some())).unwrap_or((None, false));
        let pc = self.pool.counters();
        let dc = self.counters.get();
        let opt = |v: Option<i64>| v.map_or_else(|| "unknown".to_string(), |v| v.to_string());
        vec![
            ("schema_state".into(), schema),
            ("dict_gen".into(), opt(self.dict_gen.get())),
            ("stats_gen".into(), opt(stats_gen)),
            ("stats_loaded".into(), (stats_loaded as u8).to_string()),
            ("committed_max_row".into(), self.committed_max_row.get().to_string()),
            ("decode_cache_entries".into(), dc_entries.to_string()),
            ("decode_cache_bytes".into(), dc_bytes.to_string()),
            ("decode_cache_hits".into(), dc_hits.to_string()),
            ("decode_cache_misses".into(), dc_misses.to_string()),
            ("decode_calls".into(), dc.decode_calls.to_string()),
            ("decode_db_rows".into(), dc.decode_db_rows.to_string()),
            ("decode_merge_scans".into(), dc.decode_merge_scans.to_string()),
            ("lookup_cache_entries".into(), lc_entries.to_string()),
            ("dictionary_probes".into(), dc.probes.to_string()),
            ("pool_statements".into(), self.pool.idle_statements().to_string()),
            ("pool_leased".into(), self.pool.leased_statements().to_string()),
            ("pool_prepared".into(), pc.prepared.to_string()),
            ("pool_reused".into(), pc.reused.to_string()),
            ("armed".into(), (self.pool.is_armed() as u8).to_string()),
        ]
    }
}
