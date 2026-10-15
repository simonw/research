//! Update differential: random SPARQL Update requests applied to MemStore and to both
//! SQLite stores through `sq_engine::update`; the reports, the errors and the resulting
//! datasets (modulo blank-node renaming) must agree.
use std::fmt::Write;
use std::rc::Rc;
use std::sync::Arc;

use sq_core::store::{IndexOrder, QuadStore, QuadStoreWriter, ScanSpec};
use sq_core::testing::Rng;
use sq_core::{Result, Term, TermId};
use sq_engine::{UpdateOptions, UpdateReport};

use super::compare::{same, Free, Outcome, Row};
use super::data::{Dataset, D, GRAPHS, LITERALS, PREDICATES, SUBJECTS};
use super::engines::{now, StoreKind, Stores};
use super::query::{Gen, Group, PREFIXES, T};

/// Graph names an update may use: the data's, plus one that does not exist.
const UPDATE_GRAPHS: [&str; 3] = [GRAPHS[0], GRAPHS[1], "gnew"];

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

/// An update request.
#[derive(Clone, Debug)]
pub struct Request(pub Vec<Op>);

impl std::fmt::Display for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(PREFIXES)?;
        for (i, op) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(" ;\n")?;
            }
            write!(f, "{op}")?;
        }
        f.write_str("\n")
    }
}

impl Request {
    pub fn generate(r: &mut Rng, data: &Dataset) -> Request {
        let n = 1 + r.below(3);
        let mut bnodes = 0;
        let ops = (0..n).map(|_| op(r, data, &mut bnodes)).collect();
        Request(ops)
    }

    /// One operation fewer, or a simpler operation.
    pub fn simplifications(&self) -> Vec<Request> {
        let mut out = Vec::new();
        if self.0.len() > 1 {
            for i in 0..self.0.len() {
                let mut ops = self.0.clone();
                ops.remove(i);
                out.push(Request(ops));
            }
        }
        for (i, op) in self.0.iter().enumerate() {
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
                let mut ops = self.0.clone();
                ops[i] = op2;
                out.push(Request(ops));
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
    match r.below(20) {
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
        14 => {
            let vars: Vec<usize> = (0..3).collect();
            Op::DeleteWhere((0..1 + r.below(2)).map(|_| tpl_quad(r, &vars, None)).collect())
        }
        15 | 16 => {
            let target = pick(r, &["DEFAULT", "NAMED", "ALL", GRAPHS[0], GRAPHS[1], "gnew"]);
            Op::ClearDrop(r.below(2) == 0, r.below(2) == 0, target)
        }
        17 => Op::Create(r.below(2) == 0, pick(r, &UPDATE_GRAPHS)),
        _ => {
            let from = pick(r, &["DEFAULT", GRAPHS[0], GRAPHS[1], "gnew"]);
            let to = pick(r, &["DEFAULT", GRAPHS[0], GRAPHS[1], "gnew"]);
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

fn apply(stores: &Stores, kind: StoreKind, update: &spargebra::Update) -> Applied {
    let uopts = UpdateOptions { now: Some(now()), ..UpdateOptions::default() };
    let opts = sq_core::plan::QueryOptions::default();
    let report = match kind {
        StoreKind::Mem => {
            let w: Rc<dyn QuadStoreWriter> = Rc::new(stores.mem.clone());
            sq_engine::update_with(w, update, &opts, &uopts)
        }
        StoreKind::Sqlite | StoreKind::File => {
            let db = if kind == StoreKind::Sqlite { &stores.sqlite } else { &stores.file };
            let scope = db.store.write_scope(0).expect("write scope");
            let w: Rc<dyn QuadStoreWriter> = Rc::new(db.store.clone());
            match sq_engine::update_with(w, update, &opts, &uopts) {
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
    let results: Vec<(StoreKind, Applied)> = StoreKind::ALL.iter().map(|k| (*k, apply(&stores, *k, &update))).collect();
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
                    "=== UPDATE MISMATCH (case {case} of seed {seed})\n--- data (TriG):\n{}\n--- update:\n{req}\n--- difference: {detail}\n",
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
