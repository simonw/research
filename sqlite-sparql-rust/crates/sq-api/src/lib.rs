//! sq-api: the SQL surface of sparqlite (docs/SPEC.md §8).
//!
//! [`register`] installs, on one SQLite connection,
//! * the scalar and aggregate functions of §8.2 (`functions`, `helpers`): queries, updates,
//!   loads, EXPLAIN, configuration, term accessors, and the helper functions the baseline
//!   translator's SQL calls;
//! * the virtual tables of §8.3 (`vtab`): the `sparql_rows` table-valued function, the
//!   `sparql_view` module and the `sparql_status` table, which doubles as the connection's
//!   close hook (§8.7);
//! * one per-connection state ([`state::ConnState`]): the storage layer, configuration, the
//!   parsed-query cache, open-cursor accounting and the entry protocol (read / write scopes,
//!   epoch validation, rules R1–R7 of §8.6).
//!
//! The same code runs in the loadable extension (`ext/`, rusqlite `loadable_extension`) and
//! in native tests over the system libsqlite3: SQLite is reached only through `rusqlite`
//! and the §1.6 allow-list.
//!
//! Module map: `state` (ConnState, entries), `config` (§8.5 settings), `lru` (parsed queries),
//! `query` (native and baseline runs, output), `helpers` (baseline helper functions),
//! `functions` (registration and bodies), `vtab` (`sparql_rows`, `sparql_view`,
//! `sparql_status`), `value` (native mapping, N-Triples), `guard` (panic containment),
//! `error` (error mapping).
use rusqlite::Connection;

mod config;
mod error;
mod functions;
mod guard;
mod helpers;
mod lru;
mod query;
mod state;
mod value;
mod vtab;

/// `sparql_version()` text: `sparqlite <crate version> (SQLite <library version>)`.
pub fn version_string(sqlite_version: &str) -> String {
    format!("sparqlite {} (SQLite {})", env!("CARGO_PKG_VERSION"), sqlite_version)
}

/// Registers every sparqlite function and module on `conn` (§8.1). Call once per
/// connection; every pooled connection must load the extension.
pub fn register(conn: &Connection) -> rusqlite::Result<()> {
    // SAFETY: the state is owned by the functions and modules registered on this
    // connection below; SQLite destroys them (and with them the last reference to the
    // state) while it closes the connection, so the handle outlives every use.
    let st = unsafe { state::ConnState::new(conn.handle()) };
    functions::register(conn, &st)?;
    vtab::register(conn, &st)?;
    // The close hook: connecting the eponymous `sparql_status` instance arms the pool.
    conn.prepare("SELECT 1 FROM main.sparql_status")?;
    Ok(())
}
