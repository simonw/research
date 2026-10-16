//! Differential fuzzing of the native engine against sq-oracle, and of update requests across
//! stores (see `fuzz/mod.rs`).
//!
//! `cargo test -p sq-engine --test differential` runs short seeded runs; the long runs are
//! `cargo test -p sq-engine --test differential -- --ignored --nocapture` (10,000 queries,
//! each under every configuration, and 3,000 update requests). SQ_FUZZ_SEED, SQ_FUZZ_CASES,
//! SQ_FUZZ_UPDATES and SQ_FUZZ_POLICIES (auto,hash,bind,merge,nosql) override the defaults;
//! SQ_FUZZ_BNODE_ANYWHERE=0 keeps `BNODE()` to top-level projections; SQ_FUZZ_VERBOSE=1
//! prints every query.
mod fuzz;

use fuzz::{run, Settings};

fn assert_clean(settings: Settings) {
    let totals = run(&settings);
    assert!(
        totals.invalid * 4 < totals.cases.max(1),
        "too many generated queries were rejected by the parser: {} of {}",
        totals.invalid,
        totals.cases
    );
    assert_eq!(totals.failures, 0, "{} mismatches; first reports:\n{}", totals.failures, totals.reports.join("\n"));
}

#[test]
fn differential_short() {
    assert_clean(Settings::new(1, 300).with_env_overrides());
}

/// The harness itself: a native engine that drops a row must be caught, and the repro
/// shrunk to a few quads and a small query.
#[test]
fn harness_catches_and_shrinks_a_wrong_answer() {
    fuzz::engines::set_sabotage(true);
    let mut settings = Settings::new(7, 60);
    settings.policies = vec![sq_core::plan::JoinPolicy::Auto];
    settings.max_reports = 1;
    let totals = run(&settings);
    fuzz::engines::set_sabotage(false);
    assert!(totals.failures > 0, "the sabotaged engine went unnoticed");
    let report = &totals.reports[0];
    let data = report.split("--- data (TriG):").nth(1).unwrap().split("--- query:").next().unwrap();
    let quads = data.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('@')).count();
    assert!(quads <= 3, "data not shrunk ({quads} quads):\n{report}");
    let query = report.split("--- query:").nth(1).unwrap().split("--- difference").next().unwrap();
    let lines = query.lines().filter(|l| !l.trim().is_empty() && !l.starts_with("PREFIX")).count();
    assert!(lines <= 6, "query not shrunk:\n{report}");
}

#[test]
#[ignore = "long run: 10,000 cases per configuration"]
fn differential_long() {
    assert_clean(Settings::new(20261015, 10_000).with_env_overrides());
}

fn assert_updates_agree(seed: u64, cases: usize) {
    let cases = std::env::var("SQ_FUZZ_UPDATES").ok().and_then(|s| s.parse().ok()).unwrap_or(cases);
    let seed = std::env::var("SQ_FUZZ_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(seed);
    let (n, invalid, reports) = fuzz::update::run(seed, cases, 3);
    assert!(invalid * 4 < n, "too many invalid update requests: {invalid} of {n}");
    assert!(reports.is_empty(), "{}", reports.join("\n"));
}

/// Random update requests agree on MemStore, in-memory SQLite and a SQLite file.
#[test]
fn update_differential_short() {
    assert_updates_agree(1, 150);
}

#[test]
#[ignore = "long run: 3,000 update requests"]
fn update_differential_long() {
    assert_updates_agree(20261015, 3000);
}

/// Replays a saved mismatch report under every configuration (triage aid):
/// `SQ_REPRO=report.txt [SQ_REPRO_UNION=1] [SQ_REPRO_NO_PROBES=1] [SQ_REPRO_ANALYZE=1]
/// [SQ_REPRO_PLANS=1] cargo test -p sq-engine --test differential -- --ignored --nocapture
/// replay_report`.
#[test]
#[ignore = "triage aid: needs SQ_REPRO"]
fn replay_report() {
    let Ok(path) = std::env::var("SQ_REPRO") else {
        eprintln!("set SQ_REPRO to a mismatch report file");
        return;
    };
    let report = std::fs::read_to_string(path).expect("readable report");
    let flag = |name: &str| std::env::var(name).is_ok_and(|v| v == "1");
    eprintln!("{}", fuzz::replay(&report, flag("SQ_REPRO_UNION"), !flag("SQ_REPRO_NO_PROBES"), flag("SQ_REPRO_ANALYZE")));
}
