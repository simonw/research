//! The scalar and aggregate SQL functions of §8.2.
//!
//! All use `SQLITE_UTF8`; none is DETERMINISTIC (every one depends on the dictionary or the
//! database). Each argument count is registered separately, so `sparql_config` has two flag
//! sets: the one-argument getter is a reader, the two-argument setter a writer.
//!
//! * Writers and filesystem access (`DIRECT`: init, loads, update, analyze, vacuum, the
//!   setter) are `SQLITE_DIRECTONLY`: SQLite refuses them in views, triggers and DEFAULT
//!   clauses stored in `main` or an attached database (generated columns and index
//!   expressions take only deterministic functions anyway). TEMP views and TEMP triggers are
//!   exempt (only the connection itself can create them), so they may call writers. SQLite
//!   does **not** refuse them in a stored CHECK constraint, so each call also passes
//!   `ConnState::direct_guard`, which refuses writers nested in another sparqlite call and,
//!   while a stored table's definition names a writer, any call that is not a top-level
//!   SELECT naming the function.
//! * Everything else (`READ`) is `SQLITE_INNOCUOUS`, so stored views and triggers may call
//!   it even with `PRAGMA trusted_schema = OFF` (the sqlite3 shell's default). Each one only
//!   reads `main`'s RDF store and the connection's sparqlite settings, writes nothing (no
//!   data, no settings, no files), opens no file or socket, and runs within the
//!   connection's `timeout_ms` / `max_memory_mb`, SQLite's length limit and the nesting bound
//!   of `ConnState::enter` like any query. The full argument, and the one case it does not
//!   cover (views and triggers stored in an ATTACHed database also reach `main`'s store), is
//!   in the crate README.
use std::sync::Arc;

use rusqlite::Connection;
use rusqlite::functions::{Context, FunctionFlags};
use rusqlite::types::{ToSql, ValueRef};
use sq_core::{Error, Result};

use crate::config::PanicSite;
use crate::error::sql_error;
use crate::helpers::{self, AggHelper};
use crate::state::ConnState;

mod query;
mod store;
mod terms;

/// A function body: connection state + SQL arguments → value.
type Body<T> = fn(&ConnState, &Context<'_>) -> Result<T>;

/// Readers: side-effect free, usable from stored views and triggers (see the module docs).
const READ: FunctionFlags = FunctionFlags::SQLITE_UTF8.union(FunctionFlags::SQLITE_INNOCUOUS);
/// Writers and filesystem access: top-level SQL and TEMP views / triggers only.
const DIRECT: FunctionFlags = FunctionFlags::SQLITE_UTF8.union(FunctionFlags::SQLITE_DIRECTONLY);

fn scalar<T: ToSql + 'static>(
    conn: &Connection,
    st: &Arc<ConnState>,
    name: &'static str,
    arities: &[i32],
    flags: FunctionFlags,
    body: Body<T>,
) -> rusqlite::Result<()> {
    // `sparql_config` stays callable while the testing hook is armed, so it can be disarmed.
    let hook = name != "sparql_config";
    // DIRECTONLY functions also check where the call comes from (`ConnState::direct_guard`).
    let direct = flags.contains(FunctionFlags::SQLITE_DIRECTONLY);
    // The schema scan of the guard looks for these names.
    debug_assert!(!direct || crate::state::DIRECT_FUNCTIONS.iter().any(|f| name.starts_with(f)), "{name}");
    for &n in arities {
        let st = st.clone();
        conn.create_scalar_function(name, n, flags, move |ctx| {
            if hook {
                st.debug_panic(PanicSite::Scalar);
            }
            if direct {
                st.direct_guard(name).map_err(sql_error)?;
            }
            body(&st, ctx).map_err(sql_error)
        })?;
    }
    Ok(())
}

pub(crate) fn register(conn: &Connection, st: &Arc<ConnState>) -> rusqlite::Result<()> {
    // Store and configuration (writers and file access are DIRECTONLY, readers INNOCUOUS).
    scalar(conn, st, "sparql_version", &[0], READ, store::version)?;
    scalar(conn, st, "sparql_init", &[0], DIRECT, store::init)?;
    scalar(conn, st, "sparql_load", &[1, 2, 3, 4], DIRECT, store::load)?;
    scalar(conn, st, "sparql_load_file", &[1, 2, 3, 4], DIRECT, store::load_file)?;
    scalar(conn, st, "sparql_update", &[1], DIRECT, store::update)?;
    scalar(conn, st, "sparql_analyze", &[0], DIRECT, store::analyze)?;
    scalar(conn, st, "sparql_vacuum_terms", &[0], DIRECT, store::vacuum_terms)?;
    scalar(conn, st, "sparql_check", &[0], READ, store::check)?;
    scalar(conn, st, "sparql_config", &[1], READ, store::config_get)?;
    scalar(conn, st, "sparql_config", &[2], DIRECT, store::config_set)?;

    // Queries (native engine, baseline translator).
    scalar(conn, st, "sparql_query", &[1, 2], READ, query::query)?;
    scalar(conn, st, "sparql_ask", &[1], READ, query::ask)?;
    scalar(conn, st, "sparql_count", &[1], READ, query::count)?;
    scalar(conn, st, "sparql_value", &[1], READ, query::value)?;
    scalar(conn, st, "sparql_explain", &[1, 2], READ, query::explain)?;
    scalar(conn, st, "sparql_query_sql", &[1, 2], READ, query::query_sql)?;
    scalar(conn, st, "sparql_count_sql", &[1], READ, query::count_sql)?;
    scalar(conn, st, "sparql_to_sql", &[1], READ, query::to_sql)?;

    // Terms.
    scalar(conn, st, "sparql_term", &[1], READ, terms::term)?;
    scalar(conn, st, "sparql_term_value", &[1], READ, terms::term_value)?;
    scalar(conn, st, "sparql_term_id", &[1], READ, terms::term_id)?;
    scalar(conn, st, "sparql_term_kind", &[1], READ, terms::kind)?;
    scalar(conn, st, "sparql_term_lex", &[1], READ, terms::lex)?;
    scalar(conn, st, "sparql_term_lang", &[1], READ, terms::lang)?;
    scalar(conn, st, "sparql_term_datatype", &[1], READ, terms::datatype)?;

    // Helpers of the baseline translator's SQL (sq_sqlgen::HELPER_FUNCTIONS).
    scalar(conn, st, "sparql_num", &[1], READ, helpers::num)?;
    scalar(conn, st, "sparql_str", &[1], READ, helpers::str)?;
    scalar(conn, st, "sparql_ebv", &[1], READ, helpers::ebv)?;
    scalar(conn, st, "sparql_sortkey", &[1], READ, helpers::sortkey)?;
    scalar(conn, st, "sparql_regex", &[2, 3], READ, helpers::regex)?;
    scalar(conn, st, "sparql_eval", &[-1], READ, helpers::eval)?;
    for (name, arities, func) in helpers::aggregates() {
        for &n in arities {
            conn.create_aggregate_function(name, n, READ, AggHelper::new(st.clone(), func.clone()))?;
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ argument access

/// A required TEXT argument (borrowed from SQLite).
fn text_arg<'a>(ctx: &'a Context<'_>, i: usize, what: &str) -> Result<&'a str> {
    opt_text_arg(ctx, i, what)?.ok_or_else(|| Error::Invalid(format!("{what} is NULL")))
}

/// An optional TEXT argument: absent or NULL → None.
fn opt_text_arg<'a>(ctx: &'a Context<'_>, i: usize, what: &str) -> Result<Option<&'a str>> {
    if i >= ctx.len() {
        return Ok(None);
    }
    match ctx.get_raw(i) {
        ValueRef::Null => Ok(None),
        ValueRef::Text(t) => {
            std::str::from_utf8(t).map(Some).map_err(|_| Error::Invalid(format!("{what} is not valid UTF-8")))
        }
        other => Err(Error::Invalid(format!("{what} must be TEXT, got {}", other.data_type()))),
    }
}

/// A setting value: TEXT, INTEGER or REAL, as text.
fn setting_arg(ctx: &Context<'_>, i: usize) -> Result<String> {
    match ctx.get_raw(i) {
        ValueRef::Text(t) => {
            std::str::from_utf8(t).map(str::to_string).map_err(|_| Error::Invalid("value is not valid UTF-8".into()))
        }
        ValueRef::Integer(v) => Ok(v.to_string()),
        ValueRef::Real(v) => Ok(v.to_string()),
        ValueRef::Null => Err(Error::Invalid("sparql_config: the value is NULL".into())),
        ValueRef::Blob(_) => Err(Error::Invalid("sparql_config: the value must not be a BLOB".into())),
    }
}
