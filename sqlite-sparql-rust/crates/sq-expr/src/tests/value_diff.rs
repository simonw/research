//! Differential check against pyoxigraph (ignored by default). Writes random arithmetic,
//! cast, rounding and comparison expressions with this crate's results as TSV
//! (`expression \t N-Triples result or ERROR`) to `SQ_VALUE_DIFF_OUT`; a script then
//! evaluates the same expressions with pyoxigraph and compares after numeric normalization:
//! `SQ_VALUE_DIFF_OUT=/tmp/x.tsv cargo test -p sq-expr value_diff -- --ignored`.
use std::fmt::Write as _;

use sq_core::eval_api::TermResolver;
use sq_core::testing::Rng;

use super::harness::Harness;

fn pick<'a>(r: &mut Rng, xs: &[&'a str]) -> &'a str {
    xs[r.below(xs.len() as u64) as usize]
}

fn number(r: &mut Rng) -> String {
    let i = r.below(2001) as i64 - 1000;
    match r.below(7) {
        0 | 1 => format!("{i}"),
        2 => format!("{}.{}", i / 10, r.below(1000)),
        3 => format!("{}e{}", i, r.below(7) as i64 - 3),
        4 => format!("\"{}\"^^xsd:float", pick(r, &["1.5", "-0.25", "3", "1e10", "0", "7.875", "NaN", "INF"])),
        5 => format!("\"{}\"^^xsd:double", pick(r, &["0.1", "-2.5", "1e300", "0", "NaN", "-INF", "123.456"])),
        _ => pick(r, &["0", "1", "-1", "2.5", "-2.5", "0.5", "9223372036854775807"]).to_string(),
    }
}

fn string_lit(r: &mut Rng) -> String {
    format!(
        "\"{}\"",
        pick(r, &["12", " 12 ", "+3.5", "1e3", "abc", "true", "0", "1", "-0.0", "INF", "NaN", "2.", ".5", "1.0E0", "", "0x10", "07"])
    )
}

fn expr(r: &mut Rng, depth: u32) -> String {
    if depth == 0 {
        return number(r);
    }
    let a = expr(r, depth - 1);
    match r.below(12) {
        0..=3 => format!("({a} {} {})", pick(r, &["+", "-", "*", "/"]), expr(r, depth - 1)),
        4 => format!("(-{a})"),
        5 => format!("{}({a})", pick(r, &["ABS", "CEIL", "FLOOR", "ROUND"])),
        6 => format!(
            "xsd:{}({})",
            pick(r, &["integer", "decimal", "double", "float", "boolean", "string"]),
            if r.below(2) == 0 { a } else { string_lit(r) }
        ),
        7 => format!("xsd:{}({a})", pick(r, &["integer", "decimal", "double", "float", "string"])),
        8 => format!("({a} {} {})", pick(r, &["<", "<=", ">", ">=", "=", "!="]), expr(r, depth - 1)),
        9 => format!("STR({a})"),
        10 => format!("DATATYPE({a})"),
        _ => format!("IF({a} > 0, {}, {})", expr(r, depth - 1), number(r)),
    }
}

#[test]
#[ignore = "writes a TSV for py value_diff; set SQ_VALUE_DIFF_OUT"]
fn value_diff() {
    let Ok(out) = std::env::var("SQ_VALUE_DIFF_OUT") else {
        eprintln!("SQ_VALUE_DIFF_OUT not set; skipping");
        return;
    };
    let n: usize = std::env::var("SQ_VALUE_DIFF_N").ok().and_then(|v| v.parse().ok()).unwrap_or(5000);
    let mut h = Harness::new();
    let mut r = Rng(0x5EED_1234_ABCD);
    let mut tsv = String::new();
    for _ in 0..n {
        let depth = 1 + r.below(3) as u32;
        let e = expr(&mut r, depth);
        let result = match h.eval(&e, &[]) {
            None => "ERROR".to_string(),
            Some(id) => h.env.resolver.term(id).expect("decode").to_ntriples(),
        };
        let _ = writeln!(tsv, "{e}\t{result}");
    }
    std::fs::write(&out, tsv).expect("write TSV");
    eprintln!("wrote {n} expressions to {out}");
}
