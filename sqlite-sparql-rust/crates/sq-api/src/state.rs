//! Per-connection state (§8.6) and the entry protocol every SQL function and virtual-table
//! method follows.
//!
//! **Entry protocol.** Each API entry
//! 1. re-arms the statement pool if the close hook is not connected (§8.7),
//! 2. records its nesting depth (savepoint names carry it, rule R3),
//! 3. opens the right scope — a [`ReadScope`](sq_sqlite::ReadScope) for readers; for writers
//!    the [`WriteScope`](sq_sqlite::WriteScope) *first*, so that its `write_gen` bump takes
//!    the write lock straight from "no transaction" and `busy_timeout` applies (SQLite skips
//!    the busy handler when it upgrades a read lock the connection already holds),
//! 4. validates the epoch (`StorageShared::validate_epoch`, §4.4: schema cookie, `dict_gen`,
//!    committed dictionary rows, statistics generation),
//! 5. does its work and closes its scopes, reporting a failed `RELEASE` / `COMMIT`.
//!
//! Virtual-table scans follow it in `xFilter` only: no scope is ever held across a return to
//! SQLite (a savepoint held between rows would take the connection out of autocommit mode, so
//! the user's own statements would silently join it). The scan keeps its snapshot anyway:
//! once `main`'s read transaction has started, SQLite keeps it while any statement of the
//! connection is still reading, and the host statement itself is one.
//!
//! **Nesting (R8).** Step 2 also bounds how deeply entries nest. sparqlite's own SQL reads
//! `main`'s `rdf_*` tables, and a database file may make one of them a view (or give it a
//! trigger) that calls sparqlite again; without a bound, such a file recursed until the host's
//! stack overflowed. A nested entry is refused once [`MAX_ENTRY_DEPTH`] entries are active or
//! once more than [`NESTED_STACK_BUDGET`] of stack lies between it and the outermost entry
//! (the nested request then still has its own nesting budget, `nesting.rs`, of stack left).
//! Writers never run nested ([`ConnState::direct_guard`]).
//!
//! **Re-entrancy (R1).** All mutable state sits in `Cell`s and `RefCell`s that are borrowed
//! only for O(1) reads and writes, never across a call that steps a statement or returns to
//! SQLite: the baseline engine's generated SQL calls this crate's helper functions while
//! this crate steps it. A violated rule surfaces as `Error::Reentrant`, never as a deadlock
//! (no state of a connection sits behind a `Mutex`; the one lock of the crate, a `timeout_ms`
//! watchdog's, is never held while a statement steps).
use std::cell::{Cell, RefCell};
use std::ffi::OsStr;
use std::rc::Rc;
use std::sync::Arc;

use rustc_hash::FxHashMap;
use sq_core::store::{QuadStore, QuadStoreWriter};
use sq_core::{Error, Result};
use sq_expr::CompiledExpr;
use sq_sqlite::{RawDb, SchemaState, SqliteStore, StorageConfig, StorageShared};

use crate::config::{ConnConfig, Key, PanicSite, QuerySettings};
use crate::helpers::{Baselines, HelperCtx};
use crate::lru::ParsedLru;

/// Compiled `sparql_eval` templates by operator, then arity.
type Templates = FxHashMap<Box<str>, Vec<(usize, Rc<CompiledExpr>)>>;

/// What the last writing call did (`sparql_status` keys `last_*`).
#[derive(Clone, Debug, Default)]
pub(crate) struct ChangeReport {
    /// `load`, `update`, `init`, `analyze` or `vacuum_terms` (empty before the first write).
    pub op: &'static str,
    pub inserted: u64,
    pub deleted: u64,
    /// Loads: quads read by the parser, new dictionary rows, bulk mode.
    pub quads_read: u64,
    pub terms_new: u64,
    pub bulk: bool,
    pub ms: u64,
}

/// API-level counters (`sparql_status`). `terms_decoded` proves that `count(*)` over
/// `sparql_rows` decodes nothing.
#[derive(Copy, Clone, Default, Debug)]
pub(crate) struct ApiCounters {
    /// Virtual-table scans started (`sparql_rows`, `sparql_view`).
    pub cursors_opened: u64,
    /// Rows produced by virtual-table scans.
    pub rows_emitted: u64,
    /// Terms decoded for virtual-table output columns.
    pub terms_decoded: u64,
}

/// One connection's sparqlite state. Created by `register()`, shared (`Arc`) by every
/// registered function and module of that connection.
pub(crate) struct ConnState {
    db: RawDb,
    store: Rc<SqliteStore>,
    config: RefCell<ConnConfig>,
    parsed: RefCell<ParsedLru>,
    /// Open `sparql_rows` / `sparql_view` scans; writers refuse while > 0 (Halloween).
    open_cursors: Cell<u32>,
    /// Nesting depth of API entries.
    depth: Cell<u32>,
    /// Stack position of the outermost active entry (meaningful while `depth > 0`).
    stack_base: Cell<usize>,
    /// Connected `sparql_status` instances (the close hook, §8.7).
    status_tabs: Cell<u32>,
    last_report: RefCell<ChangeReport>,
    counters: Cell<ApiCounters>,
    /// Baseline-engine statements being stepped (their helper-function contexts).
    pub(crate) baselines: Rc<Baselines>,
    /// Reusable context for helper functions called directly from user SQL; taken out while
    /// in use, so no borrow is held during evaluation.
    helper_spare: RefCell<Option<Box<HelperCtx>>>,
    /// Compiled `sparql_eval` templates.
    templates: RefCell<Templates>,
    /// The `debug_panic` testing hook can be armed (see [`debug_hooks_enabled`]).
    debug_hooks: bool,
    /// R7: thread of the outermost active entry (debug builds).
    #[cfg(debug_assertions)]
    entry_thread: Cell<Option<std::thread::ThreadId>>,
}

// SAFETY: there is one ConnState per `sqlite3*`, and it is only touched from inside SQLite
// callbacks of that connection (SQL functions, virtual-table methods, their destructors),
// which SQLite serializes: in serialized mode (THREADSAFE=1) every API call holds the
// connection mutex, and multi-thread mode forbids using one connection from two threads at
// once. The non-thread-safe parts (`Rc`, `Cell`, `RefCell`) are therefore never accessed
// concurrently. rusqlite requires function closures to be `Send + 'static` and module aux
// data to be `Send + Sync`, which is why these impls exist; debug builds check the claim at
// every entry (rule R7).
unsafe impl Send for ConnState {}
unsafe impl Sync for ConnState {}

/// An active API entry: holds the nesting depth until dropped.
pub(crate) struct Entry<'a> {
    st: &'a ConnState,
    depth: u32,
}

impl Entry<'_> {
    pub(crate) fn depth(&self) -> u32 {
        self.depth
    }
}

impl Drop for Entry<'_> {
    fn drop(&mut self) {
        self.st.depth.set(self.depth);
        #[cfg(debug_assertions)]
        if self.depth == 0 {
            self.st.entry_thread.set(None);
        }
    }
}

/// Counts one open virtual-table scan for as long as it lives.
pub(crate) struct CursorToken(Arc<ConnState>);

impl CursorToken {
    pub(crate) fn new(st: &Arc<ConnState>) -> CursorToken {
        st.open_cursors.set(st.open_cursors.get() + 1);
        st.bump(|c| c.cursors_opened += 1);
        CursorToken(st.clone())
    }
}

impl Drop for CursorToken {
    fn drop(&mut self) {
        self.0.open_cursors.set(self.0.open_cursors.get().saturating_sub(1));
    }
}

impl ConnState {
    /// # Safety
    /// `db` must be the connection this state is registered on; it must outlive the state.
    pub(crate) unsafe fn new(db: *mut rusqlite::ffi::sqlite3) -> Arc<ConnState> {
        // SAFETY: forwarded caller contract.
        let raw = unsafe { RawDb::from_ptr(db) };
        let shared = StorageShared::new(raw, StorageConfig::default());
        Arc::new(ConnState {
            db: raw,
            store: Rc::new(SqliteStore::new(shared)),
            config: RefCell::new(ConnConfig::default()),
            parsed: RefCell::new(ParsedLru::default()),
            open_cursors: Cell::new(0),
            depth: Cell::new(0),
            stack_base: Cell::new(0),
            status_tabs: Cell::new(0),
            last_report: RefCell::new(ChangeReport::default()),
            counters: Cell::new(ApiCounters::default()),
            baselines: Rc::new(Baselines::default()),
            helper_spare: RefCell::new(None),
            templates: RefCell::new(FxHashMap::default()),
            debug_hooks: debug_hooks_enabled(std::env::var_os(DEBUG_HOOKS_ENV).as_deref()),
            #[cfg(debug_assertions)]
            entry_thread: Cell::new(None),
        })
    }

    // ------------------------------------------------------------------ accessors

    pub(crate) fn store(&self) -> &SqliteStore {
        &self.store
    }

    pub(crate) fn shared(&self) -> &Rc<StorageShared> {
        self.store.shared()
    }

    /// The store as the engine's read interface.
    pub(crate) fn quad_store(&self) -> Rc<dyn QuadStore> {
        self.store.clone()
    }

    /// The store as the engine's write interface.
    pub(crate) fn writer(&self) -> Rc<dyn QuadStoreWriter> {
        self.store.clone()
    }

    pub(crate) fn bump(&self, f: impl FnOnce(&mut ApiCounters)) {
        let mut c = self.counters.get();
        f(&mut c);
        self.counters.set(c);
    }

    pub(crate) fn record(&self, report: ChangeReport) {
        if let Ok(mut r) = self.last_report.try_borrow_mut() {
            *r = report;
        }
    }

    // ------------------------------------------------------------------ close hook (§8.7)

    /// The `sparql_status` instance connected: arm the statement pool.
    pub(crate) fn status_connected(&self) {
        self.status_tabs.set(self.status_tabs.get() + 1);
        self.shared().arm_pool();
    }

    /// A `sparql_status` instance disconnected (`sqlite3_close` disconnects it before its busy
    /// check): with none left, disarm and finalize every idle statement so the connection
    /// closes fully instead of becoming a zombie.
    pub(crate) fn status_disconnected(&self) {
        let n = self.status_tabs.get().saturating_sub(1);
        self.status_tabs.set(n);
        if n == 0 {
            self.shared().disarm_and_finalize();
        }
    }

    /// Entry step 1: re-arm the pool if the close hook was disconnected behind our back.
    fn rearm(&self) {
        let shared = self.shared();
        if shared.is_armed() {
            return;
        }
        if self.status_tabs.get() > 0 {
            shared.arm_pool();
            return;
        }
        // Preparing a statement over the eponymous table connects it (xConnect arms). A
        // failure only costs speed: a disarmed pool finalizes statements instead of keeping
        // them.
        let _ = self.db.exec("SELECT 1 FROM main.sparql_status WHERE 0");
    }

    // ------------------------------------------------------------------ entries

    /// Starts an API entry (R7, the nesting bound R8, re-arm, depth).
    pub(crate) fn enter(&self) -> Result<Entry<'_>> {
        let depth = self.depth.get();
        #[cfg(debug_assertions)]
        {
            let me = std::thread::current().id();
            match self.entry_thread.get() {
                Some(t) => assert_eq!(t, me, "sparql: concurrent entry into one connection from two threads (R7)"),
                None => self.entry_thread.set(Some(me)),
            }
        }
        let here = stack_position();
        if depth == 0 {
            self.stack_base.set(here);
        } else {
            let used = self.stack_base.get().abs_diff(here);
            if depth >= MAX_ENTRY_DEPTH || used > NESTED_STACK_BUDGET {
                return Err(Error::Unsupported(format!(
                    "sparqlite calls nest too deeply ({depth} calls are active, {} KiB of stack in use): a view or \
                     trigger that sparqlite's own SQL runs calls sparqlite again; check that the rdf_* objects of \
                     this database are sparqlite's own tables",
                    used >> 10
                )));
            }
        }
        self.rearm();
        self.depth.set(depth + 1);
        Ok(Entry { st: self, depth })
    }

    /// Refuses a `SQLITE_DIRECTONLY` function (writers, file access, the `sparql_config`
    /// setter; `functions/mod.rs` calls this before each of them) where SQLite's own guard does
    /// not reach:
    ///
    /// 1. **Nested** inside another API entry: only schema code that sparqlite's own SQL fired
    ///    (a trigger on, or a view in place of, an `rdf_*` table) can call it there.
    /// 2. **From a stored CHECK constraint.** SQLite refuses DIRECTONLY functions in views,
    ///    triggers and DEFAULT clauses stored in `main` or an attached database, and generated
    ///    columns and index expressions accept only deterministic functions, but it lets a
    ///    stored CHECK constraint call them (verified with SQLite 3.45.1, also for its own
    ///    DIRECTONLY `readfile()`). A CHECK runs inside any INSERT or UPDATE of its table, also
    ///    one that a trigger or the application issues, and inside `PRAGMA integrity_check` /
    ///    `quick_check`. A function cannot tell which schema object called it, so while any
    ///    table stored in `main` or an attached database names a DIRECTONLY function in its
    ///    definition, the call must prove it is direct: the connection runs exactly one
    ///    statement, that statement is read-only (no CHECK of any table can run in it, except in
    ///    an integrity check), and its own SQL text calls the function by name (an integrity
    ///    check does not). TEMP views and triggers keep working while no such table exists.
    pub(crate) fn direct_guard(&self, name: &str) -> Result<()> {
        if self.depth.get() > 0 {
            return Err(Error::Invalid(format!(
                "{name} cannot run inside another sparqlite call (a trigger or view that sparqlite's own SQL fired \
                 called it)"
            )));
        }
        // A call that is plainly direct needs no schema scan (the common `SELECT sparql_update(?)`
        // costs a walk over the running statements, before any statement of our own exists).
        let running = self.running_statements();
        if matches!(running.as_slice(), [(true, sql)] if calls_by_name(sql, name)) {
            return Ok(());
        }
        let Some(table) = self.table_naming_a_writer()? else {
            return Ok(());
        };
        Err(Error::Invalid(format!(
            "{name} refused: the definition of table {table} names a sparqlite writer, and SQLite lets a CHECK \
             constraint stored in a database call SQLITE_DIRECTONLY functions; while such a table is in main or \
             an attached database, writers run only from a top-level SELECT that calls them by name, with no \
             other statement of this connection running"
        )))
    }

    /// Every statement of the connection that is running (stepped and not yet reset): whether
    /// it is read-only, and its SQL text.
    fn running_statements(&self) -> Vec<(bool, String)> {
        use rusqlite::ffi;
        let db = self.db.as_ptr();
        let mut out = Vec::new();
        // SAFETY: we run inside a callback of this connection, which holds its mutex, so the
        // list of its prepared statements cannot change while it is walked; `sqlite3_sql`
        // returns NULL or a NUL-terminated string owned by the statement, copied at once.
        unsafe {
            let mut stmt = ffi::sqlite3_next_stmt(db, std::ptr::null_mut());
            while !stmt.is_null() {
                if ffi::sqlite3_stmt_busy(stmt) != 0 {
                    let text = ffi::sqlite3_sql(stmt);
                    let sql = if text.is_null() {
                        String::new()
                    } else {
                        std::ffi::CStr::from_ptr(text).to_string_lossy().into_owned()
                    };
                    out.push((ffi::sqlite3_stmt_readonly(stmt) != 0, sql));
                }
                stmt = ffi::sqlite3_next_stmt(db, stmt);
            }
        }
        out
    }

    /// The first table stored in `main` or an attached database (not TEMP: only the
    /// connection itself creates objects there) whose definition mentions a DIRECTONLY
    /// function, as `schema.table`. SQLite matches function names ignoring ASCII case only,
    /// and an identifier cannot be spelled any other way, so a CHECK that calls one of them
    /// contains its name.
    fn table_naming_a_writer(&self) -> Result<Option<String>> {
        use rusqlite::types::ValueRef;
        let failed = |e: rusqlite::Error| match e {
            rusqlite::Error::SqliteFailure(f, m) => {
                Error::Sqlite { code: f.extended_code, msg: m.unwrap_or_else(|| f.to_string()) }
            }
            other => Error::internal(other.to_string()),
        };
        // SAFETY: a wrapper of this live connection that never closes it (`from_handle` is
        // not owning); it and its statements are dropped before this call returns.
        let conn = unsafe { rusqlite::Connection::from_handle(self.db.as_ptr()) }.map_err(failed)?;
        let schemas: Vec<String> = {
            let mut st = conn.prepare("SELECT name FROM pragma_database_list WHERE name <> 'temp'").map_err(failed)?;
            st.query_map([], |r| r.get(0)).map_err(failed)?.collect::<rusqlite::Result<_>>().map_err(failed)?
        };
        for schema in schemas {
            let sql = format!(
                "SELECT name, sql FROM \"{}\".sqlite_schema WHERE type = 'table'",
                schema.replace('"', "\"\"")
            );
            let mut st = conn.prepare(&sql).map_err(failed)?;
            let mut rows = st.query([]).map_err(failed)?;
            while let Some(row) = rows.next().map_err(failed)? {
                let (ValueRef::Text(text) | ValueRef::Blob(text)) = row.get_ref(1).map_err(failed)? else {
                    continue;
                };
                let text = text.to_ascii_lowercase();
                if DIRECT_FUNCTIONS.iter().any(|f| text.windows(f.len()).any(|w| w == f.as_bytes())) {
                    let table = match row.get_ref(0) {
                        Ok(ValueRef::Text(t)) => String::from_utf8_lossy(t).into_owned(),
                        _ => "?".into(),
                    };
                    return Ok(Some(format!("{schema}.{table}")));
                }
            }
        }
        Ok(None)
    }

    /// Entry step 4: the §4.4 epoch check. An incompatible schema fails every call; an absent
    /// one is an empty store for readers.
    pub(crate) fn validate(&self) -> Result<()> {
        match self.shared().validate_epoch()? {
            SchemaState::Incompatible(m) => Err(Error::Schema(m)),
            SchemaState::Absent | SchemaState::Ready { .. } => Ok(()),
        }
    }

    /// A reading entry whose work ends inside the call.
    pub(crate) fn read_call<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T> {
        let entry = self.enter()?;
        let scope = self.store.read_scope(entry.depth())?;
        self.validate()?;
        let out = f();
        let closed = scope.close();
        let value = out?;
        closed?;
        Ok(value)
    }

    /// A writing entry: refuses while scans are open, auto-initializes the schema when asked
    /// to, then runs `f` in one write scope (all or nothing).
    ///
    /// **Inside a statement that writes** (`INSERT INTO t SELECT sparql_update(…)`), SQLite
    /// refuses every `SAVEPOINT`, so no write scope can open. In autocommit mode the host
    /// statement's own transaction makes the call atomic instead: if the call fails, so does
    /// the host statement, and SQLite rolls that whole transaction back (the next entry's
    /// epoch check sees it). That needs the writer to be the host statement itself
    /// ([`ConnState::host_statement_writes`]). Inside a user transaction a failed statement may
    /// keep its partial changes, so there — and for the writers whose storage layer opens its
    /// own scope: loads, analyze, vacuum, schema creation — the refusal becomes an error naming
    /// the restriction.
    pub(crate) fn write_call<T>(&self, auto_init: bool, f: impl FnOnce() -> Result<T>) -> Result<T> {
        let entry = self.enter()?;
        self.refuse_while_reading()?;
        if auto_init && !matches!(self.shared().schema_state()?, SchemaState::Ready { .. }) {
            // Outside the write scope: its schema read must not precede the write lock.
            self.store.ensure_schema().map_err(savepoint_refusal)?;
        }
        if self.host_statement_writes()? {
            self.validate()?;
            return f().map_err(savepoint_refusal);
        }
        let scope = self.store.write_scope(entry.depth()).map_err(savepoint_refusal)?;
        self.validate()?;
        let value = f()?;
        scope.commit()?;
        Ok(value)
    }

    /// A writing entry that manages its own scopes (`sparql_init`).
    pub(crate) fn writer_call<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T> {
        let _entry = self.enter()?;
        self.refuse_while_reading()?;
        f().map_err(savepoint_refusal)
    }

    /// Whether the statement that called us writes, in autocommit mode (see `write_call`).
    ///
    /// A statement that writes is running in autocommit mode when some schema is in a write
    /// transaction although none is open: only a running writer holds one. It is the host
    /// statement when it is the only statement the connection runs (the host is running). With
    /// other statements running the writer may be another one — a partly read `INSERT …
    /// RETURNING` beside `SELECT sparql_update(…)` — whose transaction would commit the first
    /// operations of a failed request, so the call is refused.
    fn host_statement_writes(&self) -> Result<bool> {
        use rusqlite::ffi;
        let db = self.db.as_ptr();
        // SAFETY: `db` is the live connection (constructor contract); a NULL schema name asks
        // for the highest transaction state over all schemas.
        let state = unsafe { ffi::sqlite3_txn_state(db, std::ptr::null()) };
        if state != ffi::SQLITE_TXN_WRITE || !self.db.autocommit() {
            return Ok(false);
        }
        let (mut running, mut writers) = (0u32, 0u32);
        // SAFETY: we run inside a callback of this connection, which holds its mutex, so the
        // list of its prepared statements cannot change while it is walked.
        unsafe {
            let mut stmt = ffi::sqlite3_next_stmt(db, std::ptr::null_mut());
            while !stmt.is_null() {
                if ffi::sqlite3_stmt_busy(stmt) != 0 {
                    running += 1;
                    writers += u32::from(ffi::sqlite3_stmt_readonly(stmt) == 0);
                }
                stmt = ffi::sqlite3_next_stmt(db, stmt);
            }
        }
        if running == 1 && writers == 1 {
            return Ok(true);
        }
        Err(Error::Invalid(
            "a writer cannot run while another statement of this connection writes (a partly read INSERT / UPDATE \
             / DELETE … RETURNING, or a statement that writes beside other running statements): finish or reset \
             those statements, or call it as its own SELECT statement"
                .into(),
        ))
    }

    fn refuse_while_reading(&self) -> Result<()> {
        if self.open_cursors.get() > 0 { Err(Error::WriteWhileReading) } else { Ok(()) }
    }

    /// The connection's `SQLITE_LIMIT_LENGTH`: the longest TEXT or BLOB SQLite accepts as a
    /// function result (1,000,000,000 bytes unless the application lowered it).
    pub(crate) fn length_limit(&self) -> usize {
        // SAFETY: `db` is the live connection (constructor contract); a negative new value
        // only reads the current limit.
        let n = unsafe {
            rusqlite::ffi::sqlite3_limit(self.db.as_ptr(), rusqlite::ffi::SQLITE_LIMIT_LENGTH, -1)
        };
        usize::try_from(n).unwrap_or(0)
    }

    // ------------------------------------------------------------------ configuration

    /// The query settings of this connection (per-call overrides start from these).
    pub(crate) fn settings(&self) -> Result<QuerySettings> {
        Ok(self.config.try_borrow().map_err(|_| Error::Reentrant)?.query.clone())
    }

    pub(crate) fn allow_files(&self) -> Result<bool> {
        Ok(self.config.try_borrow().map_err(|_| Error::Reentrant)?.allow_files)
    }

    /// `sparql_config(key)`.
    pub(crate) fn config_get(&self, key: Key) -> Result<String> {
        self.check_key(key)?;
        let shared = self.shared();
        Ok(match key {
            Key::Reader => shared.reader().as_str().to_string(),
            Key::PageCacheMb => shared.page_cache_mb().to_string(),
            k => self
                .config
                .try_borrow()
                .map_err(|_| Error::Reentrant)?
                .get(k)
                .ok_or_else(|| Error::internal(format!("no value for {}", k.name())))?,
        })
    }

    /// `sparql_config(key, value)`: validates, applies (storage settings take effect at
    /// once) and returns the new value.
    pub(crate) fn config_set(&self, key: Key, value: &str) -> Result<String> {
        self.check_key(key)?;
        let shared = self.shared();
        match key {
            Key::Reader => shared.set_reader(sq_sqlite::ReaderMode::parse(value.trim())?),
            Key::PageCacheMb => shared.set_page_cache_mb(crate::config::parse_u64(key, value)?)?,
            Key::DecodeCacheMb => {
                let mb = crate::config::parse_u64(key, value)?;
                shared.set_config(StorageConfig { decode_cache_mb: mb, ..shared.config() })?;
                self.config.try_borrow_mut().map_err(|_| Error::Reentrant)?.set(key, value)?;
            }
            k => self.config.try_borrow_mut().map_err(|_| Error::Reentrant)?.set(k, value)?,
        }
        self.config_get(key)
    }

    /// The `debug_panic` testing hook: panics at `site` when it is selected.
    pub(crate) fn debug_panic(&self, site: PanicSite) {
        if !self.debug_hooks {
            return;
        }
        let selected = self.config.try_borrow().ok().and_then(|c| c.debug_panic);
        if selected == Some(site) {
            panic!("sparql: debug_panic at {}", site.name());
        }
    }

    /// Refuses the `debug_panic` key where the hook is disabled.
    fn check_key(&self, key: Key) -> Result<()> {
        if key == Key::DebugPanic && !self.debug_hooks {
            return Err(Error::Invalid(format!(
                "debug_panic is a testing hook: available in debug builds, or when {DEBUG_HOOKS_ENV}=1 is set \
                 before the extension is loaded"
            )));
        }
        Ok(())
    }

    // ------------------------------------------------------------------ parsed queries

    /// Parses `text` through the connection's LRU (§4.4), after the nesting check (cached texts
    /// passed it).
    pub(crate) fn parse_query(&self, text: &str, base: Option<&str>) -> Result<Arc<spargebra::Query>> {
        if let Some(q) = self.parsed.try_borrow_mut().map_err(|_| Error::Reentrant)?.get(text, base) {
            return Ok(q);
        }
        let q = Arc::new(crate::nesting::parse_query(text, base)?);
        self.parsed.try_borrow_mut().map_err(|_| Error::Reentrant)?.insert(text, base, q.clone());
        Ok(q)
    }

    // ------------------------------------------------------------------ helper functions

    /// Runs `f` with the evaluation context of the helper functions: the innermost baseline
    /// statement this connection is stepping, or — for a direct call from user SQL — a
    /// fresh context, inside a complete read entry when `needs_store`. `f`'s last argument
    /// tells whether the context belongs to a baseline statement (TEMP ids are meaningful
    /// only there).
    pub(crate) fn with_helper<R>(
        &self,
        needs_store: bool,
        f: impl FnOnce(&mut HelperCtx, &SqliteStore, bool) -> Result<R>,
    ) -> Result<R> {
        if let Some(ctx) = self.baselines.current()? {
            // Inside our own baseline statement: its entry already validated the epoch. The
            // context stays borrowed while `f` reads the dictionary; those statements call no
            // SQL function, so nothing can re-enter it (a violation would be Error::Reentrant).
            let mut ctx = ctx.try_borrow_mut().map_err(|_| Error::Reentrant)?;
            return f(&mut ctx, &self.store, true);
        }
        let taken = self.helper_spare.try_borrow_mut().map_err(|_| Error::Reentrant)?.take();
        let mut ctx = taken.unwrap_or_else(|| Box::new(HelperCtx::new(Default::default(), None)));
        let out = if needs_store {
            self.read_call(|| f(&mut ctx, &self.store, false))
        } else {
            f(&mut ctx, &self.store, false)
        };
        // A direct call starts from nothing: no TEMP ids, no cached values.
        ctx.reset();
        if let Ok(mut spare) = self.helper_spare.try_borrow_mut() {
            *spare = Some(ctx);
        }
        out
    }

    /// The compiled `sparql_eval` template for (operator, arity).
    pub(crate) fn template(&self, op: &str, nargs: usize) -> Result<Rc<CompiledExpr>> {
        if let Some(e) = self
            .templates
            .try_borrow()
            .map_err(|_| Error::Reentrant)?
            .get(op)
            .and_then(|v| v.iter().find(|(n, _)| *n == nargs))
        {
            return Ok(e.1.clone());
        }
        let expr = sq_sqlgen::eval_template(op, nargs)
            .ok_or_else(|| Error::Invalid(format!("sparql_eval: unknown operator '{op}' with {nargs} argument(s)")))?;
        let compiled = Rc::new(sq_expr::compile(&expr, &Default::default())?);
        self.templates
            .try_borrow_mut()
            .map_err(|_| Error::Reentrant)?
            .entry(op.into())
            .or_default()
            .push((nargs, compiled.clone()));
        Ok(compiled)
    }

    // ------------------------------------------------------------------ status

    /// The rows of `sparql_status` (§8.3c).
    pub(crate) fn status_rows(&self) -> Vec<(String, String)> {
        let sqlite_version = rusqlite::version();
        let mut rows: Vec<(String, String)> = vec![
            ("version".into(), env!("CARGO_PKG_VERSION").into()),
            ("sqlite_version".into(), sqlite_version.into()),
        ];
        rows.extend(self.shared().status());
        let c = self.counters.get();
        let (hits, misses) = self.parsed.try_borrow().map(|p| p.counters()).unwrap_or_default();
        let parsed = self.parsed.try_borrow().map(|p| p.len()).unwrap_or_default();
        rows.extend([
            ("open_cursors".into(), self.open_cursors.get().to_string()),
            ("parsed_cache_entries".into(), parsed.to_string()),
            ("parsed_cache_hits".into(), hits.to_string()),
            ("parsed_cache_misses".into(), misses.to_string()),
            ("cursors_opened".into(), c.cursors_opened.to_string()),
            ("rows_emitted".into(), c.rows_emitted.to_string()),
            ("terms_decoded".into(), c.terms_decoded.to_string()),
        ]);
        if let Ok(r) = self.last_report.try_borrow() {
            rows.extend([
                ("last_op".into(), r.op.to_string()),
                ("last_inserted".into(), r.inserted.to_string()),
                ("last_deleted".into(), r.deleted.to_string()),
                ("last_quads_read".into(), r.quads_read.to_string()),
                ("last_terms_new".into(), r.terms_new.to_string()),
                ("last_bulk".into(), u8::from(r.bulk).to_string()),
                ("last_ms".into(), r.ms.to_string()),
            ]);
        }
        for key in Key::DOCUMENTED {
            if let Ok(v) = self.config_get(key) {
                rows.push((format!("config.{}", key.name()), v));
            }
        }
        rows
    }
}

/// Most API entries that may be active at once (R8). Legitimate nesting is shallow: a reader in
/// a trigger on `rdf_quad` runs one level inside the writer whose INSERT fired it.
pub(crate) const MAX_ENTRY_DEPTH: u32 = 8;

/// Most stack a nested entry may find in use since the outermost entry began (R8). The nested
/// request still parses within its own nesting budget (`nesting.rs`, about 1 MiB in the worst
/// case), so the two together stay inside a 2 MiB thread stack. Legitimate nesting (a trigger
/// fired by a writer's INSERT) measures far below this; see the crate README.
pub(crate) const NESTED_STACK_BUDGET: usize = 384 << 10;

/// The `SQLITE_DIRECTONLY` functions, lower case (`sparql_load` also matches
/// `sparql_load_file`). Kept in step with `functions::register` by a unit test there.
pub(crate) const DIRECT_FUNCTIONS: [&str; 6] =
    ["sparql_init", "sparql_load", "sparql_update", "sparql_analyze", "sparql_vacuum_terms", "sparql_config"];

/// An address in the current stack frame (the stack's direction does not matter to callers,
/// which only compare distances).
#[inline(always)]
fn stack_position() -> usize {
    let marker = 0u8;
    std::hint::black_box(&marker) as *const u8 as usize
}

/// Whether `sql` calls the function `name` (ASCII case-insensitive, as SQLite resolves
/// function names): the name as a whole identifier, followed by `(`.
fn calls_by_name(sql: &str, name: &str) -> bool {
    let (hay, needle) = (sql.as_bytes(), name.as_bytes());
    let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || b >= 0x80;
    (0..hay.len().saturating_sub(needle.len() - 1)).any(|i| {
        hay[i..i + needle.len()].eq_ignore_ascii_case(needle)
            && (i == 0 || !ident(hay[i - 1]))
            && hay[i + needle.len()..].iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'(')
    })
}

/// Environment variable that enables the `debug_panic` hook in release builds (read when the
/// connection registers). Production builds cannot arm the hook through SQL alone.
pub(crate) const DEBUG_HOOKS_ENV: &str = "SPARQLITE_DEBUG_HOOKS";

/// The hook exists in debug builds, and in release builds when [`DEBUG_HOOKS_ENV`] is `1`.
fn debug_hooks_enabled(env: Option<&OsStr>) -> bool {
    cfg!(debug_assertions) || env.is_some_and(|v| v == "1")
}

/// SQLite refuses `SAVEPOINT` while a statement that writes runs on the connection; the
/// storage layer reports that as SQLITE_BUSY "cannot open savepoint - SQL statements in
/// progress", which reads like lock contention. Named for what it is.
fn savepoint_refusal(e: Error) -> Error {
    match &e {
        Error::Sqlite { code, msg }
            if code & 0xff == rusqlite::ffi::SQLITE_BUSY && msg.contains("statements in progress") =>
        {
            Error::Invalid(
                "this writer cannot run inside a statement that writes (INSERT … SELECT, UPDATE …) here: only \
                 sparql_update can, and only outside an explicit transaction; call it as its own SELECT statement"
                    .into(),
            )
        }
        _ => e,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registered(conn: &rusqlite::Connection) -> Arc<ConnState> {
        // SAFETY: `conn` outlives the state in this test.
        let st = unsafe { ConnState::new(conn.handle()) };
        crate::functions::register(conn, &st).unwrap();
        crate::vtab::register(conn, &st).unwrap();
        conn.prepare("SELECT 1 FROM main.sparql_status").unwrap();
        st
    }

    #[test]
    fn entries_rearm_a_disarmed_pool_and_track_depth() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let st = registered(&conn);
        assert!(st.shared().is_armed(), "connecting sparql_status armed the pool");
        assert_eq!(st.status_tabs.get(), 1);
        st.shared().disarm_and_finalize();
        assert!(!st.shared().is_armed());
        {
            let outer = st.enter().unwrap();
            assert!(st.shared().is_armed(), "the next entry re-arms");
            assert_eq!(outer.depth(), 0);
            let inner = st.enter().unwrap();
            assert_eq!(inner.depth(), 1);
        }
        assert_eq!(st.depth.get(), 0);
    }

    /// R8: nested entries stop at `MAX_ENTRY_DEPTH`, and at `NESTED_STACK_BUDGET` of stack
    /// between the outermost entry and the nested one, whichever comes first.
    #[test]
    fn nested_entries_are_bounded_by_depth_and_stack() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let st = registered(&conn);
        fn nest(st: &ConnState) -> u32 {
            match st.enter() {
                Ok(_e) => 1 + nest(st),
                Err(e) => {
                    assert!(e.to_string().contains("nest too deeply"), "{e}");
                    0
                }
            }
        }
        assert_eq!(nest(&st), MAX_ENTRY_DEPTH);
        assert_eq!(st.depth.get(), 0, "refused entries leave the depth as it was");
        // Burn `kib` KiB of stack in 16 KiB frames, then try a nested entry.
        #[inline(never)]
        fn deep(st: &ConnState, kib: usize) -> bool {
            let frame = [0u8; 16 << 10];
            std::hint::black_box(&frame);
            if kib <= 16 { st.enter().is_ok() } else { deep(st, kib - 16) }
        }
        let outer = st.enter().unwrap();
        assert!(deep(&st, 64), "a shallow nested entry is fine");
        assert!(!deep(&st, (NESTED_STACK_BUDGET >> 10) + 64), "one past the stack budget is refused");
        drop(outer);
        assert!(deep(&st, (NESTED_STACK_BUDGET >> 10) + 64), "an outermost entry has no stack bound");
    }

    #[test]
    fn calls_by_name_needs_the_whole_name_and_a_parenthesis() {
        assert!(calls_by_name("SELECT sparql_update(?1)", "sparql_update"));
        assert!(calls_by_name("select SPARQL_Update ('x')", "sparql_update"));
        assert!(calls_by_name("SELECT 1, sparql_config(\n'k', 2)", "sparql_config"));
        assert!(!calls_by_name("PRAGMA integrity_check", "sparql_update"));
        assert!(!calls_by_name("PRAGMA integrity_check(sparql_update)", "sparql_update"));
        assert!(!calls_by_name("SELECT sparql_load_file('x')", "sparql_load"));
        assert!(!calls_by_name("SELECT my_sparql_load('x')", "sparql_load"));
        assert!(!calls_by_name("SELECT sparql_load", "sparql_load"));
        assert!(!calls_by_name("", "sparql_load"));
    }

    #[test]
    fn writers_refuse_while_a_scan_is_open() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let st = registered(&conn);
        let token = CursorToken::new(&st);
        assert!(matches!(st.write_call(true, || Ok(())), Err(Error::WriteWhileReading)));
        assert!(matches!(st.writer_call(|| Ok(())), Err(Error::WriteWhileReading)));
        assert!(st.read_call(|| Ok(())).is_ok());
        drop(token);
        assert!(st.write_call(true, || Ok(())).is_ok());
        assert_eq!(st.counters.get().cursors_opened, 1);
    }

    #[test]
    fn debug_hooks_need_a_debug_build_or_the_environment() {
        assert!(debug_hooks_enabled(Some(OsStr::new("1"))));
        assert_eq!(debug_hooks_enabled(None), cfg!(debug_assertions));
        assert_eq!(debug_hooks_enabled(Some(OsStr::new("yes"))), cfg!(debug_assertions));
        let busy = Error::Sqlite { code: 5, msg: "cannot open savepoint - SQL statements in progress".into() };
        assert!(matches!(savepoint_refusal(busy), Error::Invalid(m) if m.contains("own SELECT statement")));
        let locked = Error::Sqlite { code: 5, msg: "database is locked".into() };
        assert!(matches!(savepoint_refusal(locked), Error::Sqlite { .. }));
    }

    #[test]
    fn parsed_queries_are_cached() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let st = registered(&conn);
        let a = st.parse_query("ASK {}", None).unwrap();
        let b = st.parse_query("ASK {}", None).unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert!(matches!(st.parse_query("ASK {", None), Err(Error::Syntax(_))));
    }
}
