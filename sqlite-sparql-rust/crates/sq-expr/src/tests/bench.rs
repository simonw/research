//! Micro-benchmarks of the hot paths (ignored by default):
//! `cargo test --release -p sq-expr bench -- --ignored --nocapture`.
//! `SQ_BENCH_ROWS` sets the rows per case (default 200 000), `SQ_BENCH_ONLY` selects cases
//! whose name contains the given text.
use std::hint::black_box;
use std::time::Instant;

use sq_core::TermId;
use sq_core::ir::{AggFunc, AggSpec};

use super::harness::Harness;
use crate::{accumulator, sort_key};

struct Bench {
    rows: usize,
    only: Option<String>,
}

impl Bench {
    fn from_env() -> Bench {
        Bench {
            rows: std::env::var("SQ_BENCH_ROWS").ok().and_then(|v| v.parse().ok()).unwrap_or(200_000),
            only: std::env::var("SQ_BENCH_ONLY").ok(),
        }
    }

    fn selected(&self, name: &str) -> bool {
        self.only.as_ref().is_none_or(|f| name.contains(f.as_str()))
    }

    fn report(&self, name: &str, start: Instant) {
        let ns = start.elapsed().as_nanos() as f64 / self.rows as f64;
        eprintln!("{name:<48} {ns:>8.1} ns/row");
    }

    /// Times `expr` as a FILTER over rows binding ?x (and ?y) to the given ids.
    fn filter(&self, h: &mut Harness, name: &str, expr: &str, xs: &[TermId], ys: &[TermId]) {
        if !self.selected(name) {
            return;
        }
        let c = h.compile(expr);
        let rows: Vec<Vec<TermId>> = (0..xs.len())
            .map(|i| {
                let mut b = vec![("x", xs[i])];
                if !ys.is_empty() {
                    b.push(("y", ys[i % ys.len()]));
                }
                h.row(&b)
            })
            .collect();
        // Warm the value cache.
        h.env.with(|cx| rows.iter().for_each(|r| drop(c.ebv(r, cx))));
        let n = self.rows;
        let start = Instant::now();
        let mut kept = 0usize;
        h.env.with(|cx| {
            for r in rows.iter().cycle().take(n) {
                if c.ebv(black_box(r), cx).unwrap() == Some(true) {
                    kept += 1;
                }
            }
        });
        self.report(name, start);
        black_box(kept);
    }
}

#[test]
#[ignore = "benchmark: run with --release --ignored --nocapture"]
fn bench_hot_paths() {
    let b = Bench::from_env();
    let mut h = Harness::new();
    let ints: Vec<TermId> = (0..1000).map(|i| h.term(&format!("{}", i * 7 % 1000))).collect();
    let iris: Vec<TermId> = (0..1000).map(|i| h.term(&format!("<http://example.org/r{i}>"))).collect();
    let strs: Vec<TermId> = (0..1000).map(|i| h.term(&format!("\"abc string number {i}\""))).collect();
    let langs: Vec<TermId> = (0..1000).map(|i| h.term(&format!("\"label {i}\"@{}", ["en", "fr", "de"][i % 3]))).collect();
    let dbls: Vec<TermId> = (0..1000).map(|i| h.term(&format!("\"{}.5e0\"^^xsd:double", i % 10))).collect();
    let decs: Vec<TermId> = (0..1000).map(|i| h.term(&format!("{}.25", i % 10))).collect();

    b.filter(&mut h, "?x > 500 (inline INT fast path)", "?x > 500", &ints, &[]);
    b.filter(&mut h, "?x = <iri> (IdEq)", "?x = <http://example.org/r5>", &iris, &[]);
    let in3 = "?x IN (<http://example.org/r1>, <http://example.org/r2>, <http://example.org/r3>)";
    b.filter(&mut h, "?x IN (3 IRIs) (InIds)", in3, &iris, &[]);
    b.filter(&mut h, "isIRI(?x) (TagIs)", "isIRI(?x)", &iris, &[]);
    b.filter(&mut h, "BOUND(?x)", "BOUND(?x)", &iris, &[]);
    b.filter(&mut h, "?x + ?y > 1000 (integer arithmetic)", "?x + ?y > 1000", &ints, &ints);
    b.filter(&mut h, "?x < ?y (two INT slots)", "?x < ?y", &ints, &ints[1..]);
    b.filter(&mut h, "?x > 4.0e0 (dictionary doubles)", "?x > 4.0e0", &dbls, &[]);
    b.filter(&mut h, "?x * 2 > 9 (inline decimals)", "?x * 2 > 9", &decs, &[]);
    b.filter(&mut h, "REGEX(?x, \"^abc string number 1\")", "REGEX(?x, \"^abc string number 1\")", &strs, &[]);
    b.filter(&mut h, "REGEX(?x, \"number [0-9]5$\")", "REGEX(?x, \"number [0-9]5$\")", &strs, &[]);
    b.filter(&mut h, "CONTAINS(?x, \"99\")", "CONTAINS(?x, \"99\")", &strs, &[]);
    b.filter(&mut h, "STRLEN(?x) > 20", "STRLEN(?x) > 20", &strs, &[]);
    b.filter(&mut h, "LANG(?x) = \"en\" (LangEq)", "LANG(?x) = \"en\"", &langs, &[]);
    b.filter(&mut h, "LANGMATCHES(LANG(?x), \"en\")", "LANGMATCHES(LANG(?x), \"en\")", &langs, &[]);
    b.filter(&mut h, "?x = \"abc string number 5\" (IdEq string)", "?x = \"abc string number 5\"", &strs, &[]);
    // Constants absent from the store (TEMP ids): decided from the ids, no decode.
    b.filter(&mut h, "?x != <absent> (IdEq, TEMP constant)", "?x != <http://example.org/absent>", &iris, &[]);
    let in_absent = "?x IN (<http://example.org/r1>, <http://example.org/absent>)";
    b.filter(&mut h, "?x IN (<r1>, <absent>) (InIds, TEMP constant)", in_absent, &iris, &[]);
    b.filter(&mut h, "sameTerm(?x, <absent>)", "sameTerm(?x, <http://example.org/absent>)", &iris, &[]);
    // Ordering a non-numeric id against a number: a type error decided from the tag.
    b.filter(&mut h, "?x > 10 over IRIs (type error from tag)", "?x > 10", &iris, &[]);
    b.filter(&mut h, "?x < 10.5 over strings (type error from tag)", "?x < 10.5", &strs, &[]);
    // Intermediates that are never interned.
    b.filter(&mut h, "xsd:int(?x) > 500 (derived-integer cast)", "xsd:int(?x) > 500", &ints, &[]);
    b.filter(&mut h, "sameTerm(CONCAT(?x, \"!\"), ?y)", "sameTerm(CONCAT(?x, \"!\"), ?y)", &strs, &strs);
    // Dynamic regex: (pattern, flags) cache hit per row.
    let pats: Vec<TermId> = ["^abc string number 1", "number [0-9]5$", "9|8"].iter().map(|p| h.term(&format!("\"{p}\""))).collect();
    b.filter(&mut h, "REGEX(?x, ?y) (dynamic pattern, cached)", "REGEX(?x, ?y)", &strs, &pats);

    // BIND producing new terms (computed value → id); `canonical` as an Extend would pass it.
    for (name, expr, xs, canonical) in [
        ("BIND(?x + 1) (inline result)", "?x + 1", &ints, false),
        ("BIND(CONCAT(?x, \"!\")) (TEMP intern)", "CONCAT(?x, \"!\")", &strs, false),
        ("BIND(UCASE(?x))", "UCASE(?x)", &strs, false),
        ("BIND(REPLACE(?x, \"(num)ber\", \"$1\")) (prepared once)", "REPLACE(?x, \"(num)ber\", \"$1\")", &strs, false),
        ("BIND(COALESCE(?u, \"no label\")) (TEMP constant)", "COALESCE(?u, \"no label available\")", &strs, true),
        ("BIND(BNODE(?x)) (BNODE(str) scope)", "BNODE(?x)", &strs, true),
    ] {
        if !b.selected(name) {
            continue;
        }
        let c = h.compile(expr);
        let rows: Vec<Vec<TermId>> = xs.iter().map(|x| h.row(&[("x", *x)])).collect();
        let start = Instant::now();
        h.env.with(|cx| {
            for r in rows.iter().cycle().take(b.rows) {
                black_box(c.eval(r, cx, canonical).unwrap());
            }
        });
        b.report(name, start);
    }

    // ORDER BY keys.
    for (name, xs) in [("sort_key(inline INT)", &ints), ("sort_key(dictionary string)", &strs), ("sort_key(double)", &dbls)] {
        if !b.selected(name) {
            continue;
        }
        let start = Instant::now();
        h.env.with(|cx| {
            for x in xs.iter().cycle().take(b.rows) {
                black_box(sort_key(*x, cx).unwrap());
            }
        });
        b.report(name, start);
    }

    // Aggregates.
    for (name, func, xs) in [
        ("SUM(inline INT)", AggFunc::Sum, &ints),
        ("AVG(double)", AggFunc::Avg, &dbls),
        ("MAX(inline INT)", AggFunc::Max, &ints),
        ("MIN(string)", AggFunc::Min, &strs),
    ] {
        if !b.selected(name) {
            continue;
        }
        let mut acc = accumulator(&AggSpec { func, distinct: false, arg: None });
        let start = Instant::now();
        h.env.with(|cx| {
            for x in xs.iter().cycle().take(b.rows) {
                acc.add(Some(*x), cx).unwrap();
            }
            black_box(acc.finish(cx).unwrap());
        });
        b.report(name, start);
    }
}
