//! The seeded query generator. Most triple patterns are made from data triples, with
//! variables standing for the same data terms across patterns, so that about half the
//! queries have answers; constants mostly come from the data too.
use sq_core::testing::Rng;

use super::*;
use crate::fuzz::data::{Dataset, Quad, CLASSES, D, GRAPHS, LITERALS, PREDICATES, SUBJECTS};

/// A construct a query concentrates on (more of it, nested deeper), to stress the areas where
/// rewrites and operators interact most.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Focus {
    None,
    Exists,
    Graph,
    Paths,
    Aggregates,
    Optional,
    Minus,
}

/// Kinds of group elements.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Kind {
    Triple,
    Path,
    Filter,
    Bind,
    Values,
    Optional,
    Union,
    Minus,
    Graph,
    Sub,
    Group,
    /// A numeric-column pattern with an integer range filter in its own group.
    Range,
}

/// Variables visible while generating one group: `vars` may be joined on (outer and local),
/// `here` are in scope of this group so far (a BIND target must not be one of them).
#[derive(Clone, Default)]
struct Scope {
    vars: Vec<usize>,
    here: Vec<usize>,
    /// Inside GRAPH (patterns read named graphs).
    in_graph: bool,
}

impl Scope {
    fn nested(&self) -> Scope {
        Scope { vars: self.vars.clone(), here: Vec::new(), in_graph: self.in_graph }
    }

    fn add(&mut self, v: usize) {
        if !self.vars.contains(&v) {
            self.vars.push(v);
        }
        if !self.here.contains(&v) {
            self.here.push(v);
        }
    }

    fn add_all(&mut self, vs: &[usize]) {
        for v in vs {
            self.add(*v);
        }
    }
}

pub struct Gen<'r> {
    r: &'r mut Rng,
    /// Literals that occur in the data (constants mostly come from there, so they match).
    lits: Vec<&'static str>,
    /// The data's quads: most triple patterns are made from one of them.
    facts: Vec<Quad>,
    /// Graphs of the query's default graph (None = the store's default graph) and named
    /// graphs, per the dataset clause: fact patterns use quads of the right context.
    default_ctx: Vec<Option<&'static str>>,
    named_ctx: Vec<&'static str>,
    /// The data term each variable was introduced for by a fact pattern, so later fact
    /// patterns join consistently and the query usually has a solution.
    anchors: [Option<D>; VARS.len()],
    bnodes: usize,
    /// The construct this query concentrates on.
    focus: Focus,
    /// Expressions may call `BNODE()` (fresh blank nodes in an ordered or limited output
    /// would make the comparison depend on blank-node labels).
    allow_bnode_fn: bool,
    /// `BNODE()` anywhere, not only in top-level projections. Off by default: sq-plan may put
    /// a subpattern that mints blank nodes on the probe side of a BindJoin, which re-evaluates
    /// it per left row (a known planner issue, see the sq-engine notes).
    bnode_anywhere: bool,
    /// Generating a top-level projection expression (evaluated once per final solution).
    in_projection: bool,
}

/// Constants used in expressions (numbers, strings, IRIs, language strings, dates).
const EXPR_CONSTS: [&str; 16] = [
    "0", "1", "2", "3", "-1", "1.5", "2.0", "\"1.0e0\"^^xsd:double", "\"a\"", "\"b\"", "\"a\"@en", "\"\"", ":s1",
    "true", "\"2020-01-01\"^^xsd:date", "\"01\"^^xsd:integer",
];

impl<'r> Gen<'r> {
    pub fn new(r: &'r mut Rng, data: &Dataset) -> Self {
        let lits = data.literals();
        let lits = if lits.is_empty() { LITERALS.to_vec() } else { lits };
        Gen {
            r,
            lits,
            facts: data.quads.clone(),
            default_ctx: vec![None],
            named_ctx: GRAPHS.to_vec(),
            anchors: [None; VARS.len()],
            bnodes: 0,
            focus: Focus::None,
            allow_bnode_fn: true,
            bnode_anywhere: false,
            in_projection: false,
        }
    }

    /// Allow `BNODE()` in any expression, not only top-level projections.
    pub fn bnode_anywhere(mut self, on: bool) -> Self {
        self.bnode_anywhere = on;
        self
    }

    /// A literal constant: usually one of the data's, sometimes any.
    fn literal(&mut self) -> &'static str {
        if self.chance(80) {
            let lits = std::mem::take(&mut self.lits);
            let l = self.pick(&lits);
            self.lits = lits;
            l
        } else {
            self.pick(&LITERALS)
        }
    }

    fn below(&mut self, n: u64) -> u64 {
        self.r.below(n)
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.r.below(100) < percent
    }

    fn pick<X: Copy>(&mut self, xs: &[X]) -> X {
        xs[self.r.below(xs.len() as u64) as usize]
    }

    fn bnode(&mut self) -> T {
        self.bnodes += 1;
        T::BNode(self.bnodes)
    }

    /// A variable for a pattern position: usually a visible one (so patterns join), rarely
    /// `avoid` (repeated variables in one pattern seldom match).
    fn pattern_var(&mut self, sc: &mut Scope, join: u64, avoid: Option<usize>) -> T {
        let mut v = if !sc.vars.is_empty() && self.chance(join) { self.pick(&sc.vars) } else { self.below(6) as usize };
        if Some(v) == avoid && self.chance(85) {
            v = (v + 1 + self.below(5) as usize) % 6;
        }
        sc.add(v);
        T::Var(v)
    }

    fn subject(&mut self, sc: &mut Scope) -> T {
        match self.below(10) {
            0..=6 => self.pattern_var(sc, 75, None),
            7..=8 => T::Iri(self.pick(&SUBJECTS)),
            _ => self.bnode(),
        }
    }

    fn object(&mut self, sc: &mut Scope, subject: &T) -> T {
        let avoid = match subject {
            T::Var(v) => Some(*v),
            _ => None,
        };
        match self.below(20) {
            0..=11 => self.pattern_var(sc, 35, avoid),
            12..=15 => T::Iri(self.pick(&SUBJECTS)),
            _ => T::Lit(self.literal()),
        }
    }

    /// A variable standing for data term `d`: one already introduced for it, else a fresh one.
    fn fact_var(&mut self, d: D, sc: &mut Scope) -> T {
        let known: Vec<usize> = (0..6).filter(|v| self.anchors[*v] == Some(d)).collect();
        let v = if !known.is_empty() && self.chance(85) {
            self.pick(&known)
        } else {
            let free: Vec<usize> = (0..6).filter(|v| self.anchors[*v].is_none()).collect();
            if free.is_empty() { self.below(6) as usize } else { self.pick(&free) }
        };
        if self.anchors[v].is_none() {
            self.anchors[v] = Some(d);
        }
        sc.add(v);
        T::Var(v)
    }

    fn fact_term(&mut self, d: D, sc: &mut Scope, constant: u64) -> T {
        match d {
            D::Iri(i) if self.chance(constant) => T::Iri(i),
            D::Lit(l) if self.chance(constant) => T::Lit(l),
            _ => self.fact_var(d, sc),
        }
    }

    /// A triple pattern made from a data triple of the current graph context, joined through
    /// a visible variable when one stands for a term of that triple.
    fn fact_triple(&mut self, sc: &mut Scope) -> Option<El> {
        let facts: Vec<usize> = (0..self.facts.len())
            .filter(|i| match self.facts[*i].graph {
                g if !sc.in_graph => self.default_ctx.contains(&g),
                Some(g) => self.named_ctx.contains(&g),
                None => false,
            })
            .collect();
        if facts.is_empty() {
            return None;
        }
        let anchored: Vec<(usize, D)> = sc.vars.iter().filter_map(|v| self.anchors.get(*v).copied().flatten().map(|d| (*v, d))).collect();
        // (fact, Some((variable, subject side?)))
        let mut choice = None;
        if !anchored.is_empty() && self.chance(75) {
            let (v, d) = self.pick(&anchored);
            let joins: Vec<(usize, bool)> = facts
                .iter()
                .flat_map(|i| {
                    let f = &self.facts[*i];
                    [(f.s == d).then_some((*i, true)), (f.o == d).then_some((*i, false))]
                })
                .flatten()
                .collect();
            if !joins.is_empty() {
                let (i, subject) = self.pick(&joins);
                choice = Some((i, Some((v, subject))));
            }
        }
        let (i, join) = choice.unwrap_or_else(|| (self.pick(&facts), None));
        let f = self.facts[i].clone();
        let s = match join {
            Some((v, true)) => T::Var(v),
            _ => self.fact_term(f.s, sc, 20),
        };
        let p = match f.p {
            D::Type => T::Type,
            D::Iri(p) if self.chance(92) => T::Iri(p),
            d => self.fact_var(d, sc),
        };
        let o = match join {
            Some((v, false)) => T::Var(v),
            _ => self.fact_term(f.o, sc, 35),
        };
        Some(El::Triple(s, p, o))
    }

    fn triple(&mut self, sc: &mut Scope) -> El {
        if self.chance(85)
            && let Some(t) = self.fact_triple(sc)
        {
            return t;
        }
        let s = self.subject(sc);
        if self.chance(8) {
            let o = if self.chance(70) { T::Iri(self.pick(&CLASSES)) } else { self.pattern_var(sc, 30, None) };
            return El::Triple(s, T::Type, o);
        }
        let p = if self.chance(10) { self.pattern_var(sc, 20, None) } else { T::Iri(self.pick(&PREDICATES)) };
        let o = self.object(sc, &s);
        El::Triple(s, p, o)
    }

    fn path(&mut self, depth: u32) -> Path {
        if depth == 0 || self.chance(35) {
            return Path::Pred(self.pick(&PREDICATES));
        }
        let sub = |me: &mut Self| Box::new(me.path(depth - 1));
        match self.below(8) {
            0 => Path::Inv(sub(self)),
            1 => Path::Seq(sub(self), sub(self)),
            2 => Path::Alt(sub(self), sub(self)),
            3 => Path::Star(sub(self)),
            4 => Path::Plus(sub(self)),
            5 => Path::Opt(sub(self)),
            _ => {
                let n = 1 + self.below(2) as usize;
                Path::Nps((0..n).map(|_| (self.chance(30), self.pick(&PREDICATES))).collect())
            }
        }
    }

    fn path_el(&mut self, sc: &mut Scope) -> El {
        let s = match self.below(20) {
            0..=13 => self.pattern_var(sc, 70, None),
            // A literal start: only zero-length paths can match it.
            14 => T::Lit(self.literal()),
            _ => T::Iri(self.pick(&SUBJECTS)),
        };
        let p = self.path(if self.focus == Focus::Paths { 3 } else { 2 });
        let o = match self.below(10) {
            0..=6 => {
                // `?x p* ?x` is interesting but rare.
                let avoid = match &s {
                    T::Var(v) if self.chance(80) => Some(*v),
                    _ => None,
                };
                self.pattern_var(sc, 35, avoid)
            }
            7..=8 => T::Iri(self.pick(&SUBJECTS)),
            _ => T::Lit(self.literal()),
        };
        El::Path(s, p, o)
    }

    /// A group graph pattern over the visible variables `outer`; returns the group and the
    /// variables it may bind.
    fn group(&mut self, depth: u32, outer: &Scope) -> (Group, Vec<usize>) {
        // Usually 1-3 elements; sometimes a longer group (join ordering over more units,
        // star merges).
        let n = if self.chance(12) { 4 + self.below(3) as usize } else { 1 + self.below(3) as usize };
        let mut sc = outer.nested();
        let els = (0..n).map(|i| self.element(depth, &mut sc, i == 0)).collect();
        (Group(els), sc.here)
    }

    /// The kind of the next group element: weighted, composite kinds only above depth 0,
    /// FILTER and BIND never first; the query's focus multiplies its construct's weight.
    fn kind(&mut self, depth: u32, first: bool) -> Kind {
        let mut weights: Vec<(Kind, u64)> = vec![(Kind::Triple, 35), (Kind::Path, 7), (Kind::Values, 5)];
        if !first {
            weights.extend([(Kind::Filter, 8), (Kind::Bind, 5)]);
        }
        if depth > 0 {
            weights.extend([
                (Kind::Optional, 10),
                (Kind::Union, 7),
                (Kind::Minus, 5),
                (Kind::Graph, 7),
                (Kind::Sub, 6),
                (Kind::Group, 3),
                (Kind::Range, 2),
            ]);
        }
        for (kind, w) in &mut weights {
            let boost = match (self.focus, *kind) {
                (Focus::Exists, Kind::Filter) => 3,
                (Focus::Graph, Kind::Graph) => 4,
                (Focus::Paths, Kind::Path) => 5,
                (Focus::Aggregates, Kind::Sub) => 3,
                (Focus::Optional, Kind::Optional) => 3,
                (Focus::Minus, Kind::Minus) => 4,
                _ => 1,
            };
            *w *= boost;
        }
        let total: u64 = weights.iter().map(|(_, w)| w).sum();
        let mut roll = self.below(total);
        for (kind, w) in weights {
            if roll < w {
                return kind;
            }
            roll -= w;
        }
        Kind::Triple
    }

    fn element(&mut self, depth: u32, sc: &mut Scope, first: bool) -> El {
        match self.kind(depth, first) {
            Kind::Triple => self.triple(sc),
            Kind::Path => self.path_el(sc),
            Kind::Filter => El::Filter(self.filter(depth, sc)),
            Kind::Bind => {
                let free: Vec<usize> = (0..6).filter(|v| !sc.here.contains(v)).collect();
                if free.is_empty() {
                    return self.triple(sc);
                }
                let v = self.pick(&free);
                let e = self.bind_expr(sc);
                sc.add(v);
                El::Bind(e, v)
            }
            Kind::Values => self.values(sc),
            Kind::Optional => {
                let (mut g, bound) = self.group(depth - 1, sc);
                // OPTIONAL with a filter, which may read variables bound outside it.
                if self.chance(if self.focus == Focus::Optional { 70 } else { 40 }) {
                    let mut inner = sc.clone();
                    inner.add_all(&bound);
                    g.0.push(El::Filter(self.filter(depth - 1, &inner)));
                }
                sc.add_all(&bound);
                El::Optional(g)
            }
            Kind::Union => {
                let (a, ba) = self.group(depth - 1, sc);
                let (b, bb) = self.group(depth - 1, sc);
                sc.add_all(&ba);
                sc.add_all(&bb);
                El::Union(a, b)
            }
            Kind::Minus => El::Minus(self.group(depth - 1, sc).0),
            Kind::Graph => {
                let name = if self.chance(60) {
                    sc.add(GRAPH_VAR);
                    T::Var(GRAPH_VAR)
                } else {
                    T::Iri(self.pick(&QUERY_GRAPHS))
                };
                let inner = Scope { in_graph: true, ..sc.clone() };
                let (g, bound) = if self.chance(5) { (Group::default(), Vec::new()) } else { self.group(depth - 1, &inner) };
                sc.add_all(&bound);
                El::Graph(name, g)
            }
            Kind::Sub => {
                let s = self.select(depth - 1, false);
                sc.add_all(&s.projected().unwrap_or_default());
                El::Sub(Box::new(s))
            }
            Kind::Group => {
                let (g, bound) = self.group(depth - 1, sc);
                sc.add_all(&bound);
                El::Group(g)
            }
            Kind::Range => self.range_group(sc),
        }
    }

    /// `{ ?x :p3 ?v FILTER(?v op n) }`: a numeric-column pattern with an integer range
    /// filter in its own group (the shape of IntRange scans).
    fn range_group(&mut self, sc: &mut Scope) -> El {
        let s = self.pattern_var(sc, 60, None);
        let v = self.pattern_var(sc, 20, None);
        let op = self.pick(&["<", "<=", ">", ">="]);
        let n = self.pick(&["0", "3", "10", "12", "20", "4.5", "\"7.5e0\"^^xsd:double", "\"10\"^^xsd:int"]);
        let mut els = vec![
            El::Triple(s, T::Iri(PREDICATES[3]), v.clone()),
            El::Filter(E::Bin(op, Box::new(match v {
                T::Var(x) => E::Var(x),
                _ => unreachable!("pattern_var returns a variable"),
            }), Box::new(E::Const(n)))),
        ];
        if self.chance(30) {
            let upper = self.pick(&["15", "25", "100"]);
            if let El::Filter(e) = &mut els[1] {
                let x = match e {
                    E::Bin(_, a, _) => (**a).clone(),
                    _ => unreachable!(),
                };
                *e = E::Bin("&&", Box::new(e.clone()), Box::new(E::Bin("<", Box::new(x), Box::new(E::Const(upper)))));
            }
        }
        El::Group(Group(els))
    }

    fn values(&mut self, sc: &mut Scope) -> El {
        let nvars = 1 + self.below(2) as usize;
        let mut vars: Vec<usize> = Vec::new();
        while vars.len() < nvars {
            let v = if !sc.vars.is_empty() && self.chance(65) { self.pick(&sc.vars) } else { self.below(6) as usize };
            if !vars.contains(&v) {
                vars.push(v);
            }
        }
        let rows = (0..1 + self.below(3))
            .map(|_| {
                vars.iter()
                    .map(|v| {
                        // Usually the data term the variable stands for, so the VALUES joins.
                        match (self.anchors[*v], self.below(10)) {
                            (Some(D::Iri(i)), 0..=5) => return Some(T::Iri(i)),
                            (Some(D::Lit(l)), 0..=5) => return Some(T::Lit(l)),
                            _ => {}
                        }
                        match self.below(10) {
                            0..=1 => None,
                            2..=6 => Some(T::Iri(self.pick(&SUBJECTS))),
                            _ => Some(T::Lit(self.literal())),
                        }
                    })
                    .collect()
            })
            .collect();
        sc.add_all(&vars);
        El::Values(vars, rows)
    }

    fn operand(&mut self, vars: &[usize]) -> E {
        if !vars.is_empty() && self.chance(70) {
            E::Var(self.pick(vars))
        } else if self.chance(10) {
            // Sometimes a variable that is not in scope (filter scoping).
            E::Var(self.below(6) as usize)
        } else {
            E::Const(self.pick(&EXPR_CONSTS))
        }
    }

    /// A condition (FILTER, HAVING, IF).
    fn filter(&mut self, depth: u32, sc: &Scope) -> E {
        // Mostly variables of this group (pushdown, range scans), sometimes outer ones
        // (filter scoping in OPTIONAL / MINUS / EXISTS).
        let pool = if !sc.here.is_empty() && self.chance(60) { sc.here.clone() } else { sc.vars.clone() };
        let vars = &pool;
        // EXISTS (rolls 88..) about 1 in 8 conditions, half of them under the Exists focus.
        let roll = match depth {
            0 => self.below(88),
            _ if self.focus == Focus::Exists && self.chance(45) => 99,
            _ => self.below(100),
        };
        match roll {
            0..=29 => {
                let op = self.pick(&["=", "!=", "<", ">", "<=", ">="]);
                let a = self.operand(vars);
                // `?x op number` is the shape of integer range pushdown.
                let b = if matches!(a, E::Var(_)) && self.chance(50) {
                    E::Const(self.pick(&["0", "1", "2", "3", "-1", "1.5", "\"2.5e0\"^^xsd:double", "10", "20", "4.5"]))
                } else {
                    self.operand(vars)
                };
                E::Bin(op, Box::new(a), Box::new(b))
            }
            30..=37 => {
                let f = self.pick(&["isIRI", "isBlank", "isLiteral", "isNumeric"]);
                E::Call(f, vec![self.operand(vars)])
            }
            38..=42 => E::Bound(if vars.is_empty() { 0 } else { self.pick(vars) }),
            43..=47 => E::Call("sameTerm", vec![self.operand(vars), self.operand(vars)]),
            48..=53 => {
                let list = (0..1 + self.below(3)).map(|_| E::Const(self.pick(&EXPR_CONSTS))).collect();
                E::In(Box::new(self.operand(vars)), list, self.chance(40))
            }
            54..=59 => {
                let op = self.pick(&["&&", "||"]);
                E::Bin(op, Box::new(self.filter(0, sc)), Box::new(self.filter(0, sc)))
            }
            60..=63 => E::Not(Box::new(self.filter(0, sc))),
            64..=71 => {
                let v = self.value_expr(vars, 1);
                let op = self.pick(&["=", "<", ">"]);
                E::Bin(op, Box::new(v), Box::new(E::Const(self.pick(&EXPR_CONSTS))))
            }
            72..=77 => {
                let f = self.pick(&["CONTAINS", "STRSTARTS", "STRENDS"]);
                let arg = E::Call("STR", vec![self.operand(vars)]);
                E::Call(f, vec![arg, E::Const(self.pick(&["\"a\"", "\"\"", "\"1\""]))])
            }
            78..=81 => {
                let lang = E::Call("LANG", vec![self.operand(vars)]);
                E::Call("LANGMATCHES", vec![lang, E::Const(self.pick(&["\"en\"", "\"*\"", "\"fr\""]))])
            }
            82..=87 => {
                let arg = E::Call("STR", vec![self.operand(vars)]);
                let pattern = E::Const(self.pick(&["\"^a\"", "\"b$\"", "\"A\""]));
                E::Call("REGEX", vec![arg, pattern, E::Const(self.pick(&["\"i\"", "\"\""]))])
            }
            _ => {
                let (g, _) = self.group(depth - 1, sc);
                E::Exists(self.chance(50), g)
            }
        }
    }

    /// A value-producing expression (BIND, projections, comparisons).
    fn value_expr(&mut self, vars: &[usize], depth: u32) -> E {
        if depth == 0 {
            return self.operand(vars);
        }
        match self.below(16) {
            0..=2 => {
                let op = self.pick(&["+", "-", "*", "/"]);
                E::Bin(op, Box::new(self.value_expr(vars, depth - 1)), Box::new(self.operand(vars)))
            }
            3 => E::Call(self.pick(&["STR", "LANG", "DATATYPE", "STRLEN", "UCASE", "LCASE"]), vec![self.operand(vars)]),
            4 => E::Call(self.pick(&["ABS", "ROUND", "CEIL", "FLOOR"]), vec![self.operand(vars)]),
            5 => E::Call(
                self.pick(&["xsd:integer", "xsd:string", "xsd:double", "xsd:decimal", "xsd:boolean"]),
                vec![self.operand(vars)],
            ),
            6 => E::Call("COALESCE", vec![self.operand(vars), self.operand(vars)]),
            7 => {
                let c = self.filter(0, &Scope { vars: vars.to_vec(), ..Scope::default() });
                E::Call("IF", vec![c, self.operand(vars), self.operand(vars)])
            }
            8 => {
                let s = E::Call("STR", vec![self.operand(vars)]);
                E::Call("CONCAT", vec![s, E::Const(self.pick(&["\"x\"", "\"\""]))])
            }
            9 => E::Call("SUBSTR", vec![E::Call("STR", vec![self.operand(vars)]), E::Const(self.pick(&["1", "2"]))]),
            10 => E::Neg(Box::new(self.operand(vars))),
            11 if self.allow_bnode_fn && (self.bnode_anywhere || self.in_projection) => E::Call("BNODE", vec![]),
            12 => E::Call("STRDT", vec![E::Call("STR", vec![self.operand(vars)]), E::Const("xsd:integer")]),
            13 => {
                let s = E::Call("STR", vec![self.operand(vars)]);
                E::Call("IRI", vec![E::Call("CONCAT", vec![E::Const("\"http://ex/\""), s])])
            }
            _ => self.operand(vars),
        }
    }

    fn bind_expr(&mut self, sc: &Scope) -> E {
        let depth = 1 + self.below(2) as u32;
        self.value_expr(&sc.vars, depth)
    }

    fn agg(&mut self, vars: &[usize]) -> Agg {
        let v = if vars.is_empty() { 0 } else { self.pick(vars) };
        let d = self.chance(25);
        match self.below(8) {
            0 => Agg::CountAll(d),
            1 | 2 => Agg::Count(d, v),
            3 => Agg::Sum(d, v),
            4 => Agg::Avg(d, v),
            5 => Agg::Min(v),
            6 => Agg::Max(v),
            _ => {
                let func = self.pick(&["SUM", "AVG", "MIN", "MAX", "COUNT"]);
                Agg::Expr(func, d, Box::new(self.value_expr(vars, 1)))
            }
        }
    }

    /// A SELECT (top level or sub-select; sub-selects always project explicitly).
    fn select(&mut self, depth: u32, top: bool) -> Select {
        let (pattern, scope) = self.group(depth, &Scope::default());
        let modifier = match self.below(10) {
            0..=1 => Modifier::Distinct,
            2 if top => Modifier::Reduced,
            _ => Modifier::None,
        };
        let mut s = Select {
            modifier,
            items: None,
            pattern,
            group_by: None,
            having: None,
            order_exprs: Vec::new(),
            order: Vec::new(),
            offset: None,
            limit: None,
        };
        let unused: Vec<usize> = (0..6).filter(|v| !scope.contains(v)).collect();
        if !scope.is_empty() && self.chance(if self.focus == Focus::Aggregates { 60 } else { 20 }) {
            // Aggregation: keys (variables, or expressions bound to unused variables, which
            // may err), aggregates bound to unused variables, maybe HAVING.
            let mut keys: Vec<(usize, Option<E>)> = Vec::new();
            for _ in 0..self.below(3) {
                let k = self.pick(&scope);
                if !keys.iter().any(|(x, _)| *x == k) {
                    keys.push((k, None));
                }
            }
            let mut unused = unused;
            if !unused.is_empty() && self.chance(25) {
                let v = unused.remove(0);
                let e = self.value_expr(&scope, 1);
                keys.push((v, Some(e)));
            }
            let mut items: Vec<Item> = keys.iter().map(|(k, _)| Item::Var(*k)).collect();
            let naggs = 1 + self.below(2) as usize;
            for (i, out) in unused.iter().take(naggs).enumerate() {
                let a = E::Agg(Box::new(self.agg(&scope)));
                let e = if i == 0 || self.chance(50) { a } else { E::Bin("+", Box::new(a), Box::new(E::Const("1"))) };
                items.push(Item::Expr(e, *out));
            }
            if self.chance(30) {
                let a = E::Agg(Box::new(self.agg(&scope)));
                let op = self.pick(&[">", "<", "="]);
                s.having = Some(E::Bin(op, Box::new(a), Box::new(E::Const(self.pick(&["0", "1", "2"])))));
            }
            s.group_by = Some(keys);
            s.items = Some(items);
        } else if !top || scope.is_empty() || self.chance(70) {
            let mut items: Vec<Item> = Vec::new();
            for v in &scope {
                if self.chance(60) {
                    items.push(Item::Var(*v));
                }
            }
            if self.chance(15)
                && let Some(out) = unused.first()
            {
                self.in_projection = top;
                let e = self.value_expr(&scope, 1);
                self.in_projection = false;
                items.push(Item::Expr(e, *out));
            }
            if items.is_empty() {
                match (scope.first(), unused.first()) {
                    (Some(v), _) => items.push(Item::Var(*v)),
                    (None, Some(out)) => items.push(Item::Expr(E::Const("1"), *out)),
                    (None, None) => {}
                }
            }
            s.items = Some(items);
        }
        // ORDER BY over every projected variable, then maybe LIMIT / OFFSET.
        if let Some(proj) = s.projected()
            && !proj.is_empty()
            && self.chance(if top { 35 } else { 25 })
        {
            let mut order: Vec<(usize, bool)> = proj.iter().map(|v| (*v, self.chance(30))).collect();
            for i in (1..order.len()).rev() {
                let j = self.below(i as u64 + 1) as usize;
                order.swap(i, j);
            }
            s.order = order;
            if s.group_by.is_none() && self.chance(25) {
                let e = self.value_expr(&scope, 1);
                s.order_exprs.push((e, self.chance(50)));
            }
            // REDUCED may keep any number of duplicates: a slice of it is not deterministic.
            if s.modifier != Modifier::Reduced {
                if self.chance(70) {
                    s.limit = Some(if self.chance(90) { 1 + self.below(4) } else { 0 });
                }
                if self.chance(40) {
                    s.offset = Some(self.below(3));
                }
            }
        }
        s
    }

    fn template_term(&mut self, vars: &[usize], allow_lit: bool) -> T {
        match self.below(10) {
            0..=5 if !vars.is_empty() => T::Var(self.pick(vars)),
            6 => self.bnode(),
            7 if allow_lit => T::Lit(self.literal()),
            _ => T::Iri(self.pick(&SUBJECTS)),
        }
    }

    /// A WHERE pattern for an update (default dataset), with the variables it may bind.
    pub fn where_pattern(&mut self) -> (Group, Vec<usize>) {
        let depth = 1 + self.below(2) as u32;
        self.allow_bnode_fn = self.chance(30);
        self.group(depth, &Scope::default())
    }

    pub fn query(&mut self) -> Query {
        let depth = match self.below(20) {
            0..=9 => 1,
            10..=16 => 2,
            _ => 3,
        };
        self.allow_bnode_fn = self.chance(30);
        self.focus = if self.chance(35) {
            self.pick(&[Focus::Exists, Focus::Graph, Focus::Paths, Focus::Aggregates, Focus::Optional, Focus::Minus])
        } else {
            Focus::None
        };
        // The dataset first: it decides which quads fact patterns are drawn from.
        let mut from = Vec::new();
        let mut from_named = Vec::new();
        if self.chance(10) {
            for _ in 0..1 + self.below(2) {
                from.push(self.pick(&QUERY_GRAPHS));
            }
        }
        if self.chance(8) {
            for _ in 0..1 + self.below(2) {
                from_named.push(self.pick(&QUERY_GRAPHS));
            }
        }
        let union_default = self.chance(12);
        self.default_ctx = if !from.is_empty() {
            from.iter().map(|g| Some(*g)).collect()
        } else if !from_named.is_empty() {
            Vec::new()
        } else if union_default {
            std::iter::once(None).chain(GRAPHS.iter().map(|g| Some(*g))).collect()
        } else {
            vec![None]
        };
        self.named_ctx = if !from_named.is_empty() {
            from_named.clone()
        } else if !from.is_empty() {
            Vec::new()
        } else {
            GRAPHS.to_vec()
        };
        let form = match self.below(20) {
            0..=14 => Form::Select(self.select(depth, true)),
            15..=16 => Form::Ask(self.group(depth, &Scope::default()).0),
            17..=18 => {
                let (g, vars) = self.group(depth, &Scope::default());
                let template = (0..1 + self.below(3))
                    .map(|_| {
                        let s = self.template_term(&vars, false);
                        let p = if self.chance(15) && !vars.is_empty() {
                            T::Var(self.pick(&vars))
                        } else {
                            T::Iri(self.pick(&PREDICATES))
                        };
                        let o = self.template_term(&vars, true);
                        (s, p, o)
                    })
                    .collect();
                Form::Construct(template, g)
            }
            _ => {
                let (g, vars) = self.group(depth, &Scope::default());
                let t = if !vars.is_empty() && self.chance(70) { T::Var(self.pick(&vars)) } else { T::Iri(self.pick(&SUBJECTS)) };
                Form::Describe(vec![t], g)
            }
        };
        let probes = self.chance(70);
        let mut q = Query { form, from, from_named, union_default, probes };
        if !self.allow_bnode_fn || matches!(&q.form, Form::Select(s) if !s.order.is_empty()) {
            strip_bnode_fn(&mut q);
        }
        q
    }
}

/// Replaces every `BNODE()` call by a constant.
fn strip_bnode_fn(q: &mut Query) {
    fn expr(e: &mut E) {
        match e {
            E::Call("BNODE", _) => *e = E::Const(":s0"),
            E::Bin(_, a, b) => {
                expr(a);
                expr(b);
            }
            E::Not(a) | E::Neg(a) => expr(a),
            E::Call(_, args) => args.iter_mut().for_each(expr),
            E::In(a, l, _) => {
                expr(a);
                l.iter_mut().for_each(expr);
            }
            E::Exists(_, g) => group(g),
            E::Agg(a) => {
                if let Agg::Expr(_, _, e) = &mut **a {
                    expr(e);
                }
            }
            E::Var(_) | E::Const(_) | E::Bound(_) => {}
        }
    }
    fn group(g: &mut Group) {
        for el in &mut g.0 {
            match el {
                El::Filter(e) | El::Bind(e, _) => expr(e),
                El::Optional(g) | El::Minus(g) | El::Graph(_, g) | El::Group(g) => group(g),
                El::Union(a, b) => {
                    group(a);
                    group(b);
                }
                El::Sub(s) => select(s),
                El::Triple(..) | El::Path(..) | El::Values(..) => {}
            }
        }
    }
    fn select(s: &mut Select) {
        group(&mut s.pattern);
        if let Some(items) = &mut s.items {
            for i in items {
                if let Item::Expr(e, _) = i {
                    expr(e);
                }
            }
        }
        for (e, _) in &mut s.order_exprs {
            expr(e);
        }
        if let Some(h) = &mut s.having {
            expr(h);
        }
        for (_, e) in s.group_by.iter_mut().flatten() {
            if let Some(e) = e {
                expr(e);
            }
        }
    }
    match &mut q.form {
        Form::Select(s) => select(s),
        Form::Ask(g) | Form::Construct(_, g) | Form::Describe(_, g) => group(g),
    }
}
