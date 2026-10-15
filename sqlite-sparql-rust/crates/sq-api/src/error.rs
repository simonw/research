//! Mapping of `sq_core::Error` onto SQLite errors.
//!
//! Every failure surfaces as a SQL error whose message is the `sq_core::Error` text, which
//! always starts with `"sparql: "`. Virtual-table methods also return a matching result
//! code: `SQLITE_INTERRUPT` for interrupts, the original code for SQLite failures
//! (`SQLITE_BUSY`, `SQLITE_LOCKED`, …) and `SQLITE_ERROR` otherwise. Scalar and aggregate
//! functions always fail with `SQLITE_ERROR`: rusqlite sets the code before the message, and
//! `sqlite3_result_error` resets it, so for them the message is the only carrier.
use rusqlite::ffi;
use sq_core::Error;

/// Converts an engine error into the SQLite error reported by a function or vtab method.
pub(crate) fn sql_error(e: Error) -> rusqlite::Error {
    let code = match &e {
        Error::Interrupted => ffi::SQLITE_INTERRUPT,
        Error::Sqlite { code, .. } if *code != ffi::SQLITE_OK => *code,
        _ => ffi::SQLITE_ERROR,
    };
    rusqlite::Error::SqliteFailure(ffi::Error::new(code), Some(e.to_string()))
}

/// `sq_core::Result` → `rusqlite::Result`.
pub(crate) trait IntoSql<T> {
    fn into_sql(self) -> rusqlite::Result<T>;
}

impl<T> IntoSql<T> for sq_core::Result<T> {
    #[inline]
    fn into_sql(self) -> rusqlite::Result<T> {
        self.map_err(sql_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(e: Error) -> (i32, String) {
        match sql_error(e) {
            rusqlite::Error::SqliteFailure(f, Some(m)) => (f.extended_code, m),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn codes_and_messages() {
        assert_eq!(code(Error::Interrupted), (ffi::SQLITE_INTERRUPT, "sparql: interrupted".into()));
        let (c, m) = code(Error::Sqlite { code: ffi::SQLITE_BUSY, msg: "database is locked".into() });
        assert_eq!(c, ffi::SQLITE_BUSY);
        assert!(m.starts_with("sparql: SQLite error 5"), "{m}");
        let (c, m) = code(Error::Syntax("bad".into()));
        assert_eq!((c, m.as_str()), (ffi::SQLITE_ERROR, "sparql: syntax error: bad"));
        assert_eq!(code(Error::Sqlite { code: 0, msg: String::new() }).0, ffi::SQLITE_ERROR);
    }
}
