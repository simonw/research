//! Adversarial review of WP8 (optimizer): plan shapes at a realistic scale on SQLite, SQL
//! chain limits, planning time, the test harness, and a richer end-to-end differential workload
//! (sq-exec on MemStore and SQLite against sq-oracle) than `differential.rs`: [NOT] EXISTS,
//! MINUS, UNION, VALUES, BIND, sub-selects, nested OPTIONALs, paths, GRAPH, integer ranges,
//! GROUP BY and queries without variables, over data large enough for `sort_left`, galloping
//! merges and long merge runs. Environment knobs: REVIEW_SEED, REVIEW_CASES, REVIEW_SIZE.
mod common;

use std::rc::Rc;

use common::*;
use sq_core::plan::{JoinPolicy, Op, QueryOptions, QueryPlan};
use sq_core::store::QuadStore;

fn with(policy: JoinPolicy) -> QueryOptions {
    QueryOptions { join_policy: policy, ..Default::default() }
}

/// Like `common::execute`, but a SELECT without variables yields one empty solution per row
/// (`common::execute` counts rows by the returned ids, of which there are none at width 0).
fn execute_rows(store: Rc<dyn QuadStore>, plan: QueryPlan, opts: &QueryOptions) -> Result<Vec<Solution>, String> {
    use sq_core::eval_api::SolutionSource;
    if !matches!(&plan.form, sq_core::ir::QueryForm::Select(v) if v.is_empty()) {
        return execute(store, plan, opts);
    }
    let caches = sq_exec::ExecCaches::default();
    let mut ex = sq_exec::Execution::new(store, plan, opts, caches).map_err(|e| e.to_string())?;
    let (mut ids, mut rows) = (Vec::new(), 0);
    loop {
        let n = ex.next_rows(&mut ids, 1024).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        rows += n;
    }
    Ok(vec![Vec::new(); rows])
}

/// The cost model's shapes at a realistic scale (20K persons, ~140K triples, SQL cursor
/// costs): chain-2 COUNT is a factorized merge of the o-sorted and s-sorted knows streams, the
/// selective star is one SqlJoin, the range filter an IntRange scan, ORDER BY + LIMIT a TopK.
#[test]
fn review_cost_model_shapes_at_scale_on_sqlite() {
    let db = sqlite_social(20_000);
    let _read = db.store.read_scope(0).expect("read scope");
    let shape = |q: &str| {
        let p = plan_on(q, &db.store, &QueryOptions::default());
        (sq_plan::explain(&p, None), p)
    };
    let (text, p) = shape("SELECT (COUNT(*) AS ?n) { ?a :knows ?b . ?b :knows ?c }");
    let count = nodes(&p).into_iter().find_map(|n| match &n.op {
        Op::Count { input, .. } => Some(input.clone()),
        _ => None,
    });
    let merge = count.expect("Count");
    assert!(matches!(&merge.op, Op::MergeJoin { check, .. } if check.is_empty()), "{text}");
    assert!(text.contains("fast=merge-factorized"), "{text}");
    let (text, p) = shape("SELECT ?x ?a ?c ?n { ?x a :C3 ; :age ?a ; :city ?c ; :name ?n }");
    assert_eq!(count_ops(&p, |o| matches!(o, Op::SqlJoin { .. })), 1, "{text}");
    let (text, p) = shape("SELECT ?x ?age { ?x :age ?age FILTER(?age > 90) }");
    assert!(nodes(&p).iter().any(|n| matches!(&n.op, Op::Scan(s) if s.range.map(|r| r.lo) == Some(91))), "{text}");
    let (text, p) = shape("SELECT ?x ?age { ?x :age ?age } ORDER BY DESC(?age) LIMIT 10");
    assert_eq!(count_ops(&p, |o| matches!(o, Op::TopK { k: 10, .. })), 1, "{text}");
}

// ------------------------------------------------------------------------------------------
// Random queries (the review-3 generator, extended with numbers and integer ranges).
// ------------------------------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[(self.next() % xs.len() as u64) as usize]
    }
    fn chance(&mut self, n: u64) -> bool {
        self.next() % n == 0
    }
}

const VARS: [&str; 5] = ["?a", "?b", "?c", "?d", "?g"];
const ALL_VARS: &str = "?a ?b ?c ?d ?g ?e ?n";

fn term(r: &mut Rng, consts: bool) -> String {
    if r.chance(9) {
        return r.pick(&["[]", "_:x"]).to_owned();
    }
    if consts && r.chance(5) {
        r.pick(&[":n0", ":n1", ":g0", "3"]).to_owned()
    } else {
        r.pick(&VARS).to_owned()
    }
}

fn triple(r: &mut Rng) -> String {
    let p = r.pick(&[":p", ":q", ":r", ":p", ":q", ":v", ":v", ":p+", "^:r", "?d", ":q?"]);
    format!("{} {p} {} . ", term(r, true), term(r, true))
}

fn number(r: &mut Rng) -> String {
    let v = (r.next() % 12) as i64 - 3;
    match r.next() % 5 {
        0 => format!("{v}"),
        1 => format!("{v}.5"),
        2 => format!("{v}.0e0"),
        3 => format!("-{v}.5"),
        _ => format!("\"{v}\"^^<http://www.w3.org/2001/XMLSchema#integer>"),
    }
}

fn expr(r: &mut Rng, depth: u32) -> String {
    let v = |r: &mut Rng| r.pick(&VARS).to_owned();
    match r.next() % if depth > 0 { 12 } else { 8 } {
        0 => format!("{} = {}", v(r), v(r)),
        1 => format!("{} != {}", v(r), v(r)),
        2 => format!("{} = {}", v(r), r.pick(&[":n0", ":n1", ":g0", "3"])),
        3 => format!("sameTerm({}, {})", v(r), r.pick(&[":n0", ":n1", ":g1"])),
        4 => format!("!bound({})", v(r)),
        5 | 6 => format!("{} {} {}", v(r), r.pick(&[">", ">=", "<", "<="]), number(r)),
        7 => format!("{} != {}", v(r), r.pick(&[":n0", ":n2"])),
        8 => format!("({} && {})", expr(r, depth - 1), expr(r, depth - 1)),
        9 => format!("({} || {})", expr(r, depth - 1), expr(r, depth - 1)),
        10 => format!("EXISTS {{ {} }}", gen_group(r, depth - 1)),
        _ => format!("NOT EXISTS {{ {} }}", gen_group(r, depth - 1)),
    }
}

fn gen_group(r: &mut Rng, depth: u32) -> String {
    let mut out = String::new();
    for _ in 0..1 + r.next() % 3 {
        out.push_str(&triple(r));
    }
    let parts = if depth > 0 { 1 + r.next() % 2 } else { 0 };
    for _ in 0..parts {
        let d = depth - 1;
        let part = match r.next() % 22 {
            0 | 1 => format!("OPTIONAL {{ {} }} ", gen_group(r, d)),
            2 => format!("OPTIONAL {{ {} FILTER({}) }} ", gen_group(r, d), expr(r, 1)),
            3 => format!("{{ {} }} UNION {{ {} }} ", gen_group(r, d), gen_group(r, d)),
            4 => format!("MINUS {{ {} }} ", gen_group(r, d)),
            5 => format!("{{ {} FILTER({}) }} ", gen_group(r, d), expr(r, 1)),
            6 => format!("{{ SELECT * {{ {} }} ORDER BY {ALL_VARS} LIMIT 2 }} ", gen_group(r, d)),
            7 => format!("{{ SELECT {} {} {{ {} }} }} ", r.pick(&VARS), r.pick(&VARS), gen_group(r, d)),
            8 => format!("{{ SELECT DISTINCT {} {{ {} }} }} ", r.pick(&VARS), gen_group(r, d)),
            9 => format!("{{ SELECT ?a (COUNT(*) AS ?n) {{ {} }} GROUP BY ?a }} ", gen_group(r, d)),
            10 => format!("GRAPH {} {{ {} }} ", r.pick(&["?g", "?g", ":g0", ":g1"]), gen_group(r, d)),
            11 => r.pick(&["VALUES ?a { :n0 :n1 UNDEF } ", "VALUES (?b ?c) { (:n1 UNDEF) (UNDEF 3) } "]).to_owned(),
            12 => format!("BIND({} AS ?e) ", r.pick(&["?a", "?b", ":n0", "?g"])),
            13 => format!("{{ SELECT (COUNT(*) AS ?n) {{ {} }} }} ", gen_group(r, d)),
            14 => format!("OPTIONAL {{ {} FILTER({} = {}) }} ", gen_group(r, d), r.pick(&VARS), r.pick(&VARS)),
            15 => format!("FILTER(EXISTS {{ {} }}) ", gen_group(r, d)),
            16 => format!("FILTER(NOT EXISTS {{ {} }}) ", gen_group(r, d)),
            17 => format!("?a :v ?c FILTER(?c {} {}) ", r.pick(&[">", ">=", "<", "<="]), number(r)),
            18 => format!("FILTER({} = {}) ", r.pick(&VARS), r.pick(&[":n0", ":n1", ":g0"])),
            19 => format!(
                "OPTIONAL {{ {} OPTIONAL {{ {} FILTER({} = {}) }} }} ",
                gen_group(r, d),
                gen_group(r, d),
                r.pick(&VARS),
                r.pick(&VARS)
            ),
            20 => format!("{{ {} }} ", gen_group(r, d)),
            _ => format!("FILTER({}) ", expr(r, 1)),
        };
        out.push_str(&part);
    }
    if r.chance(3) {
        out.push_str(&format!("FILTER({}) ", expr(r, 1)));
    }
    out
}

/// Turtle documents for the default graph and :g0 / :g1; `size` triples in the default graph.
fn data(r: &mut Rng, size: usize) -> [String; 3] {
    let nodes = [":n0", ":n1", ":n2", ":n3", ":n4", ":n5", ":g0", ":g1"];
    let objects = ["3", "-2", "0", "7", "2.5", "\"4.0e0\"^^xsd:double", "\"05\"^^xsd:integer", "\"x\""];
    let preds = [":p", ":q", ":r", ":v"];
    let head = "@prefix : <http://ex/> .\n@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .\n";
    let mut docs = [head.to_owned(), head.to_owned(), head.to_owned()];
    for (i, doc) in docs.iter_mut().enumerate() {
        let n = if i == 0 { size } else { size / 3 };
        for _ in 0..n {
            let s = nodes[(r.next() % 6) as usize];
            let p = preds[(r.next() % 4) as usize];
            let o = if p == ":v" || r.chance(6) {
                objects[(r.next() % objects.len() as u64) as usize]
            } else {
                nodes[(r.next() % nodes.len() as u64) as usize]
            };
            doc.push_str(&format!("{s} {p} {o} .\n"));
        }
    }
    docs
}

fn option_sets() -> Vec<(&'static str, QueryOptions)> {
    vec![
        ("auto", with(JoinPolicy::Auto)),
        ("prefer-merge", with(JoinPolicy::PreferMerge)),
        ("prefer-hash", with(JoinPolicy::PreferHash)),
        ("prefer-bind", with(JoinPolicy::PreferBind)),
        ("no-sql", with(JoinPolicy::NoSql)),
        ("no-reorder", QueryOptions { reorder: false, ..Default::default() }),
        ("merge-no-reorder", QueryOptions { reorder: false, ..with(JoinPolicy::PreferMerge) }),
    ]
}

fn run_differential(seed: u64, cases: usize, size: usize) -> (Vec<String>, usize, usize) {
    let mut r = Rng(0x9E37_79B9_7F4A_7C15 ^ seed.wrapping_mul(0x2545_F491_4F6C_DD1D));
    let mut failures = Vec::new();
    let (mut checked, mut merges) = (0, 0);
    for case in 0..cases {
        let docs = data(&mut r, size);
        let mem = sq_memstore::MemStore::new();
        let db = Sqlite::new();
        for (doc, g) in docs.iter().zip([None, Some("http://ex/g0"), Some("http://ex/g1")]) {
            load_ttl(&mem, doc, g);
            db.load(doc, g);
        }
        if case % 2 == 1 {
            mem.analyze();
            db.store.analyze().expect("analyze");
        }
        let mem_rc: Rc<dyn QuadStore> = Rc::new(mem.clone());
        let sql_rc: Rc<dyn QuadStore> = Rc::new(db.store.clone());
        let depth = 1 + (r.next() % 3) as u32;
        let ds = match r.next() % 8 {
            0 => "FROM :g0 FROM NAMED :g0 FROM NAMED :g1 ",
            1 => "FROM :g0 FROM :g1 FROM NAMED :g0 ",
            _ => "",
        };
        let body = gen_group(&mut r, depth);
        let q = match r.next() % 9 {
            0 => format!("SELECT DISTINCT ?a ?b {ds}{{ {body}}}"),
            1 => format!("SELECT * {ds}{{ {body}}} ORDER BY {ALL_VARS} LIMIT 3 OFFSET 1"),
            2 => format!("SELECT ?a (COUNT(*) AS ?cnt) {ds}{{ {body}}} GROUP BY ?a"),
            3 => format!("SELECT (COUNT(*) AS ?cnt) {ds}{{ {body}}}"),
            _ => format!("SELECT * {ds}{{ {body}}}"),
        };
        let union = r.chance(5);
        let Ok(parsed) = sq_plan::parse_query(&format!("{PREFIXES}{q}"), None) else { continue };
        let mut expected_q = QueryOptions::default();
        expected_q.union_default_graph = union;
        let Ok(lowered) = sq_plan::lower_query(&parsed, &mem, &expected_q) else { continue };
        let expected = match oracle(&lowered, &mem) {
            Ok(rows) => rows,
            Err(e) => {
                failures.push(format!("oracle failed on {q}: {e}"));
                continue;
            }
        };
        checked += 1;
        for (name, mut opts) in option_sets() {
            opts.union_default_graph = union;
            for (store_name, st) in [("mem", &mem_rc), ("sqlite", &sql_rc)] {
                let _read = (store_name == "sqlite").then(|| {
                    let scope = db.store.read_scope(0).expect("read scope");
                    db.store.shared().validate_epoch().expect("epoch");
                    scope
                });
                let lowered = match sq_plan::lower_query(&parsed, &**st, &opts) {
                    Ok(l) => l,
                    Err(e) => {
                        failures.push(format!("[{name}/{store_name}] lower error {e}: {q}"));
                        continue;
                    }
                };
                let plan = match sq_plan::optimize(lowered, &**st, &opts) {
                    Ok(p) => p,
                    Err(e) => {
                        failures.push(format!("[{name}/{store_name}] plan error {e}: {q}"));
                        continue;
                    }
                };
                if let Err(e) = sq_plan::validate(&plan) {
                    failures.push(format!("[{name}/{store_name}] invalid plan {e}: {q}"));
                    continue;
                }
                merges += nodes(&plan).iter().filter(|n| matches!(n.op, Op::MergeJoin { .. })).count();
                let text = sq_plan::explain(&plan, None);
                match execute_rows(Rc::clone(st), plan, &opts) {
                    Ok(got) if got == expected => {}
                    Ok(got) => failures.push(format!(
                        "[{name}/{store_name} union={union} case={case}] {q}\n{text}\
                         expected {} rows {:?}\n     got {} rows {:?}",
                        expected.len(),
                        &expected[..expected.len().min(4)],
                        got.len(),
                        &got[..got.len().min(4)]
                    )),
                    Err(e) => {
                        failures.push(format!("[{name}/{store_name} union={union} case={case}] {q}\n{text}error: {e}"))
                    }
                }
            }
        }
    }
    (failures, checked, merges)
}

#[test]
fn review_rich_queries_match_the_oracle_end_to_end() {
    let seed: u64 = std::env::var("REVIEW_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(11);
    let cases: usize = std::env::var("REVIEW_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(150);
    let size: usize = std::env::var("REVIEW_SIZE").ok().and_then(|v| v.parse().ok()).unwrap_or(90);
    let (failures, checked, merges) = run_differential(seed, cases, size);
    eprintln!("checked {checked} queries, {merges} merge joins planned");
    assert!(
        failures.is_empty(),
        "{} failures; first ones:\n{}",
        failures.len(),
        failures.iter().take(3).cloned().collect::<Vec<_>>().join("\n\n")
    );
}

// ------------------------------------------------------------------------------------------
// SQL chains.
// ------------------------------------------------------------------------------------------

/// SQLite joins at most 64 tables in one statement ("at most 64 tables in a join"); an SQL chain
/// must close before it would render a 65th alias. A selective subject star of 70
/// constant-predicate patterns becomes one SqlJoin with 70 aliases under Auto, which SQLite
/// refuses to prepare, so the query fails instead of returning its 3 rows.
#[test]
fn review_sql_join_never_renders_more_than_64_aliases() {
    let mut ttl = String::from("@prefix : <http://ex/> .\n");
    for s in 0..300 {
        if s % 100 == 0 {
            ttl.push_str(&format!(":s{s} a :C .\n"));
        }
        for p in 0..70 {
            ttl.push_str(&format!(":s{s} :p{p} {} .\n", s * 100 + p));
        }
    }
    let db = Sqlite::new();
    db.load(&ttl, None);
    db.store.analyze().expect("analyze");
    let mem = store(ttl.strip_prefix("@prefix : <http://ex/> .\n").unwrap());
    let mut q = String::from("SELECT * { ?x a :C ");
    for p in 0..70 {
        q.push_str(&format!("; :p{p} ?o{p} "));
    }
    q.push('}');
    let expected = oracle(&lower_on(&q, &mem, &QueryOptions::default()), &mem).expect("oracle");
    assert_eq!(expected.len(), 3);
    let _read = db.store.read_scope(0).expect("read scope");
    let plan = plan_on(&q, &db.store, &QueryOptions::default());
    let widest = nodes(&plan)
        .iter()
        .filter_map(|n| match &n.op {
            Op::SqlJoin { sql, .. } => Some(sql.matches(" CROSS JOIN ").count() + 1),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    let text = sq_plan::explain(&plan, None);
    let sql_rc: Rc<dyn QuadStore> = Rc::new(db.store.clone());
    let got = execute(sql_rc, plan, &QueryOptions::default());
    assert!(
        widest <= 64,
        "a SqlJoin with {widest} aliases; executing it gives {:?}",
        got.as_ref().map(|r| r.len())
    );
    assert_eq!(got, Ok(expected), "{text}");
}

// ------------------------------------------------------------------------------------------
// Planning time.
// ------------------------------------------------------------------------------------------

/// CPU time of the calling thread in milliseconds (`CLOCK_THREAD_CPUTIME_ID`).
#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
fn thread_cpu_ms() -> Option<f64> {
    #[repr(C)]
    struct Timespec {
        sec: i64,
        nsec: i64,
    }
    unsafe extern "C" {
        fn clock_gettime(clock: i32, tp: *mut Timespec) -> i32;
    }
    const CLOCK_THREAD_CPUTIME_ID: i32 = 3;
    let mut t = Timespec { sec: 0, nsec: 0 };
    // SAFETY: `t` is a valid, writable timespec (64-bit Linux layout) and the clock id exists.
    let rc = unsafe { clock_gettime(CLOCK_THREAD_CPUTIME_ID, &mut t) };
    (rc == 0).then(|| t.sec as f64 * 1e3 + t.nsec as f64 / 1e6)
}

#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
fn thread_cpu_ms() -> Option<f64> {
    None
}

/// Best of 7 `optimize` times of `q` under each of `opts`, in milliseconds, measured in turns
/// so that a change of machine load affects every one alike: CPU time of this thread where
/// available, else wall-clock time. (Under CPU oversubscription, e.g. the parallel tests of
/// this file next to other builds, best-of-7 wall time inflates the longer of two
/// measurements: a run longer than a scheduler time slice is almost always preempted, a
/// shorter one often is not.)
fn time_plans<const N: usize>(q: &str, st: &dyn QuadStore, opts: [&QueryOptions; N]) -> [f64; N] {
    let parsed = sq_plan::parse_query(&format!("{PREFIXES}{q}"), None).expect("parses");
    let mut best = [f64::INFINITY; N];
    for _ in 0..7 {
        for (b, opts) in best.iter_mut().zip(opts) {
            let lowered = sq_plan::lower_query(&parsed, st, opts).expect("lowers");
            let (wall, cpu) = (std::time::Instant::now(), thread_cpu_ms());
            sq_plan::optimize(lowered, st, opts).expect("plans");
            let ms = match (cpu, thread_cpu_ms()) {
                (Some(a), Some(b)) => b - a,
                _ => wall.elapsed().as_secs_f64() * 1e3,
            };
            *b = b.min(ms);
        }
    }
    best
}

/// The DP's work bound (`MAX_DP_STEPS` subset/unit pairs, sized in WP5 for a ~2 ms planning
/// budget with one state per subset) ignores that interesting orders keep one state per
/// sorted slot and that each step now yields bind / hash / one merge per key candidates. On a
/// 10-pattern group that is not a star (here a 5-clique of `:knows` edges, within the step
/// budget) planning under Auto is 10-17x slower than the single-order search PreferHash still
/// runs (dev profile: ~40-46 ms vs ~2.7-4 ms), far beyond the budget. Best of 7 runs each.
#[test]
fn review_interesting_order_dp_stays_within_the_planning_budget() {
    let mem = social(500);
    let q = "SELECT * { ?a :knows ?b . ?a :knows ?c . ?a :knows ?d . ?a :knows ?e . ?b :knows ?c . ?b :knows ?d . \
             ?b :knows ?e . ?c :knows ?d . ?c :knows ?e . ?d :knows ?e }";
    let [auto, single] = time_plans(q, &mem, [&QueryOptions::default(), &with(JoinPolicy::PreferHash)]);
    assert!(
        auto <= 4.0 * single.max(0.5),
        "planning a 10-pattern clique: Auto {auto:.2} ms vs single-order search {single:.2} ms"
    );
}

// ------------------------------------------------------------------------------------------
// Test harness.
// ------------------------------------------------------------------------------------------

/// `tests/common::execute` (the harness of `differential.rs`) reads a SELECT's rows back from
/// the returned ids, so a SELECT without variables (`SELECT * { :n0 :p [] }`, every pattern
/// variable a blank node) always yields 0 solutions: the differential test cannot check such
/// queries, and would report a false failure if its generator produced one.
#[test]
fn review_harness_counts_solutions_of_selects_without_variables() {
    let mem = store(":n0 :p :x1 , :x2 , :x3 .");
    let q = "SELECT * { :n0 :p [] }";
    let expected = oracle(&lower_on(q, &mem, &QueryOptions::default()), &mem).expect("oracle");
    assert_eq!(expected.len(), 3, "three solutions, each binding no variable");
    let mem_rc: Rc<dyn QuadStore> = Rc::new(mem.clone());
    let plan = plan_on(q, &mem, &QueryOptions::default());
    assert_eq!(execute_rows(Rc::clone(&mem_rc), plan.clone(), &QueryOptions::default()), Ok(expected.clone()));
    assert_eq!(execute(mem_rc, plan, &QueryOptions::default()), Ok(expected), "common::execute");
}
