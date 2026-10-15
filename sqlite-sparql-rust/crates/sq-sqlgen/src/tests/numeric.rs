//! Exact numeric fast paths (`numeric`): the integer bounds, the inline keys, and the claims
//! that make an inline decimal's REAL comparison exact.
use std::cmp::Ordering;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use sq_core::TermId;
use sq_core::codec::{encode_decimal, encode_inline_literal};

use crate::numeric::{Cmp, DEC_EXACT_VS_DEC, DEC_EXACT_VS_INT, DOTZERO_BIT, NumConst, RealOperand};

const XSD_DECIMAL: &str = "http://www.w3.org/2001/XMLSchema#decimal";

fn dec(lex: &str) -> NumConst {
    NumConst::of(encode_inline_literal(lex, XSD_DECIMAL, None).expect("inline decimal")).expect("numeric")
}

#[test]
fn constants_are_read_exactly() {
    assert_eq!(NumConst::of(TermId::from_int(-7).unwrap()), Some(NumConst::Int(-7)));
    assert_eq!(dec("-12.25"), NumConst::Dec { m: -1225, scale: 2 });
    assert_eq!(dec("5.0"), NumConst::Dec { m: 5, scale: 0 });
    assert_eq!(NumConst::of(TermId::from_bool(true)), None);
}

#[test]
fn integer_bounds_are_floor_or_ceil() {
    // For every integer v: v OP c ⇔ v OP bound.
    for (c, lo, hi) in [(dec("1.5"), 1, 2), (dec("-1.5"), -2, -1), (dec("3"), 3, 3), (NumConst::Int(-4), -4, -4)] {
        assert_eq!(c.int_bound(Cmp::Gt), lo);
        assert_eq!(c.int_bound(Cmp::Le), lo);
        assert_eq!(c.int_bound(Cmp::Ge), hi);
        assert_eq!(c.int_bound(Cmp::Lt), hi);
    }
    let exact = |v: i64, c: (i64, u32)| (v as i128 * 10i128.pow(c.1)).cmp(&(c.0 as i128));
    for (m, s) in [(15, 1), (-15, 1), (7, 0), (-1, 3), (999, 2)] {
        let c = NumConst::Dec { m, scale: s as u8 };
        for v in -12..12 {
            let ord = exact(v, (m, s));
            assert_eq!(v > c.int_bound(Cmp::Gt), ord == Ordering::Greater, "{v} > {m}e-{s}");
            assert_eq!(v >= c.int_bound(Cmp::Ge), ord != Ordering::Less, "{v} >= {m}e-{s}");
            assert_eq!(v < c.int_bound(Cmp::Lt), ord == Ordering::Less, "{v} < {m}e-{s}");
            assert_eq!(v <= c.int_bound(Cmp::Le), ord != Ordering::Greater, "{v} <= {m}e-{s}");
        }
    }
}

#[test]
fn equal_values_have_equal_keys() {
    let int = |v: i64| TermId::from_int(v).unwrap();
    assert_eq!(NumConst::Int(5).int_id(), Some(int(5)));
    assert_eq!(dec("5.0").int_id(), Some(int(5)));
    assert_eq!(dec("5.5").int_id(), None);
    // `5`, `5.0` (the `.0` bit) and the integer 5 share one decimal key.
    let five = encode_decimal(5, 0, false).raw() | DOTZERO_BIT;
    assert_eq!(dec("5").dec_key(), Some(five));
    assert_eq!(dec("5.0").dec_key(), Some(five));
    assert_eq!(NumConst::Int(5).dec_key(), Some(five));
    assert_eq!(dec("5.25").dec_key(), Some(encode_decimal(525, 2, false).raw() | DOTZERO_BIT));
    // An integer outside the inline-decimal mantissa range has no decimal twin.
    assert_eq!(NumConst::Int(1 << 55).dec_key(), None);
}

#[test]
fn real_operands_only_where_exact() {
    assert_eq!(NumConst::Int(7).real_operand(), Some(RealOperand::Int(7)));
    assert_eq!(dec("7.0").real_operand(), Some(RealOperand::Int(7)));
    assert_eq!(dec("0.25").real_operand(), Some(RealOperand::Ratio { m: 25, pow10: 100 }));
    assert_eq!(NumConst::Dec { m: DEC_EXACT_VS_DEC + 1, scale: 3 }.real_operand(), None);
    assert_eq!(RealOperand::Int(1).row_limit(), DEC_EXACT_VS_INT);
}

/// The correctly rounded f64 of m / 10^s (what `sparql_num` returns for an inline decimal).
fn real(m: i64, s: u32) -> f64 {
    format!("{m}e-{s}").parse().unwrap()
}

/// SQLite's exact comparison of a REAL with an INTEGER.
fn cmp_real_int(r: f64, k: i64) -> Ordering {
    let kf = k as f64;
    if r != kf { r.partial_cmp(&kf).unwrap() } else { (r as i128).cmp(&(k as i128)) }
}

#[test]
fn decimal_reals_compare_exactly_with_integers_below_the_limit() {
    let mut rng = StdRng::seed_from_u64(7);
    for _ in 0..200_000 {
        let s: u32 = rng.random_range(0..=15);
        let m: i64 = match rng.random_range(0..3) {
            0 => rng.random_range(-DEC_EXACT_VS_INT..=DEC_EXACT_VS_INT),
            1 => DEC_EXACT_VS_INT - rng.random_range(0..1000),
            _ => rng.random_range(-10_000..10_000),
        };
        let p = 10i128.pow(s);
        // Integers at and around the value.
        let floor = (m as i128).div_euclid(p) as i64;
        for k in [floor - 1, floor, floor + 1, floor + 2] {
            let exact = (m as i128).cmp(&(k as i128 * p));
            assert_eq!(cmp_real_int(real(m, s), k), exact, "{m}e-{s} vs {k}");
        }
    }
    // Past the limit the REAL is not exact: 9007199254740993 rounds to …992.
    let m = DEC_EXACT_VS_INT + 2;
    assert_ne!(cmp_real_int(real(m, 0), m), Ordering::Equal);
}

#[test]
fn decimal_reals_compare_exactly_with_each_other_below_the_limit() {
    let mut rng = StdRng::seed_from_u64(11);
    let mantissa = |rng: &mut StdRng| match rng.random_range(0..3) {
        0 => rng.random_range(-DEC_EXACT_VS_DEC..=DEC_EXACT_VS_DEC),
        1 => DEC_EXACT_VS_DEC - rng.random_range(0..1000),
        _ => rng.random_range(-10_000..10_000),
    };
    for _ in 0..200_000 {
        let (s1, s2): (u32, u32) = (rng.random_range(0..=15), rng.random_range(1..=15));
        let m1 = mantissa(&mut rng);
        // A second value at, or one unit of the finer scale around, the first.
        let shift = |m: i64, from: u32, to: u32| -> Option<i64> {
            if to >= from { m.checked_mul(10i64.pow(to - from)) } else { Some(m / 10i64.pow(from - to)) }
        };
        let Some(base) = shift(m1, s1, s2) else { continue };
        for m2 in [base - 1, base, base + 1] {
            if m2.abs() > DEC_EXACT_VS_DEC {
                continue;
            }
            let exact = (m1 as i128 * 10i128.pow(s2)).cmp(&(m2 as i128 * 10i128.pow(s1)));
            let reals = real(m1, s1).partial_cmp(&real(m2, s2)).unwrap();
            assert_eq!(reals, exact, "{m1}e-{s1} vs {m2}e-{s2}");
        }
    }
}
