//! `SqliteStore`: the `QuadStore` / `QuadStoreWriter` / `Dictionary` over one connection.
use std::rc::Rc;
use std::sync::Arc;

use sq_core::stats::{CostConstants, Stats, SQL_CURSOR_COSTS};
use sq_core::store::*;
use sq_core::term::Term;
use sq_core::{Error, Result, TermId};

use crate::cursor::{bind_count, EmptyCursor, SqlCursor, SqlRowsCursor};
use crate::loader::{LoadOptions, LoadReport, LoadSource};
use crate::pool::PoolKey;
use crate::schema::{self, SchemaState};
use crate::scope::{ReadScope, WriteScope};
use crate::shared::StorageShared;
use crate::template::{StmtKey, StmtKind};

/// Cheap to clone; clones share the connection state.
#[derive(Clone)]
pub struct SqliteStore(pub Rc<StorageShared>);

impl SqliteStore {
    pub fn new(shared: Rc<StorageShared>) -> Self {
        SqliteStore(shared)
    }

    pub fn shared(&self) -> &Rc<StorageShared> {
        &self.0
    }

    /// `sparql_init()`: idempotent §2.1 DDL in one write scope; returns schema_version.
    /// `Error::Schema` if the database holds an incompatible RDF schema.
    pub fn ensure_schema(&self) -> Result<i64> {
        // Fresh detection (not the cached state, which may predate other connections' DDL);
        // the cache itself is only reset when DDL actually runs.
        let detected = schema::detect(self.0.db())?;
        match detected.state {
            SchemaState::Incompatible(m) => return Err(Error::Schema(m)),
            SchemaState::Ready { schema_version } if detected.complete => return Ok(schema_version),
            _ => {}
        }
        self.0.forget_schema();
        let scope = self.write_scope(0)?;
        schema::create(&self.0)?;
        scope.commit()?;
        self.0.forget_schema();
        match self.0.schema_state()? {
            SchemaState::Ready { schema_version } => Ok(schema_version),
            SchemaState::Incompatible(m) => Err(Error::Schema(m)),
            SchemaState::Absent => Err(Error::internal("schema still absent after sparql_init")),
        }
    }

    /// RAII read scope (§8.6). `depth` is the caller's nesting depth.
    pub fn read_scope(&self, depth: u32) -> Result<ReadScope> {
        ReadScope::open(&self.0, depth)
    }

    /// RAII write scope (§8.6): `commit()`, or roll back on drop.
    pub fn write_scope(&self, depth: u32) -> Result<WriteScope> {
        WriteScope::open(&self.0, depth)
    }

    /// §2.3 bulk loader (auto-initializes the schema).
    pub fn bulk_load(&self, src: LoadSource<'_>, opts: &LoadOptions) -> Result<LoadReport> {
        crate::loader::bulk_load(self, src, opts)
    }

    /// §2.4: recomputes and persists the statistics, bumps `stats_gen`.
    pub fn analyze(&self) -> Result<Arc<Stats>> {
        self.0.require_ready()?;
        let scope = self.write_scope(0)?;
        let (generation, stats) = crate::analyze::analyze_in_scope(&self.0)?;
        scope.commit()?;
        self.0.set_stats(generation, stats.clone());
        Ok(stats)
    }

    /// `sparql_vacuum_terms()`: deletes unreferenced dictionary rows; bumps `dict_gen`.
    pub fn vacuum_terms(&self) -> Result<u64> {
        self.0.require_ready()?;
        let scope = self.write_scope(0)?;
        let n = crate::maint::vacuum_terms(&self.0)?;
        scope.commit()?;
        Ok(n)
    }

    /// `sparql_check()`: verifies the storage invariants; "ok …" or a list of problems.
    pub fn check(&self) -> Result<String> {
        self.0.require_ready()?;
        let _scope = self.read_scope(0)?;
        crate::maint::check(&self.0)
    }

    /// `EXPLAIN QUERY PLAN` of `sql`, one indented line per plan node.
    pub fn explain_query_plan(&self, sql: &str) -> Result<String> {
        crate::maint::explain_query_plan(self.0.db(), sql)
    }
}

impl Dictionary for SqliteStore {
    fn lookup(&self, t: oxrdf::TermRef<'_>) -> Result<Option<TermId>> {
        self.0.lookup_term(t)
    }

    fn decode(&self, ids: &[TermId], out: &mut Vec<Arc<Term>>) -> Result<()> {
        self.0.decode_ids(ids, out)
    }
}

impl QuadStore for SqliteStore {
    fn cursor(&self, spec: &ScanSpec) -> Result<Box<dyn QuadCursor>> {
        if spec.prefix_len > 4 {
            return Err(Error::Invalid(format!("prefix_len {} > 4", spec.prefix_len)));
        }
        if !self.0.tables_present()? {
            return Ok(Box::new(EmptyCursor { width: 4 - spec.prefix_len as usize }));
        }
        Ok(Box::new(SqlCursor::open(self.0.pool.clone(), spec)?))
    }

    fn count(&self, spec: &ScanSpec, cap: Option<u64>) -> Result<u64> {
        if spec.prefix_len > 4 {
            return Err(Error::Invalid(format!("prefix_len {} > 4", spec.prefix_len)));
        }
        if !self.0.tables_present()? {
            return Ok(0);
        }
        let key = StmtKey::for_spec(spec, if cap.is_some() { StmtKind::CountCapped } else { StmtKind::Count });
        let mut st = self.0.pool.lease(PoolKey::Scan(key))?;
        if !bind_count(&mut st, &key, spec, cap)? {
            return Ok(0);
        }
        st.step()?;
        Ok(st.col_i64(0).max(0) as u64)
    }

    fn dict(&self) -> &dyn Dictionary {
        self
    }

    fn named_graphs(&self) -> Result<Vec<TermId>> {
        self.0.named_graph_ids()
    }

    fn stats(&self) -> Option<Arc<Stats>> {
        self.0.stats()
    }

    fn costs(&self) -> &CostConstants {
        &SQL_CURSOR_COSTS
    }

    fn caps(&self) -> StoreCaps {
        StoreCaps { sql: true, reader: ReaderKind::SqlCursor }
    }

    fn sql_rows(&self, sql: &str, params: &[i64], width: usize) -> Result<Box<dyn QuadCursor>> {
        Ok(Box::new(SqlRowsCursor::open(&self.0.pool, sql, params, width)?))
    }

    fn check_interrupt(&self) -> Result<()> {
        self.0.check_interrupt()
    }
}

impl QuadStoreWriter for SqliteStore {
    fn intern(&self, t: oxrdf::TermRef<'_>) -> Result<TermId> {
        self.0.intern_term(t)
    }

    fn fresh_bnode(&self) -> Result<TermId> {
        self.0.fresh_bnode_id()
    }

    fn insert(&self, quads: &mut Vec<[TermId; 4]>) -> Result<u64> {
        self.0.insert_quads(quads)
    }

    fn remove(&self, quads: &mut Vec<[TermId; 4]>) -> Result<u64> {
        self.0.remove_quads(quads)
    }

    fn clear(&self, target: GraphTarget) -> Result<u64> {
        self.0.clear_quads(target)
    }

    fn create_graph(&self, g: TermId) -> Result<bool> {
        self.0.create_graph_id(g)
    }

    fn drop_graph(&self, g: TermId) -> Result<bool> {
        self.0.drop_graph_id(g)
    }
}
