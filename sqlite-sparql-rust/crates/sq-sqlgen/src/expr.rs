//! SPARQL expressions → SQL.
//!
//! An expression is compiled either as a *condition* (SQL 1 / 0 / NULL, NULL = SPARQL error:
//! SQL's three-valued AND / OR / NOT then coincide with SPARQL's `&&` / `||` / `!`) or as a
//! *term* (an SQL expression yielding a `TermId`, NULL = unbound or error).
//!
//! Conditions in a [`Ctx::Truth`] context (a FILTER or OPTIONAL condition, through AND / OR)
//! only need to be true exactly when SPARQL's value is true, so false and error may be
//! conflated; that admits simple-literal equality by id. Under NOT, inside IF / COALESCE and
//! for values ([`Ctx::Exact`]) the error must survive, and the translation falls back to
//! `sparql_eval`. Comparisons with an inline numeric constant split on the row's tag (see
//! `numeric`) and are exact in both contexts.
//!
//! Only *simple* operands (columns, parameters) are repeated in the SQL text of a fast path;
//! any other operand appears once, so an expression's SQL is linear in its size.
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::rc::Rc;

use sq_core::codec::encode_short_str;
use sq_core::ir::{Expr, ExistsId};
use sq_core::spargebra::algebra::Function;
use sq_core::{Result, Slot, Tag, TermId};

use crate::helpers::{FALSE_ID, INT_TAG, TRUE_ID, cast_name, function_name};
use crate::numeric::{Cmp, DEC_TAG, DOTZERO_BIT, NumConst, RealOperand, TEMP_TAG, TYPED_TAG};
use crate::sql::{Col, and_all, or_all};
use crate::translate::{Env, Translator, unsupported};

/// How much of a condition's value matters.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Ctx {
    /// Only "is it true": false and error are interchangeable.
    Truth,
    /// True, false and error must all be preserved.
    Exact,
}

/// A term-valued SQL expression.
#[derive(Clone, Debug)]
pub(crate) struct TermSql {
    pub sql: String,
    pub nullable: bool,
    /// A column reference or parameter: cheap to repeat in the SQL text.
    pub simple: bool,
    /// Only ever a BOOL id (or NULL).
    pub boolean: bool,
}

impl TermSql {
    fn null() -> TermSql {
        TermSql { sql: "NULL".into(), nullable: true, simple: true, boolean: false }
    }

    fn computed(sql: String, boolean: bool) -> TermSql {
        TermSql { sql, nullable: true, simple: false, boolean }
    }
}

/// `alias.column` or `?N`.
pub(crate) fn is_simple_sql(s: &str) -> bool {
    let ident = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    match s.strip_prefix('?') {
        Some(n) => !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()),
        None => s.split_once('.').is_some_and(|(a, c)| ident(a) && ident(c)),
    }
}

/// The bindings an expression sees: the block's columns, then the EXISTS substitution.
pub(crate) struct View<'a> {
    pub cols: &'a BTreeMap<Slot, Col>,
    pub env: &'a Env,
}

impl<'a> View<'a> {
    pub fn new(cols: &'a BTreeMap<Slot, Col>, env: &'a Env) -> Self {
        View { cols, env }
    }

    /// The value of a slot; None when it is unbound in every row.
    pub fn lookup(&self, s: Slot) -> Option<Col> {
        match (self.cols.get(&s), self.env.outer.get(&s)) {
            // Unbound here but substituted by an enclosing EXISTS row.
            (Some(c), Some(o)) if c.nullable => Some(Col::coalesce(c, o, o.nullable)),
            (Some(c), _) => Some(c.clone()),
            (None, Some(o)) => Some(o.clone()),
            (None, None) => None,
        }
    }

    /// [`View::lookup`] for a slot whose SQL is about to be pasted into an expression: the
    /// caller has settled the block (`Translator::settle`), so the binding is plain.
    pub fn read(&self, s: Slot) -> Option<Col> {
        let c = self.lookup(s);
        debug_assert!(c.as_ref().is_none_or(|c| !c.computed), "computed binding of slot {s} read unsettled");
        c
    }
}

/// Builtins whose value is always a boolean.
fn returns_boolean(f: &Function) -> bool {
    matches!(
        f,
        Function::LangMatches
            | Function::Contains
            | Function::StrStarts
            | Function::StrEnds
            | Function::IsIri
            | Function::IsBlank
            | Function::IsLiteral
            | Function::IsNumeric
            | Function::Regex
    )
}

/// EBV of a constant known without decoding.
fn const_ebv(id: TermId) -> Option<bool> {
    match id.tag() {
        Tag::Bool => id.as_bool(),
        Tag::Int => id.as_int().map(|v| v != 0),
        Tag::ShortStr => Some((id.raw() >> 4) & 0xF != 0),
        _ => None,
    }
}

/// The dictionary tag a constant has (or would have, for TEMP ids).
fn const_tag(id: TermId) -> Tag {
    if id.is_temp() { id.temp_tag() } else { id.tag() }
}

/// Is `x = c` plain id equality for the constant c in this context (see `equal`)?
fn id_equality(c: TermId, ctx: Ctx) -> bool {
    match const_tag(c) {
        Tag::Iri | Tag::Lang => true,
        Tag::Str | Tag::ShortStr => ctx == Ctx::Truth,
        _ => false,
    }
}

/// A condition as a BOOL term id.
fn bool_term(cond: String, never_error: bool) -> TermSql {
    if never_error {
        let sql = format!("CASE WHEN {cond} THEN {TRUE_ID} ELSE {FALSE_ID} END");
        TermSql { sql, nullable: false, simple: false, boolean: true }
    } else {
        TermSql::computed(format!("CASE {cond} WHEN 1 THEN {TRUE_ID} WHEN 0 THEN {FALSE_ID} END"), true)
    }
}

/// EBV of a term as a condition.
fn ebv(t: &TermSql) -> String {
    if t.sql == "NULL" {
        "NULL".into()
    } else if t.boolean {
        format!("({} = {TRUE_ID})", t.sql)
    } else {
        format!("sparql_ebv({})", t.sql)
    }
}

/// Splits the items of `x IN (…)` into constants compared by id (see `equal`), inline numeric
/// constants (compared on the row's tag, for a simple x) and the rest.
fn partition_in(list: &[Expr], simple: bool, ctx: Ctx) -> (Vec<TermId>, Vec<(TermId, NumConst)>, Vec<&Expr>) {
    let (mut by_id, mut numeric, mut rest) = (Vec::new(), Vec::new(), Vec::new());
    for item in list {
        if let Expr::Const(id) = item {
            if id_equality(*id, ctx) {
                by_id.push(*id);
                continue;
            }
            if simple && let Some(n) = NumConst::of(*id) {
                numeric.push((*id, n));
                continue;
            }
        }
        rest.push(item);
    }
    (by_id, numeric, rest)
}

/// `sparql_eval('<op>', args…)`. `op` always comes from the static vocabulary of `helpers`.
fn eval_sql(op: &str, args: &[&str]) -> String {
    debug_assert!(!op.contains('\''), "operator names never need quoting");
    let mut sql = format!("sparql_eval('{op}'");
    for a in args {
        sql.push_str(", ");
        sql.push_str(a);
    }
    sql.push(')');
    sql
}

impl Translator<'_> {
    /// An expression as a condition (1 / 0 / NULL = error).
    pub(crate) fn bool_expr(&mut self, e: &Expr, v: &View<'_>, ctx: Ctx) -> Result<String> {
        Ok(match e {
            Expr::Const(id) => match const_ebv(*id) {
                Some(b) => (if b { "1" } else { "0" }).to_owned(),
                None => format!("sparql_ebv({})", self.param(*id)),
            },
            Expr::Slot(s) => match v.read(*s) {
                None => "NULL".to_owned(),
                Some(c) => format!("sparql_ebv({})", c.sql),
            },
            Expr::Or(..) => self.chain(e, true, v, ctx)?,
            Expr::And(..) => self.chain(e, false, v, ctx)?,
            Expr::Not(a) => format!("(NOT {})", self.bool_expr(a, v, Ctx::Exact)?),
            Expr::Equal(a, b) => self.equal(a, b, v, ctx)?,
            Expr::SameTerm(a, b) => {
                // Ids are canonical (computed terms are interned with canonical = true).
                let (a, b) = (self.term_expr(a, v)?, self.term_expr(b, v)?);
                format!("({} = {})", a.sql, b.sql)
            }
            Expr::Less(a, b) => self.compare(Cmp::Lt, a, b, v)?,
            Expr::LessOrEqual(a, b) => self.compare(Cmp::Le, a, b, v)?,
            Expr::Greater(a, b) => self.compare(Cmp::Gt, a, b, v)?,
            Expr::GreaterOrEqual(a, b) => self.compare(Cmp::Ge, a, b, v)?,
            Expr::In(a, list) => self.in_list(a, list, v, ctx)?,
            Expr::Bound(s) => match v.read(*s) {
                None => "0".to_owned(),
                Some(c) if !c.nullable => "1".to_owned(),
                Some(c) => format!("({} IS NOT NULL)", c.sql),
            },
            Expr::Exists(id) => self.exists(*id, v)?,
            Expr::Call(f, args) => match self.call_condition(f, args, v)? {
                Some(sql) => sql,
                None => ebv(&self.call_value(f, args, v)?),
            },
            Expr::Add(..)
            | Expr::Subtract(..)
            | Expr::Multiply(..)
            | Expr::Divide(..)
            | Expr::UnaryPlus(_)
            | Expr::UnaryMinus(_)
            | Expr::If(..)
            | Expr::Coalesce(_) => ebv(&self.term_expr(e, v)?),
        })
    }

    /// An expression as a term id (NULL = unbound or error).
    pub(crate) fn term_expr(&mut self, e: &Expr, v: &View<'_>) -> Result<TermSql> {
        Ok(match e {
            Expr::Const(id) => TermSql {
                sql: self.param(*id),
                nullable: false,
                simple: true,
                boolean: id.tag() == Tag::Bool,
            },
            Expr::Slot(s) => match v.read(*s) {
                None => TermSql::null(),
                Some(c) => TermSql { simple: is_simple_sql(&c.sql), nullable: c.nullable, sql: c.sql, boolean: false },
            },
            Expr::Bound(_) | Expr::Exists(_) => bool_term(self.bool_expr(e, v, Ctx::Exact)?, true),
            Expr::Or(..)
            | Expr::And(..)
            | Expr::Not(_)
            | Expr::Equal(..)
            | Expr::SameTerm(..)
            | Expr::Less(..)
            | Expr::LessOrEqual(..)
            | Expr::Greater(..)
            | Expr::GreaterOrEqual(..)
            | Expr::In(..) => bool_term(self.bool_expr(e, v, Ctx::Exact)?, false),
            Expr::Add(a, b) => self.arith("+", a, b, v)?,
            Expr::Subtract(a, b) => self.arith("-", a, b, v)?,
            Expr::Multiply(a, b) => self.arith("*", a, b, v)?,
            Expr::Divide(a, b) => self.arith("/", a, b, v)?,
            Expr::UnaryPlus(a) => self.eval_call("u+", std::slice::from_ref(a.as_ref()), v, false)?,
            Expr::UnaryMinus(a) => self.eval_call("u-", std::slice::from_ref(a.as_ref()), v, false)?,
            Expr::If(c, a, b) => {
                // An erroring condition makes IF an error: CASE yields NULL.
                let c = self.bool_expr(c, v, Ctx::Exact)?;
                let (a, b) = (self.term_expr(a, v)?, self.term_expr(b, v)?);
                TermSql::computed(
                    format!("CASE {c} WHEN 1 THEN {} WHEN 0 THEN {} END", a.sql, b.sql),
                    a.boolean && b.boolean,
                )
            }
            Expr::Coalesce(list) => match list.as_slice() {
                [] => TermSql::null(),
                [one] => self.term_expr(one, v)?,
                _ => {
                    let terms = list.iter().map(|x| self.term_expr(x, v)).collect::<Result<Vec<_>>>()?;
                    let sql = terms.iter().map(|t| t.sql.as_str()).collect::<Vec<_>>().join(", ");
                    TermSql {
                        sql: format!("COALESCE({sql})"),
                        nullable: terms.iter().all(|t| t.nullable),
                        simple: false,
                        boolean: terms.iter().all(|t| t.boolean),
                    }
                }
            },
            Expr::Call(f, args) => self.call_term(f, args, v)?,
        })
    }

    /// `[NOT] EXISTS`: the pattern translated with the current row as its substitution.
    fn exists(&mut self, id: ExistsId, v: &View<'_>) -> Result<String> {
        let q = self.q;
        let pat = q.exists.get(id.0 as usize).ok_or_else(|| sq_core::Error::internal("unknown EXISTS id"))?;
        let mut outer = (*v.env.outer).clone();
        for s in v.cols.keys() {
            if let Some(c) = v.lookup(*s) {
                outer.insert(*s, c);
            }
        }
        let env = Env { graph: v.env.graph, outer: Rc::new(outer) };
        let b = self.block(pat, &env)?;
        if b.empty {
            return Ok("0".to_owned());
        }
        Ok(format!("EXISTS (SELECT 1{}{})", b.from_sql(), b.where_sql()))
    }

    /// `a || b || …` / `a && b && …`: a whole chain as one balanced SQL disjunction /
    /// conjunction (SQL's three-valued OR / AND are associative), so long chains neither nest
    /// parentheses nor deepen the expression tree.
    fn chain(&mut self, e: &Expr, or: bool, v: &View<'_>, ctx: Ctx) -> Result<String> {
        fn operands<'e>(e: &'e Expr, or: bool, out: &mut Vec<&'e Expr>) {
            match (e, or) {
                (Expr::Or(a, b), true) | (Expr::And(a, b), false) => {
                    operands(a, or, out);
                    operands(b, or, out);
                }
                _ => out.push(e),
            }
        }
        let mut list = Vec::new();
        operands(e, or, &mut list);
        let conds = list.into_iter().map(|x| self.bool_expr(x, v, ctx)).collect::<Result<Vec<_>>>()?;
        Ok(if or { or_all(&conds) } else { format!("({})", and_all(&conds)) })
    }

    /// `a = b`: the `=` matrix (§6.3) with id fast paths.
    fn equal(&mut self, a: &Expr, b: &Expr, v: &View<'_>, ctx: Ctx) -> Result<String> {
        let (x, c) = if matches!(a, Expr::Const(_)) && !matches!(b, Expr::Const(_)) { (b, a) } else { (a, b) };
        let xt = self.term_expr(x, v)?;
        self.equal_to(&xt, c, v, ctx)
    }

    /// `x = c` for a translated left operand (a constant c gets the fast paths).
    fn equal_to(&mut self, xt: &TermSql, c: &Expr, v: &View<'_>, ctx: Ctx) -> Result<String> {
        let xs = &xt.sql;
        if let Expr::Const(cid) = c {
            let pc = self.param(*cid);
            let generic = format!("{} = {TRUE_ID}", eval_sql("=", &[xs, &pc]));
            return Ok(match const_tag(*cid) {
                // An IRI or a language-tagged literal equals only itself (ids are canonical) and
                // never errors against another term: id equality is exact.
                Tag::Iri | Tag::Lang => format!("({xs} = {pc})"),
                // A simple literal equals only itself, but errors against unknown datatypes.
                Tag::Str | Tag::ShortStr if ctx == Ctx::Truth => format!("({xs} = {pc})"),
                Tag::Str | Tag::ShortStr if xt.simple => format!(
                    "CASE WHEN {xs} = {pc} THEN 1 WHEN ({xs} & 15) IN ({TYPED_TAG}, {TEMP_TAG}) THEN {generic} \
                     WHEN {xs} IS NOT NULL THEN 0 END"
                ),
                Tag::Int | Tag::Decimal if xt.simple => match NumConst::of(*cid) {
                    Some(n) => self.numeric_in(xs, &[(*cid, n)]),
                    None => format!("({generic})"),
                },
                _ => format!("({generic})"),
            });
        }
        let ct = self.term_expr(c, v)?;
        let cs = &ct.sql;
        if xt.simple && ct.simple {
            // Same id ⇒ equal unless it may be a NaN (TYPED, or TEMP); IRI / blank node vs a
            // different id ⇒ false; everything else needs the values.
            return Ok(format!(
                "CASE WHEN {xs} IS NULL OR {cs} IS NULL THEN NULL \
                 WHEN {xs} = {cs} AND ({xs} & 15) NOT IN ({TYPED_TAG}, {TEMP_TAG}) THEN 1 \
                 WHEN ({xs} & 15) IN (1, 2) OR ({cs} & 15) IN (1, 2) THEN 0 \
                 ELSE {} = {TRUE_ID} END",
                eval_sql("=", &[xs, cs]),
            ));
        }
        Ok(format!("({} = {TRUE_ID})", eval_sql("=", &[xs, cs])))
    }

    /// `x = c1 OR … OR x = cn` for inline numeric constants, as one CASE on the row's tag: an
    /// inline integer or decimal equals a constant iff it has the same value, which is id
    /// equality among integers and `id | DOTZERO_BIT` equality among decimals; TYPED and TEMP
    /// rows need sq-expr (promotion, ill-typed values); every other term is unequal (§6.3).
    /// Exact in both contexts.
    fn numeric_in(&mut self, x: &str, consts: &[(TermId, NumConst)]) -> String {
        fn member(x: &str, list: &[String]) -> String {
            match list {
                [] => "0".to_owned(),
                [one] => format!("{x} = {one}"),
                _ => format!("{x} IN ({})", list.join(", ")),
            }
        }
        let ints: Vec<String> = consts.iter().filter_map(|(_, n)| n.int_id()).map(|id| self.param(id)).collect();
        let decs: Vec<String> = consts.iter().filter_map(|(_, n)| n.dec_key()).map(|k| self.params.get(k)).collect();
        let generic: Vec<String> = consts
            .iter()
            .map(|(id, _)| {
                let pc = self.param(*id);
                format!("{} = {TRUE_ID}", eval_sql("=", &[x, &pc]))
            })
            .collect();
        format!(
            "CASE WHEN ({x} & 15) = {INT_TAG} THEN {} WHEN ({x} & 15) = {DEC_TAG} THEN {} \
             WHEN ({x} & 15) IN ({TYPED_TAG}, {TEMP_TAG}) THEN {} WHEN {x} IS NOT NULL THEN 0 END",
            member(x, &ints),
            member(&format!("({x} | {DOTZERO_BIT})"), &decs),
            or_all(&generic),
        )
    }

    /// `<`, `<=`, `>`, `>=` (§6.4) with the inline-number and inline-date fast paths. Every
    /// form is exact (true / false / NULL = error), so the context does not matter.
    fn compare(&mut self, op: Cmp, a: &Expr, b: &Expr, v: &View<'_>) -> Result<String> {
        let (x, c, op) =
            if matches!(a, Expr::Const(_)) && !matches!(b, Expr::Const(_)) { (b, a, op.mirror()) } else { (a, b, op) };
        let o = op.sql();
        let xt = self.term_expr(x, v)?;
        let xs = &xt.sql;
        if let Expr::Const(cid) = c {
            let pc = self.param(*cid);
            let generic = format!("{} = {TRUE_ID}", eval_sql(o, &[xs, &pc]));
            if !xt.simple {
                return Ok(format!("({generic})"));
            }
            if let Some(n) = NumConst::of(*cid) {
                return Ok(self.numeric_compare(op, xs, n, &generic));
            }
            if matches!(cid.tag(), Tag::DateTime | Tag::Date) {
                // Same tag and timezone field ⇒ the payload orders like the values.
                let mask = self.params.get(cid.raw() & 0xFFFF);
                let pk = self.params.get(cid.raw() >> 16);
                return Ok(format!("CASE WHEN ({xs} & 65535) = {mask} THEN ({xs} >> 16) {o} {pk} ELSE {generic} END"));
            }
            return Ok(format!("({generic})"));
        }
        let ct = self.term_expr(c, v)?;
        let cs = &ct.sql;
        let generic = format!("{} = {TRUE_ID}", eval_sql(o, &[xs, cs]));
        if xt.simple && ct.simple {
            // Two inline integers order like their ids.
            return Ok(format!(
                "CASE WHEN ({xs} & 15) = {INT_TAG} AND ({cs} & 15) = {INT_TAG} THEN {xs} {o} {cs} ELSE {generic} END"
            ));
        }
        Ok(format!("({generic})"))
    }

    /// `x OP n` for an inline numeric constant n (§9's inline-int fast path, made exact): the
    /// INT payload against an integer bound, an inline decimal's `sparql_num` REAL where that
    /// comparison is exact, `sparql_eval` for TYPED / TEMP terms and the remaining decimals;
    /// any other term is not numeric, so the comparison is an error (NULL).
    fn numeric_compare(&mut self, op: Cmp, x: &str, n: NumConst, generic: &str) -> String {
        let o = op.sql();
        let bound = self.params.get(n.int_bound(op));
        let mut sql = format!("CASE WHEN ({x} & 15) = {INT_TAG} THEN ({x} >> 4) {o} {bound}");
        if let Some(r) = n.real_operand() {
            let value = match r {
                RealOperand::Int(k) => self.params.get(k),
                RealOperand::Ratio { m, pow10 } => {
                    format!("(CAST({} AS REAL) / {})", self.params.get(m), self.params.get(pow10))
                }
            };
            let lim = r.row_limit();
            let _ = write!(
                sql,
                " WHEN ({x} & 15) = {DEC_TAG} AND ({x} >> 9) BETWEEN -{lim} AND {lim} THEN sparql_num({x}) {o} {value}"
            );
        }
        let _ = write!(sql, " WHEN ({x} & 15) IN ({TYPED_TAG}, {DEC_TAG}, {TEMP_TAG}) THEN {generic} END");
        sql
    }

    /// `a IN (…)`: the OR of `=` (an error only if nothing matched and some `=` erred), with
    /// the id-comparable constants as one SQL `IN` list and the inline numerics as one CASE.
    /// When `a` is computed and more than one term compares it, a one-row derived table
    /// computes it once (`(SELECT <terms over l.v> FROM (SELECT a AS v LIMIT -1) AS l)`).
    fn in_list(&mut self, a: &Expr, list: &[Expr], v: &View<'_>, ctx: Ctx) -> Result<String> {
        if list.is_empty() {
            return Ok("0".to_owned());
        }
        let x = self.term_expr(a, v)?;
        let (by_id, _, rest) = partition_in(list, x.simple, ctx);
        if x.simple || usize::from(!by_id.is_empty()) + rest.len() <= 1 {
            return self.in_terms(a, &x, list, v, ctx);
        }
        let l = self.alias("l");
        let named = TermSql { sql: format!("{l}.v"), simple: true, ..x.clone() };
        let cond = self.in_terms(a, &named, list, v, ctx)?;
        Ok(format!("(SELECT {cond} FROM (SELECT {} AS v LIMIT -1) AS {l})", x.sql))
    }

    /// The terms of `a IN (…)` with `a` translated as `x`.
    fn in_terms(&mut self, a: &Expr, x: &TermSql, list: &[Expr], v: &View<'_>, ctx: Ctx) -> Result<String> {
        let (by_id, numeric, rest) = partition_in(list, x.simple, ctx);
        let mut terms = Vec::with_capacity(rest.len() + 2);
        if !by_id.is_empty() {
            let ids: Vec<String> = by_id.into_iter().map(|id| self.param(id)).collect();
            terms.push(format!("({} IN ({}))", x.sql, ids.join(", ")));
        }
        if !numeric.is_empty() {
            terms.push(self.numeric_in(&x.sql, &numeric));
        }
        for item in rest {
            // A constant `a` keeps the constant on the right, where the fast paths are.
            let term = match a {
                Expr::Const(_) => self.equal(a, item, v, ctx)?,
                _ => self.equal_to(x, item, v, ctx)?,
            };
            terms.push(term);
        }
        Ok(or_all(&terms))
    }

    /// `+ - * /`; `+ - *` of two inline integers stay in SQL when the result is inline.
    fn arith(&mut self, op: &'static str, a: &Expr, b: &Expr, v: &View<'_>) -> Result<TermSql> {
        let (at, bt) = (self.term_expr(a, v)?, self.term_expr(b, v)?);
        let generic = eval_sql(op, &[&at.sql, &bt.sql]);
        if op == "/" {
            return Ok(TermSql::computed(generic, false));
        }
        // Per operand: (tag check, integer value); an INT constant needs no check.
        let mut int_operand = |e: &Expr, t: &TermSql| -> Option<(Option<String>, String)> {
            match e {
                Expr::Const(id) => id.as_int().map(|k| (None, self.params.get(k))),
                _ if t.simple => Some((Some(format!("({} & 15) = {INT_TAG}", t.sql)), format!("({} >> 4)", t.sql))),
                _ => None,
            }
        };
        let (Some((ca, va)), Some((cb, vb))) = (int_operand(a, &at), int_operand(b, &bt)) else {
            return Ok(TermSql::computed(generic, false));
        };
        let r = format!("({va} {op} {vb})");
        let mut conds: Vec<String> = ca.into_iter().chain(cb).collect();
        // `?a * ?a` tests the tag once.
        conds.dedup();
        conds.push(format!("{r} BETWEEN {} AND {}", TermId::INT_MIN, TermId::INT_MAX));
        Ok(TermSql::computed(
            format!("CASE WHEN {} THEN ({r} << 4) | {INT_TAG} ELSE {generic} END", conds.join(" AND ")),
            false,
        ))
    }

    /// `sparql_eval(op, args…)`.
    fn eval_call(&mut self, op: &str, args: &[Expr], v: &View<'_>, boolean: bool) -> Result<TermSql> {
        let terms = args.iter().map(|a| self.term_expr(a, v)).collect::<Result<Vec<_>>>()?;
        let sqls: Vec<&str> = terms.iter().map(|t| t.sql.as_str()).collect();
        Ok(TermSql::computed(eval_sql(op, &sqls), boolean))
    }

    /// A builtin call as a term.
    fn call_term(&mut self, f: &Function, args: &[Expr], v: &View<'_>) -> Result<TermSql> {
        match self.call_condition(f, args, v)? {
            Some(cond) => Ok(bool_term(cond, false)),
            None => self.call_value(f, args, v),
        }
    }

    /// A builtin call through `sparql_eval` (casts by datatype IRI).
    fn call_value(&mut self, f: &Function, args: &[Expr], v: &View<'_>) -> Result<TermSql> {
        if let Function::Custom(iri) = f {
            // Casts; any other custom function is an evaluation error (unbound).
            return match cast_name(iri.as_str()) {
                Some(op) if args.len() == 1 => self.eval_call(op, args, v, false),
                _ => Ok(TermSql::null()),
            };
        }
        match function_name(f) {
            Some(name) => self.eval_call(name, args, v, returns_boolean(f)),
            None => Err(unsupported(format!("the {f} function (non-deterministic per row)"))),
        }
    }

    /// Builtins with a direct SQL condition: the kind tests on id tags, REGEX, and
    /// CONTAINS / STRSTARTS / STRENDS against a simple-literal constant (via `sparql_str`).
    fn call_condition(&mut self, f: &Function, args: &[Expr], v: &View<'_>) -> Result<Option<String>> {
        let tag_test = |t: &TermSql, test: &str, temp_test: &str| {
            let x = &t.sql;
            format!("(({x} & 15) {test} OR ({x} & 15) = {} AND (({x} >> 4) & 7) {temp_test})", Tag::Temp as i64)
        };
        Ok(match (f, args) {
            (Function::IsIri, [a]) => {
                let t = self.term_expr(a, v)?;
                t.simple.then(|| tag_test(&t, "= 1", "= 1"))
            }
            (Function::IsBlank, [a]) => {
                let t = self.term_expr(a, v)?;
                t.simple.then(|| tag_test(&t, "= 2", "= 2"))
            }
            (Function::IsLiteral, [a]) => {
                let t = self.term_expr(a, v)?;
                t.simple.then(|| tag_test(&t, "BETWEEN 3 AND 11", ">= 3"))
            }
            (Function::Regex, [_, _] | [_, _, _]) => {
                let terms = args.iter().map(|a| self.term_expr(a, v)).collect::<Result<Vec<_>>>()?;
                let sqls: Vec<&str> = terms.iter().map(|t| t.sql.as_str()).collect();
                Some(format!("sparql_regex({})", sqls.join(", ")))
            }
            (Function::Contains | Function::StrStarts | Function::StrEnds, [a, Expr::Const(c)])
                if matches!(const_tag(*c), Tag::Str | Tag::ShortStr) =>
            {
                let s = format!("sparql_str({})", self.term_expr(a, v)?.sql);
                let p = format!("sparql_str({})", self.param(*c));
                Some(match f {
                    Function::Contains => format!("(instr({s}, {p}) > 0)"),
                    Function::StrStarts => format!("(instr({s}, {p}) = 1)"),
                    // Every string ends with "" (`substr(s, -0)` would be all of s).
                    _ if Some(*c) == encode_short_str("") => format!("({s} IS NOT NULL)"),
                    _ => format!("(substr({s}, -length({p})) = {p})"),
                })
            }
            _ => None,
        })
    }
}
