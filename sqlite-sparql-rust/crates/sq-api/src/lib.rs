//! sq-api: `register(&Connection)` — SQL functions, virtual tables (`sparql_rows`,
//! `sparql_view`, `sparql_status` + close hook), ConnState, `guard()`. Used by the cdylib
//! (ext/) and by native tests (system libsqlite3). See docs/SPEC.md §8.
use rusqlite::functions::FunctionFlags;
use rusqlite::Connection;

pub fn version_string(sqlite_version: &str) -> String {
    format!("sparqlite {} (SQLite {})", env!("CARGO_PKG_VERSION"), sqlite_version)
}

pub fn register(conn: &Connection) -> rusqlite::Result<()> {
    conn.create_scalar_function("sparql_version", 0, FunctionFlags::SQLITE_UTF8, |_ctx| {
        let v = unsafe { std::ffi::CStr::from_ptr(rusqlite::ffi::sqlite3_libversion()) };
        Ok(version_string(&v.to_string_lossy()))
    })?;
    Ok(())
}
