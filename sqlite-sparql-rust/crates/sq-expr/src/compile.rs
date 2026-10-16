//! Expression compiler (§6.1): `sq_core::ir::Expr` → a node tree with pre-decoded constants,
//! lazily compiled constant regexes and the fast paths:
//!
//! * `IntCmp`: `?x op k` with an inline INT constant compares payloads (2 compares);
//! * `IdEq`: `?x = c` for an IRI or simple-literal constant compares ids;
//! * `InIds`: `?x IN (<a>, <b>, …)` over IRI constants is an id-set test;
//! * `TagIs`: isIRI / isBlank / isLiteral read the id tag (TEMP ids via `temp_tag`);
//! * `Bound`;
//! * `LangEq`: `LANG(?x) = "c"` reads only the cached language tag;
//! * `LangMatchesConst`: LANGMATCHES with a constant range;
//! * REGEX / REPLACE with constant pattern and flags compile once and check literal prefixes;
//!   a constant REPLACE replacement string is prepared once.
use std::cell::OnceCell;
use std::sync::Arc;

use sq_core::eval_api::TempDict;
use sq_core::ir::{ExistsId, Expr};
use sq_core::spargebra::algebra::Function;
use sq_core::{Error, Kind, Result, Slot, Tag, TermId, codec};

use crate::compare::effective_tag;
use crate::numeric::ArithOp;
use crate::regex::{CompiledRegex, Replacement};
use crate::strings::HashAlg;
use crate::value::{Value, inline_value};
use crate::xsd::XsdType;

/// Ordering operators.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum CmpOp {
    Lt,
    Le,
    Gt,
    Ge,
}

impl CmpOp {
    /// The operator with its operands swapped (`k < ?x` ⇔ `?x > k`).
    fn flip(self) -> CmpOp {
        match self {
            CmpOp::Lt => CmpOp::Gt,
            CmpOp::Le => CmpOp::Ge,
            CmpOp::Gt => CmpOp::Lt,
            CmpOp::Ge => CmpOp::Le,
        }
    }
}

/// `IntCmp` operator: equality or an ordering operator.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum IntOp {
    Eq,
    Cmp(CmpOp),
}

/// Builtins evaluated by `eval::call` (REGEX, REPLACE, casts and the special forms have
/// their own nodes).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Builtin {
    Str,
    Lang,
    LangMatches,
    Datatype,
    Iri,
    BNode,
    Rand,
    Abs,
    Ceil,
    Floor,
    Round,
    Concat,
    SubStr,
    StrLen,
    UCase,
    LCase,
    EncodeForUri,
    Contains,
    StrStarts,
    StrEnds,
    StrBefore,
    StrAfter,
    Year,
    Month,
    Day,
    Hours,
    Minutes,
    Seconds,
    Timezone,
    Tz,
    Now,
    Uuid,
    StrUuid,
    Hash(HashAlg),
    StrLang,
    StrDt,
    IsNumeric,
}

/// A constant pattern/flags pair compiled on first use (dictionary constants can only be
/// decoded at evaluation time). `None` inside the cell = invalid regex (always an error).
pub(crate) struct ConstRegex {
    pub pattern: TermId,
    pub flags: Option<TermId>,
    pub cell: OnceCell<Option<Arc<CompiledRegex>>>,
}

pub(crate) enum RegexSrc {
    Const(ConstRegex),
    Dynamic { pattern: Box<Node>, flags: Option<Box<Node>> },
}

pub(crate) enum Node {
    /// Constant id and, when it is available without I/O (inline, SHORTSTR or TEMP), its
    /// pre-decoded value (used by the value-consuming fast paths; the result is still the id).
    Const(TermId, Option<Value>),
    Slot(Slot),
    Bound(Slot),
    Exists(ExistsId),
    Not(Box<Node>),
    Or(Box<Node>, Box<Node>),
    And(Box<Node>, Box<Node>),
    Equal(Box<Node>, Box<Node>),
    SameTerm(Box<Node>, Box<Node>),
    Compare(CmpOp, Box<Node>, Box<Node>),
    /// `?slot op k` with `konst` the inline INT id of `k`.
    IntCmp { slot: Slot, op: IntOp, k: i64, konst: TermId },
    /// `?slot = c`, c an IRI or simple-literal constant.
    IdEq { slot: Slot, id: TermId },
    /// `?slot IN (…)` over IRI constants.
    InIds { slot: Slot, ids: Box<[TermId]> },
    TagIs(Box<Node>, Kind),
    /// `LANG(?slot) = "lang"`.
    LangEq { slot: Slot, lang: Box<str> },
    /// `LANGMATCHES(arg, "range")`.
    LangMatchesConst { arg: Box<Node>, range: Box<str> },
    In(Box<Node>, Vec<Node>),
    Arith(ArithOp, Box<Node>, Box<Node>),
    Neg(Box<Node>),
    Plus(Box<Node>),
    If(Box<Node>, Box<Node>, Box<Node>),
    Coalesce(Vec<Node>),
    Regex { text: Box<Node>, src: RegexSrc },
    /// `prepared` is set when the pattern, flags and replacement are all constant: the
    /// replacement is then prepared for the regex once (`None` inside = invalid).
    Replace { text: Box<Node>, src: RegexSrc, replacement: Box<Node>, prepared: Option<OnceCell<Option<Replacement>>> },
    Cast(XsdType, Box<Node>),
    Call(Builtin, Vec<Node>),
    /// Unknown function or wrong cast arity: always a SPARQL error.
    Fail,
}

/// Tree with pre-encoded/pre-parsed constants, compiled regexes and fast paths.
pub struct CompiledExpr {
    pub(crate) root: Node,
    /// The expression's TEMP constants, sorted. A TEMP constant is canonical (IR contract: its
    /// term is absent from the store), so a result equal to one is never re-interned.
    pub(crate) temp_consts: Box<[TermId]>,
}

impl CompiledExpr {
    /// True if `id` is one of the expression's TEMP constants (hence already canonical).
    #[inline]
    pub(crate) fn is_temp_const(&self, id: TermId) -> bool {
        id.is_temp() && self.temp_consts.binary_search(&id).is_ok()
    }

    /// Slots whose string the expression reads first on every row: the text of a REGEX or
    /// REPLACE, or the first argument of a string function, reached from the root through
    /// the operand each node evaluates first (`!`, the left operand of `&&`, `||`, `<`, `+`,
    /// IN, the condition of IF, the first COALESCE argument, a cast's argument, the
    /// non-constant side of `=` / sameTerm, nested string functions as in
    /// `REGEX(LCASE(?n), ..)`). Evaluating the expression decodes such a slot's value before
    /// anything can end the evaluation early, when it is a string literal: a STR, SHORTSTR or
    /// LANG id (any other id is a type error decided from the tag) that the `ValueCache` does
    /// not hold yet. A host evaluating a batch of rows can therefore decode the batch's
    /// dictionary ids of tag STR and LANG in these slots in one store call, without decoding
    /// anything the row-by-row evaluation would not.
    pub fn string_slots(&self) -> Vec<sq_core::Slot> {
        let mut out = Vec::new();
        leading_strings(&self.root, &mut out);
        out.sort_unstable();
        out.dedup();
        out
    }
}

/// See [`CompiledExpr::string_slots`].
fn leading_strings(n: &Node, out: &mut Vec<sq_core::Slot>) {
    match n {
        Node::Not(a)
        | Node::And(a, _)
        | Node::Or(a, _)
        | Node::If(a, _, _)
        | Node::Compare(_, a, _)
        | Node::In(a, _)
        | Node::Arith(_, a, _)
        | Node::Neg(a)
        | Node::Plus(a)
        | Node::Cast(_, a)
        | Node::LangMatchesConst { arg: a, .. } => leading_strings(a, out),
        // `=` and sameTerm evaluate the non-constant side of a constant comparison first.
        Node::Equal(a, b) | Node::SameTerm(a, b) => match (&**a, &**b) {
            (Node::Const(..), x) => leading_strings(x, out),
            (x, _) => leading_strings(x, out),
        },
        Node::Coalesce(list) => {
            if let Some(a) = list.first() {
                leading_strings(a, out);
            }
        }
        Node::Regex { text, .. } | Node::Replace { text, .. } => string_operand(text, out),
        Node::Call(
            Builtin::Concat
            | Builtin::SubStr
            | Builtin::StrLen
            | Builtin::UCase
            | Builtin::LCase
            | Builtin::EncodeForUri
            | Builtin::Contains
            | Builtin::StrStarts
            | Builtin::StrEnds
            | Builtin::StrBefore
            | Builtin::StrAfter,
            args,
        ) => {
            if let Some(a) = args.first() {
                string_operand(a, out);
            }
        }
        _ => {}
    }
}

/// An operand read with `string_arg`: a slot is decoded right away; anything else is
/// evaluated first.
fn string_operand(n: &Node, out: &mut Vec<sq_core::Slot>) {
    match n {
        Node::Slot(s) => out.push(*s),
        n => leading_strings(n, out),
    }
}

impl std::fmt::Debug for CompiledExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CompiledExpr")
    }
}

/// Compiles an expression. Constants are canonical ids; TEMP constants are decoded through
/// `temps` (the query's TempDict). Errors only for malformed IR (builtin arity).
pub fn compile(e: &Expr, temps: &TempDict) -> Result<CompiledExpr> {
    let mut c = Compiler { temps, temp_consts: Vec::new() };
    let root = c.node(e)?;
    let mut temp_consts = c.temp_consts;
    temp_consts.sort_unstable();
    temp_consts.dedup();
    Ok(CompiledExpr { root, temp_consts: temp_consts.into() })
}

struct Compiler<'a> {
    temps: &'a TempDict,
    /// TEMP constants that became `Node::Const` (the only constants a result can pass through).
    temp_consts: Vec<TermId>,
}

/// Simple-literal value of a constant when known at compile time (SHORTSTR or TEMP).
fn const_string(v: &Option<Value>) -> Option<&str> {
    match v {
        Some(Value::Str(s)) => Some(s),
        _ => None,
    }
}

impl Compiler<'_> {
    fn decode_const(&self, id: TermId) -> Option<Value> {
        if let Some(v) = inline_value(id) {
            return Some(v);
        }
        match id.tag() {
            Tag::ShortStr => codec::decode_short_str(id).map(|s| Value::Str(s.into())),
            Tag::Temp => self.temps.get(id).map(|t| Value::from_term(t)),
            _ => None,
        }
    }

    fn boxed(&mut self, e: &Expr) -> Result<Box<Node>> {
        Ok(Box::new(self.node(e)?))
    }

    fn nodes(&mut self, es: &[Expr]) -> Result<Vec<Node>> {
        es.iter().map(|e| self.node(e)).collect()
    }

    fn node(&mut self, e: &Expr) -> Result<Node> {
        Ok(match e {
            Expr::Const(id) => {
                if id.is_temp() {
                    self.temp_consts.push(*id);
                }
                Node::Const(*id, self.decode_const(*id))
            }
            Expr::Slot(s) => Node::Slot(*s),
            Expr::Bound(s) => Node::Bound(*s),
            Expr::Exists(id) => Node::Exists(*id),
            Expr::Not(a) => Node::Not(self.boxed(a)?),
            Expr::Or(a, b) => Node::Or(self.boxed(a)?, self.boxed(b)?),
            Expr::And(a, b) => Node::And(self.boxed(a)?, self.boxed(b)?),
            Expr::Equal(a, b) => self.equal(a, b)?,
            Expr::SameTerm(a, b) => Node::SameTerm(self.boxed(a)?, self.boxed(b)?),
            Expr::Less(a, b) => self.compare(CmpOp::Lt, a, b)?,
            Expr::LessOrEqual(a, b) => self.compare(CmpOp::Le, a, b)?,
            Expr::Greater(a, b) => self.compare(CmpOp::Gt, a, b)?,
            Expr::GreaterOrEqual(a, b) => self.compare(CmpOp::Ge, a, b)?,
            Expr::In(a, list) => self.in_list(a, list)?,
            Expr::Add(a, b) => Node::Arith(ArithOp::Add, self.boxed(a)?, self.boxed(b)?),
            Expr::Subtract(a, b) => Node::Arith(ArithOp::Sub, self.boxed(a)?, self.boxed(b)?),
            Expr::Multiply(a, b) => Node::Arith(ArithOp::Mul, self.boxed(a)?, self.boxed(b)?),
            Expr::Divide(a, b) => Node::Arith(ArithOp::Div, self.boxed(a)?, self.boxed(b)?),
            Expr::UnaryPlus(a) => Node::Plus(self.boxed(a)?),
            Expr::UnaryMinus(a) => Node::Neg(self.boxed(a)?),
            Expr::If(c, a, b) => Node::If(self.boxed(c)?, self.boxed(a)?, self.boxed(b)?),
            Expr::Coalesce(l) => Node::Coalesce(self.nodes(l)?),
            Expr::Call(f, args) => self.call(f, args)?,
        })
    }

    /// Inline INT constant payload.
    fn int_const(e: &Expr) -> Option<(i64, TermId)> {
        match e {
            Expr::Const(id) => id.as_int().map(|k| (k, *id)),
            _ => None,
        }
    }

    /// IRI or simple-literal constant (the ids `IdEq` may compare).
    fn id_eq_const(e: &Expr) -> Option<TermId> {
        match e {
            Expr::Const(id) if matches!(effective_tag(*id), Tag::Iri | Tag::Str | Tag::ShortStr) => Some(*id),
            _ => None,
        }
    }

    fn equal(&mut self, a: &Expr, b: &Expr) -> Result<Node> {
        for (x, y) in [(a, b), (b, a)] {
            if let Expr::Slot(s) = x {
                if let Some((k, konst)) = Self::int_const(y) {
                    return Ok(Node::IntCmp { slot: *s, op: IntOp::Eq, k, konst });
                }
                if let Some(id) = Self::id_eq_const(y) {
                    return Ok(Node::IdEq { slot: *s, id });
                }
            }
            if let (Expr::Call(Function::Lang, args), Expr::Const(c)) = (x, y)
                && let [Expr::Slot(s)] = args.as_slice()
                && let Some(lang) = const_string(&self.decode_const(*c))
            {
                return Ok(Node::LangEq { slot: *s, lang: lang.into() });
            }
        }
        Ok(Node::Equal(self.boxed(a)?, self.boxed(b)?))
    }

    fn compare(&mut self, op: CmpOp, a: &Expr, b: &Expr) -> Result<Node> {
        if let (Expr::Slot(s), Some((k, konst))) = (a, Self::int_const(b)) {
            return Ok(Node::IntCmp { slot: *s, op: IntOp::Cmp(op), k, konst });
        }
        if let (Some((k, konst)), Expr::Slot(s)) = (Self::int_const(a), b) {
            return Ok(Node::IntCmp { slot: *s, op: IntOp::Cmp(op.flip()), k, konst });
        }
        Ok(Node::Compare(op, self.boxed(a)?, self.boxed(b)?))
    }

    fn in_list(&mut self, a: &Expr, list: &[Expr]) -> Result<Node> {
        if let Expr::Slot(s) = a
            && !list.is_empty()
            && list.iter().all(|e| matches!(e, Expr::Const(id) if effective_tag(*id) == Tag::Iri))
        {
            let ids = list
                .iter()
                .map(|e| match e {
                    Expr::Const(id) => *id,
                    _ => unreachable!("checked above"),
                })
                .collect();
            return Ok(Node::InIds { slot: *s, ids });
        }
        Ok(Node::In(self.boxed(a)?, self.nodes(list)?))
    }

    fn regex_src(&mut self, pattern: &Expr, flags: Option<&Expr>) -> Result<RegexSrc> {
        Ok(match (pattern, flags) {
            (Expr::Const(p), None) => RegexSrc::Const(ConstRegex { pattern: *p, flags: None, cell: OnceCell::new() }),
            (Expr::Const(p), Some(Expr::Const(f))) => {
                RegexSrc::Const(ConstRegex { pattern: *p, flags: Some(*f), cell: OnceCell::new() })
            }
            _ => RegexSrc::Dynamic { pattern: self.boxed(pattern)?, flags: flags.map(|f| self.boxed(f)).transpose()? },
        })
    }

    fn call(&mut self, f: &Function, args: &[Expr]) -> Result<Node> {
        let arity = |min: usize, max: usize| -> Result<()> {
            if (min..=max).contains(&args.len()) {
                Ok(())
            } else {
                Err(Error::Invalid(format!("{f}: wrong number of arguments ({})", args.len())))
            }
        };
        let b = match f {
            Function::Str => Builtin::Str,
            Function::Lang => Builtin::Lang,
            Function::LangMatches => {
                arity(2, 2)?;
                if let Expr::Const(c) = &args[1]
                    && let Some(range) = const_string(&self.decode_const(*c))
                {
                    return Ok(Node::LangMatchesConst { arg: self.boxed(&args[0])?, range: range.into() });
                }
                Builtin::LangMatches
            }
            Function::Datatype => Builtin::Datatype,
            Function::Iri => Builtin::Iri,
            Function::BNode => {
                arity(0, 1)?;
                Builtin::BNode
            }
            Function::Rand => Builtin::Rand,
            Function::Abs => Builtin::Abs,
            Function::Ceil => Builtin::Ceil,
            Function::Floor => Builtin::Floor,
            Function::Round => Builtin::Round,
            Function::Concat => return Ok(Node::Call(Builtin::Concat, self.nodes(args)?)),
            Function::SubStr => {
                arity(2, 3)?;
                Builtin::SubStr
            }
            Function::StrLen => Builtin::StrLen,
            Function::Replace => {
                arity(3, 4)?;
                let src = self.regex_src(&args[1], args.get(3))?;
                let prepared =
                    (matches!(src, RegexSrc::Const(_)) && matches!(args[2], Expr::Const(_))).then(OnceCell::new);
                return Ok(Node::Replace {
                    text: self.boxed(&args[0])?,
                    src,
                    replacement: self.boxed(&args[2])?,
                    prepared,
                });
            }
            Function::UCase => Builtin::UCase,
            Function::LCase => Builtin::LCase,
            Function::EncodeForUri => Builtin::EncodeForUri,
            Function::Contains => Builtin::Contains,
            Function::StrStarts => Builtin::StrStarts,
            Function::StrEnds => Builtin::StrEnds,
            Function::StrBefore => Builtin::StrBefore,
            Function::StrAfter => Builtin::StrAfter,
            Function::Year => Builtin::Year,
            Function::Month => Builtin::Month,
            Function::Day => Builtin::Day,
            Function::Hours => Builtin::Hours,
            Function::Minutes => Builtin::Minutes,
            Function::Seconds => Builtin::Seconds,
            Function::Timezone => Builtin::Timezone,
            Function::Tz => Builtin::Tz,
            Function::Now => Builtin::Now,
            Function::Uuid => Builtin::Uuid,
            Function::StrUuid => Builtin::StrUuid,
            Function::Md5 => Builtin::Hash(HashAlg::Md5),
            Function::Sha1 => Builtin::Hash(HashAlg::Sha1),
            Function::Sha256 => Builtin::Hash(HashAlg::Sha256),
            Function::Sha384 => Builtin::Hash(HashAlg::Sha384),
            Function::Sha512 => Builtin::Hash(HashAlg::Sha512),
            Function::StrLang => Builtin::StrLang,
            Function::StrDt => Builtin::StrDt,
            Function::IsIri | Function::IsBlank | Function::IsLiteral => {
                arity(1, 1)?;
                let kind = match f {
                    Function::IsIri => Kind::Iri,
                    Function::IsBlank => Kind::BNode,
                    _ => Kind::Literal,
                };
                return Ok(Node::TagIs(self.boxed(&args[0])?, kind));
            }
            Function::IsNumeric => Builtin::IsNumeric,
            Function::Regex => {
                arity(2, 3)?;
                return Ok(Node::Regex { text: self.boxed(&args[0])?, src: self.regex_src(&args[1], args.get(2))? });
            }
            Function::Custom(name) => {
                return Ok(match XsdType::from_iri(name.as_str()) {
                    Some(t) if args.len() == 1 => Node::Cast(t, self.boxed(&args[0])?),
                    _ => Node::Fail,
                });
            }
        };
        let (min, max) = builtin_arity(b);
        arity(min, max)?;
        Ok(Node::Call(b, self.nodes(args)?))
    }
}

/// (min, max) argument counts of the builtins dispatched by `eval::call`.
fn builtin_arity(b: Builtin) -> (usize, usize) {
    match b {
        Builtin::Rand | Builtin::Now | Builtin::Uuid | Builtin::StrUuid => (0, 0),
        Builtin::BNode => (0, 1),
        Builtin::Concat => (0, usize::MAX),
        Builtin::SubStr => (2, 3),
        Builtin::LangMatches
        | Builtin::Contains
        | Builtin::StrStarts
        | Builtin::StrEnds
        | Builtin::StrBefore
        | Builtin::StrAfter
        | Builtin::StrLang
        | Builtin::StrDt => (2, 2),
        _ => (1, 1),
    }
}
