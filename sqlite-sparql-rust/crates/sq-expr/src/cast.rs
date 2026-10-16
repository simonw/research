//! XSD constructor casts (`xsd:integer(?x)` etc., arriving as `Function::Custom(xsd:*)`),
//! following the XPath casting table restricted to SPARQL 1.1 §17.5 plus the date/time,
//! duration and derived-integer types the value model knows:
//!
//! * from a simple literal / xsd:string: whitespace-trimmed lexical parse of the target type;
//! * boolean → numeric is 1/0, numeric → boolean is "non-zero and not NaN";
//! * float/double → integer/decimal truncate toward zero (NaN/INF are errors);
//! * any typed value → string uses the canonical form; IRIs cast to string;
//! * dateTime ↔ date, dateTime → time and the g* types, durations within their family;
//! * Other / IllTyped literals and blank nodes only cast to xsd:string (lexical form) or not
//!   at all.
use std::str::FromStr;

use oxsdatatypes::{
    Date, DateTime, DayTimeDuration, Decimal, Double, Duration, Float, GDay, GMonth, GMonthDay, GYear, GYearMonth, Time,
    YearMonthDuration,
};

use crate::numeric::{Num, dec_trunc_to_i64};
use crate::value::Value;
use crate::xsd::{self, XsdType, trim_xml_ws};

/// Result of a cast: a value of a primitive type, or an integer of a derived integer type
/// (which the value model folds into `Integer`, so it is returned with its datatype).
#[derive(Clone, Debug)]
pub enum Casted {
    V(Value),
    DerivedInteger(i64, XsdType),
}

fn to_integer(v: &Value) -> Option<i64> {
    match v {
        Value::Integer(i) => Some(*i),
        Value::Decimal(d) => dec_trunc_to_i64(*d),
        Value::Float(_) | Value::Double(_) => {
            let f = v.as_num()?.to_f64();
            if !f.is_finite() {
                return None;
            }
            let t = f.trunc();
            // i64::MIN is exactly representable; values ≥ 2^63 are not i64.
            (-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&t).then_some(t as i64)
        }
        Value::Boolean(b) => Some(*b as i64),
        Value::Str(s) => xsd::parse_integer(trim_xml_ws(s)),
        _ => None,
    }
}

fn to_decimal(v: &Value) -> Option<Decimal> {
    match v {
        Value::Integer(i) => Some(Decimal::from(*i)),
        Value::Decimal(d) => Some(*d),
        Value::Float(f) => Decimal::try_from(*f).ok().filter(|_| f32::from(*f).is_finite()),
        Value::Double(d) => Decimal::try_from(*d).ok().filter(|_| f64::from(*d).is_finite()),
        Value::Boolean(b) => Some(Decimal::from(*b)),
        Value::Str(s) => xsd::parse_decimal(trim_xml_ws(s)),
        _ => None,
    }
}

fn to_double(v: &Value) -> Option<f64> {
    match v {
        Value::Boolean(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::Str(s) => xsd::parse_double(trim_xml_ws(s)),
        v => Some(v.as_num()?.to_f64()),
    }
}

fn to_float(v: &Value) -> Option<f32> {
    match v {
        Value::Boolean(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::Str(s) => xsd::parse_float(trim_xml_ws(s)),
        v => Some(v.as_num()?.to_f32()),
    }
}

fn to_boolean(v: &Value) -> Option<bool> {
    match v {
        Value::Boolean(b) => Some(*b),
        Value::Str(s) => xsd::parse_boolean(trim_xml_ws(s)),
        v => Some(!Num::is_zero_or_nan(v.as_num()?)),
    }
}

fn parse_str<T: FromStr>(v: &Value) -> Option<T> {
    match v {
        Value::Str(s) => T::from_str(trim_xml_ws(s)).ok(),
        _ => None,
    }
}

/// Casts `v` to `target`. None = SPARQL error.
pub fn cast(target: XsdType, v: &Value) -> Option<Casted> {
    let value = match target {
        XsdType::String => match v {
            Value::BNode(_) => return None,
            v => Value::Str(v.lexical().into()),
        },
        XsdType::Boolean => Value::Boolean(to_boolean(v)?),
        XsdType::Integer => Value::Integer(to_integer(v)?),
        XsdType::Decimal => Value::Decimal(to_decimal(v)?),
        XsdType::Float => Value::Float(Float::from(to_float(v)?)),
        XsdType::Double => Value::Double(Double::from(to_double(v)?)),
        XsdType::DateTime => Value::DateTime(match v {
            Value::DateTime(d) => *d,
            Value::Date(d) => DateTime::try_from(*d).ok()?,
            v => parse_str(v)?,
        }),
        XsdType::Date => Value::Date(match v {
            Value::Date(d) => *d,
            Value::DateTime(d) => Date::try_from(*d).ok()?,
            v => parse_str(v)?,
        }),
        XsdType::Time => Value::Time(match v {
            Value::Time(t) => *t,
            Value::DateTime(d) => Time::from(*d),
            v => parse_str(v)?,
        }),
        XsdType::GYear => Value::GYear(match v {
            Value::GYear(x) => *x,
            Value::GYearMonth(x) => GYear::try_from(*x).ok()?,
            Value::Date(x) => GYear::try_from(*x).ok()?,
            Value::DateTime(x) => GYear::try_from(*x).ok()?,
            v => parse_str(v)?,
        }),
        XsdType::GYearMonth => Value::GYearMonth(match v {
            Value::GYearMonth(x) => *x,
            Value::Date(x) => GYearMonth::from(*x),
            Value::DateTime(x) => GYearMonth::try_from(*x).ok()?,
            v => parse_str(v)?,
        }),
        XsdType::GMonth => Value::GMonth(match v {
            Value::GMonth(x) => *x,
            Value::GYearMonth(x) => GMonth::from(*x),
            Value::GMonthDay(x) => GMonth::from(*x),
            Value::Date(x) => GMonth::from(*x),
            Value::DateTime(x) => GMonth::from(*x),
            v => parse_str(v)?,
        }),
        XsdType::GMonthDay => Value::GMonthDay(match v {
            Value::GMonthDay(x) => *x,
            Value::Date(x) => GMonthDay::from(*x),
            Value::DateTime(x) => GMonthDay::from(*x),
            v => parse_str(v)?,
        }),
        XsdType::GDay => Value::GDay(match v {
            Value::GDay(x) => *x,
            Value::GMonthDay(x) => GDay::from(*x),
            Value::Date(x) => GDay::from(*x),
            Value::DateTime(x) => GDay::from(*x),
            v => parse_str(v)?,
        }),
        XsdType::Duration => Value::Duration(match v {
            Value::Duration(d) => *d,
            Value::YearMonthDuration(d) => Duration::from(*d),
            Value::DayTimeDuration(d) => Duration::from(*d),
            v => parse_str(v)?,
        }),
        XsdType::YearMonthDuration => Value::YearMonthDuration(match v {
            Value::YearMonthDuration(d) => *d,
            Value::Duration(d) => YearMonthDuration::try_from(*d).ok()?,
            Value::DayTimeDuration(_) => YearMonthDuration::new(0),
            v => parse_str(v)?,
        }),
        XsdType::DayTimeDuration => Value::DayTimeDuration(match v {
            Value::DayTimeDuration(d) => *d,
            Value::Duration(d) => DayTimeDuration::try_from(*d).ok()?,
            Value::YearMonthDuration(_) => DayTimeDuration::new(0),
            v => parse_str(v)?,
        }),
        derived => {
            let (lo, hi) = derived.integer_range().expect("remaining targets are derived integers");
            let i = to_integer(v)?;
            return (lo..=hi).contains(&i).then_some(Casted::DerivedInteger(i, derived));
        }
    };
    Some(Casted::V(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(x: &str) -> Value {
        Value::Str(x.into())
    }

    fn show(c: Option<Casted>) -> String {
        match c {
            None => "ERR".into(),
            Some(Casted::V(v)) => format!("{}^^{}", v.lexical(), v.xsd_type().map(XsdType::local_name).unwrap_or("?")),
            Some(Casted::DerivedInteger(i, t)) => format!("{i}^^{}", t.local_name()),
        }
    }

    #[test]
    fn w3c_cast_table_from_strings() {
        // sparql11/cast: (input string, integer, decimal, double, boolean)
        let rows = [
            ("string", "ERR", "ERR", "ERR", "ERR"),
            ("-10.2E3", "ERR", "ERR", "-10200^^double", "ERR"),
            ("+33.3300", "ERR", "33.33^^decimal", "33.33^^double", "ERR"),
            ("0.0", "ERR", "0^^decimal", "0^^double", "ERR"),
            ("0", "0^^integer", "0^^decimal", "0^^double", "false^^boolean"),
            ("0E1", "ERR", "ERR", "0^^double", "ERR"),
            ("1.5", "ERR", "1.5^^decimal", "1.5^^double", "ERR"),
            ("1", "1^^integer", "1^^decimal", "1^^double", "true^^boolean"),
            ("1E0", "ERR", "ERR", "1^^double", "ERR"),
            ("13", "13^^integer", "13^^decimal", "13^^double", "ERR"),
            ("2002-10-10T17:00:00Z", "ERR", "ERR", "ERR", "ERR"),
            ("false", "ERR", "ERR", "ERR", "false^^boolean"),
            ("true", "ERR", "ERR", "ERR", "true^^boolean"),
        ];
        for (input, int, dec, dbl, b) in rows {
            assert_eq!(show(cast(XsdType::Integer, &s(input))), int, "integer({input})");
            assert_eq!(show(cast(XsdType::Decimal, &s(input))), dec, "decimal({input})");
            assert_eq!(show(cast(XsdType::Double, &s(input))), dbl, "double({input})");
            assert_eq!(show(cast(XsdType::Boolean, &s(input))), b, "boolean({input})");
            assert_eq!(show(cast(XsdType::String, &s(input))), format!("{input}^^string"));
        }
    }

    #[test]
    fn w3c_cast_table_from_typed_values() {
        let f = |x: f32| Value::Float(Float::from(x));
        let d = |x: &str| Value::Decimal(Decimal::from_str(x).unwrap());
        assert_eq!(show(cast(XsdType::Integer, &f(-7.875))), "-7^^integer");
        assert_eq!(show(cast(XsdType::Integer, &d("-2.5"))), "-2^^integer");
        assert_eq!(show(cast(XsdType::Integer, &Value::Boolean(true))), "1^^integer");
        assert_eq!(show(cast(XsdType::Decimal, &f(1.25))), "1.25^^decimal");
        assert_eq!(show(cast(XsdType::Decimal, &Value::Boolean(false))), "0^^decimal");
        assert_eq!(show(cast(XsdType::String, &d("1.0"))), "1^^string");
        assert_eq!(show(cast(XsdType::String, &Value::Double(Double::from(1.0)))), "1^^string");
        assert_eq!(show(cast(XsdType::Boolean, &Value::Double(Double::NAN))), "false^^boolean");
        assert_eq!(show(cast(XsdType::Boolean, &f(-7.875))), "true^^boolean");
        assert_eq!(show(cast(XsdType::Float, &Value::Integer(1))), "1^^float");
        assert_eq!(show(cast(XsdType::Integer, &Value::Double(Double::INFINITY))), "ERR");
        assert_eq!(show(cast(XsdType::Decimal, &Value::Double(Double::NAN))), "ERR");
        assert_eq!(show(cast(XsdType::Integer, &Value::Double(Double::from(1e19)))), "ERR");
        assert_eq!(show(cast(XsdType::String, &Value::Iri("http://example.org/z".into()))), "http://example.org/z^^string");
        assert_eq!(show(cast(XsdType::Integer, &Value::Iri("http://x".into()))), "ERR");
        assert_eq!(show(cast(XsdType::String, &Value::BNode("b0".into()))), "ERR");
    }

    #[test]
    fn date_and_derived_casts() {
        let dt = Value::DateTime(DateTime::from_str("2002-10-10T17:00:00-05:00").unwrap());
        assert_eq!(show(cast(XsdType::Date, &dt)), "2002-10-10-05:00^^date");
        assert_eq!(show(cast(XsdType::Time, &dt)), "17:00:00-05:00^^time");
        assert_eq!(show(cast(XsdType::GYear, &dt)), "2002-05:00^^gYear");
        let date = Value::Date(Date::from_str("2002-10-10").unwrap());
        assert_eq!(show(cast(XsdType::DateTime, &date)), "2002-10-10T00:00:00^^dateTime");
        assert_eq!(show(cast(XsdType::DateTime, &s(" 2002-10-10T17:00:00Z "))), "2002-10-10T17:00:00Z^^dateTime");
        assert_eq!(show(cast(XsdType::Integer, &dt)), "ERR");
        assert_eq!(show(cast(XsdType::Byte, &s("127"))), "127^^byte");
        assert_eq!(show(cast(XsdType::Byte, &s("128"))), "ERR");
        assert_eq!(show(cast(XsdType::UnsignedInt, &Value::Integer(-1))), "ERR");
        assert_eq!(show(cast(XsdType::DayTimeDuration, &s("PT1H"))), "PT1H^^dayTimeDuration");
        assert_eq!(show(cast(XsdType::YearMonthDuration, &s("PT1H"))), "ERR");
        let other = Value::Other { lex: "x".into(), datatype: "http://example/dt".into() };
        assert_eq!(show(cast(XsdType::String, &other)), "x^^string");
        assert_eq!(show(cast(XsdType::Integer, &other)), "ERR");
    }
}
