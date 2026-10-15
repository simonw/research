//! CONSTRUCT and DESCRIBE (§5.7): triples generated from the solution stream.
//!
//! * CONSTRUCT instantiates the template per solution: template blank nodes become fresh
//!   TEMP blank nodes per (solution, label); triples with an unbound position, a literal
//!   subject or a predicate that is not an IRI are skipped. The output is a set, in
//!   first-seen order.
//! * DESCRIBE emits the concise bounded description of every described resource in the
//!   dataset's default graph: all triples with the resource as subject, recursing through
//!   blank-node objects.
use rustc_hash::FxHashSet;
use sq_core::ir::{GraphSel, PatTerm, QuadPat, TemplateTerm};
use sq_core::store::IndexOrder;
use sq_core::{Batch, Kind, Result, Slot, SlotSet, Tag, TermId};

use crate::ctx::{Charge, ExecCtx};
use crate::ops::Operator;
use crate::ops::scan::Scan;
use crate::seed::Seed;

/// Bytes charged per distinct output triple (set entry and pending buffer).
const TRIPLE_BYTES: usize = 56;

/// A source of solution rows (the execution's root).
pub(crate) trait Solutions {
    /// The next solution row, or None when exhausted.
    fn next_solution(&mut self, cx: &mut ExecCtx) -> Result<Option<&[TermId]>>;
}

/// Distinct triples in first-seen order.
#[derive(Default)]
struct TripleSet {
    seen: FxHashSet<[TermId; 3]>,
    pending: Vec<[TermId; 3]>,
    pos: usize,
    charge: Charge,
}

impl TripleSet {
    fn add(&mut self, cx: &ExecCtx, t: [TermId; 3]) -> Result<()> {
        if self.seen.insert(t) {
            cx.charge(&mut self.charge, TRIPLE_BYTES)?;
            self.pending.push(t);
        }
        Ok(())
    }

    /// Moves up to `max` pending triples to `out`.
    fn drain(&mut self, out: &mut Vec<[TermId; 3]>, max: usize) -> usize {
        let n = (self.pending.len() - self.pos).min(max);
        out.extend_from_slice(&self.pending[self.pos..self.pos + n]);
        self.pos += n;
        if self.pos == self.pending.len() {
            self.pending.clear();
            self.pos = 0;
        }
        n
    }

    fn has_pending(&self) -> bool {
        self.pos < self.pending.len()
    }

    fn release(&mut self, cx: &ExecCtx) {
        self.seen = FxHashSet::default();
        self.pending = Vec::new();
        self.pos = 0;
        cx.release(&mut self.charge);
    }
}

pub(crate) struct Construct {
    template: Vec<[TemplateTerm; 3]>,
    /// Fresh blank node per template label for the current solution (valid when the label's
    /// generation equals `gen`).
    bnodes: Vec<(u32, TermId)>,
    generation: u32,
    out: TripleSet,
    done: bool,
}

impl Construct {
    pub fn new(template: Vec<[TemplateTerm; 3]>) -> Self {
        let labels = template
            .iter()
            .flatten()
            .filter_map(|t| match t {
                TemplateTerm::BNode(n) => Some(*n as usize + 1),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        Construct {
            template,
            bnodes: vec![(0, TermId::UNBOUND); labels],
            generation: 0,
            out: TripleSet::default(),
            done: false,
        }
    }

    fn instantiate(&mut self, cx: &ExecCtx, row: &[TermId]) -> Result<()> {
        self.generation += 1;
        for t in &self.template {
            let mut ids = [TermId::UNBOUND; 3];
            for (k, term) in t.iter().enumerate() {
                ids[k] = match *term {
                    TemplateTerm::Const(id) => id,
                    TemplateTerm::Slot(s) => {
                        row.get(s as usize).copied().filter(|v| *v != TermId::ERROR).unwrap_or_default()
                    }
                    TemplateTerm::BNode(n) => {
                        let slot = &mut self.bnodes[n as usize];
                        if slot.0 != self.generation {
                            *slot = (self.generation, cx.sh.terms.borrow_mut().fresh_bnode(&cx.sh));
                        }
                        slot.1
                    }
                };
            }
            if ids.iter().any(|v| !v.is_bound()) || ids[0].kind() == Kind::Literal || ids[1].kind() != Kind::Iri {
                continue;
            }
            self.out.add(cx, ids)?;
        }
        Ok(())
    }

    pub fn next(
        &mut self,
        cx: &mut ExecCtx,
        src: &mut dyn Solutions,
        out: &mut Vec<[TermId; 3]>,
        max: usize,
    ) -> Result<usize> {
        let mut n = 0;
        while n < max {
            if self.out.has_pending() {
                n += self.out.drain(out, max - n);
                continue;
            }
            if self.done {
                break;
            }
            match src.next_solution(cx)? {
                Some(row) => self.instantiate(cx, row)?,
                None => self.done = true,
            }
        }
        Ok(n)
    }

    pub fn close(&mut self, cx: &ExecCtx) {
        self.out.release(cx);
    }
}

pub(crate) struct Describe {
    slots: Vec<Slot>,
    /// Described resources in first-seen order.
    resources: Vec<TermId>,
    collected: bool,
    next_resource: usize,
    scan: Scan,
    tmp: Batch,
    seed: [TermId; 3],
    queue: Vec<TermId>,
    visited: FxHashSet<TermId>,
    out: TripleSet,
    charge: Charge,
}

impl Describe {
    pub fn new(slots: Vec<Slot>) -> Self {
        let pat = QuadPat { g: GraphSel::Default, s: PatTerm::Slot(0), p: PatTerm::Slot(1), o: PatTerm::Slot(2) };
        Describe {
            slots,
            resources: Vec::new(),
            collected: false,
            next_resource: 0,
            scan: Scan::internal(pat, IndexOrder::Gspo, 3),
            tmp: Batch::new(3),
            seed: [TermId::UNBOUND; 3],
            queue: Vec::new(),
            visited: FxHashSet::default(),
            out: TripleSet::default(),
            charge: Charge::default(),
        }
    }

    fn collect(&mut self, cx: &mut ExecCtx, src: &mut dyn Solutions) -> Result<()> {
        let mut seen = FxHashSet::default();
        while let Some(row) = src.next_solution(cx)? {
            for s in &self.slots {
                let v = row[*s as usize];
                if v.is_bound() && v != TermId::ERROR && v.kind() != Kind::Literal && seen.insert(v) {
                    self.resources.push(v);
                }
            }
        }
        cx.charge(&mut self.charge, self.resources.len() * 24)?;
        self.collected = true;
        Ok(())
    }

    /// Emits the triples of subject `x`, queueing unvisited blank-node objects.
    fn describe(&mut self, cx: &mut ExecCtx, x: TermId) -> Result<()> {
        self.seed = [x, TermId::UNBOUND, TermId::UNBOUND];
        self.scan.open(cx, Some(Seed { row: &self.seed, subst: SlotSet::EMPTY }))?;
        loop {
            self.tmp.clear();
            let more = self.scan.next(cx, &mut self.tmp)?;
            for i in 0..self.tmp.len() {
                let r = self.tmp.row(i);
                let (p, o) = (r[1], r[2]);
                self.out.add(cx, [x, p, o])?;
                if o.tag() == Tag::BNode && self.visited.insert(o) {
                    cx.charge(&mut self.charge, 24)?;
                    self.queue.push(o);
                }
            }
            if !more {
                return Ok(());
            }
        }
    }

    pub fn next(
        &mut self,
        cx: &mut ExecCtx,
        src: &mut dyn Solutions,
        out: &mut Vec<[TermId; 3]>,
        max: usize,
    ) -> Result<usize> {
        if !self.collected {
            self.collect(cx, src)?;
        }
        let mut n = 0;
        while n < max {
            if self.out.has_pending() {
                n += self.out.drain(out, max - n);
                continue;
            }
            if let Some(x) = self.queue.pop() {
                self.describe(cx, x)?;
                continue;
            }
            let Some(&r) = self.resources.get(self.next_resource) else { break };
            self.next_resource += 1;
            if r.is_bnode() {
                self.visited.insert(r);
            }
            self.queue.push(r);
        }
        Ok(n)
    }

    pub fn close(&mut self, cx: &mut ExecCtx) {
        self.scan.close(cx);
        self.out.release(cx);
        self.visited = FxHashSet::default();
        cx.release(&mut self.charge);
    }
}
