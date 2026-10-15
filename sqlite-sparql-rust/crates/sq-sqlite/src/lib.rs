//! sq-sqlite: SQLite storage for sparqlite (docs/SPEC.md §2, §3, §4.3–§4.4, §5.2, §8.6).
//!
//! Modules, by concern:
//! * `raw`, `pool`, `template` — allow-listed C API calls (§1.6), the persistent statement
//!   pool and the SQL text of every pooled statement (§5.2 templates keyed by `StmtKey`);
//! * `cursor` — [`SqlCursor`] (index range scans over the free suffix columns, seek, rebind)
//!   and [`SqlRowsCursor`] (the SqlJoin hybrid's generated SQL);
//! * `schema` — the §2.1 DDL and schema-state detection;
//! * `shared`, `cache`, `keys`, `dict` — per-connection state: the DecodeCache / LookupCache
//!   (CLOCK), the §4.4 epoch check and committed-row admission rule, dictionary lookup and
//!   decode paths (§4.3);
//! * `scope` — [`ReadScope`] / [`WriteScope`] savepoint scopes (§8.6);
//! * `writer`, `loader`, `analyze`, `maint` — `QuadStoreWriter`, the §2.3 bulk loader, §2.4
//!   statistics, `vacuum_terms` / `check` / `explain_query_plan`;
//! * `btree` — the phase-2 direct B-tree reader (§13): [`BtreeCursor`] reads the covering
//!   index pages of `rdf_quad` itself when the database is a rollback-journal file and a read
//!   transaction is held, and falls back to a `SqlCursor` transparently otherwise.
//!
//! All state lives in one [`StorageShared`] per connection; [`SqliteStore`] is the cheap,
//! clonable `QuadStore` / `QuadStoreWriter` / `Dictionary` handle over it. The connection is
//! never owned or closed here: callers pass its `sqlite3*` through [`RawDb::from_ptr`], arm
//! the statement pool while their close hook is connected ([`StorageShared::arm_pool`]) and
//! call [`StorageShared::validate_epoch`] once per API entry inside a read scope. A read scope
//! whose savepoint started the transaction commits nested writes when it ends: callers that
//! write inside one end it with [`ReadScope::close`] to learn whether that commit succeeded.
//! Scopes never leave a transaction open behind them, not even after `sqlite3_interrupt`
//! (see the `scope` module).

mod analyze;
mod btree;
mod cache;
mod cursor;
mod dict;
mod keys;
mod loader;
mod maint;
mod pool;
mod raw;
mod schema;
mod scope;
mod shared;
mod store;
mod template;
mod writer;

pub use btree::{BtreeCounters, BtreeCursor, DEFAULT_PAGE_CACHE_MB, ReaderMode};
pub use cursor::{SqlCursor, SqlRowsCursor};
pub use loader::{BulkMode, LoadOptions, LoadReport, LoadSource};
pub use pool::PoolCounters;
pub use raw::RawDb;
pub use schema::SchemaState;
pub use scope::{ReadScope, WriteScope};
pub use shared::{DictCounters, StorageConfig, StorageShared};
pub use store::SqliteStore;

#[cfg(test)]
mod tests;
