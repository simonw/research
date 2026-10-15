//! §2.4 statistics. Per scope (0 = default graph through the g-first orders, 1 = the RDF
//! merge of all named graphs through the g-last partial orders, adjacent duplicates of
//! (s,p,o) across graphs removed):
//! * pass A (s-major: GSPO / SPOG): n, ds, per-predicate n / ds / imin / imax,
//!   characteristic sets;
//! * pass B (p-major: GPOS / POSG): per-predicate distinct objects, heavy-hitter (p,o) pairs;
//! * distinct objects of the scope: one `count(DISTINCT o)` in the VDBE (GOSP / OSPG).
//!
//! The numbers are identical to `sq_memstore::compute_stats` (verified by a test).
use std::rc::Rc;
use std::sync::Arc;

use rustc_hash::FxHashMap;
use sq_core::stats::{CharSet, PredStats, ScopeStats, Stats};
use sq_core::store::{IndexOrder, QuadCursor, ScanSpec};
use sq_core::{Error, Result, TermId};

use crate::cursor::SqlCursor;
use crate::pool::StmtPool;
use crate::raw::RawStmt;
use crate::shared::StorageShared;

const MAX_CHAR_SETS: usize = 10_000;
const MAX_HEAVY_HITTERS: usize = 100_000;
const HEAVY_HITTER_MIN: u64 = 100;
const FILL_ROWS: usize = 1024;

/// Streams the rows of `spec`, calling `f` with each row's free columns.
fn for_each_row(pool: &Rc<StmtPool>, spec: &ScanSpec, mut f: impl FnMut(&[TermId])) -> Result<()> {
    let mut cur = SqlCursor::open(pool.clone(), spec)?;
    let w = cur.width();
    let mut buf = Vec::with_capacity(FILL_ROWS * w);
    loop {
        buf.clear();
        if cur.fill(&mut buf, FILL_ROWS)? == 0 {
            return Ok(());
        }
        buf.chunks_exact(w).for_each(&mut f);
    }
}

/// Characteristic-set accumulator: (#subjects, occurrences per predicate of the set).
type CharSetAcc = (u64, Vec<u64>);

#[derive(Default)]
struct PassA {
    st: ScopeStats,
    subjects: u64,
    cur_s: Option<TermId>,
    /// (predicate, occurrences) of the current subject, in predicate order.
    set: Vec<(TermId, u64)>,
    cs: FxHashMap<Vec<TermId>, CharSetAcc>,
}

impl PassA {
    fn push(&mut self, s: TermId, p: TermId, o: TermId) {
        if self.cur_s != Some(s) {
            self.finish_subject();
            self.cur_s = Some(s);
            self.subjects += 1;
        }
        self.st.n += 1;
        let ps = self.st.pred.entry(p).or_default();
        ps.n += 1;
        if self.set.last().map(|x| x.0) != Some(p) {
            ps.ds += 1;
            self.set.push((p, 0));
        }
        if let Some(last) = self.set.last_mut() {
            last.1 += 1;
        }
        if let Some(v) = o.as_int() {
            ps.imin = Some(ps.imin.map_or(v, |m| m.min(v)));
            ps.imax = Some(ps.imax.map_or(v, |m| m.max(v)));
        }
    }

    fn finish_subject(&mut self) {
        if self.set.is_empty() {
            return;
        }
        let key: Vec<TermId> = self.set.iter().map(|x| x.0).collect();
        let e = self.cs.entry(key).or_insert_with(|| (0, vec![0; self.set.len()]));
        e.0 += 1;
        for (acc, (_, c)) in e.1.iter_mut().zip(&self.set) {
            *acc += c;
        }
        self.set.clear();
    }
}

#[derive(Default)]
struct PassB {
    cur: Option<(TermId, TermId)>,
    run: u64,
    po: Vec<((TermId, TermId), u64)>,
}

impl PassB {
    fn push(&mut self, st: &mut ScopeStats, p: TermId, o: TermId) {
        if self.cur != Some((p, o)) {
            self.finish_run(st);
            self.cur = Some((p, o));
            if let Some(ps) = st.pred.get_mut(&p) {
                ps.dobj += 1;
            }
        }
        self.run += 1;
    }

    fn finish_run(&mut self, st: &ScopeStats) {
        if let Some((p, o)) = self.cur.take() {
            let np = st.pred.get(&p).map_or(0, |ps| ps.n);
            if self.run >= HEAVY_HITTER_MIN.max(np / 1000) {
                self.po.push(((p, o), self.run));
            }
        }
        self.run = 0;
    }
}

fn count_distinct_objects(shared: &StorageShared, named: bool) -> Result<u64> {
    let sql = if named {
        "SELECT count(*) FROM (SELECT DISTINCT o FROM main.rdf_quad INDEXED BY rdf_ospg WHERE g<>0)"
    } else {
        "SELECT count(*) FROM (SELECT DISTINCT o FROM main.rdf_quad INDEXED BY rdf_gosp WHERE g=0)"
    };
    let mut st = RawStmt::prepare(shared.db(), sql, false)?;
    st.step()?;
    Ok(st.col_i64(0).max(0) as u64)
}

/// Statistics of one scope; also returns the number of stored quads it read.
fn scope_stats(shared: &StorageShared, named: bool) -> Result<(ScopeStats, u64)> {
    let pool = &shared.pool;
    let mut a = PassA::default();
    let mut raw_rows = 0u64;
    if named {
        let mut last: Option<[TermId; 3]> = None;
        for_each_row(pool, &ScanSpec::new(IndexOrder::Spog, &[]), |r| {
            raw_rows += 1;
            let spo = [r[0], r[1], r[2]];
            if last != Some(spo) {
                last = Some(spo);
                a.push(r[0], r[1], r[2]);
            }
        })?;
    } else {
        for_each_row(pool, &ScanSpec::new(IndexOrder::Gspo, &[TermId::DEFAULT_GRAPH]), |r| {
            raw_rows += 1;
            a.push(r[0], r[1], r[2]);
        })?;
    }
    a.finish_subject();
    let PassA { mut st, subjects, cs, .. } = a;

    let mut b = PassB::default();
    if named {
        let mut last: Option<[TermId; 3]> = None;
        for_each_row(pool, &ScanSpec::new(IndexOrder::Posg, &[]), |r| {
            let pos = [r[0], r[1], r[2]];
            if last != Some(pos) {
                last = Some(pos);
                b.push(&mut st, r[0], r[1]);
            }
        })?;
    } else {
        for_each_row(pool, &ScanSpec::new(IndexOrder::Gpos, &[TermId::DEFAULT_GRAPH]), |r| b.push(&mut st, r[0], r[1]))?;
    }
    b.finish_run(&st);

    st.ds = subjects;
    st.dp = st.pred.len() as u64;
    st.dobj = if st.n == 0 { 0 } else { count_distinct_objects(shared, named)? };
    let mut hh = b.po;
    hh.sort_by(|x, y| y.1.cmp(&x.1).then(x.0.cmp(&y.0)));
    hh.truncate(MAX_HEAVY_HITTERS);
    st.po = hh.into_iter().collect();
    let mut sets: Vec<(Vec<TermId>, CharSetAcc)> = cs.into_iter().collect();
    sets.sort_by(|x, y| y.1.0.cmp(&x.1.0).then(x.0.cmp(&y.0)));
    sets.truncate(MAX_CHAR_SETS);
    st.cs = sets
        .into_iter()
        .map(|(preds, (subjects, occ))| CharSet { subjects, preds: preds.into_iter().zip(occ).collect() })
        .collect();
    Ok((st, raw_rows))
}

/// `rdf_meta` counter of the store's net growth since the last analyze: quads inserted minus
/// quads removed through this crate (loads, `QuadStoreWriter` updates, clears), for the
/// §2.3 step 6 re-analysis trigger. Created by the first write that needs it, so
/// `sparql_init` writes exactly the §2.1 defaults. Plain-SQL writes to `rdf_quad` are not
/// counted (statistics are hints).
pub(crate) const GROWTH_SINCE_ANALYZE: &str = "growth_since_analyze";

/// Computes, persists and returns the statistics; bumps `stats_gen`. The caller holds a
/// write scope. Returns `(stats_gen, stats)`.
pub(crate) fn analyze_in_scope(shared: &StorageShared) -> Result<(i64, Arc<Stats>)> {
    let (s0, n0) = scope_stats(shared, false)?;
    let (s1, n1) = scope_stats(shared, true)?;
    let stats = Stats { scope: [s0, s1], analyzed_quads: n0 + n1 };
    persist(shared, &stats)?;
    let generation = shared.meta_get("stats_gen")?.unwrap_or(0);
    Ok((generation, Arc::new(stats)))
}

fn le_blob(v: impl Iterator<Item = i64>) -> Vec<u8> {
    v.flat_map(i64::to_le_bytes).collect()
}

fn from_le_blob(b: &[u8]) -> Result<Vec<i64>> {
    if !b.len().is_multiple_of(8) {
        return Err(Error::internal("rdf_stat_cs blob length is not a multiple of 8"));
    }
    Ok(b.chunks_exact(8).map(|c| i64::from_le_bytes(c.try_into().expect("8-byte chunk"))).collect())
}

fn persist(shared: &StorageShared, stats: &Stats) -> Result<()> {
    let db = shared.db();
    db.exec(
        "DELETE FROM main.rdf_stat_global; DELETE FROM main.rdf_stat_pred;
         DELETE FROM main.rdf_stat_po; DELETE FROM main.rdf_stat_cs;",
    )?;
    let mut g = RawStmt::prepare(db, "INSERT INTO main.rdf_stat_global(scope,n,ds,dp,dobj) VALUES (?1,?2,?3,?4,?5)", false)?;
    let mut p = RawStmt::prepare(
        db,
        "INSERT INTO main.rdf_stat_pred(scope,p,n,ds,dobj,imin,imax) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        false,
    )?;
    let mut po = RawStmt::prepare(db, "INSERT INTO main.rdf_stat_po(scope,p,o,n) VALUES (?1,?2,?3,?4)", false)?;
    let mut cs =
        RawStmt::prepare(db, "INSERT INTO main.rdf_stat_cs(scope,cs,subjects,preds,occ) VALUES (?1,?2,?3,?4,?5)", false)?;
    for (scope, st) in stats.scope.iter().enumerate() {
        let scope = scope as i64;
        g.reset();
        for (i, v) in [scope, st.n as i64, st.ds as i64, st.dp as i64, st.dobj as i64].into_iter().enumerate() {
            g.bind_i64(i as i32 + 1, v)?;
        }
        g.run()?;
        for (pred, ps) in &st.pred {
            p.reset();
            for (i, v) in [scope, pred.0, ps.n as i64, ps.ds as i64, ps.dobj as i64].into_iter().enumerate() {
                p.bind_i64(i as i32 + 1, v)?;
            }
            p.bind_opt_i64(6, ps.imin)?;
            p.bind_opt_i64(7, ps.imax)?;
            p.run()?;
        }
        for (&(pp, o), &n) in &st.po {
            po.reset();
            for (i, v) in [scope, pp.0, o.0, n as i64].into_iter().enumerate() {
                po.bind_i64(i as i32 + 1, v)?;
            }
            po.run()?;
        }
        for (k, set) in st.cs.iter().enumerate() {
            cs.reset();
            cs.bind_i64(1, scope)?;
            cs.bind_i64(2, k as i64)?;
            cs.bind_i64(3, set.subjects as i64)?;
            cs.bind_blob(4, &le_blob(set.preds.iter().map(|(p, _)| p.0)))?;
            cs.bind_blob(5, &le_blob(set.preds.iter().map(|(_, n)| *n as i64)))?;
            cs.run()?;
        }
    }
    shared.meta_set("analyzed_quads", stats.analyzed_quads as i64)?;
    // A no-op while the counter row does not exist yet (absent = 0).
    shared.meta_set(GROWTH_SINCE_ANALYZE, 0)?;
    shared.meta_bump("stats_gen")
}

/// Reads the persisted statistics: `(stats_gen, None if never analyzed)`.
pub(crate) fn load_stats(shared: &StorageShared) -> Result<(i64, Option<Stats>)> {
    let db = shared.db();
    let generation = shared.meta_get("stats_gen")?.unwrap_or(0);
    let mut stats = Stats::default();
    let scope_of = |v: i64| -> Result<usize> {
        usize::try_from(v).ok().filter(|s| *s < 2).ok_or_else(|| Error::internal(format!("bad statistics scope {v}")))
    };
    let mut any = false;
    let mut st = RawStmt::prepare(db, "SELECT scope, n, ds, dp, dobj FROM main.rdf_stat_global", false)?;
    while st.step()? {
        any = true;
        let s = &mut stats.scope[scope_of(st.col_i64(0))?];
        s.n = st.col_i64(1) as u64;
        s.ds = st.col_i64(2) as u64;
        s.dp = st.col_i64(3) as u64;
        s.dobj = st.col_i64(4) as u64;
    }
    if !any {
        return Ok((generation, None));
    }
    let mut st = RawStmt::prepare(db, "SELECT scope, p, n, ds, dobj, imin, imax FROM main.rdf_stat_pred", false)?;
    while st.step()? {
        let ps = PredStats {
            n: st.col_i64(2) as u64,
            ds: st.col_i64(3) as u64,
            dobj: st.col_i64(4) as u64,
            imin: st.col_opt_i64(5),
            imax: st.col_opt_i64(6),
        };
        stats.scope[scope_of(st.col_i64(0))?].pred.insert(TermId(st.col_i64(1)), ps);
    }
    let mut st = RawStmt::prepare(db, "SELECT scope, p, o, n FROM main.rdf_stat_po", false)?;
    while st.step()? {
        stats.scope[scope_of(st.col_i64(0))?].po.insert((TermId(st.col_i64(1)), TermId(st.col_i64(2))), st.col_i64(3) as u64);
    }
    let mut st =
        RawStmt::prepare(db, "SELECT scope, subjects, preds, occ FROM main.rdf_stat_cs ORDER BY scope, cs", false)?;
    while st.step()? {
        let preds = from_le_blob(st.col_bytes(2))?;
        let occ = from_le_blob(st.col_bytes(3))?;
        if preds.len() != occ.len() {
            return Err(Error::internal("rdf_stat_cs preds/occ length mismatch"));
        }
        let set = CharSet {
            subjects: st.col_i64(1) as u64,
            preds: preds.into_iter().map(TermId).zip(occ.into_iter().map(|n| n as u64)).collect(),
        };
        stats.scope[scope_of(st.col_i64(0))?].cs.push(set);
    }
    stats.analyzed_quads = shared.meta_get("analyzed_quads")?.unwrap_or(0).max(0) as u64;
    Ok((generation, Some(stats)))
}
