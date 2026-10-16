//! Thin wrappers over the SQLite C API.
//!
//! Only functions on the §1.6 allow-list (all present in libsqlite3-sys'
//! `bindgen_3.34.1_ext.rs`) are called, always through `rusqlite::ffi`, so this file compiles
//! unchanged in linked mode and in loadable-extension mode (where every `ffi::` call is routed
//! through the host's `sqlite3_api_routines` table).
use std::ffi::{c_char, c_int, CStr};
use std::ptr::{self, NonNull};

use rusqlite::ffi;
use sq_core::{Error, Result};

/// Non-owning handle to the connection's `sqlite3*` (Copy). Never closed by us.
#[derive(Copy, Clone, Debug)]
pub struct RawDb(NonNull<ffi::sqlite3>);

const MAIN: &CStr = c"main";

impl RawDb {
    /// # Safety
    /// `p` must be a valid open connection that outlives every use of this handle.
    pub unsafe fn from_ptr(p: *mut ffi::sqlite3) -> Self {
        RawDb(NonNull::new(p).expect("null sqlite3*"))
    }

    pub fn as_ptr(&self) -> *mut ffi::sqlite3 {
        self.0.as_ptr()
    }

    /// `sqlite3_txn_state(db, "main")`: `SQLITE_TXN_NONE` (0), `_READ` (1) or `_WRITE` (2).
    pub fn txn_state(&self) -> i32 {
        // SAFETY: valid connection (constructor contract); "main" is NUL-terminated.
        unsafe { ffi::sqlite3_txn_state(self.as_ptr(), MAIN.as_ptr()) }
    }

    pub fn autocommit(&self) -> bool {
        // SAFETY: valid connection.
        unsafe { ffi::sqlite3_get_autocommit(self.as_ptr()) != 0 }
    }

    /// Runs every statement of `sql` (one-shot, not pooled). For DDL and maintenance.
    /// `Error::Invalid` for text containing a NUL byte (SQLite stops parsing there).
    pub fn exec(&self, sql: &str) -> Result<()> {
        if sql.contains('\0') {
            return Err(Error::Invalid("SQL text contains a NUL byte".into()));
        }
        let mut rest = sql;
        while !rest.trim().is_empty() {
            let (stmt, tail) = RawStmt::prepare_with_tail(*self, rest, false)?;
            match stmt {
                Some(mut stmt) => while stmt.step()? {},
                // Without a statement SQLite consumed only comments / `;`; it must progress.
                None if tail.len() == rest.len() => {
                    return Err(Error::Invalid(format!("cannot parse SQL text: {rest}")));
                }
                None => {}
            }
            rest = tail;
        }
        Ok(())
    }

    /// Rows changed by the most recent INSERT/UPDATE/DELETE (32-bit API: allow-listed).
    pub fn changes(&self) -> i64 {
        // SAFETY: valid connection.
        unsafe { ffi::sqlite3_changes(self.as_ptr()) as i64 }
    }

    /// `sqlite3_interrupt`: running statements return `SQLITE_INTERRUPT` (→ `Error::Interrupted`).
    pub fn interrupt(&self) {
        // SAFETY: valid connection; sqlite3_interrupt is safe to call from any thread.
        unsafe { ffi::sqlite3_interrupt(self.as_ptr()) }
    }

    /// File name of the `main` database; `None` for in-memory / temporary databases.
    pub fn filename(&self) -> Option<String> {
        // SAFETY: valid connection; the returned string is owned by SQLite and copied here.
        let p = unsafe { ffi::sqlite3_db_filename(self.as_ptr(), MAIN.as_ptr()) };
        if p.is_null() {
            return None;
        }
        let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
        (!s.is_empty()).then_some(s)
    }

    fn errmsg(&self) -> String {
        // SAFETY: valid connection; sqlite3_errmsg never returns NULL for a valid handle.
        unsafe {
            let p = ffi::sqlite3_errmsg(self.as_ptr());
            if p.is_null() { String::new() } else { CStr::from_ptr(p).to_string_lossy().into_owned() }
        }
    }

    /// Maps a non-OK result code to an `Error`. `SQLITE_INTERRUPT` becomes `Error::Interrupted`.
    pub(crate) fn error(&self, rc: c_int) -> Error {
        if rc & 0xff == ffi::SQLITE_INTERRUPT {
            Error::Interrupted
        } else {
            Error::Sqlite { code: rc, msg: self.errmsg() }
        }
    }
}

/// An owned prepared statement; finalized on drop.
///
/// `step`, `reset` and the binders take `&mut self` so column slices borrowed from the current
/// row cannot outlive it.
pub(crate) struct RawStmt {
    ptr: NonNull<ffi::sqlite3_stmt>,
    db: RawDb,
}

impl RawStmt {
    /// Prepares exactly one statement. `persistent` sets `SQLITE_PREPARE_PERSISTENT` (pooled
    /// statements). Trailing text other than whitespace / `;` is an error.
    pub fn prepare(db: RawDb, sql: &str, persistent: bool) -> Result<RawStmt> {
        let (stmt, tail) = Self::prepare_with_tail(db, sql, persistent)?;
        if !tail.trim_start_matches(|c: char| c.is_whitespace() || c == ';').is_empty() {
            return Err(Error::Invalid(format!("expected a single SQL statement: {sql}")));
        }
        stmt.ok_or_else(|| Error::Invalid("empty SQL statement".into()))
    }

    /// Prepares the first statement of `sql`; returns it (None for comments / whitespace only)
    /// and the unparsed tail.
    fn prepare_with_tail(db: RawDb, sql: &str, persistent: bool) -> Result<(Option<RawStmt>, &str)> {
        let len = c_int::try_from(sql.len()).map_err(|_| Error::Invalid("SQL text too long".into()))?;
        let flags = if persistent { ffi::SQLITE_PREPARE_PERSISTENT } else { 0 };
        let mut stmt: *mut ffi::sqlite3_stmt = ptr::null_mut();
        let mut tail: *const c_char = ptr::null();
        // SAFETY: `sql` is valid for `len` bytes (no NUL terminator needed when nByte >= 0);
        // the out-pointers are valid locals.
        let rc = unsafe { ffi::sqlite3_prepare_v3(db.as_ptr(), sql.as_ptr().cast(), len, flags, &mut stmt, &mut tail) };
        if rc != ffi::SQLITE_OK {
            return Err(db.error(rc));
        }
        let consumed = if tail.is_null() { sql.len() } else { (tail as usize) - (sql.as_ptr() as usize) };
        let rest = sql.get(consumed..).unwrap_or("");
        Ok((NonNull::new(stmt).map(|ptr| RawStmt { ptr, db }), rest))
    }

    #[inline]
    fn raw(&self) -> *mut ffi::sqlite3_stmt {
        self.ptr.as_ptr()
    }

    #[inline]
    fn check(&self, rc: c_int) -> Result<()> {
        if rc == ffi::SQLITE_OK { Ok(()) } else { Err(self.db.error(rc)) }
    }

    #[inline]
    pub fn bind_i64(&mut self, idx: c_int, v: i64) -> Result<()> {
        // SAFETY: live statement; SQLite range-checks `idx`.
        self.check(unsafe { ffi::sqlite3_bind_int64(self.raw(), idx, v) })
    }

    pub fn bind_text(&mut self, idx: c_int, s: &str) -> Result<()> {
        let len = c_int::try_from(s.len()).map_err(|_| Error::Invalid("text too long".into()))?;
        // SAFETY: SQLITE_TRANSIENT makes SQLite copy the bytes before returning.
        self.check(unsafe { ffi::sqlite3_bind_text(self.raw(), idx, s.as_ptr().cast(), len, ffi::SQLITE_TRANSIENT()) })
    }

    pub fn bind_blob(&mut self, idx: c_int, b: &[u8]) -> Result<()> {
        let len = c_int::try_from(b.len()).map_err(|_| Error::Invalid("blob too long".into()))?;
        // SAFETY: SQLITE_TRANSIENT makes SQLite copy the bytes before returning.
        self.check(unsafe { ffi::sqlite3_bind_blob(self.raw(), idx, b.as_ptr().cast(), len, ffi::SQLITE_TRANSIENT()) })
    }

    pub fn bind_null(&mut self, idx: c_int) -> Result<()> {
        // SAFETY: live statement.
        self.check(unsafe { ffi::sqlite3_bind_null(self.raw(), idx) })
    }

    pub fn bind_opt_i64(&mut self, idx: c_int, v: Option<i64>) -> Result<()> {
        match v {
            Some(v) => self.bind_i64(idx, v),
            None => self.bind_null(idx),
        }
    }

    pub fn bind_opt_text(&mut self, idx: c_int, v: Option<&str>) -> Result<()> {
        match v {
            Some(v) => self.bind_text(idx, v),
            None => self.bind_null(idx),
        }
    }

    /// `Ok(true)` = a row is available, `Ok(false)` = done. Must not be called again after
    /// `Ok(false)` without a `reset` (SQLite would silently restart the statement).
    #[inline]
    pub fn step(&mut self) -> Result<bool> {
        // SAFETY: live statement.
        match unsafe { ffi::sqlite3_step(self.raw()) } {
            ffi::SQLITE_ROW => Ok(true),
            ffi::SQLITE_DONE => Ok(false),
            rc => Err(self.db.error(rc)),
        }
    }

    /// Steps a statement that returns no rows (or whose rows are irrelevant) to completion.
    pub fn run(&mut self) -> Result<()> {
        while self.step()? {}
        Ok(())
    }

    #[inline]
    pub fn col_i64(&self, i: c_int) -> i64 {
        // SAFETY: live statement positioned on a row; out-of-range columns return 0.
        unsafe { ffi::sqlite3_column_int64(self.raw(), i) }
    }

    #[inline]
    pub fn col_is_null(&self, i: c_int) -> bool {
        // SAFETY: as above.
        unsafe { ffi::sqlite3_column_type(self.raw(), i) == ffi::SQLITE_NULL }
    }

    pub fn col_opt_i64(&self, i: c_int) -> Option<i64> {
        (!self.col_is_null(i)).then(|| self.col_i64(i))
    }

    /// Bytes of a TEXT/BLOB column (empty for NULL). Valid until the next step/reset.
    pub fn col_bytes(&self, i: c_int) -> &[u8] {
        // SAFETY: column_blob/column_bytes describe a buffer owned by the statement that stays
        // valid until the next step/reset/finalize, all of which need `&mut self`.
        unsafe {
            let p = ffi::sqlite3_column_blob(self.raw(), i);
            let n = ffi::sqlite3_column_bytes(self.raw(), i);
            if p.is_null() || n <= 0 { &[] } else { std::slice::from_raw_parts(p.cast::<u8>(), n as usize) }
        }
    }

    /// UTF-8 text of a column; `Error::Internal` if SQLite holds invalid UTF-8.
    pub fn col_str(&self, i: c_int) -> Result<&str> {
        std::str::from_utf8(self.col_bytes(i)).map_err(|_| Error::internal("invalid UTF-8 in rdf_term"))
    }

    pub fn col_opt_str(&self, i: c_int) -> Result<Option<&str>> {
        if self.col_is_null(i) { Ok(None) } else { self.col_str(i).map(Some) }
    }

    pub fn column_count(&self) -> usize {
        // SAFETY: live statement.
        unsafe { ffi::sqlite3_column_count(self.raw()) as usize }
    }

    /// Resets to the start; bindings are kept. The result code repeats the last step error,
    /// which was already reported by `step`, so it is ignored here.
    #[inline]
    pub fn reset(&mut self) {
        // SAFETY: live statement.
        unsafe { ffi::sqlite3_reset(self.raw()) };
    }

    pub fn clear_bindings(&mut self) {
        // SAFETY: live statement.
        unsafe { ffi::sqlite3_clear_bindings(self.raw()) };
    }
}

impl Drop for RawStmt {
    fn drop(&mut self) {
        // SAFETY: we own the statement; finalize exactly once.
        unsafe { ffi::sqlite3_finalize(self.raw()) };
    }
}
