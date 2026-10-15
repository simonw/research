//! XSD datatypes known to the value model (§6.2): IRIs, well-known dictionary ids, strict
//! lexical-space parsers and the derived-integer facets.
//!
//! A literal whose datatype is one of these and whose lexical form is outside the lexical
//! space (or violates a facet, e.g. `"300"^^xsd:byte`) decodes to `Value::IllTyped`; any other
//! datatype decodes to `Value::Other`.
use std::str::FromStr;

use oxsdatatypes::Decimal;
use sq_core::codec::{WELL_KNOWN, XSD};
use sq_core::{Tag, TermId};

macro_rules! xsd_types {
    ($($variant:ident = $local:literal @ $row:literal,)*) => {
        /// Every XSD datatype the evaluator gives a value to.
        #[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
        pub enum XsdType {
            $($variant,)*
        }

        impl XsdType {
            /// Every variant, in declaration order.
            pub const ALL: &'static [XsdType] = &[$(XsdType::$variant,)*];

            /// Classifies a datatype IRI.
            pub fn from_iri(iri: &str) -> Option<XsdType> {
                match iri.strip_prefix(XSD)? {
                    $($local => Some(XsdType::$variant),)*
                    _ => None,
                }
            }

            /// Local name, e.g. `"dateTime"`.
            pub const fn local_name(self) -> &'static str {
                match self {
                    $(XsdType::$variant => $local,)*
                }
            }

            /// Full datatype IRI.
            pub const fn iri(self) -> &'static str {
                match self {
                    $(XsdType::$variant => concat!("http://www.w3.org/2001/XMLSchema#", $local),)*
                }
            }

            /// Constant dictionary id of the datatype IRI: every known XSD type has a reserved
            /// `WELL_KNOWN` row (inserted by `sparql_init` and `MemStore::new`), cross-checked
            /// against `sq_core::codec::WELL_KNOWN` by a unit test.
            pub const fn iri_id(self) -> TermId {
                match self {
                    $(XsdType::$variant => TermId::from_dict($row, Tag::Iri),)*
                }
            }
        }
    };
}

xsd_types! {
    String = "string" @ 1,
    Boolean = "boolean" @ 6,
    Decimal = "decimal" @ 3,
    Integer = "integer" @ 2,
    Long = "long" @ 18,
    Int = "int" @ 19,
    Short = "short" @ 20,
    Byte = "byte" @ 21,
    NonNegativeInteger = "nonNegativeInteger" @ 22,
    PositiveInteger = "positiveInteger" @ 23,
    NonPositiveInteger = "nonPositiveInteger" @ 24,
    NegativeInteger = "negativeInteger" @ 25,
    UnsignedLong = "unsignedLong" @ 26,
    UnsignedInt = "unsignedInt" @ 27,
    UnsignedShort = "unsignedShort" @ 28,
    UnsignedByte = "unsignedByte" @ 29,
    Float = "float" @ 5,
    Double = "double" @ 4,
    DateTime = "dateTime" @ 7,
    Date = "date" @ 8,
    Time = "time" @ 9,
    GYear = "gYear" @ 30,
    GYearMonth = "gYearMonth" @ 31,
    GMonth = "gMonth" @ 32,
    GMonthDay = "gMonthDay" @ 33,
    GDay = "gDay" @ 34,
    Duration = "duration" @ 15,
    YearMonthDuration = "yearMonthDuration" @ 17,
    DayTimeDuration = "dayTimeDuration" @ 16,
}

impl XsdType {
    /// Value range of `xsd:integer` and the derived integer types (None for other types).
    /// Bounds beyond i64 are clamped: values outside i64 are ill-typed anyway (§6.2).
    pub fn integer_range(self) -> Option<(i64, i64)> {
        Some(match self {
            XsdType::Integer | XsdType::Long => (i64::MIN, i64::MAX),
            XsdType::Int => (i32::MIN as i64, i32::MAX as i64),
            XsdType::Short => (i16::MIN as i64, i16::MAX as i64),
            XsdType::Byte => (i8::MIN as i64, i8::MAX as i64),
            XsdType::NonNegativeInteger | XsdType::UnsignedLong => (0, i64::MAX),
            XsdType::PositiveInteger => (1, i64::MAX),
            XsdType::NonPositiveInteger => (i64::MIN, 0),
            XsdType::NegativeInteger => (i64::MIN, -1),
            XsdType::UnsignedInt => (0, u32::MAX as i64),
            XsdType::UnsignedShort => (0, u16::MAX as i64),
            XsdType::UnsignedByte => (0, u8::MAX as i64),
            _ => return None,
        })
    }

    pub fn is_integer_family(self) -> bool {
        self.integer_range().is_some()
    }
}

/// Dictionary id of a well-known IRI (rows 1..=63), without any lookup.
pub fn well_known_iri_id(iri: &str) -> Option<TermId> {
    if let Some(t) = XsdType::from_iri(iri) {
        return Some(t.iri_id());
    }
    WELL_KNOWN.iter().find(|(_, w)| *w == iri).map(|(row, _)| TermId::from_dict(*row, Tag::Iri))
}

// ---------------------------------------------------------------- strict lexical parsers

/// `xsd:integer` lexical space `[+-]?[0-9]+`; values beyond i64 are rejected.
pub fn parse_integer(lex: &str) -> Option<i64> {
    let digits = lex.strip_prefix(['+', '-']).unwrap_or(lex);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    i64::from_str(lex).ok()
}

/// `xsd:decimal` lexical space `[+-]?([0-9]+(\.[0-9]*)?|\.[0-9]+)` (no exponent).
pub fn parse_decimal(lex: &str) -> Option<Decimal> {
    // oxsdatatypes implements exactly this grammar (plus range checks).
    Decimal::from_str(lex).ok()
}

/// Validates the mantissa/exponent grammar shared by `xsd:float` and `xsd:double`:
/// `[+-]?([0-9]+(\.[0-9]*)?|\.[0-9]+)([Ee][+-]?[0-9]+)?`.
fn is_float_lexical(lex: &str) -> bool {
    let b = lex.as_bytes();
    let mut i = 0;
    if matches!(b.first(), Some(b'+' | b'-')) {
        i += 1;
    }
    let int_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let mut digits = i - int_start;
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let frac_start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        digits += i - frac_start;
    }
    if digits == 0 {
        return false;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if matches!(b.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let exp_start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == exp_start {
            return false;
        }
    }
    i == b.len()
}

/// `xsd:double` lexical space (XSD 1.1: `INF`, `+INF`, `-INF`, `NaN` and the decimal/exponent
/// grammar). Rust's parser alone would also accept `inf`, `infinity`, `nan`.
pub fn parse_double(lex: &str) -> Option<f64> {
    match lex {
        "INF" | "+INF" => Some(f64::INFINITY),
        "-INF" => Some(f64::NEG_INFINITY),
        "NaN" => Some(f64::NAN),
        _ if is_float_lexical(lex) => f64::from_str(lex).ok(),
        _ => None,
    }
}

/// `xsd:float` lexical space (same grammar as double, rounded to f32 directly).
pub fn parse_float(lex: &str) -> Option<f32> {
    match lex {
        "INF" | "+INF" => Some(f32::INFINITY),
        "-INF" => Some(f32::NEG_INFINITY),
        "NaN" => Some(f32::NAN),
        _ if is_float_lexical(lex) => f32::from_str(lex).ok(),
        _ => None,
    }
}

/// `xsd:boolean` lexical space: `true`, `false`, `1`, `0`.
pub fn parse_boolean(lex: &str) -> Option<bool> {
    match lex {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

/// XML Schema whitespace "collapse" trimming used by casts from strings (§6.6: string → number
/// trims whitespace). Only the four XML whitespace characters are removed.
pub fn trim_xml_ws(s: &str) -> &str {
    s.trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_matches_well_known_rows() {
        for t in XsdType::ALL {
            let iri = format!("{XSD}{}", t.local_name());
            assert_eq!(XsdType::from_iri(&iri), Some(*t));
            assert_eq!(t.iri(), iri);
            let row = t.iri_id().dict_row();
            assert!(WELL_KNOWN.iter().any(|(r, w)| *r == row && *w == iri), "{iri}");
            assert_eq!(well_known_iri_id(&iri), Some(t.iri_id()));
        }
        assert_eq!(XsdType::Integer.iri_id(), sq_core::codec::XSD_INTEGER);
        assert_eq!(XsdType::Double.iri_id(), sq_core::codec::XSD_DOUBLE);
        assert_eq!(well_known_iri_id(sq_core::codec::RDF_LANG_STRING_IRI), Some(sq_core::codec::RDF_LANG_STRING));
        assert_eq!(XsdType::from_iri("http://www.w3.org/2001/XMLSchema#anyURI"), None);
        assert_eq!(well_known_iri_id("http://example.org/x"), None);
    }

    #[test]
    fn integer_lexical_space() {
        assert_eq!(parse_integer("0"), Some(0));
        assert_eq!(parse_integer("+12"), Some(12));
        assert_eq!(parse_integer("-012"), Some(-12));
        assert_eq!(parse_integer("9223372036854775807"), Some(i64::MAX));
        for bad in ["", "+", "-", "1.0", " 1", "1 ", "1e3", "0x10", "9223372036854775808", "+-1"] {
            assert_eq!(parse_integer(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn decimal_lexical_space() {
        for (lex, s) in [("1", "1"), ("+1.50", "1.5"), ("-.5", "-0.5"), ("5.", "5"), ("-0", "0"), ("0.0", "0")] {
            assert_eq!(parse_decimal(lex).map(|d| d.to_string()).as_deref(), Some(s), "{lex}");
        }
        for bad in ["", ".", "-", "1e2", "1.2.3", " 1", "INF", "NaN"] {
            assert!(parse_decimal(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn double_lexical_space() {
        for (lex, v) in [("1", 1.0), ("-1.5e3", -1500.0), (".5", 0.5), ("5.", 5.0), ("1E-2", 0.01), ("+0", 0.0)] {
            assert_eq!(parse_double(lex), Some(v), "{lex}");
            assert_eq!(parse_float(lex), Some(v as f32), "{lex}");
        }
        assert_eq!(parse_double("INF"), Some(f64::INFINITY));
        assert_eq!(parse_double("+INF"), Some(f64::INFINITY));
        assert_eq!(parse_double("-INF"), Some(f64::NEG_INFINITY));
        assert!(parse_double("NaN").unwrap().is_nan());
        for bad in ["", "inf", "infinity", "nan", "-NaN", "1e", "e3", ".", "1.0.0", " 1", "1d", "0x1p3", "+-1"] {
            assert_eq!(parse_double(bad), None, "{bad:?}");
            assert_eq!(parse_float(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn derived_integer_facets() {
        assert_eq!(XsdType::Byte.integer_range(), Some((-128, 127)));
        assert_eq!(XsdType::UnsignedByte.integer_range(), Some((0, 255)));
        assert_eq!(XsdType::PositiveInteger.integer_range(), Some((1, i64::MAX)));
        assert_eq!(XsdType::NegativeInteger.integer_range(), Some((i64::MIN, -1)));
        assert!(XsdType::Double.integer_range().is_none());
        assert!(XsdType::UnsignedLong.is_integer_family());
    }

    #[test]
    fn xml_whitespace_trim() {
        assert_eq!(trim_xml_ws(" \t\n 12 \r"), "12");
        assert_eq!(trim_xml_ws("\u{a0}1"), "\u{a0}1");
    }
}
