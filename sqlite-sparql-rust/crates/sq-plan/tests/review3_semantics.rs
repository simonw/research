//! Review round 3: semantic differential check of the physical planner.
//!
//! A naive evaluator of the LOWERED logical IR (SPARQL 1.1 semantics: bag joins, OPTIONAL with
//! its filter on merged rows, MINUS with the domain test, GRAPH per named graph, EXISTS by
//! substitution with Substitute-mode MINUS leaving substituted slots out of the domain test)
//! is compared with an evaluator of the PHYSICAL plan that implements each `sq_core::plan::Op`
//! exactly as documented and the planner's Join-mode seed contract literally (leaves merge the
//! full seed; Project / Group keep it on their output slots; Slice, TopK, Count, GraphLoop and
//! GraphNames are opened unseeded; a BindJoin re-opens its right side with its seed list only).
//! The physical evaluator also reports contract violations it observes (an unbound key or seed
//! slot, a slot bound differently on both join sides that is neither key nor check, a false
//! `sorted_by` claim, using the cursor output order of each Scan's index). Both evaluators
//! share expression and path evaluation, so any difference is a rewrite / planning bug.
//!
//! Random queries cover BGPs with constants and blank nodes, `+ * ? ^` paths, OPTIONAL (with
//! filters, EXISTS and nested OPTIONALs), UNION, MINUS, nested filtered groups, sub-selects
//! (projection, DISTINCT, ORDER BY + LIMIT, GROUP BY / HAVING / COUNT), GRAPH ?g / GRAPH <c>,
//! VALUES with UNDEF, BIND, sameTerm / = seeds, [NOT] EXISTS (incl. MINUS inside), dataset
//! clauses, union default graph, and top-level DISTINCT / ORDER BY + LIMIT / ASK / COUNT, each
//! under four join policies. `GRAPH <c>` also names graphs outside the dataset (`:g1` under
//! `FROM NAMED :g0`, `:absent` everywhere), which resolve to Empty and fold into their ancestors.

mod common;

use std::cell::RefCell;

use common::*;
use sq_core::batch::Slot;
use sq_core::ir::{AggFunc, Expr, GraphSel, Logical, LoweredQuery, OrderKey, PatTerm, PathPlan, QueryForm};
use sq_core::plan::{JoinKind, JoinPolicy, Op, Plan, QueryOptions, QueryPlan};
use sq_core::store::QuadStore;
use sq_core::TermId;
use sq_memstore::MemStore;

pub type Row = Vec<TermId>;

pub struct Data {
    pub quads: Vec<[TermId; 4]>,
    pub named: Vec<TermId>,
    pub default: sq_core::ir::DefaultGraph,
}

impl Data {
    pub fn new(st: &MemStore, ds: &sq_core::ir::DatasetSpec) -> Data {
        let named = match &ds.named {
            sq_core::ir::NamedGraphs::All => st.named_graphs().unwrap(),
            sq_core::ir::NamedGraphs::Only(l) => l.to_vec(),
        };
        let mut named = named;
        named.sort();
        named.dedup();
        Data { quads: st.quads(), named, default: ds.default.clone() }
    }
    /// Quads of graph `g`; g = 0 is the dataset's default graph (RDF merge, a set).
    pub fn triples(&self, g: TermId) -> Vec<[TermId; 4]> {
        use sq_core::ir::DefaultGraph;
        if g != TermId::DEFAULT_GRAPH {
            return self.quads.iter().filter(|q| q[0] == g).copied().collect();
        }
        let mut v: Vec<[TermId; 4]> = match &self.default {
            DefaultGraph::StoreDefault => return self.quads.iter().filter(|q| q[0] == g).copied().collect(),
            DefaultGraph::UnionAll => self.quads.iter().map(|q| [g, q[1], q[2], q[3]]).collect(),
            DefaultGraph::Graphs(list) => {
                self.quads.iter().filter(|q| list.contains(&q[0])).map(|q| [g, q[1], q[2], q[3]]).collect()
            }
        };
        v.sort();
        v.dedup();
        v
    }
    fn nodes(&self, g: TermId) -> Vec<TermId> {
        let mut v: Vec<TermId> = self.triples(g).iter().flat_map(|q| [q[1], q[3]]).collect();
        v.sort();
        v.dedup();
        v
    }
    fn edges(&self, g: TermId, p: TermId) -> Vec<(TermId, TermId)> {
        self.triples(g).iter().filter(|q| q[2] == p).map(|q| (q[1], q[3])).collect()
    }
}

// ------------------------------------------------------------------ shared helpers

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum V {
    Err,
    Term(TermId),
    Bool(bool),
}

fn get(row: &Row, subst: &Row, s: Slot) -> TermId {
    let v = row[s as usize];
    if v.is_bound() { v } else { subst[s as usize] }
}

fn is_int(t: TermId) -> bool {
    t.tag() == sq_core::id::Tag::Int
}

pub fn ebv(v: V) -> Option<bool> {
    match v {
        V::Bool(b) => Some(b),
        V::Term(t) if is_int(t) => Some(t != TermId::from_int(0).unwrap()),
        _ => None,
    }
}

pub fn eval_expr(e: &Expr, row: &Row, subst: &Row, ex: &mut dyn FnMut(sq_core::ir::ExistsId, &Row) -> bool) -> V {
    let sub = |x: &Expr, ex: &mut dyn FnMut(sq_core::ir::ExistsId, &Row) -> bool| eval_expr(x, row, subst, ex);
    match e {
        Expr::Const(c) => V::Term(*c),
        Expr::Slot(s) => {
            let v = get(row, subst, *s);
            if v.is_bound() { V::Term(v) } else { V::Err }
        }
        Expr::Bound(s) => V::Bool(get(row, subst, *s).is_bound()),
        Expr::Not(a) => match ebv(sub(a, ex)) {
            Some(b) => V::Bool(!b),
            None => V::Err,
        },
        Expr::And(a, b) => {
            let (x, y) = (ebv(sub(a, ex)), ebv(sub(b, ex)));
            match (x, y) {
                (Some(false), _) | (_, Some(false)) => V::Bool(false),
                (Some(true), Some(true)) => V::Bool(true),
                _ => V::Err,
            }
        }
        Expr::Or(a, b) => {
            let (x, y) = (ebv(sub(a, ex)), ebv(sub(b, ex)));
            match (x, y) {
                (Some(true), _) | (_, Some(true)) => V::Bool(true),
                (Some(false), Some(false)) => V::Bool(false),
                _ => V::Err,
            }
        }
        Expr::Equal(a, b) => match (sub(a, ex), sub(b, ex)) {
            (V::Term(x), V::Term(y)) => V::Bool(x == y),
            _ => V::Err,
        },
        Expr::SameTerm(a, b) => match (sub(a, ex), sub(b, ex)) {
            (V::Term(x), V::Term(y)) => V::Bool(x == y),
            _ => V::Err,
        },
        Expr::Exists(id) => {
            let mut full = row.clone();
            for (i, v) in full.iter_mut().enumerate() {
                if !v.is_bound() {
                    *v = subst[i];
                }
            }
            V::Bool(ex(*id, &full))
        }
        other => panic!("unsupported expression in differential test: {other:?}"),
    }
}

fn key_of(e: &Expr, row: &Row, subst: &Row, ex: &mut dyn FnMut(sq_core::ir::ExistsId, &Row) -> bool) -> (u8, i64) {
    match eval_expr(e, row, subst, ex) {
        V::Err => (0, 0),
        V::Bool(b) => (1, b as i64),
        V::Term(t) => (2, t.raw()),
    }
}

fn sort_rows(rows: &mut Vec<Row>, keys: &[OrderKey], subst: &Row, ex: &mut dyn FnMut(sq_core::ir::ExistsId, &Row) -> bool) {
    let mut keyed: Vec<(Vec<(u8, i64)>, Row)> = rows
        .drain(..)
        .map(|r| (keys.iter().map(|k| key_of(&k.expr, &r, subst, ex)).collect(), r))
        .collect();
    keyed.sort_by(|a, b| {
        for (i, k) in keys.iter().enumerate() {
            let o = a.0[i].cmp(&b.0[i]);
            let o = if k.desc { o.reverse() } else { o };
            if o != std::cmp::Ordering::Equal {
                return o;
            }
        }
        std::cmp::Ordering::Equal
    });
    rows.extend(keyed.into_iter().map(|(_, r)| r));
}

/// Merge `b` into `a` when compatible on every slot.
fn merge(a: &Row, b: &Row) -> Option<Row> {
    let mut out = a.clone();
    for (i, v) in b.iter().enumerate() {
        if v.is_bound() {
            if out[i].is_bound() && out[i] != *v {
                return None;
            }
            out[i] = *v;
        }
    }
    Some(out)
}

/// Path pairs in graph `g`; endpoints Some(x) are constants (zero-length matches x itself).
fn path_pairs(d: &Data, g: TermId, p: &PathPlan, s: Option<TermId>, o: Option<TermId>) -> Vec<(TermId, TermId)> {
    let all = |p: &PathPlan| -> Vec<(TermId, TermId)> { path_all(d, g, p) };
    let mut v = all(p);
    // zero-length with constants
    if zero(p) {
        match (s, o) {
            (Some(x), Some(y)) if x == y => v.push((x, x)),
            (Some(x), None) => v.push((x, x)),
            (None, Some(y)) => v.push((y, y)),
            _ => {}
        }
        v.sort();
        v.dedup();
    }
    v.into_iter().filter(|(a, b)| s.is_none_or(|x| x == *a) && o.is_none_or(|y| y == *b)).collect()
}

fn zero(p: &PathPlan) -> bool {
    matches!(p, PathPlan::ZeroOrMore(_) | PathPlan::ZeroOrOne(_))
}

fn path_all(d: &Data, g: TermId, p: &PathPlan) -> Vec<(TermId, TermId)> {
    match p {
        PathPlan::Pred(x) => d.edges(g, *x),
        PathPlan::Inv(x) => path_all(d, g, x).into_iter().map(|(a, b)| (b, a)).collect(),
        PathPlan::OneOrMore(x) | PathPlan::ZeroOrMore(x) => {
            let base = path_all(d, g, x);
            let mut set: Vec<(TermId, TermId)> = base.clone();
            set.sort();
            set.dedup();
            loop {
                let mut next = set.clone();
                for (a, b) in &set {
                    for (c, e) in &base {
                        if b == c {
                            next.push((*a, *e));
                        }
                    }
                }
                next.sort();
                next.dedup();
                if next.len() == set.len() {
                    break;
                }
                set = next;
            }
            if matches!(p, PathPlan::ZeroOrMore(_)) {
                for n in d.nodes(g) {
                    set.push((n, n));
                }
                set.sort();
                set.dedup();
            }
            set
        }
        PathPlan::ZeroOrOne(x) => {
            let mut set = path_all(d, g, x);
            for n in d.nodes(g) {
                set.push((n, n));
            }
            set.sort();
            set.dedup();
            set
        }
        other => panic!("unsupported path {other:?}"),
    }
}

// ------------------------------------------------------------------ reference (logical IR)

pub struct Ref<'a> {
    pub d: &'a Data,
    pub q: &'a LoweredQuery,
}

impl Ref<'_> {
    fn exists(&self, id: sq_core::ir::ExistsId, row: &Row, active: Option<TermId>) -> bool {
        !self.eval(&self.q.exists[id.0 as usize], active, row).is_empty()
    }

    fn graph_of(&self, g: GraphSel, active: Option<TermId>) -> TermId {
        match g {
            GraphSel::Default => TermId::DEFAULT_GRAPH,
            GraphSel::Active => active.expect("active graph"),
            GraphSel::Const(c) => c,
            GraphSel::Slot(_) => panic!("Slot graph in lowered IR"),
        }
    }

    pub fn eval(&self, l: &Logical, active: Option<TermId>, subst: &Row) -> Vec<Row> {
        let w = subst.len();
        let mut ex = |id, r: &Row| self.exists(id, r, active);
        match l {
            Logical::Unit => vec![subst.clone()],
            Logical::Empty => vec![],
            Logical::Service { .. } => vec![subst.clone()],
            Logical::Bgp(pats) => {
                let mut rows = vec![subst.clone()];
                for p in pats {
                    let g = self.graph_of(p.g, active);
                    let mut next = Vec::new();
                    for r in &rows {
                        for q in self.d.triples(g).iter() {
                            let mut nr = r.clone();
                            let mut ok = true;
                            for (t, v) in [(p.s, q[1]), (p.p, q[2]), (p.o, q[3])] {
                                match t {
                                    PatTerm::Const(c) => ok &= c == v,
                                    PatTerm::Slot(s) => {
                                        let cur = nr[s as usize];
                                        if cur.is_bound() {
                                            ok &= cur == v;
                                        } else {
                                            nr[s as usize] = v;
                                        }
                                    }
                                }
                            }
                            if ok {
                                next.push(nr);
                            }
                        }
                    }
                    rows = next;
                }
                rows
            }
            Logical::Path { g, s, path, o } => {
                let gid = self.graph_of(*g, active);
                let end = |t: &PatTerm| match t {
                    PatTerm::Const(c) => Some(*c),
                    PatTerm::Slot(x) => subst[*x as usize].is_bound().then(|| subst[*x as usize]),
                };
                let mut out = Vec::new();
                for (a, b) in path_pairs(self.d, gid, path, end(s), end(o)) {
                    let mut r = subst.clone();
                    let mut ok = true;
                    for (t, v) in [(s, a), (o, b)] {
                        if let PatTerm::Slot(x) = t {
                            let cur = r[*x as usize];
                            if cur.is_bound() {
                                ok &= cur == v;
                            } else {
                                r[*x as usize] = v;
                            }
                        }
                    }
                    if ok {
                        out.push(r);
                    }
                }
                out
            }
            Logical::Values { slots, rows } => {
                let mut out = Vec::new();
                for vr in rows {
                    let mut r = vec![TermId::UNBOUND; w];
                    for (s, v) in slots.iter().zip(vr) {
                        r[*s as usize] = *v;
                    }
                    if let Some(m) = merge(subst, &r) {
                        out.push(m);
                    }
                }
                out
            }
            Logical::Join(a, b) => {
                let (ra, rb) = (self.eval(a, active, subst), self.eval(b, active, subst));
                let mut out = Vec::new();
                for x in &ra {
                    for y in &rb {
                        if let Some(m) = merge(x, y) {
                            out.push(m);
                        }
                    }
                }
                out
            }
            Logical::LeftJoin { left, right, filter } => {
                let (ra, rb) = (self.eval(left, active, subst), self.eval(right, active, subst));
                let mut out = Vec::new();
                for x in &ra {
                    let mut hit = false;
                    for y in &rb {
                        if let Some(m) = merge(x, y) {
                            if filter.as_ref().is_none_or(|f| ebv(eval_expr(f, &m, subst, &mut ex)) == Some(true)) {
                                out.push(m);
                                hit = true;
                            }
                        }
                    }
                    if !hit {
                        out.push(x.clone());
                    }
                }
                out
            }
            Logical::Minus { left, right } => {
                let (ra, rb) = (self.eval(left, active, subst), self.eval(right, active, subst));
                ra.into_iter()
                    .filter(|x| {
                        !rb.iter().any(|y| {
                            merge(x, y).is_some()
                                && (0..w).any(|i| x[i].is_bound() && y[i].is_bound() && !subst[i].is_bound())
                        })
                    })
                    .collect()
            }
            Logical::Union(a, b) => {
                let mut v = self.eval(a, active, subst);
                v.extend(self.eval(b, active, subst));
                v
            }
            Logical::Filter { input, expr } => self
                .eval(input, active, subst)
                .into_iter()
                .filter(|r| ebv(eval_expr(expr, r, subst, &mut ex)) == Some(true))
                .collect(),
            Logical::Extend { input, slot, expr } => self
                .eval(input, active, subst)
                .into_iter()
                .filter_map(|mut r| {
                    let v = eval_expr(expr, &r, subst, &mut ex);
                    if subst[*slot as usize].is_bound() {
                        // BIND onto a substituted slot: keep the row iff the value agrees.
                        return (v == V::Term(subst[*slot as usize])).then_some(r);
                    }
                    if let V::Term(t) = v {
                        r[*slot as usize] = t;
                    }
                    Some(r)
                })
                .collect(),
            Logical::Graph { name: PatTerm::Const(c), input } => {
                if self.d.named.contains(c) {
                    self.eval(input, Some(*c), subst)
                } else {
                    vec![]
                }
            }
            Logical::Graph { name: PatTerm::Slot(g), input } => {
                let mut out = Vec::new();
                for &gg in &self.d.named {
                    let sv = subst[*g as usize];
                    if sv.is_bound() && sv != gg {
                        continue;
                    }
                    for mut r in self.eval(input, Some(gg), subst) {
                        if r[*g as usize].is_bound() && r[*g as usize] != gg {
                            continue;
                        }
                        r[*g as usize] = gg;
                        out.push(r);
                    }
                }
                out
            }
            Logical::Group { input, keys, aggs } => {
                let rows = self.eval(input, active, subst);
                group(&rows, keys, aggs, w, subst, &mut ex)
            }
            Logical::OrderBy { input, keys } => {
                let mut rows = self.eval(input, active, subst);
                sort_rows(&mut rows, keys, subst, &mut ex);
                rows
            }
            Logical::Project { input, slots } => {
                self.eval(input, active, subst).into_iter().map(|r| project(&r, slots)).collect()
            }
            Logical::Distinct(input) | Logical::Reduced(input) => dedupe(self.eval(input, active, subst), None),
            Logical::Slice { input, offset, limit } => slice(self.eval(input, active, subst), *offset, *limit),
        }
    }
}

fn project(r: &Row, slots: &[Slot]) -> Row {
    let mut out = vec![TermId::UNBOUND; r.len()];
    for s in slots {
        out[*s as usize] = r[*s as usize];
    }
    out
}

fn dedupe(rows: Vec<Row>, slots: Option<&[Slot]>) -> Vec<Row> {
    let mut seen: Vec<Row> = Vec::new();
    let mut out = Vec::new();
    for r in rows {
        let k = match slots {
            Some(s) => project(&r, s),
            None => r.clone(),
        };
        if !seen.contains(&k) {
            seen.push(k);
            out.push(r);
        }
    }
    out
}

fn slice(rows: Vec<Row>, offset: u64, limit: Option<u64>) -> Vec<Row> {
    rows.into_iter().skip(offset as usize).take(limit.map_or(usize::MAX, |l| l as usize)).collect()
}

fn group(
    rows: &[Row],
    keys: &[Slot],
    aggs: &[(Slot, sq_core::ir::AggSpec)],
    w: usize,
    subst: &Row,
    ex: &mut dyn FnMut(sq_core::ir::ExistsId, &Row) -> bool,
) -> Vec<Row> {
    let mut groups: Vec<(Vec<TermId>, Vec<&Row>)> = Vec::new();
    for r in rows {
        let k: Vec<TermId> = keys.iter().map(|s| r[*s as usize]).collect();
        match groups.iter_mut().find(|(gk, _)| *gk == k) {
            Some(g) => g.1.push(r),
            None => groups.push((k, vec![r])),
        }
    }
    if keys.is_empty() && groups.is_empty() {
        groups.push((vec![], vec![]));
    }
    groups
        .into_iter()
        .map(|(k, members)| {
            let mut out = vec![TermId::UNBOUND; w];
            for (s, v) in keys.iter().zip(&k) {
                out[*s as usize] = *v;
            }
            for (o, a) in aggs {
                let n = match (&a.func, &a.arg) {
                    (AggFunc::CountAll, _) if !a.distinct => members.len(),
                    (AggFunc::Count, Some(e)) if !a.distinct => {
                        members.iter().filter(|r| !matches!(eval_expr(e, r, subst, ex), V::Err)).count()
                    }
                    other => panic!("unsupported aggregate {other:?}"),
                };
                out[*o as usize] = TermId::from_int(n as i64).unwrap();
            }
            out
        })
        .collect()
}

// ------------------------------------------------------------------ physical plan

pub struct Phys<'a> {
    pub d: &'a Data,
    pub q: &'a QueryPlan,
    pub violations: RefCell<Vec<String>>,
}

impl Phys<'_> {
    fn bad(&self, p: &Plan, msg: &str) {
        let mut v = self.violations.borrow_mut();
        if v.len() < 20 {
            v.push(format!("#{}: {msg}", p.id));
        }
    }

    fn exists(&self, id: sq_core::ir::ExistsId, row: &Row, reg: &[TermId]) -> bool {
        let w = row.len();
        !self.eval(&self.q.exists[id.0 as usize], &vec![TermId::UNBOUND; w], reg, row).is_empty()
    }

    fn graph_of(&self, p: &Plan, g: GraphSel, reg: &[TermId]) -> Vec<(TermId, Option<Slot>)> {
        match g {
            GraphSel::Default => vec![(TermId::DEFAULT_GRAPH, None)],
            GraphSel::Active => match reg.last() {
                Some(g) => vec![(*g, None)],
                None => {
                    self.bad(p, "Active graph outside a GraphLoop");
                    vec![]
                }
            },
            GraphSel::Const(c) => vec![(c, None)],
            GraphSel::Slot(s) => self.d.named.iter().map(|g| (*g, Some(s))).collect(),
        }
    }

    /// Join two rows; `keys` must be bound on both sides and equal, `check` compatible, and any
    /// other slot bound on both sides must agree (the planner promises it never differs).
    fn join_rows(&self, p: &Plan, l: &Row, r: &Row, keys: &[Slot], check: &[Slot]) -> Option<Row> {
        for k in keys {
            let (a, b) = (l[*k as usize], r[*k as usize]);
            if !a.is_bound() || !b.is_bound() {
                self.bad(p, &format!("key slot {k} unbound"));
                return None;
            }
            if a != b {
                return None;
            }
        }
        for c in check {
            let (a, b) = (l[*c as usize], r[*c as usize]);
            if a.is_bound() && b.is_bound() && a != b {
                return None;
            }
        }
        let mut out = l.clone();
        for (i, v) in r.iter().enumerate() {
            if v.is_bound() {
                if out[i].is_bound() && out[i] != *v {
                    self.bad(p, &format!("slot {i} bound differently on both sides but neither key nor check"));
                    return None;
                }
                out[i] = *v;
            }
        }
        Some(out)
    }

    pub fn eval(&self, p: &Plan, jseed: &Row, reg: &[TermId], subst: &Row) -> Vec<Row> {
        let rows = self.eval_inner(p, jseed, reg, subst);
        if let Some(x) = p.sorted_by {
            if rows.windows(2).any(|w| w[0][x as usize].raw() > w[1][x as usize].raw()) {
                self.bad(p, &format!("sorted_by {x} claimed but the output is not sorted"));
            }
        }
        rows
    }

    fn eval_inner(&self, p: &Plan, jseed: &Row, reg: &[TermId], subst: &Row) -> Vec<Row> {
        let w = jseed.len();
        let empty = vec![TermId::UNBOUND; w];
        let mut ex = |id, r: &Row| self.exists(id, r, reg);
        let base = match merge(jseed, subst) {
            Some(b) => b,
            None => return vec![],
        };
        match &p.op {
            Op::Unit => vec![base],
            Op::Empty => vec![],
            Op::Scan(sc) => {
                let pat = sc.pat;
                let mut keyed: Vec<(usize, [i64; 4], Row)> = Vec::new();
                for (gi, (g, gslot)) in self.graph_of(p, pat.g, reg).into_iter().enumerate() {
                    if let Some(gs) = gslot {
                        let b = base[gs as usize];
                        if b.is_bound() && b != g {
                            continue;
                        }
                    }
                    for q in self.d.triples(g).iter() {
                        let mut nr = base.clone();
                        let mut ok = true;
                        let gterm = gslot.map(PatTerm::Slot);
                        let mut terms = vec![(pat.s, q[1]), (pat.p, q[2]), (pat.o, q[3])];
                        if let Some(t) = gterm {
                            terms.insert(0, (t, g));
                        }
                        for (t, v) in terms {
                            match t {
                                PatTerm::Const(c) => ok &= c == v,
                                PatTerm::Slot(s) => {
                                    let cur = nr[s as usize];
                                    if cur.is_bound() {
                                        ok &= cur == v;
                                    } else {
                                        nr[s as usize] = v;
                                    }
                                }
                            }
                        }
                        if ok {
                            let mut key = [0i64; 4];
                            for (i, c) in sc.order.cols().iter().enumerate() {
                                key[i] = match c {
                                    sq_core::store::Col::G => q[0].raw(),
                                    sq_core::store::Col::S => q[1].raw(),
                                    sq_core::store::Col::P => q[2].raw(),
                                    sq_core::store::Col::O => q[3].raw(),
                                };
                            }
                            keyed.push((gi, key, nr));
                        }
                    }
                }
                // Emulate the cursor's output order: one sorted stream, except GRAPH ?g over an
                // explicit FROM NAMED list, which may read one g-first cursor per graph.
                let per_graph = matches!(pat.g, GraphSel::Slot(_))
                    && matches!(self.q.dataset.named, sq_core::ir::NamedGraphs::Only(_));
                if per_graph {
                    keyed.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
                } else {
                    keyed.sort_by(|a, b| a.1.cmp(&b.1));
                }
                keyed.into_iter().map(|(_, _, r)| r).collect()
            }
            Op::Values { slots, rows } => {
                let mut out = Vec::new();
                for vr in rows {
                    let mut r = vec![TermId::UNBOUND; w];
                    for (s, v) in slots.iter().zip(vr) {
                        r[*s as usize] = *v;
                    }
                    if let Some(m) = merge(&base, &r) {
                        out.push(m);
                    }
                }
                out
            }
            Op::Path { g, s, path, o } => {
                // Join-mode seeds: has_node rule == evaluate free, keep compatible rows.
                // Substitute: constants.
                let mut out = Vec::new();
                for (gid, gslot) in self.graph_of(p, *g, reg) {
                    if let Some(gs) = gslot {
                        let b = base[gs as usize];
                        if b.is_bound() && b != gid {
                            continue;
                        }
                    }
                    let end = |t: &PatTerm| match t {
                        PatTerm::Const(c) => Some(*c),
                        PatTerm::Slot(x) => subst[*x as usize].is_bound().then(|| subst[*x as usize]),
                    };
                    for (a, b) in path_pairs(self.d, gid, path, end(s), end(o)) {
                        let mut r = base.clone();
                        let mut ok = true;
                        let mut terms = vec![(*s, a), (*o, b)];
                        if let Some(gs) = gslot {
                            terms.push((PatTerm::Slot(gs), gid));
                        }
                        for (t, v) in terms {
                            if let PatTerm::Slot(x) = t {
                                let cur = r[x as usize];
                                if cur.is_bound() {
                                    ok &= cur == v;
                                } else {
                                    r[x as usize] = v;
                                }
                            }
                        }
                        if ok {
                            out.push(r);
                        }
                    }
                }
                out
            }
            Op::BindJoin { left, right, seed, kind, .. } => {
                let lrows = self.eval(left, jseed, reg, subst);
                let mut out = Vec::new();
                for l in lrows {
                    let mut sj = vec![TermId::UNBOUND; w];
                    let mut ok = true;
                    for s in seed {
                        let v = l[*s as usize];
                        if !v.is_bound() {
                            self.bad(p, &format!("BindJoin seed slot {s} unbound in a left row"));
                            ok = false;
                        }
                        sj[*s as usize] = v;
                    }
                    if !ok {
                        continue;
                    }
                    let rrows = self.eval(right, &sj, reg, subst);
                    let mut merged = Vec::new();
                    for r in &rrows {
                        match merge(&l, r) {
                            Some(m) => merged.push(m),
                            None => self.bad(p, "BindJoin right row conflicts with its left row"),
                        }
                    }
                    match kind {
                        JoinKind::Inner => out.extend(merged),
                        JoinKind::Left { filter } => {
                            let pass: Vec<Row> = merged
                                .into_iter()
                                .filter(|m| {
                                    filter.as_ref().is_none_or(|f| ebv(eval_expr(f, m, subst, &mut ex)) == Some(true))
                                })
                                .collect();
                            if pass.is_empty() {
                                out.push(l.clone());
                            } else {
                                out.extend(pass);
                            }
                        }
                        JoinKind::Semi => {
                            if !merged.is_empty() {
                                out.push(l.clone());
                            }
                        }
                        JoinKind::Anti => {
                            if merged.is_empty() {
                                out.push(l.clone());
                            }
                        }
                    }
                }
                out
            }
            Op::HashJoin { build, probe, keys, check, kind, build_is_left } => {
                let (left, right) = if *build_is_left { (build, probe) } else { (probe, build) };
                self.join(p, left, right, keys, check, kind, jseed, reg, subst)
            }
            Op::NestedLoop { left, right, check, kind } => self.join(p, left, right, &[], check, kind, jseed, reg, subst),
            Op::Minus { left, right, keys, check } => {
                let lrows = self.eval(left, jseed, reg, subst);
                let rrows = self.eval(right, &empty, reg, subst);
                lrows
                    .into_iter()
                    .filter(|l| {
                        !rrows.iter().any(|r| {
                            let compat = keys.iter().all(|k| {
                                let (a, b) = (l[*k as usize], r[*k as usize]);
                                a.is_bound() && a == b
                            }) && check.iter().all(|c| {
                                let (a, b) = (l[*c as usize], r[*c as usize]);
                                !(a.is_bound() && b.is_bound() && a != b)
                            });
                            let shared = (0..w).any(|i| l[i].is_bound() && r[i].is_bound() && !subst[i].is_bound());
                            if compat && shared {
                                for i in 0..w {
                                    if l[i].is_bound()
                                        && r[i].is_bound()
                                        && l[i] != r[i]
                                        && !keys.contains(&(i as Slot))
                                        && !check.contains(&(i as Slot))
                                    {
                                        self.bad(p, &format!("Minus: slot {i} differs but is neither key nor check"));
                                    }
                                }
                            }
                            compat && shared
                        })
                    })
                    .collect()
            }
            Op::Union(bs) => bs.iter().flat_map(|b| self.eval(b, jseed, reg, subst)).collect(),
            Op::Filter { input, expr } => self
                .eval(input, jseed, reg, subst)
                .into_iter()
                .filter(|r| ebv(eval_expr(expr, r, subst, &mut ex)) == Some(true))
                .collect(),
            Op::Extend { input, slot, expr, .. } => self
                .eval(input, jseed, reg, subst)
                .into_iter()
                .filter_map(|mut r| {
                    let v = eval_expr(expr, &r, subst, &mut ex);
                    if subst[*slot as usize].is_bound() {
                        return (v == V::Term(subst[*slot as usize])).then_some(r);
                    }
                    if r[*slot as usize].is_bound() {
                        self.bad(p, "Extend overwrites a bound slot");
                    }
                    if let V::Term(t) = v {
                        r[*slot as usize] = t;
                    }
                    Some(r)
                })
                .collect(),
            Op::GraphLoop { slot, input } => {
                let mut out = Vec::new();
                for &g in &self.d.named {
                    let sv = subst[*slot as usize];
                    if sv.is_bound() && sv != g {
                        continue;
                    }
                    let mut reg2 = reg.to_vec();
                    reg2.push(g);
                    for mut r in self.eval(input, &empty, &reg2, subst) {
                        if r[*slot as usize].is_bound() && r[*slot as usize] != g {
                            continue;
                        }
                        r[*slot as usize] = g;
                        out.push(r);
                    }
                }
                out
            }
            Op::GraphNames { slot } => {
                let mut out = Vec::new();
                for &g in &self.d.named {
                    let sv = subst[*slot as usize];
                    if sv.is_bound() && sv != g {
                        continue;
                    }
                    let mut r = subst.clone();
                    r[*slot as usize] = g;
                    out.push(r);
                }
                out
            }
            Op::Group { input, keys, aggs, .. } => {
                let rows = self.eval(input, jseed, reg, subst);
                group(&rows, keys, aggs, w, subst, &mut ex)
            }
            Op::Count { input, out } => {
                let n = self.eval(input, &empty, reg, subst).len();
                let mut r = vec![TermId::UNBOUND; w];
                r[*out as usize] = TermId::from_int(n as i64).unwrap();
                vec![r]
            }
            Op::Sort { input, keys } => {
                let mut rows = self.eval(input, jseed, reg, subst);
                sort_rows(&mut rows, keys, subst, &mut ex);
                rows
            }
            Op::TopK { input, keys, k } => {
                let mut rows = self.eval(input, &empty, reg, subst);
                sort_rows(&mut rows, keys, subst, &mut ex);
                rows.truncate(*k as usize);
                rows
            }
            Op::Distinct { input, slots } | Op::Reduced { input, slots } => {
                dedupe(self.eval(input, jseed, reg, subst), Some(slots))
            }
            Op::Slice { input, offset, limit } => slice(self.eval(input, &empty, reg, subst), *offset, *limit),
            Op::Project { input, slots } => {
                self.eval(input, jseed, reg, subst).into_iter().map(|r| project(&r, slots)).collect()
            }
            other => panic!("operator not emitted by WP5: {other:?}"),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn join(
        &self,
        p: &Plan,
        left: &Plan,
        right: &Plan,
        keys: &[Slot],
        check: &[Slot],
        kind: &JoinKind,
        jseed: &Row,
        reg: &[TermId],
        subst: &Row,
    ) -> Vec<Row> {
        let w = jseed.len();
        let empty = vec![TermId::UNBOUND; w];
        let mut ex = |id, r: &Row| self.exists(id, r, reg);
        let lrows = self.eval(left, jseed, reg, subst);
        let rseed = if *kind == JoinKind::Inner { jseed } else { &empty };
        let rrows = self.eval(right, rseed, reg, subst);
        let mut out = Vec::new();
        for l in &lrows {
            let merged: Vec<Row> = rrows.iter().filter_map(|r| self.join_rows(p, l, r, keys, check)).collect();
            match kind {
                JoinKind::Inner => out.extend(merged),
                JoinKind::Left { filter } => {
                    let pass: Vec<Row> = merged
                        .into_iter()
                        .filter(|m| filter.as_ref().is_none_or(|f| ebv(eval_expr(f, m, subst, &mut ex)) == Some(true)))
                        .collect();
                    if pass.is_empty() {
                        out.push(l.clone());
                    } else {
                        out.extend(pass);
                    }
                }
                JoinKind::Semi => {
                    if !merged.is_empty() {
                        out.push(l.clone());
                    }
                }
                JoinKind::Anti => {
                    if merged.is_empty() {
                        out.push(l.clone());
                    }
                }
            }
        }
        out
    }
}

// ------------------------------------------------------------------ driver

pub fn output(rows: Vec<Row>, form: &QueryForm) -> Vec<Row> {
    if *form == QueryForm::Ask {
        return if rows.is_empty() { vec![] } else { vec![vec![]] };
    }
    let QueryForm::Select(slots) = form else { panic!("select only") };
    let mut out: Vec<Row> = rows.iter().map(|r| slots.iter().map(|s| r[*s as usize]).collect()).collect();
    out.sort();
    out
}

pub fn reference(st: &MemStore, opts: &QueryOptions, q: &str) -> Option<(LoweredQuery, Vec<Row>)> {
    let parsed = sq_plan::parse_query(&format!("{PREFIXES}{q}"), None).ok()?;
    let lowered = sq_plan::lower_query(&parsed, st, opts).ok()?;
    let d = &Data::new(st, &lowered.dataset);
    let w = lowered.vars.width();
    let r = Ref { d, q: &lowered };
    let rows = r.eval(&lowered.root, None, &vec![TermId::UNBOUND; w]);
    let out = output(rows, &lowered.form);
    Some((lowered, out))
}

pub fn physical(st: &MemStore, q: &str, opts: &QueryOptions) -> Result<(QueryPlan, Vec<Row>, Vec<String>), String> {
    let parsed = sq_plan::parse_query(&format!("{PREFIXES}{q}"), None).map_err(|e| e.to_string())?;
    let lowered = sq_plan::lower_query(&parsed, st, opts).map_err(|e| e.to_string())?;
    let plan = sq_plan::optimize(lowered, st, opts).map_err(|e| e.to_string())?;
    let d = &Data::new(st, &plan.dataset);
    assert_eq!(d.named, plan.named_graphs, "dataset named graphs");
    let w = plan.width();
    let ph = Phys { d, q: &plan, violations: RefCell::new(Vec::new()) };
    let rows = ph.eval(&plan.root, &vec![TermId::UNBOUND; w], &[], &vec![TermId::UNBOUND; w]);
    let out = output(rows, &plan.form);
    let v = ph.violations.into_inner();
    Ok((plan, out, v))
}

pub fn policies() -> Vec<(&'static str, QueryOptions)> {
    vec![
        ("auto", QueryOptions::default()),
        ("prefer-bind", QueryOptions { join_policy: JoinPolicy::PreferBind, ..Default::default() }),
        ("prefer-hash", QueryOptions { join_policy: JoinPolicy::PreferHash, ..Default::default() }),
        ("no-reorder", QueryOptions { reorder: false, ..Default::default() }),
    ]
}

pub fn small_store(seed: u64) -> MemStore {
    let mut x = seed | 1;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    let mut ttl = String::new();
    let nodes = [":n0", ":n1", ":n2", ":n3", ":g0", ":g1"];
    let preds = [":p", ":q", ":r"];
    for _ in 0..22 {
        let s = nodes[(next() % 4) as usize];
        let p = preds[(next() % 3) as usize];
        let o = nodes[(next() % 6) as usize];
        ttl.push_str(&format!("{s} {p} {o} .\n"));
    }
    let st = store(&ttl);
    for g in ["http://ex/g0", "http://ex/g1"] {
        let mut t = String::from("@prefix : <http://ex/> .\n");
        for _ in 0..9 {
            let s = nodes[(next() % 4) as usize];
            let p = preds[(next() % 3) as usize];
            let o = nodes[(next() % 4) as usize];
            t.push_str(&format!("{s} {p} {o} .\n"));
        }
        load_ttl(&st, &t, Some(g));
    }
    st
}

pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[(self.next() % xs.len() as u64) as usize]
    }
    pub fn chance(&mut self, n: u64) -> bool {
        self.next() % n == 0
    }
}

const VARS: [&str; 5] = ["?a", "?b", "?c", "?d", "?g"];

fn term(r: &mut Rng, consts: bool) -> String {
    if r.chance(6) {
        return r.pick(&["[]", "_:x", "_:y"]).to_owned();
    }
    if consts && r.chance(4) {
        r.pick(&[":n0", ":n1", ":g0"]).to_owned()
    } else {
        r.pick(&VARS).to_owned()
    }
}

fn triple(r: &mut Rng) -> String {
    let p = r.pick(&[":p", ":q", ":r", ":p", ":q", ":p+", ":q*", "^:r", "?d", ":r?"]);
    format!("{} {p} {} . ", term(r, true), term(r, true))
}

fn expr(r: &mut Rng, depth: u32) -> String {
    let v = |r: &mut Rng| r.pick(&VARS).to_owned();
    match r.next() % if depth > 0 { 11 } else { 7 } {
        0 => format!("{} = {}", v(r), v(r)),
        1 => format!("{} != {}", v(r), v(r)),
        2 => format!("{} = {}", v(r), r.pick(&[":n0", ":n1", ":g0", ":absent"])),
        3 => format!("sameTerm({}, {})", v(r), r.pick(&[":n0", ":n1", ":g1"])),
        4 => format!("!bound({})", v(r)),
        5 => format!("bound({})", v(r)),
        6 => format!("{} != {}", v(r), r.pick(&[":n0", ":n2"])),
        7 => format!("({} && {})", expr(r, depth - 1), expr(r, depth - 1)),
        8 => format!("({} || {})", expr(r, depth - 1), expr(r, depth - 1)),
        9 => format!("EXISTS {{ {} }}", gen_group(r, depth - 1)),
        _ => format!("NOT EXISTS {{ {} }}", gen_group(r, depth - 1)),
    }
}

pub const ALL_VARS: &str = "?a ?b ?c ?d ?g ?e ?n";

pub fn gen_group(r: &mut Rng, depth: u32) -> String {
    let mut out = String::new();
    for _ in 0..1 + r.next() % 2 {
        out.push_str(&triple(r));
    }
    let parts = if depth > 0 { 1 + r.next() % 2 } else { 0 };
    for _ in 0..parts {
        let d = depth - 1;
        let part = match r.next() % 21 {
            0 | 1 => format!("OPTIONAL {{ {} }} ", gen_group(r, d)),
            2 => format!("OPTIONAL {{ {} FILTER({}) }} ", gen_group(r, d), expr(r, 1)),
            3 => format!("{{ {} }} UNION {{ {} }} ", gen_group(r, d), gen_group(r, d)),
            4 => format!("MINUS {{ {} }} ", gen_group(r, d)),
            5 => format!("{{ {} FILTER({}) }} ", gen_group(r, d), expr(r, 1)),
            6 => format!("{{ SELECT * {{ {} }} ORDER BY {ALL_VARS} LIMIT 2 }} ", gen_group(r, d)),
            7 => format!("{{ SELECT {} {} {{ {} }} }} ", r.pick(&VARS), r.pick(&VARS), gen_group(r, d)),
            8 => format!("{{ SELECT DISTINCT {} {{ {} }} }} ", r.pick(&VARS), gen_group(r, d)),
            9 => format!("{{ SELECT ?a (COUNT(*) AS ?n) {{ {} }} GROUP BY ?a }} ", gen_group(r, d)),
            10 => format!("GRAPH {} {{ {} }} ", r.pick(&["?g", "?g", ":g0", ":g0", ":g1", ":absent"]), gen_group(r, d)),
            11 => r.pick(&["VALUES ?a { :n0 :n1 UNDEF } ", "VALUES (?b ?g) { (:n1 UNDEF) (UNDEF :g0) } "]).to_owned(),
            12 => format!("BIND({} AS ?e) ", r.pick(&["?a", "?b", ":n0", "?g"])),
            13 => format!("{{ SELECT (COUNT(*) AS ?n) {{ {} }} }} ", gen_group(r, d)),
            14 => format!("OPTIONAL {{ {} FILTER({} = {}) }} ", gen_group(r, d), r.pick(&VARS), r.pick(&VARS)),
            15 => format!("OPTIONAL {{ {} FILTER(EXISTS {{ {} }}) }} ", gen_group(r, d), gen_group(r, d)),
            16 => format!("FILTER(EXISTS {{ {} MINUS {{ {} }} }}) ", gen_group(r, d), gen_group(r, d)),
            17 => format!("{{ SELECT ?a (COUNT(*) AS ?n) {{ {} }} GROUP BY ?a HAVING(?a != :n0) }} ", gen_group(r, d)),
            18 => format!("FILTER({} = {}) ", r.pick(&VARS), r.pick(&[":n0", ":n1", ":g0"])),
            19 => format!("OPTIONAL {{ {} OPTIONAL {{ {} FILTER({} = {}) }} }} ", gen_group(r, d), gen_group(r, d), r.pick(&VARS), r.pick(&VARS)),
            _ => format!("FILTER({}) ", expr(r, 1)),
        };
        out.push_str(&part);
    }
    if r.chance(3) {
        out.push_str(&format!("FILTER({}) ", expr(r, 1)));
    }
    out
}

#[test]
fn review_physical_plans_match_naive_evaluation() {
    let n: usize = std::env::var("REVIEW_N").ok().and_then(|s| s.parse().ok()).unwrap_or(250);
    let seed0: u64 = std::env::var("REVIEW_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(7);
    let mut r = Rng(0x9E37_79B9_7F4A_7C15 ^ seed0);
    let mut problems = Vec::new();
    let (mut checked, mut nonempty) = (0, 0);
    for i in 0..n {
        let store_seed = seed0.wrapping_mul(1000) + i as u64;
        let st = small_store(store_seed);
        let depth = 1 + (r.next() % 3) as u32;
        let ds = match r.next() % 8 {
            0 => "FROM :g0 FROM NAMED :g0 FROM NAMED :g1 ",
            1 => "FROM :g0 FROM :g1 FROM NAMED :g0 ",
            _ => "",
        };
        let body = gen_group(&mut r, depth);
        let q = match r.next() % 9 {
            0 => format!("SELECT DISTINCT ?a ?b {ds}{{ {body}}}"),
            1 => format!("SELECT * {ds}{{ {body}}} ORDER BY {ALL_VARS} LIMIT 3 OFFSET 1"),
            2 => format!("SELECT DISTINCT ?a {ds}{{ {body}}} ORDER BY ?a LIMIT 2"),
            3 => format!("ASK {ds}{{ {body}}}"),
            4 => format!("SELECT (COUNT(*) AS ?cnt) {ds}{{ {body}}}"),
            5 => format!("SELECT ?a (COUNT(*) AS ?cnt) {ds}{{ {body}}} GROUP BY ?a HAVING(?a != :n1)"),
            _ => format!("SELECT * {ds}{{ {body}}}"),
        };
        let union = r.chance(5);
        let mut parsed = false;
        for (name, mut opts) in policies() {
            opts.union_default_graph = union;
            let Some((_, expected)) = reference(&st, &opts, &q) else { break };
            parsed = true;
            if name == "auto" && !expected.is_empty() {
                nonempty += 1;
            }
            match physical(&st, &q, &opts) {
                Err(e) => problems.push(format!("[{name} union={union}] store={store_seed} {q}\n   plan error: {e}")),
                Ok((plan, got, v)) => {
                    if got != expected || !v.is_empty() {
                        problems.push(format!(
                            "[{name} union={union}] store={store_seed} {q}\n   expected {} rows, got {}; violations {:?}\n{}",
                            expected.len(),
                            got.len(),
                            v,
                            sq_plan::explain(&plan, None)
                        ));
                    }
                }
            }
        }
        if parsed {
            checked += 1;
        }
    }
    assert!(checked > n / 2 && nonempty > checked / 10, "generator degenerated: {checked} checked, {nonempty} with rows");
    assert!(
        problems.is_empty(),
        "{} problems in {checked} queries; first ones:\n{}",
        problems.len(),
        problems.iter().take(4).cloned().collect::<Vec<_>>().join("\n\n")
    );
}
