//! Release micro-benchmark of the operators over MemStore (ignored by default):
//! `cargo test --release -p sq-exec bench -- --ignored --nocapture`.
//! MemStore cursors copy their range at open / rebind, so probe-heavy shapes measure the
//! store more than the executor; scan / hash / merge / sort / group numbers are the
//! executor's per-row overheads.
use std::cell::RefCell;
use std::time::Instant;

use rustc_hash::FxHashMap;
use sq_core::ir::{AggFunc, AggSpec, PathPlan};

use super::*;

/// A store whose cursors replay ranges materialized once (so the numbers below measure the
/// executor, not MemStore's per-open range copies).
type Ranges = Rc<RefCell<FxHashMap<ScanSpec, Rc<Vec<TermId>>>>>;

struct Cached {
    inner: MemStore,
    ranges: Ranges,
}

struct Replay {
    width: usize,
    spec: ScanSpec,
    inner: MemStore,
    ranges: Ranges,
    rows: Rc<Vec<TermId>>,
    pos: usize,
}

fn range_rows(inner: &MemStore, ranges: &Ranges, spec: &ScanSpec) -> Rc<Vec<TermId>> {
    if let Some(r) = ranges.borrow().get(spec) {
        return r.clone();
    }
    let mut c = inner.cursor(spec).unwrap();
    let mut v = Vec::new();
    while c.fill(&mut v, 1 << 20).unwrap() > 0 {}
    let r = Rc::new(v);
    ranges.borrow_mut().insert(*spec, r.clone());
    r
}

impl QuadCursor for Replay {
    fn width(&self) -> usize {
        self.width
    }
    fn fill(&mut self, out: &mut Vec<TermId>, max_rows: usize) -> Result<usize> {
        let w = self.width.max(1);
        let n = ((self.rows.len() - self.pos) / w).min(max_rows);
        out.extend_from_slice(&self.rows[self.pos..self.pos + n * w]);
        self.pos += n * w;
        Ok(n)
    }
    fn seek(&mut self, key: &[TermId]) -> Result<()> {
        let w = self.width;
        let n = self.rows.len() / w;
        let (mut lo, mut hi) = (0, n);
        while lo < hi {
            let mid = (lo + hi) / 2;
            if self.rows[mid * w..mid * w + key.len()] < *key {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        self.pos = lo * w;
        Ok(())
    }
    fn rebind(&mut self, p: &[TermId], l: Bound<i64>, u: Bound<i64>) -> Result<()> {
        let mut prefix = [TermId::UNBOUND; 4];
        prefix[..p.len()].copy_from_slice(p);
        self.spec = ScanSpec { prefix, lower: l, upper: u, ..self.spec };
        self.rows = range_rows(&self.inner, &self.ranges, &self.spec);
        self.pos = 0;
        Ok(())
    }
    fn counters(&self) -> sq_core::store::CursorCounters {
        Default::default()
    }
}

impl Cached {
    fn rows(&self, spec: &ScanSpec) -> Rc<Vec<TermId>> {
        range_rows(&self.inner, &self.ranges, spec)
    }
}

impl QuadStore for Cached {
    fn cursor(&self, spec: &ScanSpec) -> Result<Box<dyn QuadCursor>> {
        Ok(Box::new(Replay {
            width: 4 - spec.prefix_len as usize,
            spec: *spec,
            inner: self.inner.clone(),
            ranges: self.ranges.clone(),
            rows: self.rows(spec),
            pos: 0,
        }))
    }
    fn count(&self, spec: &ScanSpec, cap: Option<u64>) -> Result<u64> {
        let n = (self.rows(spec).len() / (4 - spec.prefix_len as usize).max(1)) as u64;
        Ok(cap.map_or(n, |c| n.min(c + 1)))
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
        StoreCaps::default()
    }
}

const N: i64 = 200_000;

fn setup() -> (Fx, TermId, TermId, TermId) {
    let fx = Fx::new();
    let (p, q, knows) = (fx.iri("p"), fx.iri("q"), fx.iri("knows"));
    let subjects: Vec<TermId> = (0..N).map(|i| fx.iri(&format!("s{i}"))).collect();
    let mut quads = Vec::with_capacity(3 * N as usize);
    for (i, s) in subjects.iter().enumerate() {
        let i = i as i64;
        quads.push([TermId::DEFAULT_GRAPH, *s, p, int((i * 7919) % 100_000)]);
        quads.push([TermId::DEFAULT_GRAPH, *s, q, int(i % 1000)]);
        if i % 10 == 0 {
            quads.push([TermId::DEFAULT_GRAPH, *s, knows, subjects[((i * 31) % N) as usize]]);
        }
    }
    fx.store.insert(&mut quads).unwrap();
    (fx, p, q, knows)
}

fn time(name: &str, rows_in: i64, f: impl Fn() -> u64) {
    f();
    let t = Instant::now();
    let out = f();
    let ns = t.elapsed().as_nanos() as f64;
    println!("{name:<36} {:>8.1} ns/input row  ({out} rows out, {:.1} ms)", ns / rows_in as f64, ns / 1e6);
}

thread_local! {
    static STORE: RefCell<Option<Rc<Cached>>> = const { RefCell::new(None) };
}

fn store_of(fx: &Fx) -> Rc<dyn QuadStore> {
    STORE.with(|s| {
        s.borrow_mut()
            .get_or_insert_with(|| Rc::new(Cached { inner: fx.store.clone(), ranges: Ranges::default() }))
            .clone()
    })
}

fn count(fx: &Fx, width: usize, p: Plan) -> u64 {
    // Warm the range cache, then measure.
    let _ = Q::new(&fx.store, width, p.clone()).exec(store_of(fx)).count();
    let mut e = Q::new(&fx.store, width, p).exec(store_of(fx));
    let mut buf = Vec::new();
    let mut n = 0;
    loop {
        buf.clear();
        let got = e.next_rows(&mut buf, 1024).unwrap();
        if got == 0 {
            return n;
        }
        n += got as u64;
    }
}

#[test]
#[ignore]
fn bench_operators() {
    let (fx, p, q, knows) = setup();
    let s = |a: Slot, pr: TermId, b: Slot| scan(v(a), c(pr), v(b));
    time("scan", N, || count(&fx, 3, s(0, p, 1)));
    time("filter ?o > 50000", N, || {
        count(&fx, 3, filter(s(0, p, 1), Expr::Greater(Box::new(sv(1)), Box::new(cv(int(50_000))))))
    });
    time("extend ?o + 1", N, || count(&fx, 3, extend(s(0, p, 1), 2, Expr::Add(Box::new(sv(1)), Box::new(cv(int(1)))))));
    time("hash join (s)", 2 * N, || {
        count(&fx, 3, hash_join(s(0, q, 2), s(0, p, 1), &[0], &[], JoinKind::Inner, false))
    });
    time("merge join (s)", 2 * N, || count(&fx, 3, merge_join(s(0, p, 1), s(0, q, 2), 0, &[], 6)));
    time("merge join count (factorized)", 2 * N, || {
        let mut e = Q::new(&fx.store, 3, merge_join(s(0, p, 1), s(0, q, 2), 0, &[], 6)).exec(store_of(&fx));
        e.count().unwrap()
    });
    time("sort by ?o (INT fast path)", N, || {
        count(&fx, 3, plan(Op::Sort { input: Box::new(s(0, p, 1)), keys: vec![order_key(sv(1), false)] }))
    });
    time("sort by -?o (SortKeys)", N, || {
        let key = Expr::UnaryMinus(Box::new(sv(1)));
        count(&fx, 3, plan(Op::Sort { input: Box::new(s(0, p, 1)), keys: vec![order_key(key, false)] }))
    });
    time("top 10 by ?o", N, || {
        count(&fx, 3, plan(Op::TopK { input: Box::new(s(0, p, 1)), keys: vec![order_key(sv(1), true)], k: 10 }))
    });
    time("group by ?o count(*) (1000 groups)", N, || {
        let agg = AggSpec { func: AggFunc::CountAll, distinct: false, arg: None };
        count(
            &fx,
            3,
            plan(Op::Group { input: Box::new(s(0, q, 1)), keys: vec![1], aggs: vec![(2, agg)], streaming: false }),
        )
    });
    time("distinct ?o", N, || {
        count(&fx, 3, plan(Op::Distinct { input: Box::new(project(s(0, p, 1), &[1])), slots: vec![1] }))
    });
    time("count(*) scan (store count)", N, || {
        let mut e = Q::new(&fx.store, 3, s(0, p, 1)).exec(store_of(&fx));
        e.count().unwrap()
    });
    // Probe-heavy shapes on MemStore itself (cursor rebinds copy the probed range).
    let mem = |width: usize, p: Plan| {
        let mut e = Q::new(&fx.store, width, p).exec(fx.rc());
        e.count().unwrap()
    };
    time("bind join knows -> p (20K probes, mem)", N / 10, || {
        mem(4, bind_join(s(0, knows, 3), scan_bound(GraphSel::Default, v(3), c(p), v(1), &[3]), &[3], JoinKind::Inner))
    });
    time("knows+ from one node", N / 10, || {
        let path = PathPlan::OneOrMore(Box::new(PathPlan::Pred(knows)));
        let start = fx.lookup("s10").unwrap();
        count(&fx, 2, plan(Op::Path { g: GraphSel::Default, s: c(start), path, o: v(0) }))
    });
}

#[test]
#[ignore]
fn bench_hash_join_only() {
    let (fx, p, q, _) = setup();
    let s = |a: Slot, pr: TermId, b: Slot| scan(v(a), c(pr), v(b));
    time("hash join (s)", 2 * N, || {
        count(&fx, 3, hash_join(s(0, q, 2), s(0, p, 1), &[0], &[], JoinKind::Inner, false))
    });
    time("scan", N, || count(&fx, 3, s(0, p, 1)));
}
