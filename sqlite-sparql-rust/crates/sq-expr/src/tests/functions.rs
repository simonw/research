//! Table tests per builtin. Expected values follow SPARQL 1.1 §17 / XPath F&O and the W3C
//! `sparql11/functions` and `sparql10/expr-builtin` result files.
use sq_core::TermId;

use super::harness::Harness;

/// Asserts `expr ↦ expected` for constant expressions.
fn table(h: &mut Harness, cases: &[(&str, &str)]) {
    for (expr, expected) in cases {
        assert_eq!(h.show(expr, &[]), *expected, "{expr}");
    }
}

/// Asserts `f(?x) ↦ expected` with ?x bound to a stored term (exercises dictionary/inline ids).
fn table_x(h: &mut Harness, f: &str, cases: &[(&str, &str)]) {
    for (input, expected) in cases {
        let x = h.term(input);
        assert_eq!(h.show(f, &[("x", x)]), *expected, "{f} with ?x = {input}");
    }
}

#[test]
fn str_lang_datatype() {
    let mut h = Harness::new();
    table_x(
        &mut h,
        "STR(?x)",
        &[
            ("<http://example.org/a>", r#""http://example.org/a""#),
            (r#""abc""#, r#""abc""#),
            (r#""a longer string""#, r#""a longer string""#),
            (r#""chat"@fr"#, r#""chat""#),
            (r#""01"^^xsd:integer"#, r#""01""#),
            (r#""5.0"^^xsd:decimal"#, r#""5.0""#),
            (r#""1.5e0"^^xsd:double"#, r#""1.5e0""#),
            (r#""2011-01-10T14:45:13.815-05:00"^^xsd:dateTime"#, r#""2011-01-10T14:45:13.815-05:00""#),
            (r#""x"^^<http://example.org/dt>"#, r#""x""#),
            ("_:b", "ERROR"),
        ],
    );
    table_x(
        &mut h,
        "LANG(?x)",
        &[
            (r#""chat"@FR"#, r#""fr""#),
            (r#""abc""#, r#""""#),
            (r#""1"^^xsd:integer"#, r#""""#),
            ("<http://example.org/a>", "ERROR"),
            ("_:b", "ERROR"),
        ],
    );
    table_x(
        &mut h,
        "DATATYPE(?x)",
        &[
            (r#""abc""#, "<http://www.w3.org/2001/XMLSchema#string>"),
            (r#""a long simple literal""#, "<http://www.w3.org/2001/XMLSchema#string>"),
            (r#""chat"@fr"#, "<http://www.w3.org/1999/02/22-rdf-syntax-ns#langString>"),
            (r#""1"^^xsd:integer"#, "<http://www.w3.org/2001/XMLSchema#integer>"),
            (r#""01"^^xsd:integer"#, "<http://www.w3.org/2001/XMLSchema#integer>"),
            (r#""1"^^xsd:int"#, "<http://www.w3.org/2001/XMLSchema#int>"),
            (r#""1.5"^^xsd:decimal"#, "<http://www.w3.org/2001/XMLSchema#decimal>"),
            (r#""1e0"^^xsd:double"#, "<http://www.w3.org/2001/XMLSchema#double>"),
            ("true", "<http://www.w3.org/2001/XMLSchema#boolean>"),
            (r#""2010-06-21"^^xsd:date"#, "<http://www.w3.org/2001/XMLSchema#date>"),
            (r#""x"^^<http://example.org/dt>"#, "<http://example.org/dt>"),
            (r#""abc"^^xsd:integer"#, "<http://www.w3.org/2001/XMLSchema#integer>"),
            ("<http://example.org/a>", "ERROR"),
        ],
    );
    // Datatype ids of inline values are constants (no lookup) and equal to the stored IRI.
    let int_dt = h.term("xsd:integer");
    assert_eq!(h.eval("DATATYPE(1)", &[]), Some(int_dt));
    table(
        &mut h,
        &[
            ("DATATYPE(1 + 1)", "<http://www.w3.org/2001/XMLSchema#integer>"),
            ("DATATYPE(1 / 2)", "<http://www.w3.org/2001/XMLSchema#decimal>"),
            ("DATATYPE(1.0e0 + 1)", "<http://www.w3.org/2001/XMLSchema#double>"),
            ("DATATYPE(STRDT(\"5\", xsd:short))", "<http://www.w3.org/2001/XMLSchema#short>"),
            ("DATATYPE(xsd:byte(\"5\"))", "<http://www.w3.org/2001/XMLSchema#byte>"),
        ],
    );
}

#[test]
fn iri_and_bnode() {
    let mut h = Harness::new();
    table(
        &mut h,
        &[
            ("IRI(\"http://example.org/x\")", "<http://example.org/x>"),
            ("URI(\"http://example.org/x\")", "<http://example.org/x>"),
            ("IRI(<http://example.org/y>)", "<http://example.org/y>"),
            ("IRI(\"relative\")", "ERROR"),
            ("IRI(1)", "ERROR"),
            ("isBlank(BNODE())", r#""true"^^xsd:boolean"#),
            ("isBlank(BNODE(\"x\"))", r#""true"^^xsd:boolean"#),
            ("BNODE(1)", "ERROR"),
        ],
    );
    h.env.base = Some("http://example.org/base/".into());
    table(&mut h, &[("IRI(\"rel\")", "<http://example.org/base/rel>"), ("IRI(\"../up\")", "<http://example.org/up>")]);
    // IRI() of a stored IRI string returns the stored (canonical) id.
    let stored = h.term("<http://example.org/stored>");
    assert_eq!(h.eval("IRI(\"http://example.org/stored\")", &[]), Some(stored));
    // BNODE() is fresh per call.
    assert_ne!(h.eval("BNODE()", &[]), h.eval("BNODE()", &[]));
}

#[test]
fn bnode_with_string_is_stable_within_a_row() {
    // W3C functions/bnode01: BNODE(?s1) = BNODE(?s2) iff ?s1 = ?s2, within one solution;
    // a fresh blank node for every other solution.
    let mut h = Harness::new();
    let (foo, baz) = (h.term("\"foo\""), h.term("\"BAZ\""));
    let b1 = h.compile("BNODE(?s1)");
    let b2 = h.compile("BNODE(?s2)");
    let mut results = vec![];
    for (s1, s2) in [(baz, baz), (foo, foo), (foo, baz), (baz, foo)] {
        let mut row = h.row(&[("s1", s1), ("s2", s2), ("b1", TermId::UNBOUND)]);
        let slot_b1 = h.low.slot("b1") as usize;
        let x = h.env.with(|cx| b1.eval(&row, cx, true)).unwrap().unwrap();
        row[slot_b1] = x; // Extend: the second expression sees the extended row
        let y = h.env.with(|cx| b2.eval(&row, cx, true)).unwrap().unwrap();
        results.push((x, y));
    }
    assert_eq!(results[0].0, results[0].1);
    assert_eq!(results[1].0, results[1].1);
    assert_ne!(results[2].0, results[2].1);
    assert_ne!(results[3].0, results[3].1);
    let all: std::collections::HashSet<_> = results.iter().flat_map(|(a, b)| [*a, *b]).collect();
    assert_eq!(all.len(), 6, "fresh blank nodes for every solution");
}

/// BNODE(str) across a projected sub-select: the inner blank nodes are ordinary values of the
/// outer solutions (they tell them apart), while the outer expressions chained on one outer
/// solution share their blank nodes.
#[test]
fn bnode_with_string_lineage_across_a_sub_select() {
    let mut h = Harness::new();
    let inner = h.compile("BNODE(\"k\")");
    let outer = h.compile("BNODE(\"a\")");
    let outer2 = h.compile("BNODE(\"a\")");
    let (s_v, s_b1, s_b2) = (h.low.slot("v") as usize, h.low.slot("b1") as usize, h.low.slot("b2") as usize);
    let mut generated = vec![];
    for v in ["1", "2", "3"] {
        let v = h.term(v);
        let mut row = h.row(&[("v", v)]);
        row.resize(s_b2.max(s_b1) + 1, TermId::UNBOUND);
        let k = h.env.with(|cx| inner.eval(&row, cx, true)).unwrap().unwrap();
        row[s_b1] = k;
        // Projection drops ?v; the outer query then binds ?b2 twice on the same solution.
        row[s_v] = TermId::UNBOUND;
        let a = h.env.with(|cx| outer.eval(&row, cx, true)).unwrap().unwrap();
        row[s_b2] = a;
        let again = h.env.with(|cx| outer2.eval(&row, cx, true)).unwrap().unwrap();
        assert_eq!(a, again, "same outer solution, same label");
        generated.extend([k, a]);
    }
    let distinct: std::collections::HashSet<_> = generated.iter().collect();
    assert_eq!(distinct.len(), generated.len(), "distinct blank nodes per solution");
}

#[test]
fn term_tests() {
    let mut h = Harness::new();
    for (input, iri, blank, literal, numeric) in [
        ("<http://example.org/a>", true, false, false, false),
        ("_:b", false, true, false, false),
        ("\"abc\"", false, false, true, false),
        ("\"chat\"@fr", false, false, true, false),
        ("1", false, false, true, true),
        ("\"1\"^^xsd:int", false, false, true, true),
        ("\"300\"^^xsd:byte", false, false, true, false),
        ("1.5", false, false, true, true),
        ("\"1e0\"^^xsd:double", false, false, true, true),
        ("\"NaN\"^^xsd:float", false, false, true, true),
        ("true", false, false, true, false),
        ("\"abc\"^^xsd:integer", false, false, true, false),
        ("\"2010-06-21\"^^xsd:date", false, false, true, false),
    ] {
        let x = h.term(input);
        assert_eq!(h.ebv("isIRI(?x)", &[("x", x)]), Some(iri), "isIRI {input}");
        assert_eq!(h.ebv("isURI(?x)", &[("x", x)]), Some(iri), "isURI {input}");
        assert_eq!(h.ebv("isBlank(?x)", &[("x", x)]), Some(blank), "isBlank {input}");
        assert_eq!(h.ebv("isLiteral(?x)", &[("x", x)]), Some(literal), "isLiteral {input}");
        assert_eq!(h.ebv("isNumeric(?x)", &[("x", x)]), Some(numeric), "isNumeric {input}");
        assert_eq!(h.ebv("isIRI(STR(?x)) || true", &[("x", x)]), Some(true));
    }
    // Errors: unbound arguments.
    assert_eq!(h.ebv("isIRI(?unbound)", &[]), None);
    assert_eq!(h.ebv("isNumeric(?unbound)", &[]), None);
    // TEMP ids report their would-be kind.
    assert_eq!(h.ebv("isIRI(<http://example.org/not-stored>)", &[]), Some(true));
    assert_eq!(h.ebv("isLiteral(\"a string not in the store\")", &[]), Some(true));
}

#[test]
fn same_term() {
    let mut h = Harness::new();
    let (a, a2, one, one_dec) = (h.term("\"a\""), h.term("\"a\"^^xsd:string"), h.term("1"), h.term("1.0"));
    assert_eq!(a, a2);
    assert_eq!(h.ebv("sameTerm(?x, ?y)", &[("x", a), ("y", a2)]), Some(true));
    assert_eq!(h.ebv("sameTerm(?x, ?y)", &[("x", one), ("y", one_dec)]), Some(false));
    assert_eq!(h.ebv("?x = ?y", &[("x", one), ("y", one_dec)]), Some(true));
    assert_eq!(h.ebv("sameTerm(1 + 1, 2)", &[]), Some(true));
    assert_eq!(h.ebv("sameTerm(1 + 1, \"02\"^^xsd:integer)", &[]), Some(false));
    assert_eq!(h.ebv("sameTerm(CONCAT(\"a long \", \"string\"), \"a long string\")", &[]), Some(true));
    assert_eq!(h.ebv("sameTerm(?x, ?unbound)", &[("x", a)]), None);
}

#[test]
fn numeric_operators() {
    let mut h = Harness::new();
    table(
        &mut h,
        &[
            ("1 + 2", r#""3"^^xsd:integer"#),
            ("1 - 2", r#""-1"^^xsd:integer"#),
            ("2 * 3", r#""6"^^xsd:integer"#),
            ("1 / 2", r#""0.5"^^xsd:decimal"#),
            ("6 / 3", r#""2"^^xsd:decimal"#),
            ("1 / 3", r#""0.333333333333333333"^^xsd:decimal"#),
            ("1 + 1.5", r#""2.5"^^xsd:decimal"#),
            ("1.5 + 1.5", r#""3"^^xsd:decimal"#),
            ("1 + 1.5e0", r#""2.5"^^xsd:double"#),
            ("xsd:float(1.5) + 1", r#""2.5"^^xsd:float"#),
            ("xsd:float(1.5) + 1.0e0", r#""2.5"^^xsd:double"#),
            ("-(3)", r#""-3"^^xsd:integer"#),
            ("+(3)", r#""3"^^xsd:integer"#),
            ("-(1.5)", r#""-1.5"^^xsd:decimal"#),
            ("-(\"x\")", "ERROR"),
            ("+(\"x\")", "ERROR"),
            ("1 / 0", "ERROR"),
            ("1.5 / 0", "ERROR"),
            ("1.0e0 / 0", r#""INF"^^xsd:double"#),
            ("-1.0e0 / 0", r#""-INF"^^xsd:double"#),
            ("0.0e0 / 0", r#""NaN"^^xsd:double"#),
            ("xsd:float(1) / 0", r#""INF"^^xsd:float"#),
            ("9223372036854775807 + 1", "ERROR"),
            ("-9223372036854775807 - 2", "ERROR"),
            ("4611686018427387904 * 2", "ERROR"),
            ("1 + \"1\"", "ERROR"),
            ("1 + <http://example.org/a>", "ERROR"),
            ("1 + true", "ERROR"),
            ("1 + ?unbound", "ERROR"),
            ("\"1\"^^xsd:int + \"2\"^^xsd:short", r#""3"^^xsd:integer"#),
            ("\"300\"^^xsd:byte + 1", "ERROR"),
        ],
    );
    // A computed integer gets the inline id of its canonical form (sameTerm ⇔ id equality).
    let two = h.term("2");
    assert_eq!(h.eval("1 + 1", &[]), Some(two));
    assert_eq!(two, TermId::from_int(2).unwrap());
    let two_dec = h.term("\"2\"^^xsd:decimal");
    assert_eq!(h.eval("1.5 + 0.5", &[]), Some(two_dec));
}

#[test]
fn numeric_functions() {
    let mut h = Harness::new();
    // W3C functions/abs01, ceil01, floor01, round01 data: -2, -1, -1.6, 1.1, 2.5.
    table_x(&mut h, "ABS(?x)", &[("-2", r#""2"^^xsd:integer"#), ("-1.6", r#""1.6"^^xsd:decimal"#), ("\"-1.5e0\"^^xsd:double", r#""1.5"^^xsd:double"#)]);
    table_x(
        &mut h,
        "CEIL(?x)",
        &[("-2", r#""-2"^^xsd:integer"#), ("-1.6", r#""-1"^^xsd:decimal"#), ("1.1", r#""2"^^xsd:decimal"#), ("2.5", r#""3"^^xsd:decimal"#)],
    );
    table_x(
        &mut h,
        "FLOOR(?x)",
        &[("-2", r#""-2"^^xsd:integer"#), ("-1.6", r#""-2"^^xsd:decimal"#), ("1.1", r#""1"^^xsd:decimal"#), ("2.5", r#""2"^^xsd:decimal"#)],
    );
    table_x(
        &mut h,
        "ROUND(?x)",
        &[
            ("-2", r#""-2"^^xsd:integer"#),
            ("-1.6", r#""-2"^^xsd:decimal"#),
            ("1.1", r#""1"^^xsd:decimal"#),
            ("2.5", r#""3"^^xsd:decimal"#),
            ("-2.5", r#""-2"^^xsd:decimal"#),
            ("\"2.5e0\"^^xsd:double", r#""3"^^xsd:double"#),
            ("\"-2.5e0\"^^xsd:double", r#""-2"^^xsd:double"#),
            ("\"NaN\"^^xsd:double", r#""NaN"^^xsd:double"#),
        ],
    );
    table(&mut h, &[("ABS(\"x\")", "ERROR"), ("ROUND(true)", "ERROR"), ("CEIL(?u)", "ERROR")]);
    // RAND: a double in [0, 1).
    for _ in 0..20 {
        let r = h.show("RAND()", &[]);
        assert!(r.ends_with("^^xsd:double"), "{r}");
        let v: f64 = r.trim_end_matches("^^xsd:double").trim_matches('"').parse().unwrap();
        assert!((0.0..1.0).contains(&v));
    }
}

#[test]
fn string_functions() {
    let mut h = Harness::new();
    table(
        &mut h,
        &[
            // CONCAT: tags kept only when all arguments share it (functions/concat01, 02).
            ("CONCAT(\"foo\", \"bar\")", r#""foobar""#),
            ("CONCAT(\"foo\"@en, \"bar\"@en)", r#""foobar"@en"#),
            ("CONCAT(\"foo\"@en, \"bar\")", r#""foobar""#),
            ("CONCAT(\"foo\"@en, \"bar\"@fr)", r#""foobar""#),
            ("CONCAT(\"foo\"^^xsd:string, \"bar\")", r#""foobar""#),
            ("CONCAT()", r#""""#),
            ("CONCAT(\"a\"@en)", r#""a"@en"#),
            ("CONCAT(\"a\", 1)", "ERROR"),
            ("CONCAT(\"a\", <http://example.org/>)", "ERROR"),
            // SUBSTR (fn:substring rounding, code points).
            ("SUBSTR(\"foobar\", 4)", r#""bar""#),
            ("SUBSTR(\"foobar\", 4, 1)", r#""b""#),
            ("SUBSTR(\"foobar\"@en, 4, 1)", r#""b"@en"#),
            ("SUBSTR(\"食べ物\", 2, 1)", r#""べ""#),
            ("SUBSTR(\"12345\", 1.5, 2.6)", r#""234""#),
            ("SUBSTR(\"12345\", 0, 3)", r#""12""#),
            ("SUBSTR(\"12345\", -3, 5)", r#""1""#),
            ("SUBSTR(\"12345\", 8)", r#""""#),
            ("SUBSTR(\"12345\", \"1\")", "ERROR"),
            ("SUBSTR(1234, 1)", "ERROR"),
            // STRLEN.
            ("STRLEN(\"食べ物\")", r#""3"^^xsd:integer"#),
            ("STRLEN(\"chat\"@fr)", r#""4"^^xsd:integer"#),
            ("STRLEN(\"\")", r#""0"^^xsd:integer"#),
            ("STRLEN(1)", "ERROR"),
            // UCASE / LCASE keep the tag.
            ("UCASE(\"foo\")", r#""FOO""#),
            ("UCASE(\"foo\"@en)", r#""FOO"@en"#),
            ("LCASE(\"BAR\"^^xsd:string)", r#""bar""#),
            ("UCASE(\"straße\")", r#""STRASSE""#),
            ("LCASE(<http://example.org/>)", "ERROR"),
            // ENCODE_FOR_URI.
            ("ENCODE_FOR_URI(\"Los Angeles\")", r#""Los%20Angeles""#),
            ("ENCODE_FOR_URI(\"Los Angeles\"@en)", r#""Los%20Angeles""#),
            ("ENCODE_FOR_URI(\"100%\")", r#""100%25""#),
            // CONTAINS / STRSTARTS / STRENDS.
            ("CONTAINS(\"foobar\", \"bar\")", r#""true"^^xsd:boolean"#),
            ("CONTAINS(\"foobar\"@en, \"bar\")", r#""true"^^xsd:boolean"#),
            ("CONTAINS(\"foobar\"@en, \"bar\"@en)", r#""true"^^xsd:boolean"#),
            ("CONTAINS(\"foobar\"@en, \"bar\"@fr)", "ERROR"),
            ("CONTAINS(\"foobar\", \"bar\"@en)", "ERROR"),
            ("CONTAINS(\"foobar\", \"\")", r#""true"^^xsd:boolean"#),
            ("STRSTARTS(\"foobar\", \"foo\")", r#""true"^^xsd:boolean"#),
            ("STRSTARTS(\"foobar\", \"bar\")", r#""false"^^xsd:boolean"#),
            ("STRENDS(\"foobar\", \"bar\")", r#""true"^^xsd:boolean"#),
            ("STRENDS(\"foobar\"^^xsd:string, \"bar\"^^xsd:string)", r#""true"^^xsd:boolean"#),
            ("STRSTARTS(1, \"1\")", "ERROR"),
            // STRBEFORE / STRAFTER keep arg1's tag on any match, incl. the empty pattern.
            ("STRBEFORE(\"abc\", \"b\")", r#""a""#),
            ("STRBEFORE(\"abc\"@en, \"bc\")", r#""a"@en"#),
            ("STRBEFORE(\"abc\"@en, \"b\"@cy)", "ERROR"),
            ("STRBEFORE(\"abc\"^^xsd:string, \"\")", r#""""#),
            ("STRBEFORE(\"abc\"@en, \"\")", r#"""@en"#),
            ("STRBEFORE(\"abc\"@en, \"a\")", r#"""@en"#),
            ("STRBEFORE(\"abc\"@en, \"z\")", r#""""#),
            ("STRBEFORE(\"abc\", \"xyz\")", r#""""#),
            ("STRAFTER(\"abc\", \"b\")", r#""c""#),
            ("STRAFTER(\"abc\"@en, \"ab\")", r#""c"@en"#),
            ("STRAFTER(\"abc\"@en, \"c\")", r#"""@en"#),
            ("STRAFTER(\"abc\"@en, \"\")", r#""abc"@en"#),
            ("STRAFTER(\"abc\", \"xyz\")", r#""""#),
            ("STRAFTER(\"abc\"@en, \"b\"@cy)", "ERROR"),
            ("STRAFTER(\"食べ物\", \"べ\")", r#""物""#),
        ],
    );
    // Stored (dictionary and SHORTSTR) arguments behave like constants.
    table_x(&mut h, "STRLEN(?x)", &[("\"a much longer string\"", r#""20"^^xsd:integer"#), ("\"short\"", r#""5"^^xsd:integer"#)]);
    table_x(&mut h, "UCASE(?x)", &[("\"a much longer string\"@en-GB", r#""A MUCH LONGER STRING"@en-gb"#)]);
}

#[test]
fn regex_and_replace() {
    let mut h = Harness::new();
    table(
        &mut h,
        &[
            ("REGEX(\"abcd\", \"^ab\")", r#""true"^^xsd:boolean"#),
            ("REGEX(\"xabcd\", \"^ab\")", r#""false"^^xsd:boolean"#),
            ("REGEX(\"ABCD\", \"^ab\", \"i\")", r#""true"^^xsd:boolean"#),
            ("REGEX(\"chat\"@fr, \"^ch\")", r#""true"^^xsd:boolean"#),
            ("REGEX(\"abc\", \"b\"@en)", "ERROR"),
            ("REGEX(1, \"1\")", "ERROR"),
            ("REGEX(\"abc\", \"(\")", "ERROR"),
            ("REGEX(\"abc\", \"b\", \"z\")", "ERROR"),
            ("REGEX(\"a.c\", \".\", \"q\")", r#""true"^^xsd:boolean"#),
            ("REGEX(\"abc\", \".\", \"q\")", r#""false"^^xsd:boolean"#),
            // functions/replace01..03, replace-case-insensitive.
            ("REPLACE(\"abcd\", \"b\", \"Z\")", r#""aZcd""#),
            ("REPLACE(\"abab\", \"B\", \"Z\", \"i\")", r#""aZaZ""#),
            ("REPLACE(\"abcd\"@en, \"b\", \"Z\")", r#""aZcd"@en"#),
            ("REPLACE(\"abcd\"^^xsd:string, \"b\", \"Z\")", r#""aZcd""#),
            ("REPLACE(\"abracadabra\", \"bra\", \"*\")", r#""a*cada*""#),
            ("REPLACE(\"abracadabra\", \"a.*a\", \"*\")", r#""*""#),
            ("REPLACE(\"abracadabra\", \"a.*?a\", \"*\")", r#""*c*bra""#),
            ("REPLACE(\"abracadabra\", \"a\", \"\")", r#""brcdbr""#),
            ("REPLACE(\"abracadabra\", \"a(.)\", \"a$1$1\")", r#""abbraccaddabbra""#),
            ("REPLACE(\"darted\", \"^(.*?)d(.*)$\", \"$1c$2\")", r#""carted""#),
            ("REPLACE(\"abc\", \"b\", \"\\\\$\")", r#""a$c""#),
            ("REPLACE(\"abracadabra\", \".*?\", \"$1\")", "ERROR"),
            ("REPLACE(\"abc\", \"b\", \"$\")", "ERROR"),
            ("REPLACE(\"abc\", \"b\", \"x\"@en)", "ERROR"),
            // `q`: pattern and replacement are both literal.
            ("REPLACE(\"a.b.c\", \".\", \"$0\", \"q\")", r#""a$0b$0c""#),
            ("REPLACE(\"a.b\", \".\", \"[$0]\")", r#""[a][.][b]""#),
            // A top-level alternation: `^Mr|Dr` is `(^Mr)|(Dr)`.
            ("REGEX(\"Dr Who\", \"^Mr|Dr\")", r#""true"^^xsd:boolean"#),
            ("REGEX(\"Mr Who\", \"^Mr|Dr\")", r#""true"^^xsd:boolean"#),
            ("REGEX(\"A Mr\", \"^Mr|Dr\")", r#""false"^^xsd:boolean"#),
            ("REGEX(\"a&b\", \"^a[&]b$\")", r#""true"^^xsd:boolean"#),
        ],
    );
    // Dynamic patterns (from variables) go through the per-execution cache.
    let (p, t) = (h.term("\"^a+$\""), h.term("\"aaaa\""));
    for _ in 0..3 {
        assert_eq!(h.ebv("REGEX(?t, ?p)", &[("t", t), ("p", p)]), Some(true));
    }
    let i = h.term("\"i\"");
    let up = h.term("\"AAA\"");
    assert_eq!(h.ebv("REGEX(?t, ?p, ?f)", &[("t", up), ("p", p), ("f", i)]), Some(true));
    assert_eq!(h.ebv("REGEX(?t, ?p)", &[("t", up), ("p", p)]), Some(false));
    // A constant pattern that is a dictionary string compiles lazily on first use.
    let long = h.term("\"^http://example\"");
    let iri_str = h.term("\"http://example.org/x\"");
    let _ = long;
    assert_eq!(h.ebv("REGEX(?t, \"^http://example\")", &[("t", iri_str)]), Some(true));
}

#[test]
fn date_functions() {
    let mut h = Harness::new();
    // W3C functions/year-01 .. tz-01 data.
    let d = [
        "\"2010-06-21T11:28:01Z\"^^xsd:dateTime",
        "\"2010-12-21T15:38:02-08:00\"^^xsd:dateTime",
        "\"2008-06-20T23:59:00Z\"^^xsd:dateTime",
        "\"2011-02-01T01:02:03\"^^xsd:dateTime",
    ];
    let expect = [
        ("YEAR(?x)", ["\"2010\"^^xsd:integer", "\"2010\"^^xsd:integer", "\"2008\"^^xsd:integer", "\"2011\"^^xsd:integer"]),
        ("MONTH(?x)", ["\"6\"^^xsd:integer", "\"12\"^^xsd:integer", "\"6\"^^xsd:integer", "\"2\"^^xsd:integer"]),
        ("DAY(?x)", ["\"21\"^^xsd:integer", "\"21\"^^xsd:integer", "\"20\"^^xsd:integer", "\"1\"^^xsd:integer"]),
        ("HOURS(?x)", ["\"11\"^^xsd:integer", "\"15\"^^xsd:integer", "\"23\"^^xsd:integer", "\"1\"^^xsd:integer"]),
        ("MINUTES(?x)", ["\"28\"^^xsd:integer", "\"38\"^^xsd:integer", "\"59\"^^xsd:integer", "\"2\"^^xsd:integer"]),
        ("SECONDS(?x)", ["\"1\"^^xsd:decimal", "\"2\"^^xsd:decimal", "\"0\"^^xsd:decimal", "\"3\"^^xsd:decimal"]),
        ("TIMEZONE(?x)", ["\"PT0S\"^^xsd:dayTimeDuration", "\"-PT8H\"^^xsd:dayTimeDuration", "\"PT0S\"^^xsd:dayTimeDuration", "ERROR"]),
        ("TZ(?x)", ["\"Z\"", "\"-08:00\"", "\"Z\"", "\"\""]),
    ];
    for (f, results) in expect {
        for (input, out) in d.iter().zip(results) {
            let x = h.term(input);
            assert_eq!(h.show(f, &[("x", x)]), out, "{f} on {input}");
        }
    }
    table(
        &mut h,
        &[
            ("SECONDS(\"2011-01-10T14:45:13.815-05:00\"^^xsd:dateTime)", r#""13.815"^^xsd:decimal"#),
            ("YEAR(\"2010-06-21\"^^xsd:date)", r#""2010"^^xsd:integer"#),
            ("HOURS(\"12:30:00\"^^xsd:time)", r#""12"^^xsd:integer"#),
            ("YEAR(\"2010\")", "ERROR"),
            ("HOURS(\"2010-06-21\"^^xsd:date)", "ERROR"),
            ("TZ(\"2010-06-21+05:30\"^^xsd:date)", r#""+05:30""#),
            ("TIMEZONE(\"2010-06-21+05:30\"^^xsd:date)", r#""PT5H30M"^^xsd:dayTimeDuration"#),
            ("TZ(1)", "ERROR"),
            ("NOW()", r#""2026-10-15T12:00:00Z"^^xsd:dateTime"#),
            ("DATATYPE(NOW())", "<http://www.w3.org/2001/XMLSchema#dateTime>"),
        ],
    );
}

#[test]
fn hash_functions() {
    let mut h = Harness::new();
    // functions/md5-01, sha1-01, sha256-01 ("abc" vectors) and unicode inputs.
    table(
        &mut h,
        &[
            ("MD5(\"abc\")", r#""900150983cd24fb0d6963f7d28e17f72""#),
            ("MD5(\"abc\"^^xsd:string)", r#""900150983cd24fb0d6963f7d28e17f72""#),
            ("SHA1(\"abc\")", r#""a9993e364706816aba3e25717850c26c9cd0d89d""#),
            ("SHA256(\"abc\")", r#""ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad""#),
            ("SHA1(\"食べ物\")", r#""caf1f189d7064f8987e972525091c29f7e462c74""#),
            ("MD5(\"abc\"@en)", "ERROR"),
            ("SHA512(1)", "ERROR"),
        ],
    );
    let r = h.show("SHA384(\"abc\")", &[]);
    assert_eq!(r.len(), 96 + 2);
    let r = h.show("SHA512(\"abc\")", &[]);
    assert!(r.starts_with("\"ddaf35a193617aba"), "{r}");
}

#[test]
fn uuid_functions() {
    let mut h = Harness::new();
    let u = h.show("UUID()", &[]);
    assert!(u.starts_with("<urn:uuid:") && u.len() == "<urn:uuid:>".len() + 36, "{u}");
    assert_eq!(&u[10 + 14..10 + 15], "4");
    let s = h.show("STRUUID()", &[]);
    assert_eq!(s.len(), 36 + 2);
    assert_ne!(h.show("STRUUID()", &[]), s);
    assert_eq!(h.ebv("isIRI(UUID())", &[]), Some(true));
    assert_eq!(h.ebv("isLiteral(STRUUID())", &[]), Some(true));
}

#[test]
fn strlang_strdt() {
    let mut h = Harness::new();
    table(
        &mut h,
        &[
            ("STRLANG(\"chat\", \"FR\")", r#""chat"@fr"#),
            ("STRLANG(\"chat\", \"en-GB\")", r#""chat"@en-gb"#),
            ("STRLANG(\"chat\"@en, \"fr\")", "ERROR"),
            ("STRLANG(\"chat\", \"\")", "ERROR"),
            ("STRLANG(\"chat\", \"not a tag\")", "ERROR"),
            ("STRLANG(1, \"fr\")", "ERROR"),
            ("STRDT(\"123\", xsd:integer)", r#""123"^^xsd:integer"#),
            ("STRDT(\"0123\", xsd:integer)", r#""0123"^^xsd:integer"#),
            ("STRDT(\"iiii\", <http://example/romanNumeral>)", r#""iiii"^^<http://example/romanNumeral>"#),
            ("STRDT(\"x\", xsd:string)", r#""x""#),
            ("STRDT(\"x\"@en, xsd:string)", "ERROR"),
            ("STRDT(\"x\", \"http://example/dt\")", "ERROR"),
            ("STRDT(\"x\", rdf:langString)", "ERROR"),
        ],
    );
    // STRDT of an inline-encodable form gives the inline id.
    assert_eq!(h.eval("STRDT(\"123\", xsd:integer)", &[]), TermId::from_int(123));
    assert_eq!(h.ebv("STRDT(\"123\", xsd:integer) = 123", &[]), Some(true));
}

#[test]
fn coalesce_if_in() {
    let mut h = Harness::new();
    let x = h.term("\"x\"");
    table(
        &mut h,
        &[
            ("COALESCE(?u, 1/0, 3)", r#""3"^^xsd:integer"#),
            ("COALESCE(?u, 1/0)", "ERROR"),
            ("COALESCE()", "ERROR"),
            ("IF(true, 1, 1/0)", r#""1"^^xsd:integer"#),
            ("IF(false, 1/0, 2)", r#""2"^^xsd:integer"#),
            ("IF(1/0, 1, 2)", "ERROR"),
            ("IF(\"\", 1, 2)", r#""2"^^xsd:integer"#),
            ("IF(?u, 1, 2)", "ERROR"),
            ("2 IN (1, 2, 3)", r#""true"^^xsd:boolean"#),
            ("2 IN (1, 2.0)", r#""true"^^xsd:boolean"#),
            ("2 IN ()", r#""false"^^xsd:boolean"#),
            ("2 IN (1, 1/0, 2)", r#""true"^^xsd:boolean"#),
            ("2 IN (1, 1/0)", "ERROR"),
            ("2 NOT IN (1, 3)", r#""true"^^xsd:boolean"#),
            ("2 NOT IN ()", r#""true"^^xsd:boolean"#),
            ("2 NOT IN (1/0, 2)", r#""false"^^xsd:boolean"#),
            ("2 NOT IN (1/0, 3)", "ERROR"),
            ("<http://example.org/a> IN (<http://example.org/a>, <http://example.org/b>)", r#""true"^^xsd:boolean"#),
        ],
    );
    // Pass-through: COALESCE / IF / a bare variable return the argument's id.
    assert_eq!(h.eval("COALESCE(?u, ?x)", &[("x", x)]), Some(x));
    assert_eq!(h.eval("IF(true, ?x, 1)", &[("x", x)]), Some(x));
    assert_eq!(h.eval("?x", &[("x", x)]), Some(x));
    // IN over IRI constants (id-set fast path), with unbound and non-IRI values.
    let (a, c, lit) = (h.term("<http://example.org/a>"), h.term("<http://example.org/c>"), h.term("\"a\""));
    let e = "?v IN (<http://example.org/a>, <http://example.org/b>)";
    assert_eq!(h.ebv(e, &[("v", a)]), Some(true));
    assert_eq!(h.ebv(e, &[("v", c)]), Some(false));
    assert_eq!(h.ebv(e, &[("v", lit)]), Some(false));
    assert_eq!(h.ebv(e, &[]), None);
    assert_eq!(h.ebv(&format!("!({e})"), &[("v", lit)]), Some(true));
}

#[test]
fn logical_operators_three_valued() {
    let mut h = Harness::new();
    // E = error (1/0), T = true, F = false: §6.2.
    let cases = [
        ("true || true", Some(true)),
        ("true || false", Some(true)),
        ("false || false", Some(false)),
        ("(1/0) || true", Some(true)),
        ("true || (1/0)", Some(true)),
        ("(1/0) || false", None),
        ("false || (1/0)", None),
        ("(1/0) || (1/0)", None),
        ("true && true", Some(true)),
        ("true && false", Some(false)),
        ("(1/0) && false", Some(false)),
        ("false && (1/0)", Some(false)),
        ("(1/0) && true", None),
        ("true && (1/0)", None),
        ("!(1/0)", None),
        ("!true", Some(false)),
        ("!\"\"", Some(true)),
        ("BOUND(?u)", Some(false)),
        ("!BOUND(?u)", Some(true)),
    ];
    for (e, expect) in cases {
        assert_eq!(h.ebv(e, &[]), expect, "{e}");
    }
    let x = h.term("1");
    assert_eq!(h.ebv("BOUND(?x)", &[("x", x)]), Some(true));
    // ERROR group-key sentinels behave as unbound.
    assert_eq!(h.ebv("BOUND(?x)", &[("x", TermId::ERROR)]), Some(false));
    assert_eq!(h.ebv("?x = 1", &[("x", TermId::ERROR)]), None);
}

#[test]
fn exists_never_errors() {
    let mut h = Harness::new();
    let x = h.term("<http://example.org/a>");
    let mut seen = vec![];
    let r = h.ebv_with_exists("EXISTS { ?x ?p ?o }", &[("x", x)], |id, row| {
        seen.push((id.0, row.to_vec()));
        true
    });
    assert_eq!(r, Some(true));
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].1[0], x, "the row is passed for substitution");
    assert_eq!(h.ebv_with_exists("NOT EXISTS { ?x ?p ?o }", &[("x", x)], |_, _| true), Some(false));
    assert_eq!(h.ebv_with_exists("EXISTS { ?x ?p ?o } || (1/0)", &[], |_, _| false), None);
    assert_eq!(h.ebv_with_exists("IF(EXISTS { ?x ?p ?o }, 1, 2) = 2", &[], |_, _| false), Some(true));
}

#[test]
fn ebv_rules() {
    let mut h = Harness::new();
    // §6.4 and sparql10/expr-builtin, boolean-logic EBV tests.
    for (input, expect) in [
        ("true", Some(true)),
        ("false", Some(false)),
        ("\"\"", Some(false)),
        ("\"a\"", Some(true)),
        ("\"a long non-empty string\"", Some(true)),
        ("\"\"^^xsd:string", Some(false)),
        ("0", Some(false)),
        ("1", Some(true)),
        ("0.0", Some(false)),
        ("-0.5", Some(true)),
        ("\"0.0e0\"^^xsd:double", Some(false)),
        ("\"NaN\"^^xsd:double", Some(false)),
        ("\"NaN\"^^xsd:float", Some(false)),
        ("\"INF\"^^xsd:double", Some(true)),
        ("\"abc\"^^xsd:integer", Some(false)),
        ("\"maybe\"^^xsd:boolean", Some(false)),
        ("\"300\"^^xsd:byte", Some(false)),
        ("\"chat\"@fr", None),
        ("<http://example.org/a>", None),
        ("_:b", None),
        ("\"x\"^^<http://example.org/dt>", None),
        ("\"2010-06-21\"^^xsd:date", None),
        ("\"2010-06-21T00:00:00Z\"^^xsd:dateTime", None),
    ] {
        let x = h.term(input);
        assert_eq!(h.ebv("?x", &[("x", x)]), expect, "EBV({input})");
    }
    assert_eq!(h.ebv("?unbound", &[]), None);
}
