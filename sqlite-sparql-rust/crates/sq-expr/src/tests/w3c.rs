//! W3C-driven check of expression results. A deliberately small reference evaluator (nested
//! loops over the default graph, bags of rows in a global slot layout) runs the
//! query-evaluation tests of the expression-heavy W3C directories through this crate's
//! `compile` / `eval` / `ebv` / `sort_key` / `accumulator` and compares with the expected
//! result files (multiset equality; blank nodes compared up to renaming of labels; a
//! lenient pass accepts equal numeric values whose lexical forms differ).
//!
//! The suite lives outside the repository: set `SQ_W3C_DIR` to the `rdf-tests/sparql`
//! directory (default: the scratchpad checkout). Without it the test is skipped.
//! Tests using features this harness does not implement (named graphs, property paths,
//! EXISTS, SERVICE, CONSTRUCT/DESCRIBE, CSV/TTL result files) are skipped.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use oxrdf::{NamedOrBlankNode, Triple};
use oxrdfio::{RdfFormat, RdfParser};
use sparesults::{QueryResultsFormat, QueryResultsParser, SliceQueryResultsParserOutput};
use sq_core::eval_api::TermResolver;
use sq_core::ir::{AggFunc, AggSpec};
use sq_core::spargebra::algebra::{AggregateExpression, AggregateFunction, Expression, GraphPattern, OrderExpression};
use sq_core::spargebra::term::{NamedNodePattern, TermPattern};
use sq_core::spargebra::{Query, SparqlParser};
use sq_core::{Slot, Term, TermId};

use super::harness::{Lowering, compact, term_of};
use crate::testing::TestEnv;
use crate::{CompiledExpr, accumulator, compile};

const DEFAULT_DIR: &str = "/tmp/claude-0/-home-user-research/82148577-c93b-5a93-8757-e7fe894ecb5e/scratchpad/w3c/rdf-tests/sparql";

const DIRS: &[&str] = &[
    "sparql10/expr-builtin",
    "sparql10/expr-ops",
    "sparql10/expr-equals",
    "sparql10/open-world",
    "sparql10/type-promotion",
    "sparql10/boolean-effective-value",
    "sparql10/bound",
    "sparql10/cast",
    "sparql10/regex",
    "sparql10/sort",
    "sparql11/functions",
    "sparql11/cast",
    "sparql11/aggregates",
    "sparql11/bind",
    "sparql11/grouping",
    "sparql11/project-expression",
];

/// Tests whose expected results deliberately differ from this crate's documented semantics
/// (DECISIONS.md / §6), with the reason.
const KNOWN_DIVERGENCES: &[(&str, &str)] = &[];

// ---------------------------------------------------------------- the reference evaluator

#[derive(Copy, Clone)]
enum Pt {
    Slot(Slot),
    Const(TermId),
}

enum Plan {
    Bgp(Vec<[Pt; 3]>),
    Join(Box<Plan>, Box<Plan>),
    LeftJoin(Box<Plan>, Box<Plan>, Option<CompiledExpr>),
    Filter(CompiledExpr, Box<Plan>),
    Union(Box<Plan>, Box<Plan>),
    Extend(Box<Plan>, Slot, CompiledExpr),
    Minus(Box<Plan>, Box<Plan>),
    Values(Vec<Slot>, Vec<Vec<TermId>>),
    OrderBy(Box<Plan>, Vec<(CompiledExpr, bool)>),
    Project(Box<Plan>, Vec<Slot>),
    Distinct(Box<Plan>),
    Slice(Box<Plan>, usize, Option<usize>),
    Group(Box<Plan>, Vec<Slot>, Vec<(Slot, AggSpec, Option<CompiledExpr>)>),
}

struct Lower<'a> {
    env: &'a mut TestEnv,
    low: Lowering,
}

impl Lower<'_> {
    fn expr(&mut self, e: &Expression) -> Result<CompiledExpr, String> {
        let e = self.low.lower(self.env, e);
        if !self.low.exists.is_empty() {
            return Err("EXISTS".into());
        }
        compile(&e, &self.env.resolver.temps).map_err(|e| e.to_string())
    }

    fn term(&mut self, t: &TermPattern) -> Result<Pt, String> {
        Ok(match t {
            TermPattern::Variable(v) => Pt::Slot(self.low.slot(v.as_str())),
            TermPattern::BlankNode(b) => Pt::Slot(self.low.slot(&format!("_:{}", b.as_str()))),
            TermPattern::NamedNode(n) => Pt::Const(Lowering::constant(self.env, Term::Iri(n.as_str().into()))),
            TermPattern::Literal(l) => Pt::Const(Lowering::constant(self.env, term_of(l.as_ref().into()))),
        })
    }

    fn plan(&mut self, p: &GraphPattern) -> Result<Plan, String> {
        Ok(match p {
            GraphPattern::Bgp { patterns } => {
                let mut out = vec![];
                for t in patterns {
                    let p = match &t.predicate {
                        NamedNodePattern::NamedNode(n) => Pt::Const(Lowering::constant(self.env, Term::Iri(n.as_str().into()))),
                        NamedNodePattern::Variable(v) => Pt::Slot(self.low.slot(v.as_str())),
                    };
                    out.push([self.term(&t.subject)?, p, self.term(&t.object)?]);
                }
                Plan::Bgp(out)
            }
            GraphPattern::Join { left, right } => Plan::Join(Box::new(self.plan(left)?), Box::new(self.plan(right)?)),
            GraphPattern::LeftJoin { left, right, expression } => Plan::LeftJoin(
                Box::new(self.plan(left)?),
                Box::new(self.plan(right)?),
                expression.as_ref().map(|e| self.expr(e)).transpose()?,
            ),
            GraphPattern::Filter { expr, inner } => Plan::Filter(self.expr(expr)?, Box::new(self.plan(inner)?)),
            GraphPattern::Union { left, right } => Plan::Union(Box::new(self.plan(left)?), Box::new(self.plan(right)?)),
            GraphPattern::Extend { inner, variable, expression } => {
                let inner = self.plan(inner)?;
                Plan::Extend(Box::new(inner), self.low.slot(variable.as_str()), self.expr(expression)?)
            }
            GraphPattern::Minus { left, right } => Plan::Minus(Box::new(self.plan(left)?), Box::new(self.plan(right)?)),
            GraphPattern::Values { variables, bindings } => {
                let slots = variables.iter().map(|v| self.low.slot(v.as_str())).collect();
                let rows = bindings
                    .iter()
                    .map(|row| {
                        row.iter()
                            .map(|t| match t {
                                None => TermId::UNBOUND,
                                Some(g) => {
                                    let t = Term::from_oxrdf(oxrdf::Term::from(g.clone()).as_ref());
                                    Lowering::constant(self.env, t)
                                }
                            })
                            .collect()
                    })
                    .collect();
                Plan::Values(slots, rows)
            }
            GraphPattern::OrderBy { inner, expression } => {
                let inner = self.plan(inner)?;
                let keys = expression
                    .iter()
                    .map(|o| match o {
                        OrderExpression::Asc(e) => self.expr(e).map(|c| (c, false)),
                        OrderExpression::Desc(e) => self.expr(e).map(|c| (c, true)),
                    })
                    .collect::<Result<_, _>>()?;
                Plan::OrderBy(Box::new(inner), keys)
            }
            GraphPattern::Project { inner, variables } => {
                let inner = self.plan(inner)?;
                Plan::Project(Box::new(inner), variables.iter().map(|v| self.low.slot(v.as_str())).collect())
            }
            GraphPattern::Distinct { inner } | GraphPattern::Reduced { inner } => Plan::Distinct(Box::new(self.plan(inner)?)),
            GraphPattern::Slice { inner, start, length } => Plan::Slice(Box::new(self.plan(inner)?), *start, *length),
            GraphPattern::Group { inner, variables, aggregates } => {
                let inner = self.plan(inner)?;
                let keys = variables.iter().map(|v| self.low.slot(v.as_str())).collect();
                let mut aggs = vec![];
                for (v, a) in aggregates {
                    let slot = self.low.slot(v.as_str());
                    let (func, distinct, arg) = match a {
                        AggregateExpression::CountSolutions { distinct } => (AggFunc::CountAll, *distinct, None),
                        AggregateExpression::FunctionCall { name, expr, distinct } => {
                            let func = match name {
                                AggregateFunction::Count => AggFunc::Count,
                                AggregateFunction::Sum => AggFunc::Sum,
                                AggregateFunction::Avg => AggFunc::Avg,
                                AggregateFunction::Min => AggFunc::Min,
                                AggregateFunction::Max => AggFunc::Max,
                                AggregateFunction::Sample => AggFunc::Sample,
                                AggregateFunction::GroupConcat { separator } => {
                                    AggFunc::GroupConcat { separator: separator.clone().unwrap_or_else(|| " ".into()) }
                                }
                                AggregateFunction::Custom(n) => AggFunc::Custom(n.as_str().into()),
                            };
                            (func, *distinct, Some(self.expr(expr)?))
                        }
                    };
                    aggs.push((slot, AggSpec { func, distinct, arg: None }, arg));
                }
                Plan::Group(Box::new(inner), keys, aggs)
            }
            other => return Err(format!("unsupported pattern: {}", other.to_string().chars().take(40).collect::<String>())),
        })
    }
}

type Row = Vec<TermId>;

fn compatible(a: &[TermId], b: &[TermId]) -> bool {
    a.iter().zip(b).all(|(x, y)| !x.is_bound() || !y.is_bound() || x == y)
}

fn merge(a: &[TermId], b: &[TermId]) -> Row {
    a.iter().zip(b).map(|(x, y)| if x.is_bound() { *x } else { *y }).collect()
}

struct Eval<'a> {
    env: &'a mut TestEnv,
    triples: &'a [[TermId; 3]],
    width: usize,
    /// Slots of user variables (blank-node pattern variables are hidden).
    visible: Vec<Slot>,
}

impl Eval<'_> {
    fn ebv(&mut self, e: &CompiledExpr, row: &[TermId]) -> bool {
        self.env.with(|cx| e.ebv(row, cx)).expect("fatal") == Some(true)
    }

    fn run(&mut self, p: &Plan) -> Vec<Row> {
        match p {
            Plan::Bgp(pats) => {
                let mut rows = vec![vec![TermId::UNBOUND; self.width]];
                for pat in pats {
                    let mut next = vec![];
                    for row in &rows {
                        'triples: for t in self.triples {
                            let mut r = row.clone();
                            for (pt, v) in pat.iter().zip(t) {
                                match pt {
                                    Pt::Const(c) if c != v => continue 'triples,
                                    Pt::Const(_) => {}
                                    Pt::Slot(s) => {
                                        let cur = &mut r[*s as usize];
                                        if cur.is_bound() && cur != v {
                                            continue 'triples;
                                        }
                                        *cur = *v;
                                    }
                                }
                            }
                            next.push(r);
                        }
                    }
                    rows = next;
                }
                rows
            }
            Plan::Join(a, b) => {
                let (l, r) = (self.run(a), self.run(b));
                let mut out = vec![];
                for x in &l {
                    for y in &r {
                        if compatible(x, y) {
                            out.push(merge(x, y));
                        }
                    }
                }
                out
            }
            Plan::LeftJoin(a, b, f) => {
                let (l, r) = (self.run(a), self.run(b));
                let mut out = vec![];
                for x in &l {
                    let mut matched = false;
                    for y in &r {
                        if compatible(x, y) {
                            let m = merge(x, y);
                            if f.as_ref().is_none_or(|f| self.ebv(f, &m)) {
                                out.push(m);
                                matched = true;
                            }
                        }
                    }
                    if !matched {
                        out.push(x.clone());
                    }
                }
                out
            }
            Plan::Filter(f, inner) => {
                let rows = self.run(inner);
                rows.into_iter().filter(|r| self.ebv(f, r)).collect()
            }
            Plan::Union(a, b) => {
                let mut l = self.run(a);
                l.extend(self.run(b));
                l
            }
            Plan::Extend(inner, slot, e) => {
                let mut rows = self.run(inner);
                for r in &mut rows {
                    if let Some(id) = self.env.with(|cx| e.eval(r, cx, true)).expect("fatal") {
                        r[*slot as usize] = id;
                    }
                }
                rows
            }
            Plan::Minus(a, b) => {
                let (l, r) = (self.run(a), self.run(b));
                l.into_iter()
                    .filter(|x| {
                        !r.iter().any(|y| compatible(x, y) && x.iter().zip(y).any(|(p, q)| p.is_bound() && q.is_bound()))
                    })
                    .collect()
            }
            Plan::Values(slots, rows) => rows
                .iter()
                .map(|vals| {
                    let mut r = vec![TermId::UNBOUND; self.width];
                    for (s, v) in slots.iter().zip(vals) {
                        r[*s as usize] = *v;
                    }
                    r
                })
                .collect(),
            Plan::OrderBy(inner, keys) => {
                let rows = self.run(inner);
                let mut keyed: Vec<(Vec<crate::SortKey>, Row)> = rows
                    .into_iter()
                    .map(|r| {
                        let k = keys.iter().map(|(e, _)| self.env.with(|cx| e.sort_key(&r, cx)).expect("fatal")).collect();
                        (k, r)
                    })
                    .collect();
                keyed.sort_by(|(a, _), (b, _)| {
                    for (i, (_, desc)) in keys.iter().enumerate() {
                        let o = a[i].cmp(&b[i]);
                        let o = if *desc { o.reverse() } else { o };
                        if o.is_ne() {
                            return o;
                        }
                    }
                    std::cmp::Ordering::Equal
                });
                keyed.into_iter().map(|(_, r)| r).collect()
            }
            Plan::Project(inner, slots) => {
                let mut rows = self.run(inner);
                for r in &mut rows {
                    for (i, v) in r.iter_mut().enumerate() {
                        if !slots.contains(&(i as Slot)) {
                            *v = TermId::UNBOUND;
                        }
                    }
                }
                rows
            }
            Plan::Distinct(inner) => {
                let mut seen = std::collections::HashSet::new();
                self.run(inner).into_iter().filter(|r| seen.insert(r.clone())).collect()
            }
            Plan::Slice(inner, start, len) => {
                let rows = self.run(inner).into_iter().skip(*start);
                match len {
                    Some(n) => rows.take(*n).collect(),
                    None => rows.collect(),
                }
            }
            Plan::Group(inner, keys, aggs) => {
                let rows = self.run(inner);
                let mut groups: Vec<(Vec<TermId>, Vec<Row>)> = vec![];
                let mut index: HashMap<Vec<TermId>, usize> = HashMap::new();
                for r in rows {
                    let k: Vec<TermId> = keys.iter().map(|s| r[*s as usize]).collect();
                    let i = *index.entry(k.clone()).or_insert_with(|| {
                        groups.push((k, vec![]));
                        groups.len() - 1
                    });
                    groups[i].1.push(r);
                }
                if groups.is_empty() && keys.is_empty() {
                    groups.push((vec![], vec![]));
                }
                let mut out = vec![];
                for (k, members) in groups {
                    let mut row = vec![TermId::UNBOUND; self.width];
                    for (s, v) in keys.iter().zip(&k) {
                        row[*s as usize] = *v;
                    }
                    for (slot, spec, arg) in aggs {
                        let mut acc = accumulator(spec);
                        let v = self
                            .env
                            .with(|cx| {
                                for m in &members {
                                    match arg {
                                        None => acc.add_row(m, &self.visible, cx)?,
                                        Some(e) => {
                                            let v = e.eval(m, cx, true)?;
                                            acc.add(v, cx)?
                                        }
                                    }
                                }
                                acc.finish(cx)
                            })
                            .expect("fatal");
                        if let Some(v) = v {
                            row[*slot as usize] = v;
                        }
                    }
                    out.push(row);
                }
                out
            }
        }
    }
}

// ---------------------------------------------------------------- manifests and results

const MF: &str = "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#";
const QT: &str = "http://www.w3.org/2001/sw/DataAccess/tests/test-query#";

struct TestCase {
    name: String,
    query: PathBuf,
    data: Vec<PathBuf>,
    result: Option<PathBuf>,
    named_graphs: bool,
}

fn file_url(p: &Path) -> String {
    format!("file://{}", p.display())
}

fn url_path(u: &str) -> PathBuf {
    PathBuf::from(u.strip_prefix("file://").unwrap_or(u))
}

/// Parses a Turtle or RDF/XML file (by extension) into triples.
fn parse_rdf(path: &Path) -> Option<Vec<Triple>> {
    let format = match path.extension()?.to_str()? {
        "ttl" | "nt" => RdfFormat::Turtle,
        "rdf" => RdfFormat::RdfXml,
        _ => return None,
    };
    let bytes = std::fs::read(path).ok()?;
    let parser = RdfParser::from_format(format).with_base_iri(file_url(path)).ok()?;
    parser.for_slice(&bytes).map(|q| q.ok().map(Triple::from)).collect()
}

/// Objects of (s, p) in a triple list.
fn objects<'a>(triples: &'a [Triple], s: &NamedOrBlankNode, p: &str) -> std::vec::IntoIter<&'a oxrdf::Term> {
    let found: Vec<_> = triples.iter().filter(|t| &t.subject == s && t.predicate.as_str() == p).map(|t| &t.object).collect();
    found.into_iter()
}

fn as_subject(t: &oxrdf::Term) -> Option<NamedOrBlankNode> {
    match t {
        oxrdf::Term::BlankNode(b) => Some(b.clone().into()),
        oxrdf::Term::NamedNode(n) => Some(n.clone().into()),
        _ => None,
    }
}

fn read_manifest(dir: &Path) -> Vec<TestCase> {
    let Some(triples) = parse_rdf(&dir.join("manifest.ttl")) else { return vec![] };
    let iri = |t: &oxrdf::Term| match t {
        oxrdf::Term::NamedNode(n) => Some(url_path(n.as_str())),
        _ => None,
    };
    let mut out = vec![];
    for t in &triples {
        let is_eval = t.predicate.as_str() == RDF_TYPE
            && matches!(&t.object, oxrdf::Term::NamedNode(n) if n.as_str() == format!("{MF}QueryEvaluationTest"));
        if !is_eval {
            continue;
        }
        let name = match &t.subject {
            NamedOrBlankNode::NamedNode(n) => n.as_str().rsplit(['#', '/']).next().unwrap_or("").to_string(),
            NamedOrBlankNode::BlankNode(_) => continue,
        };
        let mf = |p: &str| format!("{MF}{p}");
        let qt = |p: &str| format!("{QT}{p}");
        let Some(action) = objects(&triples, &t.subject, &mf("action")).next().and_then(as_subject) else { continue };
        let Some(query) = objects(&triples, &action, &qt("query")).next().and_then(iri) else { continue };
        out.push(TestCase {
            name,
            query,
            data: objects(&triples, &action, &qt("data")).filter_map(iri).collect(),
            result: objects(&triples, &t.subject, &mf("result")).next().and_then(iri),
            named_graphs: objects(&triples, &action, &qt("graphData")).next().is_some(),
        });
    }
    out
}

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const RS: &str = "http://www.w3.org/2001/sw/DataAccess/tests/result-set#";

/// Reads a DAWG result-set graph (`rs:ResultSet`, Turtle or RDF/XML). Solutions with
/// `rs:index` are returned in index order and flagged as ordered.
fn read_result_set(path: &Path) -> Option<Expected> {
    let triples = parse_rdf(path)?;
    let rs = |p: &str| format!("{RS}{p}");
    let set = triples
        .iter()
        .find(|t| t.predicate.as_str() == RDF_TYPE && matches!(&t.object, oxrdf::Term::NamedNode(n) if n.as_str() == rs("ResultSet")))?
        .subject
        .clone();
    if let Some(oxrdf::Term::Literal(b)) = objects(&triples, &set, &rs("boolean")).next() {
        return Some(Expected::Boolean(b.value() == "true"));
    }
    let mut rows = vec![];
    let mut ordered = false;
    for sol in objects(&triples, &set, &rs("solution")).filter_map(as_subject) {
        let index = objects(&triples, &sol, &rs("index")).next().and_then(|i| match i {
            oxrdf::Term::Literal(l) => l.value().parse::<u64>().ok(),
            _ => None,
        });
        ordered |= index.is_some();
        let mut row = vec![];
        for b in objects(&triples, &sol, &rs("binding")).filter_map(as_subject) {
            let var = objects(&triples, &b, &rs("variable")).next()?;
            let value = objects(&triples, &b, &rs("value")).next()?;
            let oxrdf::Term::Literal(var) = var else { return None };
            row.push((var.value().to_string(), Term::from_oxrdf(value.as_ref())));
        }
        rows.push((index.unwrap_or(0), row));
    }
    rows.sort_by_key(|(i, _)| *i);
    Some(Expected::Solutions(rows.into_iter().map(|(_, r)| r).collect(), ordered))
}

/// A solution as sorted (variable, rendered term) pairs.
type Solution = Vec<(String, String)>;

fn render(t: &Term, lenient: bool) -> String {
    match t {
        Term::BNode(_) => "_:".into(),
        Term::Literal(l) if lenient => {
            let v = crate::Value::from_literal(l);
            match v.as_num() {
                Some(n) if !matches!(v, crate::Value::IllTyped { .. }) => format!("{}^^{}", n.to_f64(), l.datatype),
                _ => compact(t),
            }
        }
        t => compact(t),
    }
}

fn normalize(sols: &[Vec<(String, Term)>], lenient: bool) -> Vec<Solution> {
    let mut out: Vec<Solution> = sols
        .iter()
        .map(|s| {
            let mut s: Solution = s.iter().map(|(v, t)| (v.clone(), render(t, lenient))).collect();
            s.sort();
            s
        })
        .collect();
    out.sort();
    out
}

enum Expected {
    /// Rows, and whether their order is significant (ordered result sets).
    Solutions(Vec<Vec<(String, Term)>>, bool),
    Boolean(bool),
}

fn read_expected(path: &Path) -> Option<Expected> {
    let format = match path.extension()?.to_str()? {
        "srx" => QueryResultsFormat::Xml,
        "srj" => QueryResultsFormat::Json,
        "tsv" => QueryResultsFormat::Tsv,
        "ttl" | "rdf" => return read_result_set(path),
        _ => return None,
    };
    let bytes = std::fs::read(path).ok()?;
    match QueryResultsParser::from_format(format).for_slice(&bytes).ok()? {
        SliceQueryResultsParserOutput::Boolean(b) => Some(Expected::Boolean(b)),
        SliceQueryResultsParserOutput::Solutions(s) => {
            let mut rows = vec![];
            for sol in s {
                let sol = sol.ok()?;
                rows.push(sol.iter().map(|(v, t)| (v.as_str().to_string(), Term::from_oxrdf(t.as_ref()))).collect());
            }
            Some(Expected::Solutions(rows, false))
        }
    }
}

#[derive(Debug, PartialEq)]
enum Outcome {
    Pass,
    PassLenient,
    Fail(String),
    Skip(String),
}

fn run_test(tc: &TestCase) -> Outcome {
    if tc.named_graphs {
        return Outcome::Skip("named graphs".into());
    }
    let Some(result) = &tc.result else { return Outcome::Skip("no result file".into()) };
    let Some(expected) = read_expected(result) else { return Outcome::Skip("result format".into()) };
    let mut env = TestEnv::new();
    let mut triples = vec![];
    for d in &tc.data {
        let Some(data) = parse_rdf(d) else { return Outcome::Skip("data file".into()) };
        for t in data {
            let s = env.resolver.add(Term::from_oxrdf(t.subject.as_ref().into()));
            let p = env.resolver.add(Term::Iri(t.predicate.as_str().into()));
            let o = env.resolver.add(Term::from_oxrdf(t.object.as_ref()));
            triples.push([s, p, o]);
        }
    }
    let text = std::fs::read_to_string(&tc.query).unwrap_or_default();
    let query = match SparqlParser::new().with_base_iri(file_url(&tc.query)).expect("base").parse_query(&text) {
        Ok(q) => q,
        Err(e) => return Outcome::Skip(format!("parse: {e}")),
    };
    let (pattern, ask) = match &query {
        Query::Select { pattern, dataset: None, .. } => (pattern, false),
        Query::Ask { pattern, dataset: None, .. } => (pattern, true),
        _ => return Outcome::Skip("query form / dataset".into()),
    };
    env.base = match &query {
        Query::Select { base_iri, .. } | Query::Ask { base_iri, .. } => base_iri.as_ref().map(|b| b.as_str().to_string()),
        _ => None,
    };
    let mut lower = Lower { env: &mut env, low: Lowering::default() };
    let plan = match lower.plan(pattern) {
        Ok(p) => p,
        Err(e) => return Outcome::Skip(e),
    };
    let vars = lower.low.vars.clone();
    let visible = (0..vars.len()).filter(|i| !vars[*i].starts_with("_:")).map(|i| i as Slot).collect();
    let mut ev = Eval { env: &mut env, triples: &triples, width: vars.len().max(1), visible };
    let rows = ev.run(&plan);
    match expected {
        Expected::Boolean(b) => {
            if ask && (!rows.is_empty()) == b {
                Outcome::Pass
            } else {
                Outcome::Fail(format!("ASK: expected {b}"))
            }
        }
        Expected::Solutions(exp, ordered) => {
            let mut actual = vec![];
            for r in &rows {
                let mut sol = vec![];
                for (i, id) in r.iter().enumerate() {
                    if id.is_bound() && !vars[i].starts_with("_:") {
                        sol.push((vars[i].clone(), (*env.resolver.term(*id).expect("decode")).clone()));
                    }
                }
                actual.push(sol);
            }
            let in_order = |rows: &[Vec<(String, Term)>]| -> Vec<Solution> {
                rows.iter()
                    .map(|s| {
                        let mut s: Solution = s.iter().map(|(v, t)| (v.clone(), render(t, true))).collect();
                        s.sort();
                        s
                    })
                    .collect()
            };
            if ordered && normalize(&actual, true) == normalize(&exp, true) && in_order(&actual) != in_order(&exp) {
                Outcome::Fail(format!("\n    order: expected {:?}\n    actual {:?}", in_order(&exp), in_order(&actual)))
            } else if normalize(&actual, false) == normalize(&exp, false) {
                Outcome::Pass
            } else if normalize(&actual, true) == normalize(&exp, true) {
                Outcome::PassLenient
            } else {
                Outcome::Fail(format!(
                    "\n    expected {:?}\n    actual   {:?}",
                    normalize(&exp, false),
                    normalize(&actual, false)
                ))
            }
        }
    }
}

#[test]
fn w3c_expression_tests() {
    let root = PathBuf::from(std::env::var("SQ_W3C_DIR").unwrap_or_else(|_| DEFAULT_DIR.into()));
    if !root.join("sparql11/functions/manifest.ttl").exists() {
        eprintln!("W3C test suite not found at {} (set SQ_W3C_DIR); skipping", root.display());
        return;
    }
    let (mut pass, mut lenient, mut skipped, mut failures, mut diverged) = (0, 0, vec![], vec![], vec![]);
    for dir in DIRS {
        for tc in read_manifest(&root.join(dir)) {
            match run_test(&tc) {
                Outcome::Pass => pass += 1,
                Outcome::PassLenient => {
                    lenient += 1;
                    eprintln!("  pass-lenient {dir}/{}", tc.name);
                }
                Outcome::Skip(why) => skipped.push(format!("{dir}/{}: {why}", tc.name)),
                Outcome::Fail(msg) => {
                    if KNOWN_DIVERGENCES.iter().any(|(n, _)| *n == tc.name) {
                        diverged.push(tc.name.clone());
                    } else {
                        failures.push(format!("{dir}/{}: {msg}", tc.name));
                    }
                }
            }
        }
    }
    eprintln!("W3C expression tests: {pass} pass, {lenient} pass-lenient, {} skipped, {} unexpected failures", skipped.len(), failures.len());
    for s in &skipped {
        eprintln!("  skipped {s}");
    }
    assert!(failures.is_empty(), "unexpected failures:\n{}", failures.join("\n"));
    for (name, why) in KNOWN_DIVERGENCES {
        assert!(diverged.iter().any(|d| d == name), "{name} no longer diverges ({why}): update KNOWN_DIVERGENCES");
    }
    assert!(pass + lenient >= 290, "too few tests ran: {}", pass + lenient);
}
