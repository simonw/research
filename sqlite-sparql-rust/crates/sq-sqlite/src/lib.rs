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
//!   statistics, `vacuum_terms` / `check` / `explain_query_plan`.
//!
//! All state lives in one [`StorageShared`] per connection; [`SqliteStore`] is the cheap,
//! clonable `QuadStore` / `QuadStoreWriter` / `Dictionary` handle over it. The connection is
//! never owned or closed here: callers pass its `sqlite3*` through [`RawDb::from_ptr`], arm
//! the statement pool while their close hook is connected ([`StorageShared::arm_pool`]) and
//! call [`StorageShared::validate_epoch`] once per API entry inside a read scope.

mod analyze;
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
