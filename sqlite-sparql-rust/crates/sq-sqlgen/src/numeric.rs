//! Exact numeric comparisons with an inline integer or decimal constant, without decoding.
//!
//! SPARQL compares numerics after XPath promotion (§6.2 / §6.4): integer and decimal operands
//! compare as exact decimals, and a float or double operand promotes the other side to its
//! type. The generated SQL therefore splits on the row's tag:
//!
//! * inline INT (tag 6): the payload `x >> 4` against an integer bound derived from the
//!   constant (`floor` / `ceil` for a fractional decimal), exact and helper-free;
//! * inline DECIMAL (tag 8): `sparql_num(x)` (the correctly rounded REAL) against the
//!   constant, but only where that REAL comparison provably equals the exact decimal one (see
//!   [`RealOperand`]); equality needs no helper at all (`(x | 256)` drops the `.0` bit);
//! * TYPED (5) and TEMP (14) — floats, doubles, derived integer types, non-canonical lexical
//!   forms, computed terms, and inline decimals outside the exact range: `sparql_eval`, i.e.
//!   sq-expr's full promotion rules;
//! * every other tag is not numeric: `<` is an error and `=` is false.
use sq_core::codec::{decimal_parts, encode_decimal};
use sq_core::{Tag, TermId};

/// Tag bits (`id & 15`) of the classes the fast paths dispatch on (INT: `helpers::INT_TAG`).
pub(crate) const DEC_TAG: i64 = Tag::Decimal as i64;
pub(crate) const TYPED_TAG: i64 = Tag::Typed as i64;
pub(crate) const TEMP_TAG: i64 = Tag::Temp as i64;

/// Bit 8 of an inline decimal: the `.0` lexical marker (`5` vs `5.0`), irrelevant to its value.
pub(crate) const DOTZERO_BIT: i64 = 1 << 8;

/// Mantissa range of inline decimals (`m: i55`, codec::encode_decimal).
const DEC_M_MIN: i64 = -(1 << 54);
const DEC_M_MAX: i64 = (1 << 54) - 1;

/// Largest inline-decimal mantissa whose REAL compares exactly with an integer: a value m/10^s
/// that differs from an integer differs by at least 10^-s, while rounding moves it by at most
/// |m|·10^-s·2^-53 < 10^-s; an integral value (|m| < 2^53) is exact. SQLite compares INTEGER
/// with REAL exactly.
pub(crate) const DEC_EXACT_VS_INT: i64 = (1 << 53) - 1;

/// Largest mantissa (on both sides) for an exact REAL comparison of two decimals: distinct
/// values differ by at least 10^-S (S the larger scale), and with both |m| ≤ 2^51 the two
/// rounding errors together stay below half of that.
pub(crate) const DEC_EXACT_VS_DEC: i64 = (1 << 51) - 1;

/// An inline numeric constant.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum NumConst {
    /// `xsd:integer` k.
    Int(i64),
    /// `xsd:decimal` m / 10^scale (scale ≤ 15; no trailing fraction zero, so (m, scale) is
    /// unique per value).
    Dec { m: i64, scale: u8 },
}

/// What `sparql_num(x)` of an inline decimal is compared with, when that comparison is exact.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum RealOperand {
    /// The integer value itself (rows with |m| ≤ [`DEC_EXACT_VS_INT`]).
    Int(i64),
    /// `CAST(m AS REAL) / 10^scale`, correctly rounded (rows with |m| ≤ [`DEC_EXACT_VS_DEC`]).
    Ratio { m: i64, pow10: i64 },
}

impl RealOperand {
    /// The mantissa bound a row must satisfy for the REAL comparison to be exact.
    pub(crate) fn row_limit(self) -> i64 {
        match self {
            RealOperand::Int(_) => DEC_EXACT_VS_INT,
            RealOperand::Ratio { .. } => DEC_EXACT_VS_DEC,
        }
    }
}

/// The ordering operators.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Cmp {
    Lt,
    Le,
    Gt,
    Ge,
}

impl Cmp {
    pub(crate) fn sql(self) -> &'static str {
        match self {
            Cmp::Lt => "<",
            Cmp::Le => "<=",
            Cmp::Gt => ">",
            Cmp::Ge => ">=",
        }
    }

    /// `c OP x` ⇔ `x mirror(OP) c`.
    pub(crate) fn mirror(self) -> Cmp {
        match self {
            Cmp::Lt => Cmp::Gt,
            Cmp::Le => Cmp::Ge,
            Cmp::Gt => Cmp::Lt,
            Cmp::Ge => Cmp::Le,
        }
    }
}

fn pow10(scale: u8) -> i64 {
    10i64.pow(u32::from(scale))
}

impl NumConst {
    /// The constant as an exact number, if it is an inline integer or decimal.
    pub(crate) fn of(id: TermId) -> Option<NumConst> {
        match id.tag() {
            Tag::Int => id.as_int().map(NumConst::Int),
            Tag::Decimal => {
                let (m, scale, _) = decimal_parts(id);
                Some(NumConst::Dec { m, scale })
            }
            _ => None,
        }
    }

    fn floor(self) -> i64 {
        match self {
            NumConst::Int(k) => k,
            NumConst::Dec { m, scale } => m.div_euclid(pow10(scale)),
        }
    }

    fn ceil(self) -> i64 {
        match self {
            NumConst::Int(k) => k,
            NumConst::Dec { m, scale } => -(-m).div_euclid(pow10(scale)),
        }
    }

    /// The bound B such that, for every integer v, `v OP self` ⇔ `v OP B`.
    pub(crate) fn int_bound(self, op: Cmp) -> i64 {
        match op {
            Cmp::Gt | Cmp::Le => self.floor(),
            Cmp::Ge | Cmp::Lt => self.ceil(),
        }
    }

    /// The inline integer id with this value (None when the value is not an integer).
    pub(crate) fn int_id(self) -> Option<TermId> {
        match self {
            NumConst::Int(k) => TermId::from_int(k),
            NumConst::Dec { m, scale: 0 } => TermId::from_int(m),
            NumConst::Dec { .. } => None,
        }
    }

    /// `id | DOTZERO_BIT` of the inline decimals with this value (None when no inline decimal
    /// has it).
    pub(crate) fn dec_key(self) -> Option<i64> {
        let (m, scale) = match self {
            NumConst::Int(k) => (k, 0),
            NumConst::Dec { m, scale } => (m, scale),
        };
        (DEC_M_MIN..=DEC_M_MAX).contains(&m).then(|| encode_decimal(m, scale, false).raw() | DOTZERO_BIT)
    }

    /// The operand an inline decimal's `sparql_num` REAL is compared with, if that comparison
    /// can be exact.
    pub(crate) fn real_operand(self) -> Option<RealOperand> {
        match self {
            NumConst::Int(k) => Some(RealOperand::Int(k)),
            NumConst::Dec { m, scale: 0 } => Some(RealOperand::Int(m)),
            NumConst::Dec { m, scale } => {
                (m.abs() <= DEC_EXACT_VS_DEC).then(|| RealOperand::Ratio { m, pow10: pow10(scale) })
            }
        }
    }
}
