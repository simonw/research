//! Running queries: the native engine (sq-engine over the connection's `SqliteStore`) or the
//! baseline translator (sq-sqlgen's SQL stepped on the same connection), behind one [`Run`]
//! whose results flow through the same output path (`SolutionSource` / `TripleSource`,
//! sq-results writers, the native value mapping).
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use sq_core::eval_api::{SolutionSource, TripleSource};
use sq_core::ir::{LoweredQuery, QueryForm};
use sq_core::plan::{EngineKind, QueryOptions};
use sq_core::store::{Dictionary, QuadCursor, QuadStore};
use sq_core::{Error, Result, Tag, Term, TermId};
use sq_exec::{ExecCaches, Execution};
use sq_results::ResultFormat;
use sq_sqlite::{RawDb, SqliteStore};

use crate::config::QuerySettings;
use crate::helpers::{Baselines, HelperCtx};
use crate::state::ConnState;
use crate::watchdog::Watchdog;

/// The result shape of a query.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Form {
    Select,
    Ask,
    /// CONSTRUCT / DESCRIBE.
    Graph,
}

impl Form {
    fn of(f: &QueryForm) -> Form {
        match f {
            QueryForm::Select(_) => Form::Select,
            QueryForm::Ask => Form::Ask,
            QueryForm::Construct(_) | QueryForm::Describe(_) => Form::Graph,
        }
    }

    /// `sparql_query`'s default format: SPARQL JSON for solutions, N-Triples for graphs.
    fn default_format(self) -> ResultFormat {
        match self {
            Form::Select | Form::Ask => ResultFormat::Json,
            Form::Graph => ResultFormat::NTriples,
        }
    }

    fn check_format(self, fmt: ResultFormat) -> Result<()> {
        let fits = match self {
            Form::Select | Form::Ask => fmt.is_solutions_format(),
            Form::Graph => fmt.is_graph_format(),
        };
        if fits {
            return Ok(());
        }
        let (what, expected) = match self {
            Form::Select => ("SELECT", "json, xml, csv or tsv"),
            Form::Ask => ("ASK", "json, xml, csv or tsv"),
            Form::Graph => ("CONSTRUCT/DESCRIBE", "ntriples, nquads or turtle"),
        };
        Err(Error::Invalid(format!("'{}' is not a {what} result format (use {expected})", fmt.name())))
    }
}

/// A prepared query, streaming its results.
pub(crate) enum Run {
    Native(Box<Execution>),
    Baseline(Box<BaselineRun>),
}

impl Run {
    pub(crate) fn form(&self) -> Form {
        match self {
            Run::Native(e) => Form::of(e.form()),
            Run::Baseline(b) => b.form,
        }
    }

    pub(crate) fn solutions(&mut self) -> &mut dyn SolutionSource {
        match self {
            Run::Native(e) => &mut **e,
            Run::Baseline(b) => &mut **b,
        }
    }

    pub(crate) fn triples(&mut self) -> Result<&mut dyn TripleSource> {
        match self {
            Run::Native(e) => Ok(&mut **e),
            Run::Baseline(_) => Err(Error::Unsupported("baseline: CONSTRUCT / DESCRIBE".into())),
        }
    }

    /// Decodes ids produced by this run (solutions or triples).
    pub(crate) fn decode(&mut self, ids: &[TermId], out: &mut Vec<Option<Arc<Term>>>) -> Result<()> {
        match self {
            Run::Native(e) if Form::of(e.form()) == Form::Graph => TripleSource::decode(&mut **e, ids, out),
            Run::Native(e) => SolutionSource::decode(&mut **e, ids, out),
            Run::Baseline(b) => b.decode(ids, out),
        }
    }

    /// Whether there is at least one solution (or triple).
    pub(crate) fn ask(&mut self) -> Result<bool> {
        if self.form() == Form::Graph {
            let mut one = Vec::with_capacity(1);
            return Ok(self.triples()?.next_triples(&mut one, 1)? > 0);
        }
        match self {
            Run::Native(e) => e.ask(),
            Run::Baseline(b) => b.any(),
        }
    }

    /// Number of solutions (or triples), with no decoding and no serialization.
    pub(crate) fn count(&mut self) -> Result<u64> {
        match self {
            Run::Native(e) => e.count(),
            Run::Baseline(b) if b.form == Form::Ask => Ok(u64::from(b.any()?)),
            Run::Baseline(b) => b.count(),
        }
    }
}

/// Parses (through the connection's LRU) and prepares `text` with `settings`.
pub(crate) fn prepare(st: &ConnState, text: &str, settings: &QuerySettings) -> Result<Run> {
    let opts = settings.options();
    let query = st.parse_query(text, opts.base_iri.as_deref())?;
    prepare_parsed(st, &query, &opts)
}

pub(crate) fn prepare_parsed(st: &ConnState, query: &spargebra::Query, opts: &QueryOptions) -> Result<Run> {
    match opts.engine {
        EngineKind::Native => {
            Ok(Run::Native(Box::new(sq_engine::prepare(st.quad_store(), query, opts, ExecCaches::default())?)))
        }
        EngineKind::Sql => {
            let lowered = sq_plan::lower_query(query, st.store(), opts)?;
            Ok(Run::Baseline(Box::new(BaselineRun::new(st, lowered, opts)?)))
        }
    }
}

/// Prepares an already lowered (and possibly rewritten) query: `sparql_view` adds its
/// pushed-down `sameTerm` filters at the IR level.
pub(crate) fn prepare_lowered(st: &ConnState, lowered: LoweredQuery, opts: &QueryOptions) -> Result<Run> {
    match opts.engine {
        EngineKind::Native => {
            let store = st.quad_store();
            let plan = sq_plan::optimize(lowered, &*store, opts)?;
            Ok(Run::Native(Box::new(Execution::new(store, plan, opts, ExecCaches::default())?)))
        }
        EngineKind::Sql => Ok(Run::Baseline(Box::new(BaselineRun::new(st, lowered, opts)?))),
    }
}

/// Serializes the whole result (`sparql_query`, `sparql_query_sql`). `fmt` None picks the
/// form's default (JSON, or N-Triples for CONSTRUCT / DESCRIBE).
///
/// `limit` is the connection's `SQLITE_LIMIT_LENGTH` ([`ConnState::length_limit`]): SQLite
/// refuses a longer function result anyway, so serialization stops as soon as the text
/// would exceed it instead of building all of it first. The memory a call can take for its
/// result is then bounded as it is for SQLite's own `group_concat`.
pub(crate) fn write_output(run: &mut Run, fmt: Option<ResultFormat>, limit: usize) -> Result<String> {
    let form = run.form();
    let fmt = fmt.unwrap_or(form.default_format());
    form.check_format(fmt)?;
    let mut out = CappedOutput { buf: Vec::with_capacity(4096.min(limit)), limit, exceeded: false };
    let written = match form {
        Form::Select => sq_results::write_solutions(run.solutions(), fmt, &mut out),
        Form::Ask => run.ask().and_then(|b| sq_results::write_boolean(b, fmt, &mut out)),
        Form::Graph => run.triples().and_then(|t| sq_results::write_triples(t, fmt, &mut out)),
    };
    if out.exceeded {
        return Err(Error::Sqlite {
            code: rusqlite::ffi::SQLITE_TOOBIG,
            msg: format!(
                "string or blob too big: the serialized result is longer than this connection's \
                 SQLITE_LIMIT_LENGTH ({limit} bytes)"
            ),
        });
    }
    written?;
    String::from_utf8(out.buf).map_err(|e| Error::internal(format!("result serializer produced invalid UTF-8: {e}")))
}

/// The output buffer of [`write_output`]: refuses (with an I/O error the caller replaces)
/// a write that would take it past `limit` bytes.
struct CappedOutput {
    buf: Vec<u8>,
    limit: usize,
    exceeded: bool,
}

impl std::io::Write for CappedOutput {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        if data.len() > self.limit - self.buf.len() {
            self.exceeded = true;
            return Err(std::io::Error::other("result longer than SQLITE_LIMIT_LENGTH"));
        }
        self.buf.extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The first projected value of the first solution (`sparql_value`; ASK gives its boolean,
/// CONSTRUCT / DESCRIBE the subject of the first triple).
pub(crate) fn first_value(run: &mut Run) -> Result<Option<Arc<Term>>> {
    let mut ids = Vec::with_capacity(16);
    let first = match run.form() {
        Form::Ask => {
            return Ok(Some(Arc::new(sq_core::codec::decode_inline(TermId::from_bool(run.ask()?)).expect("bool"))));
        }
        Form::Graph => {
            let mut triples = Vec::with_capacity(1);
            if run.triples()?.next_triples(&mut triples, 1)? == 0 {
                return Ok(None);
            }
            triples[0][0]
        }
        Form::Select => {
            let src = run.solutions();
            if src.variables().is_empty() || src.next_rows(&mut ids, 1)? == 0 {
                return Ok(None);
            }
            ids[0]
        }
    };
    if !first.is_bound() || first == TermId::ERROR {
        return Ok(None);
    }
    let mut out = Vec::with_capacity(1);
    run.decode(&[first], &mut out)?;
    Ok(out.pop().flatten())
}

// ---------------------------------------------------------------- baseline engine

/// The baseline engine's run: sq-sqlgen's SQL stepped through the store's `sql_rows`
/// cursor, with the query's [`HelperCtx`] active while the statement steps.
///
/// `timeout_ms` is checked before each batch and, since one step can take unbounded time,
/// enforced during the steps by a [`Watchdog`] (started with the first batch).
pub(crate) struct BaselineRun {
    baselines: Rc<Baselines>,
    ctx: Rc<RefCell<HelperCtx>>,
    store: SqliteStore,
    cursor: Box<dyn QuadCursor>,
    db: RawDb,
    watchdog: Option<Watchdog>,
    form: Form,
    vars: Vec<String>,
    /// Result columns of the SQL (a query projecting nothing has one placeholder column).
    width: usize,
    scratch: Vec<TermId>,
    /// `timeout_ms` from the start of the run, as in the native engine (see `QuerySettings`).
    deadline: Option<Instant>,
    /// The statement returned SQLITE_DONE.
    done: bool,
    /// ASK through `SolutionSource`: its single row was delivered.
    ask_done: bool,
}

impl BaselineRun {
    pub(crate) fn new(st: &ConnState, lowered: LoweredQuery, opts: &QueryOptions) -> Result<BaselineRun> {
        let form = Form::of(&lowered.form);
        let sql = sq_sqlgen::translate(&lowered)?;
        let width = sql.columns.len().max(1);
        let cursor = st.store().sql_rows(&sql.sql, &sql.params, width)?;
        let vars = if form == Form::Select { sql.columns } else { Vec::new() };
        Ok(BaselineRun::with_cursor(st, lowered, opts, cursor, form, vars, width))
    }

    /// `SELECT count(*) FROM (<sql>)` for `sparql_count_sql`: the counting happens in the
    /// VDBE; the helpers still run with the query's context.
    pub(crate) fn count_in_sql(st: &ConnState, lowered: LoweredQuery, opts: &QueryOptions) -> Result<u64> {
        let sql = sq_sqlgen::translate(&lowered)?;
        let counting = format!("SELECT count(*) FROM ({})", sql.sql);
        let cursor = st.store().sql_rows(&counting, &sql.params, 1)?;
        let mut run = BaselineRun::with_cursor(st, lowered, opts, cursor, Form::Select, Vec::new(), 1);
        let mut out = Vec::with_capacity(1);
        run.fill(&mut out, 1)?;
        Ok(out.first().map_or(0, |n| n.raw().max(0) as u64))
    }

    fn with_cursor(
        st: &ConnState,
        lowered: LoweredQuery,
        opts: &QueryOptions,
        cursor: Box<dyn QuadCursor>,
        form: Form,
        vars: Vec<String>,
        width: usize,
    ) -> BaselineRun {
        let base = lowered.base_iri.or_else(|| opts.base_iri.clone());
        BaselineRun {
            baselines: st.baselines.clone(),
            ctx: Rc::new(RefCell::new(HelperCtx::new(lowered.temps, base))),
            store: st.store().clone(),
            cursor,
            db: st.shared().db(),
            watchdog: None,
            form,
            vars,
            width,
            scratch: Vec::new(),
            deadline: opts.timeout_ms.map(|ms| Instant::now() + Duration::from_millis(ms)),
            done: false,
            ask_done: false,
        }
    }

    /// Steps the statement with this query's helper context active (rule R1: no borrow of
    /// the context is held while SQLite runs the helpers) and the watchdog armed.
    fn fill(&mut self, out: &mut Vec<TermId>, max_rows: usize) -> Result<usize> {
        if self.done || max_rows == 0 {
            return Ok(0);
        }
        if let Some(deadline) = self.deadline {
            if Instant::now() >= deadline {
                return Err(Error::Timeout);
            }
            if self.watchdog.is_none() {
                self.watchdog = Some(Watchdog::start(self.db, deadline)?);
            }
        }
        let _active = self.baselines.activate(&self.ctx)?;
        let armed = self.watchdog.as_ref().map(Watchdog::arm);
        let stepped = self.cursor.fill(out, max_rows);
        drop(armed);
        if self.watchdog.as_ref().is_some_and(Watchdog::fired) {
            // The deadline passed mid-step: the connection is interrupted, whatever the step
            // returned.
            return Err(Error::Timeout);
        }
        let n = stepped.map_err(helper_interrupt)?;
        if n == 0 {
            self.done = true;
        }
        Ok(n)
    }

    fn any(&mut self) -> Result<bool> {
        let mut scratch = std::mem::take(&mut self.scratch);
        scratch.clear();
        let r = self.fill(&mut scratch, 1);
        self.scratch = scratch;
        Ok(r? > 0)
    }

    fn count(&mut self) -> Result<u64> {
        let mut scratch = std::mem::take(&mut self.scratch);
        let mut n = 0u64;
        let r = loop {
            scratch.clear();
            match self.fill(&mut scratch, sq_core::BATCH_ROWS) {
                Ok(0) => break Ok(n),
                Ok(k) => n += k as u64,
                Err(e) => break Err(e),
            }
        };
        self.scratch = scratch;
        r
    }
}

/// An interrupt that reaches a helper function fails the statement with SQLITE_ERROR and the
/// helper's message (scalar functions cannot set their result code): still an interrupt.
fn helper_interrupt(e: Error) -> Error {
    match e {
        Error::Sqlite { msg, .. } if msg == Error::Interrupted.to_string() => Error::Interrupted,
        e => e,
    }
}

impl SolutionSource for BaselineRun {
    fn variables(&self) -> &[String] {
        &self.vars
    }

    fn next_rows(&mut self, out: &mut Vec<TermId>, max_rows: usize) -> Result<usize> {
        match self.form {
            Form::Ask => {
                if self.ask_done || max_rows == 0 {
                    return Ok(0);
                }
                self.ask_done = true;
                Ok(usize::from(self.any()?))
            }
            _ if self.vars.is_empty() => {
                // Only the placeholder column: rows without ids.
                let mut scratch = std::mem::take(&mut self.scratch);
                scratch.clear();
                let r = self.fill(&mut scratch, max_rows);
                self.scratch = scratch;
                r
            }
            _ => {
                debug_assert_eq!(self.width, self.vars.len());
                self.fill(out, max_rows)
            }
        }
    }

    /// Store ids through the store's (cached, batched) dictionary; TEMP ids through the
    /// query's TempDict.
    fn decode(&mut self, ids: &[TermId], out: &mut Vec<Option<Arc<Term>>>) -> Result<()> {
        let base = out.len();
        out.resize(base + ids.len(), None);
        let mut stored: Vec<TermId> = Vec::new();
        let mut at: Vec<usize> = Vec::new();
        {
            let ctx = self.ctx.try_borrow().map_err(|_| Error::Reentrant)?;
            for (i, &id) in ids.iter().enumerate() {
                match id.tag() {
                    Tag::Unbound | Tag::Error => {}
                    Tag::Temp => {
                        let t = ctx.temps.get(id).ok_or_else(|| Error::internal(format!("unknown TEMP id {id:?}")))?;
                        out[base + i] = Some(t.clone());
                    }
                    _ => {
                        stored.push(id);
                        at.push(base + i);
                    }
                }
            }
        }
        if !stored.is_empty() {
            let mut terms = Vec::with_capacity(stored.len());
            self.store.decode(&stored, &mut terms)?;
            for (pos, t) in at.into_iter().zip(terms) {
                out[pos] = Some(t);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_follow_the_query_form() {
        assert_eq!(Form::Select.default_format(), ResultFormat::Json);
        assert_eq!(Form::Ask.default_format(), ResultFormat::Json);
        assert_eq!(Form::Graph.default_format(), ResultFormat::NTriples);
        for fmt in ResultFormat::ALL {
            assert_eq!(Form::Select.check_format(fmt).is_ok(), fmt.is_solutions_format(), "{fmt:?}");
            assert_eq!(Form::Graph.check_format(fmt).is_ok(), fmt.is_graph_format(), "{fmt:?}");
        }
        let e = Form::Ask.check_format(ResultFormat::Turtle).unwrap_err().to_string();
        assert!(e.contains("'turtle' is not a ASK result format"), "{e}");
    }

    #[test]
    fn interrupted_helpers_are_interrupts() {
        let helper = Error::Sqlite { code: 1, msg: Error::Interrupted.to_string() };
        assert!(matches!(helper_interrupt(helper), Error::Interrupted));
        let other = Error::Sqlite { code: 1, msg: "sparql: type error".into() };
        assert!(matches!(helper_interrupt(other), Error::Sqlite { .. }));
        assert!(matches!(helper_interrupt(Error::Timeout), Error::Timeout));
    }
}
