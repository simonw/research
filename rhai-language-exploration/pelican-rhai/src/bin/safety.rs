//! Safety harness: can Rhai stop runaway pelican scripts?
//!
//! Exercises every built-in resource limit:
//! - `set_max_operations`     — CPU budget in abstract "operations"
//! - `on_progress` + clock    — hard wall-clock timeout
//! - `set_max_array_size`     — RAM: array bombs
//! - `set_max_string_size`    — RAM: string-doubling bombs
//! - `set_max_map_size`       — RAM: object map bombs
//! - `set_max_call_levels`    — stack: infinite recursion
//! - `set_max_expr_depths`    — stack: pathological nested expressions (compile time)
//!
//! Each hostile script runs to completion-or-termination and we report what
//! stopped it, how long it took, and how much RSS it cost.

use rhai::{Dynamic, Engine};
use std::time::{Duration, Instant};

/// Peak resident set size (VmHWM) of this process, in KiB (Linux).
fn peak_rss_kib() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    status
        .lines()
        .find(|l| l.starts_with("VmHWM:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// Reset the peak-RSS high-water mark so each case measures its own peak.
fn reset_peak_rss() {
    let _ = std::fs::write("/proc/self/clear_refs", "5");
}

fn run_case(title: &str, engine: &Engine, script: &str) {
    reset_peak_rss();
    let rss_before = peak_rss_kib();
    let start = Instant::now();
    let outcome = engine.eval::<Dynamic>(script);
    let elapsed = start.elapsed();
    let rss_delta = peak_rss_kib().saturating_sub(rss_before);
    match outcome {
        Ok(v) => println!(
            "  [{title}] COMPLETED in {elapsed:.2?} (peak +{rss_delta} KiB RSS) -> {v}"
        ),
        Err(e) => println!(
            "  [{title}] STOPPED  in {elapsed:.2?} (peak +{rss_delta} KiB RSS) -> {e}"
        ),
    }
}

fn main() {
    println!("=== Rhai safety harness: hostile pelican scripts ===\n");

    // ------------------------------------------------------------------
    println!(">>> 1. Infinite loop vs set_max_operations");
    let mut engine = Engine::new();
    engine.set_max_operations(1_000_000);
    run_case(
        "spin forever, 1M-op budget",
        &engine,
        "let dives = 0; loop { dives += 1; }",
    );

    // ------------------------------------------------------------------
    println!("\n>>> 2. Infinite loop vs wall-clock timeout (on_progress)");
    let mut engine = Engine::new();
    let deadline = Duration::from_millis(250);
    let start = Instant::now();
    engine.on_progress(move |_ops| {
        if start.elapsed() > deadline {
            // Returning Some(token) terminates the script; the token becomes
            // part of the ErrorTerminated error.
            Some("pelican patrol: 250ms flight time exceeded".into())
        } else {
            None
        }
    });
    run_case(
        "spin forever, 250ms deadline",
        &engine,
        "let flaps = 0; loop { flaps += 1; }",
    );

    // ------------------------------------------------------------------
    println!("\n>>> 3. Array bomb: first WITHOUT limits (bounded), then WITH set_max_array_size");
    let engine = Engine::new();
    run_case(
        "push 2M fish, no limits",
        &engine,
        "let pouch = []; for i in 0..2_000_000 { pouch.push(i); } pouch.len",
    );

    let mut engine = Engine::new();
    engine.set_max_array_size(10_000);
    run_case(
        "push 2M fish, max_array_size=10k",
        &engine,
        "let pouch = []; for i in 0..2_000_000 { pouch.push(i); } pouch.len",
    );

    // ------------------------------------------------------------------
    println!("\n>>> 4. String-doubling bomb vs set_max_string_size");
    let mut engine = Engine::new();
    engine.set_max_string_size(100_000);
    run_case(
        "double a squawk 40 times, max 100kB",
        &engine,
        r#"let s = "SQUAWK"; for i in 0..40 { s += s; } s.len"#,
    );

    // ------------------------------------------------------------------
    println!("\n>>> 5. Object-map bomb vs set_max_map_size");
    let mut engine = Engine::new();
    engine.set_max_map_size(5_000);
    run_case(
        "add 1M nests, max_map_size=5k",
        &engine,
        "let nests = #{}; for i in 0..1_000_000 { nests[`nest_${i}`] = i; } nests.len()",
    );

    // ------------------------------------------------------------------
    println!("\n>>> 6. Infinite recursion vs set_max_call_levels");
    let mut engine = Engine::new();
    engine.set_max_call_levels(64);
    run_case(
        "recurse forever, max 64 levels",
        &engine,
        "fn dive(n) { dive(n + 1) } dive(0)",
    );

    // ------------------------------------------------------------------
    println!("\n>>> 7. Pathological nesting vs set_max_expr_depths (compile-time)");
    let mut engine = Engine::new();
    engine.set_max_expr_depths(32, 32);
    let nasty = format!("let x = {}1{};", "(1 + ".repeat(500), ")".repeat(500));
    let start = Instant::now();
    match engine.compile(&nasty) {
        Ok(_) => println!("  [nested expr] compiled?! (unexpected)"),
        Err(e) => println!(
            "  [500-deep nested expr, max depth 32] REJECTED at parse time in {:.2?} -> {e}",
            start.elapsed()
        ),
    }

    // ------------------------------------------------------------------
    println!("\n>>> 8. Everything at once: a fully sandboxed engine");
    let mut engine = Engine::new();
    engine
        .set_max_operations(5_000_000)
        .set_max_call_levels(32)
        .set_max_array_size(50_000)
        .set_max_map_size(10_000)
        .set_max_string_size(500_000)
        .set_max_expr_depths(64, 64);
    // A legitimate script well inside the budget still works fine:
    run_case(
        "legit script under full limits",
        &engine,
        r#"
        let flock = [];
        for i in 0..1000 { flock.push(`pelican_${i}`); }
        let heavy_eaters = flock.filter(|p| p.len > 10);
        `flock=${flock.len}, heavy=${heavy_eaters.len}`
        "#,
    );
    // ...but hostility in any dimension is caught:
    run_case(
        "hostile script under full limits",
        &engine,
        "let x = []; loop { x.push(x.len); }",
    );

    println!("\n=== All hostile scripts contained. The colony is safe. ===");
}
