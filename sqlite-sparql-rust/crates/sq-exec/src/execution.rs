//! `Execution`: one running query (§11). It owns the operator tree, the execution context and
//! the result form, and is a `SolutionSource` (SELECT; ASK as 0 variables and one row iff
//! true) and a `TripleSource` (CONSTRUCT / DESCRIBE).
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use rand::SeedableRng;
use rand::rngs::StdRng;
use sq_core::eval_api::{SolutionSource, TripleSource};
use sq_core::ir::{QueryForm, TemplateTerm, VarOrigin};
use sq_core::plan::{ExecProfile, Plan, QueryOptions, QueryPlan};
use sq_core::store::QuadStore;
use sq_core::{Batch, Error, Result, Slot, SlotSet, Term, TermId};

use crate::ExecCaches;
use crate::build::Builder;
use crate::construct::{Construct, Describe, Solutions};
use crate::ctx::{ExecCtx, Shared, SharedInit};
use crate::exists::ExistsState;
use crate::ops::BoxOp;
use crate::resolver::Terms;

/// The root operator read solution by solution.
struct Root {
    op: BoxOp,
    batch: Batch,
    pos: usize,
    more: bool,
    opened: bool,
    closed: bool,
}

impl Root {
    fn open(&mut self, cx: &mut ExecCtx) -> Result<()> {
        if !self.opened {
            self.opened = true;
            self.op.open(cx, None)?;
        }
        Ok(())
    }

    fn finish(&mut self, cx: &mut ExecCtx) {
        if !self.closed {
            self.closed = true;
            self.more = false;
            self.op.close(cx);
        }
    }

    /// Solutions left by counting (`fast_count` when nothing was read yet).
    fn count(&mut self, cx: &mut ExecCtx) -> Result<u64> {
        if !self.opened {
            self.open(cx)?;
            if let Some(n) = self.op.fast_count(cx) {
                let n = n?;
                self.finish(cx);
                return Ok(n);
            }
        }
        let mut n = (self.batch.len() - self.pos) as u64;
        self.pos = self.batch.len();
        while self.more {
            self.batch.clear();
            self.pos = 0;
            self.more = self.op.next(cx, &mut self.batch)?;
            n += self.batch.len() as u64;
            self.pos = self.batch.len();
        }
        self.finish(cx);
        Ok(n)
    }
}

impl Solutions for Root {
    fn next_solution(&mut self, cx: &mut ExecCtx) -> Result<Option<&[TermId]>> {
        self.open(cx)?;
        while self.pos >= self.batch.len() {
            if !self.more {
                self.finish(cx);
                return Ok(None);
            }
            self.batch.clear();
            self.pos = 0;
            self.more = self.op.next(cx, &mut self.batch)?;
        }
        let i = self.pos;
        self.pos += 1;
        Ok(Some(self.batch.row(i)))
    }
}

enum Triples {
    Construct(Construct),
    Describe(Describe),
}

/// Owns the operator tree, the store handle (in the execution context), the form and the
/// projection.
pub struct Execution {
    cx: ExecCtx,
    root: Root,
    form: QueryForm,
    vars: Vec<String>,
    proj: Vec<Slot>,
    triples: Option<Triples>,
    /// ASK through `SolutionSource`: the single row has been delivered.
    ask_done: bool,
    exec_ns: u64,
}

/// Largest plan id of a tree.
fn max_id(p: &Plan) -> u32 {
    p.children().into_iter().map(max_id).fold(p.id, u32::max)
}

impl Execution {
    pub fn new(store: Rc<dyn QuadStore>, plan: QueryPlan, opts: &QueryOptions, caches: ExecCaches) -> Result<Self> {
        // The store's dictionary already applies the connection-level caches.
        let ExecCaches {} = caches;
        let width = plan.width();
        if width > SlotSet::MAX_SLOTS {
            return Err(Error::Unsupported(format!("query needs {width} variables (at most {})", SlotSet::MAX_SLOTS)));
        }
        let builder = Builder { temps: &plan.temps, width };
        builder.check_layout(&plan.root)?;
        for e in &plan.exists {
            builder.check_layout(e)?;
        }
        let form_slots: Vec<Slot> = match &plan.form {
            QueryForm::Select(s) | QueryForm::Describe(s) => s.clone(),
            QueryForm::Ask => Vec::new(),
            QueryForm::Construct(t) => t
                .iter()
                .flatten()
                .filter_map(|x| match x {
                    TemplateTerm::Slot(s) => Some(*s),
                    _ => None,
                })
                .collect(),
        };
        if let Some(s) = form_slots.iter().find(|s| **s as usize >= width) {
            return Err(Error::internal(format!("result slot {s} outside the row layout of width {width}")));
        }
        let root = builder.build(&plan.root)?;
        let exists_ops = plan.exists.iter().map(|p| builder.build(p)).collect::<Result<Vec<_>>>()?;
        let nodes = plan.exists.iter().map(max_id).fold(max_id(&plan.root), u32::max) as usize + 1;
        let visible = plan
            .vars
            .vars
            .iter()
            .enumerate()
            .filter(|(_, (_, o))| matches!(o, VarOrigin::User | VarOrigin::Renamed))
            .map(|(i, _)| i as Slot)
            .collect();
        let (vars, proj) = match &plan.form {
            QueryForm::Select(slots) => (slots.iter().map(|s| plan.vars.name(*s).to_string()).collect(), slots.clone()),
            _ => (Vec::new(), Vec::new()),
        };
        let triples = match &plan.form {
            QueryForm::Construct(t) => Some(Triples::Construct(Construct::new(t.clone()))),
            QueryForm::Describe(s) => Some(Triples::Describe(Describe::new(s.clone()))),
            _ => None,
        };
        let exists = ExistsState::new(exists_ops, &plan.exists, width);
        let QueryPlan { form, dataset, named_graphs, temps, base_iri, .. } = plan;
        let sh = Shared::new(SharedInit {
            store,
            dataset,
            named_graphs,
            terms: Terms::new(temps),
            exists,
            base_iri: base_iri.or_else(|| opts.base_iri.clone()),
            visible,
            timeout_ms: opts.timeout_ms,
            max_memory_mb: opts.max_memory_mb,
            profile_nodes: nodes,
            rng: StdRng::from_os_rng(),
        });
        let sh = Rc::new(sh);
        let values = sh.take_values();
        Ok(Execution {
            cx: ExecCtx::new(sh, values, Vec::new()),
            root: Root { op: root, batch: Batch::new(width), pos: 0, more: true, opened: false, closed: false },
            form,
            vars,
            proj,
            triples,
            ask_done: false,
            exec_ns: 0,
        })
    }

    pub fn form(&self) -> &QueryForm {
        &self.form
    }

    /// Fixes the value of NOW() (an update request evaluates every operation with one value).
    pub fn set_now(&mut self, now: oxsdatatypes::DateTime) {
        self.cx.sh.now.set(now);
    }

    /// Records per-operator wall time in the profile (EXPLAIN ANALYZE). Off by default: it
    /// reads the clock around every `next` call.
    pub fn enable_timing(&mut self) {
        self.cx.sh.timing.set(true);
    }

    /// Bytes currently charged against the memory budget.
    pub fn memory_used(&self) -> u64 {
        self.cx.sh.mem_used()
    }

    fn timed<R>(&mut self, f: impl FnOnce(&mut Self) -> Result<R>) -> Result<R> {
        let t = Instant::now();
        let r = self.cx.sh.poll().and_then(|_| f(self));
        self.exec_ns += t.elapsed().as_nanos() as u64;
        if r.is_err() {
            self.root.finish(&mut self.cx);
        }
        r
    }

    /// True iff the query has at least one solution.
    pub fn ask(&mut self) -> Result<bool> {
        self.timed(|me| {
            let found = me.root.next_solution(&mut me.cx)?.is_some();
            me.root.finish(&mut me.cx);
            Ok(found)
        })
    }

    /// Solutions (or triples): no decode.
    pub fn count(&mut self) -> Result<u64> {
        match self.form {
            QueryForm::Ask => Ok(u64::from(self.ask()?)),
            QueryForm::Select(_) => self.timed(|me| me.root.count(&mut me.cx)),
            QueryForm::Construct(_) | QueryForm::Describe(_) => {
                let mut buf = Vec::with_capacity(1024);
                let mut n = 0u64;
                loop {
                    buf.clear();
                    let got = self.next_triples(&mut buf, 1024)?;
                    if got == 0 {
                        return Ok(n);
                    }
                    n += got as u64;
                }
            }
        }
    }

    pub fn profile(&self) -> ExecProfile {
        let terms = self.cx.sh.terms.borrow();
        ExecProfile {
            nodes: self.cx.sh.profile.borrow().clone(),
            decode_misses: terms.misses,
            decode_hits: terms.hits,
            plan_ns: 0,
            exec_ns: self.exec_ns,
        }
    }

    fn decode_ids(&mut self, ids: &[TermId], out: &mut Vec<Option<Arc<Term>>>) -> Result<()> {
        let sh = &self.cx.sh;
        sh.terms.borrow_mut().decode_batch(&*sh.store, ids, out)
    }
}

impl Drop for Execution {
    fn drop(&mut self) {
        self.root.finish(&mut self.cx);
        if let Some(t) = self.triples.as_mut() {
            match t {
                Triples::Construct(c) => c.close(&self.cx),
                Triples::Describe(d) => d.close(&mut self.cx),
            }
        }
        let sh = Rc::clone(&self.cx.sh);
        if let Ok(mut st) = sh.exists.try_borrow_mut() {
            st.close(&mut self.cx);
        }
    }
}

impl SolutionSource for Execution {
    fn variables(&self) -> &[String] {
        &self.vars
    }

    fn next_rows(&mut self, out: &mut Vec<TermId>, max_rows: usize) -> Result<usize> {
        match self.form {
            QueryForm::Select(_) => self.timed(|me| {
                let mut n = 0;
                while n < max_rows {
                    let Some(row) = me.root.next_solution(&mut me.cx)? else { break };
                    out.extend(me.proj.iter().map(|s| {
                        let v = row[*s as usize];
                        if v == TermId::ERROR { TermId::UNBOUND } else { v }
                    }));
                    n += 1;
                }
                Ok(n)
            }),
            QueryForm::Ask => {
                if self.ask_done || max_rows == 0 {
                    return Ok(0);
                }
                self.ask_done = true;
                Ok(usize::from(self.ask()?))
            }
            QueryForm::Construct(_) | QueryForm::Describe(_) => {
                Err(Error::Invalid("CONSTRUCT and DESCRIBE produce triples, not solutions".into()))
            }
        }
    }

    fn decode(&mut self, ids: &[TermId], out: &mut Vec<Option<Arc<Term>>>) -> Result<()> {
        self.decode_ids(ids, out)
    }
}

impl TripleSource for Execution {
    fn next_triples(&mut self, out: &mut Vec<[TermId; 3]>, max: usize) -> Result<usize> {
        let Some(mut t) = self.triples.take() else {
            return Err(Error::Invalid("SELECT and ASK produce solutions, not triples".into()));
        };
        let r = self.timed(|me| match &mut t {
            Triples::Construct(c) => c.next(&mut me.cx, &mut me.root, out, max),
            Triples::Describe(d) => d.next(&mut me.cx, &mut me.root, out, max),
        });
        self.triples = Some(t);
        r
    }

    fn decode(&mut self, ids: &[TermId], out: &mut Vec<Option<Arc<Term>>>) -> Result<()> {
        self.decode_ids(ids, out)
    }
}
