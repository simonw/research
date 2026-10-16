//! `sparql_status`: the eponymous `(key TEXT, value)` status table (§8.3c) — and the
//! connection's **close hook** (§8.7).
//!
//! `register()` prepares `SELECT 1 FROM main.sparql_status` once, which connects the
//! eponymous instance; its `xConnect` arms the statement pool. `sqlite3_close` disconnects
//! eponymous tables *before* its busy check, so the instance's destructor disarms the pool
//! and finalizes every idle statement: the connection then closes fully (no zombie keeping
//! the database and its `-wal` file open). The instance stays connected across DDL, other
//! connections' DDL and VACUUM; should it ever be disconnected while the connection lives,
//! the next API entry re-arms (`ConnState::enter`).
use std::borrow::Cow;
use std::ffi::{CStr, c_int};
use std::sync::Arc;

use rusqlite::types::Null;
use rusqlite::vtab::{
    Context, Filters, IndexInfo, VTab, VTabConfig, VTabConnection, VTabCursor, sqlite3_vtab, sqlite3_vtab_cursor,
};

use crate::guard::{guard, guard_drop, guard_eof};
use crate::state::ConnState;

const SCHEMA: &CStr = c"CREATE TABLE x(key TEXT, value)";

#[repr(C)]
pub(crate) struct StatusTab {
    /// Base class; must be first.
    base: sqlite3_vtab,
    st: Arc<ConnState>,
}

impl Drop for StatusTab {
    fn drop(&mut self) {
        // xDisconnect: the close hook.
        guard_drop(|| self.st.status_disconnected());
    }
}

unsafe impl<'vtab> VTab<'vtab> for StatusTab {
    type Aux = Arc<ConnState>;
    type Cursor = StatusCursor;

    fn connect(
        db: &mut VTabConnection,
        aux: Option<&Arc<ConnState>>,
        _module: &[u8],
        _database: &[u8],
        _table: &[u8],
        _args: &[&[u8]],
    ) -> rusqlite::Result<(Cow<'static, CStr>, Self)> {
        guard("sparql_status xConnect", || {
            let st = aux
                .cloned()
                .ok_or_else(|| rusqlite::Error::ModuleError("sparql_status: no connection state".into()))?;
            db.config(VTabConfig::Innocuous)?;
            // Built before arming: the destructor (disarm) runs only for a connected instance.
            let tab = StatusTab { base: sqlite3_vtab::default(), st };
            tab.st.status_connected();
            Ok((Cow::Borrowed(SCHEMA), tab))
        })
    }

    fn best_index(&self, info: &mut IndexInfo) -> rusqlite::Result<bool> {
        guard("sparql_status xBestIndex", || {
            info.set_estimated_rows(64);
            info.set_estimated_cost(64.0);
            Ok(true)
        })
    }

    fn open(&'vtab mut self) -> rusqlite::Result<StatusCursor> {
        guard("sparql_status xOpen", || {
            Ok(StatusCursor { base: sqlite3_vtab_cursor::default(), st: self.st.clone(), rows: Vec::new(), pos: 0 })
        })
    }
}

#[repr(C)]
pub(crate) struct StatusCursor {
    /// Base class; must be first.
    base: sqlite3_vtab_cursor,
    st: Arc<ConnState>,
    rows: Vec<(String, String)>,
    pos: usize,
}

unsafe impl VTabCursor for StatusCursor {
    fn filter(&mut self, _idx_num: c_int, _idx_str: Option<&str>, _args: &Filters<'_>) -> rusqlite::Result<()> {
        guard("sparql_status xFilter", || {
            // A snapshot of the counters: reading the status touches no table.
            self.rows = self.st.status_rows();
            self.pos = 0;
            Ok(())
        })
    }

    fn next(&mut self) -> rusqlite::Result<()> {
        guard("sparql_status xNext", || {
            self.pos += 1;
            Ok(())
        })
    }

    fn eof(&self) -> bool {
        guard_eof(|| self.pos >= self.rows.len())
    }

    fn column(&self, ctx: &mut Context, i: c_int) -> rusqlite::Result<()> {
        guard("sparql_status xColumn", || {
            let Some((key, value)) = self.rows.get(self.pos) else { return ctx.set_result(&Null) };
            match i {
                0 => ctx.set_result(&key.as_str()),
                // Numbers as INTEGER, so `value > 0` and arithmetic work in SQL.
                1 => match value.parse::<i64>() {
                    Ok(v) => ctx.set_result(&v),
                    Err(_) => ctx.set_result(&value.as_str()),
                },
                _ => ctx.set_result(&Null),
            }
        })
    }

    fn rowid(&self) -> rusqlite::Result<i64> {
        guard("sparql_status xRowid", || Ok(self.pos as i64 + 1))
    }
}
