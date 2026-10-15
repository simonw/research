//! Property paths (§5.5): `Op::Path { g, s, path, o }`.
//!
//! A path is evaluated by `step(path, dir, starts) -> (start, end) pairs` over sorted,
//! deduplicated start sets:
//! * `Pred(p)`: a probe per start (forward GSPO `(g,s,p)`, backward GPOS `(g,p,o)`), or one
//!   scan of all p edges semi-joined with the starts when that is cheaper
//!   (`|starts| * seek_sorted > |edges(p)| * scan_row`);
//! * `Inv` flips the direction, `Seq` composes, `Alt` concatenates (bag semantics);
//! * `Nps(set)` scans `(g,s)` (backward `(g,o)`) and drops predicates in the set;
//! * `*`, `+`: multi-source BFS by level with a visited set of (start, node) pairs, so it
//!   terminates on cycles and has set semantics per (start, end); `?` likewise.
//!
//! Endpoints: a bound subject evaluates forward, a bound object backward over the inverse;
//! when both are bound the path runs from a constant side (forward by default, with early exit
//! for `*` / `+`) and keeps the pairs ending on the other value; with both free, one BFS per
//! start candidate (the possible first-step subjects; every node of the graph when the path
//! can be zero-length). Zero-length rules:
//! * a constant (or Substitute-mode seeded) endpoint matches itself even when it is absent
//!   from the graph;
//! * a Join-mode seeded endpoint must satisfy `has_node` (it occurs as a subject or object in
//!   the active graph), so a seeded evaluation equals the free one filtered by the seed;
//! * both endpoints free: every node of the active graph (loose index scans of the distinct
//!   subjects on GSPO and objects on GOSP); `?x p* ?x` binds every node, `?x p+ ?x` needs a
//!   cycle.
//!
//! Paths never cross graphs: the edge scans read the default graph (merged when it has several
//! graphs), the GraphLoop register (`Active`), a constant graph, or — for `GraphSel::Slot` —
//! each named graph in turn, pushed on the register by this operator.
use rustc_hash::FxHashSet;
use sq_core::ir::{GraphSel, PatTerm, PathPlan, QuadPat};
use sq_core::store::IndexOrder;
use sq_core::{BATCH_ROWS, Batch, Error, Result, Slot, SlotSet, TermId};

use super::Operator;
use super::scan::Scan;
use crate::ctx::{Charge, ExecCtx};
use crate::seed::{Seed, SeedBuf};

/// Private row layout of the edge scans.
const PW: usize = 3;
const S: Slot = 0;
const O: Slot = 1;
const P: Slot = 2;
/// Start candidates evaluated per step when both endpoints are free.
const CHUNK_STARTS: usize = 256;
/// Bytes charged per (start, end) pair held and per visited entry.
const PAIR_BYTES: usize = 16;
const VISITED_BYTES: usize = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dir {
    Fwd,
    Bwd,
}

impl Dir {
    fn flip(self) -> Dir {
        match self {
            Dir::Fwd => Dir::Bwd,
            Dir::Bwd => Dir::Fwd,
        }
    }
}

fn pat(g: GraphSel, p: PatTerm) -> QuadPat {
    QuadPat { g, s: PatTerm::Slot(S), p, o: PatTerm::Slot(O) }
}

fn order_for(bound: [bool; 4]) -> IndexOrder {
    IndexOrder::candidates(bound)[0].0
}

/// The edge scans of a predicate.
struct PredScans {
    /// (g, s, p) -> o
    fwd: Scan,
    /// (g, p, o) -> s
    bwd: Scan,
    /// (g, p) -> s, o: the p edges in subject order.
    by_s: Scan,
    /// (g, p) -> o, s: the p edges in object order.
    by_o: Scan,
    /// |edges(p)| for the register value it was counted under (None: not countable).
    edges: Option<(TermId, Option<u64>)>,
}

/// The edge scans of a negated property set.
struct NpsScans {
    set: Vec<TermId>,
    /// (g, s) -> p, o
    fwd: Scan,
    /// (g, o) -> s, p
    bwd: Scan,
}

/// A path with its edge scans.
enum CPath {
    Pred(Box<PredScans>),
    Inv(Box<CPath>),
    Seq(Box<CPath>, Box<CPath>),
    Alt(Box<CPath>, Box<CPath>),
    Star(Box<CPath>),
    Plus(Box<CPath>),
    Opt(Box<CPath>),
    Nps(Box<NpsScans>),
}

impl CPath {
    fn new(p: &PathPlan, g: GraphSel) -> CPath {
        let b = |s: bool, p: bool, o: bool| order_for([true, s, p, o]);
        match p {
            PathPlan::Pred(id) => {
                let pt = PatTerm::Const(*id);
                CPath::Pred(Box::new(PredScans {
                    fwd: Scan::internal(pat(g, pt), b(true, true, false), PW),
                    bwd: Scan::internal(pat(g, pt), b(false, true, true), PW),
                    by_s: Scan::internal(pat(g, pt), IndexOrder::Gpso, PW),
                    by_o: Scan::internal(pat(g, pt), IndexOrder::Gpos, PW),
                    edges: None,
                }))
            }
            PathPlan::Inv(q) => CPath::Inv(Box::new(CPath::new(q, g))),
            PathPlan::Seq(a, c) => CPath::Seq(Box::new(CPath::new(a, g)), Box::new(CPath::new(c, g))),
            PathPlan::Alt(a, c) => CPath::Alt(Box::new(CPath::new(a, g)), Box::new(CPath::new(c, g))),
            PathPlan::ZeroOrMore(q) => CPath::Star(Box::new(CPath::new(q, g))),
            PathPlan::OneOrMore(q) => CPath::Plus(Box::new(CPath::new(q, g))),
            PathPlan::ZeroOrOne(q) => CPath::Opt(Box::new(CPath::new(q, g))),
            PathPlan::Nps(set) => {
                let mut set = set.clone();
                set.sort();
                set.dedup();
                let pt = PatTerm::Slot(P);
                CPath::Nps(Box::new(NpsScans {
                    set,
                    fwd: Scan::internal(pat(g, pt), b(true, false, false), PW),
                    bwd: Scan::internal(pat(g, pt), b(false, false, true), PW),
                }))
            }
        }
    }

    /// Can the path match a zero-length path (the `has_node` rule applies)?
    fn nullable(&self) -> bool {
        match self {
            CPath::Pred(_) | CPath::Nps(_) => false,
            CPath::Star(_) | CPath::Opt(_) => true,
            CPath::Inv(q) | CPath::Plus(q) => q.nullable(),
            CPath::Seq(a, b) => a.nullable() && b.nullable(),
            CPath::Alt(a, b) => a.nullable() || b.nullable(),
        }
    }

    /// Releases the cursors of every edge scan.
    fn close(&mut self, cx: &mut ExecCtx) {
        match self {
            CPath::Pred(e) => {
                for s in [&mut e.fwd, &mut e.bwd, &mut e.by_s, &mut e.by_o] {
                    s.close(cx);
                }
                e.edges = None;
            }
            CPath::Nps(e) => {
                e.fwd.close(cx);
                e.bwd.close(cx);
            }
            CPath::Inv(q) | CPath::Star(q) | CPath::Plus(q) | CPath::Opt(q) => q.close(cx),
            CPath::Seq(a, b) | CPath::Alt(a, b) => {
                a.close(cx);
                b.close(cx);
            }
        }
    }

    /// Set semantics at the top: at most one solution per (start, end).
    fn is_set(&self) -> bool {
        matches!(self, CPath::Star(_) | CPath::Plus(_) | CPath::Opt(_))
    }
}

/// Node scans and buffers shared by the evaluation.
struct Env {
    tmp: Batch,
    /// (g) -> s, p, o: distinct subjects.
    nodes_s: Scan,
    /// (g) -> o, s, p: distinct objects.
    nodes_o: Scan,
    /// (g, s) -> ...: is a node a subject?
    as_s: Scan,
    /// (g, o) -> ...: is a node an object?
    as_o: Scan,
    seed: Vec<TermId>,
    charge: Charge,
    seek_sorted: f64,
    scan_row: f64,
}

impl Env {
    fn new(g: GraphSel) -> Env {
        let all = QuadPat { g, s: PatTerm::Slot(S), p: PatTerm::Slot(P), o: PatTerm::Slot(O) };
        Env {
            tmp: Batch::new(PW),
            nodes_s: Scan::internal(all, IndexOrder::Gspo, PW),
            nodes_o: Scan::internal(all, IndexOrder::Gosp, PW),
            as_s: Scan::internal(all, IndexOrder::Gspo, PW),
            as_o: Scan::internal(all, IndexOrder::Gosp, PW),
            seed: vec![TermId::UNBOUND; PW],
            charge: Charge::default(),
            seek_sorted: 0.0,
            scan_row: 0.0,
        }
    }

    /// Opens `scan` seeded with `slot = v` (Join mode).
    fn open_seeded(seed: &mut [TermId], scan: &mut Scan, cx: &mut ExecCtx, slot: Slot, v: TermId) -> Result<()> {
        seed.fill(TermId::UNBOUND);
        seed[slot as usize] = v;
        scan.open(cx, Some(Seed { row: seed, subst: SlotSet::EMPTY }))
    }

    /// Does `x` occur as a subject or an object in the active graph(s)?
    fn has_node(&mut self, cx: &mut ExecCtx, x: TermId) -> Result<bool> {
        for (scan, slot) in [(&mut self.as_s, S), (&mut self.as_o, O)] {
            Self::open_seeded(&mut self.seed, scan, cx, slot, x)?;
            self.tmp.clear();
            scan.next_limited(cx, &mut self.tmp, 1)?;
            if !self.tmp.is_empty() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Distinct values of `slot` (the first free column of `scan`), sorted.
    fn distinct(cx: &mut ExecCtx, scan: &mut Scan, slot: Slot, tmp: &mut Batch, out: &mut Vec<TermId>) -> Result<()> {
        scan.open(cx, None)?;
        let mut last = None;
        while let Some(v) = scan.next_distinct(cx, slot, tmp, &mut last)? {
            out.push(v);
            if out.len().is_multiple_of(BATCH_ROWS) {
                cx.tick(BATCH_ROWS)?;
            }
        }
        Ok(())
    }

    /// Every node of the active graph(s), sorted.
    fn all_nodes(&mut self, cx: &mut ExecCtx, out: &mut Vec<TermId>) -> Result<()> {
        Self::distinct(cx, &mut self.nodes_s, S, &mut self.tmp, out)?;
        Self::distinct(cx, &mut self.nodes_o, O, &mut self.tmp, out)?;
        sort_dedup(out);
        Ok(())
    }

    fn charge_pairs(&mut self, cx: &ExecCtx, n: usize) -> Result<()> {
        cx.charge(&mut self.charge, n * PAIR_BYTES)
    }
}

fn sort_dedup(v: &mut Vec<TermId>) {
    v.sort_unstable();
    v.dedup();
}

/// Reads every row of an opened scan, calling `f` on each (private layout).
fn drain(cx: &mut ExecCtx, scan: &mut Scan, tmp: &mut Batch, mut f: impl FnMut(&[TermId])) -> Result<()> {
    loop {
        tmp.clear();
        let more = scan.next(cx, tmp)?;
        for i in 0..tmp.len() {
            f(tmp.row(i));
        }
        if !more {
            return Ok(());
        }
    }
}

/// Appends the (start, end) pairs of `path` from `starts` (sorted, deduplicated) in direction
/// `dir`. `target` (top-level `*` / `+` with both endpoints bound) allows an early exit once
/// (start, target) is found.
fn step(
    path: &mut CPath,
    env: &mut Env,
    cx: &mut ExecCtx,
    dir: Dir,
    starts: &[TermId],
    target: Option<TermId>,
    out: &mut Vec<(TermId, TermId)>,
) -> Result<()> {
    if starts.is_empty() {
        return Ok(());
    }
    let before = out.len();
    match path {
        CPath::Pred(e) => {
            let PredScans { fwd, bwd, by_s, by_o, edges } = &mut **e;
            let reg = cx.active_graph().unwrap_or(TermId::UNBOUND);
            if starts.len() > 64 && edges.is_none_or(|(g, _)| g != reg) {
                by_s.open(cx, None)?;
                let n = by_s.fast_count(cx).transpose()?;
                by_s.close(cx);
                *edges = Some((reg, n));
            }
            let full_scan = matches!(edges, Some((g, Some(n))) if *g == reg
                && starts.len() > 64 && starts.len() as f64 * env.seek_sorted > *n as f64 * env.scan_row);
            let (probe, from, to, all, key) = match dir {
                Dir::Fwd => (fwd, S, O, by_s, S),
                Dir::Bwd => (bwd, O, S, by_o, O),
            };
            if full_scan {
                // One scan of all p edges, semi-joined with the start set.
                all.open(cx, None)?;
                drain(cx, all, &mut env.tmp, |r| {
                    let (k, v) = (r[key as usize], r[to as usize]);
                    if starts.binary_search(&k).is_ok() {
                        out.push((k, v));
                    }
                })?;
            } else {
                for &x in starts {
                    Env::open_seeded(&mut env.seed, probe, cx, from, x)?;
                    drain(cx, probe, &mut env.tmp, |r| out.push((x, r[to as usize])))?;
                }
            }
        }
        CPath::Nps(e) => {
            let NpsScans { set, fwd, bwd } = &mut **e;
            let (scan, from, to) = match dir {
                Dir::Fwd => (fwd, S, O),
                Dir::Bwd => (bwd, O, S),
            };
            for &x in starts.iter() {
                Env::open_seeded(&mut env.seed, scan, cx, from, x)?;
                drain(cx, scan, &mut env.tmp, |r| {
                    if set.binary_search(&r[P as usize]).is_err() {
                        out.push((x, r[to as usize]));
                    }
                })?;
            }
        }
        CPath::Inv(q) => step(q, env, cx, dir.flip(), starts, target, out)?,
        CPath::Seq(a, b) => {
            let (first, second) = match dir {
                Dir::Fwd => (a, b),
                Dir::Bwd => (b, a),
            };
            let mut p1 = Vec::new();
            step(first, env, cx, dir, starts, None, &mut p1)?;
            let mut mids: Vec<TermId> = p1.iter().map(|p| p.1).collect();
            sort_dedup(&mut mids);
            let mut p2 = Vec::new();
            step(second, env, cx, dir, &mids, None, &mut p2)?;
            p2.sort_by_key(|p| p.0);
            env.charge_pairs(cx, p1.len() + p2.len())?;
            for (x, m) in p1 {
                let lo = p2.partition_point(|p| p.0 < m);
                for &(_, y) in p2[lo..].iter().take_while(|p| p.0 == m) {
                    out.push((x, y));
                }
            }
        }
        CPath::Alt(a, b) => {
            step(a, env, cx, dir, starts, None, out)?;
            step(b, env, cx, dir, starts, None, out)?;
        }
        CPath::Opt(q) => {
            out.extend(starts.iter().map(|x| (*x, *x)));
            step(q, env, cx, dir, starts, None, out)?;
            out[before..].sort_unstable();
            let mut tail = out.split_off(before);
            tail.dedup();
            out.extend(tail);
        }
        CPath::Star(q) => closure(q, env, cx, dir, starts, true, target, out)?,
        CPath::Plus(q) => closure(q, env, cx, dir, starts, false, target, out)?,
    }
    let n = out.len() - before;
    env.charge_pairs(cx, n)?;
    cx.tick(n)
}

/// Multi-source BFS for `q*` (`star`) / `q+`: set semantics per (start, end).
#[allow(clippy::too_many_arguments)]
fn closure(
    q: &mut CPath,
    env: &mut Env,
    cx: &mut ExecCtx,
    dir: Dir,
    starts: &[TermId],
    star: bool,
    target: Option<TermId>,
    out: &mut Vec<(TermId, TermId)>,
) -> Result<()> {
    let mut visited: FxHashSet<(TermId, TermId)> = FxHashSet::default();
    let mut frontier: Vec<(TermId, TermId)> = starts.iter().map(|x| (*x, *x)).collect();
    if star {
        for &x in starts {
            visited.insert((x, x));
            out.push((x, x));
        }
        if target.is_some_and(|t| starts.contains(&t)) {
            return Ok(());
        }
    }
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    while !frontier.is_empty() {
        nodes.clear();
        nodes.extend(frontier.iter().map(|p| p.1));
        sort_dedup(&mut nodes);
        edges.clear();
        step(q, env, cx, dir, &nodes, None, &mut edges)?;
        edges.sort_unstable_by_key(|p| p.0);
        let mut next = Vec::new();
        for &(x, m) in &frontier {
            let lo = edges.partition_point(|p| p.0 < m);
            for &(_, y) in edges[lo..].iter().take_while(|p| p.0 == m) {
                if visited.insert((x, y)) {
                    out.push((x, y));
                    next.push((x, y));
                    if target == Some(y) {
                        return Ok(());
                    }
                }
            }
        }
        cx.charge(&mut env.charge, next.len() * VISITED_BYTES)?;
        cx.tick(edges.len() + next.len())?;
        frontier = next;
    }
    Ok(())
}

/// Start (or end) candidates of `path`: the nodes a non-empty evaluation can start from.
fn candidates(path: &mut CPath, env: &mut Env, cx: &mut ExecCtx, start: bool, out: &mut Vec<TermId>) -> Result<()> {
    match path {
        CPath::Pred(e) => {
            if start {
                Env::distinct(cx, &mut e.by_s, S, &mut env.tmp, out)?;
            } else {
                Env::distinct(cx, &mut e.by_o, O, &mut env.tmp, out)?;
            }
        }
        CPath::Nps(_) => {
            if start {
                Env::distinct(cx, &mut env.nodes_s, S, &mut env.tmp, out)?;
            } else {
                Env::distinct(cx, &mut env.nodes_o, O, &mut env.tmp, out)?;
            }
        }
        CPath::Inv(q) => candidates(q, env, cx, !start, out)?,
        CPath::Seq(a, b) => candidates(if start { a } else { b }, env, cx, start, out)?,
        CPath::Alt(a, b) => {
            candidates(a, env, cx, start, out)?;
            candidates(b, env, cx, start, out)?;
        }
        CPath::Plus(q) => candidates(q, env, cx, start, out)?,
        CPath::Star(_) | CPath::Opt(_) => env.all_nodes(cx, out)?,
    }
    sort_dedup(out);
    Ok(())
}

/// An endpoint at the start of an evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum End {
    /// A constant, or a Substitute-mode seed: matches itself even when absent.
    Const(TermId),
    /// A Join-mode seed (or the graph slot of a looped graph): must be a node of the graph.
    Join(TermId),
    Free(Slot),
}

impl End {
    fn value(self) -> Option<TermId> {
        match self {
            End::Const(v) | End::Join(v) => Some(v),
            End::Free(_) => None,
        }
    }
}

/// Pending evaluation of the current graph when both endpoints are free.
struct Chunks {
    cands: Vec<TermId>,
    pos: usize,
    /// `?x p ?x`: keep the pairs that end where they start.
    same: bool,
}

pub(crate) struct PathOp {
    g: GraphSel,
    s: PatTerm,
    o: PatTerm,
    path: CPath,
    nullable: bool,
    env: Env,
    seed: SeedBuf,
    /// Output image: the seed plus the graph slot of the current graph.
    base: Vec<TermId>,
    /// Graphs still to evaluate (`GraphSel::Slot`), or one placeholder.
    graphs: Vec<TermId>,
    gi: usize,
    /// Graph of the pairs being emitted (`GraphSel::Slot`).
    cur_graph: Option<TermId>,
    chunks: Option<Chunks>,
    pairs: Vec<(TermId, TermId)>,
    pp: usize,
    /// Slots the pairs bind: the start and the end of each pair.
    bind_start: Option<Slot>,
    bind_end: Option<Slot>,
    /// Memory of the start candidates of the current graph (`env.charge` holds the
    /// temporaries of one evaluation step).
    cand_charge: Charge,
}

impl PathOp {
    pub fn new(g: GraphSel, s: PatTerm, path: &PathPlan, o: PatTerm, width: usize) -> Self {
        // A graph variable is evaluated graph by graph through the register.
        let inner = match g {
            GraphSel::Slot(_) => GraphSel::Active,
            other => other,
        };
        let path = CPath::new(path, inner);
        let nullable = path.nullable();
        PathOp {
            g,
            s,
            o,
            path,
            nullable,
            env: Env::new(inner),
            seed: SeedBuf::new(width),
            base: vec![TermId::UNBOUND; width],
            graphs: Vec::new(),
            gi: 0,
            cur_graph: None,
            chunks: None,
            pairs: Vec::new(),
            pp: 0,
            bind_start: None,
            bind_end: None,
            cand_charge: Charge::default(),
        }
    }

    fn end_of(&self, t: PatTerm, graph: Option<(Slot, TermId)>) -> End {
        match t {
            PatTerm::Const(v) => End::Const(v),
            PatTerm::Slot(s) => match self.seed.value(s) {
                Some(v) if self.seed.is_subst(s) => End::Const(v),
                Some(v) => End::Join(v),
                None => match graph {
                    Some((gs, g)) if gs == s => End::Join(g),
                    _ => End::Free(s),
                },
            },
        }
    }

    /// Runs `f` with the current graph on the register (for `GraphSel::Slot`).
    fn in_graph<R>(&mut self, cx: &mut ExecCtx, f: impl FnOnce(&mut Self, &mut ExecCtx) -> Result<R>) -> Result<R> {
        let pushed = self.cur_graph.inspect(|g| cx.graphs.push(*g)).is_some();
        let r = f(self, cx);
        if pushed {
            cx.graphs.pop();
        }
        r
    }

    /// Prepares the evaluation of the next graph; false when there is none.
    fn start_graph(&mut self, cx: &mut ExecCtx) -> Result<bool> {
        let Some(&g) = self.graphs.get(self.gi) else { return Ok(false) };
        self.gi += 1;
        self.base.copy_from_slice(self.seed.base());
        let graph = match self.g {
            GraphSel::Slot(gs) => {
                self.base[gs as usize] = g;
                self.cur_graph = Some(g);
                Some((gs, g))
            }
            _ => None,
        };
        let (s, o) = (self.end_of(self.s, graph), self.end_of(self.o, graph));
        cx.sh.release(&mut self.env.charge);
        cx.sh.release(&mut self.cand_charge);
        self.pairs.clear();
        self.pp = 0;
        self.chunks = None;
        self.in_graph(cx, |me, cx| me.evaluate(cx, s, o))?;
        Ok(true)
    }

    /// Evaluates the path for fixed endpoints (pairs, or start candidates when both are free).
    fn evaluate(&mut self, cx: &mut ExecCtx, s: End, o: End) -> Result<()> {
        let nullable = self.nullable;
        let set = self.path.is_set();
        let env = &mut self.env;
        let path = &mut self.path;
        // A Join-mode seeded endpoint of a zero-length-capable path must be a graph node.
        let absent = |env: &mut Env, cx: &mut ExecCtx, e: End| -> Result<bool> {
            Ok(match e {
                End::Join(v) if nullable => !env.has_node(cx, v)?,
                _ => false,
            })
        };
        match (s, o) {
            (End::Free(x), End::Free(y)) => {
                let mut cands = Vec::new();
                candidates(path, env, cx, true, &mut cands)?;
                cx.charge(&mut self.cand_charge, cands.len() * 8)?;
                self.bind_start = Some(x);
                self.bind_end = (x != y).then_some(y);
                self.chunks = Some(Chunks { cands, pos: 0, same: x == y });
            }
            (known, End::Free(y)) => {
                if !absent(env, cx, known)? {
                    let a = known.value().expect("bound endpoint");
                    step(path, env, cx, Dir::Fwd, &[a], None, &mut self.pairs)?;
                }
                self.bind_start = None;
                self.bind_end = Some(y);
            }
            (End::Free(x), known) => {
                if !absent(env, cx, known)? {
                    let b = known.value().expect("bound endpoint");
                    step(path, env, cx, Dir::Bwd, &[b], None, &mut self.pairs)?;
                }
                self.bind_start = None;
                self.bind_end = Some(x);
            }
            (a, b) => {
                // Run from a constant side so that its zero-length match needs no graph node.
                let (from, to, dir) = match (a, b) {
                    (End::Const(_), _) => (a, b, Dir::Fwd),
                    (_, End::Const(_)) => (b, a, Dir::Bwd),
                    _ => (a, b, Dir::Fwd),
                };
                if !absent(env, cx, from)? {
                    let (x, t) = (from.value().expect("bound"), to.value().expect("bound"));
                    let target = set.then_some(t);
                    step(path, env, cx, dir, &[x], target, &mut self.pairs)?;
                    self.pairs.retain(|p| p.1 == t);
                    if set {
                        self.pairs.truncate(1);
                    }
                }
                self.bind_start = None;
                self.bind_end = None;
            }
        }
        Ok(())
    }

    /// Evaluates the next chunk of start candidates; false when they are used up.
    fn next_chunk(&mut self, cx: &mut ExecCtx) -> Result<bool> {
        let Some(ch) = self.chunks.as_mut() else { return Ok(false) };
        if ch.pos >= ch.cands.len() {
            self.chunks = None;
            return Ok(false);
        }
        let end = (ch.pos + CHUNK_STARTS).min(ch.cands.len());
        let starts: Vec<TermId> = ch.cands[ch.pos..end].to_vec();
        ch.pos = end;
        let same = ch.same;
        self.pairs.clear();
        self.pp = 0;
        cx.sh.release(&mut self.env.charge);
        self.in_graph(cx, |me, cx| step(&mut me.path, &mut me.env, cx, Dir::Fwd, &starts, None, &mut me.pairs))?;
        if same {
            self.pairs.retain(|p| p.0 == p.1);
        }
        Ok(true)
    }
}

impl Operator for PathOp {
    fn open(&mut self, cx: &mut ExecCtx, seed: Option<Seed<'_>>) -> Result<()> {
        self.seed.set(seed);
        let costs = cx.store().costs();
        self.env.seek_sorted = costs.seek_sorted;
        self.env.scan_row = costs.scan_row;
        self.graphs.clear();
        self.gi = 0;
        self.cur_graph = None;
        self.chunks = None;
        self.pairs.clear();
        self.pp = 0;
        match self.g {
            GraphSel::Slot(gs) => match self.seed.value(gs) {
                Some(v) => {
                    if cx.sh.is_named_graph(v) {
                        self.graphs.push(v);
                    }
                }
                None => self.graphs.extend_from_slice(&cx.sh.named_graphs),
            },
            GraphSel::Active if cx.active_graph().is_none() => {
                return Err(Error::internal("GraphSel::Active outside of a GraphLoop"));
            }
            // One evaluation; the placeholder is never read.
            _ => self.graphs.push(TermId::UNBOUND),
        }
        Ok(())
    }

    fn next(&mut self, cx: &mut ExecCtx, out: &mut Batch) -> Result<bool> {
        loop {
            while self.pp < self.pairs.len() {
                if out.is_full() {
                    return Ok(true);
                }
                let (a, b) = self.pairs[self.pp];
                self.pp += 1;
                out.push_row(&self.base);
                let i = out.len() - 1;
                let row = out.row_mut(i);
                if let Some(s) = self.bind_start {
                    row[s as usize] = a;
                }
                if let Some(s) = self.bind_end {
                    row[s as usize] = b;
                }
            }
            if self.next_chunk(cx)? {
                continue;
            }
            if !self.start_graph(cx)? {
                cx.sh.release(&mut self.env.charge);
                cx.sh.release(&mut self.cand_charge);
                return Ok(false);
            }
        }
    }

    fn close(&mut self, cx: &mut ExecCtx) {
        self.pairs = Vec::new();
        self.chunks = None;
        self.gi = self.graphs.len();
        self.path.close(cx);
        for s in [&mut self.env.nodes_s, &mut self.env.nodes_o, &mut self.env.as_s, &mut self.env.as_o] {
            s.close(cx);
        }
        cx.release(&mut self.env.charge);
        cx.release(&mut self.cand_charge);
    }
}
