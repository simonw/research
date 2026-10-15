//! Expression evaluation (§6.2, §6.6). Every node is evaluated either as a term (`eval`,
//! returning a lazy `Val`) or as a boolean (`test`, used by FILTER via `ebv` and by the
//! boolean operators). Errors are `Ok(None)` and propagate except through `||`, `&&`,
//! COALESCE, IF's branches, IN/NOT IN, BOUND and EXISTS (§6.2).
use std::sync::Arc;

use oxsdatatypes::{DayTimeDuration, Double, TimezoneOffset};
use rustc_hash::{FxHashMap, FxHashSet};
use sq_core::codec::{self, RDF_LANG_STRING, RDF_LANG_STRING_IRI, XSD_STRING, XSD_STRING_IRI};
use sq_core::{Kind, Literal, Result, Tag, Term, TermId};

use crate::EvalCtx;
use crate::cast::{Casted, cast};
use crate::compare::{self, Order, ambiguous_identity, effective_tag, equals, same_term, same_term_ids};
use crate::compile::{Builtin, CmpOp, CompiledExpr, ConstRegex, IntOp, Node, RegexSrc};
use crate::numeric::{self, Num};
use crate::regex::CompiledRegex;
use crate::sortkey::{SortKey, sort_key_val};
use crate::strings::{self, StrArg};
use crate::value::{Val, Value, id_of_val, is_present, kind_of_val, term_of_id, value_of, value_of_val};
use crate::xsd::well_known_iri_id;

/// Unwraps an `Option` inside `Result`: `None` (a SPARQL error) returns `Ok(None)`.
macro_rules! tri {
    ($e:expr) => {
        match $e? {
            Some(v) => v,
            None => return Ok(None),
        }
    };
}

/// Unwraps an `Option`: `None` (a SPARQL error) returns `Ok(None)`.
macro_rules! opt {
    ($e:expr) => {
        match $e {
            Some(v) => v,
            None => return Ok(None),
        }
    };
}

/// The empty simple literal (inline SHORTSTR of length 0).
const EMPTY_STR: TermId = TermId(Tag::ShortStr as i64);

impl CompiledExpr {
    /// Ok(None) = SPARQL error or unbound; Err only for fatal errors (I/O, interrupt, memory).
    /// `canonical = true` returns the canonical id of the result (§4.3); pass-through
    /// expressions (a variable, COALESCE, IF) return the argument's id unchanged.
    pub fn eval(&self, row: &[TermId], cx: &mut EvalCtx, canonical: bool) -> Result<Option<TermId>> {
        match eval(&self.root, row, cx, canonical)? {
            None => Ok(None),
            Some(v) => id_of_val(v, cx, canonical).map(Some),
        }
    }

    /// Effective boolean value (FILTER: `Some(true)` keeps the row).
    pub fn ebv(&self, row: &[TermId], cx: &mut EvalCtx) -> Result<Option<bool>> {
        test(&self.root, row, cx)
    }

    /// The result as a lazy value, without interning computed terms.
    pub fn eval_val(&self, row: &[TermId], cx: &mut EvalCtx) -> Result<Option<Val>> {
        eval(&self.root, row, cx, false)
    }

    /// ORDER BY key of the result; an error or unbound result gets the lowest key.
    /// Computed values are keyed without being interned.
    pub fn sort_key(&self, row: &[TermId], cx: &mut EvalCtx) -> Result<SortKey> {
        match eval(&self.root, row, cx, false)? {
            None => Ok(SortKey::lowest()),
            Some(v) => sort_key_val(&v, cx),
        }
    }

    /// The slot if the expression is a bare variable (pass-through BIND, ORDER BY ?x).
    pub fn as_slot(&self) -> Option<sq_core::Slot> {
        match self.root {
            Node::Slot(s) => Some(s),
            _ => None,
        }
    }

    /// The id if the expression is a constant.
    pub fn as_const(&self) -> Option<TermId> {
        match self.root {
            Node::Const(id, _) => Some(id),
            _ => None,
        }
    }
}

#[inline]
fn slot(row: &[TermId], s: sq_core::Slot) -> Option<TermId> {
    row.get(s as usize).copied().filter(|id| is_present(*id))
}

/// Evaluates an operand: variables and constants are resolved inline (the common case),
/// anything else through `eval`.
#[inline(always)]
fn operand(n: &Node, row: &[TermId], cx: &mut EvalCtx) -> Result<Option<Val>> {
    match n {
        Node::Slot(s) => Ok(slot(row, *s).map(Val::Id)),
        Node::Const(id, _) => Ok(Some(Val::Id(*id))),
        _ => eval(n, row, cx, false),
    }
}

/// Term evaluation of a node.
pub(crate) fn eval(n: &Node, row: &[TermId], cx: &mut EvalCtx, canonical: bool) -> Result<Option<Val>> {
    Ok(Some(match n {
        Node::Const(id, _) => Val::Id(*id),
        Node::Slot(s) => Val::Id(opt!(slot(row, *s))),
        Node::Arith(op, a, b) => {
            let x = tri!(num(a, row, cx));
            let y = tri!(num(b, row, cx));
            Val::V(opt!(numeric::arith(*op, x, y)).into())
        }
        Node::Neg(a) => Val::V(opt!(numeric::neg(tri!(num(a, row, cx)))).into()),
        Node::Plus(a) => {
            let v = tri!(eval(a, row, cx, false));
            match v {
                Val::Id(id) if matches!(id.tag(), Tag::Int | Tag::Decimal) => v,
                v => Val::V(opt!(value_of_val(v, cx)?.as_num()).into()),
            }
        }
        Node::If(c, a, b) => match test(c, row, cx)? {
            None => return Ok(None),
            Some(true) => return eval(a, row, cx, canonical),
            Some(false) => return eval(b, row, cx, canonical),
        },
        Node::Coalesce(list) => {
            for e in list {
                if let Some(v) = eval(e, row, cx, canonical)? {
                    return Ok(Some(v));
                }
            }
            return Ok(None);
        }
        Node::Replace { text, src, replacement } => {
            let text = tri!(string_arg(text, row, cx));
            let repl = tri!(simple_string(replacement, row, cx));
            let re = tri!(regex(src, row, cx));
            Val::V(text.with_same_tag(&opt!(re.replace(&text.s, &repl))))
        }
        Node::Cast(t, a) => {
            let v = tri!(eval(a, row, cx, false));
            if *t == crate::xsd::XsdType::String
                && let Val::Id(id) = v
                && matches!(id.tag(), Tag::ShortStr | Tag::Str)
            {
                return Ok(Some(v));
            }
            match opt!(cast(*t, &value_of_val(v, cx)?)) {
                Casted::V(v) => Val::V(v),
                Casted::DerivedInteger(i, dt) => {
                    Val::Id(cx.resolver.intern(&Term::Literal(Literal::typed(&i.to_string(), dt.iri())), canonical)?)
                }
            }
        }
        Node::Call(b, args) => return call(*b, args, row, cx, canonical),
        Node::Fail => return Ok(None),
        Node::Bound(_)
        | Node::Exists(_)
        | Node::Not(_)
        | Node::Or(..)
        | Node::And(..)
        | Node::Equal(..)
        | Node::SameTerm(..)
        | Node::Compare(..)
        | Node::IntCmp { .. }
        | Node::IdEq { .. }
        | Node::InIds { .. }
        | Node::TagIs(..)
        | Node::LangEq { .. }
        | Node::LangMatchesConst { .. }
        | Node::In(..)
        | Node::Regex { .. } => Val::Id(TermId::from_bool(tri!(test(n, row, cx)))),
    }))
}

/// Boolean evaluation of a node (EBV for term-valued nodes).
pub(crate) fn test(n: &Node, row: &[TermId], cx: &mut EvalCtx) -> Result<Option<bool>> {
    Ok(match n {
        Node::Bound(s) => Some(slot(row, *s).is_some()),
        Node::Exists(id) => Some(cx.exists.exists(*id, row)?),
        Node::Not(a) => test(a, row, cx)?.map(|b| !b),
        Node::Or(a, b) => match test(a, row, cx)? {
            Some(true) => Some(true),
            l => match (l, test(b, row, cx)?) {
                (_, Some(true)) => Some(true),
                (Some(false), Some(false)) => Some(false),
                _ => None,
            },
        },
        Node::And(a, b) => match test(a, row, cx)? {
            Some(false) => Some(false),
            l => match (l, test(b, row, cx)?) {
                (_, Some(false)) => Some(false),
                (Some(true), Some(true)) => Some(true),
                _ => None,
            },
        },
        Node::Equal(a, b) => {
            let x = tri!(operand(a, row, cx));
            let y = tri!(operand(b, row, cx));
            equals(&x, &y, cx)?
        }
        Node::SameTerm(a, b) => {
            let x = tri!(operand(a, row, cx));
            let y = tri!(operand(b, row, cx));
            Some(same_term(x, y, cx)?)
        }
        Node::Compare(op, a, b) if is_numeric_node(a) || is_numeric_node(b) => {
            // numeric vs non-numeric is a type error, exactly what `num` failing gives.
            let x = tri!(num(a, row, cx));
            let y = tri!(num(b, row, cx));
            Some(apply_cmp(*op, numeric::compare(x, y).into()))
        }
        Node::Compare(op, a, b) => {
            let x = tri!(operand(a, row, cx));
            let y = tri!(operand(b, row, cx));
            compare::compare(&x, &y, cx)?.map(|o| apply_cmp(*op, o))
        }
        Node::IntCmp { slot: s, op, k, konst } => {
            let id = opt!(slot(row, *s));
            match (id.as_int(), op) {
                (Some(v), IntOp::Eq) => Some(v == *k),
                (Some(v), IntOp::Cmp(c)) => Some(apply_cmp(*c, v.cmp(k).into())),
                (None, IntOp::Eq) => equals(&Val::Id(id), &Val::Id(*konst), cx)?,
                (None, IntOp::Cmp(c)) => compare::compare(&Val::Id(id), &Val::Id(*konst), cx)?.map(|o| apply_cmp(*c, o)),
            }
        }
        Node::IdEq { slot: s, id: c } => {
            let x = opt!(slot(row, *s));
            id_eq(x, *c, cx)?
        }
        Node::InIds { slot: s, ids } => {
            let x = opt!(slot(row, *s));
            if ids.contains(&x) {
                Some(true)
            } else {
                // IRI constants: `=` is false for every other term (no error), except that a
                // TEMP id may denote a dictionary IRI.
                let mut found = false;
                for c in ids.iter() {
                    if ambiguous_identity(x, *c) && same_term_ids(x, *c, cx)? {
                        found = true;
                        break;
                    }
                }
                Some(found)
            }
        }
        Node::TagIs(a, kind) => Some(kind_of_val(&tri!(operand(a, row, cx))) == *kind),
        Node::LangEq { slot: s, lang } => {
            let x = opt!(slot(row, *s));
            match effective_tag(x) {
                Tag::Iri | Tag::BNode => None,
                Tag::Lang => match value_of(x, cx)? {
                    Value::LangStr { lang: l, .. } => Some(*l == **lang),
                    _ => None,
                },
                _ => Some(lang.is_empty()),
            }
        }
        Node::LangMatchesConst { arg, range } => {
            let tag = tri!(simple_string(arg, row, cx));
            Some(strings::lang_matches(&tag, range))
        }
        Node::In(a, list) => {
            let x = tri!(operand(a, row, cx));
            let mut error = false;
            for e in list {
                match operand(e, row, cx)? {
                    None => error = true,
                    Some(y) => match equals(&x, &y, cx)? {
                        Some(true) => return Ok(Some(true)),
                        Some(false) => {}
                        None => error = true,
                    },
                }
            }
            if error { None } else { Some(false) }
        }
        Node::Regex { text, src } => {
            let text = tri!(string_arg(text, row, cx));
            let re = tri!(regex(src, row, cx));
            Some(re.is_match(&text.s))
        }
        Node::Const(..)
        | Node::Slot(_)
        | Node::Arith(..)
        | Node::Neg(_)
        | Node::Plus(_)
        | Node::If(..)
        | Node::Coalesce(_)
        | Node::Replace { .. }
        | Node::Cast(..)
        | Node::Call(..)
        | Node::Fail => {
            let v = tri!(eval(n, row, cx, false));
            compare::ebv(&v, cx)?
        }
    })
}

#[inline]
fn apply_cmp(op: CmpOp, o: Order) -> bool {
    matches!(
        (op, o),
        (CmpOp::Lt, Order::Less)
            | (CmpOp::Le, Order::Less | Order::Equal)
            | (CmpOp::Gt, Order::Greater)
            | (CmpOp::Ge, Order::Greater | Order::Equal)
    )
}

/// `?x = c` for an IRI or simple-literal constant `c`, decided from tags whenever possible.
fn id_eq(x: TermId, c: TermId, cx: &mut EvalCtx) -> Result<Option<bool>> {
    if x == c {
        return Ok(Some(true));
    }
    if ambiguous_identity(x, c) {
        return equals(&Val::Id(x), &Val::Id(c), cx);
    }
    let c_is_iri = effective_tag(c) == Tag::Iri;
    Ok(match effective_tag(x) {
        // Typed literals may be Other / IllTyped: an error against a simple literal.
        Tag::Typed if !c_is_iri => return equals(&Val::Id(x), &Val::Id(c), cx),
        // Distinct canonical ids of IRIs / simple literals are distinct terms; every other
        // class is unequal to an IRI or a simple literal.
        _ => Some(false),
    })
}

/// Number of an id (inline integers without decoding).
#[inline]
fn num_of_id(id: TermId, cx: &mut EvalCtx) -> Result<Option<Num>> {
    if let Some(i) = id.as_int() {
        return Ok(Some(Num::Integer(i)));
    }
    Ok(value_of(id, cx)?.as_num())
}

/// Evaluates a node as a number. Arithmetic subtrees are evaluated directly on `Num`s,
/// without materializing intermediate values.
fn num(n: &Node, row: &[TermId], cx: &mut EvalCtx) -> Result<Option<Num>> {
    match n {
        Node::Slot(s) => num_of_id(opt!(slot(row, *s)), cx),
        Node::Const(_, Some(v)) => Ok(v.as_num()),
        Node::Const(id, None) => num_of_id(*id, cx),
        Node::Arith(op, a, b) => {
            let x = tri!(num(a, row, cx));
            let y = tri!(num(b, row, cx));
            Ok(numeric::arith(*op, x, y))
        }
        Node::Neg(a) => Ok(numeric::neg(tri!(num(a, row, cx)))),
        _ => {
            let v = tri!(eval(n, row, cx, false));
            match v {
                Val::Id(id) => num_of_id(id, cx),
                Val::V(v) => Ok(v.as_num()),
            }
        }
    }
}

/// Nodes whose value is always numeric (or an error): a comparison involving one of them is
/// either numeric or a type error, so it can run on `Num`s directly.
fn is_numeric_node(n: &Node) -> bool {
    match n {
        Node::Arith(..) | Node::Neg(_) => true,
        Node::Const(_, Some(v)) => v.is_numeric(),
        Node::Call(b, _) => matches!(
            b,
            Builtin::Abs
                | Builtin::Ceil
                | Builtin::Floor
                | Builtin::Round
                | Builtin::StrLen
                | Builtin::Year
                | Builtin::Month
                | Builtin::Day
                | Builtin::Hours
                | Builtin::Minutes
                | Builtin::Seconds
                | Builtin::Rand
        ),
        _ => false,
    }
}

/// Evaluates a node to a value (pre-decoded constants need no lookup).
fn value_arg(n: &Node, row: &[TermId], cx: &mut EvalCtx) -> Result<Option<Value>> {
    if let Node::Const(_, Some(v)) = n {
        return Ok(Some(v.clone()));
    }
    let v = tri!(eval(n, row, cx, false));
    value_of_val(v, cx).map(Some)
}

/// Evaluates a node as a string literal (simple, xsd:string or language-tagged).
fn string_arg(n: &Node, row: &[TermId], cx: &mut EvalCtx) -> Result<Option<StrArg>> {
    Ok(StrArg::from_value(tri!(value_arg(n, row, cx))))
}

/// Evaluates a node as a simple literal / xsd:string.
fn simple_string(n: &Node, row: &[TermId], cx: &mut EvalCtx) -> Result<Option<Arc<str>>> {
    Ok(match tri!(value_arg(n, row, cx)) {
        Value::Str(s) => Some(s),
        _ => None,
    })
}

/// Simple-literal string of a constant id.
fn const_simple_string(id: TermId, cx: &mut EvalCtx) -> Result<Option<Arc<str>>> {
    Ok(match value_of(id, cx)? {
        Value::Str(s) => Some(s),
        _ => None,
    })
}

fn build_const_regex(c: &ConstRegex, cx: &mut EvalCtx) -> Result<Option<Arc<CompiledRegex>>> {
    let pattern = tri!(const_simple_string(c.pattern, cx));
    let flags = match c.flags {
        Some(f) => tri!(const_simple_string(f, cx)),
        None => "".into(),
    };
    Ok(crate::regex::compile(&pattern, &flags).map(Arc::new))
}

/// The compiled regex of a REGEX / REPLACE call (constant: compiled once; dynamic: cached
/// per (pattern, flags) in the ValueCache).
fn regex(src: &RegexSrc, row: &[TermId], cx: &mut EvalCtx) -> Result<Option<Arc<CompiledRegex>>> {
    match src {
        RegexSrc::Const(c) => {
            if let Some(r) = c.cell.get() {
                return Ok(r.clone());
            }
            let r = build_const_regex(c, cx)?;
            Ok(c.cell.get_or_init(|| r).clone())
        }
        RegexSrc::Dynamic { pattern, flags } => {
            let pattern = tri!(simple_string(pattern, row, cx));
            let flags = match flags {
                Some(f) => tri!(simple_string(f, row, cx)),
                None => "".into(),
            };
            Ok(cx.values.regexes.get(&pattern, &flags))
        }
    }
}

/// `BNODE(str)` scope: the same string gives the same blank node within one solution row,
/// across all expressions evaluated on it (e.g. several SELECT expressions), and a fresh one
/// for every other row. Rows are identified by their content with the blank nodes created by
/// `BNODE(str)` itself masked out, so a row extended by an earlier `BNODE(str)` binding still
/// belongs to the same solution.
#[derive(Default)]
pub(crate) struct BNodeScope {
    map: FxHashMap<(Box<[TermId]>, Arc<str>), TermId>,
    generated: FxHashSet<TermId>,
}

/// Entries kept before the scope is reset (bounded memory on long streams).
const BNODE_SCOPE_LIMIT: usize = 1 << 20;

fn scoped_bnode(row: &[TermId], label: Arc<str>, cx: &mut EvalCtx) -> TermId {
    let scope = &mut cx.values.bnodes;
    let masked: Box<[TermId]> =
        row.iter().map(|id| if scope.generated.contains(id) { TermId::UNBOUND } else { *id }).collect();
    let key = (masked, label);
    if let Some(id) = scope.map.get(&key) {
        return *id;
    }
    if scope.map.len() >= BNODE_SCOPE_LIMIT {
        scope.map.clear();
        scope.generated.clear();
    }
    let id = cx.resolver.fresh_bnode();
    let scope = &mut cx.values.bnodes;
    scope.generated.insert(id);
    scope.map.insert(key, id);
    id
}

/// Timezone of a date/time value: outer None = not a date/time type.
fn timezone_of(v: &Value) -> Option<Option<TimezoneOffset>> {
    Some(match v {
        Value::DateTime(x) => x.timezone_offset(),
        Value::Date(x) => x.timezone_offset(),
        Value::Time(x) => x.timezone_offset(),
        Value::GYear(x) => x.timezone_offset(),
        Value::GYearMonth(x) => x.timezone_offset(),
        Value::GMonth(x) => x.timezone_offset(),
        Value::GMonthDay(x) => x.timezone_offset(),
        Value::GDay(x) => x.timezone_offset(),
        _ => return None,
    })
}

fn random_f64(cx: &mut EvalCtx) -> f64 {
    (cx.rng.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

/// RFC 4122 version 4 UUID from the execution's RNG.
fn uuid_v4(cx: &mut EvalCtx) -> String {
    let mut b = [0u8; 16];
    cx.rng.fill_bytes(&mut b);
    b[6] = (b[6] & 0x0F) | 0x40;
    b[8] = (b[8] & 0x3F) | 0x80;
    let mut s = String::with_capacity(36);
    for (i, byte) in b.iter().enumerate() {
        if matches!(i, 4 | 6 | 8 | 10) {
            s.push('-');
        }
        s.push_str(&format!("{byte:02x}"));
    }
    s
}

/// Datatype IRI as a term: the constant well-known id when there is one.
fn datatype_val(iri: &str) -> Val {
    match well_known_iri_id(iri) {
        Some(id) => Val::Id(id),
        None => Val::V(Value::Iri(iri.into())),
    }
}

/// STR of a lazy value: the stored lexical form (simple literals pass through).
pub(crate) fn str_of(v: Val, cx: &mut EvalCtx) -> Result<Option<Val>> {
    Ok(Some(match v {
        Val::Id(id) => match effective_tag(id) {
            Tag::ShortStr | Tag::Str => Val::Id(id),
            Tag::BNode => return Ok(None),
            _ if id.is_inline() => match codec::decode_inline(id) {
                Some(Term::Literal(l)) => Val::V(Value::Str(l.lex)),
                _ => return Ok(None),
            },
            _ => match &*term_of_id(id, cx)? {
                Term::Iri(i) => Val::V(Value::Str(i.clone())),
                Term::Literal(l) => Val::V(Value::Str(l.lex.clone())),
                Term::BNode(_) => return Ok(None),
            },
        },
        Val::V(v) => match v {
            Value::BNode(_) => return Ok(None),
            Value::Str(_) => Val::V(v),
            v => Val::V(Value::Str(v.lexical().into())),
        },
    }))
}

/// Builtin dispatch. Arguments are evaluated left to right; any argument error is an error.
fn call(b: Builtin, args: &[Node], row: &[TermId], cx: &mut EvalCtx, canonical: bool) -> Result<Option<Val>> {
    macro_rules! arg {
        ($i:expr) => {
            tri!(eval(&args[$i], row, cx, false))
        };
    }
    macro_rules! value {
        ($i:expr) => {
            tri!(value_arg(&args[$i], row, cx))
        };
    }
    macro_rules! string {
        ($i:expr) => {
            tri!(string_arg(&args[$i], row, cx))
        };
    }
    let bool_val = |b: bool| Val::Id(TermId::from_bool(b));
    Ok(Some(match b {
        Builtin::Str => return str_of(arg!(0), cx),
        Builtin::Lang => match arg!(0) {
            Val::Id(id) => match effective_tag(id) {
                Tag::Iri | Tag::BNode => return Ok(None),
                Tag::Lang => match value_of(id, cx)? {
                    Value::LangStr { lang, .. } => Val::V(Value::Str(lang)),
                    _ => return Ok(None),
                },
                _ => Val::Id(EMPTY_STR),
            },
            Val::V(Value::Iri(_) | Value::BNode(_)) => return Ok(None),
            Val::V(Value::LangStr { lang, .. }) => Val::V(Value::Str(lang)),
            Val::V(_) => Val::Id(EMPTY_STR),
        },
        Builtin::LangMatches => {
            let tag = tri!(simple_string(&args[0], row, cx));
            let range = tri!(simple_string(&args[1], row, cx));
            bool_val(strings::lang_matches(&tag, &range))
        }
        Builtin::Datatype => match arg!(0) {
            Val::Id(id) => match (codec::inline_datatype(id), effective_tag(id)) {
                (Some(dt), _) => Val::Id(dt),
                (None, Tag::Str) => Val::Id(XSD_STRING),
                (None, Tag::Lang) => Val::Id(RDF_LANG_STRING),
                (None, Tag::Iri | Tag::BNode) => return Ok(None),
                _ => match &*term_of_id(id, cx)? {
                    Term::Literal(l) => datatype_val(&l.datatype),
                    _ => return Ok(None),
                },
            },
            Val::V(v) => datatype_val(opt!(v.datatype_iri())),
        },
        Builtin::Iri => match arg!(0) {
            v @ Val::Id(id) if id.kind() == Kind::Iri => v,
            v => match value_of_val(v, cx)? {
                Value::Iri(i) => Val::V(Value::Iri(i)),
                Value::Str(s) => {
                    let resolved = match cx.base_iri {
                        Some(base) => oxiri::Iri::parse(base).ok().and_then(|b| b.resolve(&s).ok()),
                        None => oxiri::Iri::parse(s.to_string()).ok(),
                    };
                    Val::V(Value::Iri(opt!(resolved).into_inner().into()))
                }
                _ => return Ok(None),
            },
        },
        Builtin::BNode => {
            if args.is_empty() {
                Val::Id(cx.resolver.fresh_bnode())
            } else {
                let label = tri!(simple_string(&args[0], row, cx));
                Val::Id(scoped_bnode(row, label, cx))
            }
        }
        Builtin::Rand => Val::V(Value::Double(Double::from(random_f64(cx)))),
        Builtin::Abs => Val::V(opt!(numeric::abs(tri!(num(&args[0], row, cx)))).into()),
        Builtin::Ceil => Val::V(opt!(numeric::ceil(tri!(num(&args[0], row, cx)))).into()),
        Builtin::Floor => Val::V(opt!(numeric::floor(tri!(num(&args[0], row, cx)))).into()),
        Builtin::Round => Val::V(opt!(numeric::round(tri!(num(&args[0], row, cx)))).into()),
        Builtin::Concat => {
            let mut out = String::new();
            // None = no argument yet; Some(None) = mixed or no tags.
            let mut lang: Option<Option<Arc<str>>> = None;
            for arg in args {
                let s = tri!(string_arg(arg, row, cx));
                out.push_str(&s.s);
                lang = Some(match lang {
                    None => s.lang,
                    Some(l) if l == s.lang => l,
                    Some(_) => None,
                });
            }
            match lang.flatten() {
                Some(lang) => Val::V(Value::LangStr { lex: out.into(), lang }),
                None => Val::V(Value::Str(out.into())),
            }
        }
        Builtin::SubStr => {
            let s = string!(0);
            let start = tri!(num(&args[1], row, cx)).to_f64();
            let len = match args.get(2) {
                Some(n) => Some(tri!(num(n, row, cx)).to_f64()),
                None => None,
            };
            Val::V(s.with_same_tag(strings::substring(&s.s, start, len)))
        }
        Builtin::StrLen => Val::V(Value::Integer(string!(0).s.chars().count() as i64)),
        Builtin::UCase => {
            let s = string!(0);
            Val::V(s.with_same_tag(&s.s.to_uppercase()))
        }
        Builtin::LCase => {
            let s = string!(0);
            Val::V(s.with_same_tag(&s.s.to_lowercase()))
        }
        Builtin::EncodeForUri => Val::V(Value::Str(strings::encode_for_uri(&string!(0).s).into())),
        Builtin::Contains | Builtin::StrStarts | Builtin::StrEnds | Builtin::StrBefore | Builtin::StrAfter => {
            let (a, p) = (string!(0), string!(1));
            if !strings::compatible(&a, &p) {
                return Ok(None);
            }
            let (s, pat): (&str, &str) = (&a.s, &p.s);
            match b {
                Builtin::Contains => bool_val(s.contains(pat)),
                Builtin::StrStarts => bool_val(s.starts_with(pat)),
                Builtin::StrEnds => bool_val(s.ends_with(pat)),
                // A match (even at position 0 or of the empty pattern) keeps arg1's tag; no
                // match gives the tagless empty string.
                Builtin::StrBefore => match s.find(pat) {
                    Some(i) => Val::V(a.with_same_tag(&s[..i])),
                    None => Val::Id(EMPTY_STR),
                },
                _ => match s.find(pat) {
                    Some(i) => Val::V(a.with_same_tag(&s[i + pat.len()..])),
                    None => Val::Id(EMPTY_STR),
                },
            }
        }
        Builtin::Year | Builtin::Month | Builtin::Day => {
            let (y, m, d) = match value!(0) {
                Value::DateTime(x) => (x.year(), x.month(), x.day()),
                Value::Date(x) => (x.year(), x.month(), x.day()),
                _ => return Ok(None),
            };
            Val::V(Value::Integer(match b {
                Builtin::Year => y,
                Builtin::Month => m.into(),
                _ => d.into(),
            }))
        }
        Builtin::Hours | Builtin::Minutes | Builtin::Seconds => {
            let (h, m, s) = match value!(0) {
                Value::DateTime(x) => (x.hour(), x.minute(), x.second()),
                Value::Time(x) => (x.hour(), x.minute(), x.second()),
                _ => return Ok(None),
            };
            Val::V(match b {
                Builtin::Hours => Value::Integer(h.into()),
                Builtin::Minutes => Value::Integer(m.into()),
                _ => Value::Decimal(s),
            })
        }
        Builtin::Timezone => {
            let tz = opt!(opt!(timezone_of(&value!(0))));
            Val::V(Value::DayTimeDuration(DayTimeDuration::from(tz)))
        }
        Builtin::Tz => match opt!(timezone_of(&value!(0))) {
            Some(tz) => Val::V(Value::Str(tz.to_string().into())),
            None => Val::Id(EMPTY_STR),
        },
        Builtin::Now => Val::V(Value::DateTime(cx.now)),
        Builtin::Uuid => Val::V(Value::Iri(format!("urn:uuid:{}", uuid_v4(cx)).into())),
        Builtin::StrUuid => Val::V(Value::Str(uuid_v4(cx).into())),
        Builtin::Hash(alg) => {
            let s = tri!(simple_string(&args[0], row, cx));
            Val::V(Value::Str(strings::hash_hex(alg, &s).into()))
        }
        Builtin::StrLang => {
            let lex = tri!(simple_string(&args[0], row, cx));
            let lang = tri!(simple_string(&args[1], row, cx));
            let lit = opt!(oxrdf::Literal::new_language_tagged_literal(&*lex, &*lang).ok());
            let lang = opt!(lit.language()).to_ascii_lowercase();
            Val::V(Value::LangStr { lex, lang: lang.into() })
        }
        Builtin::StrDt => {
            let lex = tri!(simple_string(&args[0], row, cx));
            let dt = match value!(1) {
                Value::Iri(i) => i,
                _ => return Ok(None),
            };
            match &*dt {
                XSD_STRING_IRI => Val::V(Value::Str(lex)),
                RDF_LANG_STRING_IRI => return Ok(None),
                // The exact lexical form and datatype are kept: intern the term.
                _ => Val::Id(cx.resolver.intern(&Term::Literal(Literal { lex, datatype: dt, lang: None }), canonical)?),
            }
        }
        Builtin::IsNumeric => match arg!(0) {
            Val::Id(id) => match effective_tag(id) {
                Tag::Int | Tag::Decimal => bool_val(true),
                Tag::Typed => bool_val(value_of(id, cx)?.is_numeric()),
                _ => bool_val(false),
            },
            Val::V(v) => bool_val(v.is_numeric()),
        },
    }))
}
