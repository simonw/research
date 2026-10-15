//! ORDER BY (§6.5): Sort (stable, SortKeys computed once per row), TopK (bounded heap of
//! k = offset + limit rows, stable for ties) and the SortBySlot merge-join enforcer (stable
//! sort by the raw signed id of one slot).
//!
//! When every key is a bare variable and every value is an inline INT, rows are compared on
//! the raw ids (INT ids are monotonic in value, and the order agrees with the SortKey order),
//! so no SortKey is ever built.
//!
//! Otherwise SortKeys are built lazily ("late key materialization"): a key is only built for
//! rows whose order is still undecided by the earlier keys. Sort refines runs of rows that tie
//! on the earlier keys; TopK compares bare-variable keys on the raw ids where that decides the
//! order (`cheap_cmp`) and builds a key, at most once per row, only for comparisons it cannot
//! decide that way, typically ties on the earlier keys at the heap boundary. `ORDER BY
//! DESC(?age) ?x LIMIT 10` thus decodes the IRIs of the few rows that tie with the 10th best
//! age instead of every `?x`. The order is exactly the SortKey order either way.
use std::cmp::Ordering;

use sq_core::ir::OrderKey;
use sq_core::{Batch, Kind, Result, Slot, Tag, TermId};
use sq_expr::{CompiledExpr, SortKey};

use super::{BoxOp, Operator};
use crate::ctx::{Charge, ExecCtx};
use crate::rows::Rows;
use crate::seed::{Seed, SeedBuf};

/// A compiled ORDER BY key.
pub(crate) struct SortSpec {
    pub expr: CompiledExpr,
    pub desc: bool,
}

impl SortSpec {
    pub fn compile(k: &OrderKey, temps: &sq_core::eval_api::TempDict) -> Result<Self> {
        Ok(SortSpec { expr: sq_expr::compile(&k.expr, temps)?, desc: k.desc })
    }
}

#[inline]
fn directed(o: Ordering, desc: bool) -> Ordering {
    if desc { o.reverse() } else { o }
}

/// Bare-variable key slots, when every key is one.
fn slot_keys(keys: &[SortSpec]) -> Option<Vec<(Slot, bool)>> {
    keys.iter().map(|k| k.expr.as_slot().map(|s| (s, k.desc))).collect()
}

/// The SortKey class of a present id (blank node < IRI < literal), known without a lookup.
#[inline]
fn class_of(id: TermId) -> u8 {
    match id.kind() {
        Kind::Unbound => 0,
        Kind::BNode => 1,
        Kind::Iri => 2,
        Kind::Literal => 3,
    }
}

/// The ascending SortKey order of two values of a bare-variable key, when it is decided
/// without building the keys: the same id (the same term), UNBOUND or ERROR (the lowest key),
/// two inline INTs (raw ids are monotonic in value), or two different term classes. None:
/// only the SortKeys can tell (two IRIs, two literals that are not both INTs, ...).
#[inline]
fn cheap_cmp(a: TermId, b: TermId) -> Option<Ordering> {
    if a == b {
        return Some(Ordering::Equal);
    }
    let (pa, pb) = (sq_expr::is_present(a), sq_expr::is_present(b));
    if !pa || !pb {
        return Some(pa.cmp(&pb));
    }
    if a.tag() == Tag::Int && b.tag() == Tag::Int {
        return Some(a.0.cmp(&b.0));
    }
    let (ca, cb) = (class_of(a), class_of(b));
    if ca != cb { Some(ca.cmp(&cb)) } else { None }
}

/// A raw INT id as an i64 whose ascending order is the requested direction.
#[inline]
fn directed_int(id: TermId, desc: bool) -> i64 {
    if desc { !id.0 } else { id.0 }
}

/// The permutation that sorts `keys` (one key per row), ties in row order. Sorting compact
/// (key, row) pairs keeps the comparisons cache-friendly.
fn sorted_perm<K: Ord>(keys: impl Iterator<Item = K>) -> Vec<u32> {
    let mut v: Vec<(K, u32)> = keys.zip(0u32..).collect();
    v.sort_unstable();
    v.into_iter().map(|(_, i)| i).collect()
}

fn all_int(row: &[TermId], slots: &[(Slot, bool)]) -> bool {
    slots.iter().all(|(s, _)| row[*s as usize].tag() == Tag::Int)
}

/// Materializes the input, then emits it in order.
pub(crate) struct Sort {
    input: BoxOp,
    keys: Vec<SortSpec>,
    slots: Option<Vec<(Slot, bool)>>,
    seed: SeedBuf,
    rows: Rows,
    perm: Vec<u32>,
    pos: usize,
    built: bool,
    inb: Batch,
    charge: Charge,
}

impl Sort {
    pub fn new(input: BoxOp, keys: Vec<SortSpec>, width: usize) -> Self {
        let slots = slot_keys(&keys);
        Sort {
            input,
            keys,
            slots,
            seed: SeedBuf::new(width),
            rows: Rows::new(width),
            perm: Vec::new(),
            pos: 0,
            built: false,
            inb: Batch::new(width),
            charge: Charge::default(),
        }
    }

    fn build(&mut self, cx: &mut ExecCtx) -> Result<()> {
        loop {
            self.inb.clear();
            let more = self.input.next(cx, &mut self.inb)?;
            cx.charge(&mut self.charge, self.inb.len() * (self.rows.row_bytes() + 4))?;
            self.rows.extend_from_batch(&self.inb);
            if !more {
                break;
            }
        }
        self.input.reset(cx);
        let n = self.rows.len();
        let rows = &self.rows;
        cx.charge(&mut self.charge, n * 16)?;
        // Ties keep the input order: every sort below breaks ties on the row index.
        self.perm = match self.slots.as_deref() {
            Some(&[(s, desc)]) if (0..n).all(|i| rows.row(i)[s as usize].tag() == Tag::Int) => {
                sorted_perm((0..n).map(|i| directed_int(rows.row(i)[s as usize], desc)))
            }
            Some(slots) if (0..n).all(|i| all_int(rows.row(i), slots)) => {
                let k = slots.len();
                let flat: Vec<i64> = (0..n)
                    .flat_map(|i| slots.iter().map(move |(s, d)| directed_int(rows.row(i)[*s as usize], *d)))
                    .collect();
                let mut perm: Vec<u32> = (0..n as u32).collect();
                perm.sort_unstable_by(|a, b| {
                    let (x, y) = (*a as usize * k, *b as usize * k);
                    flat[x..x + k].cmp(&flat[y..y + k]).then(a.cmp(b))
                });
                perm
            }
            _ => {
                let mut perm: Vec<u32> = (0..n as u32).collect();
                self.sort_runs(cx, &mut perm)?;
                perm
            }
        };
        self.built = true;
        Ok(())
    }

    /// Sorts `perm` (row indexes in ascending order) by the keys, ties in row order, by run
    /// refinement: a run of rows tied on the keys before `level` is sorted by key `level`, and
    /// each of its sub-runs still tied on that key becomes a run for `level + 1`. A later key
    /// is thus never built for a row that an earlier key already placed. Runs never overlap,
    /// so they are kept on an explicit work list (in any order) rather than recursed into: the
    /// depth would be the number of ORDER BY keys, which the nesting budget does not charge.
    fn sort_runs(&mut self, cx: &mut ExecCtx, perm: &mut [u32]) -> Result<()> {
        let nkeys = self.keys.len();
        // (start, end, level): perm[start..end] is tied on the keys before `level`.
        let mut work: Vec<(usize, usize, usize)> = vec![(0, perm.len(), 0)];
        let mut tied: Vec<(usize, usize)> = Vec::new();
        // SortKeys built so far (interrupt polling).
        let mut built = 0usize;
        while let Some((start, end, level)) = work.pop() {
            if end - start < 2 || level == nkeys {
                continue;
            }
            tied.clear();
            self.sort_level(cx, &mut perm[start..end], level, &mut built, &mut tied)?;
            work.extend(tied.iter().map(|&(a, b)| (start + a, start + b, level + 1)));
        }
        Ok(())
    }

    /// Sorts `run` by key `level` alone, ties in row order (`run` is in ascending row order
    /// on entry), and appends the ranges of `run` that tie on that key (two rows or more) to
    /// `tied`, unless it is the last key.
    fn sort_level(
        &mut self,
        cx: &mut ExecCtx,
        run: &mut [u32],
        level: usize,
        built: &mut usize,
        tied: &mut Vec<(usize, usize)>,
    ) -> Result<()> {
        let desc = self.keys[level].desc;
        // The last key's ties are final (row order).
        let last = level + 1 == self.keys.len();
        let rows = &self.rows;
        let mut push_tied = |eq: &dyn Fn(usize, usize) -> bool, len: usize| {
            let mut start = 0;
            for i in 1..=len {
                if i == len || !eq(i - 1, i) {
                    if i - start > 1 {
                        tied.push((start, i));
                    }
                    start = i;
                }
            }
        };
        match self.keys[level].expr.as_slot() {
            Some(s) if run.iter().all(|&i| rows.row(i as usize)[s as usize].tag() == Tag::Int) => {
                let mut v: Vec<(i64, u32)> =
                    run.iter().map(|&i| (directed_int(rows.row(i as usize)[s as usize], desc), i)).collect();
                v.sort_unstable();
                for (dst, (_, i)) in run.iter_mut().zip(&v) {
                    *dst = *i;
                }
                if last {
                    return Ok(());
                }
                push_tied(&|a, b| v[a].0 == v[b].0, v.len());
            }
            _ => {
                let mut v: Vec<(SortKey, u32)> = Vec::with_capacity(run.len());
                let mut bytes = 0usize;
                for &i in run.iter() {
                    let row = self.rows.row(i as usize);
                    let expr = &self.keys[level].expr;
                    let key = cx.with_eval(|ecx| expr.sort_key(row, ecx))?;
                    bytes += key.as_bytes().len() + 24;
                    v.push((key, i));
                    *built += 1;
                    if *built % 1024 == 0 {
                        cx.tick(1024)?;
                    }
                }
                cx.charge(&mut self.charge, bytes)?;
                v.sort_unstable_by(|a, b| directed(a.0.cmp(&b.0), desc).then(a.1.cmp(&b.1)));
                for (dst, (_, i)) in run.iter_mut().zip(&v) {
                    *dst = *i;
                }
                if last {
                    return Ok(());
                }
                push_tied(&|a, b| v[a].0 == v[b].0, v.len());
            }
        }
        Ok(())
    }
}

impl Sort {
    /// Frees the materialized rows.
    fn release_rows(&mut self, cx: &ExecCtx) {
        self.rows.reset();
        self.perm = Vec::new();
        cx.release(&mut self.charge);
    }
}

impl Operator for Sort {
    fn open(&mut self, cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        self.rows.clear();
        self.perm.clear();
        self.pos = 0;
        self.built = false;
        cx.release(&mut self.charge);
        self.input.open(cx, self.seed.full())
    }

    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        if !self.built {
            self.build(cx)?;
        }
        while self.pos < self.perm.len() {
            if out.is_full() {
                return Ok(true);
            }
            out.push_row(self.rows.row(self.perm[self.pos] as usize));
            self.pos += 1;
        }
        Ok(false)
    }

    fn fast_count(&mut self, cx: &mut ExecCtx, cap: Option<u64>) -> Option<Result<u64>> {
        if self.built { None } else { self.input.fast_count(cx, cap) }
    }

    fn reset(&mut self, cx: &mut ExecCtx) {
        self.release_rows(cx);
        self.input.reset(cx)
    }

    fn close(&mut self, cx: &mut ExecCtx) {
        self.release_rows(cx);
        self.input.close(cx)
    }
}

/// Merge-join enforcer: stable sort by the raw signed id of `key`.
pub(crate) struct SortBySlot {
    input: BoxOp,
    key: Slot,
    seed: SeedBuf,
    rows: Rows,
    perm: Vec<u32>,
    pos: usize,
    built: bool,
    inb: Batch,
    charge: Charge,
}

impl SortBySlot {
    pub fn new(input: BoxOp, key: Slot, width: usize) -> Self {
        SortBySlot {
            input,
            key,
            seed: SeedBuf::new(width),
            rows: Rows::new(width),
            perm: Vec::new(),
            pos: 0,
            built: false,
            inb: Batch::new(width),
            charge: Charge::default(),
        }
    }

    fn build(&mut self, cx: &mut ExecCtx) -> Result<()> {
        loop {
            self.inb.clear();
            let more = self.input.next(cx, &mut self.inb)?;
            cx.charge(&mut self.charge, self.inb.len() * (self.rows.row_bytes() + 4))?;
            self.rows.extend_from_batch(&self.inb);
            if !more {
                break;
            }
        }
        self.input.reset(cx);
        let k = self.key as usize;
        let rows = &self.rows;
        cx.charge(&mut self.charge, rows.len() * 16)?;
        self.perm = sorted_perm((0..rows.len()).map(|i| rows.row(i)[k]));
        self.built = true;
        Ok(())
    }
}

impl SortBySlot {
    /// Frees the materialized rows.
    fn release_rows(&mut self, cx: &ExecCtx) {
        self.rows.reset();
        self.perm = Vec::new();
        cx.release(&mut self.charge);
    }
}

impl Operator for SortBySlot {
    fn open(&mut self, cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        self.rows.clear();
        self.perm.clear();
        self.pos = 0;
        self.built = false;
        cx.release(&mut self.charge);
        self.input.open(cx, self.seed.full())
    }

    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        if !self.built {
            self.build(cx)?;
        }
        while self.pos < self.perm.len() {
            if out.is_full() {
                return Ok(true);
            }
            out.push_row(self.rows.row(self.perm[self.pos] as usize));
            self.pos += 1;
        }
        Ok(false)
    }

    fn fast_count(&mut self, cx: &mut ExecCtx, cap: Option<u64>) -> Option<Result<u64>> {
        if self.built { None } else { self.input.fast_count(cx, cap) }
    }

    /// Binary search in the sorted buffer (merge-join galloping over an enforcer).
    fn seek(&mut self, cx: &mut ExecCtx, slot: Slot, key: TermId) -> Result<bool> {
        if slot != self.key {
            return Ok(false);
        }
        if !self.built {
            self.build(cx)?;
        }
        let (rows, k) = (&self.rows, slot as usize);
        let rest = &self.perm[self.pos..];
        self.pos += rest.partition_point(|i| rows.row(*i as usize)[k] < key);
        Ok(true)
    }

    fn reset(&mut self, cx: &mut ExecCtx) {
        self.release_rows(cx);
        self.input.reset(cx)
    }

    fn close(&mut self, cx: &mut ExecCtx) {
        self.release_rows(cx);
        self.input.close(cx)
    }
}

/// Heap entry of TopK: the arena row, its input position (ties keep input order) and its
/// SortKeys, built lazily (`keys[i]` is None, or `keys` shorter than the key list, until
/// key `i` is needed by a comparison).
struct Entry {
    row: u32,
    seq: u64,
    keys: Vec<Option<SortKey>>,
}

/// One side of a TopK comparison: the candidate row `r` of the input batch (input position
/// `seq`), or a heap entry.
#[derive(Copy, Clone)]
enum Side {
    Cand(usize, u64),
    Heap(usize),
}

/// ORDER BY + LIMIT: a bounded max-heap of the k best rows (by key, then input order). An
/// evicted row's arena slot is overwritten by the row that evicts it. Keys are compared
/// cheaply on raw ids where possible and built lazily otherwise (module doc).
pub(crate) struct TopK {
    input: BoxOp,
    keys: Vec<SortSpec>,
    k: u64,
    /// Per key: its slot when the key is a bare variable (`cheap_cmp` applies).
    key_slots: Vec<Option<Slot>>,
    seed: SeedBuf,
    rows: Rows,
    heap: Vec<Entry>,
    out_order: Vec<u32>,
    pos: usize,
    built: bool,
    inb: Batch,
    /// The lazily built keys of the current candidate row.
    cand: Vec<Option<SortKey>>,
    charge: Charge,
}

impl TopK {
    pub fn new(input: BoxOp, keys: Vec<SortSpec>, k: u64, width: usize) -> Self {
        let key_slots = keys.iter().map(|k| k.expr.as_slot()).collect();
        TopK {
            input,
            keys,
            k,
            key_slots,
            seed: SeedBuf::new(width),
            rows: Rows::new(width),
            heap: Vec::new(),
            out_order: Vec::new(),
            pos: 0,
            built: false,
            inb: Batch::new(width),
            cand: Vec::new(),
            charge: Charge::default(),
        }
    }

    #[inline]
    fn row_of(&self, s: Side) -> &[TermId] {
        match s {
            Side::Cand(r, _) => self.inb.row(r),
            Side::Heap(e) => self.rows.row(self.heap[e].row as usize),
        }
    }

    #[inline]
    fn seq_of(&self, s: Side) -> u64 {
        match s {
            Side::Cand(_, seq) => seq,
            Side::Heap(e) => self.heap[e].seq,
        }
    }

    #[inline]
    fn key_of(&self, s: Side, i: usize) -> Option<&SortKey> {
        let keys = match s {
            Side::Cand(..) => &self.cand,
            Side::Heap(e) => &self.heap[e].keys,
        };
        keys.get(i).and_then(Option::as_ref)
    }

    /// Builds key `i` of `s` unless it is built already (each row's key is built at most once,
    /// so a non-deterministic key expression still gives every row one value).
    fn ensure_key(&mut self, cx: &mut ExecCtx, s: Side, i: usize) -> Result<()> {
        if self.key_of(s, i).is_some() {
            return Ok(());
        }
        let row = self.row_of(s);
        let expr = &self.keys[i].expr;
        let key = cx.with_eval(|ecx| expr.sort_key(row, ecx))?;
        let n = self.keys.len();
        let keys = match s {
            Side::Cand(..) => &mut self.cand,
            Side::Heap(e) => &mut self.heap[e].keys,
        };
        if keys.len() < n {
            keys.resize(n, None);
        }
        keys[i] = Some(key);
        Ok(())
    }

    /// Entry order: keys, then input position (a total order).
    fn cmp(&mut self, cx: &mut ExecCtx, a: Side, b: Side) -> Result<Ordering> {
        for i in 0..self.keys.len() {
            let cheap = match self.key_slots[i] {
                Some(s) => cheap_cmp(self.row_of(a)[s as usize], self.row_of(b)[s as usize]),
                None => None,
            };
            let o = match cheap {
                Some(o) => o,
                None => {
                    self.ensure_key(cx, a, i)?;
                    self.ensure_key(cx, b, i)?;
                    self.key_of(a, i).cmp(&self.key_of(b, i))
                }
            };
            let o = directed(o, self.keys[i].desc);
            if o != Ordering::Equal {
                return Ok(o);
            }
        }
        Ok(self.seq_of(a).cmp(&self.seq_of(b)))
    }

    fn sift_up(&mut self, cx: &mut ExecCtx, mut i: usize) -> Result<()> {
        while i > 0 {
            let p = (i - 1) / 2;
            if self.cmp(cx, Side::Heap(i), Side::Heap(p))? != Ordering::Greater {
                break;
            }
            self.heap.swap(i, p);
            i = p;
        }
        Ok(())
    }

    fn sift_down(&mut self, cx: &mut ExecCtx, mut i: usize) -> Result<()> {
        let n = self.heap.len();
        loop {
            let (l, r) = (2 * i + 1, 2 * i + 2);
            let mut m = i;
            if l < n && self.cmp(cx, Side::Heap(l), Side::Heap(m))? == Ordering::Greater {
                m = l;
            }
            if r < n && self.cmp(cx, Side::Heap(r), Side::Heap(m))? == Ordering::Greater {
                m = r;
            }
            if m == i {
                return Ok(());
            }
            self.heap.swap(i, m);
            i = m;
        }
    }

    /// Offers row `r` of the input batch (input position `seq`).
    fn offer(&mut self, cx: &mut ExecCtx, r: usize, seq: u64) -> Result<()> {
        if (self.heap.len() as u64) < self.k {
            cx.charge(&mut self.charge, self.rows.row_bytes() + 48)?;
            self.rows.push(self.inb.row(r));
            let row = (self.rows.len() - 1) as u32;
            self.heap.push(Entry { row, seq, keys: Vec::new() });
            let last = self.heap.len() - 1;
            return self.sift_up(cx, last);
        }
        // Full. Most rows lose to the worst retained row on the first key alone: reject them
        // without the general comparison.
        if let Some(&Some(s)) = self.key_slots.first() {
            let top = self.rows.row(self.heap[0].row as usize)[s as usize];
            if let Some(o) = cheap_cmp(self.inb.row(r)[s as usize], top)
                && directed(o, self.keys[0].desc) == Ordering::Greater
            {
                return Ok(());
            }
        }
        // Replace the worst retained row if the new one sorts before it.
        self.cand.clear();
        if self.cmp(cx, Side::Cand(r, seq), Side::Heap(0))? != Ordering::Less {
            return Ok(());
        }
        let arena = self.heap[0].row as usize;
        self.rows.row_mut(arena).copy_from_slice(self.inb.row(r));
        self.heap[0].seq = seq;
        std::mem::swap(&mut self.heap[0].keys, &mut self.cand);
        self.sift_down(cx, 0)
    }

    fn build(&mut self, cx: &mut ExecCtx) -> Result<()> {
        let mut seq = 0u64;
        loop {
            self.inb.clear();
            let more = self.input.next(cx, &mut self.inb)?;
            for r in 0..self.inb.len() {
                self.offer(cx, r, seq)?;
                seq += 1;
            }
            if !more {
                break;
            }
        }
        self.input.reset(cx);
        // Heap sort with the same lazy comparisons: pop the worst row until the heap is empty.
        let mut order = Vec::with_capacity(self.heap.len());
        while let Some(last) = self.heap.len().checked_sub(1) {
            self.heap.swap(0, last);
            order.push(self.heap.pop().expect("non-empty").row);
            if !self.heap.is_empty() {
                self.sift_down(cx, 0)?;
            }
        }
        order.reverse();
        self.out_order = order;
        self.built = true;
        Ok(())
    }
}

impl TopK {
    /// Frees the materialized rows.
    fn release_rows(&mut self, cx: &ExecCtx) {
        self.rows.reset();
        self.heap = Vec::new();
        self.out_order = Vec::new();
        cx.release(&mut self.charge);
    }
}

impl Operator for TopK {
    fn open(&mut self, cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        self.rows.clear();
        self.heap.clear();
        self.out_order.clear();
        self.pos = 0;
        self.built = self.k == 0;
        self.cand.clear();
        cx.release(&mut self.charge);
        if self.k == 0 {
            return Ok(());
        }
        // Never seeded with Join-mode values (it is a Slice).
        self.input.open(cx, self.seed.subst_only())
    }

    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        if !self.built {
            self.build(cx)?;
        }
        while self.pos < self.out_order.len() {
            if out.is_full() {
                return Ok(true);
            }
            out.push_row(self.rows.row(self.out_order[self.pos] as usize));
            self.pos += 1;
        }
        Ok(false)
    }

    fn reset(&mut self, cx: &mut ExecCtx) {
        self.release_rows(cx);
        self.input.reset(cx)
    }

    fn close(&mut self, cx: &mut ExecCtx) {
        self.release_rows(cx);
        self.input.close(cx)
    }
}
