//! Term identity of computed values (§3, §4.3): results are interned only when an id is
//! required, so FILTER and ORDER BY intermediates never reach the append-only TempDict;
//! derived-integer casts and STRDT keep their exact term lazily (`Val::Typed`); sameTerm
//! compares computed values with decoded terms; and the constant fast paths treat a TEMP
//! (absent) constant as distinct from every stored id, decoding only a TEMP row id against a
//! stored constant.
use sq_core::codec::{self, XSD_DECIMAL_IRI};
use sq_core::{Literal, Term, TermId};

use super::harness::Harness;
use crate::{Val, Value, sort_key, sort_key_val};

const XSD_INT: &str = "http://www.w3.org/2001/XMLSchema#int";

#[test]
fn typed_results_keep_their_exact_term() {
    let mut h = Harness::new();
    for (expr, expected) in [
        (r#"xsd:int("05")"#, r#""5"^^xsd:int"#),
        (r#"xsd:unsignedByte(" 7 ")"#, r#""7"^^xsd:unsignedByte"#),
        (r#"STRDT("05", xsd:integer)"#, r#""05"^^xsd:integer"#),
        (r#"STRDT("1", xsd:boolean)"#, r#""1"^^xsd:boolean"#),
        (r#"STRDT("1.50", xsd:decimal)"#, r#""1.50"^^xsd:decimal"#),
        (r#"STRDT("5", xsd:short)"#, r#""5"^^xsd:short"#),
        (r#"STRDT("300", xsd:byte)"#, r#""300"^^xsd:byte"#),
        (r#"STR(xsd:int("05"))"#, r#""5""#),
        (r#"STR(STRDT("05", xsd:integer))"#, r#""05""#),
        (r#"LANG(xsd:int("1"))"#, r#""""#),
        (r#"DATATYPE(STRDT("05", xsd:integer))"#, "<http://www.w3.org/2001/XMLSchema#integer>"),
        (r#"DATATYPE(xsd:short("5"))"#, "<http://www.w3.org/2001/XMLSchema#short>"),
        (r#"xsd:int("05") + 1"#, r#""6"^^xsd:integer"#),
        (r#"-xsd:byte("5")"#, r#""-5"^^xsd:integer"#),
        (r#"STRDT("1.50", xsd:decimal) * 2"#, r#""3"^^xsd:decimal"#),
        (r#"xsd:string(STRDT("05", xsd:integer))"#, r#""5""#),
        (r#"xsd:long(xsd:int("05"))"#, r#""5"^^xsd:long"#),
        (r#"isNumeric(xsd:byte("1"))"#, r#""true"^^xsd:boolean"#),
        (r#"isLiteral(xsd:byte("1"))"#, r#""true"^^xsd:boolean"#),
        (r#"IF(true, xsd:short("7"), 1)"#, r#""7"^^xsd:short"#),
        (r#"COALESCE(?unbound, STRDT("07", xsd:integer))"#, r#""07"^^xsd:integer"#),
        (r#"xsd:int("05") = 5"#, r#""true"^^xsd:boolean"#),
        (r#"xsd:int("05") < STRDT("06", xsd:integer)"#, r#""true"^^xsd:boolean"#),
    ] {
        assert_eq!(h.show(expr, &[]), expected, "{expr}");
    }
    // A canonical-form result is a plain value: inline when encodable.
    assert_eq!(h.eval(r#"STRDT("5", xsd:integer)"#, &[]), TermId::from_int(5));
    let five_dot_zero = codec::encode_inline_literal("5.0", XSD_DECIMAL_IRI, None);
    assert!(five_dot_zero.is_some());
    assert_eq!(h.eval(r#"STRDT("5.0", xsd:decimal)"#, &[]), five_dot_zero);
    // A stored term gets its dictionary id with canonical = true.
    let stored = h.term(r#""5"^^xsd:int"#);
    assert_eq!(h.eval(r#"xsd:int("05")"#, &[]), Some(stored));
}

#[test]
fn same_term_of_typed_results() {
    let mut h = Harness::new();
    let stored_int = h.term(r#""5"^^xsd:int"#);
    let integer = h.term("5");
    let temp_int = h.env.resolver.temp(Term::Literal(Literal::typed("5", XSD_INT)));
    let padded = h.term(r#""05"^^xsd:integer"#);
    for (expr, y, expected) in [
        (r#"sameTerm(xsd:int("05"), ?y)"#, stored_int, true),
        (r#"sameTerm(xsd:int("05"), ?y)"#, temp_int, true),
        (r#"sameTerm(xsd:int("05"), ?y)"#, integer, false),
        (r#"sameTerm(?y, xsd:int("5"))"#, stored_int, true),
        (r#"sameTerm(STRDT("05", xsd:integer), ?y)"#, padded, true),
        (r#"sameTerm(STRDT("05", xsd:integer), ?y)"#, integer, false),
        (r#"sameTerm(STRDT("5", xsd:integer), ?y)"#, integer, true),
        (r#"sameTerm(xsd:int("05"), xsd:int("5"))"#, integer, true),
        (r#"sameTerm(STRDT("05", xsd:integer), STRDT("5", xsd:integer))"#, integer, false),
        (r#"sameTerm(xsd:int("5"), "5"^^xsd:int)"#, integer, true),
    ] {
        assert_eq!(h.ebv(expr, &[("y", y)]), Some(expected), "{expr} with ?y = {}", h.render(Some(y)));
    }
}

#[test]
fn same_term_of_computed_values_and_ids() {
    let mut h = Harness::new();
    let x = h.term(r#""a-stored""#);
    let long = h.term(r#""a-stored-suffix""#);
    let long_temp = h.env.resolver.temp(Term::Literal(Literal::simple("a-stored-suffix")));
    let other = h.term(r#""a-stored-other""#);
    let iri = h.term("<http://example.org/a-stored-suffix>");
    let lang = h.term(r#""a-stored-suffix"@en"#);
    let dbl = h.term(r#""1.5"^^xsd:double"#);
    let dbl_e = h.term(r#""1.5e0"^^xsd:double"#);
    let short = h.term(r#""st""#);
    for (expr, y, expected) in [
        (r#"sameTerm(CONCAT(?x, "-suffix"), ?y)"#, long, true),
        (r#"sameTerm(CONCAT(?x, "-suffix"), ?y)"#, long_temp, true),
        (r#"sameTerm(CONCAT(?x, "-suffix"), ?y)"#, other, false),
        (r#"sameTerm(CONCAT(?x, "-suffix"), ?y)"#, iri, false),
        (r#"sameTerm(CONCAT(?x, "-suffix"), ?y)"#, lang, false),
        (r#"sameTerm(?y, STRLANG(CONCAT(?x, "-suffix"), "EN"))"#, lang, true),
        (r#"sameTerm(IRI(CONCAT("http://example.org/", ?x, "-suffix")), ?y)"#, iri, true),
        (r#"sameTerm(?y * 1, ?y)"#, dbl, true),
        (r#"sameTerm(?y * 1, ?y)"#, dbl_e, false),
        (r#"sameTerm(SUBSTR(?x, 3, 2), ?y)"#, short, true),
        (r#"sameTerm(SUBSTR(?x, 3, 2), "st")"#, short, true),
        (r#"sameTerm(CONCAT(?x, "-suffix"), "a-stored-suffix")"#, short, true),
        (r#"sameTerm(CONCAT(?x, "-suffix"), "not the same")"#, short, false),
    ] {
        let before = h.env.resolver.temps.len();
        assert_eq!(h.ebv(expr, &[("x", x), ("y", y)]), Some(expected), "{expr} with ?y = {}", h.render(Some(y)));
        // Only the query's own constants may be added (at lowering), never per-row values.
        let c = h.compile(expr);
        let after_compile = h.env.resolver.temps.len();
        let row = h.row(&[("x", x), ("y", y)]);
        h.env.with(|cx| c.ebv(&row, cx)).expect("fatal");
        assert_eq!(h.env.resolver.temps.len(), after_compile, "{expr}: evaluation interned a value");
        assert!(after_compile >= before);
    }
}

/// FILTER and ORDER BY over many rows: the TempDict only holds what lowering added.
#[test]
fn filter_and_order_by_intermediates_are_not_interned() {
    let mut h = Harness::new();
    let xs: Vec<TermId> = (0..200).map(|i| h.term(&format!("\"{}\"^^xsd:integer", 1_000_000 + i))).collect();
    let y = h.term("<http://example.org/y>");
    let filters = [
        "xsd:int(?x) > 5",
        "xsd:short(?x) > 5",
        "STRDT(STR(?x), xsd:int) > 5",
        "STRDT(CONCAT(\"0\", STR(?x)), xsd:integer) > 5",
        "DATATYPE(xsd:int(?x)) = xsd:int",
        "STR(xsd:long(?x)) != \"\"",
        "sameTerm(xsd:int(?x), ?y)",
        "sameTerm(CONCAT(STR(?x), \"-suffix\"), ?y)",
        "xsd:int(?x) IN (1, 2, ?y)",
        "COALESCE(xsd:int(?x), 1) = 1",
        "xsd:int(?x)",
    ];
    for expr in filters {
        let c = h.compile(expr);
        let before = h.env.resolver.temps.len();
        for x in &xs {
            let row = h.row(&[("x", *x), ("y", y)]);
            h.env.with(|cx| c.ebv(&row, cx)).expect("fatal");
        }
        assert_eq!(h.env.resolver.temps.len(), before, "FILTER {expr}");
    }
    for expr in ["xsd:int(?x)", "STRDT(STR(?x), xsd:unsignedInt)", "STRDT(CONCAT(\"+\", STR(?x)), xsd:integer)"] {
        let c = h.compile(expr);
        let before = h.env.resolver.temps.len();
        let mut keys = vec![];
        for x in &xs {
            let row = h.row(&[("x", *x)]);
            keys.push(h.env.with(|cx| c.sort_key(&row, cx)).expect("fatal"));
        }
        assert_eq!(h.env.resolver.temps.len(), before, "ORDER BY {expr}");
        assert!(keys.windows(2).all(|w| w[0] < w[1]), "ORDER BY {expr} follows the values");
        // The key of the lazy value is the key of its interned id.
        let row = h.row(&[("x", xs[3])]);
        let (lazy, interned) = h
            .env
            .with(|cx| -> sq_core::Result<_> {
                let v = c.eval_val(&row, cx)?.expect("bound");
                let lazy = sort_key_val(&v, cx)?;
                let id = crate::id_of_val(v, cx, false)?;
                Ok((lazy, sort_key(id, cx)?))
            })
            .expect("fatal");
        assert_eq!(lazy, interned, "ORDER BY {expr}");
    }
}

#[test]
fn lazy_values_report_value_and_term() {
    let typed = Val::Typed { value: Value::Integer(5), datatype: crate::XsdType::Int, lex: None };
    assert!(matches!(typed.computed_value(), Some(Value::Integer(5))));
    assert_eq!(typed.computed_term(), Some(Term::Literal(Literal::typed("5", XSD_INT))));
    let padded = Val::Typed { value: Value::Integer(5), datatype: crate::XsdType::Integer, lex: Some("05".into()) };
    assert_eq!(
        padded.computed_term(),
        Some(Term::Literal(Literal::typed("05", "http://www.w3.org/2001/XMLSchema#integer")))
    );
    assert_eq!(crate::inline_id_of_val(&padded), None);
    assert_eq!(crate::inline_id_of_val(&Val::V(Value::Integer(5))), TermId::from_int(5));
    assert!(Val::Id(TermId::from_int(5).unwrap()).computed_term().is_none());
}

/// Constant fast paths (`IdEq`, `InIds`, sameTerm / `=` against a constant) and the general
/// path agree on TEMP row ids, TEMP constants and typed literals.
#[test]
fn constant_aliasing_rules() {
    let mut h = Harness::new();
    let stored_iri = h.term("<http://example.org/a>");
    let temp_of_stored = h.env.resolver.temp(Term::Iri("http://example.org/a".into()));
    let temp_absent = h.env.resolver.temp(Term::Iri("http://example.org/absent".into()));
    let stored_lit = h.term(r#""a long simple literal""#);
    let temp_lit = h.env.resolver.temp(Term::Literal(Literal::simple("a long simple literal")));
    let dbl = h.term(r#""5e0"^^xsd:double"#);
    let odd = h.term(r#""x"^^<http://example/dt>"#);
    for (expr, x, expected) in [
        // A TEMP row id may denote a stored constant's term.
        ("?x = <http://example.org/a>", temp_of_stored, Some(true)),
        ("?x != <http://example.org/a>", temp_of_stored, Some(false)),
        ("?x IN (<http://example.org/b>, <http://example.org/a>)", temp_of_stored, Some(true)),
        ("sameTerm(?x, <http://example.org/a>)", temp_of_stored, Some(true)),
        ("sameTerm(<http://example.org/a>, ?x)", temp_of_stored, Some(true)),
        ("?x = \"a long simple literal\"", temp_lit, Some(true)),
        // A TEMP constant (absent from the store) never denotes a stored term...
        ("?x = <http://example.org/absent>", stored_iri, Some(false)),
        ("?x != <http://example.org/absent>", stored_iri, Some(true)),
        ("?x IN (<http://example.org/absent>)", stored_iri, Some(false)),
        ("sameTerm(?x, <http://example.org/absent>)", stored_iri, Some(false)),
        ("?x = \"an absent long simple literal\"", stored_lit, Some(false)),
        // ...but equals the execution's TEMP id of the same term.
        ("?x = <http://example.org/absent>", temp_absent, Some(true)),
        ("?x IN (<http://example.org/b>, <http://example.org/absent>)", temp_absent, Some(true)),
        // Literal `=` is value equality: an absent typed constant can equal a stored value.
        ("?x = \"5.0E0\"^^xsd:double", dbl, Some(true)),
        ("?x IN (\"5.0E0\"^^xsd:double)", dbl, Some(true)),
        // Unknown datatype vs a simple literal: a type error.
        ("?x = \"an absent long simple literal\"", odd, None),
        ("?x = \"abc\"", odd, None),
        ("?x = <http://example.org/absent>", odd, Some(false)),
    ] {
        assert_eq!(h.ebv(expr, &[("x", x)]), expected, "{expr} with ?x = {}", h.render(Some(x)));
    }
}

#[test]
fn constant_replace_is_prepared_once_and_reused() {
    let mut h = Harness::new();
    let c = h.compile(r#"REPLACE(?x, "(o+)", "[$1]")"#);
    let bad = h.compile(r#"REPLACE(?x, "o", "$")"#);
    let empty = h.compile(r#"REPLACE(?x, "o*", "x")"#);
    for (text, expected) in [("foo", r#""f[oo]""#), ("bar", r#""bar""#), ("o o", r#""[o] [o]""#)] {
        let x = h.term(&format!("\"{text}\""));
        let row = h.row(&[("x", x)]);
        let r = h.env.with(|cx| c.eval(&row, cx, true)).expect("fatal");
        assert_eq!(h.render(r), expected, "{text}");
        assert_eq!(h.env.with(|cx| bad.eval(&row, cx, true)).expect("fatal"), None, "invalid replacement");
        assert_eq!(h.env.with(|cx| empty.eval(&row, cx, true)).expect("fatal"), None, "empty match");
    }
    // A dynamic replacement is prepared per row.
    let dynamic = h.compile(r#"REPLACE("foo", "(o)", ?r)"#);
    for (r, expected) in [("<$1>", r#""f<o><o>""#), ("$", "ERROR"), ("-", r#""f--""#)] {
        let rid = h.term(&format!("\"{r}\""));
        let row = h.row(&[("r", rid)]);
        let out = h.env.with(|cx| dynamic.eval(&row, cx, true)).expect("fatal");
        assert_eq!(h.render(out), expected, "{r}");
    }
}

/// A TEMP query constant is canonical (absent from the store), so a pass-through constant is
/// returned as is; a TEMP id from a slot (which the planner may leave non-canonical) is still
/// re-interned to the stored id when a canonical id is required.
#[test]
fn temp_constants_pass_through_and_temp_slots_are_canonicalised() {
    let mut h = Harness::new();
    let stored = h.term("<http://example.org/stored-iri>");
    let temp_of_stored = h.env.resolver.temp(Term::Iri("http://example.org/stored-iri".into()));
    for expr in [
        "COALESCE(?x, <http://example.org/absent-iri>)",
        "IF(BOUND(?x), ?x, <http://example.org/absent-iri>)",
        "IRI(COALESCE(?x, <http://example.org/absent-iri>))",
    ] {
        let c = h.compile(expr);
        let row = h.row(&[]);
        let out = h.env.with(|cx| c.eval(&row, cx, true)).expect("fatal").expect("bound");
        assert!(out.is_temp(), "{expr}");
        assert_eq!(h.render(Some(out)), "<http://example.org/absent-iri>", "{expr}");
        // The execution's TEMP id of the constant's term is that same canonical id.
        let row = h.row(&[("x", out)]);
        assert_eq!(h.env.with(|cx| c.eval(&row, cx, true)).expect("fatal"), Some(out), "{expr}");
        // A TEMP slot id of a stored term: canonical = true gives the stored id.
        let row = h.row(&[("x", temp_of_stored)]);
        assert_eq!(h.env.with(|cx| c.eval(&row, cx, true)).expect("fatal"), Some(stored), "{expr}");
        assert_eq!(h.env.with(|cx| c.eval(&row, cx, false)).expect("fatal"), Some(temp_of_stored), "{expr}");
    }
}
