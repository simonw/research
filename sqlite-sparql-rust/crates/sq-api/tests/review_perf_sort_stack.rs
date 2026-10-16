//! Reviewer test of the engine-lane performance pass (lazy sort keys in sq-exec's Sort).
//!
//! `Sort::sort_run` (crates/sq-exec/src/ops/sort.rs) refines tied runs by recursing once per
//! ORDER BY key. A long key list whose rows all tie therefore recurses as deep as the list is
//! long, and the nesting budget (`src/nesting.rs`, sized so that every accepted request runs
//! on a 2 MiB thread) does not charge ORDER BY keys: before the pass, Sort built the keys in a
//! flat loop and 50,000 keys ran fine on a 2 MiB thread. A stack overflow is not a panic: it
//! kills the host process, so this test aborts its own binary (it is alone in this file).
mod common;

use common::*;

/// `ORDER BY ?p ?p ... ?p` (6,000 keys) over rows that share `?p`: every level of the run
/// refinement keeps a tied run, so the recursion depth equals the key count. The request
/// passes the nesting budget, so it must run on the 2 MiB thread the budget is sized for.
#[test]
fn review_perf_long_order_by_key_list_runs_on_a_2mib_thread() {
    std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(|| {
            let c = loaded();
            let q = format!("SELECT ?s {{ ?s ?p ?o }} ORDER BY {}", "?p ".repeat(6000));
            let n: i64 = one_p(&c, "SELECT count(*) FROM sparql_rows(?1)", [&q]);
            assert_eq!(n, 13);
            // The same key list in TopK (ORDER BY + LIMIT) has no recursion.
            let n: i64 = one_p(&c, "SELECT count(*) FROM sparql_rows(?1)", [format!("{q} LIMIT 3")]);
            assert_eq!(n, 3);
        })
        .unwrap()
        .join()
        .unwrap();
}
