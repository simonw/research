//! Shared test utilities (feature "testing"): deterministic random quads and the QuadCursor
//! conformance suite run against MemStore, SqliteStore (SQL cursors) and the phase-2 reader.
use std::ops::Bound;
use std::sync::Arc;

use crate::codec::encode_decimal;
use crate::error::{Error, Result};
use crate::id::{Tag, TermId};
use crate::stats::{CostConstants, Stats, MEMORY_COSTS};
use crate::store::*;
use crate::term::Term;

/// xorshift64* — deterministic, dependency-free.
pub struct Rng(pub u64);
impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n.max(1)
    }
}

/// A small-domain random term id covering every storable tag (incl. negative ints, so the
/// signed order matters, and tags 5/8 so rdf_gpox is exercised).
pub fn random_id(r: &mut Rng, domain: i64) -> TermId {
    let v = (r.below(domain as u64) as i64) + 1;
    match r.below(8) {
        0..=2 => TermId::from_dict(1024 + v, Tag::Iri),
        3 => TermId::from_int(v - domain / 2).unwrap(),
        4 => TermId::from_dict(1024 + v, Tag::Typed),
        5 => encode_decimal(v - domain / 2, 1, false),
        6 => TermId::bnode(v),
        _ => TermId::from_dict(1024 + v, Tag::Str),
    }
}

/// Random quads: g in {0, a few named graphs}, s/p/o from small domains (many duplicates
/// prefixes, so prefix scans return several rows). Sorted, deduplicated, [g,s,p,o].
pub fn random_quads(seed: u64, n: usize, named: bool) -> Vec<[TermId; 4]> {
    let mut r = Rng(seed | 1);
    let mut v: Vec<[TermId; 4]> = (0..n)
        .map(|_| {
            let g = if named && r.below(3) == 0 { TermId::from_dict(2000 + r.below(3) as i64, Tag::Iri) } else { TermId::DEFAULT_GRAPH };
            let s = if r.below(4) == 0 { TermId::bnode(1 + r.below(20) as i64) } else { TermId::from_dict(1024 + r.below(40) as i64, Tag::Iri) };
            let p = TermId::from_dict(1100 + r.below(6) as i64, Tag::Iri);
            let o = random_id(&mut r, 50);
            [g, s, p, o]
        })
        .collect();
    v.sort();
    v.dedup();
    v
}

fn project(q: &[TermId; 4], spec: &ScanSpec) -> Vec<TermId> {
    spec.free_cols().iter().map(|c| q[*c as usize]).collect()
}

/// Brute-force expected rows for a spec.
pub fn expected_rows(quads: &[[TermId; 4]], spec: &ScanSpec) -> Vec<Vec<TermId>> {
    let mut rows: Vec<Vec<TermId>> = quads.iter().filter(|q| spec.matches(q)).map(|q| project(q, spec)).collect();
    rows.sort();
    rows.dedup();
    rows
}

fn drain(c: &mut dyn QuadCursor, max: usize) -> Result<Vec<Vec<TermId>>> {
    let w = c.width();
    let mut buf = Vec::new();
    let mut rows = 0;
    loop {
        let n = c.fill(&mut buf, max)?;
        if n == 0 {
            break;
        }
        rows += n;
    }
    assert_eq!(buf.len(), rows * w, "fill must append width() values per row");
    Ok((0..rows).map(|i| buf[i * w..(i + 1) * w].to_vec()).collect())
}

fn random_bound(r: &mut Rng, rows: &[Vec<TermId>]) -> Bound<i64> {
    let v = if rows.is_empty() || r.below(4) == 0 { (r.next_u64() as i64) >> 4 } else { rows[r.below(rows.len() as u64) as usize][0].0 };
    match r.below(3) {
        0 => Bound::Unbounded,
        1 => Bound::Included(v),
        _ => Bound::Excluded(v),
    }
}

/// The conformance suite. `store` must contain exactly `quads` (plus nothing else).
/// Checks, for every order x prefix_len (0..=4) x random prefixes (present and absent) x
/// random ranges: full drains with several fill sizes, forward/backward seeks with 1..=width
/// key columns, and rebind. Panics with a descriptive message on the first mismatch.
pub fn cursor_conformance(store: &dyn QuadStore, quads: &[[TermId; 4]], seed: u64, iterations: usize) {
    let mut r = Rng(seed | 1);
    for order in IndexOrder::ALL {
        for prefix_len in 0..=4u8 {
            for _ in 0..iterations {
                // prefix taken from a real quad (or perturbed to be absent)
                let mut prefix = [TermId::UNBOUND; 4];
                if !quads.is_empty() {
                    let q = quads[r.below(quads.len() as u64) as usize];
                    for i in 0..prefix_len as usize {
                        prefix[i] = q[order.cols()[i] as usize];
                    }
                    if r.below(5) == 0 && prefix_len > 0 {
                        prefix[prefix_len as usize - 1] = TermId(prefix[prefix_len as usize - 1].0 + 16);
                    }
                }
                let mut spec = ScanSpec::new(order, &prefix[..prefix_len as usize]);
                if prefix_len < 4 && r.below(2) == 0 {
                    let all = expected_rows(quads, &spec);
                    spec.lower = random_bound(&mut r, &all);
                    spec.upper = random_bound(&mut r, &all);
                }
                let want = expected_rows(quads, &spec);
                let mut cur = store.cursor(&spec).unwrap_or_else(|e| panic!("cursor({spec:?}): {e}"));
                assert_eq!(cur.width(), 4 - prefix_len as usize, "width {spec:?}");
                let max = [1usize, 3, 1024][r.below(3) as usize];
                let got = drain(cur.as_mut(), max).unwrap();
                assert_eq!(got, want, "drain mismatch for {spec:?} (fill max {max})");
                let cnt = store.count(&spec, None).unwrap();
                assert_eq!(cnt as usize, want.len(), "count mismatch for {spec:?}");
                let cap = r.below(5);
                assert_eq!(store.count(&spec, Some(cap)).unwrap(), (want.len() as u64).min(cap + 1), "capped count {spec:?}");
                // seeks (forward and backward), 1..=width key columns
                if prefix_len < 4 {
                    for _ in 0..4 {
                        let klen = 1 + r.below((4 - prefix_len) as u64) as usize;
                        let key: Vec<TermId> = if !want.is_empty() && r.below(3) != 0 {
                            want[r.below(want.len() as u64) as usize][..klen].to_vec()
                        } else {
                            (0..klen).map(|_| random_id(&mut r, 60)).collect()
                        };
                        cur.seek(&key).unwrap();
                        let got = drain(cur.as_mut(), 7).unwrap();
                        let exp: Vec<Vec<TermId>> = want.iter().filter(|row| row[..klen] >= key[..]).cloned().collect();
                        assert_eq!(got, exp, "seek({key:?}) mismatch for {spec:?}");
                    }
                }
                // rebind to another prefix of the same length
                if !quads.is_empty() {
                    let q = quads[r.below(quads.len() as u64) as usize];
                    let mut p2 = [TermId::UNBOUND; 4];
                    for i in 0..prefix_len as usize {
                        p2[i] = q[order.cols()[i] as usize];
                    }
                    let spec2 = ScanSpec { prefix: p2, ..spec };
                    cur.rebind(&p2[..prefix_len as usize], spec.lower, spec.upper).unwrap();
                    let got = drain(cur.as_mut(), 1024).unwrap();
                    assert_eq!(got, expected_rows(quads, &spec2), "rebind mismatch for {spec2:?}");
                }
            }
        }
    }
}

/// Reference store used to self-test the suite (sorted Vec + brute force). Dictionary only
/// knows inline/bnode ids.
pub struct BruteStore {
    pub quads: Arc<Vec<[TermId; 4]>>,
}

struct BruteCursor {
    quads: Arc<Vec<[TermId; 4]>>,
    spec: ScanSpec,
    rows: Vec<Vec<TermId>>,
    pos: usize,
    seeks: u64,
}

impl QuadCursor for BruteCursor {
    fn width(&self) -> usize {
        4 - self.spec.prefix_len as usize
    }
    fn fill(&mut self, out: &mut Vec<TermId>, max_rows: usize) -> Result<usize> {
        let end = (self.pos + max_rows).min(self.rows.len());
        for r in &self.rows[self.pos..end] {
            out.extend_from_slice(r);
        }
        let n = end - self.pos;
        self.pos = end;
        Ok(n)
    }
    fn seek(&mut self, key: &[TermId]) -> Result<()> {
        self.seeks += 1;
        self.pos = self.rows.partition_point(|r| r[..key.len()] < *key);
        Ok(())
    }
    fn rebind(&mut self, prefix: &[TermId], lower: Bound<i64>, upper: Bound<i64>) -> Result<()> {
        let mut p = [TermId::UNBOUND; 4];
        p[..prefix.len()].copy_from_slice(prefix);
        self.spec = ScanSpec { prefix: p, lower, upper, ..self.spec };
        self.rows = expected_rows(&self.quads, &self.spec);
        self.pos = 0;
        Ok(())
    }
    fn counters(&self) -> CursorCounters {
        CursorCounters { rows: self.pos as u64, seeks: self.seeks, fills: 0, reader: ReaderKind::Memory }
    }
}

struct NoDict;
impl Dictionary for NoDict {
    fn lookup(&self, t: oxrdf::TermRef<'_>) -> Result<Option<TermId>> {
        Ok(crate::codec::encode_inline(t))
    }
    fn decode(&self, ids: &[TermId], out: &mut Vec<Arc<Term>>) -> Result<()> {
        for id in ids {
            out.push(Arc::new(crate::codec::decode_inline(*id).ok_or_else(|| Error::internal("BruteStore cannot decode"))?));
        }
        Ok(())
    }
}

impl QuadStore for BruteStore {
    fn cursor(&self, spec: &ScanSpec) -> Result<Box<dyn QuadCursor>> {
        Ok(Box::new(BruteCursor { quads: self.quads.clone(), spec: *spec, rows: expected_rows(&self.quads, spec), pos: 0, seeks: 0 }))
    }
    fn count(&self, spec: &ScanSpec, cap: Option<u64>) -> Result<u64> {
        let n = expected_rows(&self.quads, spec).len() as u64;
        Ok(cap.map_or(n, |c| n.min(c + 1)))
    }
    fn dict(&self) -> &dyn Dictionary {
        &NoDict
    }
    fn named_graphs(&self) -> Result<Vec<TermId>> {
        let mut g: Vec<TermId> = self.quads.iter().map(|q| q[0]).filter(|g| g.is_bound()).collect();
        g.sort();
        g.dedup();
        Ok(g)
    }
    fn stats(&self) -> Option<Arc<Stats>> {
        None
    }
    fn costs(&self) -> &CostConstants {
        &MEMORY_COSTS
    }
    fn caps(&self) -> StoreCaps {
        StoreCaps::default()
    }
}
