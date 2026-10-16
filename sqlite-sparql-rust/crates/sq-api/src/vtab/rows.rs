//! `sparql_rows(query [, opts])`: the eponymous table-valued function of §8.3a.
//!
//! ```sql
//! CREATE TABLE x(n INTEGER,                    -- col 0: solution ordinal (1-based)
//!   v0..v15,                                   -- cols 1..16: native value of the k-th variable
//!   t0 TEXT..t15 TEXT,                         -- cols 17..32: N-Triples text
//!   id0 INTEGER..id15 INTEGER,                 -- cols 33..48: TermId (NULL: unbound / TEMP)
//!   vars TEXT,                                 -- col 49: JSON array of variable names
//!   json TEXT,                                 -- col 50: SPARQL-JSON bindings of the row
//!   query HIDDEN, opts HIDDEN)                 -- cols 51, 52
//! ```
//! `xBestIndex` requires a usable `=` constraint on `query` (and takes `opts` when there is
//! one) and passes `colUsed` to `xFilter` as hex in `idx_str`. `xFilter` prepares a streaming
//! execution and fetches the first batch as one read entry; later batches read the same
//! snapshot through the outer statement's read transaction (see `state`). `xNext` pulls
//! 1024-row batches and decodes only the columns the statement reads, so `count(*)` decodes
//! nothing.
//!
//! ASK gives one row (`v0` = 0/1); CONSTRUCT / DESCRIBE give one row per triple (`v0..v2` =
//! subject, predicate, object). Queries with more than 16 variables are read through `json`
//! (or `sparql_view`).
use std::borrow::Cow;
use std::cell::RefCell;
use std::ffi::{CStr, c_int};
use std::sync::Arc;

use rusqlite::types::{Null, ValueRef};
use rusqlite::vtab::{
    Context, Filters, IndexConstraintOp, IndexInfo, VTab, VTabConfig, VTabConnection, VTabCursor, sqlite3_vtab,
    sqlite3_vtab_cursor,
};
use sq_core::{Error, Result};

use super::{Source, Stream, col_used, column_error};
use crate::config::PanicSite;
use crate::error::{IntoSql, sql_error};
use crate::guard::{guard, guard_drop, guard_eof};
use crate::query::{self, Form};
use crate::state::{ConnState, CursorToken};
use crate::value::{native, write_ntriples};

const SCHEMA: &CStr = c"CREATE TABLE x(n INTEGER, \
    v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,v10,v11,v12,v13,v14,v15, \
    t0 TEXT,t1 TEXT,t2 TEXT,t3 TEXT,t4 TEXT,t5 TEXT,t6 TEXT,t7 TEXT,t8 TEXT,t9 TEXT,t10 TEXT,t11 TEXT,t12 TEXT,\
    t13 TEXT,t14 TEXT,t15 TEXT, \
    id0 INTEGER,id1 INTEGER,id2 INTEGER,id3 INTEGER,id4 INTEGER,id5 INTEGER,id6 INTEGER,id7 INTEGER,id8 INTEGER,\
    id9 INTEGER,id10 INTEGER,id11 INTEGER,id12 INTEGER,id13 INTEGER,id14 INTEGER,id15 INTEGER, \
    vars TEXT, json TEXT, query HIDDEN, opts HIDDEN)";

/// Variables with their own `v` / `t` / `id` columns.
const VAR_COLUMNS: usize = 16;
const COL_N: c_int = 0;
const COL_V: c_int = 1;
const COL_T: c_int = COL_V + VAR_COLUMNS as c_int;
const COL_ID: c_int = COL_T + VAR_COLUMNS as c_int;
const COL_VARS: c_int = COL_ID + VAR_COLUMNS as c_int;
const COL_JSON: c_int = COL_VARS + 1;
const COL_QUERY: c_int = COL_JSON + 1;
const COL_OPTS: c_int = COL_QUERY + 1;
/// `idx_num` flag: `opts` is argument 2 of `xFilter`.
const HAS_OPTS: c_int = 1;

#[repr(C)]
pub(crate) struct RowsTab {
    /// Base class; must be first.
    base: sqlite3_vtab,
    st: Arc<ConnState>,
}

unsafe impl<'vtab> VTab<'vtab> for RowsTab {
    type Aux = Arc<ConnState>;
    type Cursor = RowsCursor;

    fn connect(
        db: &mut VTabConnection,
        aux: Option<&Arc<ConnState>>,
        _module: &[u8],
        _database: &[u8],
        _table: &[u8],
        _args: &[&[u8]],
    ) -> rusqlite::Result<(Cow<'static, CStr>, Self)> {
        guard("sparql_rows xConnect", || {
            let st =
                aux.cloned().ok_or_else(|| rusqlite::Error::ModuleError("sparql_rows: no connection state".into()))?;
            // Read-only and side-effect free: usable from views and triggers.
            db.config(VTabConfig::Innocuous)?;
            Ok((Cow::Borrowed(SCHEMA), RowsTab { base: sqlite3_vtab::default(), st }))
        })
    }

    fn best_index(&self, info: &mut IndexInfo) -> rusqlite::Result<bool> {
        guard("sparql_rows xBestIndex", || {
            self.st.debug_panic(PanicSite::BestIndex);
            let (mut query, mut opts, mut opts_unusable) = (None, None, false);
            for (i, c) in info.constraints().enumerate() {
                if c.operator() != IndexConstraintOp::SQLITE_INDEX_CONSTRAINT_EQ {
                    continue;
                }
                match (c.column(), c.is_usable()) {
                    (COL_QUERY, true) if query.is_none() => query = Some(i),
                    (COL_OPTS, true) if opts.is_none() => opts = Some(i),
                    (COL_OPTS, false) => opts_unusable = true,
                    _ => {}
                }
            }
            // Without the query (or with opts that are not available yet) this plan cannot
            // run: SQLITE_CONSTRAINT makes SQLite try another join order.
            let Some(query) = query else { return Ok(false) };
            if opts.is_none() && opts_unusable {
                return Ok(false);
            }
            {
                let mut u = info.constraint_usage(query);
                u.set_argv_index(1);
                u.set_omit(true);
            }
            let mut flags = 0;
            if let Some(o) = opts {
                let mut u = info.constraint_usage(o);
                u.set_argv_index(2);
                u.set_omit(true);
                flags |= HAS_OPTS;
            }
            info.set_idx_num(flags);
            info.set_idx_str(&format!("{:x}", info.col_used()));
            info.set_estimated_rows(1000);
            info.set_estimated_cost(1000.0);
            Ok(true)
        })
    }

    fn open(&'vtab mut self) -> rusqlite::Result<RowsCursor> {
        guard("sparql_rows xOpen", || {
            self.st.debug_panic(PanicSite::Open);
            Ok(RowsCursor {
                base: sqlite3_vtab_cursor::default(),
                st: self.st.clone(),
                scan: RefCell::new(None),
                text: RefCell::new(String::new()),
            })
        })
    }
}

/// One running query.
struct Scan {
    // Field order is drop order: the execution (and its leased statements) first, then the
    // open-cursor count.
    stream: Stream,
    form: Form,
    /// Names for the `json` column (variables, or subject / predicate / object).
    names: Vec<String>,
    vars_json: String,
    query: String,
    opts: Option<String>,
    _token: CursorToken,
}

impl Scan {
    /// Prepares the query and fetches the first batch, as one read entry.
    fn open(st: &Arc<ConnState>, query: String, opts: Option<String>, used: u64) -> Result<Scan> {
        st.read_call(|| Scan::start(st, query, opts, used))
    }

    fn start(st: &Arc<ConnState>, query: String, opts: Option<String>, used: u64) -> Result<Scan> {
        let conn = st.settings()?;
        let mut settings = conn.clone();
        if let Some(o) = &opts {
            settings.apply_json(o)?;
            settings.within_limits_of(&conn);
        }
        let mut run = query::prepare(st, &query, &settings)?;
        let form = run.form();
        let (source, names, vars) = match form {
            Form::Select => {
                let vars = run.solutions().variables().to_vec();
                (Source::Solutions(run), vars.clone(), vars)
            }
            Form::Ask => (Source::Ask(Some(run.ask()?)), Vec::new(), Vec::new()),
            Form::Graph => {
                let names: Vec<String> = ["subject", "predicate", "object"].map(String::from).into();
                (Source::Triples(run), names.clone(), names)
            }
        };
        let width = if form == Form::Ask { 1 } else { names.len() };
        // Late materialization: decode only what the statement reads.
        let decode = (0..width)
            .filter(|&k| {
                col_used(used, COL_JSON as usize)
                    || (k < VAR_COLUMNS && (col_used(used, COL_V as usize + k) || col_used(used, COL_T as usize + k)))
            })
            .collect();
        let vars_json = serde_json::to_string(&vars).map_err(|e| Error::internal(e.to_string()))?;
        let token = CursorToken::new(st);
        let stream = Stream::open(st, source, width, decode)?;
        Ok(Scan { stream, form, names, vars_json, query, opts, _token: token })
    }
}

/// A TEXT argument of `xFilter` (NULL → None), reported like the scalar functions' arguments.
fn text_value<'a>(value: Option<ValueRef<'a>>, what: &str) -> Result<Option<&'a str>> {
    match value.unwrap_or(ValueRef::Null) {
        ValueRef::Null => Ok(None),
        ValueRef::Text(t) => std::str::from_utf8(t)
            .map(Some)
            .map_err(|_| Error::Invalid(format!("sparql_rows: the {what} is not valid UTF-8"))),
        other => Err(Error::Invalid(format!("sparql_rows: the {what} must be TEXT, got {}", other.data_type()))),
    }
}

#[repr(C)]
pub(crate) struct RowsCursor {
    /// Base class; must be first.
    base: sqlite3_vtab_cursor,
    st: Arc<ConnState>,
    /// `RefCell`: `xColumn` (`&self`) may decode a column on demand.
    scan: RefCell<Option<Scan>>,
    /// Reusable buffer for the `t*` and `json` columns.
    text: RefCell<String>,
}

impl Drop for RowsCursor {
    fn drop(&mut self) {
        // xClose: end the execution (its statements go back to the pool).
        let scan = self.scan.get_mut().take();
        guard_drop(|| {
            self.st.debug_panic(PanicSite::Close);
            drop(scan);
        });
    }
}

impl RowsCursor {
    /// The `xColumn` body; its error goes through [`column_error`].
    fn column_value(&self, ctx: &mut Context, i: c_int) -> rusqlite::Result<()> {
        let mut scan = self.scan.try_borrow_mut().map_err(|_| sql_error(Error::Reentrant))?;
        let Some(scan) = scan.as_mut() else { return ctx.set_result(&Null) };
        let st = &*self.st;
        match i {
            COL_N => ctx.set_result(&scan.stream.ordinal()),
            _ if (COL_V..COL_T).contains(&i) => {
                let term = scan.stream.term(st, (i - COL_V) as usize).into_sql()?;
                ctx.set_result(&native(term.as_deref()))
            }
            _ if (COL_T..COL_ID).contains(&i) => match scan.stream.term(st, (i - COL_T) as usize).into_sql()? {
                Some(t) => {
                    let mut text = self.text.borrow_mut();
                    text.clear();
                    write_ntriples(&t, &mut text);
                    ctx.set_result(&text.as_str())
                }
                None => ctx.set_result(&Null),
            },
            _ if (COL_ID..COL_VARS).contains(&i) => {
                ctx.set_result(&scan.stream.public_id(st, (i - COL_ID) as usize).into_sql()?)
            }
            COL_VARS => ctx.set_result(&scan.vars_json.as_str()),
            COL_JSON => {
                let mut text = self.text.borrow_mut();
                text.clear();
                if scan.form == Form::Ask {
                    let b = scan.stream.id(0) == sq_core::TermId::from_bool(true);
                    text.push_str(if b { r#"{"boolean":true}"# } else { r#"{"boolean":false}"# });
                } else {
                    let terms = scan.stream.row_terms(st).into_sql()?;
                    sq_results::append_solution_json(&mut text, &scan.names, terms);
                }
                ctx.set_result(&text.as_str())
            }
            COL_QUERY => ctx.set_result(&scan.query.as_str()),
            COL_OPTS => ctx.set_result(&scan.opts.as_deref()),
            _ => ctx.set_result(&Null),
        }
    }
}

unsafe impl VTabCursor for RowsCursor {
    fn filter(&mut self, idx_num: c_int, idx_str: Option<&str>, args: &Filters<'_>) -> rusqlite::Result<()> {
        guard("sparql_rows xFilter", || {
            self.st.debug_panic(PanicSite::Filter);
            // A re-filtered cursor (correlated join) ends its previous scan first.
            *self.scan.get_mut() = None;
            let mut values = args.iter();
            let query = text_value(values.next(), "query").into_sql()?;
            let opts = if idx_num & HAS_OPTS != 0 { text_value(values.next(), "opts").into_sql()? } else { None };
            // A NULL query gives no rows, as NULL does for SQLite's own table-valued functions
            // (`json_each(NULL)`): a correlated `sparql_rows(t.q)` skips rows without a query.
            let Some(query) = query else { return Ok(()) };
            let used = idx_str.and_then(|s| u64::from_str_radix(s, 16).ok()).unwrap_or(u64::MAX);
            *self.scan.get_mut() =
                Some(Scan::open(&self.st, query.to_string(), opts.map(str::to_string), used).into_sql()?);
            Ok(())
        })
    }

    fn next(&mut self) -> rusqlite::Result<()> {
        guard("sparql_rows xNext", || {
            self.st.debug_panic(PanicSite::Next);
            match self.scan.get_mut() {
                Some(scan) => scan.stream.advance(&self.st).into_sql(),
                None => Ok(()),
            }
        })
    }

    fn eof(&self) -> bool {
        guard_eof(|| {
            self.st.debug_panic(PanicSite::Eof);
            self.scan.try_borrow().map_or(true, |s| s.as_ref().is_none_or(|s| s.stream.eof()))
        })
    }

    fn column(&self, ctx: &mut Context, i: c_int) -> rusqlite::Result<()> {
        guard("sparql_rows xColumn", || {
            self.st.debug_panic(PanicSite::Column);
            let r = self.column_value(ctx, i);
            column_error(ctx, r)
        })
    }

    fn rowid(&self) -> rusqlite::Result<i64> {
        guard("sparql_rows xRowid", || {
            self.st.debug_panic(PanicSite::Rowid);
            Ok(self.scan.try_borrow().ok().and_then(|s| s.as_ref().map(|s| s.stream.ordinal())).unwrap_or(0))
        })
    }
}
