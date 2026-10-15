//! sq-sqlite: SQLite storage — DDL, RawDb, StmtPool, SqlCursor, dictionary + caches,
//! read/write scopes, epoch validation, bulk loader, analyze. See docs/SPEC.md §2, §4.4, §5.2, §8.6.
#![allow(unused_variables, dead_code)]
use std::rc::Rc;
use std::sync::Arc;

use sq_core::stats::{CostConstants, Stats};
use sq_core::store::*;
use sq_core::term::Term;
use sq_core::{Result, TermId};

/// Non-owning handle to the connection's `sqlite3*` (Copy).
#[derive(Copy, Clone, Debug)]
pub struct RawDb(std::ptr::NonNull<rusqlite::ffi::sqlite3>);
impl RawDb {
    /// # Safety
    /// `p` must be a valid open connection that outlives every use of this handle.
    pub unsafe fn from_ptr(p: *mut rusqlite::ffi::sqlite3) -> Self {
        RawDb(std::ptr::NonNull::new(p).expect("null sqlite3*"))
    }
    pub fn as_ptr(&self) -> *mut rusqlite::ffi::sqlite3 {
        self.0.as_ptr()
    }
    pub fn txn_state(&self) -> i32 {
        todo!()
    }
    pub fn autocommit(&self) -> bool {
        todo!()
    }
    pub fn exec(&self, sql: &str) -> Result<()> {
        todo!()
    }
}

#[derive(Clone, Debug)]
pub struct StorageConfig {
    pub decode_cache_mb: u64,
    pub lookup_cache_entries: usize,
}
impl Default for StorageConfig {
    fn default() -> Self {
        StorageConfig { decode_cache_mb: 64, lookup_cache_entries: 65536 }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchemaState {
    Absent,
    Ready { schema_version: i64 },
    Incompatible(String),
}

/// Connection-wide storage state: StmtPool, DecodeCache, LookupCache, stats, epoch,
/// committed_max_row, armed flag.
pub struct StorageShared {
    db: RawDb,
    cfg: StorageConfig,
}
impl StorageShared {
    pub fn new(db: RawDb, cfg: StorageConfig) -> Rc<Self> {
        Rc::new(StorageShared { db, cfg })
    }
    pub fn db(&self) -> RawDb {
        self.db
    }
    /// §4.4, once per API entry.
    pub fn validate_epoch(&self) -> Result<SchemaState> {
        todo!()
    }
    pub fn arm_pool(&self) {
        todo!()
    }
    pub fn is_armed(&self) -> bool {
        todo!()
    }
    pub fn disarm_and_finalize(&self) {
        todo!()
    }
    pub fn status(&self) -> Vec<(String, String)> {
        todo!()
    }
}

/// RAII read scope (§8.6): SAVEPOINT sq_r{depth} when no transaction is open; RELEASE on drop.
pub struct ReadScope {
    _p: (),
}
/// RAII write scope (§8.6): `commit()` releases; drop without commit rolls back.
pub struct WriteScope {
    _p: (),
}
impl WriteScope {
    pub fn commit(self) -> Result<()> {
        todo!()
    }
}

#[derive(Clone)]
pub struct SqliteStore(pub Rc<StorageShared>);
impl SqliteStore {
    pub fn new(shared: Rc<StorageShared>) -> Self {
        SqliteStore(shared)
    }
    /// `sparql_init()`: idempotent DDL; returns schema_version.
    pub fn ensure_schema(&self) -> Result<i64> {
        todo!()
    }
    pub fn read_scope(&self, depth: u32) -> Result<ReadScope> {
        todo!()
    }
    pub fn write_scope(&self, depth: u32) -> Result<WriteScope> {
        todo!()
    }
    /// §2.3
    pub fn bulk_load(&self, src: LoadSource<'_>, opts: &LoadOptions) -> Result<LoadReport> {
        todo!()
    }
    pub fn analyze(&self) -> Result<Arc<Stats>> {
        todo!()
    }
    pub fn vacuum_terms(&self) -> Result<u64> {
        todo!()
    }
    pub fn check(&self) -> Result<String> {
        todo!()
    }
    pub fn explain_query_plan(&self, sql: &str) -> Result<String> {
        todo!()
    }
}

pub enum LoadSource<'a> {
    Bytes(&'a [u8]),
    File(&'a std::path::Path),
}
#[derive(Clone, Debug)]
pub struct LoadOptions {
    pub format: oxrdfio::RdfFormat,
    pub base_iri: Option<String>,
    pub graph: Option<String>,
    pub bulk: BulkMode,
}
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum BulkMode {
    #[default]
    Auto,
    Never,
    Always,
}
#[derive(Clone, Debug, Default)]
pub struct LoadReport {
    pub quads_read: u64,
    pub quads_new: u64,
    pub terms_new: u64,
    pub bulk: bool,
    pub ms: u64,
}

impl Dictionary for SqliteStore {
    fn lookup(&self, t: oxrdf::TermRef<'_>) -> Result<Option<TermId>> {
        todo!()
    }
    fn decode(&self, ids: &[TermId], out: &mut Vec<Arc<Term>>) -> Result<()> {
        todo!()
    }
}

impl QuadStore for SqliteStore {
    fn cursor(&self, spec: &ScanSpec) -> Result<Box<dyn QuadCursor>> {
        todo!()
    }
    fn count(&self, spec: &ScanSpec, cap: Option<u64>) -> Result<u64> {
        todo!()
    }
    fn dict(&self) -> &dyn Dictionary {
        self
    }
    fn named_graphs(&self) -> Result<Vec<TermId>> {
        todo!()
    }
    fn stats(&self) -> Option<Arc<Stats>> {
        todo!()
    }
    fn costs(&self) -> &CostConstants {
        &sq_core::stats::SQL_CURSOR_COSTS
    }
    fn caps(&self) -> StoreCaps {
        StoreCaps { sql: true, reader: ReaderKind::SqlCursor }
    }
}

impl QuadStoreWriter for SqliteStore {
    fn intern(&self, t: oxrdf::TermRef<'_>) -> Result<TermId> {
        todo!()
    }
    fn fresh_bnode(&self) -> Result<TermId> {
        todo!()
    }
    fn insert(&self, quads: &mut Vec<[TermId; 4]>) -> Result<u64> {
        todo!()
    }
    fn remove(&self, quads: &mut Vec<[TermId; 4]>) -> Result<u64> {
        todo!()
    }
    fn clear(&self, target: GraphTarget) -> Result<u64> {
        todo!()
    }
    fn create_graph(&self, g: TermId) -> Result<bool> {
        todo!()
    }
    fn drop_graph(&self, g: TermId) -> Result<bool> {
        todo!()
    }
}
