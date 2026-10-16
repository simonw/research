//! Diagnostic for the `max_map_size` enforcement gap.
//!
//! Finding: object-map index assignment (`m[k] = v`) and property assignment
//! (`m.k = v`) do NOT re-check the size of the enclosing map — `check_data_size`
//! at src/eval/chaining.rs:790 inspects the *inserted item*, not the container.
//! The limit is only enforced later, when the oversized map is passed to (or
//! returned from) a function call (src/func/call.rs:452). So the memory is
//! already spent by the time the error fires.
use rhai::{Engine, Map};
use std::time::Instant;

fn peak_rss_kib() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .unwrap_or_default()
        .lines()
        .find(|l| l.starts_with("VmHWM:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

fn main() {
    println!("=== max_map_size enforcement gap ===\n");

    // A) Grow the map by index assignment and *return the map itself*.
    // The script never calls a function on the map, so nothing is checked.
    let mut engine = Engine::new();
    engine.set_max_map_size(1_000);
    let before = peak_rss_kib();
    let start = Instant::now();
    let script = "let m = #{}; for i in 0..200_000 { m[`k_${i}`] = i; } m";
    match engine.eval::<Map>(script) {
        Ok(m) => println!(
            "A) index-assign, return map directly: max_map_size=1000 but map has {} entries \
             ({:.2?}, peak +{} KiB) -> LIMIT BYPASSED",
            m.len(),
            start.elapsed(),
            peak_rss_kib().saturating_sub(before)
        ),
        Err(e) => println!("A) stopped -> {e}"),
    }

    // B) Contrast: touching the map with a *property* assignment DOES trigger a
    // check of the whole enclosing map (chaining.rs:925 checks `target.source()`),
    // so a single dummy `m.tick = i` inside the loop restores enforcement.
    let mut engine = Engine::new();
    engine.set_max_map_size(1_000);
    let script = "let m = #{}; for i in 0..200_000 { m[`k_${i}`] = i; m.tick = i; } m";
    let start = Instant::now();
    match engine.eval::<Map>(script) {
        Ok(m) => println!("B) index+property assign: {} entries -> BYPASSED", m.len()),
        Err(e) => println!(
            "B) index-assign + a property-assign in the loop: STOPPED after {:.2?} -> {e} \
             (property assignment checks the whole map, index assignment does not)",
            start.elapsed()
        ),
    }

    // C) Contrast: arrays ARE checked, because growth goes through push().
    let mut engine = Engine::new();
    engine.set_max_array_size(1_000);
    let script = "let a = []; for i in 0..200_000 { a.push(i); } a";
    let start = Instant::now();
    match engine.eval::<rhai::Array>(script) {
        Ok(a) => println!("C) array push: {} entries -> BYPASSED", a.len()),
        Err(e) => println!(
            "C) array push, max_array_size=1000: STOPPED after {:.2?} -> {e} (enforced promptly)",
            start.elapsed()
        ),
    }

    // D) Workaround #1: an operations budget bounds the loop regardless.
    let mut engine = Engine::new();
    engine.set_max_map_size(1_000).set_max_operations(100_000);
    let start = Instant::now();
    let script = "let m = #{}; for i in 0..200_000 { m[`k_${i}`] = i; } m";
    match engine.eval::<Map>(script) {
        Ok(m) => println!("D) with max_operations: {} entries -> still bypassed", m.len()),
        Err(e) => println!(
            "D) max_map_size + max_operations=100k: STOPPED after {:.2?} -> {e} \
             (operations budget is the real backstop)",
            start.elapsed()
        ),
    }

    // E) Workaround #2: check the returned value on the Rust side.
    let mut engine = Engine::new();
    engine.set_max_map_size(1_000);
    let script = "let m = #{}; for i in 0..50_000 { m[`k_${i}`] = i; } m";
    let value = engine.eval::<rhai::Dynamic>(script).unwrap();
    match engine.ensure_data_size_within_limits(&value) {
        Ok(()) => println!("E) host-side re-check passed (unexpected)"),
        Err(e) => println!(
            "E) host-side engine.ensure_data_size_within_limits() on the result -> {e} \
             (catches it, but only after the RAM was spent)"
        ),
    }
}
