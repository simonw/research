//! Queries: lower + optimize + execute, and result serialization.
use std::io::Write;
use std::rc::Rc;

use sq_core::ir::QueryForm;
use sq_core::plan::QueryOptions;
use sq_core::store::QuadStore;
use sq_core::{Error, Result};
use sq_exec::{ExecCaches, Execution};
use sq_results::ResultFormat;

pub(crate) fn prepare(
    store: Rc<dyn QuadStore>,
    query: &spargebra::Query,
    opts: &QueryOptions,
    caches: ExecCaches,
) -> Result<Execution> {
    let lowered = sq_plan::lower_query(query, &*store, opts)?;
    let plan = sq_plan::optimize(lowered, &*store, opts)?;
    Execution::new(store, plan, opts, caches)
}

pub(crate) fn query_to_string(
    store: Rc<dyn QuadStore>,
    text: &str,
    fmt: ResultFormat,
    opts: &QueryOptions,
) -> Result<String> {
    let query = sq_plan::parse_query(text, opts.base_iri.as_deref())?;
    // Reject a mismatching format before doing any work.
    check_format(form_kind(&query), fmt)?;
    let mut exec = prepare(store, &query, opts, ExecCaches::default())?;
    let mut out = Vec::new();
    write_results(&mut exec, fmt, &mut out)?;
    String::from_utf8(out).map_err(|e| Error::internal(format!("result serializer produced invalid UTF-8: {e}")))
}

/// Serializes the whole result of a prepared execution: SELECT and ASK in a solution format,
/// CONSTRUCT and DESCRIBE in a graph format (`Error::Invalid` otherwise).
pub fn write_results(exec: &mut Execution, fmt: ResultFormat, w: &mut dyn Write) -> Result<()> {
    let kind = match exec.form() {
        QueryForm::Select(_) => FormKind::Select,
        QueryForm::Ask => FormKind::Ask,
        QueryForm::Construct(_) | QueryForm::Describe(_) => FormKind::Graph,
    };
    check_format(kind, fmt)?;
    match kind {
        FormKind::Select => sq_results::write_solutions(exec, fmt, w),
        FormKind::Ask => {
            let b = exec.ask()?;
            sq_results::write_boolean(b, fmt, w)
        }
        FormKind::Graph => sq_results::write_triples(exec, fmt, w),
    }
}

/// The result shape of a query form.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum FormKind {
    Select,
    Ask,
    Graph,
}

fn form_kind(q: &spargebra::Query) -> FormKind {
    match q {
        spargebra::Query::Select { .. } => FormKind::Select,
        spargebra::Query::Ask { .. } => FormKind::Ask,
        spargebra::Query::Construct { .. } | spargebra::Query::Describe { .. } => FormKind::Graph,
    }
}

fn check_format(kind: FormKind, fmt: ResultFormat) -> Result<()> {
    let fits = match kind {
        FormKind::Select | FormKind::Ask => fmt.is_solutions_format(),
        FormKind::Graph => fmt.is_graph_format(),
    };
    if fits {
        return Ok(());
    }
    let (what, expected) = match kind {
        FormKind::Select => ("SELECT", "json, xml, csv or tsv"),
        FormKind::Ask => ("ASK", "json, xml, csv or tsv"),
        FormKind::Graph => ("CONSTRUCT/DESCRIBE", "ntriples, nquads or turtle"),
    };
    Err(Error::Invalid(format!("'{}' is not a {what} result format (use {expected})", fmt.name())))
}
