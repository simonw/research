//! SPARQL expressions → SQL.
//!
//! An expression is compiled either as a *condition* (SQL 1 / 0 / NULL, NULL = SPARQL error:
//! SQL's three-valued AND / OR / NOT then coincide with SPARQL's `&&` / `||` / `!`) or as a
//! *term* (an SQL expression yielding a `TermId`, NULL = unbound or error).
//!
//! Conditions in a [`Ctx::Truth`] context (a FILTER or OPTIONAL condition, through AND / OR)
//! only need to be true exactly when SPARQL's value is true, so false and error may be
//! conflated; that admits the cheap forms the spec asks for (`sparql_num` comparisons,
//! simple-literal equality by id). Under NOT, inside IF / COALESCE and for values
//! ([`Ctx::Exact`]) the error must survive, and the translation falls back to `sparql_eval`.
use std::collections::BTreeMap;
use std::rc::Rc;

use sq_core::ir::{Expr, ExistsId};
use sq_core::spargebra::algebra::Function;
use sq_core::{Result, Slot, Tag, TermId};

use crate::helpers::{FALSE_ID, INT_TAG, TRUE_ID, cast_name, function_name};
use crate::sql::Col;
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
            (Some(c), Some(o)) if c.nullable => {
                Some(Col { sql: format!("COALESCE({}, {})", c.sql, o.sql), nullable: o.nullable })
            }
            (Some(c), _) => Some(c.clone()),
            (None, Some(o)) => Some(o.clone()),
            (None, None) => None,
        }
    }
}

/// Comparison operators of the `<` family.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Cmp {
    Lt,
    Le,
    Gt,
    Ge,
}

impl Cmp {
    fn sql(self) -> &'static str {
        match self {
            Cmp::Lt => "<",
            Cmp::Le => "<=",
            Cmp::Gt => ">",
            Cmp::Ge => ">=",
        }
    }

    /// `c OP x` ⇔ `x mirror(OP) c`.
    fn mirror(self) -> Cmp {
        match self {
            Cmp::Lt => Cmp::Gt,
            Cmp::Le => Cmp::Ge,
            Cmp::Gt => Cmp::Lt,
            Cmp::Ge => Cmp::Le,
        }
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
            Expr::Slot(s) => match v.lookup(*s) {
                None => "NULL".to_owned(),
                Some(c) => format!("sparql_ebv({})", c.sql),
            },
            Expr::Or(a, b) => format!("({} OR {})", self.bool_expr(a, v, ctx)?, self.bool_expr(b, v, ctx)?),
            Expr::And(a, b) => format!("({} AND {})", self.bool_expr(a, v, ctx)?, self.bool_expr(b, v, ctx)?),
            Expr::Not(a) => format!("(NOT {})", self.bool_expr(a, v, Ctx::Exact)?),
            Expr::Equal(a, b) => self.equal(a, b, v, ctx)?,
            Expr::SameTerm(a, b) => {
                // Ids are canonical (computed terms are interned with canonical = true).
                let (a, b) = (self.term_expr(a, v)?, self.term_expr(b, v)?);
                format!("({} = {})", a.sql, b.sql)
            }
            Expr::Less(a, b) => self.compare(Cmp::Lt, a, b, v, ctx)?,
            Expr::LessOrEqual(a, b) => self.compare(Cmp::Le, a, b, v, ctx)?,
            Expr::Greater(a, b) => self.compare(Cmp::Gt, a, b, v, ctx)?,
            Expr::GreaterOrEqual(a, b) => self.compare(Cmp::Ge, a, b, v, ctx)?,
            Expr::In(a, list) => self.in_list(a, list, v, ctx)?,
            Expr::Bound(s) => match v.lookup(*s) {
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
            Expr::Slot(s) => match v.lookup(*s) {
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

    /// `a = b`: the `=` matrix (§6.3) with id fast paths.
    fn equal(&mut self, a: &Expr, b: &Expr, v: &View<'_>, ctx: Ctx) -> Result<String> {
        let (x, c) = if matches!(a, Expr::Const(_)) && !matches!(b, Expr::Const(_)) { (b, a) } else { (a, b) };
        let xt = self.term_expr(x, v)?;
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
                    "CASE WHEN {xs} = {pc} THEN 1 WHEN ({xs} & 15) IN ({}, {}) THEN {generic} WHEN {xs} IS NOT NULL \
                     THEN 0 END",
                    Tag::Typed as i64,
                    Tag::Temp as i64
                ),
                // Integers: same id ⇔ same value among inline ints; other numerics by value.
                Tag::Int if xt.simple => {
                    let slow = match ctx {
                        Ctx::Truth => format!("sparql_num({xs}) = {}", self.params.get(cid.as_int().expect("INT"))),
                        Ctx::Exact => generic,
                    };
                    format!("(({xs} & 15) = {INT_TAG} AND {xs} = {pc} OR ({xs} & 15) <> {INT_TAG} AND {slow})")
                }
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
                 WHEN {xs} = {cs} AND ({xs} & 15) NOT IN ({typed}, {temp}) THEN 1 \
                 WHEN ({xs} & 15) IN (1, 2) OR ({cs} & 15) IN (1, 2) THEN 0 \
                 ELSE {} = {TRUE_ID} END",
                eval_sql("=", &[xs, cs]),
                typed = Tag::Typed as i64,
                temp = Tag::Temp as i64,
            ));
        }
        Ok(format!("({} = {TRUE_ID})", eval_sql("=", &[xs, cs])))
    }

    /// `<`, `<=`, `>`, `>=` (§6.4) with the inline-int and inline-date fast paths.
    fn compare(&mut self, op: Cmp, a: &Expr, b: &Expr, v: &View<'_>, ctx: Ctx) -> Result<String> {
        let (x, c, op) =
            if matches!(a, Expr::Const(_)) && !matches!(b, Expr::Const(_)) { (b, a, op.mirror()) } else { (a, b, op) };
        let o = op.sql();
        let xt = self.term_expr(x, v)?;
        let xs = &xt.sql;
        if let Expr::Const(cid) = c {
            let pc = self.param(*cid);
            let generic = format!("{} = {TRUE_ID}", eval_sql(o, &[xs, &pc]));
            return Ok(match cid.as_int() {
                // §9: integers compare on the payload; every other numeric through sparql_num.
                Some(k) if xt.simple => {
                    let pk = self.params.get(k);
                    let slow = match ctx {
                        Ctx::Truth => format!("sparql_num({xs}) {o} {pk}"),
                        Ctx::Exact => generic,
                    };
                    format!("(({xs} & 15) = {INT_TAG} AND ({xs} >> 4) {o} {pk} OR ({xs} & 15) <> {INT_TAG} AND {slow})")
                }
                _ if xt.simple && matches!(cid.tag(), Tag::DateTime | Tag::Date) => {
                    // Same tag and timezone field ⇒ the payload orders like the values.
                    let mask = self.params.get(cid.raw() & 0xFFFF);
                    let pk = self.params.get(cid.raw() >> 16);
                    format!(
                        "(({xs} & 65535) = {mask} AND ({xs} >> 16) {o} {pk} \
                         OR ({xs} & 65535) <> {mask} AND {generic})"
                    )
                }
                _ if ctx == Ctx::Truth && cid.tag() == Tag::Decimal => {
                    format!("(sparql_num({xs}) {o} sparql_num({pc}))")
                }
                _ => format!("({generic})"),
            });
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

    /// `a IN (…)`: the OR of `=` (an error only if nothing matched and some `=` erred).
    fn in_list(&mut self, a: &Expr, list: &[Expr], v: &View<'_>, ctx: Ctx) -> Result<String> {
        if list.is_empty() {
            return Ok("0".to_owned());
        }
        // Constants whose `=` is plain id equality in this context (see `equal`).
        let by_id: Option<Vec<TermId>> = list
            .iter()
            .map(|e| match e {
                Expr::Const(id) => match const_tag(*id) {
                    Tag::Iri | Tag::Lang => Some(*id),
                    Tag::Str | Tag::ShortStr if ctx == Ctx::Truth => Some(*id),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        if let Some(ids) = by_id {
            let x = self.term_expr(a, v)?;
            let ids: Vec<String> = ids.into_iter().map(|id| self.param(id)).collect();
            return Ok(format!("({} IN ({}))", x.sql, ids.join(", ")));
        }
        let terms = list.iter().map(|item| self.equal(a, item, v, ctx)).collect::<Result<Vec<_>>>()?;
        Ok(format!("({})", terms.join(" OR ")))
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
                    _ => format!("(substr({s}, length({s}) - length({p}) + 1) = {p})"),
                })
            }
            _ => None,
        })
    }
}
