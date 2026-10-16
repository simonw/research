//! Adversarial review tests (WP4 review). Each `review_*` test pins one SPARQL 1.1 / spec §6
//! behaviour; a failing one documents a defect in the implementation.
use sq_core::{Literal, Term, TermId};

use super::harness::Harness;

/// Terms bound to ?x in the differential tests: stored (canonical) ids plus TEMP ids for
/// terms that are also stored (a slot the planner left non-canonical) and for absent terms.
fn binding_terms(h: &mut Harness) -> Vec<(String, TermId)> {
    let stored = [
        "<http://example.org/a>",
        "<http://example.org/absent-elsewhere>",
        "\"abc\"",
        "\"a long simple literal\"",
        "\"\"",
        "\"en\"",
        "\"abc\"@en",
        "\"abc\"@en-gb",
        "\"a long language literal\"@fr",
        "5",
        "\"05\"^^xsd:integer",
        "5.0",
        "\"5\"^^xsd:int",
        "\"5e0\"^^xsd:double",
        "\"NaN\"^^xsd:double",
        "true",
        "\"x\"^^xsd:integer",
        "\"x\"^^<http://example/dt>",
        "\"2002-04-01T00:00:00Z\"^^xsd:dateTime",
        "_:b",
    ];
    let mut out = vec![];
    for t in stored {
        out.push((t.to_string(), h.term(t)));
    }
    // TEMP ids of stored terms (non-canonical slots) and of absent terms.
    let temps = [
        Term::Iri("http://example.org/a".into()),
        Term::Literal(Literal::simple("a long simple literal")),
        Term::Literal(Literal::lang("abc", "en")),
        Term::Literal(Literal::typed("5e0", "http://www.w3.org/2001/XMLSchema#double")),
        Term::Literal(Literal::typed("NaN", "http://www.w3.org/2001/XMLSchema#double")),
        Term::Iri("http://example.org/temp-only".into()),
        Term::Literal(Literal::simple("a temp-only long literal")),
        Term::Literal(Literal::typed("y", "http://example/dt")),
    ];
    for t in temps {
        let label = format!("TEMP {t:?}");
        out.push((label, h.env.resolver.temp(t)));
    }
    out
}

/// Every fast path selected by `compile` must agree with the general evaluator. The general
/// form is obtained by wrapping the variable in COALESCE (which defeats the slot patterns).
#[test]
fn review_fast_paths_agree_with_general_path() {
    let mut h = Harness::new();
    let xs = binding_terms(&mut h);
    let forms = [
        ("?x = <http://example.org/a>", "COALESCE(?x) = <http://example.org/a>"),
        ("?x = <http://example.org/temp-only>", "COALESCE(?x) = <http://example.org/temp-only>"),
        ("?x = \"abc\"", "COALESCE(?x) = \"abc\""),
        ("?x = \"a long simple literal\"", "COALESCE(?x) = \"a long simple literal\""),
        ("?x = \"a temp-only long literal\"", "COALESCE(?x) = \"a temp-only long literal\""),
        ("?x = 5", "COALESCE(?x) = 5"),
        ("5 = ?x", "5 = COALESCE(?x)"),
        ("?x < 6", "COALESCE(?x) < 6"),
        ("6 > ?x", "6 > COALESCE(?x)"),
        ("?x >= 5", "COALESCE(?x) >= 5"),
        ("?x IN (<http://example.org/a>, <http://example.org/temp-only>)", "COALESCE(?x) IN (<http://example.org/a>, <http://example.org/temp-only>)"),
        ("LANG(?x) = \"en\"", "LANG(COALESCE(?x)) = \"en\""),
        ("LANG(?x) = \"\"", "LANG(COALESCE(?x)) = \"\""),
        ("LANGMATCHES(LANG(?x), \"en\")", "LANGMATCHES(LANG(?x), COALESCE(\"en\"))"),
        ("LANGMATCHES(LANG(?x), \"*\")", "LANGMATCHES(LANG(?x), COALESCE(\"*\"))"),
        ("isIRI(?x)", "isIRI(COALESCE(?x))"),
        ("isBlank(?x)", "isBlank(COALESCE(?x))"),
        ("isLiteral(?x)", "isLiteral(COALESCE(?x))"),
        ("?x = ?x", "COALESCE(?x) = COALESCE(?x)"),
        ("sameTerm(?x, <http://example.org/a>)", "sameTerm(COALESCE(?x), <http://example.org/a>)"),
        ("REGEX(?x, \"^ab\")", "REGEX(?x, COALESCE(\"^ab\"))"),
    ];
    let mut mismatches = vec![];
    for (fast, slow) in forms {
        for (label, id) in &xs {
            let a = h.ebv(fast, &[("x", *id)]);
            let b = h.ebv(slow, &[("x", *id)]);
            if a != b {
                mismatches.push(format!("{fast} with ?x = {label}: fast {a:?}, general {b:?}"));
            }
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

/// XPath fn:matches: `^abc|xyz` is the alternation (^abc)|(xyz), so any string containing
/// "xyz" matches. The literal-prefix plan (`Plan::Prefix`) wrongly requires the "abc" prefix.
#[test]
fn review_regex_anchored_prefix_with_top_level_alternation() {
    let mut h = Harness::new();
    for (expr, expected) in [
        (r#"REGEX("fooxyz", "^abc|xyz")"#, Some(true)),
        (r#"REGEX("xyz", "^abc$|xyz")"#, Some(true)),
        (r#"REGEX("zzz", "^ab|z")"#, Some(true)),
        (r#"REGEX("abcd", "^abc|xyz")"#, Some(true)),
        (r#"REGEX("foo", "^abc|xyz")"#, Some(false)),
    ] {
        assert_eq!(h.ebv(expr, &[]), expected, "{expr}");
    }
    // Same through a variable (constant pattern compiled once) and through REPLACE.
    let x = h.term("\"fooxyz\"");
    assert_eq!(h.ebv(r#"REGEX(?x, "^abc|xyz")"#, &[("x", x)]), Some(true));
}

/// Differential probe (ignored): writes random string / date / term-function expressions and
/// this crate's results as TSV to `SQ_REVIEW_DIFF_OUT` for comparison with pyoxigraph.
#[test]
#[ignore = "writes a TSV; set SQ_REVIEW_DIFF_OUT"]
fn review_string_function_diff() {
    use sq_core::eval_api::TermResolver;
    use sq_core::testing::Rng;
    use std::fmt::Write as _;
    let Ok(out) = std::env::var("SQ_REVIEW_DIFF_OUT") else { return };
    fn pick<'a>(r: &mut Rng, xs: &[&'a str]) -> &'a str {
        xs[r.below(xs.len() as u64) as usize]
    }
    let strs = [
        "\"abc\"", "\"\"", "\"b\"", "\"abc\"@en", "\"b\"@en", "\"B\"@en-GB", "\"abc\"@fr", "\"abc\"^^xsd:string",
        "1", "<http://ex/a>", "\"a.c\"", "\"ABC\"", "\"日本語\"@ja", "\"x\"^^<http://ex/dt>", "\"bc\"", "\"c\"@en", "\"\"@en",
        "\"a b\"", "\"ß\"", "\"en\"", "\"EN-us\"", "\"*\"", "\"1.5\"", "2.5", "\"2011-01-10T14:45:13.815-05:00\"^^xsd:dateTime",
        "\"2011-01-10\"^^xsd:date", "\"12:30:00Z\"^^xsd:time", "\"2011-01-10T14:45:13Z\"^^xsd:dateTime",
    ];
    let mut r = Rng(0xBADC_0FFE);
    let mut h = Harness::new();
    let mut tsv = String::new();
    for _ in 0..6000 {
        let a = pick(&mut r, &strs).to_string();
        let b = pick(&mut r, &strs).to_string();
        let c = pick(&mut r, &strs).to_string();
        let regex_pats = [
            "\"^abc|xyz\"", "\"^a\"", "\"c$\"", "\"^abc$\"", "\"b|^x\"", "\"^(a|x)\"", "\"^a.c\"", "\"^ab?c\"",
            "\"^a|^b\"", "\"^$\"", "\"^\"", "\"x*\"", "\"[a-c]+\"", "\"^[^a]\"", "\"^ab{1,2}\"", "\"^b*a\"",
            "\"(?i)ABC\"", "\"^a(b|c)\"", "\"a|\"", "\"^.$\"", "\"^abc|\"", "\"^z|c$\"",
        ];
        let texts = ["\"abc\"", "\"xyz\"", "\"fooxyz\"", "\"ab\"", "\"\"", "\"ac\"", "\"zbc\"", "\"a\\nb\"", "\"bac\""];
        if r.below(3) == 0 {
            let t = pick(&mut r, &texts);
            let p = pick(&mut r, &regex_pats);
            let f = pick(&mut r, &["", ", \"\"", ", \"i\"", ", \"m\"", ", \"s\"", ", \"q\"", ", \"x\""]);
            let e = format!("REGEX({t}, {p}{f})");
            let result = match h.eval(&e, &[]) {
                None => "ERROR".to_string(),
                Some(id) => h.env.resolver.term(id).expect("decode").to_ntriples(),
            };
            let _ = writeln!(tsv, "{e}\t{result}");
            continue;
        }
        let e = match r.below(26) {
            0 => format!("STRBEFORE({a}, {b})"),
            1 => format!("STRAFTER({a}, {b})"),
            2 => format!("CONTAINS({a}, {b})"),
            3 => format!("STRSTARTS({a}, {b})"),
            4 => format!("STRENDS({a}, {b})"),
            5 => format!("CONCAT({a}, {b}, {c})"),
            6 => format!("SUBSTR({a}, {}, {})", r.below(5) as i64 - 1, r.below(4)),
            7 => format!("UCASE({a})"),
            8 => format!("LCASE({a})"),
            9 => format!("STRLEN({a})"),
            10 => format!("REPLACE({a}, {b}, {c})"),
            11 => format!("REGEX({a}, {b})"),
            12 => format!("ENCODE_FOR_URI({a})"),
            13 => format!("LANGMATCHES(LANG({a}), {b})"),
            14 => format!("STR({a})"),
            15 => format!("LANG({a})"),
            16 => format!("DATATYPE({a})"),
            17 => format!("STRLANG({a}, {b})"),
            18 => format!("isNumeric({a})"),
            19 => format!("({a} = {b})"),
            20 => format!("({a} < {b})"),
            21 => format!("({a} IN ({b}, {c}))"),
            22 => format!("{}({a})", pick(&mut r, &["YEAR", "MONTH", "DAY", "HOURS", "MINUTES", "SECONDS", "TIMEZONE", "TZ"])),
            23 => format!("xsd:{}({a})", pick(&mut r, &["string", "dateTime", "date", "boolean", "integer", "double"])),
            24 => format!("MD5({a})"),
            _ => format!("sameTerm({a}, {b})"),
        };
        let result = match h.eval(&e, &[]) {
            None => "ERROR".to_string(),
            Some(id) => h.env.resolver.term(id).expect("decode").to_ntriples(),
        };
        let _ = writeln!(tsv, "{e}\t{result}");
    }
    std::fs::write(&out, tsv).expect("write TSV");
}

/// Differential probe (ignored): date/time accessors, casts and comparisons, written as TSV
/// to `SQ_REVIEW_DIFF_OUT` for comparison with pyoxigraph.
#[test]
#[ignore = "writes a TSV; set SQ_REVIEW_DIFF_OUT"]
fn review_date_function_diff() {
    use sq_core::eval_api::TermResolver;
    use sq_core::testing::Rng;
    use std::fmt::Write as _;
    let Ok(out) = std::env::var("SQ_REVIEW_DIFF_OUT") else { return };
    fn pick<'a>(r: &mut Rng, xs: &[&'a str]) -> &'a str {
        xs[r.below(xs.len() as u64) as usize]
    }
    let vals = [
        "\"2011-01-10T14:45:13.815-05:00\"^^xsd:dateTime", "\"2011-01-10T14:45:13Z\"^^xsd:dateTime",
        "\"2011-01-10T14:45:13\"^^xsd:dateTime", "\"2011-01-10T14:45:13+00:00\"^^xsd:dateTime",
        "\"1999-12-31T23:59:59.999+14:00\"^^xsd:dateTime", "\"2000-01-01T00:00:00-14:00\"^^xsd:dateTime",
        "\"2011-01-10T24:00:00\"^^xsd:dateTime", "\"-0044-03-15T12:00:00Z\"^^xsd:dateTime",
        "\"2011-01-10\"^^xsd:date", "\"2011-01-10Z\"^^xsd:date", "\"2011-01-10-05:00\"^^xsd:date",
        "\"12:30:00Z\"^^xsd:time", "\"12:30:00\"^^xsd:time", "\"23:59:59.5+01:00\"^^xsd:time",
        "\"2011\"^^xsd:gYear", "\"2011-02\"^^xsd:gYearMonth", "\"--02-29\"^^xsd:gMonthDay", "\"---31\"^^xsd:gDay",
        "\"--12\"^^xsd:gMonth", "\"P1Y2M\"^^xsd:yearMonthDuration", "\"PT36H\"^^xsd:dayTimeDuration",
        "\"P1DT2H\"^^xsd:duration", "\"-P1D\"^^xsd:dayTimeDuration", "\"2011-01-10T14:45:13.815-05:00\"",
        "\" 2011-01-10 \"", "\"2011-01-10T14:45:13.1234567Z\"^^xsd:dateTime",
    ];
    let mut r = Rng(0xDA7E_5EED);
    let mut h = Harness::new();
    let mut tsv = String::new();
    for _ in 0..5000 {
        let a = pick(&mut r, &vals).to_string();
        let b = pick(&mut r, &vals).to_string();
        let e = match r.below(5) {
            0 | 1 => format!(
                "{}({a})",
                pick(&mut r, &["YEAR", "MONTH", "DAY", "HOURS", "MINUTES", "SECONDS", "TIMEZONE", "TZ", "STR", "DATATYPE"])
            ),
            2 => format!(
                "xsd:{}({a})",
                pick(&mut r, &["string", "dateTime", "date", "time", "gYear", "gYearMonth", "gMonth", "gMonthDay", "gDay", "duration", "dayTimeDuration", "yearMonthDuration"])
            ),
            3 => format!("({a} {} {b})", pick(&mut r, &["=", "!=", "<", ">", "<=", ">="])),
            _ => format!("sameTerm({a}, {b})"),
        };
        let result = match h.eval(&e, &[]) {
            None => "ERROR".to_string(),
            Some(id) => h.env.resolver.term(id).expect("decode").to_ntriples(),
        };
        let _ = writeln!(tsv, "{e}\t{result}");
    }
    std::fs::write(&out, tsv).expect("write TSV");
}

/// Resolver wrapper counting `term()` decodes (dictionary I/O in the real host).
struct CountingResolver<'a> {
    inner: &'a mut crate::testing::TestResolver,
    decodes: usize,
}

impl sq_core::eval_api::TermResolver for CountingResolver<'_> {
    fn term(&mut self, id: TermId) -> sq_core::Result<std::sync::Arc<Term>> {
        self.decodes += 1;
        self.inner.term(id)
    }
    fn intern(&mut self, t: &Term, canonical: bool) -> sq_core::Result<TermId> {
        self.inner.intern(t, canonical)
    }
    fn fresh_bnode(&mut self) -> TermId {
        self.inner.fresh_bnode()
    }
}

/// Number of `TermResolver::term` calls made while evaluating `expr` as a FILTER on each
/// binding of ?x (fresh ValueCache, so every decode reaches the resolver).
fn filter_decodes(h: &mut Harness, expr: &str, xs: &[TermId]) -> (usize, Vec<Option<bool>>) {
    use rand::SeedableRng;
    let c = h.compile(expr);
    let rows: Vec<Vec<TermId>> = xs.iter().map(|x| h.row(&[("x", *x)])).collect();
    let mut counting = CountingResolver { inner: &mut h.env.resolver, decodes: 0 };
    let mut values = crate::ValueCache::new();
    let mut rng = rand::rngs::StdRng::seed_from_u64(1);
    let mut no = crate::testing::NoExists;
    let mut out = vec![];
    {
        let mut cx = crate::EvalCtx {
            resolver: &mut counting,
            exists: &mut no,
            values: &mut values,
            now: h.env.now,
            base_iri: None,
            rng: &mut rng,
        };
        for row in &rows {
            out.push(c.ebv(row, &mut cx).expect("fatal"));
        }
    }
    (counting.decodes, out)
}

/// Spec §6.1: `IdEq` / `InIds` compare ids. Query constants are canonical (IR contract), so a
/// constant that is a TEMP id is absent from the store and can never equal a dictionary id.
/// The fast paths nevertheless treat "dictionary slot vs TEMP constant" as ambiguous and
/// decode BOTH terms for every row: `FILTER(?x != <absent>)` or `?x IN (<a>, <absent>)`
/// pays a dictionary decode per row instead of ~10 ns.
#[test]
fn review_id_fast_paths_decode_every_row_against_absent_constants() {
    let mut h = Harness::new();
    let xs: Vec<TermId> = (0..100).map(|i| h.term(&format!("<http://example.org/s{i}>"))).collect();
    let lits: Vec<TermId> = (0..100).map(|i| h.term(&format!("\"a stored long literal {i}\""))).collect();
    // Sanity: a stored constant needs no decode at all.
    let (d, r) = filter_decodes(&mut h, "?x = <http://example.org/s0>", &xs);
    assert_eq!(r.iter().filter(|b| **b == Some(true)).count(), 1);
    assert_eq!(d, 0, "stored IRI constant");
    for expr in [
        "?x = <http://example.org/absent>",
        "?x != <http://example.org/absent>",
        "?x IN (<http://example.org/s0>, <http://example.org/absent>)",
    ] {
        let (d, r) = filter_decodes(&mut h, expr, &xs);
        assert!(r.iter().all(Option::is_some), "{expr}: no errors expected");
        assert_eq!(d, 0, "{expr}: {d} term decodes for 100 rows of dictionary IRIs");
    }
    let (d, _) = filter_decodes(&mut h, "?x = \"an absent long simple literal\"", &lits);
    assert_eq!(d, 0, "absent simple-literal constant: {d} term decodes for 100 rows");
}

/// Intermediate values inside a FILTER should not be interned: the execution `TempDict` is
/// append-only, so interning per row grows memory with the number of distinct rows. A
/// derived-integer cast compared numerically (`xsd:int(?x) > 5`) and `sameTerm` on a computed
/// string both intern a TEMP term for every row.
#[test]
fn review_filter_intermediates_grow_the_temp_dict() {
    let mut h = Harness::new();
    let xs: Vec<TermId> = (0..1000).map(|i| h.term(&format!("\"{}\"^^xsd:integer", 1_000_000 + i))).collect();
    let longs: Vec<TermId> = (0..1000).map(|i| h.term(&format!("\"stored literal number {i}\""))).collect();
    let y = h.term("<http://example.org/y>");
    let mut grew = vec![];
    for (expr, inputs) in [("xsd:int(?x) > 5", &xs), ("sameTerm(CONCAT(?x, \"-suffix\"), ?y)", &longs)] {
        let c = h.compile(expr);
        let before = h.env.resolver.temps.len();
        for x in inputs.iter() {
            let row = h.row(&[("x", *x), ("y", y)]);
            let r = h.env.with(|cx| c.ebv(&row, cx)).expect("fatal");
            assert!(r.is_some(), "{expr}");
        }
        let grown = h.env.resolver.temps.len() - before;
        if grown != 0 {
            grew.push(format!("{expr}: TempDict grew by {grown} entries over {} FILTER rows", inputs.len()));
        }
    }
    assert!(grew.is_empty(), "{}", grew.join("\n"));
}

/// XSD/XPath character classes have no set operators: `[a&&b]` is the class {a, &, b} and
/// `[a~~b]` is {a, ~, b}. The `regex` crate reads `&&` / `~~` inside a class as intersection /
/// symmetric difference, and `translate` copies them through unchanged, so these patterns
/// silently match a different language instead of being translated (escaped) or rejected.
#[test]
fn review_regex_class_set_operators_are_literal_in_xpath() {
    let mut h = Harness::new();
    for (expr, expected) in [
        (r#"REGEX("&", "[a&&b]")"#, Some(true)),
        (r#"REGEX("a", "[a&&b]")"#, Some(true)),
        (r#"REGEX("~", "[a~~b]")"#, Some(true)),
    ] {
        assert_eq!(h.ebv(expr, &[]), expected, "{expr}");
    }
}
