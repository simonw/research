//! Diagnostic: how big does a map actually get before max_map_size fires,
//! and where is the time going?
use rhai::{Engine, INT};
use std::time::Instant;

fn main() {
    let mut engine = Engine::new();
    engine.set_max_map_size(5_000);
    // Catch the error inside the script so we can inspect the map afterwards.
    let script = r#"
        let nests = #{};
        let err = "";
        let last_i = -1;
        try {
            for i in 0..1_000_000 { nests[`nest_${i}`] = i; last_i = i; }
        } catch (e) { err = e.to_string(); }
        [last_i, err]
    "#;
    let start = Instant::now();
    let result = engine.eval::<rhai::Array>(script).unwrap();
    println!("last successful insert index: {}, err: {} ({:.2?})", result[0], result[1], start.elapsed());

    // Timing scaling: is the per-insert cost O(n)?
    for n in [1000 as INT, 2000, 4000] {
        let mut engine = Engine::new();
        engine.set_max_map_size(1_000_000);
        let s = format!("let m = #{{}}; for i in 0..{n} {{ m[`k_${{i}}`] = i; }} m.len()");
        let start = Instant::now();
        let len = engine.eval::<INT>(&s).unwrap();
        println!("insert {len} entries with size-check: {:.2?}", start.elapsed());
    }
    // Same but no limits => no size checking at all.
    for n in [1000 as INT, 2000, 4000] {
        let engine = Engine::new();
        let s = format!("let m = #{{}}; for i in 0..{n} {{ m[`k_${{i}}`] = i; }} m.len()");
        let start = Instant::now();
        let len = engine.eval::<INT>(&s).unwrap();
        println!("insert {len} entries, no limits: {:.2?}", start.elapsed());
    }
}
