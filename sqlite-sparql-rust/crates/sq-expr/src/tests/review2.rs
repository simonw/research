//! Adversarial review tests, round 2 (WP4 review). Each `review_*` test pins one SPARQL 1.1 /
//! spec §6 behaviour; a failing one documents a defect in the implementation.
use sq_core::TermId;

use super::harness::Harness;

/// Probe (ignored): evaluates each line of `SQ_REVIEW_PROBE_IN` (one expression per line) and
/// writes `expr \t result` lines to `SQ_REVIEW_PROBE_OUT` for comparison with pyoxigraph.
#[test]
#[ignore = "reads SQ_REVIEW_PROBE_IN, writes SQ_REVIEW_PROBE_OUT"]
fn review_probe_expressions() {
    use sq_core::eval_api::TermResolver;
    use std::fmt::Write as _;
    let (Ok(input), Ok(out)) = (std::env::var("SQ_REVIEW_PROBE_IN"), std::env::var("SQ_REVIEW_PROBE_OUT")) else {
        return;
    };
    let mut h = Harness::new();
    let mut tsv = String::new();
    for line in std::fs::read_to_string(input).expect("read").lines() {
        if line.trim().is_empty() {
            continue;
        }
        let result = match h.eval(line, &[]) {
            None => "ERROR".to_string(),
            Some(id) => h.env.resolver.term(id).expect("decode").to_ntriples(),
        };
        let _ = writeln!(tsv, "{line}\t{result}");
    }
    std::fs::write(out, tsv).expect("write");
}

/// Property check (passes): the id-level paths of `=`, `<`, sameTerm and the ORDER BY key agree
/// with the value-level definitions (`equals_values`, `compare_values`, term identity) over
/// inline, dictionary and TEMP-duplicate ids of every value class, incl. NaN, ill-typed and
/// unknown-datatype literals.
#[test]
fn review_id_paths_match_value_semantics() {
    use crate::{Order, compare_values, equals_values, value_of};
    use sq_core::eval_api::TermResolver;
    let terms = [
        "1", "\"01\"^^xsd:integer", "1.0", "\"1.00\"^^xsd:decimal", "\"1\"^^xsd:int", "\"1e0\"^^xsd:double",
        "\"1\"^^xsd:float", "\"NaN\"^^xsd:double", "true", "\"1\"^^xsd:boolean", "\"abc\"", "\"abcdefghij\"",
        "\"abc\"@en", "\"abc\"@en-gb", "\"x\"^^xsd:integer", "\"x\"^^<http://ex/t>", "<http://ex/a>", "_:b",
        "\"2002-04-01T00:00:00Z\"^^xsd:dateTime", "\"2002-04-01T01:00:00+01:00\"^^xsd:dateTime",
        "\"2002-04-01T00:00:00\"^^xsd:dateTime", "\"2002-04-01T00:00:00.000Z\"^^xsd:dateTime", "\"2002-04-01\"^^xsd:date",
        "\"2002-04-01Z\"^^xsd:date", "\"P1Y\"^^xsd:yearMonthDuration", "\"P12M\"^^xsd:yearMonthDuration",
        "\"PT1H\"^^xsd:dayTimeDuration", "\"\"", "\"5.0\"^^xsd:decimal", "5",
    ];
    let mut h = Harness::new();
    let mut ids: Vec<TermId> = terms.iter().map(|t| h.term(t)).collect();
    // TEMP duplicates of every stored non-inline, non-bnode term.
    let dups: Vec<TermId> = ids
        .iter()
        .filter(|id| !id.is_inline() && !id.is_bnode())
        .map(|id| {
            let t = h.env.resolver.term(*id).unwrap();
            h.env.resolver.temp((*t).clone())
        })
        .collect();
    ids.extend(dups);
    let mut bad = vec![];
    for &a in &ids {
        for &b in &ids {
            let va = h.env.with(|cx| value_of(a, cx)).unwrap();
            let vb = h.env.with(|cx| value_of(b, cx)).unwrap();
            let ta = h.env.resolver.term(a).unwrap();
            let tb = h.env.resolver.term(b).unwrap();
            let same = ta == tb;
            let nan = va.as_num().is_some_and(|n| n.is_nan());
            let want_eq = if same && !nan { Some(true) } else { equals_values(&va, &vb) };
            let got_eq = h.ebv("?a = ?b", &[("a", a), ("b", b)]);
            if got_eq != want_eq {
                bad.push(format!("{ta:?} = {tb:?}: got {got_eq:?} want {want_eq:?}"));
            }
            let want_lt = compare_values(&va, &vb).map(|o| o == Order::Less);
            let got_lt = h.ebv("?a < ?b", &[("a", a), ("b", b)]);
            if got_lt != want_lt {
                bad.push(format!("{ta:?} < {tb:?}: got {got_lt:?} want {want_lt:?}"));
            }
            let got_st = h.ebv("sameTerm(?a, ?b)", &[("a", a), ("b", b)]);
            if got_st != Some(same) {
                bad.push(format!("sameTerm({ta:?}, {tb:?}): got {got_st:?} want {same}"));
            }
            let ka = h.env.with(|cx| crate::sort_key(a, cx)).unwrap();
            let kb = h.env.with(|cx| crate::sort_key(b, cx)).unwrap();
            if (ka == kb) != same {
                bad.push(format!("sort keys of {ta:?} / {tb:?}: equal={} same={same}", ka == kb));
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Resolver wrapper counting `term()` decodes and `intern()` calls (dictionary I/O and
/// dictionary probes in the real host).
struct CountingResolver<'a> {
    inner: &'a mut crate::testing::TestResolver,
    decodes: usize,
    interns: usize,
}

impl sq_core::eval_api::TermResolver for CountingResolver<'_> {
    fn term(&mut self, id: TermId) -> sq_core::Result<std::sync::Arc<sq_core::Term>> {
        self.decodes += 1;
        self.inner.term(id)
    }
    fn intern(&mut self, t: &sq_core::Term, canonical: bool) -> sq_core::Result<TermId> {
        self.interns += 1;
        self.inner.intern(t, canonical)
    }
    fn fresh_bnode(&mut self) -> TermId {
        self.inner.fresh_bnode()
    }
}

/// Evaluates `expr` with `canonical = true` (BIND feeding DISTINCT / GROUP BY / a join) on
/// each row; returns (term() calls, intern() calls, results).
fn eval_canonical_counting(h: &mut Harness, expr: &str, rows: &[Vec<TermId>]) -> (usize, usize, Vec<Option<TermId>>) {
    use rand::SeedableRng;
    let c = h.compile(expr);
    let mut counting = CountingResolver { inner: &mut h.env.resolver, decodes: 0, interns: 0 };
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
        for row in rows {
            out.push(c.eval(row, &mut cx, true).expect("fatal"));
        }
    }
    (counting.decodes, counting.interns, out)
}

/// §6.1 / IR contract: query constants are canonical ids, and a TEMP constant denotes a term
/// absent from the store. A pass-through constant (`BIND(<c> AS ?y)`, the default of
/// `COALESCE(?x, "default label")`, a branch of `IF`) is therefore already canonical, yet
/// `id_of_val` re-interns every TEMP `Val::Id` when `canonical = true`: one `term()` decode
/// plus one canonical `intern()` (a dictionary probe in sq-exec) per row, for a constant.
#[test]
fn review_temp_constant_pass_through_is_reinterned_per_row() {
    let mut h = Harness::new();
    // ?x is mostly unbound (an OPTIONAL label), so COALESCE falls through to the constant.
    let stored = h.term("\"a stored label value\"");
    let rows: Vec<Vec<TermId>> =
        (0..100).map(|i| if i % 10 == 0 { h.row(&[("x", stored)]) } else { h.row(&[]) }).collect();
    for expr in [
        "COALESCE(?x, \"no label available\")",
        "<http://example.org/absent-constant>",
        "IF(BOUND(?x), ?x, \"no label available\")",
    ] {
        let (decodes, interns, out) = eval_canonical_counting(&mut h, expr, &rows);
        assert!(out.iter().all(Option::is_some), "{expr}");
        assert!(
            decodes == 0 && interns == 0,
            "{expr}: {decodes} term() decodes and {interns} canonical intern() probes for 100 rows \
             of an already-canonical TEMP constant"
        );
    }
}

/// SPARQL 1.1 §17.4.2.9: BNODE(str) gives "distinct blank nodes ... for other query
/// solutions". `BNodeScope` identifies a solution by its row content with every blank node
/// created by BNODE(str) masked to UNBOUND, so two solutions that differ ONLY in such a blank
/// node collapse into one key and share the blank node. Query:
///
/// ```sparql
/// SELECT ?b1 ?b2 WHERE {
///   { SELECT ?b1 WHERE { VALUES ?v { 1 2 } BIND(BNODE("k") AS ?b1) } }
///   BIND(BNODE("a") AS ?b2)
/// }
/// ```
/// The sub-select yields two solutions {?b1 = B1} and {?b1 = B2} (?v is projected away, so its
/// slot is reset to UNBOUND); the outer BNODE("a") must give two distinct blank nodes.
#[test]
fn review_bnode_str_distinct_solutions_differing_only_in_generated_bnodes() {
    let mut h = Harness::new();
    let (one, two) = (h.term("1"), h.term("2"));
    let inner = h.compile("BNODE(\"k\")");
    let outer = h.compile("BNODE(\"a\")");
    // Inner sub-select: one blank node per ?v solution (rows use the global slot layout).
    let r1 = h.row(&[("v", one), ("b1", TermId::UNBOUND), ("b2", TermId::UNBOUND)]);
    let r2 = h.row(&[("v", two), ("b1", TermId::UNBOUND), ("b2", TermId::UNBOUND)]);
    let b1 = h.env.with(|cx| inner.eval(&r1, cx, true)).expect("fatal").expect("bnode");
    let b2 = h.env.with(|cx| inner.eval(&r2, cx, true)).expect("fatal").expect("bnode");
    assert_ne!(b1, b2, "inner solutions get distinct blank nodes");
    // Outer BIND over the projected solutions {?b1 = B1} and {?b1 = B2}.
    let s1 = h.row(&[("b1", b1)]);
    let s2 = h.row(&[("b1", b2)]);
    let a1 = h.env.with(|cx| outer.eval(&s1, cx, true)).expect("fatal").expect("bnode");
    let a2 = h.env.with(|cx| outer.eval(&s2, cx, true)).expect("fatal").expect("bnode");
    assert_ne!(a1, a2, "two different solutions share the BNODE(\"a\") blank node");
}

/// Evaluates `expr` as a FILTER (EBV) on each row; returns (term() calls, results).
fn filter_counting(h: &mut Harness, expr: &str, rows: &[Vec<TermId>]) -> (usize, Vec<Option<bool>>) {
    use rand::SeedableRng;
    let c = h.compile(expr);
    let mut counting = CountingResolver { inner: &mut h.env.resolver, decodes: 0, interns: 0 };
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
        for row in rows {
            out.push(c.ebv(row, &mut cx).expect("fatal"));
        }
    }
    (counting.decodes, out)
}

/// §6.1 / §6.4: `?o > 10` against an inline INT constant is "2 compares" for INT ids, but
/// every other id goes through `compare()`, which decodes BOTH operands before discovering a
/// type error. IRIs, blank nodes, simple literals (SHORTSTR / dictionary STR), language
/// literals, booleans, dates and dateTimes can never be ordered against a number (§6.4), so
/// the error is decidable from the tag; instead `FILTER(?o > 10)` over a mixed object column
/// (`?s ?p ?o`, no IntRange pushdown) pays one dictionary decode per distinct IRI / string.
#[test]
fn review_ordering_against_int_constant_decodes_non_numeric_ids() {
    let mut h = Harness::new();
    let mut rows = vec![];
    for i in 0..50 {
        let iri = h.term(&format!("<http://example.org/resource/{i}>"));
        rows.push(h.row(&[("o", iri)]));
        let s = h.term(&format!("\"a long stored literal number {i}\""));
        rows.push(h.row(&[("o", s)]));
        let l = h.term(&format!("\"label {i}\"@en"));
        rows.push(h.row(&[("o", l)]));
    }
    for expr in ["?o > 10", "10 <= ?o", "?o < 10.5"] {
        let (decodes, out) = filter_counting(&mut h, expr, &rows);
        assert!(out.iter().all(Option::is_none), "{expr}: type errors expected");
        assert_eq!(decodes, 0, "{expr}: {decodes} term() decodes for 150 non-numeric rows (errors decidable from tags)");
    }
}
