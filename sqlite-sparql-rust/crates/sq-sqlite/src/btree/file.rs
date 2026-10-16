//! The reader's only unsafe code: two `sqlite3_file_control` opcodes from the §1.6 allow-list
//! (`SQLITE_FCNTL_FILE_POINTER`, `SQLITE_FCNTL_DATA_VERSION`) and the VFS `xRead` method of the
//! main database file.
//!
//! No `sqlite3_file*` is ever kept. Every read asks SQLite for the pointer again, checks it
//! against the identity validated for the snapshot, and uses it before control returns to
//! SQLite's caller. The pointer is therefore only dereferenced while SQLite guarantees that it
//! is valid. Because reads go through the connection's own file object, VFS shims (encryption,
//! checksums, in-memory VFSes) see every read; no second file descriptor is opened.
use std::ffi::{CStr, c_int, c_uint, c_void};
use std::ptr;

use rusqlite::ffi;

use crate::raw::RawDb;

const MAIN: &CStr = c"main";

/// Identity of the main database's `sqlite3_file`: its address and the address of its method
/// table (`pMethods`). It is compared, never dereferenced. The method table tells apart files
/// of different VFSes that happen to be allocated at the same address (ABA: the main
/// database replaced by `sqlite3_deserialize` can land on the freed `sqlite3_file` of the
/// previous one).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) struct FileId(usize, usize);

impl FileId {
    fn of(file: *mut ffi::sqlite3_file) -> FileId {
        // SAFETY: `file` was just returned by SQLite for this connection's main database and
        // nothing between that call and this read calls back into SQLite, so it is live; only
        // the `pMethods` pointer value is read, never what it points to.
        let methods = unsafe { (*file).pMethods };
        FileId(file as usize, methods as usize)
    }
}

/// Why a page read failed.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum ReadError {
    /// SQLite reports a different main-database file (or none) than the snapshot's.
    FileChanged,
    /// The file is not open, or its VFS has no `xRead`.
    NoRead,
    /// The read went past the end of the file (`SQLITE_IOERR_SHORT_READ`).
    ShortRead,
    /// Any other VFS result code.
    Io(c_int),
}

/// `SQLITE_FCNTL_FILE_POINTER` on `main`.
fn file_pointer(db: RawDb) -> Option<*mut ffi::sqlite3_file> {
    let mut file: *mut ffi::sqlite3_file = ptr::null_mut();
    // SAFETY: `db` is a valid open connection (the `RawDb` contract), "main" is NUL-terminated,
    // and for SQLITE_FCNTL_FILE_POINTER SQLite writes exactly one `sqlite3_file*` through the
    // argument, which points to a live local of that type.
    let rc = unsafe {
        ffi::sqlite3_file_control(
            db.as_ptr(),
            MAIN.as_ptr(),
            ffi::SQLITE_FCNTL_FILE_POINTER,
            (&raw mut file).cast::<c_void>(),
        )
    };
    (rc == ffi::SQLITE_OK && !file.is_null()).then_some(file)
}

/// Identity of the main database file, `None` if SQLite reports none.
pub(crate) fn identity(db: RawDb) -> Option<FileId> {
    file_pointer(db).map(FileId::of)
}

/// `SQLITE_FCNTL_DATA_VERSION` on `main`. It changes whenever the content of the database
/// changes, whether through this connection's commits or through other connections' commits
/// (seen when this connection next takes its read lock).
pub(crate) fn data_version(db: RawDb) -> Option<u32> {
    let mut version: c_uint = 0;
    // SAFETY: as in `file_pointer`; for SQLITE_FCNTL_DATA_VERSION SQLite writes exactly one
    // `unsigned int` through the argument.
    let rc = unsafe {
        ffi::sqlite3_file_control(
            db.as_ptr(),
            MAIN.as_ptr(),
            ffi::SQLITE_FCNTL_DATA_VERSION,
            (&raw mut version).cast::<c_void>(),
        )
    };
    (rc == ffi::SQLITE_OK).then_some(version)
}

/// Reads `buf.len()` bytes at `offset` of the main database file through its VFS, provided
/// SQLite still reports the file identified by `expected`.
pub(crate) fn read_at(db: RawDb, expected: FileId, buf: &mut [u8], offset: u64) -> Result<(), ReadError> {
    let file = file_pointer(db).ok_or(ReadError::FileChanged)?;
    if FileId::of(file) != expected {
        return Err(ReadError::FileChanged);
    }
    let len = c_int::try_from(buf.len()).map_err(|_| ReadError::Io(ffi::SQLITE_IOERR))?;
    let offset = i64::try_from(offset).map_err(|_| ReadError::Io(ffi::SQLITE_IOERR))?;
    // SAFETY: SQLite returned `file` above for this connection's main database, and nothing
    // between that call and this use calls back into SQLite, so it is still live. Its
    // `pMethods` is either NULL (file not open) or points to the VFS's method table, which
    // lives as long as the file.
    let x_read = unsafe { (*file).pMethods.as_ref() }.and_then(|m| m.xRead).ok_or(ReadError::NoRead)?;
    // SAFETY: `file` is live (see above). xRead writes at most `len` bytes into `buf`, which is
    // valid for writes of `len` bytes. On a short read it zero-fills the rest of the buffer.
    let rc = unsafe { x_read(file, buf.as_mut_ptr().cast::<c_void>(), len, offset) };
    match rc {
        ffi::SQLITE_OK => Ok(()),
        ffi::SQLITE_IOERR_SHORT_READ => Err(ReadError::ShortRead),
        rc => Err(ReadError::Io(rc)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two files at the same address (a freed `sqlite3_file` reused by the next one, e.g.
    /// after `sqlite3_deserialize`) are still told apart by their method tables.
    #[test]
    fn file_identity_includes_the_method_table() {
        // SAFETY: an all-zero `sqlite3_io_methods` is valid (an int and `Option` fn pointers).
        let methods: ffi::sqlite3_io_methods = unsafe { std::mem::zeroed() };
        let mut file = ffi::sqlite3_file { pMethods: ptr::null() };
        let closed = FileId::of(&raw mut file);
        file.pMethods = &raw const methods;
        let open = FileId::of(&raw mut file);
        assert_ne!(closed, open);
        assert_eq!(open, FileId::of(&raw mut file));
    }
}
