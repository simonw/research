//! The helper SQL functions the generated SQL calls, and the `sparql_eval` operator vocabulary.
//!
//! The baseline does the joins, filtering on ids, grouping and ordering in SQL; value semantics
//! (numeric promotion, the `=` matrix, EBV, ORDER BY keys, builtins, aggregates) come from
//! helper functions that sq-api registers on every connection and implements with sq-expr over
//! the store's dictionary. This module is the single source of truth for that contract:
//! [`HELPER_FUNCTIONS`] lists every function with its arity and semantics, and
//! [`eval_template`] maps a `sparql_eval` operator name back to the `sq_core::ir::Expr` it
//! stands for, so the implementation never re-derives the vocabulary.
//!
//! Conventions shared by every helper:
//! * A *term argument* is an INTEGER `TermId` or NULL (unbound). TEMP ids are those of the
//!   executing query: helpers decode them through the `LoweredQuery::temps` TempDict that the
//!   statement was translated from.
//! * A *term result* is a canonical id (inline if inline-encodable, the dictionary id if the
//!   term is stored, otherwise a TEMP id interned into that same TempDict) or NULL for a SPARQL
//!   error / unbound value.
//! * A *boolean result* is 1, 0 or NULL (error); SQL's three-valued AND / OR / NOT then match
//!   SPARQL's error semantics exactly.
use sq_core::Slot;
use sq_core::codec::{WELL_KNOWN, XSD};
use sq_core::ir::Expr;
use sq_core::oxrdf::NamedNode;
use sq_core::spargebra::algebra::Function;
use sq_core::{Tag, TermId};

/// `TermId` of `"true"^^xsd:boolean` (inline BOOL), written literally into generated SQL.
pub const TRUE_ID: i64 = TermId::from_bool(true).raw();
/// `TermId` of `"false"^^xsd:boolean`.
pub const FALSE_ID: i64 = TermId::from_bool(false).raw();
/// Tag bits of an inline integer (`id & 15`).
pub const INT_TAG: i64 = Tag::Int as i64;

/// Scalar or aggregate SQL function.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum HelperKind {
    Scalar,
    /// Aggregate; SQL `DISTINCT` is used only with the one-argument forms.
    Aggregate,
}

/// One helper SQL function the generated SQL may call.
#[derive(Copy, Clone, Debug)]
pub struct HelperFunction {
    pub name: &'static str,
    /// Accepted argument counts (inclusive); `usize::MAX` = variadic.
    pub min_args: usize,
    pub max_args: usize,
    pub kind: HelperKind,
    /// Exact semantics (what sq-api must implement).
    pub semantics: &'static str,
}

/// Every helper function the translator emits. None may be registered DETERMINISTIC: they
/// depend on the dictionary and on the executing query's TempDict.
pub const HELPER_FUNCTIONS: &[HelperFunction] = &[
    HelperFunction {
        name: "sparql_num",
        min_args: 1,
        max_args: 1,
        kind: HelperKind::Scalar,
        semantics: "Numeric value of a numeric literal: xsd:integer and the derived integer types \
                    -> INTEGER; xsd:decimal, xsd:float, xsd:double -> REAL (NaN -> NULL); NULL for \
                    every other term, ill-typed literals and NULL input.",
    },
    HelperFunction {
        name: "sparql_str",
        min_args: 1,
        max_args: 1,
        kind: HelperKind::Scalar,
        semantics: "Lexical form (TEXT) of a string literal: simple, xsd:string or \
                    language-tagged. NULL for IRIs, blank nodes, other literals and NULL input \
                    (it is the argument check of CONTAINS / STRSTARTS / STRENDS, not STR()).",
    },
    HelperFunction {
        name: "sparql_ebv",
        min_args: 1,
        max_args: 1,
        kind: HelperKind::Scalar,
        semantics: "Effective boolean value (SPEC 6.4): 1, 0, or NULL when the EBV is an error \
                    (including NULL input).",
    },
    HelperFunction {
        name: "sparql_sortkey",
        min_args: 1,
        max_args: 1,
        kind: HelperKind::Scalar,
        semantics: "The memcmp-ordered ORDER BY key of a term (sq_expr::SortKey bytes) as a BLOB; \
                    NULL input -> NULL (SQL sorts NULL first, matching SPARQL's unbound-lowest \
                    rule in both directions).",
    },
    HelperFunction {
        name: "sparql_regex",
        min_args: 2,
        max_args: 3,
        kind: HelperKind::Scalar,
        semantics: "REGEX(text, pattern[, flags]) on term arguments: 1, 0 or NULL (error). \
                    Equivalent to the EBV of sparql_eval('REGEX', ...); compiled regexes should \
                    be cached per (pattern, flags).",
    },
    HelperFunction {
        name: "sparql_eval",
        min_args: 1,
        max_args: usize::MAX,
        kind: HelperKind::Scalar,
        semantics: "sparql_eval(op, a1, ..., an): evaluates the operator or builtin named by the \
                    constant TEXT `op` on term arguments exactly like sq-expr evaluates \
                    `eval_template(op, n)` with slot i bound to a(i+1) (NULL = unbound) and \
                    canonical = true. Returns a term result (booleans as BOOL ids 23 / 7). \
                    Operators: = < <= > >= + - * / (binary), u+ u- (unary); builtins by SPARQL \
                    name (STR, LANG, ..., isIRI, REGEX); casts by the full XSD datatype IRI.",
    },
    HelperFunction {
        name: "sparql_sum",
        min_args: 1,
        max_args: 1,
        kind: HelperKind::Aggregate,
        semantics: "SUM aggregate: sq_expr::accumulator(SUM) fed add(Some(id)) per non-NULL row \
                    and add(None) per NULL row; finalize returns finish() (a fresh accumulator's \
                    finish() for an empty group).",
    },
    HelperFunction {
        name: "sparql_avg",
        min_args: 1,
        max_args: 1,
        kind: HelperKind::Aggregate,
        semantics: "AVG aggregate, same protocol as sparql_sum.",
    },
    HelperFunction {
        name: "sparql_min",
        min_args: 1,
        max_args: 1,
        kind: HelperKind::Aggregate,
        semantics: "MIN aggregate (SortKey order), same protocol as sparql_sum.",
    },
    HelperFunction {
        name: "sparql_max",
        min_args: 1,
        max_args: 1,
        kind: HelperKind::Aggregate,
        semantics: "MAX aggregate (SortKey order), same protocol as sparql_sum.",
    },
    HelperFunction {
        name: "sparql_sample",
        min_args: 1,
        max_args: 1,
        kind: HelperKind::Aggregate,
        semantics: "SAMPLE aggregate, same protocol as sparql_sum.",
    },
    HelperFunction {
        name: "sparql_group_concat",
        min_args: 1,
        max_args: 2,
        kind: HelperKind::Aggregate,
        semantics: "GROUP_CONCAT aggregate, same protocol as sparql_sum. One argument: separator \
                    \" \". Two arguments: the second is the separator as a simple-literal term id \
                    (constant for the whole group).",
    },
];

/// Looks up a helper function by name.
pub fn helper(name: &str) -> Option<&'static HelperFunction> {
    HELPER_FUNCTIONS.iter().find(|h| h.name == name)
}

// ---------------------------------------------------------------- sparql_eval vocabulary

/// Binary operators of `sparql_eval` and the `Expr` each stands for.
const BINARY_OPS: &[(&str, fn(Box<Expr>, Box<Expr>) -> Expr)] = &[
    ("=", Expr::Equal),
    ("<", Expr::Less),
    ("<=", Expr::LessOrEqual),
    (">", Expr::Greater),
    (">=", Expr::GreaterOrEqual),
    ("+", Expr::Add),
    ("-", Expr::Subtract),
    ("*", Expr::Multiply),
    ("/", Expr::Divide),
];

/// Unary operators of `sparql_eval`.
const UNARY_OPS: &[(&str, fn(Box<Expr>) -> Expr)] = &[("u+", Expr::UnaryPlus), ("u-", Expr::UnaryMinus)];

/// SPARQL builtins callable through `sparql_eval` (deterministic per argument tuple). BNODE,
/// RAND, NOW, UUID and STRUUID are not: the baseline rejects them.
static FUNCTIONS: [(&str, Function); 41] = [
    ("STR", Function::Str),
    ("LANG", Function::Lang),
    ("LANGMATCHES", Function::LangMatches),
    ("DATATYPE", Function::Datatype),
    ("IRI", Function::Iri),
    ("ABS", Function::Abs),
    ("CEIL", Function::Ceil),
    ("FLOOR", Function::Floor),
    ("ROUND", Function::Round),
    ("CONCAT", Function::Concat),
    ("SUBSTR", Function::SubStr),
    ("STRLEN", Function::StrLen),
    ("REPLACE", Function::Replace),
    ("UCASE", Function::UCase),
    ("LCASE", Function::LCase),
    ("ENCODE_FOR_URI", Function::EncodeForUri),
    ("CONTAINS", Function::Contains),
    ("STRSTARTS", Function::StrStarts),
    ("STRENDS", Function::StrEnds),
    ("STRBEFORE", Function::StrBefore),
    ("STRAFTER", Function::StrAfter),
    ("YEAR", Function::Year),
    ("MONTH", Function::Month),
    ("DAY", Function::Day),
    ("HOURS", Function::Hours),
    ("MINUTES", Function::Minutes),
    ("SECONDS", Function::Seconds),
    ("TIMEZONE", Function::Timezone),
    ("TZ", Function::Tz),
    ("MD5", Function::Md5),
    ("SHA1", Function::Sha1),
    ("SHA256", Function::Sha256),
    ("SHA384", Function::Sha384),
    ("SHA512", Function::Sha512),
    ("STRLANG", Function::StrLang),
    ("STRDT", Function::StrDt),
    ("isIRI", Function::IsIri),
    ("isBLANK", Function::IsBlank),
    ("isLITERAL", Function::IsLiteral),
    ("isNUMERIC", Function::IsNumeric),
    ("REGEX", Function::Regex),
];

/// The `sparql_eval` name of a builtin, or None if the baseline cannot evaluate it per call.
pub(crate) fn function_name(f: &Function) -> Option<&'static str> {
    FUNCTIONS.iter().find(|(_, g)| g == f).map(|(name, _)| *name)
}

/// The `sparql_eval` name of an XSD cast (`Function::Custom(xsd:T)`): the datatype IRI itself,
/// taken from the static `WELL_KNOWN` table, so no query text ever reaches the SQL.
pub(crate) fn cast_name(iri: &str) -> Option<&'static str> {
    WELL_KNOWN.iter().map(|(_, known)| *known).find(|known| known.starts_with(XSD) && *known == iri)
}

/// The expression a `sparql_eval(op, a1..an)` call stands for: slot i is argument i+1.
/// None for an unknown operator or a wrong operator arity (builtin arities are checked by
/// `sq_expr::compile`).
pub fn eval_template(op: &str, nargs: usize) -> Option<Expr> {
    let slot = |i: usize| Box::new(Expr::Slot(i as Slot));
    if let Some((_, make)) = BINARY_OPS.iter().find(|(name, _)| *name == op) {
        return (nargs == 2).then(|| make(slot(0), slot(1)));
    }
    if let Some((_, make)) = UNARY_OPS.iter().find(|(name, _)| *name == op) {
        return (nargs == 1).then(|| make(slot(0)));
    }
    if nargs > sq_core::batch::SlotSet::MAX_SLOTS {
        return None;
    }
    let args = || (0..nargs).map(|i| Expr::Slot(i as Slot)).collect();
    if let Some((_, f)) = FUNCTIONS.iter().find(|(name, _)| *name == op) {
        return Some(Expr::Call(f.clone(), args()));
    }
    let iri = cast_name(op)?;
    Some(Expr::Call(Function::Custom(NamedNode::new_unchecked(iri)), args()))
}

/// Name of the aggregate helper for an aggregate function (COUNT is plain SQL).
pub(crate) fn aggregate_name(f: &sq_core::ir::AggFunc) -> Option<&'static str> {
    use sq_core::ir::AggFunc;
    Some(match f {
        AggFunc::Sum => "sparql_sum",
        AggFunc::Avg => "sparql_avg",
        AggFunc::Min => "sparql_min",
        AggFunc::Max => "sparql_max",
        AggFunc::Sample => "sparql_sample",
        AggFunc::GroupConcat { .. } => "sparql_group_concat",
        AggFunc::CountAll | AggFunc::Count | AggFunc::Custom(_) => return None,
    })
}
