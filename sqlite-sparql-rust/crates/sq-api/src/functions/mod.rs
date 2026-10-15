//! The scalar and aggregate SQL functions of §8.2.
//!
//! All use `SQLITE_UTF8`; none is DETERMINISTIC (every one depends on the dictionary or the
//! database). Writers and filesystem access are `SQLITE_DIRECTONLY`, so they cannot run from
//! views, triggers or the schema; `sparql_config` is registered per argument count so that
//! only the two-argument setter is DIRECTONLY. Each argument count is registered separately.
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

const PLAIN: FunctionFlags = FunctionFlags::SQLITE_UTF8;
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
    for &n in arities {
        let st = st.clone();
        conn.create_scalar_function(name, n, flags, move |ctx| {
            if hook {
                st.debug_panic(PanicSite::Scalar);
            }
            body(&st, ctx).map_err(sql_error)
        })?;
    }
    Ok(())
}

pub(crate) fn register(conn: &Connection, st: &Arc<ConnState>) -> rusqlite::Result<()> {
    // Store and configuration (writers are DIRECTONLY).
    scalar(conn, st, "sparql_version", &[0], PLAIN, store::version)?;
    scalar(conn, st, "sparql_init", &[0], DIRECT, store::init)?;
    scalar(conn, st, "sparql_load", &[1, 2, 3, 4], DIRECT, store::load)?;
    scalar(conn, st, "sparql_load_file", &[1, 2, 3, 4], DIRECT, store::load_file)?;
    scalar(conn, st, "sparql_update", &[1], DIRECT, store::update)?;
    scalar(conn, st, "sparql_analyze", &[0], DIRECT, store::analyze)?;
    scalar(conn, st, "sparql_vacuum_terms", &[0], DIRECT, store::vacuum_terms)?;
    scalar(conn, st, "sparql_check", &[0], PLAIN, store::check)?;
    scalar(conn, st, "sparql_config", &[1], PLAIN, store::config_get)?;
    scalar(conn, st, "sparql_config", &[2], DIRECT, store::config_set)?;

    // Queries (native engine, baseline translator).
    scalar(conn, st, "sparql_query", &[1, 2], PLAIN, query::query)?;
    scalar(conn, st, "sparql_ask", &[1], PLAIN, query::ask)?;
    scalar(conn, st, "sparql_count", &[1], PLAIN, query::count)?;
    scalar(conn, st, "sparql_value", &[1], PLAIN, query::value)?;
    scalar(conn, st, "sparql_explain", &[1, 2], PLAIN, query::explain)?;
    scalar(conn, st, "sparql_query_sql", &[1, 2], PLAIN, query::query_sql)?;
    scalar(conn, st, "sparql_count_sql", &[1], PLAIN, query::count_sql)?;
    scalar(conn, st, "sparql_to_sql", &[1], PLAIN, query::to_sql)?;

    // Terms.
    scalar(conn, st, "sparql_term", &[1], PLAIN, terms::term)?;
    scalar(conn, st, "sparql_term_value", &[1], PLAIN, terms::term_value)?;
    scalar(conn, st, "sparql_term_id", &[1], PLAIN, terms::term_id)?;
    scalar(conn, st, "sparql_term_kind", &[1], PLAIN, terms::kind)?;
    scalar(conn, st, "sparql_term_lex", &[1], PLAIN, terms::lex)?;
    scalar(conn, st, "sparql_term_lang", &[1], PLAIN, terms::lang)?;
    scalar(conn, st, "sparql_term_datatype", &[1], PLAIN, terms::datatype)?;

    // Helpers of the baseline translator's SQL (sq_sqlgen::HELPER_FUNCTIONS).
    scalar(conn, st, "sparql_num", &[1], PLAIN, helpers::num)?;
    scalar(conn, st, "sparql_str", &[1], PLAIN, helpers::str)?;
    scalar(conn, st, "sparql_ebv", &[1], PLAIN, helpers::ebv)?;
    scalar(conn, st, "sparql_sortkey", &[1], PLAIN, helpers::sortkey)?;
    scalar(conn, st, "sparql_regex", &[2, 3], PLAIN, helpers::regex)?;
    scalar(conn, st, "sparql_eval", &[-1], PLAIN, helpers::eval)?;
    for (name, arities, func) in helpers::aggregates() {
        for &n in arities {
            conn.create_aggregate_function(name, n, PLAIN, AggHelper::new(st.clone(), func.clone()))?;
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
