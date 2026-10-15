//! Integer range pushdown (spec §7.3.7).
//!
//! A filter conjunct `?o op c` (op one of `<`, `<=`, `>`, `>=`; c a numeric constant) on the
//! object slot of a quad pattern with a fixed graph and a constant predicate lets the pattern be
//! read as `Scan { order: Gpos, range: IntRange { lo, hi } }`: the inline xsd:integer objects
//! in [lo, hi] (a raw-id range of rdf_gpos) followed by every dictionary-typed or decimal object
//! of (g, p) (the partial index rdf_gpox). A value that can pass the conjunct is numeric, and
//! every stored numeric is an inline integer, an inline decimal or a dictionary-typed literal,
//! so the scan returns a superset of the passing rows. The conjunct stays above the scan, which
//! makes the plan exact without trusting statistics.
//!
//! The integer bounds are exact for the comparison the evaluator performs (spec §6.2): integers
//! and decimals compare exactly; float and double constants promote the integer, and rounding is
//! monotonic, so `> c` and `< c` exclude only integers that cannot pass; `>= c` and `<= c` are
//! pushed down only while every integer near c is exactly representable (|c| < 2^24 for float,
//! 2^53 for double).
use sq_core::batch::Slot;
use sq_core::codec::{self, XSD};
use sq_core::id::{Tag, TermId};
use sq_core::ir::Expr;
use sq_core::plan::IntRange;
use sq_core::term::Term;

/// A numeric constant as the evaluator compares it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum Num {
    Int(i128),
    /// `m / 10^scale`.
    Dec { m: i128, scale: u32 },
    Float(f32),
    Double(f64),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Cmp {
    Gt,
    Ge,
    Lt,
    Le,
}

impl Cmp {
    /// `c op ?x` is `?x flip(op) c`.
    fn flip(self) -> Cmp {
        match self {
            Cmp::Gt => Cmp::Lt,
            Cmp::Ge => Cmp::Le,
            Cmp::Lt => Cmp::Gt,
            Cmp::Le => Cmp::Ge,
        }
    }
}

/// `?s op c` or `c op ?s` (op ∈ <, ≤, >, ≥; c numeric): the slot and the inclusive range of
/// inline integers that can satisfy the comparison. None for every other shape, for constants
/// `num` cannot read, and when the range is empty or not exact (see the module docs).
pub(crate) fn int_bounds(e: &Expr, num: &mut impl FnMut(TermId) -> Option<Num>) -> Option<(Slot, IntRange)> {
    let (op, a, b) = match e {
        Expr::Greater(a, b) => (Cmp::Gt, a, b),
        Expr::GreaterOrEqual(a, b) => (Cmp::Ge, a, b),
        Expr::Less(a, b) => (Cmp::Lt, a, b),
        Expr::LessOrEqual(a, b) => (Cmp::Le, a, b),
        _ => return None,
    };
    let (slot, op, c) = match (&**a, &**b) {
        (Expr::Slot(s), c) => (*s, op, c),
        (c, Expr::Slot(s)) => (*s, op.flip(), c),
        _ => return None,
    };
    Some((slot, bounds(op, constant(c, num)?)?))
}

/// A numeric constant expression: `c`, `+c` or `-c` (SPARQL parses `-3` as a negation).
fn constant(e: &Expr, num: &mut impl FnMut(TermId) -> Option<Num>) -> Option<Num> {
    match e {
        Expr::Const(c) => num(*c),
        Expr::UnaryPlus(x) => constant(x, num),
        Expr::UnaryMinus(x) => Some(match constant(x, num)? {
            Num::Int(v) => Num::Int(-v),
            Num::Dec { m, scale } => Num::Dec { m: -m, scale },
            Num::Float(f) => Num::Float(-f),
            Num::Double(d) => Num::Double(-d),
        }),
        _ => None,
    }
}

/// Both ranges at once; None when no integer satisfies both.
pub(crate) fn intersect(a: IntRange, b: IntRange) -> Option<IntRange> {
    let r = IntRange { lo: a.lo.max(b.lo), hi: a.hi.min(b.hi) };
    (r.lo <= r.hi).then_some(r)
}

/// Inline integers v with `v op c`, clamped to the inline range.
fn bounds(op: Cmp, c: Num) -> Option<IntRange> {
    let inclusive = matches!(op, Cmp::Ge | Cmp::Le);
    let (floor, ceil) = match c {
        Num::Int(v) => (v, v),
        Num::Dec { m, scale } => {
            let p = 10i128.checked_pow(scale)?;
            (m.div_euclid(p), -(-m).div_euclid(p))
        }
        Num::Float(f) => float_floor_ceil(f64::from(f), inclusive, (1u64 << 24) as f64)?,
        Num::Double(d) => float_floor_ceil(d, inclusive, (1u64 << 53) as f64)?,
    };
    let (lo, hi) = match op {
        Cmp::Gt => (floor.saturating_add(1), i128::MAX),
        Cmp::Ge => (ceil, i128::MAX),
        Cmp::Lt => (i128::MIN, ceil.saturating_sub(1)),
        Cmp::Le => (i128::MIN, floor),
    };
    let lo = lo.max(i128::from(TermId::INT_MIN));
    let hi = hi.min(i128::from(TermId::INT_MAX));
    (lo <= hi).then_some(IntRange { lo: lo as i64, hi: hi as i64 })
}

/// floor / ceil of a float or double constant; None for NaN and the infinities, and for
/// inclusive comparisons beyond `exact` (where neighbouring integers round onto c).
fn float_floor_ceil(c: f64, inclusive: bool, exact: f64) -> Option<(i128, i128)> {
    if !c.is_finite() || (inclusive && c.abs() >= exact) {
        return None;
    }
    // `as` saturates; anything beyond the inline range is clamped by the caller.
    Some((c.floor() as i128, c.ceil() as i128))
}

/// The numeric value of constant `id`: inline integers and decimals directly, dictionary-typed
/// and TEMP literals through `term` (their lexical form). None for non-numeric constants and
/// lexical forms this reader does not accept (no pushdown then).
pub(crate) fn numeric(id: TermId, term: impl FnOnce() -> Option<Term>) -> Option<Num> {
    let typed = match id.tag() {
        Tag::Int => return id.as_int().map(|v| Num::Int(i128::from(v))),
        Tag::Decimal => {
            let (m, scale, _) = codec::decimal_parts(id);
            return Some(Num::Dec { m: i128::from(m), scale: u32::from(scale) });
        }
        Tag::Typed => true,
        Tag::Temp => id.temp_tag() == Tag::Typed,
        _ => false,
    };
    if !typed {
        return None;
    }
    match term()? {
        Term::Literal(l) if l.lang.is_none() => parse_typed(&l.lex, &l.datatype),
        _ => None,
    }
}

/// Lexical form of an XSD numeric literal (integer types, decimal, float, double).
fn parse_typed(lex: &str, datatype: &str) -> Option<Num> {
    match datatype.strip_prefix(XSD)? {
        "integer" | "long" | "int" | "short" | "byte" | "nonNegativeInteger" | "positiveInteger"
        | "nonPositiveInteger" | "negativeInteger" | "unsignedLong" | "unsignedInt" | "unsignedShort"
        | "unsignedByte" => parse_decimal(lex, false).map(|(m, _)| Num::Int(m)),
        "decimal" => parse_decimal(lex, true).map(|(m, scale)| Num::Dec { m, scale }),
        "double" => float_lexical(lex).then(|| lex.parse::<f64>().ok()).flatten().map(Num::Double),
        "float" => float_lexical(lex).then(|| lex.parse::<f32>().ok()).flatten().map(Num::Float),
        _ => None,
    }
}

/// `[+-]digits` (and `[.digits]` when `fraction`) as (mantissa, scale); at most 36 digits.
fn parse_decimal(lex: &str, fraction: bool) -> Option<(i128, u32)> {
    let (neg, body) = match lex.as_bytes().first()? {
        b'-' => (true, &lex[1..]),
        b'+' => (false, &lex[1..]),
        _ => (false, lex),
    };
    let (int, frac) = match body.split_once('.') {
        Some((i, f)) if fraction => (i, f),
        Some(_) => return None,
        None => (body, ""),
    };
    let digits = int.len() + frac.len();
    if digits == 0 || digits > 36 || !int.bytes().chain(frac.bytes()).all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut m: i128 = 0;
    for b in int.bytes().chain(frac.bytes()) {
        m = m * 10 + i128::from(b - b'0');
    }
    Some((if neg { -m } else { m }, frac.len() as u32))
}

/// The xsd:float / xsd:double decimal and exponent grammar (no INF / NaN, no whitespace).
fn float_lexical(lex: &str) -> bool {
    let b = lex.as_bytes();
    let mut i = usize::from(matches!(b.first(), Some(b'+' | b'-')));
    let start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let mut digits = i - start;
    if b.get(i) == Some(&b'.') {
        i += 1;
        let f = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        digits += i - f;
    }
    if digits == 0 {
        return false;
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(b.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let e = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == e {
            return false;
        }
    }
    i == b.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sq_core::term::Literal;

    fn r(lo: i64, hi: i64) -> Option<IntRange> {
        Some(IntRange { lo, hi })
    }

    fn cmp(op: fn(Box<Expr>, Box<Expr>) -> Expr, c: TermId, slot_first: bool) -> Expr {
        let (s, k) = (Box::new(Expr::Slot(3)), Box::new(Expr::Const(c)));
        if slot_first { op(s, k) } else { op(k, s) }
    }

    fn typed(lex: &str, dt: &str) -> Term {
        Term::Literal(Literal::typed(lex, &format!("{XSD}{dt}")))
    }

    #[test]
    fn integer_bounds_of_each_operator() {
        let (max, min) = (TermId::INT_MAX, TermId::INT_MIN);
        let int = |v| Num::Int(v);
        let dec = |m, scale| Num::Dec { m, scale };
        assert_eq!(bounds(Cmp::Gt, dec(905, 1)), r(91, max)); // > 90.5
        assert_eq!(bounds(Cmp::Gt, int(90)), r(91, max));
        assert_eq!(bounds(Cmp::Ge, int(90)), r(90, max));
        assert_eq!(bounds(Cmp::Ge, dec(905, 1)), r(91, max));
        assert_eq!(bounds(Cmp::Lt, dec(105, 1)), r(min, 10)); // < 10.5
        assert_eq!(bounds(Cmp::Lt, int(10)), r(min, 9));
        assert_eq!(bounds(Cmp::Le, int(-3)), r(min, -3));
        assert_eq!(bounds(Cmp::Le, dec(-35, 1)), r(min, -4)); // <= -3.5
        assert_eq!(bounds(Cmp::Gt, dec(-35, 1)), r(-3, max)); // > -3.5
        assert_eq!(bounds(Cmp::Gt, Num::Double(90.5)), r(91, max));
        assert_eq!(bounds(Cmp::Le, Num::Float(2.5)), r(min, 2));
        // Beyond the inline range: clamped, or empty.
        assert_eq!(bounds(Cmp::Gt, int(i128::from(max) + 5)), None);
        assert_eq!(bounds(Cmp::Lt, int(1 << 70)), r(min, max));
        assert_eq!(bounds(Cmp::Gt, Num::Double(-1e300)), r(min, max));
    }

    #[test]
    fn inexact_float_constants_are_not_pushed() {
        assert_eq!(bounds(Cmp::Gt, Num::Double(f64::NAN)), None);
        assert_eq!(bounds(Cmp::Lt, Num::Double(f64::INFINITY)), None);
        // Inclusive comparisons with constants where integers round onto c.
        assert_eq!(bounds(Cmp::Ge, Num::Double(2f64.powi(54))), None);
        assert_eq!(bounds(Cmp::Le, Num::Float(2f32.powi(25))), None);
        // Strict comparisons stay exact (rounding is monotonic).
        assert_eq!(bounds(Cmp::Gt, Num::Double(2f64.powi(54))), r((1 << 54) + 1, TermId::INT_MAX));
    }

    #[test]
    fn conjunct_shapes() {
        let c = TermId::from_int(90).unwrap();
        let mut num = |id: TermId| numeric(id, || None);
        let (min, max) = (TermId::INT_MIN, TermId::INT_MAX);
        assert_eq!(int_bounds(&cmp(Expr::Greater, c, true), &mut num), Some((3, IntRange { lo: 91, hi: max })));
        // `90 > ?x` is `?x < 90`.
        assert_eq!(int_bounds(&cmp(Expr::Greater, c, false), &mut num), Some((3, IntRange { lo: min, hi: 89 })));
        assert_eq!(int_bounds(&cmp(Expr::LessOrEqual, c, false), &mut num), Some((3, IntRange { lo: 90, hi: max })));
        assert_eq!(int_bounds(&cmp(Expr::Equal, c, true), &mut num), None);
        let iri = TermId::from_dict(2000, Tag::Iri);
        assert_eq!(int_bounds(&cmp(Expr::Greater, iri, true), &mut num), None);
        let two_slots = Expr::Greater(Box::new(Expr::Slot(1)), Box::new(Expr::Slot(2)));
        assert_eq!(int_bounds(&two_slots, &mut num), None);
        // `?x <= -3` arrives as a negation of 3.
        let three = Box::new(Expr::Const(TermId::from_int(3).unwrap()));
        let neg = Expr::LessOrEqual(Box::new(Expr::Slot(3)), Box::new(Expr::UnaryMinus(three)));
        assert_eq!(int_bounds(&neg, &mut num), Some((3, IntRange { lo: min, hi: -3 })));
        let plus = Expr::Less(Box::new(Expr::UnaryPlus(Box::new(Expr::Const(c)))), Box::new(Expr::Slot(3)));
        assert_eq!(int_bounds(&plus, &mut num), Some((3, IntRange { lo: 91, hi: max })));
    }

    #[test]
    fn typed_constants() {
        let id = TermId::from_dict(5000, Tag::Typed);
        let read = |t: Term| numeric(id, || Some(t));
        assert_eq!(read(typed("90.5e0", "double")), Some(Num::Double(90.5)));
        assert_eq!(read(typed("-1.5", "float")), Some(Num::Float(-1.5)));
        assert_eq!(read(typed("+007", "integer")), Some(Num::Int(7)));
        assert_eq!(read(typed("12", "unsignedByte")), Some(Num::Int(12)));
        assert_eq!(read(typed("-0.50", "decimal")), Some(Num::Dec { m: -50, scale: 2 }));
        // Not numeric, ill-typed or non-finite: no pushdown.
        assert_eq!(read(typed("INF", "double")), None);
        assert_eq!(read(typed("1.5", "integer")), None);
        assert_eq!(read(typed(" 7", "integer")), None);
        assert_eq!(read(typed("2020-01-01", "date")), None);
        assert_eq!(read(Term::Literal(Literal::simple("7"))), None);
        // Inline constants need no term; TEMP literals are typed only with a typed would-be tag.
        assert_eq!(numeric(codec::encode_decimal(25, 1, false), || None), Some(Num::Dec { m: 25, scale: 1 }));
        assert_eq!(numeric(TermId::temp(0, Tag::Iri), || Some(typed("1", "integer"))), None);
        assert_eq!(numeric(TermId::temp(0, Tag::Typed), || Some(typed("1", "integer"))), Some(Num::Int(1)));
        assert_eq!(intersect(IntRange { lo: 1, hi: 10 }, IntRange { lo: 5, hi: 20 }), r(5, 10));
        assert_eq!(intersect(IntRange { lo: 1, hi: 4 }, IntRange { lo: 5, hi: 20 }), None);
    }
}
