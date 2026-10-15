//! Query functions: `sparql_query`, `sparql_ask`, `sparql_count`, `sparql_value`,
//! `sparql_explain` and the baseline translator's `sparql_query_sql`, `sparql_count_sql`,
//! `sparql_to_sql`. Each runs as one read entry; the execution is dropped (its statements
//! returned to the pool) before the read scope closes.
use std::fmt::Write as _;

use rusqlite::functions::Context;
use rusqlite::types::Value;
use sq_core::plan::{EngineKind, QueryOptions};
use sq_core::store::Dictionary;
use sq_core::{Error, Result, TermId};
use sq_plan::ExplainMode;
use sq_results::ResultFormat;

use super::{opt_text_arg, text_arg};
use crate::query::{self, BaselineRun};
use crate::state::ConnState;
use crate::value::native;

/// `sparql_query(query [, format])`.
pub(super) fn query(st: &ConnState, ctx: &Context<'_>) -> Result<String> {
    let text = text_arg(ctx, 0, "query")?;
    let fmt = opt_text_arg(ctx, 1, "format")?.map(ResultFormat::parse).transpose()?;
    let settings = st.settings()?;
    st.read_call(|| query::write_output(&mut query::prepare(st, text, &settings)?, fmt))
}

/// `sparql_ask(query)`: 1 if there is at least one solution (or triple).
pub(super) fn ask(st: &ConnState, ctx: &Context<'_>) -> Result<i64> {
    let text = text_arg(ctx, 0, "query")?;
    let settings = st.settings()?;
    st.read_call(|| Ok(i64::from(query::prepare(st, text, &settings)?.ask()?)))
}

/// `sparql_count(query)`: number of solutions (or triples); no decode, no serialization.
pub(super) fn count(st: &ConnState, ctx: &Context<'_>) -> Result<i64> {
    let text = text_arg(ctx, 0, "query")?;
    let settings = st.settings()?;
    st.read_call(|| Ok(query::prepare(st, text, &settings)?.count()? as i64))
}

/// `sparql_value(query)`: native value of the first projected variable of the first row.
pub(super) fn value(st: &ConnState, ctx: &Context<'_>) -> Result<Value> {
    let text = text_arg(ctx, 0, "query")?;
    let settings = st.settings()?;
    st.read_call(|| {
        let term = query::first_value(&mut query::prepare(st, text, &settings)?)?;
        Ok(native(term.as_deref()).into_value())
    })
}

/// `sparql_explain(query [, mode])`: `algebra`, `logical`, `plan` (default), `analyze`
/// (executes), `sql` (the baseline's SQL) or `sqlplan` (its EXPLAIN QUERY PLAN).
pub(super) fn explain(st: &ConnState, ctx: &Context<'_>) -> Result<String> {
    let text = text_arg(ctx, 0, "query")?;
    let mode = opt_text_arg(ctx, 1, "mode")?.unwrap_or("plan").trim().to_ascii_lowercase();
    let opts = st.settings()?.options();
    let engine_mode = match mode.as_str() {
        "algebra" => Some(ExplainMode::Algebra),
        "logical" => Some(ExplainMode::Logical),
        "plan" => Some(ExplainMode::Plan),
        "analyze" => Some(ExplainMode::Analyze),
        "sql" | "sqlplan" => None,
        other => {
            return Err(Error::Invalid(format!(
                "unknown explain mode '{other}' (expected algebra, logical, plan, analyze, sql or sqlplan)"
            )));
        }
    };
    st.read_call(|| match engine_mode {
        Some(m) => sq_engine::explain(st.quad_store(), text, m, &opts),
        None => {
            let sql = translate(st, text, &opts)?;
            if mode == "sqlplan" {
                return st.store().explain_query_plan(&sql.sql);
            }
            let mut out = sql.sql;
            for (i, p) in sql.params.iter().enumerate() {
                let _ = write!(out, "\n-- ?{} = {}", i + 1, describe_param(st, TermId(*p)));
            }
            Ok(out)
        }
    })
}

/// A baseline parameter for humans: the N-Triples text of a term id, or the raw number.
fn describe_param(st: &ConnState, id: TermId) -> String {
    let term = if id.is_storable() { st.store().decode_one(id).ok() } else { None };
    match term {
        Some(t) => format!("{} ({})", id.raw(), t.to_ntriples()),
        None => id.raw().to_string(),
    }
}

/// Parses, lowers (constants resolve against the dictionary) and translates to SQL.
fn translate(st: &ConnState, text: &str, opts: &QueryOptions) -> Result<sq_sqlgen::SqlQuery> {
    let parsed = st.parse_query(text, opts.base_iri.as_deref())?;
    sq_sqlgen::translate(&sq_plan::lower_query(&parsed, st.store(), opts)?)
}

/// `sparql_query_sql(query [, format])`: the baseline engine, whatever `engine` says.
pub(super) fn query_sql(st: &ConnState, ctx: &Context<'_>) -> Result<String> {
    let text = text_arg(ctx, 0, "query")?;
    let fmt = opt_text_arg(ctx, 1, "format")?.map(ResultFormat::parse).transpose()?;
    let mut settings = st.settings()?;
    settings.engine = EngineKind::Sql;
    st.read_call(|| query::write_output(&mut query::prepare(st, text, &settings)?, fmt))
}

/// `sparql_count_sql(query)`: `SELECT count(*) FROM (<baseline SQL>)`.
pub(super) fn count_sql(st: &ConnState, ctx: &Context<'_>) -> Result<i64> {
    let text = text_arg(ctx, 0, "query")?;
    let opts = st.settings()?.options();
    st.read_call(|| {
        let parsed = st.parse_query(text, opts.base_iri.as_deref())?;
        let lowered = sq_plan::lower_query(&parsed, st.store(), &opts)?;
        Ok(BaselineRun::count_in_sql(st, lowered, &opts)? as i64)
    })
}

/// `sparql_to_sql(query)`: the baseline translator's SQL text.
pub(super) fn to_sql(st: &ConnState, ctx: &Context<'_>) -> Result<String> {
    let text = text_arg(ctx, 0, "query")?;
    let opts = st.settings()?.options();
    st.read_call(|| Ok(translate(st, text, &opts)?.sql))
}
