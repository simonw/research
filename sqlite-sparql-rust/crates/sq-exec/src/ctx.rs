//! Execution context (§5.1).
//!
//! [`Shared`] is the per-execution state every evaluation level sees: the store, the term
//! state behind the resolver, the dataset, the EXISTS plans and memo, `now`, the RNG, the
//! deadline, the memory budget, the interrupt-polling counter and the profile. It lives in an
//! `Rc` so that an EXISTS evaluation nested inside an expression evaluation can build its own
//! [`ExecCtx`] while the outer level's `EvalCtx` is still alive.
//!
//! [`ExecCtx`] is one evaluation level: the shared state plus the level's `ValueCache` and the
//! graph register stack (`GraphSel::Active`).
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use rand::rngs::StdRng;
use sq_core::ir::DatasetSpec;
use sq_core::plan::NodeProfile;
use sq_core::store::QuadStore;
use sq_core::{Error, Result, Slot, TermId};
use sq_expr::{EvalCtx, ValueCache};

use crate::exists::{ExistsHook, ExistsState};
use crate::resolver::{Resolver, Terms};

/// CPU loops poll for interrupts and the deadline every this many rows of work.
pub(crate) const POLL_ROWS: u64 = 4096;

/// Bytes charged against the memory budget by one operator (released on close / re-open).
#[derive(Debug, Default)]
pub(crate) struct Charge(u64);

/// Per-execution state shared by every evaluation level.
pub(crate) struct Shared {
    pub store: Rc<dyn QuadStore>,
    pub dataset: DatasetSpec,
    /// Named graphs of the dataset, sorted.
    pub named_graphs: Vec<TermId>,
    pub terms: RefCell<Terms>,
    pub exists: RefCell<ExistsState>,
    pub base_iri: Option<String>,
    pub now: Cell<oxsdatatypes::DateTime>,
    pub rng: RefCell<StdRng>,
    /// User-visible slots (user variables and renamed sub-select variables): what `SELECT *`
    /// projects; `COUNT(DISTINCT *)` dedupes rows projected onto them.
    pub visible: Vec<Slot>,
    deadline: Option<Instant>,
    mem_used: Cell<u64>,
    mem_limit: u64,
    mem_mb: u64,
    work: Cell<u64>,
    next_poll: Cell<u64>,
    pub profile: RefCell<Vec<NodeProfile>>,
    pub timing: Cell<bool>,
    values_pool: RefCell<Vec<ValueCache>>,
}

/// Construction parameters of [`Shared`].
pub(crate) struct SharedInit {
    pub store: Rc<dyn QuadStore>,
    pub dataset: DatasetSpec,
    pub named_graphs: Vec<TermId>,
    pub terms: Terms,
    pub exists: ExistsState,
    pub base_iri: Option<String>,
    pub visible: Vec<Slot>,
    pub timeout_ms: Option<u64>,
    pub max_memory_mb: u64,
    pub profile_nodes: usize,
    pub rng: StdRng,
}

impl Shared {
    pub fn new(i: SharedInit) -> Self {
        let mut named_graphs = i.named_graphs;
        named_graphs.sort();
        named_graphs.dedup();
        Shared {
            store: i.store,
            dataset: i.dataset,
            named_graphs,
            terms: RefCell::new(i.terms),
            exists: RefCell::new(i.exists),
            base_iri: i.base_iri,
            now: Cell::new(oxsdatatypes::DateTime::now()),
            rng: RefCell::new(i.rng),
            visible: i.visible,
            deadline: i.timeout_ms.map(|ms| Instant::now() + std::time::Duration::from_millis(ms)),
            mem_used: Cell::new(0),
            mem_limit: i.max_memory_mb.saturating_mul(1 << 20),
            mem_mb: i.max_memory_mb,
            work: Cell::new(0),
            next_poll: Cell::new(POLL_ROWS),
            profile: RefCell::new(vec![NodeProfile::default(); i.profile_nodes]),
            timing: Cell::new(false),
            values_pool: RefCell::new(Vec::new()),
        }
    }

    /// Checks the interrupt flag and the deadline.
    pub fn poll(&self) -> Result<()> {
        self.store.check_interrupt()?;
        self.check_deadline()
    }

    pub fn check_deadline(&self) -> Result<()> {
        match self.deadline {
            Some(d) if Instant::now() >= d => Err(Error::Timeout),
            _ => Ok(()),
        }
    }

    /// Records `n` rows of CPU work; polls every [`POLL_ROWS`] rows.
    #[inline]
    pub fn tick(&self, n: usize) -> Result<()> {
        let w = self.work.get() + n as u64;
        self.work.set(w);
        if w >= self.next_poll.get() {
            self.next_poll.set(w + POLL_ROWS);
            self.poll()?;
        }
        Ok(())
    }

    /// Charges `bytes` to `c` and to the execution's memory budget.
    #[inline]
    pub fn charge(&self, c: &mut Charge, bytes: usize) -> Result<()> {
        let used = self.mem_used.get() + bytes as u64;
        self.mem_used.set(used);
        c.0 += bytes as u64;
        if used > self.mem_limit {
            return Err(Error::MemoryBudget(self.mem_mb));
        }
        Ok(())
    }

    /// Releases everything charged to `c`.
    pub fn release(&self, c: &mut Charge) {
        self.mem_used.set(self.mem_used.get().saturating_sub(c.0));
        c.0 = 0;
    }

    pub fn mem_used(&self) -> u64 {
        self.mem_used.get()
    }

    pub fn is_named_graph(&self, g: TermId) -> bool {
        self.named_graphs.binary_search(&g).is_ok()
    }

    pub fn take_values(&self) -> ValueCache {
        self.values_pool.borrow_mut().pop().unwrap_or_default()
    }

    pub fn return_values(&self, mut v: ValueCache) {
        v.clear();
        self.values_pool.borrow_mut().push(v);
    }

    #[inline]
    pub fn prof(&self, id: Option<u32>, f: impl FnOnce(&mut NodeProfile)) {
        if let Some(id) = id
            && let Some(n) = self.profile.borrow_mut().get_mut(id as usize)
        {
            f(n);
        }
    }
}

/// `rand::RngCore` over the shared RNG (borrowed per call).
struct SharedRng<'a>(&'a RefCell<StdRng>);

impl rand::RngCore for SharedRng<'_> {
    fn next_u32(&mut self) -> u32 {
        self.0.borrow_mut().next_u32()
    }
    fn next_u64(&mut self) -> u64 {
        self.0.borrow_mut().next_u64()
    }
    fn fill_bytes(&mut self, dst: &mut [u8]) {
        self.0.borrow_mut().fill_bytes(dst)
    }
}

/// One evaluation level (the query, or one EXISTS evaluation).
pub(crate) struct ExecCtx {
    pub sh: Rc<Shared>,
    pub values: ValueCache,
    /// Graph register stack (`GraphSel::Active` reads the top).
    pub graphs: Vec<TermId>,
}

impl ExecCtx {
    pub fn new(sh: Rc<Shared>, values: ValueCache, graphs: Vec<TermId>) -> Self {
        ExecCtx { sh, values, graphs }
    }

    #[inline]
    pub fn store(&self) -> &dyn QuadStore {
        &*self.sh.store
    }

    #[inline]
    pub fn active_graph(&self) -> Option<TermId> {
        self.graphs.last().copied()
    }

    #[inline]
    pub fn tick(&self, n: usize) -> Result<()> {
        self.sh.tick(n)
    }

    #[inline]
    pub fn charge(&self, c: &mut Charge, bytes: usize) -> Result<()> {
        self.sh.charge(c, bytes)
    }

    #[inline]
    pub fn release(&self, c: &mut Charge) {
        self.sh.release(c)
    }

    /// Runs `f` with an expression-evaluation context for this level.
    pub fn with_eval<R>(&mut self, f: impl FnOnce(&mut EvalCtx<'_>) -> R) -> R {
        let sh: &Shared = &self.sh;
        let mut resolver = Resolver { sh };
        let mut exists = ExistsHook { sh: &self.sh, graph: self.graphs.last().copied() };
        let mut rng = SharedRng(&sh.rng);
        let base_iri = sh.base_iri.as_deref();
        let mut ecx = EvalCtx {
            resolver: &mut resolver,
            exists: &mut exists,
            values: &mut self.values,
            now: sh.now.get(),
            base_iri,
            rng: &mut rng,
        };
        f(&mut ecx)
    }
}
