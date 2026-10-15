//! EXPLAIN (`sparql_explain`): the algebra, logical and physical views of a query, and
//! EXPLAIN ANALYZE (the physical plan annotated with the counters of a complete run).
use std::fmt::Write;
use std::rc::Rc;
use std::time::Instant;

use sq_core::eval_api::{SolutionSource, TripleSource};
use sq_core::ir::QueryForm;
use sq_core::plan::QueryOptions;
use sq_core::store::QuadStore;
use sq_core::Result;
use sq_exec::{ExecCaches, Execution};
use sq_plan::ExplainMode;

/// Rows (or triples) pulled per call while running a query for EXPLAIN ANALYZE.
const DRAIN_ROWS: usize = 1024;

pub(crate) fn explain(store: Rc<dyn QuadStore>, text: &str, mode: ExplainMode, opts: &QueryOptions) -> Result<String> {
    let query = sq_plan::parse_query(text, opts.base_iri.as_deref())?;
    if mode == ExplainMode::Algebra {
        return Ok(format!("{}\n", query.to_sse()));
    }
    let lowered = sq_plan::lower_query(&query, &*store, opts)?;
    if mode == ExplainMode::Logical {
        return Ok(sq_plan::explain_logical_with(&lowered, Some(store.dict())));
    }
    let started = Instant::now();
    let plan = sq_plan::optimize(lowered, &*store, opts)?;
    let plan_ns = started.elapsed().as_nanos() as u64;
    if mode == ExplainMode::Plan {
        return Ok(sq_plan::explain_with(&plan, None, Some(store.dict())));
    }
    let shown = plan.clone();
    let mut exec = Execution::new(Rc::clone(&store), plan, opts, ExecCaches::default())?;
    exec.enable_timing();
    let produced = drain(&mut exec)?;
    let mut profile = exec.profile();
    profile.plan_ns = plan_ns;
    drop(exec);
    let mut out = sq_plan::explain_with(&shown, Some(&profile), Some(store.dict()));
    let _ = writeln!(
        out,
        "-- {produced}; planning {:.3} ms, execution {:.3} ms; decode hits {}, misses {}",
        ms(profile.plan_ns),
        ms(profile.exec_ns),
        profile.decode_hits,
        profile.decode_misses
    );
    Ok(out)
}

/// Runs the execution to completion without decoding anything; describes what it produced.
fn drain(exec: &mut Execution) -> Result<String> {
    match exec.form() {
        QueryForm::Ask => Ok(format!("ASK {}", exec.ask()?)),
        QueryForm::Select(_) => {
            let mut ids = Vec::new();
            let mut rows = 0u64;
            loop {
                ids.clear();
                let n = exec.next_rows(&mut ids, DRAIN_ROWS)?;
                if n == 0 {
                    return Ok(format!("{rows} rows"));
                }
                rows += n as u64;
            }
        }
        QueryForm::Construct(_) | QueryForm::Describe(_) => {
            let mut triples = Vec::new();
            let mut n = 0u64;
            loop {
                triples.clear();
                let got = exec.next_triples(&mut triples, DRAIN_ROWS)?;
                if got == 0 {
                    return Ok(format!("{n} triples"));
                }
                n += got as u64;
            }
        }
    }
}

fn ms(ns: u64) -> f64 {
    ns as f64 / 1e6
}
