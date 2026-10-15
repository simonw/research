//! Prototype of the sq-api skeleton: ConnState + close hook + sparql_rows col_used plumbing.
use rusqlite::vtab::*;
use rusqlite::{ffi, functions::FunctionFlags, Connection, Result};
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::ffi::CStr;
use std::os::raw::c_int;
use std::sync::Arc;

pub struct RawStmt(*mut ffi::sqlite3_stmt);
pub struct ConnState {
    db: *mut ffi::sqlite3,
    pool: RefCell<Vec<RawStmt>>,
    armed: Cell<bool>,
    pub finalized: Cell<u32>,
}
// SAFETY: touched only inside SQLite callbacks of this connection (serialized by SQLite).
unsafe impl Send for ConnState {}
unsafe impl Sync for ConnState {}

impl ConnState {
    fn disarm_and_finalize(&self) {
        self.armed.set(false);
        for s in self.pool.borrow_mut().drain(..) {
            unsafe { ffi::sqlite3_finalize(s.0) };
            self.finalized.set(self.finalized.get() + 1);
        }
    }
    fn lease_and_keep(&self, sql: &CStr) -> i64 {
        let mut st = std::ptr::null_mut();
        let rc = unsafe { ffi::sqlite3_prepare_v3(self.db, sql.as_ptr(), -1, ffi::SQLITE_PREPARE_PERSISTENT, &mut st, std::ptr::null_mut()) };
        if rc != 0 { return -1; }
        let mut v = 0;
        unsafe { if ffi::sqlite3_step(st) == ffi::SQLITE_ROW { v = ffi::sqlite3_column_int64(st, 0); } ffi::sqlite3_reset(st); }
        if self.armed.get() { self.pool.borrow_mut().push(RawStmt(st)); } else { unsafe { ffi::sqlite3_finalize(st) }; }
        v
    }
}

#[repr(C)]
struct StatusTab { base: sqlite3_vtab, st: Arc<ConnState> }
impl Drop for StatusTab {
    fn drop(&mut self) {
        let st = self.st.clone();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| st.disarm_and_finalize()));
    }
}
#[repr(C)]
struct StatusCur { base: sqlite3_vtab_cursor, n: i64, armed: i64 }
unsafe impl<'v> VTab<'v> for StatusTab {
    type Aux = Arc<ConnState>;
    type Cursor = StatusCur;
    fn connect(_db: &mut VTabConnection, aux: Option<&Arc<ConnState>>, _m: &[u8], _d: &[u8], _t: &[u8], _a: &[&[u8]]) -> Result<(Cow<'static, CStr>, Self)> {
        let st = aux.unwrap().clone();
        st.armed.set(true);
        Ok((Cow::Borrowed(c"CREATE TABLE x(key TEXT, value)"), StatusTab { base: sqlite3_vtab::default(), st }))
    }
    fn best_index(&self, _i: &mut IndexInfo) -> Result<bool> { Ok(true) }
    fn open(&'v mut self) -> Result<StatusCur> { Ok(StatusCur { base: sqlite3_vtab_cursor::default(), n: 0, armed: self.st.armed.get() as i64 }) }
}
unsafe impl VTabCursor for StatusCur {
    fn filter(&mut self, _: c_int, _: Option<&str>, _: &Filters<'_>) -> Result<()> { self.n = 0; Ok(()) }
    fn next(&mut self) -> Result<()> { self.n += 1; Ok(()) }
    fn eof(&self) -> bool { self.n >= 1 }
    fn column(&self, ctx: &mut Context, i: c_int) -> Result<()> { if i == 0 { ctx.set_result(&"armed") } else { ctx.set_result(&self.armed) } }
    fn rowid(&self) -> Result<i64> { Ok(self.n) }
}

#[repr(C)]
struct RowsTab { base: sqlite3_vtab }
#[repr(C)]
struct RowsCur { base: sqlite3_vtab_cursor, n: i64, col_used: u64 }
const ROWS_SCHEMA: &CStr = c"CREATE TABLE x(n INTEGER, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,v10,v11,v12,v13,v14,v15, t0 TEXT,t1 TEXT,t2 TEXT,t3 TEXT,t4 TEXT,t5 TEXT,t6 TEXT,t7 TEXT,t8 TEXT,t9 TEXT,t10 TEXT,t11 TEXT,t12 TEXT,t13 TEXT,t14 TEXT,t15 TEXT, id0 INTEGER,id1 INTEGER,id2 INTEGER,id3 INTEGER,id4 INTEGER,id5 INTEGER,id6 INTEGER,id7 INTEGER,id8 INTEGER,id9 INTEGER,id10 INTEGER,id11 INTEGER,id12 INTEGER,id13 INTEGER,id14 INTEGER,id15 INTEGER, vars TEXT, json TEXT, query HIDDEN, opts HIDDEN)";
unsafe impl<'v> VTab<'v> for RowsTab {
    type Aux = Arc<ConnState>;
    type Cursor = RowsCur;
    fn connect(db: &mut VTabConnection, _aux: Option<&Arc<ConnState>>, _m: &[u8], _d: &[u8], _t: &[u8], _a: &[&[u8]]) -> Result<(Cow<'static, CStr>, Self)> {
        db.config(VTabConfig::Innocuous)?;
        Ok((Cow::Borrowed(ROWS_SCHEMA), RowsTab { base: sqlite3_vtab::default() }))
    }
    fn best_index(&self, info: &mut IndexInfo) -> Result<bool> {
        let mut have_q = false;
        let mut opts_idx = None;
        for (i, c) in info.constraints().enumerate() {
            if c.is_usable() && c.operator() == IndexConstraintOp::SQLITE_INDEX_CONSTRAINT_EQ {
                if c.column() == 51 { have_q = true; let _ = i; }
                if c.column() == 52 { opts_idx = Some(i); }
            }
        }
        if !have_q { return Ok(false); }
        let mut argv = 1;
        let cols: Vec<(usize, c_int)> = info.constraints().enumerate().map(|(i, c)| (i, c.column())).collect();
        for (i, col) in cols {
            if col == 51 { let mut u = info.constraint_usage(i); u.set_argv_index(1); u.set_omit(true); }
            if Some(i) == opts_idx { argv += 1; let mut u = info.constraint_usage(i); u.set_argv_index(argv); u.set_omit(true); }
        }
        let cu = info.col_used();
        info.set_idx_str(&format!("{cu:x}"));
        info.set_estimated_rows(1000);
        info.set_estimated_cost(1000.0);
        Ok(true)
    }
    fn open(&'v mut self) -> Result<RowsCur> { Ok(RowsCur { base: sqlite3_vtab_cursor::default(), n: 0, col_used: 0 }) }
}
unsafe impl VTabCursor for RowsCur {
    fn filter(&mut self, _: c_int, idx_str: Option<&str>, args: &Filters<'_>) -> Result<()> {
        let _q: String = args.get(0)?;
        self.col_used = u64::from_str_radix(idx_str.unwrap_or("0"), 16).unwrap_or(u64::MAX);
        self.n = 1; Ok(())
    }
    fn next(&mut self) -> Result<()> { self.n += 1; Ok(()) }
    fn eof(&self) -> bool { self.n > 3 }
    fn column(&self, ctx: &mut Context, i: c_int) -> Result<()> {
        match i { 0 => ctx.set_result(&self.n), 49 => ctx.set_result(&format!("{:x}", self.col_used)), _ => ctx.set_result(&rusqlite::types::Null) }
    }
    fn rowid(&self) -> Result<i64> { Ok(self.n) }
}

pub fn register(conn: &Connection) -> Result<()> {
    let st = Arc::new(ConnState { db: unsafe { conn.handle() }, pool: RefCell::new(Vec::new()), armed: Cell::new(false), finalized: Cell::new(0) });
    conn.create_module("sparql_status", const { &Module::<StatusTab>::eponymous_only_module() }, Some(st.clone()))?;
    conn.create_module("sparql_rows", const { &Module::<RowsTab>::eponymous_only_module() }, Some(st.clone()))?;
    // arm: connect the status vtab (prepare + finalize; the eponymous instance stays connected)
    conn.prepare("SELECT 1 FROM sparql_status")?;
    let s2 = st.clone();
    conn.create_scalar_function("sq_probe", 0, FunctionFlags::SQLITE_UTF8, move |_ctx| {
        let t = unsafe { ffi::sqlite3_txn_state(s2.db, std::ptr::null()) };
        let v = s2.lease_and_keep(c"SELECT count(*) FROM sqlite_schema");
        Ok(format!("txn={t} v={v} pooled={} armed={}", s2.pool.borrow().len(), s2.armed.get()))
    })?;
    Ok(())
}
