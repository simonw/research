//! Update differential: random SPARQL Update requests applied to MemStore and to both
//! SQLite stores through `sq_engine::update`; the reports, the errors and the resulting
//! datasets (modulo blank-node renaming) must agree. Each request runs under random query
//! options (join policy, reorder, `union_default_graph`, which updates must ignore), the same
//! on every store, and the postcondition of its last operation is checked against the state
//! the earlier operations left (COPY copies the source, DELETE WHERE leaves no match, ...).
use std::collections::BTreeSet;
use std::fmt::Write;
use std::rc::Rc;
use std::sync::Arc;

use sq_core::plan::{JoinPolicy, QueryOptions};
use sq_core::store::{IndexOrder, QuadStore, QuadStoreWriter, ScanSpec};
use sq_core::testing::Rng;
use sq_core::{Result, Term, TermId};
use sq_engine::{UpdateOptions, UpdateReport};
use sq_memstore::MemStore;

use super::compare::{same, Free, Outcome, Row};
use super::data::{Dataset, D, EMPTY_GRAPH, GRAPHS, LITERALS, PREDICATES, SUBJECTS};
use super::engines::{mem_store, now, StoreKind, Stores};
use super::query::{Gen, Group, PREFIXES, T};

/// Graph names an update may use: the data's, the one some datasets register empty, and one
/// that does not exist.
const UPDATE_GRAPHS: [&str; 4] = [GRAPHS[0], GRAPHS[1], EMPTY_GRAPH, "gnew"];

/// A template quad: graph None = default graph.
type Tpl = (Option<T>, T, T, T);

#[derive(Clone, Debug)]
pub enum Op {
    InsertData(Vec<Tpl>),
    DeleteData(Vec<Tpl>),
    Modify { with: Option<&'static str>, delete: Vec<Tpl>, insert: Vec<Tpl>, using: Vec<&'static str>, pattern: Group },
    DeleteWhere(Vec<Tpl>),
    /// CLEAR / DROP: (drop?, silent, target: graph name or DEFAULT / NAMED / ALL).
    ClearDrop(bool, bool, &'static str),
    Create(bool, &'static str),
    /// ADD / COPY / MOVE (keyword, silent, from, to; "DEFAULT" or a graph).
    Transfer(&'static str, bool, &'static str, &'static str),
}

fn term_text(t: &T) -> String {
    match t {
        // Template blank nodes are labelled per operation.
        T::BNode(n) => format!("_:u{n}"),
        t => t.to_string(),
    }
}

fn quads_text(quads: &[Tpl]) -> String {
    let mut out = String::new();
    for (g, s, p, o) in quads {
        let triple = format!("{} {} {} .", term_text(s), term_text(p), term_text(o));
        match g {
            None => {
                let _ = write!(out, " {triple}");
            }
            Some(g) => {
                let _ = write!(out, " GRAPH {} {{ {triple} }}", term_text(g));
            }
        }
    }
    out
}

fn graph_ref(g: &str) -> String {
    match g {
        "DEFAULT" | "NAMED" | "ALL" => g.to_owned(),
        g => format!("GRAPH :{g}"),
    }
}

fn plain_graph(g: &str) -> String {
    if g == "DEFAULT" { g.to_owned() } else { format!(":{g}") }
}

impl std::fmt::Display for Op {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let silent = |s: &bool| if *s { " SILENT" } else { "" };
        match self {
            Op::InsertData(q) => write!(f, "INSERT DATA {{{} }}", quads_text(q)),
            Op::DeleteData(q) => write!(f, "DELETE DATA {{{} }}", quads_text(q)),
            Op::DeleteWhere(q) => write!(f, "DELETE WHERE {{{} }}", quads_text(q)),
            Op::Modify { with, delete, insert, using, pattern } => {
                if let Some(g) = with {
                    write!(f, "WITH :{g} ")?;
                }
                if !delete.is_empty() {
                    write!(f, "DELETE {{{} }} ", quads_text(delete))?;
                }
                if !insert.is_empty() {
                    write!(f, "INSERT {{{} }} ", quads_text(insert))?;
                }
                for g in using {
                    write!(f, "USING :{g} ")?;
                }
                let mut body = String::new();
                pattern.print(&mut body, 0);
                write!(f, "WHERE {body}")
            }
            Op::ClearDrop(drop, s, target) => {
                write!(f, "{}{} {}", if *drop { "DROP" } else { "CLEAR" }, silent(s), graph_ref(target))
            }
            Op::Create(s, g) => write!(f, "CREATE{} GRAPH :{g}", silent(s)),
            Op::Transfer(kw, s, from, to) => write!(f, "{kw}{} {} TO {}", silent(s), plain_graph(from), plain_graph(to)),
        }
    }
}

/// An update request and the query options it runs under.
#[derive(Clone, Debug)]
pub struct Request {
    pub ops: Vec<Op>,
    pub union_default: bool,
    pub policy: JoinPolicy,
    pub reorder: bool,
}

impl std::fmt::Display for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(PREFIXES)?;
        for (i, op) in self.ops.iter().enumerate() {
            if i > 0 {
                f.write_str(" ;\n")?;
            }
            write!(f, "{op}")?;
        }
        f.write_str("\n")
    }
}

const POLICIES: [JoinPolicy; 5] =
    [JoinPolicy::Auto, JoinPolicy::PreferHash, JoinPolicy::PreferBind, JoinPolicy::PreferMerge, JoinPolicy::NoSql];

impl Request {
    pub fn generate(r: &mut Rng, data: &Dataset) -> Request {
        let n = 1 + r.below(3);
        let mut bnodes = 0;
        let ops = (0..n).map(|_| op(r, data, &mut bnodes)).collect();
        Request { ops, union_default: r.below(3) == 0, policy: pick(r, &POLICIES), reorder: r.below(4) != 0 }
    }

    fn with_ops(&self, ops: Vec<Op>) -> Request {
        Request { ops, ..self.clone() }
    }

    /// The query options of the request's WHERE clauses.
    pub fn options(&self) -> QueryOptions {
        QueryOptions {
            union_default_graph: self.union_default,
            join_policy: self.policy,
            reorder: self.reorder,
            timeout_ms: Some(60_000),
            ..QueryOptions::default()
        }
    }

    /// One operation fewer, or a simpler operation (or default options).
    pub fn simplifications(&self) -> Vec<Request> {
        let mut out = Vec::new();
        if self.union_default || self.policy != JoinPolicy::Auto || !self.reorder {
            out.push(Request { union_default: false, policy: JoinPolicy::Auto, reorder: true, ..self.clone() });
        }
        if self.ops.len() > 1 {
            for i in 0..self.ops.len() {
                let mut ops = self.ops.clone();
                ops.remove(i);
                out.push(self.with_ops(ops));
            }
        }
        for (i, op) in self.ops.iter().enumerate() {
            let smaller: Vec<Op> = match op {
                Op::InsertData(q) | Op::DeleteData(q) | Op::DeleteWhere(q) if q.len() > 1 => (0..q.len())
                    .map(|k| {
                        let mut q2 = q.clone();
                        q2.remove(k);
                        match op {
                            Op::InsertData(_) => Op::InsertData(q2),
                            Op::DeleteData(_) => Op::DeleteData(q2),
                            _ => Op::DeleteWhere(q2),
                        }
                    })
                    .collect(),
                Op::Modify { with, delete, insert, using, pattern } => {
                    let mut v = Vec::new();
                    if with.is_some() {
                        v.push(Op::Modify { with: None, delete: delete.clone(), insert: insert.clone(), using: using.clone(), pattern: pattern.clone() });
                    }
                    if !using.is_empty() {
                        v.push(Op::Modify { with: *with, delete: delete.clone(), insert: insert.clone(), using: Vec::new(), pattern: pattern.clone() });
                    }
                    for k in (0..delete.len()).filter(|_| delete.len() + insert.len() > 1) {
                        let mut d = delete.clone();
                        d.remove(k);
                        v.push(Op::Modify { with: *with, delete: d, insert: insert.clone(), using: using.clone(), pattern: pattern.clone() });
                    }
                    for k in (0..insert.len()).filter(|_| delete.len() + insert.len() > 1) {
                        let mut ins = insert.clone();
                        ins.remove(k);
                        v.push(Op::Modify { with: *with, delete: delete.clone(), insert: ins, using: using.clone(), pattern: pattern.clone() });
                    }
                    for k in 0..pattern.0.len() {
                        let mut p = pattern.clone();
                        p.0.remove(k);
                        v.push(Op::Modify { with: *with, delete: delete.clone(), insert: insert.clone(), using: using.clone(), pattern: p });
                    }
                    v
                }
                _ => Vec::new(),
            };
            for op2 in smaller {
                let mut ops = self.ops.clone();
                ops[i] = op2;
                out.push(self.with_ops(ops));
            }
        }
        out
    }
}

fn pick<X: Copy>(r: &mut Rng, xs: &[X]) -> X {
    xs[r.below(xs.len() as u64) as usize]
}

fn data_term(d: D) -> T {
    match d {
        D::Iri(i) => T::Iri(i),
        D::Type => T::Type,
        D::Lit(l) => T::Lit(l),
        D::BNode(n) => T::BNode(n as usize + 1000),
    }
}

/// A ground quad: usually one of the data's (so deletes hit), sometimes new.
fn ground_quad(r: &mut Rng, data: &Dataset) -> Tpl {
    if !data.quads.is_empty() && r.below(3) != 0 {
        let q = &data.quads[r.below(data.quads.len() as u64) as usize];
        if !matches!(q.s, D::BNode(_)) && !matches!(q.o, D::BNode(_)) {
            return (q.graph.map(T::Iri), data_term(q.s), data_term(q.p), data_term(q.o));
        }
    }
    let g = if r.below(3) == 0 { Some(T::Iri(pick(r, &UPDATE_GRAPHS))) } else { None };
    let o = if r.below(2) == 0 { T::Iri(pick(r, &SUBJECTS[..6])) } else { T::Lit(pick(r, &LITERALS)) };
    (g, T::Iri(pick(r, &SUBJECTS[..6])), T::Iri(pick(r, &PREDICATES)), o)
}

/// A template term over the WHERE variables.
fn tpl_term(r: &mut Rng, vars: &[usize], bnodes: Option<&mut usize>, literal: bool) -> T {
    match r.below(10) {
        0..=5 if !vars.is_empty() => T::Var(pick(r, vars)),
        6 if bnodes.is_some() => {
            let b = bnodes.unwrap();
            *b += 1;
            T::BNode(*b)
        }
        7 if literal => T::Lit(pick(r, &LITERALS)),
        _ => T::Iri(pick(r, &SUBJECTS[..6])),
    }
}

fn tpl_quad(r: &mut Rng, vars: &[usize], mut bnodes: Option<&mut usize>) -> Tpl {
    let g = match r.below(6) {
        0 => Some(T::Iri(pick(r, &UPDATE_GRAPHS))),
        1 if !vars.is_empty() => Some(T::Var(pick(r, vars))),
        _ => None,
    };
    let s = tpl_term(r, vars, bnodes.as_deref_mut(), false);
    let p = if r.below(8) == 0 && !vars.is_empty() { T::Var(pick(r, vars)) } else { T::Iri(pick(r, &PREDICATES)) };
    let o = tpl_term(r, vars, bnodes, true);
    (g, s, p, o)
}

fn op(r: &mut Rng, data: &Dataset, bnodes: &mut usize) -> Op {
    match r.below(22) {
        0..=3 => {
            let quads = (0..1 + r.below(3))
                .map(|_| {
                    let (g, s, p, o) = ground_quad(r, data);
                    // Some fresh blank nodes in INSERT DATA.
                    if r.below(4) == 0 {
                        *bnodes += 1;
                        (g, T::BNode(*bnodes), p, o)
                    } else {
                        (g, s, p, o)
                    }
                })
                .collect();
            Op::InsertData(quads)
        }
        4..=6 => Op::DeleteData((0..1 + r.below(3)).map(|_| ground_quad(r, data)).collect()),
        7..=13 => {
            let mut gr = Rng(r.next_u64() | 1);
            let (pattern, vars) = Gen::new(&mut gr, data).where_pattern();
            let delete: Vec<Tpl> = (0..r.below(3)).map(|_| tpl_quad(r, &vars, None)).collect();
            // At least one template (DELETE {} INSERT {} is not SPARQL).
            let ninsert = if delete.is_empty() { 1 + r.below(2) } else { r.below(3) };
            let insert = (0..ninsert).map(|_| tpl_quad(r, &vars, Some(bnodes))).collect();
            let with = (r.below(6) == 0).then(|| pick(r, &UPDATE_GRAPHS));
            let using = if r.below(6) == 0 { vec![pick(r, &UPDATE_GRAPHS)] } else { Vec::new() };
            Op::Modify { with, delete, insert, using, pattern }
        }
        14 | 15 => {
            let vars: Vec<usize> = (0..3).collect();
            Op::DeleteWhere((0..1 + r.below(2)).map(|_| tpl_quad(r, &vars, None)).collect())
        }
        16 | 17 => {
            let target = pick(r, &["DEFAULT", "NAMED", "ALL", GRAPHS[0], GRAPHS[1], EMPTY_GRAPH, "gnew"]);
            Op::ClearDrop(r.below(2) == 0, r.below(2) == 0, target)
        }
        18 => Op::Create(r.below(2) == 0, pick(r, &UPDATE_GRAPHS)),
        _ => {
            let from = pick(r, &["DEFAULT", "DEFAULT", GRAPHS[0], GRAPHS[1], EMPTY_GRAPH, "gnew"]);
            let to = pick(r, &["DEFAULT", GRAPHS[0], GRAPHS[1], EMPTY_GRAPH, "gnew"]);
            Op::Transfer(pick(r, &["ADD", "COPY", "MOVE"]), r.below(2) == 0, from, to)
        }
    }
}

/// The result of a request on one store: the report (or error) and the dataset after it.
struct Applied {
    report: std::result::Result<UpdateReport, String>,
    dataset: Outcome,
    graphs: Outcome,
}

fn apply(stores: &Stores, kind: StoreKind, update: &spargebra::Update, opts: &QueryOptions) -> Applied {
    let uopts = UpdateOptions { now: Some(now()), ..UpdateOptions::default() };
    let report = match kind {
        StoreKind::Mem => {
            let w: Rc<dyn QuadStoreWriter> = Rc::new(stores.mem.clone());
            sq_engine::update_with(w, update, opts, &uopts)
        }
        StoreKind::Sqlite | StoreKind::File => {
            let db = if kind == StoreKind::Sqlite { &stores.sqlite } else { &stores.file };
            let scope = db.store.write_scope(0).expect("write scope");
            let w: Rc<dyn QuadStoreWriter> = Rc::new(db.store.clone());
            match sq_engine::update_with(w, update, opts, &uopts) {
                Ok(r) => scope.commit().map(|_| r),
                Err(e) => {
                    scope.rollback().expect("rollback");
                    Err(e)
                }
            }
        }
    };
    // MemStore has no rollback: a failed request may have applied some operations, so only
    // compare datasets after successful requests.
    let (dataset, graphs) = stores.with(kind, |s| (dump(&*s), named(&*s)));
    Applied {
        report: report.map_err(|e| e.to_string()),
        dataset: dataset.unwrap_or_else(|e| Outcome::Error(e.to_string())),
        graphs: graphs.unwrap_or_else(|e| Outcome::Error(e.to_string())),
    }
}

/// Every quad, decoded (graph UNDEF = default graph).
fn dump(store: &dyn QuadStore) -> Result<Outcome> {
    let mut cur = store.cursor(&ScanSpec::new(IndexOrder::Gspo, &[]))?;
    let mut ids = Vec::new();
    while cur.fill(&mut ids, 1024)? > 0 {}
    let mut rows = Vec::with_capacity(ids.len() / 4);
    for q in ids.chunks(4) {
        let mut row: Row = Vec::with_capacity(4);
        for id in q {
            row.push(if *id == TermId::DEFAULT_GRAPH { None } else { Some(store.dict().decode_one(*id)?) });
        }
        rows.push(row);
    }
    Ok(Outcome::Graph(rows))
}

fn named(store: &dyn QuadStore) -> Result<Outcome> {
    let mut rows = Vec::new();
    for g in store.named_graphs()? {
        let t: Arc<Term> = store.dict().decode_one(g)?;
        rows.push(vec![Some(t)]);
    }
    Ok(Outcome::Graph(rows))
}

/// Applies `req` to fresh stores loaded with `data`; Some(difference) if the stores disagree.
pub fn check(data: &Dataset, req: &Request) -> Option<std::result::Result<String, ()>> {
    let text = req.to_string();
    let Ok(update) = sq_plan::parse_update(&text, None) else {
        return Some(Err(()));
    };
    let stores = Stores::load(data);
    let opts = req.options();
    let results: Vec<(StoreKind, Applied)> =
        StoreKind::ALL.iter().map(|k| (*k, apply(&stores, *k, &update, &opts))).collect();
    let (_, base) = &results[0];
    for (kind, a) in &results[1..] {
        let reports_agree = match (&base.report, &a.report) {
            (Ok(x), Ok(y)) => x == y,
            (Err(_), Err(_)) => true,
            _ => false,
        };
        if !reports_agree {
            return Some(Ok(format!("{kind:?}: report {:?}, MemStore: {:?}", a.report, base.report)));
        }
        if base.report.is_err() {
            continue;
        }
        if let Err(d) = same(&base.dataset, &a.dataset, super::query::Order::Set, Free::All) {
            return Some(Ok(format!(
                "{kind:?}: dataset differs from MemStore's: {d}\n--- MemStore:\n{}--- {kind:?}:\n{}",
                base.dataset.describe(super::query::Order::Set, 40),
                a.dataset.describe(super::query::Order::Set, 40)
            )));
        }
        if let Err(d) = same(&base.graphs, &a.graphs, super::query::Order::Set, Free::All) {
            return Some(Ok(format!("{kind:?}: named graphs differ from MemStore's: {d}")));
        }
    }
    postcondition(data, req).map(Ok)
}

/// A quad as N-Triples terms (graph "" = the default graph).
type QuadText = [String; 4];

fn quad_set(store: &MemStore) -> BTreeSet<QuadText> {
    let text = |id: TermId| -> String {
        if id == TermId::DEFAULT_GRAPH { String::new() } else { store.dict().decode_one(id).expect("decodes").to_ntriples() }
    };
    store.quads().into_iter().map(|q| q.map(text)).collect()
}

/// Runs `req` on `store`; false if the request fails.
fn run_on(store: &MemStore, req: &Request) -> bool {
    let Ok(update) = sq_plan::parse_update(&req.to_string(), None) else {
        return false;
    };
    let uopts = UpdateOptions { now: Some(now()), ..UpdateOptions::default() };
    sq_engine::update_with(Rc::new(store.clone()), &update, &req.options(), &uopts).is_ok()
}

fn ask(store: &MemStore, pattern: &str) -> bool {
    let q = sq_plan::parse_query(&format!("{PREFIXES}ASK {{ {pattern} }}"), None).expect("ASK parses");
    let mut exec = sq_engine::prepare(Rc::new(store.clone()), &q, &QueryOptions::default(), Default::default())
        .expect("ASK prepares");
    exec.ask().expect("ASK runs")
}

/// The postcondition of the request's last operation, on a MemStore holding the state that
/// the earlier operations left: None when it holds, does not apply, or the operation failed.
fn postcondition(data: &Dataset, req: &Request) -> Option<String> {
    let (last, prefix) = req.ops.split_last()?;
    let store = mem_store(data);
    if !prefix.is_empty() && !run_on(&store, &req.with_ops(prefix.to_vec())) {
        return None;
    }
    let before = quad_set(&store);
    if !run_on(&store, &req.with_ops(vec![last.clone()])) {
        return None;
    }
    let after = quad_set(&store);
    let graph = |g: &str| if g == "DEFAULT" { String::new() } else { format!("<http://ex/{g}>") };
    let fail = |what: &str| Some(format!("postcondition of `{last}`: {what}"));
    let unchanged_except = |dropped: &dyn Fn(&QuadText) -> bool| -> BTreeSet<QuadText> {
        before.iter().filter(|q| !dropped(q)).cloned().collect()
    };
    match last {
        Op::Transfer(kw, _, from, to) => {
            let (from, to) = (graph(from), graph(to));
            let mut expected = before.clone();
            if from != to {
                let source: Vec<QuadText> = before.iter().filter(|q| q[0] == from).cloned().collect();
                if *kw != "ADD" {
                    expected.retain(|q| q[0] != to);
                }
                if *kw == "MOVE" {
                    expected.retain(|q| q[0] != from);
                }
                for [_, s, p, o] in source {
                    expected.insert([to.clone(), s, p, o]);
                }
            }
            if after != expected {
                let missing: Vec<_> = expected.difference(&after).take(4).collect();
                let extra: Vec<_> = after.difference(&expected).take(4).collect();
                return fail(&format!("missing {missing:?}, unexpected {extra:?}"));
            }
        }
        Op::ClearDrop(_, _, target) => {
            let expected = match *target {
                "DEFAULT" => unchanged_except(&|q| q[0].is_empty()),
                "NAMED" => unchanged_except(&|q| !q[0].is_empty()),
                "ALL" => BTreeSet::new(),
                g => {
                    let g = graph(g);
                    unchanged_except(&|q| q[0] == g)
                }
            };
            if after != expected {
                return fail("the dataset is not the old one without the cleared graphs");
            }
        }
        Op::Create(..) if after != before => return fail("CREATE changed the quads"),
        Op::DeleteWhere(tpl) => {
            if !after.is_subset(&before) {
                return fail("DELETE WHERE added quads");
            }
            if ask(&store, &quads_text(tpl)) {
                return fail("the pattern still matches");
            }
        }
        Op::DeleteData(quads) => {
            if !after.is_subset(&before) {
                return fail("DELETE DATA added quads");
            }
            if let Some(q) = quads.iter().find(|q| ask(&store, &quads_text(std::slice::from_ref(q)))) {
                return fail(&format!("{q:?} is still there"));
            }
        }
        Op::InsertData(quads) => {
            if !after.is_superset(&before) {
                return fail("INSERT DATA removed quads");
            }
            let ground = quads.iter().filter(|(_, s, _, o)| !matches!(s, T::BNode(_)) && !matches!(o, T::BNode(_)));
            if let Some(q) = ground.into_iter().find(|q| !ask(&store, &quads_text(std::slice::from_ref(q)))) {
                return fail(&format!("{q:?} is missing"));
            }
        }
        Op::Modify { delete, insert, .. } => {
            if insert.is_empty() && !after.is_subset(&before) {
                return fail("a DELETE-only modify added quads");
            }
            if delete.is_empty() && !after.is_superset(&before) {
                return fail("an INSERT-only modify removed quads");
            }
        }
        Op::Create(..) => {}
    }
    None
}

/// Runs `cases` update requests; returns (cases, invalid, failure reports).
pub fn run(seed: u64, cases: usize, max_reports: usize) -> (usize, usize, Vec<String>) {
    let mut r = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) | 1);
    let (mut invalid, mut reports) = (0, Vec::new());
    for case in 1..=cases {
        let data = Dataset::generate(&mut r);
        let req = Request::generate(&mut r, &data);
        if std::env::var("SQ_FUZZ_TRACE").is_ok_and(|v| v == "1") {
            eprintln!("--- case {case}\n{}\n{req}", data.trig());
        }
        match check(&data, &req) {
            None => {}
            Some(Err(())) => invalid += 1,
            Some(Ok(detail)) => {
                if reports.len() >= max_reports {
                    reports.push(String::new());
                    continue;
                }
                let (data, req, detail) = shrink(data, req, detail);
                let rep = format!(
                    "=== UPDATE MISMATCH (case {case} of seed {seed}; union_default_graph={}; policy={:?}; reorder={})\n\
                     --- data (TriG):\n{}\n--- update:\n{req}\n--- difference: {detail}\n",
                    req.union_default,
                    req.policy,
                    req.reorder,
                    data.trig()
                );
                eprintln!("{rep}");
                reports.push(rep);
            }
        }
    }
    let failures = reports.len();
    eprintln!("update differential: seed {seed} - {cases} requests ({invalid} invalid), {failures} failures");
    (cases, invalid, reports.into_iter().filter(|r| !r.is_empty()).collect())
}

fn shrink(mut data: Dataset, mut req: Request, mut detail: String) -> (Dataset, Request, String) {
    let mut budget = 400;
    loop {
        let mut progress = false;
        for cand in req.simplifications() {
            budget -= 1;
            if budget == 0 {
                return (data, req, detail);
            }
            if let Some(Ok(d)) = check(&data, &cand) {
                req = cand;
                detail = d;
                progress = true;
                break;
            }
        }
        if !progress {
            for cand in data.simplifications() {
                budget -= 1;
                if budget == 0 {
                    return (data, req, detail);
                }
                if let Some(Ok(d)) = check(&cand, &req) {
                    data = cand;
                    detail = d;
                    progress = true;
                    break;
                }
            }
        }
        if !progress {
            return (data, req, detail);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The postconditions of ADD / COPY / MOVE, DELETE WHERE and CLEAR hold, including under
    /// `union_default_graph` (which update WHERE clauses must ignore).
    #[test]
    fn postconditions_hold_for_graph_transfers() {
        let data = Dataset::from_trig(":s0 :p0 :s1 .\n:g0 { :s1 :p1 :s2 }\n:g1 { :s2 :p2 \"a\" }", false);
        let ops = [
            Op::Transfer("COPY", false, "DEFAULT", "g1"),
            Op::Transfer("MOVE", false, "DEFAULT", "g0"),
            Op::Transfer("ADD", false, "DEFAULT", "gnew"),
            Op::Transfer("MOVE", false, "g0", "DEFAULT"),
            Op::Transfer("COPY", true, "gnew", "g0"),
            Op::ClearDrop(true, false, "NAMED"),
            Op::DeleteWhere(vec![(None, T::Var(0), T::Var(1), T::Var(2))]),
        ];
        for union_default in [false, true] {
            for op in &ops {
                let req = Request { ops: vec![op.clone()], union_default, policy: JoinPolicy::Auto, reorder: true };
                assert_eq!(postcondition(&data, &req), None, "{req}");
            }
        }
    }

    #[test]
    fn requests_print_valid_sparql() {
        let mut r = Rng(5);
        let mut invalid = 0;
        for _ in 0..200 {
            let data = Dataset::generate(&mut r);
            let req = Request::generate(&mut r, &data);
            if let Err(e) = sq_plan::parse_update(&req.to_string(), None) {
                invalid += 1;
                assert!(invalid < 30, "too many invalid requests: {e}\n{req}");
            }
        }
    }
}
