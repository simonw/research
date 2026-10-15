use rusqlite::{ffi, Connection};
use std::os::raw::{c_char, c_int};

// libsqlite3-sys 0.38.2 uses its bindgen_3.34.1_ext bindings for loadable_extension (3034001).
// If "bundled"/"modern_sqlite" leaks in, the bindings become 3.53.x and extension_init2 aborts the host.
const _: () = assert!(ffi::SQLITE_VERSION_NUMBER <= 3_045_000, "rusqlite 'bundled'/'modern_sqlite' leaked into the loadable extension build");

fn init(conn: Connection) -> rusqlite::Result<bool> {
    sq_api::register(&conn)?;
    Ok(true) // SQLITE_OK_LOAD_PERMANENTLY: never dlclose a Rust cdylib
}

/// # Safety
/// Called by SQLite's extension loader with a valid connection and API routines table.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_sparqlite_init(db: *mut ffi::sqlite3, pz_err_msg: *mut *mut c_char, p_api: *mut ffi::sqlite3_api_routines) -> c_int {
    std::panic::catch_unwind(|| unsafe { Connection::extension_init2(db, pz_err_msg, p_api, init) }).unwrap_or(ffi::SQLITE_ERROR)
}
