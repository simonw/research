//! Phase 2: the direct B-tree reader (docs/SPEC.md §13).
//!
//! A [`BtreeCursor`] serves the same `QuadCursor` contract as `SqlCursor`. It reads the
//! covering-index b-tree pages of `rdf_quad` itself instead of stepping a prepared statement
//! through SQLite's VDBE. The pages are those of the WITHOUT ROWID table's primary key for
//! GSPO and of the secondary indexes for the other orders, the partial ones included. The
//! reader decodes only the free suffix columns of each record into the output buffer.
//!
//! Modules, by concern:
//! * `file`: the only unsafe code. It obtains the main database's `sqlite3_file*` with
//!   `SQLITE_FCNTL_FILE_POINTER`, reads pages through the VFS `xRead` (so no second file
//!   descriptor is opened), and reads `SQLITE_FCNTL_DATA_VERSION`;
//! * `format`: the file format (database header, index b-tree pages, varints, records with 4
//!   integer columns). It treats every byte as untrusted;
//! * `roots`: root pages from `sqlite_schema`, with each b-tree's column layout and
//!   partial-index predicate verified before it is read directly;
//! * `reader`: per-connection state. It holds the §13 preconditions, the validated snapshot,
//!   the CLOCK page cache (`page_cache_mb`) and the reader mode (`auto` | `sql` | `btree`);
//! * `cursor`: [`BtreeCursor`], with leaf pinning, finger seeks, and the transparent fallback
//!   to a `SqlCursor` at the row-value resume key.
mod cursor;
mod file;
mod format;
mod reader;
mod roots;

pub use cursor::BtreeCursor;
pub use reader::{BtreeCounters, DEFAULT_PAGE_CACHE_MB, ReaderMode};
pub(crate) use reader::BtreeReader;

/// Why the reader cannot serve a scan, or can no longer serve it. The scan then continues as
/// a `SqlCursor`, so callers never see a difference.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Fallback {
    /// A §13 precondition does not hold: not a rollback-journal file database, no read
    /// transaction, an open write transaction, or a changed data version. This is transient:
    /// a later `rebind` returns to the reader once the preconditions hold again.
    Precondition(&'static str),
    /// A page or record feature the reader does not implement: an overflow payload, a
    /// non-integer column, or an unexpected b-tree layout.
    Unsupported(&'static str),
    /// An inconsistent page: a bad page type, a pointer out of range, or keys out of order.
    /// SQLite reports the corruption itself once the SQL cursor reaches it.
    Corrupt(&'static str),
}

impl Fallback {
    /// Precondition failures can clear; unsupported or corrupt structures cannot.
    pub fn is_transient(&self) -> bool {
        matches!(self, Fallback::Precondition(_))
    }
}

impl std::fmt::Display for Fallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Fallback::Precondition(why) => write!(f, "precondition: {why}"),
            Fallback::Unsupported(why) => write!(f, "unsupported: {why}"),
            Fallback::Corrupt(why) => write!(f, "corrupt: {why}"),
        }
    }
}
