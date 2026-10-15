//! `Op::Scan` (§5.3): one quad pattern over range cursors.
//!
//! At `open` the bound positions are computed from the constants, the seeded slots and the
//! graph selection. When the planned order's prefix is exactly the bound columns the planned
//! order is used (and a cursor of the same shape is `rebind`-ed, which allocates nothing);
//! otherwise (Substitute-mode seeds vary per row) `IndexOrder::candidates(bound)[0]` is used.
//! Free columns map to slots; repeated variables (`?x :p ?x`), the INT tag of an
//! [`IntRange`] scan and FROM NAMED membership are post-checks. A TEMP (or otherwise
//! non-storable) bound value means "absent from the store": 0 rows, no cursor.
//!
//! Graph selection:
//! * default graph `StoreDefault` (g = 0), `Graphs([g])`, `GraphSel::Const`, `Active` (the
//!   GraphLoop register) and a seeded `GraphSel::Slot`: one g-first cursor;
//! * `Graphs([g1..gk])`: a k-way merge of g-first cursors with an adjacent dedupe (the RDF
//!   merge of the FROM graphs is a set);
//! * `UnionAll`: a 2-way merge of the g = 0 cursor and the partner g-last partial cursor (GSPO
//!   with SPOG, GPSO with PSOG, GPOS with POSG, GOSP with OSPG) with g projected away and an
//!   adjacent dedupe;
//! * free `GraphSel::Slot`: with `NamedGraphs::All` the g-last partial cursor binds the slot;
//!   with `Only(list)` and at most 8 graphs one g-first cursor per graph, otherwise the partial
//!   cursor plus a membership check.
use std::ops::Bound;

use smallvec::SmallVec;
use sq_core::ir::{DefaultGraph, GraphSel, NamedGraphs, PatTerm, QuadPat};
use sq_core::plan::IntRange;
use sq_core::store::{Col, IndexOrder, QuadCursor, ScanSpec};
use sq_core::{BATCH_ROWS, Batch, Error, Result, Slot, Tag, TermId};

use super::Operator;
use crate::ctx::ExecCtx;
use crate::seed::{Seed, SeedBuf};

/// `Only(list)` graph selections up to this size use one g-first cursor per graph.
const PER_GRAPH_MAX: usize = 8;
/// Rows read per refill of a merged source.
const MERGE_CHUNK: usize = 256;

/// What to do with one free column of a cursor row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Act {
    /// Write the value into the slot (first occurrence of the slot).
    Bind(Slot),
    /// Repeated variable: the value must equal the slot's (already written) value.
    Check(Slot),
    /// IntRange scan: the value must be an inline INT.
    IntTag,
    /// The value must be one of the dataset's named graphs.
    Member,
}

/// Access path of the current open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Access {
    Empty,
    /// One cursor (`srcs[0]`).
    Single,
    /// `srcs[0]` over the INT range on GPOS, then the numeric exceptions on GPOX.
    Range {
        phase: u8,
    },
    /// One g-first cursor per FROM NAMED graph, scanned one after another; the graph slot is
    /// bound in the output image.
    PerGraph {
        slot: Slot,
    },
    /// `srcs[..n]` merged in suffix order with an adjacent dedupe.
    Merge {
        n: usize,
    },
}

/// Value of a pattern position at open time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pos {
    Bound(TermId),
    /// Bound to the graph of the cursor (merged or per-graph access).
    Graph,
    Free(Slot),
}

/// A positioned cursor and its read buffer.
struct Source {
    order: IndexOrder,
    prefix_len: u8,
    cursor: Box<dyn QuadCursor>,
    buf: Vec<TermId>,
    /// Values per buffered row (cursor width; the partner cursor has a trailing g column).
    stride: usize,
    pos: usize,
    rows: usize,
    done: bool,
}

impl Source {
    #[inline]
    fn row(&self, i: usize) -> &[TermId] {
        &self.buf[i * self.stride..(i + 1) * self.stride]
    }

    fn refill(&mut self, max: usize) -> Result<usize> {
        self.buf.clear();
        self.pos = 0;
        self.rows = self.cursor.fill(&mut self.buf, max.max(1))?;
        if self.rows == 0 {
            self.done = true;
        }
        Ok(self.rows)
    }
}

pub(crate) struct Scan {
    id: Option<u32>,
    pat: QuadPat,
    planned: IndexOrder,
    range: Option<IntRange>,
    seed: SeedBuf,
    access: Access,
    acts: SmallVec<[(u8, Act); 6]>,
    /// Any post-check act (rows can be dropped): disables `fast_count`.
    checks: bool,
    /// Free columns compared by the merge / used by the acts.
    fw: usize,
    /// The order and prefix of the current access (per-graph and range accesses re-position
    /// `srcs[0]` from them).
    order: IndexOrder,
    prefix: [TermId; 4],
    prefix_len: usize,
    /// Prefix positions holding the cursor's graph (`Pos::Graph`).
    graph_cols: SmallVec<[usize; 2]>,
    srcs: Vec<Source>,
    /// Graphs of a per-graph or merged access.
    graphs: Vec<TermId>,
    gnext: usize,
    /// Output row image: the seed, plus the graph slot of a per-graph access.
    base: Vec<TermId>,
    last: [TermId; 4],
    has_last: bool,
}

/// The g-last partial order with the same suffix as a g-first order.
fn partner(o: IndexOrder) -> IndexOrder {
    match o {
        IndexOrder::Gspo => IndexOrder::Spog,
        IndexOrder::Gpso => IndexOrder::Psog,
        IndexOrder::Gpos => IndexOrder::Posg,
        IndexOrder::Gosp => IndexOrder::Ospg,
        other => other,
    }
}

#[inline]
fn emit(out: &mut Batch, base: &[TermId], acts: &[(u8, Act)], named: &[TermId], row: &[TermId]) {
    out.push_row(base);
    let i = out.len() - 1;
    let r = out.row_mut(i);
    for &(c, act) in acts {
        let v = row[c as usize];
        let ok = match act {
            Act::Bind(s) => {
                r[s as usize] = v;
                true
            }
            Act::Check(s) => r[s as usize] == v,
            Act::IntTag => v.tag() == Tag::Int,
            Act::Member => named.binary_search(&v).is_ok(),
        };
        if !ok {
            out.truncate(i);
            return;
        }
    }
}

impl Scan {
    pub fn new(id: Option<u32>, pat: QuadPat, planned: IndexOrder, range: Option<IntRange>, width: usize) -> Self {
        Scan {
            id,
            pat,
            planned,
            range,
            seed: SeedBuf::new(width),
            access: Access::Empty,
            acts: SmallVec::new(),
            checks: false,
            fw: 0,
            order: planned,
            prefix: [TermId::UNBOUND; 4],
            prefix_len: 0,
            graph_cols: SmallVec::new(),
            srcs: Vec::new(),
            graphs: Vec::new(),
            gnext: 0,
            base: vec![TermId::UNBOUND; width],
            last: [TermId::UNBOUND; 4],
            has_last: false,
        }
    }

    /// An internal scan (paths, DESCRIBE): no profile entry; `width` is the private row width.
    pub fn internal(pat: QuadPat, planned: IndexOrder, width: usize) -> Self {
        Scan::new(None, pat, planned, None, width)
    }

    /// Positions source `i` on `spec` with the given stride, re-binding a cursor of the same
    /// order and prefix length when there is one.
    fn position(&mut self, cx: &ExecCtx, i: usize, spec: &ScanSpec, stride: usize) -> Result<()> {
        cx.sh.prof(self.id, |n| n.seeks += 1);
        let prefix = &spec.prefix[..spec.prefix_len as usize];
        if let Some(s) = self.srcs.get_mut(i)
            && s.order == spec.order
            && s.prefix_len == spec.prefix_len
        {
            s.cursor.rebind(prefix, spec.lower, spec.upper)?;
            s.stride = stride;
            s.buf.clear();
            s.pos = 0;
            s.rows = 0;
            s.done = false;
            return Ok(());
        }
        let src = Source {
            order: spec.order,
            prefix_len: spec.prefix_len,
            cursor: cx.store().cursor(spec)?,
            buf: Vec::new(),
            stride,
            pos: 0,
            rows: 0,
            done: false,
        };
        if i < self.srcs.len() {
            self.srcs[i] = src;
        } else {
            debug_assert_eq!(i, self.srcs.len());
            self.srcs.push(src);
        }
        Ok(())
    }

    /// The prefix of the current access with the graph positions set to `g`.
    fn prefix_for(&self, g: TermId) -> [TermId; 4] {
        let mut p = self.prefix;
        for &c in &self.graph_cols {
            p[c] = g;
        }
        p
    }

    fn spec_for(&self, g: TermId) -> ScanSpec {
        let p = self.prefix_for(g);
        ScanSpec::new(self.order, &p[..self.prefix_len])
    }

    fn range_spec(&self, phase: u8) -> ScanSpec {
        let r = self.range.expect("range access without a range");
        if phase == 0 {
            let (lo, hi) = if r.lo <= r.hi { TermId::int_raw_range(r.lo, r.hi) } else { (1, 0) };
            ScanSpec::new(IndexOrder::Gpos, &self.prefix[..2]).with_range(Bound::Included(lo), Bound::Included(hi))
        } else {
            ScanSpec::new(IndexOrder::Gpox, &self.prefix[..2])
        }
    }

    /// Sets up the access path for the current seed.
    fn plan_access(&mut self, cx: &ExecCtx) -> Result<()> {
        self.access = Access::Empty;
        self.acts.clear();
        self.checks = false;
        self.graph_cols.clear();
        self.graphs.clear();
        self.gnext = 0;
        self.has_last = false;
        let sh = &*cx.sh;

        enum Gm {
            Empty,
            Fixed(TermId),
            Merge,
            UnionAll,
            Named { member: bool },
            PerGraph(Slot),
        }
        let gm = match self.pat.g {
            GraphSel::Default => match &sh.dataset.default {
                DefaultGraph::StoreDefault => Gm::Fixed(TermId::DEFAULT_GRAPH),
                DefaultGraph::UnionAll => Gm::UnionAll,
                DefaultGraph::Graphs(gs) => {
                    // Graphs absent from the store (TEMP ids) are empty.
                    self.graphs.extend(gs.iter().copied().filter(|g| g.is_storable()));
                    self.graphs.sort();
                    self.graphs.dedup();
                    match self.graphs.len() {
                        0 => Gm::Empty,
                        1 => Gm::Fixed(self.graphs[0]),
                        _ => Gm::Merge,
                    }
                }
            },
            GraphSel::Active => {
                Gm::Fixed(cx.active_graph().ok_or_else(|| Error::internal("GraphSel::Active outside of a GraphLoop"))?)
            }
            GraphSel::Const(g) => Gm::Fixed(g),
            GraphSel::Slot(gs) => match self.seed.value(gs) {
                Some(v) if sh.is_named_graph(v) => Gm::Fixed(v),
                Some(_) => Gm::Empty,
                None => match &sh.dataset.named {
                    NamedGraphs::All => Gm::Named { member: false },
                    NamedGraphs::Only(_) if sh.named_graphs.len() <= PER_GRAPH_MAX => {
                        self.graphs.extend(sh.named_graphs.iter().copied().filter(|g| g.is_storable()));
                        Gm::PerGraph(gs)
                    }
                    NamedGraphs::Only(_) => Gm::Named { member: true },
                },
            },
        };
        let g_pos = match gm {
            Gm::Empty => return Ok(()),
            Gm::Fixed(g) if g != TermId::DEFAULT_GRAPH && !g.is_storable() => return Ok(()),
            Gm::Fixed(g) => Pos::Bound(g),
            Gm::Merge | Gm::UnionAll | Gm::PerGraph(_) => Pos::Graph,
            Gm::Named { .. } => match self.pat.g {
                GraphSel::Slot(gs) => Pos::Free(gs),
                _ => unreachable!("named-graph access needs a graph slot"),
            },
        };
        let per_graph_slot = match gm {
            Gm::PerGraph(gs) => Some(gs),
            _ => None,
        };
        let term = |t: PatTerm| match t {
            PatTerm::Const(id) => Pos::Bound(id),
            PatTerm::Slot(s) => match self.seed.value(s) {
                Some(v) => Pos::Bound(v),
                None if per_graph_slot == Some(s) => Pos::Graph,
                None => Pos::Free(s),
            },
        };
        let pos = [g_pos, term(self.pat.s), term(self.pat.p), term(self.pat.o)];
        if pos[1..].iter().any(|p| matches!(p, Pos::Bound(v) if !v.is_storable())) {
            return Ok(());
        }
        let mask = pos.map(|p| !matches!(p, Pos::Free(_)));
        let k = mask.iter().filter(|b| **b).count();
        let valid = |o: IndexOrder| {
            !o.numeric_exceptions() && o.named_only() != mask[0] && o.cols()[..k].iter().all(|c| mask[*c as usize])
        };
        let order = if valid(self.planned) {
            self.planned
        } else {
            IndexOrder::candidates(mask).first().map(|c| c.0).ok_or_else(|| Error::internal("no index order"))?
        };
        self.order = order;
        self.prefix_len = k;
        self.prefix = [TermId::UNBOUND; 4];
        for (i, c) in order.cols()[..k].iter().enumerate() {
            match pos[*c as usize] {
                Pos::Bound(v) => self.prefix[i] = v,
                Pos::Graph => self.graph_cols.push(i),
                Pos::Free(_) => unreachable!("prefix columns are bound"),
            }
        }
        let mut assigned: SmallVec<[Slot; 4]> = SmallVec::new();
        for (i, c) in order.cols()[k..].iter().enumerate() {
            let Pos::Free(s) = pos[*c as usize] else { unreachable!("suffix columns are free") };
            if assigned.contains(&s) {
                self.acts.push((i as u8, Act::Check(s)));
                self.checks = true;
            } else {
                self.acts.push((i as u8, Act::Bind(s)));
                assigned.push(s);
            }
            if *c == Col::G && matches!(gm, Gm::Named { member: true }) {
                self.acts.push((i as u8, Act::Member));
                self.checks = true;
            }
        }
        self.fw = 4 - k;
        match gm {
            Gm::Fixed(g) => {
                if self.range.is_some() && order == IndexOrder::Gpos && k == 2 {
                    self.acts.insert(0, (0, Act::IntTag));
                    self.checks = true;
                    let spec = self.range_spec(0);
                    self.position(cx, 0, &spec, self.fw)?;
                    self.access = Access::Range { phase: 0 };
                } else {
                    let spec = self.spec_for(g);
                    self.position(cx, 0, &spec, self.fw)?;
                    self.access = Access::Single;
                }
            }
            Gm::Named { .. } => {
                let spec = ScanSpec::new(order, &self.prefix[..k]);
                self.position(cx, 0, &spec, self.fw)?;
                self.access = Access::Single;
            }
            Gm::PerGraph(gs) => {
                // Sources are positioned lazily, one graph at a time.
                self.access = if self.graphs.is_empty() { Access::Empty } else { Access::PerGraph { slot: gs } };
                self.gnext = 0;
                if let Some(s) = self.srcs.first_mut() {
                    s.done = true;
                    s.pos = 0;
                    s.rows = 0;
                }
            }
            Gm::Merge => {
                for i in 0..self.graphs.len() {
                    let spec = self.spec_for(self.graphs[i]);
                    self.position(cx, i, &spec, self.fw)?;
                }
                self.access = Access::Merge { n: self.graphs.len() };
            }
            Gm::UnionAll => {
                let spec = self.spec_for(TermId::DEFAULT_GRAPH);
                self.position(cx, 0, &spec, self.fw)?;
                let p = self.prefix_for(TermId::DEFAULT_GRAPH);
                let named = ScanSpec::new(partner(order), &p[1..k]);
                self.position(cx, 1, &named, self.fw + 1)?;
                self.access = Access::Merge { n: 2 };
            }
            Gm::Empty => unreachable!(),
        }
        Ok(())
    }

    /// Reads `srcs[0]` into `out` until `out` reaches `limit` rows (true) or the source is
    /// exhausted (false).
    fn next_single(&mut self, cx: &ExecCtx, out: &mut Batch, limit: usize) -> Result<bool> {
        let named = &cx.sh.named_graphs[..];
        loop {
            if out.len() >= limit {
                return Ok(true);
            }
            let s = &mut self.srcs[0];
            if s.pos == s.rows {
                if s.done {
                    return Ok(false);
                }
                let n = s.refill(limit - out.len())?;
                cx.tick(n)?;
                if n == 0 {
                    return Ok(false);
                }
            }
            while s.pos < s.rows && out.len() < limit {
                emit(out, &self.base, &self.acts, named, s.row(s.pos));
                s.pos += 1;
            }
        }
    }

    fn next_merge(&mut self, cx: &ExecCtx, out: &mut Batch, n: usize, limit: usize) -> Result<bool> {
        let named = &cx.sh.named_graphs[..];
        let fw = self.fw;
        loop {
            if out.len() >= limit {
                return Ok(true);
            }
            for s in &mut self.srcs[..n] {
                if s.pos == s.rows && !s.done {
                    let read = s.refill(MERGE_CHUNK)?;
                    cx.tick(read)?;
                }
            }
            let mut best: Option<usize> = None;
            for i in 0..n {
                let s = &self.srcs[i];
                if s.pos < s.rows {
                    let r = &s.row(s.pos)[..fw];
                    if best.is_none_or(|b| r < &self.srcs[b].row(self.srcs[b].pos)[..fw]) {
                        best = Some(i);
                    }
                }
            }
            let Some(b) = best else { return Ok(false) };
            let mut cur = [TermId::UNBOUND; 4];
            {
                let s = &self.srcs[b];
                cur[..fw].copy_from_slice(&s.row(s.pos)[..fw]);
            }
            self.srcs[b].pos += 1;
            if !(self.has_last && self.last[..fw] == cur[..fw]) {
                emit(out, &self.base, &self.acts, named, &cur[..fw]);
                self.last = cur;
                self.has_last = true;
            }
        }
    }

    /// Positions `srcs[0]` on the next per-graph source; false when there is none.
    fn next_graph(&mut self, cx: &ExecCtx, slot: Slot) -> Result<bool> {
        let Some(&g) = self.graphs.get(self.gnext) else { return Ok(false) };
        self.gnext += 1;
        let spec = self.spec_for(g);
        self.position(cx, 0, &spec, self.fw)?;
        self.base[slot as usize] = g;
        Ok(true)
    }

    /// `next` with a row limit (`out.len() + max` rows at most).
    fn read(&mut self, cx: &mut ExecCtx, out: &mut Batch, max: usize) -> Result<bool> {
        let limit = (out.len() + max).min(BATCH_ROWS);
        match self.access {
            Access::Empty => Ok(false),
            Access::Single => self.next_single(cx, out, limit),
            Access::Range { phase } => {
                if self.next_single(cx, out, limit)? {
                    return Ok(true);
                }
                if phase != 0 {
                    self.access = Access::Empty;
                    return Ok(false);
                }
                // Phase 1: the numeric exceptions of (g, p) (dictionary-typed literals and
                // inline decimals); the FILTER above the scan decides on each of them.
                self.acts.retain(|a| a.1 != Act::IntTag);
                let spec = self.range_spec(1);
                self.position(cx, 0, &spec, self.fw)?;
                self.access = Access::Range { phase: 1 };
                self.read(cx, out, limit - out.len())
            }
            Access::PerGraph { slot } => loop {
                let active = self.srcs.first().is_some_and(|s| !s.done || s.pos < s.rows);
                if active && self.next_single(cx, out, limit)? {
                    return Ok(true);
                }
                if !self.next_graph(cx, slot)? {
                    self.access = Access::Empty;
                    return Ok(false);
                }
            },
            Access::Merge { n } => self.next_merge(cx, out, n, limit),
        }
    }

    /// Loose index scan: the next distinct value of `slot`, which must be bound by the first
    /// free column (rows sorted by it). Seeks past each value when the access supports it.
    pub fn next_distinct(
        &mut self,
        cx: &mut ExecCtx,
        slot: Slot,
        tmp: &mut Batch,
        last: &mut Option<TermId>,
    ) -> Result<Option<TermId>> {
        loop {
            tmp.clear();
            self.read(cx, tmp, 1)?;
            if tmp.is_empty() {
                return Ok(None);
            }
            let v = tmp.get(0, slot);
            if *last == Some(v) {
                continue;
            }
            *last = Some(v);
            if let Some(n) = v.0.checked_add(1) {
                self.seek(cx, slot, TermId(n))?;
            }
            return Ok(Some(v));
        }
    }
}

impl Operator for Scan {
    fn open(&mut self, cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        self.base.copy_from_slice(self.seed.base());
        self.plan_access(cx)
    }

    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        self.read(cx, out, BATCH_ROWS)
    }

    fn next_limited(&mut self, cx: &mut ExecCtx, out: &mut Batch, max: usize) -> Result<bool> {
        self.read(cx, out, max)
    }

    fn fast_count(&mut self, cx: &mut ExecCtx) -> Option<Result<u64>> {
        if self.checks {
            return None;
        }
        match self.access {
            Access::Empty => Some(Ok(0)),
            Access::Single => {
                // Single accesses have no graph positions in the prefix.
                let spec = ScanSpec::new(self.order, &self.prefix[..self.prefix_len]);
                self.access = Access::Empty;
                Some(cx.store().count(&spec, None))
            }
            Access::PerGraph { .. } => {
                let mut total = 0;
                for &g in &self.graphs[self.gnext..] {
                    match cx.store().count(&self.spec_for(g), None) {
                        Ok(n) => total += n,
                        Err(e) => return Some(Err(e)),
                    }
                }
                self.access = Access::Empty;
                Some(Ok(total))
            }
            Access::Range { .. } | Access::Merge { .. } => None,
        }
    }

    fn seek(&mut self, cx: &mut ExecCtx, slot: Slot, key: TermId) -> Result<bool> {
        if self.access != Access::Single || self.acts.first() != Some(&(0, Act::Bind(slot))) {
            return Ok(false);
        }
        cx.sh.prof(self.id, |n| n.seeks += 1);
        let s = &mut self.srcs[0];
        s.cursor.seek(&[key])?;
        s.buf.clear();
        s.pos = 0;
        s.rows = 0;
        s.done = false;
        Ok(true)
    }

    fn close(&mut self, _cx: &mut ExecCtx) {
        // Dropping the cursors returns their leased statements to the store's pool.
        self.srcs.clear();
        self.access = Access::Empty;
    }
}
