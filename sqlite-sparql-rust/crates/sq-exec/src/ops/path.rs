//! Property paths (§5.5): `Op::Path { g, s, path, o }`.
//!
//! A path is evaluated by `step(path, dir, starts, ends) -> (start, end) pairs` over sorted,
//! deduplicated start sets, following SPARQL 1.1 §18.4 literally:
//! * `Pred(p)`: a probe per start (forward GSPO `(g,s,p)`, backward GPOS `(g,p,o)`), or one
//!   scan of all p edges semi-joined with the starts when that is cheaper
//!   (`|starts| * seek_sorted > |edges(p)| * scan_row`), decided with a count of the p edges
//!   capped at the break-even point and remembered per graph for the whole execution;
//! * `Nps(set)` scans `(g,s)` (backward `(g,o)`), drops predicates in the set and yields a SET
//!   of pairs per start ("there exists a triple x p y with p not in the set");
//! * `Inv` flips the direction; `Alt` is a bag union (`!(a|^b)` = `Alt(Nps, Inv(Nps))` counts a
//!   self-loop found both ways twice);
//! * `Seq` is a join on a fresh middle variable, duplicates kept (see below);
//! * `*`, `+`: multi-source BFS by level with a visited set of (start, node) pairs (ALP), so it
//!   terminates on cycles and has set semantics per (start, end); `?` is a set too.
//!
//! Zero-length rules. A constant (or Substitute-mode seeded) endpoint matches itself even when
//! it is absent from the graph; any other value bound by a zero-length step must be a node of
//! the active graph (it occurs as a subject or object there), except that it also matches a
//! constant at the other end:
//! * a Join-mode seeded endpoint must satisfy `has_node`, so a seeded evaluation equals the
//!   free one filtered by the seed;
//! * both endpoints free: every node of the active graph (loose index scans of the distinct
//!   subjects on GSPO and objects on GOSP); `?x p* ?x` binds every node, `?x p+ ?x` needs a
//!   cycle;
//! * the middle of a sequence is such a variable: expanding the second operand from a middle
//!   `m`, a zero-length step matches iff `has_node(m)` when its other end is free, and iff
//!   that end is a constant equal to `m` otherwise. So `:zz (:p?/:q?) ?y` is empty when `:zz`
//!   is not a node, while `:zz :p?/:q? :zz` holds (each operand binds the middle to `:zz` from
//!   its own constant endpoint). Steps therefore carry the binding of their starts ([`Kind`])
//!   and the constant far end, if any ([`Ends`]).
//!
//! Endpoints: a bound subject evaluates forward, a bound object backward over the inverse;
//! when both are bound the path runs from a constant side (forward by default, with early exit
//! for `*` / `+`) and keeps the pairs ending on the other value; with both free, one BFS per
//! start candidate (the possible first-step subjects; every node of the graph when the path
//! can be zero-length).
//!
//! Paths never cross graphs: the edge scans read the default graph (merged when it has several
//! graphs), the GraphLoop register (`Active`), a constant graph, or — for `GraphSel::Slot` —
//! each named graph in turn, pushed on the register by this operator.
use rustc_hash::{FxHashMap, FxHashSet};
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
/// Steps from at most this many starts always probe.
const PROBE_ALWAYS: usize = 64;
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
    /// What the capped counts found out about |edges(p)|, per register value (the graph of
    /// `GraphSel::Active`; UNBOUND otherwise). The store does not change during an execution,
    /// so this survives `reset` and `close` (EXISTS patterns are re-opened per outer row).
    edges: FxHashMap<TermId, EdgeCount>,
}

/// What is known about the number of edges of a predicate in one graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EdgeCount {
    /// The scan cannot be counted without reading it (a merged default graph): always probe.
    Unknown,
    Exact(u64),
    /// More than this many edges.
    Over(u64),
}

impl PredScans {
    /// Is one scan of every p edge, semi-joined with `starts` start nodes, cheaper than a
    /// probe per start (`|edges(p)| * scan_row < |starts| * seek_sorted`, §5.5)? Only the
    /// break-even point matters, so the edges are counted with that cap (§7.4: runtime counts
    /// are bounded), and a count is only repeated for a larger cap than a previous one.
    fn scan_all(&mut self, cx: &mut ExecCtx, env: &Env, starts: usize) -> Result<bool> {
        if starts <= PROBE_ALWAYS {
            return Ok(false);
        }
        // The largest edge count for which the scan wins: n < starts * seek_sorted / scan_row.
        let break_even = starts as f64 * env.seek_sorted / env.scan_row;
        let max = break_even.ceil() - 1.0;
        if max.is_nan() || max < 0.0 {
            return Ok(false);
        }
        let max = if max >= u64::MAX as f64 { u64::MAX - 1 } else { max as u64 };
        let reg = cx.active_graph().unwrap_or(TermId::UNBOUND);
        match self.edges.get(&reg) {
            Some(EdgeCount::Unknown) => return Ok(false),
            Some(EdgeCount::Exact(n)) => return Ok(*n <= max),
            Some(EdgeCount::Over(n)) if *n >= max => return Ok(false),
            _ => {}
        }
        // A capped count gives min(|edges(p)|, max + 1).
        self.by_s.open(cx, None)?;
        let known = match self.by_s.fast_count(cx, Some(max)).transpose()? {
            None => EdgeCount::Unknown,
            Some(n) if n <= max => EdgeCount::Exact(n),
            Some(_) => EdgeCount::Over(max),
        };
        self.edges.insert(reg, known);
        Ok(matches!(known, EdgeCount::Exact(_)))
    }
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
                    edges: FxHashMap::default(),
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

    /// Number of zero-length solutions binding a value that is not a graph node to itself,
    /// with one endpoint that constant and the other a free variable (either way round: the
    /// count is the same in both directions). A sequence has none, since its middle would have
    /// to be a graph node; `|` adds (a bag union); `?`, `*`, `+` are sets.
    fn absent_zero(&self) -> usize {
        match self {
            CPath::Pred(_) | CPath::Nps(_) | CPath::Seq(..) => 0,
            CPath::Star(_) | CPath::Opt(_) => 1,
            CPath::Inv(q) => q.absent_zero(),
            CPath::Plus(q) => q.absent_zero().min(1),
            CPath::Alt(a, b) => a.absent_zero() + b.absent_zero(),
        }
    }

    /// Releases the cursors of every edge scan.
    fn close(&mut self, cx: &mut ExecCtx) {
        match self {
            CPath::Pred(e) => {
                for s in [&mut e.fwd, &mut e.bwd, &mut e.by_s, &mut e.by_o] {
                    s.close(cx);
                }
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

/// Sorts `v[from..]` and removes its duplicates in place.
fn dedup_tail(v: &mut Vec<(TermId, TermId)>, from: usize) {
    v[from..].sort_unstable();
    let mut w = from;
    for r in from..v.len() {
        if w == from || v[r] != v[w - 1] {
            v[w] = v[r];
            w += 1;
        }
    }
    v.truncate(w);
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

/// How the starts of a step are bound, for its zero-length matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// Constants (or substituted values): a zero-length step matches a start even when it is
    /// not a node of the active graph.
    Const,
    /// Nodes of the active graph: free endpoints, checked Join-mode seeds, sequence middles
    /// and BFS frontiers.
    Nodes,
}

/// The binding of the two ends of a step.
#[derive(Clone, Copy, Debug)]
struct Ends {
    starts: Kind,
    /// The far end, when it is a constant: a zero-length step of a sequence's last operand from
    /// a middle that is not a graph node still matches it. None: the far end is a variable.
    far: Option<TermId>,
    /// Early exit once (start, target) is found (top-level `*` / `+` with both endpoints bound).
    target: Option<TermId>,
}

impl Ends {
    /// Starts that are graph nodes, far end free.
    const NODES: Ends = Ends { starts: Kind::Nodes, far: None, target: None };

    fn far_free(starts: Kind) -> Ends {
        Ends { starts, far: None, target: None }
    }
}

/// Appends the (start, end) pairs of `path` from `starts` (sorted, deduplicated) in direction
/// `dir`, with the starts and the far end bound as `ends` says. The result is exact when the
/// far end is free; with a constant far end `y` it may also hold pairs ending elsewhere than
/// `y`, which the caller drops (those ending on `y` are exact, multiplicities included).
fn step(
    path: &mut CPath,
    env: &mut Env,
    cx: &mut ExecCtx,
    dir: Dir,
    starts: &[TermId],
    ends: Ends,
    out: &mut Vec<(TermId, TermId)>,
) -> Result<()> {
    if starts.is_empty() {
        return Ok(());
    }
    let before = out.len();
    let inner = Ends { target: None, ..ends };
    match path {
        CPath::Pred(e) => {
            let full_scan = e.scan_all(cx, env, starts.len())?;
            let PredScans { fwd, bwd, by_s, by_o, .. } = &mut **e;
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
            for &x in starts {
                let first = out.len();
                Env::open_seeded(&mut env.seed, scan, cx, from, x)?;
                drain(cx, scan, &mut env.tmp, |r| {
                    if set.binary_search(&r[P as usize]).is_err() {
                        out.push((x, r[to as usize]));
                    }
                })?;
                // A set of pairs: two predicates outside the set linking x and y give one pair.
                dedup_tail(out, first);
            }
        }
        CPath::Inv(q) => step(q, env, cx, dir.flip(), starts, ends, out)?,
        CPath::Seq(a, b) => {
            let (first, second) = match dir {
                Dir::Fwd => (a, b),
                Dir::Bwd => (b, a),
            };
            sequence(first, second, env, cx, dir, starts, inner, out)?;
        }
        CPath::Alt(a, b) => {
            step(a, env, cx, dir, starts, inner, out)?;
            step(b, env, cx, dir, starts, inner, out)?;
        }
        CPath::Opt(q) => {
            // Every start is a constant or a node: its zero-length match holds.
            out.extend(starts.iter().map(|x| (*x, *x)));
            step(q, env, cx, dir, starts, inner, out)?;
            dedup_tail(out, before);
        }
        CPath::Star(q) => closure(q, env, cx, dir, starts, ends, true, out)?,
        CPath::Plus(q) => closure(q, env, cx, dir, starts, ends, false, out)?,
    }
    let n = out.len() - before;
    env.charge_pairs(cx, n)?;
    cx.tick(n)
}

/// `first/second` (§18.4): `Join(eval(X first ?V), eval(?V second Y))` on a fresh middle
/// variable `?V`, projected, duplicates kept.
///
/// `?V` is a variable, so `second` runs from the middles as graph nodes: a middle reached
/// over an edge is one, a middle reached by a zero-length `first` from a constant start is one
/// iff that start is. A middle that is not a node only has the zero-length matches of
/// `second` onto a constant far end equal to it ([`CPath::absent_zero`]).
#[allow(clippy::too_many_arguments)]
fn sequence(
    first: &mut CPath,
    second: &mut CPath,
    env: &mut Env,
    cx: &mut ExecCtx,
    dir: Dir,
    starts: &[TermId],
    ends: Ends,
    out: &mut Vec<(TermId, TermId)>,
) -> Result<()> {
    let mut p1 = Vec::new();
    step(first, env, cx, dir, starts, Ends::far_free(ends.starts), &mut p1)?;
    let mut mids: Vec<TermId> = p1.iter().map(|p| p.1).collect();
    sort_dedup(&mut mids);
    // Constant starts that are middles and not graph nodes (sorted, as the starts are).
    let mut absent = Vec::new();
    if ends.starts == Kind::Const {
        for &x in starts {
            if mids.binary_search(&x).is_ok() && !env.has_node(cx, x)? {
                absent.push(x);
            }
        }
        mids.retain(|m| absent.binary_search(m).is_err());
    }
    let mut p2 = Vec::new();
    step(second, env, cx, dir, &mids, Ends { starts: Kind::Nodes, ..ends }, &mut p2)?;
    if let Some(y) = ends.far
        && absent.binary_search(&y).is_ok()
    {
        p2.extend(std::iter::repeat_n((y, y), second.absent_zero()));
    }
    p2.sort_unstable_by_key(|p| p.0);
    env.charge_pairs(cx, p1.len() + p2.len())?;
    for (x, m) in p1 {
        let lo = p2.partition_point(|p| p.0 < m);
        for &(_, y) in p2[lo..].iter().take_while(|p| p.0 == m) {
            out.push((x, y));
        }
    }
    Ok(())
}

/// Multi-source BFS for `q*` (`star`) / `q+` (ALP): set semantics per (start, end).
///
/// The body runs from each frontier node with a free far end. The first level keeps the
/// binding of the starts; later frontiers are graph nodes, except a constant start reached
/// again by a zero-length step, from which nothing new can be reached anyway.
#[allow(clippy::too_many_arguments)]
fn closure(
    q: &mut CPath,
    env: &mut Env,
    cx: &mut ExecCtx,
    dir: Dir,
    starts: &[TermId],
    ends: Ends,
    star: bool,
    out: &mut Vec<(TermId, TermId)>,
) -> Result<()> {
    let target = ends.target;
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
    let mut body = Ends::far_free(ends.starts);
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    while !frontier.is_empty() {
        nodes.clear();
        nodes.extend(frontier.iter().map(|p| p.1));
        sort_dedup(&mut nodes);
        edges.clear();
        step(q, env, cx, dir, &nodes, body, &mut edges)?;
        body = Ends::NODES;
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
        // How a bound endpoint binds the start of the evaluation; None: nothing can match. A
        // Join-mode seed of a zero-length-capable path must be a graph node. One of a path that
        // is not may be taken as a constant: every match then needs an edge from it.
        let kind = |env: &mut Env, cx: &mut ExecCtx, e: End| -> Result<Option<Kind>> {
            Ok(match e {
                End::Join(v) if nullable => env.has_node(cx, v)?.then_some(Kind::Nodes),
                _ => Some(Kind::Const),
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
                if let Some(k) = kind(env, cx, known)? {
                    let a = known.value().expect("bound endpoint");
                    step(path, env, cx, Dir::Fwd, &[a], Ends::far_free(k), &mut self.pairs)?;
                }
                self.bind_start = None;
                self.bind_end = Some(y);
            }
            (End::Free(x), known) => {
                if let Some(k) = kind(env, cx, known)? {
                    let b = known.value().expect("bound endpoint");
                    step(path, env, cx, Dir::Bwd, &[b], Ends::far_free(k), &mut self.pairs)?;
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
                if let Some(k) = kind(env, cx, from)? {
                    let (x, t) = (from.value().expect("bound"), to.value().expect("bound"));
                    // A Join-mode far end is a variable joined afterwards: the pairs ending on it.
                    let far = matches!(to, End::Const(_)).then_some(t);
                    let ends = Ends { starts: k, far, target: set.then_some(t) };
                    step(path, env, cx, dir, &[x], ends, &mut self.pairs)?;
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

    /// Ends the evaluation: drops the start candidates and releases the memory charges.
    fn release(&mut self, cx: &ExecCtx) {
        self.chunks = None;
        self.gi = self.graphs.len();
        cx.release(&mut self.env.charge);
        cx.release(&mut self.cand_charge);
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
        self.in_graph(cx, |me, cx| step(&mut me.path, &mut me.env, cx, Dir::Fwd, &starts, Ends::NODES, &mut me.pairs))?;
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

    fn reset(&mut self, cx: &mut ExecCtx) {
        // Keeps a batch worth of pair capacity for the next evaluation.
        self.pairs.clear();
        self.pairs.shrink_to(BATCH_ROWS);
        self.release(cx);
    }

    fn close(&mut self, cx: &mut ExecCtx) {
        self.pairs = Vec::new();
        self.release(cx);
        self.path.close(cx);
        for s in [&mut self.env.nodes_s, &mut self.env.nodes_o, &mut self.env.as_s, &mut self.env.as_o] {
            s.close(cx);
        }
    }
}
