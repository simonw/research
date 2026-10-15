//! Test harness: SPARQL expression text → spargebra → `sq_core::ir::Expr` (variables to slots,
//! constants to canonical ids or TEMP ids, as lowering does) → `CompiledExpr` on a `TestEnv`.
use oxrdf::TermRef;
use sq_core::ir::{ExistsId, Expr};
use sq_core::spargebra::algebra::{Expression, GraphPattern};
use sq_core::spargebra::{Query, SparqlParser};
use sq_core::{Slot, Term, TermId};

use crate::testing::{FnExists, TestEnv};
use crate::{CompiledExpr, compile};

pub const PREFIXES: &str = "PREFIX xsd: <http://www.w3.org/2001/XMLSchema#> \
     PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> PREFIX : <http://example.org/> ";

/// Compact rendering for assertions: `"a"`, `"a"@en`, `"1"^^xsd:integer`, `<iri>`, `_:label`.
pub fn compact(t: &Term) -> String {
    match t {
        Term::Iri(i) => format!("<{i}>"),
        Term::BNode(b) => format!("_:{b}"),
        Term::Literal(l) => {
            let lex = format!("{:?}", &*l.lex);
            match &l.lang {
                Some(lang) => format!("{lex}@{lang}"),
                None if l.is_simple() => lex,
                None => match l.datatype.strip_prefix(sq_core::codec::XSD) {
                    Some(local) => format!("{lex}^^xsd:{local}"),
                    None => format!("{lex}^^<{}>", l.datatype),
                },
            }
        }
    }
}

/// Converts an oxrdf term to an sq-core term.
pub fn term_of(t: TermRef<'_>) -> Term {
    Term::from_oxrdf(t)
}

/// Lowers spargebra expressions against a growing variable table.
#[derive(Default)]
pub struct Lowering {
    pub vars: Vec<String>,
    pub exists: Vec<GraphPattern>,
}

impl Lowering {
    pub fn slot(&mut self, name: &str) -> Slot {
        match self.vars.iter().position(|v| v == name) {
            Some(i) => i as Slot,
            None => {
                self.vars.push(name.to_string());
                (self.vars.len() - 1) as Slot
            }
        }
    }

    /// Constants resolve like lowering: canonical id if stored, else a TEMP id.
    pub fn constant(env: &mut TestEnv, t: Term) -> TermId {
        use sq_core::eval_api::TermResolver;
        env.resolver.intern(&t, true).expect("intern")
    }

    pub fn lower(&mut self, env: &mut TestEnv, e: &Expression) -> Expr {
        let mut b = |x: &Expression| Box::new(self.lower(env, x));
        match e {
            Expression::NamedNode(n) => Expr::Const(Self::constant(env, Term::Iri(n.as_str().into()))),
            Expression::Literal(l) => Expr::Const(Self::constant(env, term_of(l.as_ref().into()))),
            Expression::Variable(v) => Expr::Slot(self.slot(v.as_str())),
            Expression::Or(x, y) => Expr::Or(b(x), b(y)),
            Expression::And(x, y) => Expr::And(b(x), b(y)),
            Expression::Equal(x, y) => Expr::Equal(b(x), b(y)),
            Expression::SameTerm(x, y) => Expr::SameTerm(b(x), b(y)),
            Expression::Greater(x, y) => Expr::Greater(b(x), b(y)),
            Expression::GreaterOrEqual(x, y) => Expr::GreaterOrEqual(b(x), b(y)),
            Expression::Less(x, y) => Expr::Less(b(x), b(y)),
            Expression::LessOrEqual(x, y) => Expr::LessOrEqual(b(x), b(y)),
            Expression::Add(x, y) => Expr::Add(b(x), b(y)),
            Expression::Subtract(x, y) => Expr::Subtract(b(x), b(y)),
            Expression::Multiply(x, y) => Expr::Multiply(b(x), b(y)),
            Expression::Divide(x, y) => Expr::Divide(b(x), b(y)),
            Expression::UnaryPlus(x) => Expr::UnaryPlus(b(x)),
            Expression::UnaryMinus(x) => Expr::UnaryMinus(b(x)),
            Expression::Not(x) => Expr::Not(b(x)),
            Expression::In(x, list) => {
                let x = Box::new(self.lower(env, x));
                Expr::In(x, list.iter().map(|e| self.lower(env, e)).collect())
            }
            Expression::Exists(p) => {
                self.exists.push((**p).clone());
                Expr::Exists(ExistsId(self.exists.len() as u32 - 1))
            }
            Expression::Bound(v) => Expr::Bound(self.slot(v.as_str())),
            Expression::If(c, x, y) => Expr::If(b(c), b(x), b(y)),
            Expression::Coalesce(list) => Expr::Coalesce(list.iter().map(|e| self.lower(env, e)).collect()),
            Expression::FunctionCall(f, args) => Expr::Call(f.clone(), args.iter().map(|e| self.lower(env, e)).collect()),
        }
    }
}

/// Finds the expression bound to `?result_` / the first FILTER in a parsed query pattern.
fn find_expr(p: &GraphPattern) -> Option<&Expression> {
    match p {
        GraphPattern::Extend { variable, expression, inner } => {
            if variable.as_str() == "result_" { Some(expression) } else { find_expr(inner) }
        }
        GraphPattern::Filter { expr, .. } => Some(expr),
        GraphPattern::Project { inner, .. }
        | GraphPattern::Distinct { inner }
        | GraphPattern::Reduced { inner }
        | GraphPattern::Slice { inner, .. }
        | GraphPattern::OrderBy { inner, .. } => find_expr(inner),
        _ => None,
    }
}

pub fn parse_expression(text: &str) -> Expression {
    let q = format!("{PREFIXES} SELECT ({text} AS ?result_) WHERE {{}}");
    let Query::Select { pattern, .. } = SparqlParser::new().parse_query(&q).unwrap_or_else(|e| panic!("{text}: {e}"))
    else {
        unreachable!()
    };
    find_expr(&pattern).cloned().expect("expression")
}

/// Expression-level test fixture.
pub struct Harness {
    pub env: TestEnv,
    pub low: Lowering,
}

impl Default for Harness {
    fn default() -> Self {
        Self::new()
    }
}

impl Harness {
    pub fn new() -> Self {
        Harness { env: TestEnv::new(), low: Lowering::default() }
    }

    /// Stores a term written in SPARQL syntax (`"1"^^xsd:integer`, `<http://x>`, `"a"@en`)
    /// and returns its canonical id. `_:x` gives a fresh store blank node.
    pub fn term(&mut self, text: &str) -> TermId {
        if text.starts_with("_:") {
            return self.env.resolver.bnode();
        }
        match parse_expression(text) {
            Expression::NamedNode(n) => self.env.resolver.iri(n.as_str()),
            Expression::Literal(l) => self.env.resolver.add(term_of(l.as_ref().into())),
            // `-2` parses as a unary minus in expression syntax.
            Expression::UnaryMinus(inner) => match *inner {
                Expression::Literal(l) => {
                    let neg = oxrdf::Literal::new_typed_literal(format!("-{}", l.value()), l.datatype().into_owned());
                    self.env.resolver.add(term_of(neg.as_ref().into()))
                }
                other => panic!("not a term: -{other}"),
            },
            other => panic!("not a term: {other}"),
        }
    }

    pub fn compile(&mut self, text: &str) -> CompiledExpr {
        let e = parse_expression(text);
        let e = self.low.lower(&mut self.env, &e);
        compile(&e, &self.env.resolver.temps).expect("compile")
    }

    pub fn row(&mut self, binds: &[(&str, TermId)]) -> Vec<TermId> {
        let slots: Vec<(Slot, TermId)> = binds.iter().map(|(n, id)| (self.low.slot(n), *id)).collect();
        let mut row = vec![TermId::UNBOUND; self.low.vars.len().max(1)];
        for (s, id) in slots {
            row[s as usize] = id;
        }
        row
    }

    /// Evaluates `text` with the bindings; canonical ids.
    pub fn eval(&mut self, text: &str, binds: &[(&str, TermId)]) -> Option<TermId> {
        let c = self.compile(text);
        let row = self.row(binds);
        self.env.with(|cx| c.eval(&row, cx, true)).expect("fatal error")
    }

    /// Evaluates and renders the result compactly ("ERROR" for errors / unbound).
    pub fn show(&mut self, text: &str, binds: &[(&str, TermId)]) -> String {
        let r = self.eval(text, binds);
        self.render(r)
    }

    pub fn render(&mut self, id: Option<TermId>) -> String {
        use sq_core::eval_api::TermResolver;
        match id {
            None => "ERROR".into(),
            Some(id) => compact(&self.env.resolver.term(id).expect("decode")),
        }
    }

    /// FILTER semantics: the EBV (None = error).
    pub fn ebv(&mut self, text: &str, binds: &[(&str, TermId)]) -> Option<bool> {
        let c = self.compile(text);
        let row = self.row(binds);
        self.env.with(|cx| c.ebv(&row, cx)).expect("fatal error")
    }

    /// EBV with EXISTS answered by `exists`.
    pub fn ebv_with_exists(
        &mut self,
        text: &str,
        binds: &[(&str, TermId)],
        exists: impl FnMut(ExistsId, &[TermId]) -> bool,
    ) -> Option<bool> {
        let c = self.compile(text);
        let row = self.row(binds);
        let mut ex = FnExists(exists);
        self.env.with_exists(&mut ex, |cx| c.ebv(&row, cx)).expect("fatal error")
    }
}
