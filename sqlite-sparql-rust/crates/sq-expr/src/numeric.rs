//! Numeric tower (§6.2): integer → decimal → float → double promotion, checked integer and
//! decimal arithmetic (overflow / division by zero are errors), IEEE float/double arithmetic
//! (±INF / NaN), XPath `abs`/`ceiling`/`floor`/`round` and comparisons.
//!
//! `xsd:integer` is an `i64` (values beyond are ill-typed, a documented divergence) and
//! `xsd:decimal` is oxsdatatypes' fixed-point `Decimal` (an i128 scaled by 10^18). Decimal
//! products and quotients are computed exactly with a 256-bit intermediate and truncated to
//! 18 fractional digits, so precision loss is never reported as an overflow.
use std::cmp::Ordering;

use oxsdatatypes::{Decimal, Double, Float};

/// 10^18: the fixed-point scale of `oxsdatatypes::Decimal`.
pub const DEC_SCALE: i128 = 1_000_000_000_000_000_000;

/// Raw fixed-point representation (value × 10^18).
#[inline]
pub fn dec_raw(d: Decimal) -> i128 {
    i128::from_be_bytes(d.to_be_bytes())
}

/// Inverse of [`dec_raw`].
#[inline]
pub fn dec_from_raw(raw: i128) -> Decimal {
    Decimal::from_be_bytes(raw.to_be_bytes())
}

/// Exact powers of ten representable as f64 (used by the correctly rounded fast path).
const POW10: [f64; 19] = [
    1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16, 1e17, 1e18,
];

/// Correctly rounded decimal → f64 conversion. Correct rounding is monotone, which the
/// ORDER BY key relies on (a < b ⇒ f64(a) ≤ f64(b)), and agrees with `i64 as f64` on integers.
pub fn dec_to_f64(d: Decimal) -> f64 {
    let raw = dec_raw(d);
    if raw % DEC_SCALE == 0 {
        return (raw / DEC_SCALE) as f64;
    }
    let (mut m, mut k) = (raw, 18usize);
    while m % 10 == 0 {
        m /= 10;
        k -= 1;
    }
    if m.unsigned_abs() < (1u128 << 53) {
        // Both operands exact: a single, correctly rounded division (Clinger's fast path).
        return m as f64 / POW10[k];
    }
    d.to_string().parse().unwrap_or(f64::NAN)
}

/// 128 × 128 → 256-bit unsigned product as (high, low).
fn widening_mul(a: u128, b: u128) -> (u128, u128) {
    const MASK: u128 = u64::MAX as u128;
    let (a1, a0) = (a >> 64, a & MASK);
    let (b1, b0) = (b >> 64, b & MASK);
    let p00 = a0 * b0;
    let p01 = a0 * b1;
    let p10 = a1 * b0;
    let p11 = a1 * b1;
    let mid = (p00 >> 64) + (p01 & MASK) + (p10 & MASK);
    let lo = (p00 & MASK) | (mid << 64);
    let hi = p11 + (p01 >> 64) + (p10 >> 64) + (mid >> 64);
    (hi, lo)
}

/// trunc(a × b / d) without intermediate overflow; None if d == 0 or the result overflows i128.
pub fn mul_div_trunc(a: i128, b: i128, d: i128) -> Option<i128> {
    if d == 0 {
        return None;
    }
    let negative = (a < 0) ^ (b < 0) ^ (d < 0);
    let (hi, lo) = widening_mul(a.unsigned_abs(), b.unsigned_abs());
    let d = d.unsigned_abs();
    let q = if hi == 0 {
        lo / d
    } else {
        if hi >= d {
            return None; // quotient ≥ 2^128
        }
        // Restoring long division of the 256-bit (hi, lo) by d; the remainder stays < d.
        let (mut rem, mut q) = (hi, 0u128);
        for i in (0..128).rev() {
            let carry = rem >> 127;
            rem = (rem << 1) | ((lo >> i) & 1);
            q <<= 1;
            if carry == 1 || rem >= d {
                rem = rem.wrapping_sub(d);
                q |= 1;
            }
        }
        q
    };
    if negative {
        if q <= i128::MAX as u128 + 1 { Some((q as i128).wrapping_neg()) } else { None }
    } else {
        i128::try_from(q).ok()
    }
}

/// Exact decimal product truncated to 18 fractional digits (None on overflow).
pub fn dec_mul(a: Decimal, b: Decimal) -> Option<Decimal> {
    mul_div_trunc(dec_raw(a), dec_raw(b), DEC_SCALE).map(dec_from_raw)
}

/// Decimal quotient truncated to 18 fractional digits (None on division by zero / overflow).
pub fn dec_div(a: Decimal, b: Decimal) -> Option<Decimal> {
    let b = dec_raw(b);
    if b == 0 {
        return None;
    }
    mul_div_trunc(dec_raw(a), DEC_SCALE, b).map(dec_from_raw)
}

/// fn:floor on decimals.
pub fn dec_floor(d: Decimal) -> Option<Decimal> {
    dec_raw(d).div_euclid(DEC_SCALE).checked_mul(DEC_SCALE).map(dec_from_raw)
}

/// fn:ceiling on decimals.
pub fn dec_ceil(d: Decimal) -> Option<Decimal> {
    let r = dec_raw(d);
    let f = r.div_euclid(DEC_SCALE);
    let c = if r.rem_euclid(DEC_SCALE) == 0 { f } else { f + 1 };
    c.checked_mul(DEC_SCALE).map(dec_from_raw)
}

/// fn:round on decimals: round half toward positive infinity, i.e. floor(x + 0.5).
pub fn dec_round(d: Decimal) -> Option<Decimal> {
    let r = dec_raw(d).checked_add(DEC_SCALE / 2)?;
    r.div_euclid(DEC_SCALE).checked_mul(DEC_SCALE).map(dec_from_raw)
}

/// Truncation toward zero of a decimal to an integer (cast decimal → integer).
pub fn dec_trunc_to_i64(d: Decimal) -> Option<i64> {
    i64::try_from(dec_raw(d) / DEC_SCALE).ok()
}

/// fn:round on doubles: half toward +∞, keeping -0 for arguments in [-0.5, -0] (XPath).
pub fn f64_round(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let f = x.floor();
    // x - floor(x) is exact for every finite double.
    let r = if x - f >= 0.5 { f + 1.0 } else { f };
    if r == 0.0 && x.is_sign_negative() { -0.0 } else { r }
}

/// A numeric value of one of the four primitive numeric types.
#[derive(Copy, Clone, Debug)]
pub enum Num {
    Integer(i64),
    Decimal(Decimal),
    Float(Float),
    Double(Double),
}

impl Num {
    /// Promotion rank: integer 0, decimal 1, float 2, double 3.
    #[inline]
    pub fn rank(self) -> u8 {
        match self {
            Num::Integer(_) => 0,
            Num::Decimal(_) => 1,
            Num::Float(_) => 2,
            Num::Double(_) => 3,
        }
    }

    /// Exact decimal value of an integer or decimal.
    #[inline]
    pub fn to_decimal(self) -> Option<Decimal> {
        match self {
            Num::Integer(i) => Some(Decimal::from(i)),
            Num::Decimal(d) => Some(d),
            _ => None,
        }
    }

    /// Promotion to double (correctly rounded for integers and decimals).
    #[inline]
    pub fn to_f64(self) -> f64 {
        match self {
            Num::Integer(i) => i as f64,
            Num::Decimal(d) => dec_to_f64(d),
            Num::Float(f) => f64::from(f32::from(f)),
            Num::Double(d) => f64::from(d),
        }
    }

    /// Promotion to float (via the correctly rounded double, so the order stays monotone).
    #[inline]
    pub fn to_f32(self) -> f32 {
        match self {
            Num::Float(f) => f32::from(f),
            other => other.to_f64() as f32,
        }
    }

    pub fn is_nan(self) -> bool {
        match self {
            Num::Float(f) => f32::from(f).is_nan(),
            Num::Double(d) => f64::from(d).is_nan(),
            _ => false,
        }
    }

    /// Value is zero (or NaN for float/double): the numeric EBV is `!is_zero_or_nan`.
    pub fn is_zero_or_nan(self) -> bool {
        match self {
            Num::Integer(i) => i == 0,
            Num::Decimal(d) => dec_raw(d) == 0,
            Num::Float(f) => {
                let f = f32::from(f);
                f == 0.0 || f.is_nan()
            }
            Num::Double(d) => {
                let d = f64::from(d);
                d == 0.0 || d.is_nan()
            }
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ArithOp {
    Add,
    Sub,
    Mul,
    Div,
}

/// Binary arithmetic with type promotion. `integer / integer` is a decimal division.
/// None = SPARQL error (integer/decimal overflow, decimal division by zero).
pub fn arith(op: ArithOp, a: Num, b: Num) -> Option<Num> {
    match a.rank().max(b.rank()) {
        0 => {
            let (Num::Integer(x), Num::Integer(y)) = (a, b) else { unreachable!("rank 0 means two integers") };
            match op {
                ArithOp::Add => x.checked_add(y).map(Num::Integer),
                ArithOp::Sub => x.checked_sub(y).map(Num::Integer),
                ArithOp::Mul => x.checked_mul(y).map(Num::Integer),
                // x × 10^18 fits i128 for every i64, so this is the exact truncated quotient.
                ArithOp::Div if y != 0 => Some(Num::Decimal(dec_from_raw(x as i128 * DEC_SCALE / y as i128))),
                ArithOp::Div => None,
            }
        }
        1 => {
            // Decimal × integer and decimal ÷ integer stay exact on the raw i128.
            match (op, a, b) {
                (ArithOp::Mul, Num::Decimal(d), Num::Integer(i)) | (ArithOp::Mul, Num::Integer(i), Num::Decimal(d)) => {
                    return dec_raw(d).checked_mul(i as i128).map(|r| Num::Decimal(dec_from_raw(r)));
                }
                (ArithOp::Div, Num::Decimal(d), Num::Integer(i)) => {
                    return dec_raw(d).checked_div(i as i128).map(|r| Num::Decimal(dec_from_raw(r)));
                }
                _ => {}
            }
            let (x, y) = (a.to_decimal()?, b.to_decimal()?);
            match op {
                ArithOp::Add => x.checked_add(y),
                ArithOp::Sub => x.checked_sub(y),
                ArithOp::Mul => dec_mul(x, y),
                ArithOp::Div => dec_div(x, y),
            }
            .map(Num::Decimal)
        }
        2 => {
            let (x, y) = (a.to_f32(), b.to_f32());
            Some(Num::Float(Float::from(match op {
                ArithOp::Add => x + y,
                ArithOp::Sub => x - y,
                ArithOp::Mul => x * y,
                ArithOp::Div => x / y,
            })))
        }
        _ => {
            let (x, y) = (a.to_f64(), b.to_f64());
            Some(Num::Double(Double::from(match op {
                ArithOp::Add => x + y,
                ArithOp::Sub => x - y,
                ArithOp::Mul => x * y,
                ArithOp::Div => x / y,
            })))
        }
    }
}

/// op:numeric-unary-minus.
pub fn neg(a: Num) -> Option<Num> {
    Some(match a {
        Num::Integer(i) => Num::Integer(i.checked_neg()?),
        Num::Decimal(d) => Num::Decimal(d.checked_neg()?),
        Num::Float(f) => Num::Float(-f),
        Num::Double(d) => Num::Double(-d),
    })
}

/// fn:abs (type preserving).
pub fn abs(a: Num) -> Option<Num> {
    Some(match a {
        Num::Integer(i) => Num::Integer(i.checked_abs()?),
        Num::Decimal(d) => Num::Decimal(d.checked_abs()?),
        Num::Float(f) => Num::Float(f.abs()),
        Num::Double(d) => Num::Double(d.abs()),
    })
}

/// fn:ceiling (type preserving).
pub fn ceil(a: Num) -> Option<Num> {
    Some(match a {
        Num::Integer(_) => a,
        Num::Decimal(d) => Num::Decimal(dec_ceil(d)?),
        Num::Float(f) => Num::Float(f.ceil()),
        Num::Double(d) => Num::Double(d.ceil()),
    })
}

/// fn:floor (type preserving).
pub fn floor(a: Num) -> Option<Num> {
    Some(match a {
        Num::Integer(_) => a,
        Num::Decimal(d) => Num::Decimal(dec_floor(d)?),
        Num::Float(f) => Num::Float(f.floor()),
        Num::Double(d) => Num::Double(d.floor()),
    })
}

/// fn:round (type preserving, half toward +∞).
pub fn round(a: Num) -> Option<Num> {
    Some(match a {
        Num::Integer(_) => a,
        Num::Decimal(d) => Num::Decimal(dec_round(d)?),
        Num::Float(f) => Num::Float(Float::from(f64_round(f64::from(f32::from(f))) as f32)),
        Num::Double(d) => Num::Double(Double::from(f64_round(f64::from(d)))),
    })
}

/// Numeric comparison with promotion. None only when a NaN is involved (unordered).
pub fn compare(a: Num, b: Num) -> Option<Ordering> {
    match a.rank().max(b.rank()) {
        0 | 1 => match (a, b) {
            (Num::Integer(x), Num::Integer(y)) => Some(x.cmp(&y)),
            _ => Some(a.to_decimal()?.cmp(&b.to_decimal()?)),
        },
        2 => a.to_f32().partial_cmp(&b.to_f32()),
        _ => a.to_f64().partial_cmp(&b.to_f64()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    #[test]
    fn raw_round_trip() {
        for s in ["0", "1", "-1", "0.5", "-12.25", "123456789.000000000000000001"] {
            assert_eq!(dec_from_raw(dec_raw(dec(s))), dec(s));
        }
        assert_eq!(dec_raw(dec("1")), DEC_SCALE);
    }

    #[test]
    fn widening_mul_matches_small_products() {
        let cases = [(0u128, 5u128), (u64::MAX as u128, u64::MAX as u128), (1 << 100, 1 << 27), (u128::MAX, 2)];
        for (a, b) in cases {
            let (hi, lo) = widening_mul(a, b);
            if let Some(p) = a.checked_mul(b) {
                assert_eq!((hi, lo), (0, p));
            }
        }
        assert_eq!(widening_mul(u128::MAX, 2), (1, u128::MAX - 1));
        assert_eq!(widening_mul(u128::MAX, u128::MAX), (u128::MAX - 1, 1));
    }

    #[test]
    fn mul_div_trunc_cases() {
        assert_eq!(mul_div_trunc(7, 3, 2), Some(10));
        assert_eq!(mul_div_trunc(-7, 3, 2), Some(-10));
        assert_eq!(mul_div_trunc(7, -3, -2), Some(10));
        assert_eq!(mul_div_trunc(1, 1, 0), None);
        // Needs the 256-bit path: (10^30 × 10^20) / 10^18 = 10^32.
        let e = |n: u32| 10i128.pow(n);
        assert_eq!(mul_div_trunc(e(30), e(20), e(18)), Some(e(32)));
        assert_eq!(mul_div_trunc(e(30), e(30), e(18)), None);
        assert_eq!(mul_div_trunc(i128::MAX, 2, 2), Some(i128::MAX));
        assert_eq!(mul_div_trunc(i128::MIN, 1, 1), Some(i128::MIN));
        assert_eq!(mul_div_trunc(i128::MIN, -1, 1), None);
    }

    #[test]
    fn decimal_arithmetic() {
        let d = |s| Num::Decimal(dec(s));
        let show = |n: Option<Num>| match n {
            Some(Num::Decimal(x)) => x.to_string(),
            Some(Num::Integer(i)) => format!("int {i}"),
            other => format!("{other:?}"),
        };
        assert_eq!(show(arith(ArithOp::Div, Num::Integer(1), Num::Integer(3))), "0.333333333333333333");
        assert_eq!(show(arith(ArithOp::Div, Num::Integer(6), Num::Integer(3))), "2");
        assert_eq!(show(arith(ArithOp::Div, Num::Integer(-7), Num::Integer(2))), "-3.5");
        assert!(arith(ArithOp::Div, Num::Integer(1), Num::Integer(0)).is_none());
        assert!(arith(ArithOp::Div, d("1.5"), d("0")).is_none());
        // More than 18 fractional digits in the exact product: truncated, not an error.
        assert_eq!(show(arith(ArithOp::Mul, d("0.1234567891"), d("0.1234567891"))), "0.015241578774881878");
        assert_eq!(show(arith(ArithOp::Add, Num::Integer(1), d("0.5"))), "1.5");
        assert!(arith(ArithOp::Add, Num::Integer(i64::MAX), Num::Integer(1)).is_none());
        assert!(arith(ArithOp::Mul, Num::Integer(i64::MAX), Num::Integer(2)).is_none());
        assert!(arith(ArithOp::Mul, d("100000000000000000000"), d("100000000000000000000")).is_none());
        // Decimal × / ÷ integer fast paths agree with the general decimal path.
        assert_eq!(show(arith(ArithOp::Mul, d("1.25"), Num::Integer(-3))), "-3.75");
        assert_eq!(show(arith(ArithOp::Mul, Num::Integer(4), d("0.25"))), "1");
        assert_eq!(show(arith(ArithOp::Div, d("1"), Num::Integer(3))), "0.333333333333333333");
        assert_eq!(show(arith(ArithOp::Div, d("-7.5"), Num::Integer(2))), "-3.75");
        assert!(arith(ArithOp::Div, d("7.5"), Num::Integer(0)).is_none());
        assert!(arith(ArithOp::Mul, d("100000000000000000000"), Num::Integer(i64::MAX)).is_none());
        assert_eq!(show(arith(ArithOp::Div, Num::Integer(i64::MIN), Num::Integer(-1))), "9223372036854775808");
    }

    #[test]
    fn float_arithmetic_gives_inf_and_nan() {
        let r = arith(ArithOp::Div, Num::Double(Double::from(1.0)), Num::Integer(0)).unwrap();
        assert!(matches!(r, Num::Double(x) if f64::from(x) == f64::INFINITY));
        let r = arith(ArithOp::Div, Num::Float(Float::from(0.0)), Num::Integer(0)).unwrap();
        assert!(matches!(r, Num::Float(x) if f32::from(x).is_nan()));
        let r = arith(ArithOp::Add, Num::Integer(1), Num::Float(Float::from(0.5))).unwrap();
        assert!(matches!(r, Num::Float(x) if f32::from(x) == 1.5));
        let r = arith(ArithOp::Mul, Num::Float(Float::from(2.0)), Num::Double(Double::from(0.25))).unwrap();
        assert!(matches!(r, Num::Double(x) if f64::from(x) == 0.5));
    }

    #[test]
    fn rounding_functions() {
        let r = |s: &str| match round(Num::Decimal(dec(s))) {
            Some(Num::Decimal(x)) => x.to_string(),
            _ => panic!(),
        };
        assert_eq!(r("2.5"), "3");
        assert_eq!(r("2.4999"), "2");
        assert_eq!(r("-2.5"), "-2");
        assert_eq!(r("-2.51"), "-3");
        assert_eq!(r("-0.4"), "0");
        assert_eq!(dec_ceil(dec("-1.6")).unwrap().to_string(), "-1");
        assert_eq!(dec_ceil(dec("1.1")).unwrap().to_string(), "2");
        assert_eq!(dec_floor(dec("-1.6")).unwrap().to_string(), "-2");
        assert_eq!(dec_floor(dec("2")).unwrap().to_string(), "2");
        assert_eq!(f64_round(2.5), 3.0);
        assert_eq!(f64_round(-2.5), -2.0);
        assert_eq!(f64_round(-2.6), -3.0);
        assert_eq!(f64_round(0.49999999999999994), 0.0);
        assert!(f64_round(-0.3).is_sign_negative());
        assert!(f64_round(f64::NAN).is_nan());
        assert_eq!(f64_round(f64::NEG_INFINITY), f64::NEG_INFINITY);
        assert_eq!(dec_trunc_to_i64(dec("-2.9")), Some(-2));
    }

    #[test]
    fn decimal_to_f64_is_correctly_rounded() {
        for s in ["0.1", "-0.3", "1.5", "123456789.123456789", "0.000000000000000001", "170141183460469231731.687303715884105727"] {
            let expect: f64 = s.parse().unwrap();
            assert_eq!(dec_to_f64(dec(s)), expect, "{s}");
        }
        assert_eq!(dec_to_f64(dec("9007199254740993")), 9007199254740993i64 as f64);
    }

    #[test]
    fn comparisons_promote() {
        assert_eq!(compare(Num::Integer(1), Num::Decimal(dec("1.0"))), Some(Ordering::Equal));
        assert_eq!(compare(Num::Integer(1), Num::Double(Double::from(1.5))), Some(Ordering::Less));
        assert_eq!(compare(Num::Decimal(dec("0.1")), Num::Decimal(dec("0.10000000000000001"))), Some(Ordering::Less));
        assert_eq!(compare(Num::Double(Double::NAN), Num::Integer(1)), None);
        assert_eq!(compare(Num::Float(Float::from(0.5)), Num::Decimal(dec("0.5"))), Some(Ordering::Equal));
        assert_eq!(compare(Num::Integer(i64::MAX), Num::Integer(i64::MAX - 1)), Some(Ordering::Greater));
    }
}
