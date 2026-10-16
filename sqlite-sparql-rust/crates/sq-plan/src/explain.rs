//! EXPLAIN output (spec §7.6): readable pre-order trees of the logical IR and of physical
//! plans (`#id op details  est= cost=`, plus `rows= opens= seeks= ms=` with a profile).
//! A SqlJoin prints its aliases (`t1:Gpso(s=t0.s,p=:age)`: index order and the bound columns
//! of its prefix, the default graph omitted) and a Count the fast path it takes.
//!
//! Constants are rendered without I/O by default (inline values, TEMP terms, well-known IRIs;
//! other dictionary ids as `<#id>`); the `_with` variants decode them through a dictionary.
use std::fmt::Write;

use sq_core::batch::Slot;
use sq_core::codec::{self, WELL_KNOWN};
use sq_core::eval_api::TempDict;
use sq_core::id::Tag;
use sq_core::ir::{
    AggFunc, AggSpec, DatasetSpec, DefaultGraph, Expr, GraphSel, Logical, LoweredQuery, NamedGraphs, OrderKey,
    PatTerm, PathPlan, QueryForm, QuadPat, TemplateTerm, VarOrigin, VarTable,
};
use sq_core::plan::{ExecProfile, JoinKind, Op, Plan, QueryPlan};
use sq_core::store::{Col, Dictionary};
use sq_core::term::Term;
use sq_core::TermId;

use crate::physical::sqljoin::{self, Operand};
use crate::physical::{count_fast_path, FastCount};

/// What `sparql_explain` prints.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ExplainMode {
    /// spargebra's algebra (SSE), printed by the caller.
    Algebra,
    /// The lowered logical IR ([`explain_logical`]).
    Logical,
    /// The physical plan with estimates ([`explain`]).
    Plan,
    /// The physical plan with runtime counters ([`explain`] with a profile).
    Analyze,
}

/// Column at which estimates start in physical plan lines.
const EST_COLUMN: usize = 52;

const PREFIXES: &[(&str, &str)] = &[
    ("rdf:", "http://www.w3.org/1999/02/22-rdf-syntax-ns#"),
    ("rdfs:", "http://www.w3.org/2000/01/rdf-schema#"),
    ("xsd:", "http://www.w3.org/2001/XMLSchema#"),
    ("owl:", "http://www.w3.org/2002/07/owl#"),
];

/// The lowered query as an indented tree (form, dataset, pattern, EXISTS patterns).
pub fn explain_logical(q: &LoweredQuery) -> String {
    explain_logical_with(q, None)
}

/// [`explain_logical`], decoding dictionary constants through `dict`.
pub fn explain_logical_with(q: &LoweredQuery, dict: Option<&dyn Dictionary>) -> String {
    let r = Render { vars: &q.vars, temps: &q.temps, dict, dataset: None };
    let mut out = String::new();
    let _ = writeln!(out, "{}", r.form(&q.form));
    let _ = writeln!(out, "Dataset {}", dataset(&q.dataset, &r));
    r.logical(&q.root, 0, &mut out);
    for (i, e) in q.exists.iter().enumerate() {
        let _ = writeln!(out, "EXISTS #{i}");
        r.logical(e, 1, &mut out);
    }
    out
}

/// The physical plan as pre-order lines; `profile` adds the EXPLAIN ANALYZE counters.
pub fn explain(p: &QueryPlan, profile: Option<&ExecProfile>) -> String {
    explain_with(p, profile, None)
}

/// [`explain`], decoding dictionary constants through `dict`.
pub fn explain_with(p: &QueryPlan, profile: Option<&ExecProfile>, dict: Option<&dyn Dictionary>) -> String {
    let r = Render { vars: &p.vars, temps: &p.temps, dict, dataset: Some(&p.dataset) };
    let mut out = String::new();
    r.plan(&p.root, 0, profile, &mut out);
    let used = referenced_exists(p);
    for (i, e) in p.exists.iter().enumerate() {
        let note = if used.contains(&(i as u32)) { "" } else { " (unused: decorrelated or eliminated)" };
        let _ = writeln!(out, "EXISTS #{i}{note}");
        r.plan(e, 1, profile, &mut out);
    }
    out
}

/// EXISTS ids referenced by expressions of the root plan or of referenced EXISTS plans.
fn referenced_exists(p: &QueryPlan) -> Vec<u32> {
    let mut used: Vec<u32> = Vec::new();
    let mut stack: Vec<&Plan> = vec![&p.root];
    while let Some(node) = stack.pop() {
        for e in crate::physical::node_exprs(node) {
            for id in crate::expr::exists_ids(e) {
                if !used.contains(&id.0) {
                    used.push(id.0);
                    if let Some(plan) = p.exists.get(id.0 as usize) {
                        stack.push(plan);
                    }
                }
            }
        }
        stack.extend(node.children());
    }
    used
}

fn dataset(d: &DatasetSpec, r: &Render<'_>) -> String {
    let list = |ids: &[TermId]| ids.iter().map(|g| r.term(*g)).collect::<Vec<_>>().join(" ");
    let default = match &d.default {
        DefaultGraph::StoreDefault => "store-default".to_owned(),
        DefaultGraph::UnionAll => "union-all".to_owned(),
        DefaultGraph::Graphs(g) => format!("[{}]", list(g)),
    };
    let named = match &d.named {
        NamedGraphs::All => "all".to_owned(),
        NamedGraphs::Only(g) => format!("[{}]", list(g)),
    };
    format!("default={default} named={named}")
}

/// `2.0e4` style for large numbers, plain for small ones.
fn fmt_est(x: f64) -> String {
    if x < 1000.0 {
        if (x - x.round()).abs() < 0.05 { format!("{}", x.round() as i64) } else { format!("{x:.1}") }
    } else {
        format!("{x:.1e}")
    }
}

fn fmt_ns(ns: f64) -> String {
    if ns < 1e3 {
        format!("{ns:.0}ns")
    } else if ns < 1e6 {
        format!("{:.1}us", ns / 1e3)
    } else if ns < 1e9 {
        format!("{:.1}ms", ns / 1e6)
    } else {
        format!("{:.2}s", ns / 1e9)
    }
}

fn iri(s: &str) -> String {
    for (p, ns) in PREFIXES {
        if let Some(local) = s.strip_prefix(ns)
            && !local.is_empty()
            && local.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return format!("{p}{local}");
        }
    }
    format!("<{s}>")
}

fn term_text(t: &Term) -> String {
    match t {
        Term::Iri(i) => iri(i),
        Term::BNode(b) => format!("_:{b}"),
        Term::Literal(l) => {
            let quoted = format!("{:?}", &*l.lex);
            if let Some(lang) = &l.lang {
                format!("{quoted}@{lang}")
            } else if l.is_simple() {
                quoted
            } else {
                let dt = &*l.datatype;
                let bare = dt == codec::XSD_INTEGER_IRI || dt == codec::XSD_DECIMAL_IRI || dt == codec::XSD_BOOLEAN_IRI;
                if bare { l.lex.to_string() } else { format!("{quoted}^^{}", iri(dt)) }
            }
        }
    }
}

fn looks_random(label: &str) -> bool {
    label.len() >= 16 && label.bytes().all(|b| b.is_ascii_hexdigit())
}

struct Render<'a> {
    vars: &'a VarTable,
    temps: &'a TempDict,
    dict: Option<&'a dyn Dictionary>,
    /// The dataset of a physical plan (Count fast paths depend on it).
    dataset: Option<&'a DatasetSpec>,
}

impl Render<'_> {
    fn slot(&self, s: Slot) -> String {
        let Some((name, origin)) = self.vars.vars.get(s as usize) else { return format!("?#{s}") };
        match origin {
            VarOrigin::User => format!("?{name}"),
            VarOrigin::Renamed => format!("?{name}.{s}"),
            VarOrigin::Generated => format!("?_{s}"),
            VarOrigin::BlankNode => {
                let label = name.strip_prefix("_:").unwrap_or(name);
                if looks_random(label) { format!("_:b{s}") } else { format!("_:{label}") }
            }
        }
    }

    fn slots(&self, slots: impl IntoIterator<Item = Slot>) -> String {
        format!("[{}]", slots.into_iter().map(|s| self.slot(s)).collect::<Vec<_>>().join(" "))
    }

    fn term(&self, id: TermId) -> String {
        if !id.is_bound() {
            return "UNDEF".to_owned();
        }
        if id == TermId::ERROR {
            return "ERROR".to_owned();
        }
        if id.is_temp() {
            return self.temps.get(id).map_or_else(|| format!("<temp#{}>", id.raw()), |t| term_text(t));
        }
        if let Some(t) = codec::decode_inline(id) {
            return term_text(&t);
        }
        if id.tag() == Tag::BNode {
            return format!("_:{}", codec::bnode_label(id));
        }
        if let Some(t) = self.dict.and_then(|d| d.decode_one(id).ok()) {
            return term_text(&t);
        }
        if id.tag() == Tag::Iri
            && let Some((_, s)) = WELL_KNOWN.iter().find(|(row, _)| *row == id.dict_row())
        {
            return iri(s);
        }
        format!("<#{}>", id.raw())
    }

    fn pat_term(&self, t: PatTerm) -> String {
        match t {
            PatTerm::Slot(s) => self.slot(s),
            PatTerm::Const(c) => self.term(c),
        }
    }

    fn graph(&self, g: GraphSel) -> String {
        match g {
            GraphSel::Default => String::new(),
            GraphSel::Active => "GRAPH active ".to_owned(),
            GraphSel::Const(c) => format!("GRAPH {} ", self.term(c)),
            GraphSel::Slot(s) => format!("GRAPH {} ", self.slot(s)),
        }
    }

    fn quad(&self, q: &QuadPat) -> String {
        format!("{}{} {} {}", self.graph(q.g), self.pat_term(q.s), self.pat_term(q.p), self.pat_term(q.o))
    }

    fn path(&self, p: &PathPlan) -> String {
        match p {
            PathPlan::Pred(id) => self.term(*id),
            PathPlan::Inv(p) => format!("^{}", self.path_atom(p)),
            PathPlan::Seq(a, b) => format!("{}/{}", self.path_atom(a), self.path_atom(b)),
            PathPlan::Alt(a, b) => format!("{}|{}", self.path_atom(a), self.path_atom(b)),
            PathPlan::ZeroOrMore(p) => format!("{}*", self.path_atom(p)),
            PathPlan::OneOrMore(p) => format!("{}+", self.path_atom(p)),
            PathPlan::ZeroOrOne(p) => format!("{}?", self.path_atom(p)),
            PathPlan::Nps(ids) => format!("!({})", ids.iter().map(|i| self.term(*i)).collect::<Vec<_>>().join("|")),
        }
    }

    fn path_atom(&self, p: &PathPlan) -> String {
        match p {
            PathPlan::Pred(_) | PathPlan::Nps(_) => self.path(p),
            _ => format!("({})", self.path(p)),
        }
    }

    fn expr(&self, e: &Expr) -> String {
        let bin = |op: &str, a: &Expr, b: &Expr| format!("({} {op} {})", self.expr(a), self.expr(b));
        let list = |l: &[Expr]| l.iter().map(|x| self.expr(x)).collect::<Vec<_>>().join(", ");
        match e {
            Expr::Const(c) => self.term(*c),
            Expr::Slot(s) => self.slot(*s),
            Expr::Or(a, b) => bin("||", a, b),
            Expr::And(a, b) => bin("&&", a, b),
            Expr::Not(a) => format!("!{}", self.expr(a)),
            Expr::Equal(a, b) => bin("=", a, b),
            Expr::SameTerm(a, b) => format!("sameTerm({}, {})", self.expr(a), self.expr(b)),
            Expr::Greater(a, b) => bin(">", a, b),
            Expr::GreaterOrEqual(a, b) => bin(">=", a, b),
            Expr::Less(a, b) => bin("<", a, b),
            Expr::LessOrEqual(a, b) => bin("<=", a, b),
            Expr::In(a, l) => format!("({} IN ({}))", self.expr(a), list(l)),
            Expr::Add(a, b) => bin("+", a, b),
            Expr::Subtract(a, b) => bin("-", a, b),
            Expr::Multiply(a, b) => bin("*", a, b),
            Expr::Divide(a, b) => bin("/", a, b),
            Expr::UnaryPlus(a) => format!("+{}", self.expr(a)),
            Expr::UnaryMinus(a) => format!("-{}", self.expr(a)),
            Expr::Bound(s) => format!("bound({})", self.slot(*s)),
            Expr::If(a, b, c) => format!("if({}, {}, {})", self.expr(a), self.expr(b), self.expr(c)),
            Expr::Coalesce(l) => format!("coalesce({})", list(l)),
            Expr::Exists(id) => format!("EXISTS#{}", id.0),
            Expr::Call(f, args) => match f {
                spargebra::algebra::Function::Custom(n) => format!("{}({})", iri(n.as_str()), list(args)),
                f => format!("{}({})", f.to_string().to_lowercase(), list(args)),
            },
        }
    }

    fn agg(&self, out: Slot, a: &AggSpec) -> String {
        let name = match &a.func {
            AggFunc::CountAll | AggFunc::Count => "count".to_owned(),
            AggFunc::Sum => "sum".to_owned(),
            AggFunc::Avg => "avg".to_owned(),
            AggFunc::Min => "min".to_owned(),
            AggFunc::Max => "max".to_owned(),
            AggFunc::Sample => "sample".to_owned(),
            AggFunc::GroupConcat { separator } => format!("group_concat[sep={separator:?}]"),
            AggFunc::Custom(n) => iri(n),
        };
        let distinct = if a.distinct { "DISTINCT " } else { "" };
        let arg = a.arg.as_ref().map_or_else(|| "*".to_owned(), |e| self.expr(e));
        format!("{}={name}({distinct}{arg})", self.slot(out))
    }

    fn order_keys(&self, keys: &[OrderKey]) -> String {
        let k: Vec<String> = keys
            .iter()
            .map(|k| if k.desc { format!("DESC({})", self.expr(&k.expr)) } else { self.expr(&k.expr) })
            .collect();
        format!("[{}]", k.join(" "))
    }

    fn form(&self, f: &QueryForm) -> String {
        match f {
            QueryForm::Select(s) => format!("Select {}", self.slots(s.iter().copied())),
            QueryForm::Ask => "Ask".to_owned(),
            QueryForm::Describe(s) => format!("Describe {}", self.slots(s.iter().copied())),
            QueryForm::Construct(t) => {
                let tt = |x: &TemplateTerm| match x {
                    TemplateTerm::Const(c) => self.term(*c),
                    TemplateTerm::Slot(s) => self.slot(*s),
                    TemplateTerm::BNode(n) => format!("_:t{n}"),
                };
                let triples: Vec<String> = t.iter().map(|[s, p, o]| format!("{} {} {}", tt(s), tt(p), tt(o))).collect();
                format!("Construct {{ {} }}", triples.join(" . "))
            }
        }
    }

    // ------------------------------------------------------------ logical

    fn logical(&self, l: &Logical, depth: usize, out: &mut String) {
        let pad = "  ".repeat(depth);
        let line = match l {
            Logical::Unit => "Unit".to_owned(),
            Logical::Empty => "Empty".to_owned(),
            Logical::Bgp(pats) => {
                let _ = writeln!(out, "{pad}Bgp");
                for p in pats {
                    let _ = writeln!(out, "{pad}  {}", self.quad(p));
                }
                return;
            }
            Logical::Path { g, s, path, o } => {
                format!("Path {}{} {} {}", self.graph(*g), self.pat_term(*s), self.path(path), self.pat_term(*o))
            }
            Logical::Join(..) => "Join".to_owned(),
            Logical::LeftJoin { filter, .. } => match filter {
                Some(f) => format!("LeftJoin filter={}", self.expr(f)),
                None => "LeftJoin".to_owned(),
            },
            Logical::Minus { .. } => "Minus".to_owned(),
            Logical::Union(..) => "Union".to_owned(),
            Logical::Filter { expr, .. } => format!("Filter {}", self.expr(expr)),
            Logical::Extend { slot, expr, .. } => format!("Extend {} := {}", self.slot(*slot), self.expr(expr)),
            Logical::Values { slots, rows } => {
                let _ = writeln!(out, "{pad}Values {}", self.slots(slots.iter().copied()));
                for r in rows {
                    let _ = writeln!(out, "{pad}  ({})", r.iter().map(|v| self.term(*v)).collect::<Vec<_>>().join(" "));
                }
                return;
            }
            Logical::Graph { name, .. } => format!("Graph {}", self.pat_term(*name)),
            Logical::Group { keys, aggs, .. } => format!(
                "Group keys={} aggs=[{}]",
                self.slots(keys.iter().copied()),
                aggs.iter().map(|(o, a)| self.agg(*o, a)).collect::<Vec<_>>().join(", ")
            ),
            Logical::OrderBy { keys, .. } => format!("OrderBy {}", self.order_keys(keys)),
            Logical::Project { slots, .. } => format!("Project {}", self.slots(slots.iter().copied())),
            Logical::Distinct(_) => "Distinct".to_owned(),
            Logical::Reduced(_) => "Reduced".to_owned(),
            Logical::Slice { offset, limit, .. } => format!("Slice offset={offset} limit={}", fmt_limit(*limit)),
            Logical::Service { silent } => format!("Service silent={silent}"),
        };
        let _ = writeln!(out, "{pad}{line}");
        for c in crate::logical::children(l) {
            self.logical(c, depth + 1, out);
        }
    }

    // ------------------------------------------------------------ physical

    fn kind(&self, k: &JoinKind) -> String {
        match k {
            JoinKind::Inner => String::new(),
            JoinKind::Left { filter: None } => " left".to_owned(),
            JoinKind::Left { filter: Some(f) } => format!(" left filter={}", self.expr(f)),
            JoinKind::Semi => " semi".to_owned(),
            JoinKind::Anti => " anti".to_owned(),
        }
    }

    fn op(&self, p: &Plan) -> String {
        let s = |v: &[Slot]| self.slots(v.iter().copied());
        match &p.op {
            Op::Unit => "Unit".to_owned(),
            Op::Empty => "Empty".to_owned(),
            Op::Scan(sc) => {
                let mut t = format!("Scan {:?} {}", sc.order, self.quad(&sc.pat));
                if let Some(r) = sc.range {
                    let lo = if r.lo == TermId::INT_MIN { String::new() } else { r.lo.to_string() };
                    let hi = if r.hi == TermId::INT_MAX { String::new() } else { r.hi.to_string() };
                    let _ = write!(t, " range=[{lo}..{hi}]");
                }
                if let Some(x) = p.sorted_by {
                    let _ = write!(t, " (sorted {})", self.slot(x));
                }
                t
            }
            Op::Values { slots, rows } => format!("Values {} rows={}", s(slots), rows.len()),
            Op::BindJoin { seed, kind, sort_left, adaptive, .. } => format!(
                "BindJoin seed={}{}{}{}",
                s(seed),
                if *sort_left { " sort_left" } else { "" },
                if *adaptive { " adaptive" } else { "" },
                self.kind(kind)
            ),
            Op::HashJoin { keys, check, kind, build_is_left, .. } => format!(
                "HashJoin keys={}{} build={}{}",
                s(keys),
                check_text(&s(check), check.is_empty()),
                if *build_is_left { "left" } else { "right" },
                self.kind(kind)
            ),
            Op::MergeJoin { key, check, gallop, .. } => {
                format!("MergeJoin key={} gallop={gallop}{}", self.slot(*key), check_text(&s(check), check.is_empty()))
            }
            Op::StarMerge { key, check, .. } => {
                format!("StarMerge key={}{}", self.slot(*key), check_text(&s(check), check.is_empty()))
            }
            Op::NestedLoop { check, kind, .. } => {
                format!("NestedLoop{}{}", check_text(&s(check), check.is_empty()), self.kind(kind))
            }
            Op::Minus { keys, check, .. } => {
                format!("Minus keys={}{}", s(keys), check_text(&s(check), check.is_empty()))
            }
            Op::Union(_) => "Union".to_owned(),
            Op::Filter { expr, .. } => format!("Filter {}", self.expr(expr)),
            Op::Extend { slot, expr, canonical, .. } => format!(
                "Extend {} := {}{}",
                self.slot(*slot),
                self.expr(expr),
                if *canonical { "" } else { " (output-only)" }
            ),
            Op::Path { g, s: subj, path, o } => {
                format!("Path {}{} {} {}", self.graph(*g), self.pat_term(*subj), self.path(path), self.pat_term(*o))
            }
            Op::GraphLoop { slot, .. } => format!("GraphLoop {}", self.slot(*slot)),
            Op::GraphNames { slot } => format!("GraphNames {}", self.slot(*slot)),
            Op::Group { keys, aggs, streaming, .. } => format!(
                "Group keys={} aggs=[{}]{}",
                s(keys),
                aggs.iter().map(|(o, a)| self.agg(*o, a)).collect::<Vec<_>>().join(", "),
                if *streaming { " streaming" } else { "" }
            ),
            Op::Count { out, input } => {
                let fast = match self.dataset.and_then(|d| count_fast_path(input, d)) {
                    Some(FastCount::Scan) => " fast=scan-vdbe",
                    Some(FastCount::Merge) => " fast=merge-factorized",
                    Some(FastCount::Star) => " fast=star-factorized",
                    None => "",
                };
                format!("Count -> {}{fast}", self.slot(*out))
            }
            Op::Sort { keys, .. } => format!("Sort {}", self.order_keys(keys)),
            Op::SortBySlot { key, .. } => format!("SortBySlot {}", self.slot(*key)),
            Op::TopK { keys, k, .. } => format!("TopK k={k} {}", self.order_keys(keys)),
            Op::Distinct { slots, .. } => format!("Distinct {}", s(slots)),
            Op::Reduced { slots, .. } => format!("Reduced {}", s(slots)),
            Op::Slice { offset, limit, .. } => format!("Slice offset={offset} limit={}", fmt_limit(*limit)),
            Op::Project { slots, .. } => format!("Project {}", s(slots)),
            Op::SqlJoin { sql, params, out } => self.sql_join(sql, params, out),
        }
    }

    /// `SqlJoin t0:Gpos(p=rdf:type,o=:C3) t1:Gpso(s=t0.s,p=:age) -> [?x ?a]`; the raw text for a
    /// statement of another shape.
    fn sql_join(&self, sql: &str, params: &[i64], out: &[Slot]) -> String {
        let outs = self.slots(out.iter().copied());
        let Some(parsed) = sqljoin::parse(sql) else {
            return format!("SqlJoin {sql} -> {outs}");
        };
        let col = |c: Col| ["g", "s", "p", "o"][c as usize];
        let mut text = String::from("SqlJoin");
        for (i, alias) in parsed.aliases.iter().enumerate() {
            let terms: Vec<String> = alias
                .terms
                .iter()
                .filter_map(|(c, operand)| match operand {
                    Operand::Param(n) => {
                        let v = TermId(params.get(n - 1).copied().unwrap_or_default());
                        (*c != Col::G || v != TermId::DEFAULT_GRAPH).then(|| format!("{}={}", col(*c), self.term(v)))
                    }
                    Operand::Column(a, d) => Some(format!("{}=t{a}.{}", col(*c), col(*d))),
                })
                .collect();
            let _ = write!(text, " t{i}:{:?}({})", alias.order, terms.join(","));
        }
        let _ = write!(text, " -> {outs}");
        text
    }

    fn plan(&self, p: &Plan, depth: usize, profile: Option<&ExecProfile>, out: &mut String) {
        let mut line = format!("#{} {}{}", p.id, " ".repeat(depth), self.op(p));
        let width = line.chars().count();
        line.push_str(&" ".repeat(EST_COLUMN.saturating_sub(width).max(1)));
        let _ = write!(line, "est={}", fmt_est(p.est_rows));
        if p.est_cost_ns > 0.0 {
            let _ = write!(line, " cost={}", fmt_ns(p.est_cost_ns));
        }
        if let Some(n) = profile.and_then(|pr| pr.nodes.get(p.id as usize)) {
            let _ = write!(
                line,
                " rows={} opens={} seeks={} ms={:.3}",
                n.rows_out,
                n.opens,
                n.seeks,
                n.time_ns as f64 / 1e6
            );
        }
        let _ = writeln!(out, "{line}");
        for c in p.children() {
            self.plan(c, depth + 1, profile, out);
        }
    }
}

fn check_text(slots: &str, empty: bool) -> String {
    if empty { String::new() } else { format!(" check={slots}") }
}

fn fmt_limit(l: Option<u64>) -> String {
    l.map_or_else(|| "none".to_owned(), |l| l.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_formats() {
        assert_eq!(fmt_est(1.0), "1");
        assert_eq!(fmt_est(12.34), "12.3");
        assert_eq!(fmt_est(20_000.0), "2.0e4");
        assert_eq!(fmt_ns(450.0), "450ns");
        assert_eq!(fmt_ns(35e6), "35.0ms");
        assert_eq!(fmt_ns(2.5e3), "2.5us");
    }

    #[test]
    fn terms() {
        let vars = VarTable::default();
        let mut temps = TempDict::new();
        let t = temps.intern(Term::Iri("http://example.org/absent".into()));
        let r = Render { vars: &vars, temps: &temps, dict: None, dataset: None };
        assert_eq!(r.term(t), "<http://example.org/absent>");
        assert_eq!(r.term(codec::RDF_TYPE), "rdf:type");
        assert_eq!(r.term(TermId::from_int(-4).unwrap()), "-4");
        assert_eq!(r.term(codec::encode_short_str("hi").unwrap()), "\"hi\"");
        assert_eq!(r.term(TermId::UNBOUND), "UNDEF");
        assert_eq!(r.term(TermId::from_dict(5000, Tag::Iri)), format!("<#{}>", TermId::from_dict(5000, Tag::Iri).raw()));
    }
}
