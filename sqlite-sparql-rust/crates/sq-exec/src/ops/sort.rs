//! ORDER BY (§6.5): Sort (stable, SortKeys computed once per row), TopK (bounded heap of
//! k = offset + limit rows, stable for ties) and the SortBySlot merge-join enforcer (stable
//! sort by the raw signed id of one slot).
//!
//! When every key is a bare variable and every value is an inline INT, rows are compared on
//! the raw ids (INT ids are monotonic in value, and the order agrees with the SortKey order),
//! so no SortKey is ever built.
use std::cmp::Ordering;

use sq_core::ir::OrderKey;
use sq_core::{Batch, Result, Slot, Tag, TermId};
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

/// Compares two rows on raw INT ids of the key slots.
#[inline]
fn cmp_ints(a: &[TermId], b: &[TermId], slots: &[(Slot, bool)]) -> Ordering {
    for &(s, desc) in slots {
        let o = directed(a[s as usize].0.cmp(&b[s as usize].0), desc);
        if o != Ordering::Equal {
            return o;
        }
    }
    Ordering::Equal
}

#[inline]
fn cmp_keys(a: &[SortKey], b: &[SortKey], keys: &[SortSpec]) -> Ordering {
    for ((x, y), k) in a.iter().zip(b).zip(keys) {
        let o = directed(x.cmp(y), k.desc);
        if o != Ordering::Equal {
            return o;
        }
    }
    Ordering::Equal
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

/// Computes the sort keys of `row` into `out`.
fn keys_of(keys: &[SortSpec], row: &[TermId], cx: &mut ExecCtx, out: &mut Vec<SortKey>) -> Result<()> {
    cx.with_eval(|ecx| {
        for k in keys {
            out.push(k.expr.sort_key(row, ecx)?);
        }
        Ok(())
    })
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
        self.input.close(cx);
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
                let flat: Vec<i64> =
                    (0..n).flat_map(|i| slots.iter().map(move |(s, d)| directed_int(rows.row(i)[*s as usize], *d))).collect();
                let mut perm: Vec<u32> = (0..n as u32).collect();
                perm.sort_unstable_by(|a, b| {
                    let (x, y) = (*a as usize * k, *b as usize * k);
                    flat[x..x + k].cmp(&flat[y..y + k]).then(a.cmp(b))
                });
                perm
            }
            _ => {
                let k = self.keys.len();
                let mut sk: Vec<SortKey> = Vec::with_capacity(n * k);
                for i in 0..n {
                    keys_of(&self.keys, self.rows.row(i), cx, &mut sk)?;
                    if i % 1024 == 1023 {
                        cx.tick(1024)?;
                    }
                }
                let bytes: usize = sk.iter().map(|s| s.as_bytes().len() + 24).sum();
                cx.charge(&mut self.charge, bytes)?;
                let keys = &self.keys;
                if let [key] = keys.as_slice() {
                    let mut v: Vec<(SortKey, u32)> = sk.into_iter().zip(0u32..).collect();
                    v.sort_unstable_by(|a, b| directed(a.0.cmp(&b.0), key.desc).then(a.1.cmp(&b.1)));
                    v.into_iter().map(|(_, i)| i).collect()
                } else {
                    let mut perm: Vec<u32> = (0..n as u32).collect();
                    perm.sort_unstable_by(|a, b| {
                        let (x, y) = (*a as usize * k, *b as usize * k);
                        cmp_keys(&sk[x..x + k], &sk[y..y + k], keys).then(a.cmp(b))
                    });
                    perm
                }
            }
        };
        self.built = true;
        Ok(())
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

    fn fast_count(&mut self, cx: &mut ExecCtx) -> Option<Result<u64>> {
        if self.built { None } else { self.input.fast_count(cx) }
    }

    fn close(&mut self, cx: &mut ExecCtx) {
        self.rows.reset();
        self.perm = Vec::new();
        cx.release(&mut self.charge);
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
        self.input.close(cx);
        let k = self.key as usize;
        let rows = &self.rows;
        cx.charge(&mut self.charge, rows.len() * 16)?;
        self.perm = sorted_perm((0..rows.len()).map(|i| rows.row(i)[k]));
        self.built = true;
        Ok(())
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

    fn fast_count(&mut self, cx: &mut ExecCtx) -> Option<Result<u64>> {
        if self.built { None } else { self.input.fast_count(cx) }
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

    fn close(&mut self, cx: &mut ExecCtx) {
        self.rows.reset();
        self.perm = Vec::new();
        cx.release(&mut self.charge);
        self.input.close(cx)
    }
}

/// Heap entry of TopK: the arena row, its keys (empty in INT mode, where the keys are read
/// from the row) and its input position (ties keep input order).
struct Entry {
    row: u32,
    seq: u64,
    keys: Vec<SortKey>,
}

/// ORDER BY + LIMIT: a bounded max-heap of the k best rows (by key, then input order). An
/// evicted row's arena slot is overwritten by the row that evicts it.
pub(crate) struct TopK {
    input: BoxOp,
    keys: Vec<SortSpec>,
    k: u64,
    slots: Option<Vec<(Slot, bool)>>,
    /// INT mode: every key a bare variable and every value seen so far an inline INT.
    int_mode: bool,
    seed: SeedBuf,
    rows: Rows,
    heap: Vec<Entry>,
    out_order: Vec<u32>,
    pos: usize,
    built: bool,
    inb: Batch,
    tmp: Vec<SortKey>,
    charge: Charge,
}

impl TopK {
    pub fn new(input: BoxOp, keys: Vec<SortSpec>, k: u64, width: usize) -> Self {
        let slots = slot_keys(&keys);
        TopK {
            input,
            keys,
            k,
            int_mode: slots.is_some(),
            slots,
            seed: SeedBuf::new(width),
            rows: Rows::new(width),
            heap: Vec::new(),
            out_order: Vec::new(),
            pos: 0,
            built: false,
            inb: Batch::new(width),
            tmp: Vec::new(),
            charge: Charge::default(),
        }
    }

    /// Entry order: keys, then input position (a total order).
    fn cmp(&self, a: &Entry, b: &Entry) -> Ordering {
        let o = match (&self.slots, self.int_mode) {
            (Some(slots), true) => cmp_ints(self.rows.row(a.row as usize), self.rows.row(b.row as usize), slots),
            _ => cmp_keys(&a.keys, &b.keys, &self.keys),
        };
        o.then(a.seq.cmp(&b.seq))
    }

    fn sift_up(&mut self, mut i: usize) {
        while i > 0 {
            let p = (i - 1) / 2;
            if self.cmp(&self.heap[i], &self.heap[p]) != Ordering::Greater {
                break;
            }
            self.heap.swap(i, p);
            i = p;
        }
    }

    fn sift_down(&mut self, mut i: usize) {
        let n = self.heap.len();
        loop {
            let (l, r) = (2 * i + 1, 2 * i + 2);
            let mut m = i;
            if l < n && self.cmp(&self.heap[l], &self.heap[m]) == Ordering::Greater {
                m = l;
            }
            if r < n && self.cmp(&self.heap[r], &self.heap[m]) == Ordering::Greater {
                m = r;
            }
            if m == i {
                break;
            }
            self.heap.swap(i, m);
            i = m;
        }
    }

    /// Leaves INT mode: computes the SortKeys of every retained row and re-heapifies.
    fn to_key_mode(&mut self, cx: &mut ExecCtx) -> Result<()> {
        self.int_mode = false;
        for e in 0..self.heap.len() {
            let mut ks = Vec::with_capacity(self.keys.len());
            keys_of(&self.keys, self.rows.row(self.heap[e].row as usize), cx, &mut ks)?;
            self.heap[e].keys = ks;
        }
        for i in (0..self.heap.len() / 2).rev() {
            self.sift_down(i);
        }
        Ok(())
    }

    /// Offers row `r` of the input batch (input position `seq`).
    fn offer(&mut self, cx: &mut ExecCtx, r: usize, seq: u64) -> Result<()> {
        if let (Some(slots), true) = (&self.slots, self.int_mode)
            && !all_int(self.inb.row(r), slots)
        {
            self.to_key_mode(cx)?;
        }
        if !self.int_mode {
            self.tmp.clear();
            keys_of(&self.keys, self.inb.row(r), cx, &mut self.tmp)?;
        }
        if (self.heap.len() as u64) < self.k {
            cx.charge(&mut self.charge, self.rows.row_bytes() + 48)?;
            self.rows.push(self.inb.row(r));
            let row = (self.rows.len() - 1) as u32;
            self.heap.push(Entry { row, seq, keys: std::mem::take(&mut self.tmp) });
            let last = self.heap.len() - 1;
            self.sift_up(last);
            return Ok(());
        }
        // Full: replace the worst retained row if the new one sorts before it.
        let top = &self.heap[0];
        let o = match (&self.slots, self.int_mode) {
            (Some(slots), true) => cmp_ints(self.inb.row(r), self.rows.row(top.row as usize), slots),
            _ => cmp_keys(&self.tmp, &top.keys, &self.keys),
        };
        if o.then(seq.cmp(&top.seq)) != Ordering::Less {
            return Ok(());
        }
        let arena = top.row as usize;
        self.rows.row_mut(arena).copy_from_slice(self.inb.row(r));
        self.heap[0].seq = seq;
        std::mem::swap(&mut self.heap[0].keys, &mut self.tmp);
        self.sift_down(0);
        Ok(())
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
        self.input.close(cx);
        let mut order: Vec<usize> = (0..self.heap.len()).collect();
        order.sort_by(|a, b| self.cmp(&self.heap[*a], &self.heap[*b]));
        self.out_order = order.into_iter().map(|i| self.heap[i].row).collect();
        self.built = true;
        Ok(())
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
        self.int_mode = self.slots.is_some();
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

    fn close(&mut self, cx: &mut ExecCtx) {
        self.rows.reset();
        self.heap = Vec::new();
        self.out_order = Vec::new();
        cx.release(&mut self.charge);
        self.input.close(cx)
    }
}
