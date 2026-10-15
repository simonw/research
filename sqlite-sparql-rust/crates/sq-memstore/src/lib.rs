//! sq-memstore: in-memory `QuadStore` + `QuadStoreWriter`.
//!
//! The oracle store and test double for every other crate. One `BTreeSet<[TermId; 4]>` per
//! physical order, holding each quad permuted into that order's column order, so cursors,
//! counts, partial orders (named-only, `Gpox`) and signed-i64 ordering behave exactly like
//! the SQLite store. Cursors snapshot their matching range at create / rebind time.
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::ops::Bound;
use std::rc::Rc;
use std::sync::Arc;

use rustc_hash::{FxHashMap, FxHashSet};
use sq_core::codec::{self, TermKey, FIRST_USER_ROW, WELL_KNOWN};
use sq_core::stats::{CharSet, CostConstants, PredStats, ScopeStats, Stats, MEMORY_COSTS};
use sq_core::store::*;
use sq_core::term::{Literal, Term};
use sq_core::{Error, Result, Tag, TermId};

const ORDERS: usize = IndexOrder::ALL.len();

fn order_index(o: IndexOrder) -> usize {
    IndexOrder::ALL.iter().position(|x| *x == o).unwrap()
}

fn permute(q: &[TermId; 4], order: IndexOrder) -> [TermId; 4] {
    let c = order.cols();
    [q[c[0] as usize], q[c[1] as usize], q[c[2] as usize], q[c[3] as usize]]
}

fn unpermute(k: &[TermId; 4], order: IndexOrder) -> [TermId; 4] {
    let c = order.cols();
    let mut q = [TermId::UNBOUND; 4];
    for i in 0..4 {
        q[c[i] as usize] = k[i];
    }
    q
}

fn belongs(q: &[TermId; 4], order: IndexOrder) -> bool {
    (!order.named_only() || q[0] != TermId::DEFAULT_GRAPH) && (!order.numeric_exceptions() || matches!(q[3].0 & 15, 5 | 8))
}

struct Inner {
    sets: [BTreeSet<[TermId; 4]>; ORDERS],
    /// Dictionary rows: `rows[r]` is the term of dictionary row r (None = unused row).
    rows: Vec<Option<Arc<Term>>>,
    index: FxHashMap<TermKey, TermId>,
    next_row: i64,
    next_bnode: i64,
    registry: BTreeSet<TermId>,
    stats: Option<Arc<Stats>>,
}

/// Cheap to clone; clones share the same data.
#[derive(Clone)]
pub struct MemStore {
    inner: Rc<RefCell<Inner>>,
    costs: Rc<Cell<&'static CostConstants>>,
}

impl Default for MemStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MemStore {
    pub fn new() -> Self {
        let mut rows = vec![None; FIRST_USER_ROW as usize];
        let mut index = FxHashMap::default();
        for (row, iri) in WELL_KNOWN {
            let t = Term::Iri((*iri).into());
            let id = TermId::from_dict(*row, Tag::Iri);
            rows[*row as usize] = Some(Arc::new(t));
            index.insert(TermKey { tag: Tag::Iri, lex: (*iri).into(), extra: "".into() }, id);
        }
        MemStore {
            inner: Rc::new(RefCell::new(Inner {
                sets: Default::default(),
                rows,
                index,
                next_row: FIRST_USER_ROW,
                next_bnode: 1,
                registry: BTreeSet::new(),
                stats: None,
            })),
            costs: Rc::new(Cell::new(&MEMORY_COSTS)),
        }
    }

    /// All quads `[g,s,p,o]`, sorted.
    pub fn quads(&self) -> Vec<[TermId; 4]> {
        self.inner.borrow().sets[order_index(IndexOrder::Gspo)].iter().copied().collect()
    }

    /// Same algorithm as `sq-sqlite`'s analyze (hints only).
    pub fn analyze(&self) -> Arc<Stats> {
        let quads = self.quads();
        let stats = Arc::new(compute_stats(&quads));
        self.inner.borrow_mut().stats = Some(stats.clone());
        stats
    }

    pub fn set_costs(&self, c: CostConstants) {
        self.costs.set(Box::leak(Box::new(c)));
    }

    /// (row-major free-column values, number of rows). Width-0 specs have rows but no values.
    fn rows_for(&self, spec: &ScanSpec) -> (Vec<TermId>, usize) {
        let inner = self.inner.borrow();
        let set = &inner.sets[order_index(spec.order)];
        let k = spec.prefix_len as usize;
        let mut lo = [TermId(i64::MIN); 4];
        let mut hi = [TermId(i64::MAX); 4];
        lo[..k].copy_from_slice(&spec.prefix[..k]);
        hi[..k].copy_from_slice(&spec.prefix[..k]);
        if k < 4 {
            if let Bound::Included(v) | Bound::Excluded(v) = spec.lower {
                lo[k] = TermId(v);
            }
            if let Bound::Included(v) | Bound::Excluded(v) = spec.upper {
                hi[k] = TermId(v);
            }
        }
        let mut out = Vec::new();
        let mut n = 0;
        if lo > hi {
            return (out, 0);
        }
        for key in set.range(lo..=hi) {
            let q = unpermute(key, spec.order);
            if spec.matches(&q) {
                out.extend_from_slice(&key[k..]);
                n += 1;
            }
        }
        (out, n)
    }
}

struct MemCursor {
    store: MemStore,
    spec: ScanSpec,
    rows: Arc<Vec<TermId>>,
    nrows: usize,
    /// Row position (not value position).
    pos: usize,
    counters: CursorCounters,
}

impl QuadCursor for MemCursor {
    fn width(&self) -> usize {
        4 - self.spec.prefix_len as usize
    }
    fn fill(&mut self, out: &mut Vec<TermId>, max_rows: usize) -> Result<usize> {
        let w = self.width();
        let end = (self.pos + max_rows).min(self.nrows);
        if w > 0 {
            out.extend_from_slice(&self.rows[self.pos * w..end * w]);
        }
        let n = end.saturating_sub(self.pos);
        self.pos = end.max(self.pos);
        self.counters.rows += n as u64;
        self.counters.fills += 1;
        Ok(n)
    }
    fn seek(&mut self, key: &[TermId]) -> Result<()> {
        self.counters.seeks += 1;
        let w = self.width();
        if w == 0 || key.is_empty() {
            self.pos = 0;
            return Ok(());
        }
        let k = key.len().min(w);
        let n = self.nrows;
        let rows = &self.rows;
        let (mut lo, mut hi) = (0usize, n);
        while lo < hi {
            let mid = (lo + hi) / 2;
            if rows[mid * w..mid * w + k] < key[..k] {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        self.pos = lo;
        Ok(())
    }
    fn rebind(&mut self, prefix: &[TermId], lower: Bound<i64>, upper: Bound<i64>) -> Result<()> {
        let mut p = [TermId::UNBOUND; 4];
        p[..prefix.len()].copy_from_slice(prefix);
        self.spec = ScanSpec { prefix: p, lower, upper, ..self.spec };
        let (rows, n) = self.store.rows_for(&self.spec);
        self.rows = Arc::new(rows);
        self.nrows = n;
        self.pos = 0;
        Ok(())
    }
    fn counters(&self) -> CursorCounters {
        self.counters
    }
}

impl Dictionary for MemStore {
    fn lookup(&self, t: oxrdf::TermRef<'_>) -> Result<Option<TermId>> {
        if let Some(id) = codec::encode_inline(t) {
            return Ok(Some(id));
        }
        if let oxrdf::TermRef::BlankNode(b) = t {
            return Ok(codec::parse_bnode_label(b.as_str()));
        }
        let Some(key) = TermKey::of(t) else { return Ok(None) };
        Ok(self.inner.borrow().index.get(&key).copied())
    }
    fn decode(&self, ids: &[TermId], out: &mut Vec<Arc<Term>>) -> Result<()> {
        let inner = self.inner.borrow();
        for &id in ids {
            let t = if id.is_inline() {
                Arc::new(codec::decode_inline(id).ok_or_else(|| Error::internal(format!("bad inline id {id:?}")))?)
            } else if id.is_bnode() {
                Arc::new(Term::BNode(codec::bnode_label(id).into()))
            } else if id.is_dict() {
                inner
                    .rows
                    .get(id.dict_row() as usize)
                    .and_then(|r| r.clone())
                    .ok_or_else(|| Error::internal(format!("unknown dictionary id {id:?}")))?
            } else {
                return Err(Error::internal(format!("decode of non-storable id {id:?}")));
            };
            out.push(t);
        }
        Ok(())
    }
}

impl QuadStore for MemStore {
    fn cursor(&self, spec: &ScanSpec) -> Result<Box<dyn QuadCursor>> {
        let (rows, nrows) = self.rows_for(spec);
        Ok(Box::new(MemCursor {
            store: self.clone(),
            spec: *spec,
            rows: Arc::new(rows),
            nrows,
            pos: 0,
            counters: CursorCounters { reader: ReaderKind::Memory, ..Default::default() },
        }))
    }
    fn count(&self, spec: &ScanSpec, cap: Option<u64>) -> Result<u64> {
        let n = self.rows_for(spec).1 as u64;
        Ok(cap.map_or(n, |c| n.min(c + 1)))
    }
    fn dict(&self) -> &dyn Dictionary {
        self
    }
    fn named_graphs(&self) -> Result<Vec<TermId>> {
        let inner = self.inner.borrow();
        let mut g: BTreeSet<TermId> = inner.registry.clone();
        // Distinct g of the named-only GSPO-like order: walk Spog (g is last) is costly; use Gspo.
        let gspo = &inner.sets[order_index(IndexOrder::Gspo)];
        let mut cur = gspo.range([TermId(1), TermId(i64::MIN), TermId(i64::MIN), TermId(i64::MIN)]..).next().copied();
        while let Some(k) = cur {
            g.insert(k[0]);
            if k[0].0 == i64::MAX {
                break;
            }
            cur = gspo.range([TermId(k[0].0 + 1), TermId(i64::MIN), TermId(i64::MIN), TermId(i64::MIN)]..).next().copied();
        }
        // negative graph ids (never produced by the codec, but keep the contract total)
        for k in gspo.range(..[TermId(0), TermId(i64::MIN), TermId(i64::MIN), TermId(i64::MIN)]) {
            g.insert(k[0]);
        }
        g.remove(&TermId::DEFAULT_GRAPH);
        Ok(g.into_iter().collect())
    }
    fn stats(&self) -> Option<Arc<Stats>> {
        self.inner.borrow().stats.clone()
    }
    fn costs(&self) -> &CostConstants {
        self.costs.get()
    }
    fn caps(&self) -> StoreCaps {
        StoreCaps { sql: false, reader: ReaderKind::Memory }
    }
}

impl QuadStoreWriter for MemStore {
    fn intern(&self, t: oxrdf::TermRef<'_>) -> Result<TermId> {
        if let Some(id) = codec::encode_inline(t) {
            return Ok(id);
        }
        if let oxrdf::TermRef::BlankNode(_) = t {
            return Err(Error::Invalid("blank nodes cannot be interned; use fresh_bnode()".into()));
        }
        let key = TermKey::of(t).ok_or_else(|| Error::Invalid(format!("cannot intern {t}")))?;
        if let Some(id) = self.inner.borrow().index.get(&key) {
            return Ok(*id);
        }
        // Mirror sq-sqlite: a TYPED literal's datatype IRI is encoded first.
        if let oxrdf::TermRef::Literal(l) = t {
            if key.tag == Tag::Typed {
                self.intern(l.datatype().into())?;
            }
        }
        let term = Term::from_oxrdf(t);
        let mut inner = self.inner.borrow_mut();
        let row = inner.next_row;
        inner.next_row += 1;
        let id = TermId::from_dict(row, key.tag);
        if inner.rows.len() <= row as usize {
            inner.rows.resize(row as usize + 1, None);
        }
        inner.rows[row as usize] = Some(Arc::new(term));
        inner.index.insert(key, id);
        Ok(id)
    }
    fn fresh_bnode(&self) -> Result<TermId> {
        let mut inner = self.inner.borrow_mut();
        let n = inner.next_bnode;
        inner.next_bnode += 1;
        Ok(TermId::bnode(n))
    }
    fn insert(&self, quads: &mut Vec<[TermId; 4]>) -> Result<u64> {
        quads.sort();
        quads.dedup();
        for q in quads.iter() {
            let g_ok = q[0] == TermId::DEFAULT_GRAPH || q[0].is_storable();
            if !g_ok || !q[1..].iter().all(|t| t.is_storable()) {
                return Err(Error::internal(format!("insert of non-storable quad {q:?}")));
            }
        }
        let mut inner = self.inner.borrow_mut();
        let mut new = 0;
        for q in quads.iter() {
            if inner.sets[0].insert(permute(q, IndexOrder::ALL[0])) {
                new += 1;
                for (i, order) in IndexOrder::ALL.iter().enumerate().skip(1) {
                    if belongs(q, *order) {
                        inner.sets[i].insert(permute(q, *order));
                    }
                }
            }
            if q[0] != TermId::DEFAULT_GRAPH {
                inner.registry.insert(q[0]);
            }
        }
        Ok(new)
    }
    fn remove(&self, quads: &mut Vec<[TermId; 4]>) -> Result<u64> {
        quads.sort();
        quads.dedup();
        let mut inner = self.inner.borrow_mut();
        let mut removed = 0;
        for q in quads.iter() {
            if inner.sets[0].remove(&permute(q, IndexOrder::ALL[0])) {
                removed += 1;
                for (i, order) in IndexOrder::ALL.iter().enumerate().skip(1) {
                    if belongs(q, *order) {
                        inner.sets[i].remove(&permute(q, *order));
                    }
                }
            }
        }
        Ok(removed)
    }
    fn clear(&self, target: GraphTarget) -> Result<u64> {
        let mut victims: Vec<[TermId; 4]> = self
            .quads()
            .into_iter()
            .filter(|q| match target {
                GraphTarget::Default => q[0] == TermId::DEFAULT_GRAPH,
                GraphTarget::Named(g) => q[0] == g,
                GraphTarget::AllNamed => q[0] != TermId::DEFAULT_GRAPH,
                GraphTarget::All => true,
            })
            .collect();
        self.remove(&mut victims)
    }
    fn create_graph(&self, g: TermId) -> Result<bool> {
        if !g.is_storable() || g == TermId::DEFAULT_GRAPH {
            return Err(Error::Invalid(format!("not a storable graph name: {g:?}")));
        }
        let exists = self.named_graphs()?.contains(&g);
        self.inner.borrow_mut().registry.insert(g);
        Ok(!exists)
    }
    fn drop_graph(&self, g: TermId) -> Result<bool> {
        Ok(self.inner.borrow_mut().registry.remove(&g))
    }
}

/// Statistics over one scope's distinct (s,p,o) triples (sorted by s,p,o).
fn scope_stats(spo: &[[TermId; 3]]) -> ScopeStats {
    let mut st = ScopeStats { n: spo.len() as u64, ..Default::default() };
    let mut subjects = 0u64;
    let mut preds: FxHashSet<TermId> = FxHashSet::default();
    let mut objs: FxHashSet<TermId> = FxHashSet::default();
    let mut pred_objs: FxHashMap<TermId, FxHashSet<TermId>> = FxHashMap::default();
    let mut po: FxHashMap<(TermId, TermId), u64> = FxHashMap::default();
    let mut cs: FxHashMap<Vec<TermId>, (u64, Vec<u64>)> = FxHashMap::default();
    let mut i = 0;
    while i < spo.len() {
        let s = spo[i][0];
        subjects += 1;
        let mut j = i;
        let mut set: Vec<(TermId, u64)> = Vec::new();
        while j < spo.len() && spo[j][0] == s {
            let [_, p, o] = spo[j];
            preds.insert(p);
            objs.insert(o);
            let ps = st.pred.entry(p).or_insert_with(PredStats::default);
            ps.n += 1;
            if set.last().map(|x| x.0) != Some(p) {
                ps.ds += 1;
                set.push((p, 0));
            }
            set.last_mut().unwrap().1 += 1;
            if let Some(v) = o.as_int() {
                ps.imin = Some(ps.imin.map_or(v, |m| m.min(v)));
                ps.imax = Some(ps.imax.map_or(v, |m| m.max(v)));
            }
            pred_objs.entry(p).or_default().insert(o);
            *po.entry((p, o)).or_insert(0) += 1;
            j += 1;
        }
        let key: Vec<TermId> = set.iter().map(|x| x.0).collect();
        let e = cs.entry(key).or_insert_with(|| (0, vec![0; set.len()]));
        e.0 += 1;
        for (k, (_, c)) in set.iter().enumerate() {
            e.1[k] += c;
        }
        i = j;
    }
    st.ds = subjects;
    st.dp = preds.len() as u64;
    st.dobj = objs.len() as u64;
    for (p, os) in pred_objs {
        st.pred.get_mut(&p).unwrap().dobj = os.len() as u64;
    }
    let mut hh: Vec<((TermId, TermId), u64)> =
        po.into_iter().filter(|((p, _), n)| *n >= 100u64.max(st.pred[p].n / 1000)).collect();
    hh.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    hh.truncate(100_000);
    st.po = hh.into_iter().collect();
    let mut sets: Vec<(Vec<TermId>, (u64, Vec<u64>))> = cs.into_iter().collect();
    sets.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.0.cmp(&b.0)));
    sets.truncate(10_000);
    st.cs = sets
        .into_iter()
        .map(|(ps, (subjects, occ))| CharSet { subjects, preds: ps.into_iter().zip(occ).collect() })
        .collect();
    st
}

/// The shared statistics algorithm (sq-sqlite must produce identical numbers): scope 0 is the
/// default graph, scope 1 the RDF merge (distinct s,p,o) of all named graphs.
pub fn compute_stats(quads: &[[TermId; 4]]) -> Stats {
    let mut def: Vec<[TermId; 3]> = Vec::new();
    let mut named: Vec<[TermId; 3]> = Vec::new();
    for q in quads {
        let t = [q[1], q[2], q[3]];
        if q[0] == TermId::DEFAULT_GRAPH { def.push(t) } else { named.push(t) }
    }
    def.sort();
    def.dedup();
    named.sort();
    named.dedup();
    Stats { scope: [scope_stats(&def), scope_stats(&named)], analyzed_quads: quads.len() as u64 }
}

/// Convenience for tests: decode-free literal helper.
pub fn simple(lex: &str) -> Term {
    Term::Literal(Literal::simple(lex))
}

#[cfg(test)]
mod tests;
