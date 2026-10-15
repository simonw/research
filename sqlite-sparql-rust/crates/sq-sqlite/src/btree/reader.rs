//! Per-connection state of the reader: the §13 preconditions, the validated [`Snapshot`], the
//! page cache and the root pages.
//!
//! **Why the file can be read directly.** In rollback-journal mode, a connection that holds a
//! read transaction holds a SHARED lock on the database file. While it does, no connection can
//! write the file: writers write it only under an EXCLUSIVE lock, and that includes cache
//! spills. If this connection has no write transaction itself, the file holds exactly the
//! committed snapshot that its own statements see. In WAL mode, by contrast, committed pages
//! live in the WAL. Those are preconditions 1–3; the reader re-checks them at every cursor
//! entry point and never reads otherwise.
//!
//! **Why cached pages stay valid.** `SQLITE_FCNTL_DATA_VERSION` changes on every commit of this
//! connection, and whenever the pager discards its cache because another connection changed the
//! file (detected when the read lock is taken). The page cache belongs to one
//! (file, data version) pair and is flushed when the pair changes (precondition 4). Pages are
//! read only while preconditions 1–3 hold, so a cached page is always committed content of the
//! current version.
//!
//! **Shared cache.** Connections of one shared cache (`cache=shared`, `SQLITE_OPEN_SHAREDCACHE`,
//! `sqlite3_enable_shared_cache`) share one pager and therefore one file lock. A read
//! transaction of this connection then does not keep the others out of the file: another
//! connection of the cache can escalate the shared pager to EXCLUSIVE and spill its
//! uncommitted pages into the file, and SQLite isolates the connections only through table
//! locks that page reads bypass. The safety argument above does not hold, so the reader treats
//! a sharable `main` as a failed precondition (part of precondition 1). No allow-listed API
//! reports shared-cache mode, but SQLite codes an `OP_TableLock` into a statement exactly when
//! the b-tree it reads is sharable, and `EXPLAIN` lists the opcodes. The probe runs once per
//! main database file.
use std::cell::{Cell, RefCell};
use std::sync::Arc;

use rusqlite::ffi;
use sq_core::Result;

use super::Fallback;
use super::file::{self, FileId, ReadError};
use super::format::{DbHeader, Geometry, HEADER_LEN};
use super::roots::{self, Roots};
use crate::cache::ClockCache;
use crate::raw::{RawDb, RawStmt};

/// Which reader `SqliteStore::cursor` uses (`sparql_config('reader', …)`).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum ReaderMode {
    /// The B-tree reader whenever its preconditions hold, otherwise SQL cursors (default).
    #[default]
    Auto,
    /// Kill switch: SQL cursors only. Open B-tree cursors continue as SQL cursors at their
    /// next call, at the exact key.
    Sql,
    /// The B-tree reader, requested explicitly. It still falls back to SQL cursors when a
    /// precondition does not hold (callers never see a difference), so today it behaves
    /// like `Auto`. Only `sparql_status` tells the two apart.
    Btree,
}

impl ReaderMode {
    /// Parses `auto`, `sql` or `btree` (case-insensitive).
    pub fn parse(s: &str) -> Result<ReaderMode> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(ReaderMode::Auto),
            "sql" => Ok(ReaderMode::Sql),
            "btree" => Ok(ReaderMode::Btree),
            other => Err(sq_core::Error::Invalid(format!("reader must be auto, sql or btree, not '{other}'"))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ReaderMode::Auto => "auto",
            ReaderMode::Sql => "sql",
            ReaderMode::Btree => "btree",
        }
    }
}

/// Default page-cache budget (`page_cache_mb`).
pub const DEFAULT_PAGE_CACHE_MB: u64 = 128;

/// Reader counters (`sparql_status`, tests).
#[derive(Copy, Clone, Default, Debug)]
pub struct BtreeCounters {
    /// Cursors served by the reader (`SqliteStore::cursor` returned a B-tree cursor).
    pub cursors: u64,
    /// Pages read from the file (page-cache misses).
    pub pages_read: u64,
    /// Page-cache hits.
    pub cache_hits: u64,
    /// Page-cache flushes because the snapshot (file or data version) changed.
    pub cache_flushes: u64,
    /// Root-page loads (first use, schema cookie changes).
    pub root_loads: u64,
    /// Scans that re-opened as a `SqlCursor` (a precondition stopped holding, or an
    /// unsupported or inconsistent page or record).
    pub fallbacks: u64,
    /// Fallen-back cursors that a later `rebind` returned to the reader.
    pub resumes: u64,
}

/// A validated read snapshot: the main database file (by identity) at one data version,
/// with its page geometry and the verified root pages.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Snapshot {
    pub file: FileId,
    pub data_version: u32,
    pub geom: Geometry,
    /// Pages in the file (the in-header size), `u32::MAX` when the header does not say.
    pub page_count: u32,
    pub roots: Roots,
}

impl Snapshot {
    /// Same file and data version (everything else is derived from those).
    #[inline]
    pub fn same(&self, other: &Snapshot) -> bool {
        self.data_version == other.data_version && self.file == other.file
    }
}

struct State {
    /// The snapshot the page cache belongs to.
    current: Option<Snapshot>,
    /// The (file, data version) last rejected, with the reason (e.g. a WAL database), so
    /// that rejection costs no header read until something changes.
    rejected: Option<(FileId, u32, Fallback)>,
    /// Verified root pages by file and schema cookie.
    roots: Option<(FileId, u32, Roots)>,
    /// Whether the main database file is opened in shared-cache mode, by file.
    shared_cache: Option<(FileId, bool)>,
    /// Page number -> page, CLOCK eviction, budget `page_cache_mb`.
    cache: ClockCache<Arc<[u8]>>,
}

pub(crate) struct BtreeReader {
    db: RawDb,
    mode: Cell<ReaderMode>,
    page_cache_mb: Cell<u64>,
    state: RefCell<State>,
    counters: Cell<BtreeCounters>,
    /// Why the reader last declined to serve, or fell back (for `sparql_status`).
    last_fallback: Cell<Option<Fallback>>,
}

fn budget_bytes(mb: u64) -> usize {
    usize::try_from(mb).unwrap_or(usize::MAX).saturating_mul(1 << 20)
}

const REENTRANT: Fallback = Fallback::Precondition("re-entrant use of the reader state");
/// A schema read that failed (e.g. interrupted): transient, so it is not remembered.
const SCHEMA_UNREADABLE: Fallback = Fallback::Precondition("cannot read the schema");
const SWITCHED_OFF: Fallback = Fallback::Precondition("the reader is switched off (reader = sql)");
const SHARED_CACHE: Fallback = Fallback::Precondition("main is opened in shared-cache mode");

impl BtreeReader {
    pub fn new(db: RawDb) -> BtreeReader {
        BtreeReader {
            db,
            mode: Cell::new(ReaderMode::default()),
            page_cache_mb: Cell::new(DEFAULT_PAGE_CACHE_MB),
            state: RefCell::new(State {
                current: None,
                rejected: None,
                roots: None,
                shared_cache: None,
                cache: ClockCache::new(budget_bytes(DEFAULT_PAGE_CACHE_MB)),
            }),
            counters: Cell::new(BtreeCounters::default()),
            last_fallback: Cell::new(None),
        }
    }

    pub fn mode(&self) -> ReaderMode {
        self.mode.get()
    }

    pub fn set_mode(&self, mode: ReaderMode) {
        self.mode.set(mode);
    }

    pub fn page_cache_mb(&self) -> u64 {
        self.page_cache_mb.get()
    }

    pub fn set_page_cache_mb(&self, mb: u64) -> Result<()> {
        self.state.try_borrow_mut().map_err(|_| sq_core::Error::Reentrant)?.cache.set_budget(budget_bytes(mb));
        self.page_cache_mb.set(mb);
        Ok(())
    }

    /// Drops every cached page (cold-cache measurements, tests).
    pub fn clear_page_cache(&self) -> Result<()> {
        self.state.try_borrow_mut().map_err(|_| sq_core::Error::Reentrant)?.cache.clear();
        Ok(())
    }

    pub fn counters(&self) -> BtreeCounters {
        self.counters.get()
    }

    pub(super) fn count(&self, f: impl FnOnce(&mut BtreeCounters)) {
        let mut c = self.counters.get();
        f(&mut c);
        self.counters.set(c);
    }

    /// Records a fallback of a cursor (counted) or a declined cursor (not counted).
    pub(super) fn note(&self, why: Fallback, fallback: bool) {
        self.last_fallback.set(Some(why));
        if fallback {
            self.count(|c| c.fallbacks += 1);
        }
    }

    /// The kill switch (`reader = sql`) applies to open cursors too: their next call falls
    /// back at the exact key.
    #[inline]
    fn enabled(&self) -> Result<(), Fallback> {
        if self.mode.get() == ReaderMode::Sql { Err(SWITCHED_OFF) } else { Ok(()) }
    }

    /// Preconditions 2 and 3: a read transaction on `main`, and no write transaction.
    #[inline]
    fn transaction_ok(&self) -> Result<(), Fallback> {
        match self.db.txn_state() {
            ffi::SQLITE_TXN_READ => Ok(()),
            ffi::SQLITE_TXN_WRITE => Err(Fallback::Precondition("this connection has a write transaction open")),
            _ => Err(Fallback::Precondition("no read transaction is held on main")),
        }
    }

    /// Cheap re-check at every cursor entry point: the mode and preconditions 2–4 against the
    /// snapshot the cursor positioned with (two `sqlite3_*` calls, no I/O). Precondition 1
    /// cannot change without changing the data version: switching to WAL rewrites the header,
    /// and shared-cache mode is fixed when the connection opens.
    #[inline]
    pub fn check(&self, snap: &Snapshot) -> Result<(), Fallback> {
        self.enabled()?;
        self.transaction_ok()?;
        if file::data_version(self.db) != Some(snap.data_version) {
            return Err(Fallback::Precondition("the data version changed"));
        }
        Ok(())
    }

    /// Full validation: the mode, all four preconditions (precondition 1 including the
    /// private-cache check), the database header, and the root pages. Returns the current
    /// snapshot. When the file or the data version changed, it releases the page cache (also
    /// when the new pair is then rejected) and re-reads the header.
    pub fn snapshot(&self) -> Result<Snapshot, Fallback> {
        self.enabled()?;
        self.transaction_ok()?;
        let file = file::identity(self.db).ok_or(Fallback::Precondition("main has no database file"))?;
        let version = file::data_version(self.db).ok_or(Fallback::Precondition("no data version for main"))?;
        {
            let st = self.state.try_borrow().map_err(|_| REENTRANT)?;
            if let Some(s) = st.current.filter(|s| s.file == file && s.data_version == version) {
                return Ok(s);
            }
            if let Some((f, v, why)) = st.rejected
                && f == file
                && v == version
            {
                return Err(why);
            }
        }
        self.release_superseded()?;
        match self.validate(file, version) {
            Ok(snap) => {
                let mut st = self.state.try_borrow_mut().map_err(|_| REENTRANT)?;
                // Already empty (`release_superseded`, and `page` caches nothing while
                // `current` is `None`); kept so the new snapshot never starts with stale pages.
                st.cache.clear();
                st.current = Some(snap);
                st.rejected = None;
                Ok(snap)
            }
            Err(why) => {
                // A transient failure (e.g. an interrupted schema read) is not remembered.
                if why != SCHEMA_UNREADABLE {
                    self.state.try_borrow_mut().map_err(|_| REENTRANT)?.rejected = Some((file, version, why));
                }
                Err(why)
            }
        }
    }

    /// Drops the snapshot the page cache belongs to and releases its pages, when the
    /// (file, data version) pair changed. This happens before the new pair is validated,
    /// whatever the verdict: once the pair changed, no cursor can read the cached pages again
    /// (`page` serves only `current`, and `check` fails for every cursor of the old version,
    /// data versions only move forward). A rejected pair (e.g. the database switched to WAL)
    /// can stay current indefinitely, and the cache would otherwise keep up to
    /// `page_cache_mb` of dead pages alive until a valid snapshot came back.
    ///
    /// The cache is replaced rather than cleared, so the slot table and the page map sized
    /// for the old working set are freed too (a new `ClockCache` allocates nothing).
    fn release_superseded(&self) -> Result<(), Fallback> {
        let mut st = self.state.try_borrow_mut().map_err(|_| REENTRANT)?;
        if st.current.take().is_some() {
            self.count(|c| c.cache_flushes += 1);
        }
        st.cache = ClockCache::new(budget_bytes(self.page_cache_mb.get()));
        Ok(())
    }

    /// Precondition 1 and the header checks for a new (file, data version) pair.
    fn validate(&self, file: FileId, version: u32) -> Result<Snapshot, Fallback> {
        if self.db.filename().is_none() {
            return Err(Fallback::Precondition("main is not a file database"));
        }
        if self.shared_cache(file)? {
            return Err(SHARED_CACHE);
        }
        let mut hdr = [0u8; HEADER_LEN];
        file::read_at(self.db, file, &mut hdr, 0).map_err(read_error)?;
        let header = DbHeader::parse(&hdr)?;
        if !header.rollback_journal() {
            return Err(Fallback::Precondition("the database is in WAL mode"));
        }
        Ok(Snapshot {
            file,
            data_version: version,
            geom: header.geometry(),
            page_count: header.page_count.unwrap_or(u32::MAX),
            roots: self.roots_for(file, header.schema_cookie)?,
        })
    }

    /// Whether `file` (main) is opened in shared-cache mode; probed once per file.
    fn shared_cache(&self, file: FileId) -> Result<bool, Fallback> {
        if let Some((f, shared)) = self.state.try_borrow().map_err(|_| REENTRANT)?.shared_cache
            && f == file
        {
            return Ok(shared);
        }
        // No borrow is held while SQL runs (rule R1). A failed probe (e.g. interrupted) is
        // transient and not remembered.
        let shared = uses_shared_cache(self.db).map_err(|_| SCHEMA_UNREADABLE)?;
        self.state.try_borrow_mut().map_err(|_| REENTRANT)?.shared_cache = Some((file, shared));
        Ok(shared)
    }

    /// Root pages for a schema cookie (loaded through SQL when the file or the cookie
    /// changed). The schema is read in the same read transaction as the header, so the two
    /// agree.
    fn roots_for(&self, file: FileId, cookie: u32) -> Result<Roots, Fallback> {
        if let Some((f, c, roots)) = self.state.try_borrow().map_err(|_| REENTRANT)?.roots
            && f == file
            && c == cookie
        {
            return Ok(roots);
        }
        // No borrow is held while SQL runs (rule R1).
        let roots = roots::load(self.db).map_err(|_| SCHEMA_UNREADABLE)?;
        self.state.try_borrow_mut().map_err(|_| REENTRANT)?.roots = Some((file, cookie, roots));
        self.count(|c| c.root_loads += 1);
        Ok(roots)
    }

    /// Page `pgno` of the snapshot `snap`, from the page cache or the file. The caller has
    /// checked `snap` at its entry point (`check` or `snapshot`).
    pub fn page(&self, snap: &Snapshot, pgno: u32) -> Result<Arc<[u8]>, Fallback> {
        if pgno == 0 || pgno > snap.page_count {
            return Err(Fallback::Corrupt("page number beyond the end of the database"));
        }
        {
            let mut st = self.state.try_borrow_mut().map_err(|_| REENTRANT)?;
            if !st.current.is_some_and(|s| s.same(snap)) {
                return Err(Fallback::Precondition("the page cache belongs to another snapshot"));
            }
            if let Some(page) = st.cache.get(i64::from(pgno)) {
                let page = page.clone();
                drop(st);
                self.count(|c| c.cache_hits += 1);
                return Ok(page);
            }
        }
        let size = snap.geom.page_size;
        let mut page: Arc<[u8]> = std::iter::repeat_n(0u8, size).collect();
        let buf = Arc::get_mut(&mut page).ok_or(Fallback::Precondition("page buffer is shared"))?;
        let offset = (u64::from(pgno) - 1) * size as u64;
        file::read_at(self.db, snap.file, buf, offset).map_err(read_error)?;
        self.state.try_borrow_mut().map_err(|_| REENTRANT)?.cache.insert(i64::from(pgno), page.clone(), size);
        self.count(|c| c.pages_read += 1);
        Ok(page)
    }

    /// Mode, whether the reader currently serves cursors (or why not), counters and cache
    /// occupancy, as `sparql_status` rows.
    pub fn status(&self) -> Vec<(String, String)> {
        let c = self.counters.get();
        let (pages, bytes) = self.state.try_borrow().map(|s| (s.cache.len(), s.cache.cost())).unwrap_or_default();
        let last = self.last_fallback.get().map_or_else(|| "none".to_string(), |f| f.to_string());
        vec![
            ("reader".into(), self.mode().as_str().into()),
            ("page_cache_mb".into(), self.page_cache_mb().to_string()),
            ("btree_cursors".into(), c.cursors.to_string()),
            ("btree_pages_read".into(), c.pages_read.to_string()),
            ("btree_cache_hits".into(), c.cache_hits.to_string()),
            ("btree_cache_pages".into(), pages.to_string()),
            ("btree_cache_bytes".into(), bytes.to_string()),
            ("btree_cache_flushes".into(), c.cache_flushes.to_string()),
            ("btree_root_loads".into(), c.root_loads.to_string()),
            ("btree_fallbacks".into(), c.fallbacks.to_string()),
            ("btree_resumes".into(), c.resumes.to_string()),
            ("btree_last_fallback".into(), last),
        ]
    }
}

/// Whether `main` is a sharable b-tree (shared-cache mode). SQLite codes `OP_TableLock`
/// into a statement that reads a table of a sharable b-tree, and only then
/// (`sqlite3TableLock` returns early otherwise); `EXPLAIN` lists the opcodes without running
/// them. `sqlite_schema` always exists, so the probe does not depend on our schema.
fn uses_shared_cache(db: RawDb) -> Result<bool> {
    /// `EXPLAIN` result columns: addr, opcode, p1, p2, p3, p4, p5, comment.
    const OPCODE: std::ffi::c_int = 1;
    let mut st = RawStmt::prepare(db, "EXPLAIN SELECT 1 FROM main.sqlite_schema WHERE 0", false)?;
    while st.step()? {
        if st.col_str(OPCODE)? == "TableLock" {
            return Ok(true);
        }
    }
    Ok(false)
}

fn read_error(e: ReadError) -> Fallback {
    match e {
        ReadError::FileChanged => Fallback::Precondition("the main database file changed"),
        ReadError::NoRead => Fallback::Unsupported("the database file cannot be read through its VFS"),
        ReadError::ShortRead => Fallback::Corrupt("page beyond the end of the file"),
        ReadError::Io(_) => Fallback::Corrupt("I/O error while reading a page"),
    }
}
