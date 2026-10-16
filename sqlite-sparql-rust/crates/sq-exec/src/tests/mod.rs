//! Hand-built plan tests over MemStore (no planner): every operator, join kind and seed
//! mode, paths, datasets, EXISTS, CONSTRUCT / DESCRIBE, limits, and a join-equivalence
//! property test.
use std::cell::Cell;
use std::ops::Bound;
use std::rc::Rc;
use std::sync::Arc;

use sq_core::eval_api::{SolutionSource, TempDict, TripleSource};
use sq_core::ir::{
    DatasetSpec, DefaultGraph, Expr, GraphSel, NamedGraphs, OrderKey, PatTerm, QuadPat, QueryForm, VarOrigin, VarTable,
};
use sq_core::plan::{JoinKind, Op, Plan, QueryOptions, QueryPlan, ScanOp};
use sq_core::stats::{CostConstants, Stats};
use sq_core::store::{Dictionary, IndexOrder, QuadCursor, QuadStore, QuadStoreWriter, ScanSpec, StoreCaps};
use sq_core::{Error, Literal, Result, Slot, Term, TermId};
use sq_memstore::MemStore;

use crate::{ExecCaches, Execution};

mod bench;
mod construct;
mod exists;
mod graphs;
mod joins;
mod limits;
mod modifiers;
mod paths;
mod prop;
mod review;
mod review_exec;
mod scan;
mod seeds;

pub const EX: &str = "http://ex.org/";

/// A MemStore with term helpers.
pub struct Fx {
    pub store: MemStore,
}

impl Fx {
    pub fn new() -> Self {
        Fx { store: MemStore::new() }
    }

    pub fn iri(&self, name: &str) -> TermId {
        let t = oxrdf::NamedNode::new_unchecked(format!("{EX}{name}"));
        self.store.intern(t.as_ref().into()).unwrap()
    }

    pub fn lit(&self, s: &str) -> TermId {
        let t = oxrdf::Literal::new_simple_literal(s);
        self.store.intern(t.as_ref().into()).unwrap()
    }

    pub fn typed(&self, lex: &str, dt: &str) -> TermId {
        let t = oxrdf::Literal::new_typed_literal(
            lex,
            oxrdf::NamedNode::new_unchecked(format!("{}{dt}", sq_core::codec::XSD)),
        );
        self.store.intern(t.as_ref().into()).unwrap()
    }

    pub fn bnode(&self) -> TermId {
        self.store.fresh_bnode().unwrap()
    }

    pub fn add(&self, s: TermId, p: TermId, o: TermId) {
        self.add_g(TermId::DEFAULT_GRAPH, s, p, o);
    }

    pub fn add_g(&self, g: TermId, s: TermId, p: TermId, o: TermId) {
        self.store.insert(&mut vec![[g, s, p, o]]).unwrap();
    }

    pub fn rc(&self) -> Rc<dyn QuadStore> {
        Rc::new(self.store.clone())
    }

    /// Lookup-only id of an IRI (None when absent).
    pub fn lookup(&self, name: &str) -> Option<TermId> {
        let t = oxrdf::NamedNode::new_unchecked(format!("{EX}{name}"));
        self.store.lookup(t.as_ref().into()).unwrap()
    }
}

pub fn int(v: i64) -> TermId {
    TermId::from_int(v).unwrap()
}

pub fn v(s: Slot) -> PatTerm {
    PatTerm::Slot(s)
}

pub fn c(id: TermId) -> PatTerm {
    PatTerm::Const(id)
}

pub fn plan(op: Op) -> Plan {
    Plan { id: 0, est_rows: 10.0, est_cost_ns: 0.0, sorted_by: None, op }
}

/// The planner's order choice: the first candidate for the constants, the graph and `bound`.
pub fn scan_bound(g: GraphSel, s: PatTerm, p: PatTerm, o: PatTerm, bound: &[Slot]) -> Plan {
    let is_b = |t: PatTerm| match t {
        PatTerm::Const(_) => true,
        PatTerm::Slot(x) => bound.contains(&x),
    };
    let gb = match g {
        GraphSel::Slot(x) => bound.contains(&x),
        _ => true,
    };
    let order = IndexOrder::candidates([gb, is_b(s), is_b(p), is_b(o)])[0].0;
    plan(Op::Scan(ScanOp { pat: QuadPat { g, s, p, o }, order, range: None }))
}

pub fn scan(s: PatTerm, p: PatTerm, o: PatTerm) -> Plan {
    scan_bound(GraphSel::Default, s, p, o, &[])
}

/// A scan with an explicit order.
pub fn scan_order(g: GraphSel, s: PatTerm, p: PatTerm, o: PatTerm, order: IndexOrder) -> Plan {
    plan(Op::Scan(ScanOp { pat: QuadPat { g, s, p, o }, order, range: None }))
}

pub fn values(slots: &[Slot], rows: Vec<Vec<TermId>>) -> Plan {
    plan(Op::Values { slots: slots.to_vec(), rows })
}

pub fn bind_join(left: Plan, right: Plan, seed: &[Slot], kind: JoinKind) -> Plan {
    plan(Op::BindJoin {
        left: Box::new(left),
        right: Box::new(right),
        seed: seed.to_vec(),
        kind,
        sort_left: false,
        adaptive: false,
    })
}

pub fn hash_join(build: Plan, probe: Plan, keys: &[Slot], check: &[Slot], kind: JoinKind, build_is_left: bool) -> Plan {
    plan(Op::HashJoin {
        build: Box::new(build),
        probe: Box::new(probe),
        keys: keys.to_vec(),
        check: check.to_vec(),
        kind,
        build_is_left,
    })
}

pub fn nested(left: Plan, right: Plan, check: &[Slot], kind: JoinKind) -> Plan {
    plan(Op::NestedLoop { left: Box::new(left), right: Box::new(right), check: check.to_vec(), kind })
}

pub fn merge_join(left: Plan, right: Plan, key: Slot, check: &[Slot], gallop: u32) -> Plan {
    plan(Op::MergeJoin { left: Box::new(left), right: Box::new(right), key, check: check.to_vec(), gallop })
}

pub fn sort_by(input: Plan, key: Slot) -> Plan {
    plan(Op::SortBySlot { input: Box::new(input), key })
}

pub fn filter(input: Plan, expr: Expr) -> Plan {
    plan(Op::Filter { input: Box::new(input), expr })
}

pub fn extend(input: Plan, slot: Slot, expr: Expr) -> Plan {
    plan(Op::Extend { input: Box::new(input), slot, expr, canonical: true })
}

pub fn project(input: Plan, slots: &[Slot]) -> Plan {
    plan(Op::Project { input: Box::new(input), slots: slots.to_vec() })
}

pub fn union(branches: Vec<Plan>) -> Plan {
    plan(Op::Union(branches))
}

pub fn slice(input: Plan, offset: u64, limit: Option<u64>) -> Plan {
    plan(Op::Slice { input: Box::new(input), offset, limit })
}

pub fn order_key(expr: Expr, desc: bool) -> OrderKey {
    OrderKey { expr, desc }
}

pub fn left(filter: Option<Expr>) -> JoinKind {
    JoinKind::Left { filter }
}

pub fn sv(s: Slot) -> Expr {
    Expr::Slot(s)
}

pub fn cv(id: TermId) -> Expr {
    Expr::Const(id)
}

/// Pre-order ids over the root, then the EXISTS plans.
fn assign_ids(q: &mut QueryPlan) {
    fn walk(p: &mut Plan, next: &mut u32) {
        p.id = *next;
        *next += 1;
        match &mut p.op {
            Op::BindJoin { left, right, .. }
            | Op::MergeJoin { left, right, .. }
            | Op::NestedLoop { left, right, .. }
            | Op::Minus { left, right, .. } => {
                walk(left, next);
                walk(right, next);
            }
            Op::HashJoin { build, probe, .. } => {
                walk(build, next);
                walk(probe, next);
            }
            Op::StarMerge { inputs, .. } | Op::Union(inputs) => inputs.iter_mut().for_each(|i| walk(i, next)),
            Op::Filter { input, .. }
            | Op::Extend { input, .. }
            | Op::GraphLoop { input, .. }
            | Op::Group { input, .. }
            | Op::Count { input, .. }
            | Op::Sort { input, .. }
            | Op::SortBySlot { input, .. }
            | Op::TopK { input, .. }
            | Op::Distinct { input, .. }
            | Op::Reduced { input, .. }
            | Op::Slice { input, .. }
            | Op::Project { input, .. } => walk(input, next),
            _ => {}
        }
    }
    let mut next = 0;
    walk(&mut q.root, &mut next);
    for e in &mut q.exists {
        walk(e, &mut next);
    }
}

/// A query plan builder.
pub struct Q {
    pub plan: QueryPlan,
    pub opts: QueryOptions,
}

impl Q {
    /// SELECT of every slot, `width` user variables `v0..`, the store's default graph and all
    /// named graphs of `store`.
    pub fn new(store: &dyn QuadStore, width: usize, root: Plan) -> Q {
        let vars = VarTable { vars: (0..width).map(|i| (format!("v{i}"), VarOrigin::User)).collect() };
        let plan = QueryPlan {
            form: QueryForm::Select((0..width as Slot).collect()),
            root,
            vars,
            dataset: DatasetSpec { default: DefaultGraph::StoreDefault, named: NamedGraphs::All },
            named_graphs: store.named_graphs().unwrap(),
            exists: Vec::new(),
            temps: TempDict::new(),
            base_iri: None,
        };
        Q { plan, opts: QueryOptions::default() }
    }

    pub fn form(mut self, f: QueryForm) -> Q {
        self.plan.form = f;
        self
    }

    pub fn dataset(mut self, default: DefaultGraph, named: NamedGraphs, named_graphs: Vec<TermId>) -> Q {
        self.plan.dataset = DatasetSpec { default, named };
        self.plan.named_graphs = named_graphs;
        self
    }

    pub fn exists(mut self, e: Vec<Plan>) -> Q {
        self.plan.exists = e;
        self
    }

    pub fn temps(mut self, t: TempDict) -> Q {
        self.plan.temps = t;
        self
    }

    pub fn origin(mut self, slot: Slot, o: VarOrigin) -> Q {
        self.plan.vars.vars[slot as usize].1 = o;
        self
    }

    pub fn exec(mut self, store: Rc<dyn QuadStore>) -> Execution {
        assign_ids(&mut self.plan);
        Execution::new(store, self.plan, &self.opts, ExecCaches::default()).unwrap()
    }

    pub fn try_exec(mut self, store: Rc<dyn QuadStore>) -> Result<Execution> {
        assign_ids(&mut self.plan);
        Execution::new(store, self.plan, &self.opts, ExecCaches::default())
    }

    /// All rows of the SELECT.
    pub fn rows(self, store: Rc<dyn QuadStore>) -> Vec<Vec<TermId>> {
        collect(&mut self.exec(store)).unwrap()
    }

    /// All rows, sorted (multiset comparison).
    pub fn sorted(self, store: Rc<dyn QuadStore>) -> Vec<Vec<TermId>> {
        let mut r = self.rows(store);
        r.sort();
        r
    }
}

/// Drains a SELECT execution.
pub fn collect(e: &mut Execution) -> Result<Vec<Vec<TermId>>> {
    let w = e.variables().len();
    let mut out = Vec::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        let n = e.next_rows(&mut buf, 7)?;
        if n == 0 {
            return Ok(out);
        }
        for i in 0..n {
            out.push(buf[i * w..(i + 1) * w].to_vec());
        }
    }
}

/// Drains a CONSTRUCT / DESCRIBE execution.
pub fn triples(e: &mut Execution) -> Result<Vec<[TermId; 3]>> {
    let mut out = Vec::new();
    loop {
        let n = e.next_triples(&mut out, 5)?;
        if n == 0 {
            return Ok(out);
        }
    }
}

/// N-Triples rendering of ids decoded by the execution.
pub fn show(e: &mut Execution, ids: &[TermId]) -> Vec<String> {
    let mut out = Vec::new();
    SolutionSource::decode(e, ids, &mut out).unwrap();
    out.into_iter().map(|t| t.map_or("UNDEF".to_string(), |t| t.to_ntriples())).collect()
}

pub fn u() -> TermId {
    TermId::UNBOUND
}

/// Rows as multisets: sorted copies.
pub fn ms(mut rows: Vec<Vec<TermId>>) -> Vec<Vec<TermId>> {
    rows.sort();
    rows
}

/// A QuadStore wrapper: interrupt after a number of polls, SqlJoin rows, call counters.
pub struct TestStore {
    pub inner: MemStore,
    pub interrupt_after: Cell<Option<u64>>,
    pub polls: Cell<u64>,
    pub cursors: Cell<u64>,
    pub counts: Cell<u64>,
    /// `count` calls without a cap (exact counts of a whole range).
    pub uncapped: Cell<u64>,
    pub sql: Option<(usize, Vec<TermId>)>,
}

impl TestStore {
    pub fn new(inner: MemStore) -> Self {
        TestStore {
            inner,
            interrupt_after: Cell::new(None),
            polls: Cell::new(0),
            cursors: Cell::new(0),
            counts: Cell::new(0),
            uncapped: Cell::new(0),
            sql: None,
        }
    }
}

struct VecCursor {
    width: usize,
    data: Vec<TermId>,
    pos: usize,
}

impl QuadCursor for VecCursor {
    fn width(&self) -> usize {
        self.width
    }
    fn fill(&mut self, out: &mut Vec<TermId>, max_rows: usize) -> Result<usize> {
        let rows = (self.data.len() - self.pos) / self.width.max(1);
        let n = rows.min(max_rows);
        out.extend_from_slice(&self.data[self.pos..self.pos + n * self.width]);
        self.pos += n * self.width;
        Ok(n)
    }
    fn seek(&mut self, _key: &[TermId]) -> Result<()> {
        Err(Error::Unsupported("seek".into()))
    }
    fn rebind(&mut self, _p: &[TermId], _l: Bound<i64>, _u: Bound<i64>) -> Result<()> {
        Err(Error::Unsupported("rebind".into()))
    }
    fn counters(&self) -> sq_core::store::CursorCounters {
        Default::default()
    }
}

impl QuadStore for TestStore {
    fn cursor(&self, spec: &ScanSpec) -> Result<Box<dyn QuadCursor>> {
        self.cursors.set(self.cursors.get() + 1);
        self.inner.cursor(spec)
    }
    fn count(&self, spec: &ScanSpec, cap: Option<u64>) -> Result<u64> {
        self.counts.set(self.counts.get() + 1);
        if cap.is_none() {
            self.uncapped.set(self.uncapped.get() + 1);
        }
        self.inner.count(spec, cap)
    }
    fn dict(&self) -> &dyn Dictionary {
        self.inner.dict()
    }
    fn named_graphs(&self) -> Result<Vec<TermId>> {
        self.inner.named_graphs()
    }
    fn stats(&self) -> Option<Arc<Stats>> {
        None
    }
    fn costs(&self) -> &CostConstants {
        self.inner.costs()
    }
    fn caps(&self) -> StoreCaps {
        StoreCaps { sql: self.sql.is_some(), ..Default::default() }
    }
    fn sql_rows(&self, _sql: &str, _params: &[i64], width: usize) -> Result<Box<dyn QuadCursor>> {
        let (w, data) = self.sql.clone().ok_or_else(|| Error::Unsupported("sql_rows".into()))?;
        assert_eq!(w, width);
        Ok(Box::new(VecCursor { width, data, pos: 0 }))
    }
    fn check_interrupt(&self) -> Result<()> {
        self.polls.set(self.polls.get() + 1);
        match self.interrupt_after.get() {
            Some(n) if self.polls.get() > n => Err(Error::Interrupted),
            _ => Ok(()),
        }
    }
}

/// A simple-literal term.
pub fn simple_term(s: &str) -> Term {
    Term::Literal(Literal::simple(s))
}
