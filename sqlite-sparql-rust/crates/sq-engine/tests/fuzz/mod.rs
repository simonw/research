//! The differential fuzzer (docs/SPEC.md §12.1, sq-engine row): random datasets and random
//! queries, evaluated by sq-oracle (ground truth) and by the native engine under every join
//! policy x reorder on / off, on a MemStore, an in-memory SQLite store (SQL cursors) and a
//! SQLite file with small pages (the direct B-tree reader), compared as multisets (sets for
//! REDUCED, sequences when ORDER BY covers every projected variable) modulo a bijection of
//! minted blank nodes. The oracle's answers on the SQLite stores are also compared with its
//! answer on MemStore (every blank node free), which checks the stores themselves. The
//! `update` module does the same for SPARQL Update requests across the three stores.
//!
//! Every run is seeded and reproducible; a mismatch is shrunk (query first, then data) and
//! printed as a self-contained repro.
pub mod compare;
pub mod data;
pub mod engines;
pub mod query;
pub mod update;

use std::fmt::Write;
use std::time::Instant;

use sq_core::plan::JoinPolicy;
use sq_core::testing::Rng;

use compare::{same, Free, Outcome};
use data::Dataset;
use engines::{native, oracle, oracle_options, options, Config, StoreKind, Stores};
use query::{Gen, Order, Query};

/// Fuzzer settings.
#[derive(Clone, Debug)]
pub struct Settings {
    pub seed: u64,
    pub cases: usize,
    /// Queries run against each generated dataset.
    pub queries_per_dataset: usize,
    pub policies: Vec<JoinPolicy>,
    /// Oracle outputs larger than this are skipped (the comparison would dominate).
    pub max_rows: usize,
    /// Shrink and print at most this many failures.
    pub max_reports: usize,
    /// Print every case (query and oracle answer size): SQ_FUZZ_VERBOSE=1.
    pub verbose: bool,
    /// Generate `BNODE()` in any expression, not only top-level projections (on by default;
    /// SQ_FUZZ_BNODE_ANYWHERE=0 turns it off). A subpattern that mints blank nodes must be
    /// evaluated once, whatever join re-opens it.
    pub bnode_anywhere: bool,
}

impl Settings {
    pub fn new(seed: u64, cases: usize) -> Settings {
        Settings {
            seed,
            cases,
            queries_per_dataset: 10,
            policies: vec![
                JoinPolicy::Auto,
                JoinPolicy::PreferHash,
                JoinPolicy::PreferBind,
                JoinPolicy::PreferMerge,
                JoinPolicy::NoSql,
            ],
            max_rows: 4000,
            max_reports: 5,
            verbose: false,
            bnode_anywhere: true,
        }
    }

    /// Overrides from the environment: SQ_FUZZ_SEED, SQ_FUZZ_CASES, SQ_FUZZ_POLICIES
    /// (comma-separated: auto, hash, bind, merge, nosql), SQ_FUZZ_BNODE_ANYWHERE.
    pub fn with_env_overrides(mut self) -> Settings {
        if let Some(seed) = std::env::var("SQ_FUZZ_SEED").ok().and_then(|s| s.parse().ok()) {
            self.seed = seed;
        }
        if let Some(cases) = std::env::var("SQ_FUZZ_CASES").ok().and_then(|s| s.parse().ok()) {
            self.cases = cases;
        }
        self.verbose = std::env::var("SQ_FUZZ_VERBOSE").is_ok_and(|v| v == "1");
        if let Ok(v) = std::env::var("SQ_FUZZ_BNODE_ANYWHERE") {
            self.bnode_anywhere = v != "0";
        }
        if let Ok(p) = std::env::var("SQ_FUZZ_POLICIES") {
            self.policies = p
                .split(',')
                .filter_map(|name| match name.trim() {
                    "auto" => Some(JoinPolicy::Auto),
                    "hash" => Some(JoinPolicy::PreferHash),
                    "bind" => Some(JoinPolicy::PreferBind),
                    "merge" => Some(JoinPolicy::PreferMerge),
                    "nosql" => Some(JoinPolicy::NoSql),
                    _ => None,
                })
                .collect();
        }
        self
    }
}

/// What disagreed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Culprit {
    /// The native engine under this configuration differs from the oracle on its store.
    Native(Config),
    /// The oracle gives different answers on MemStore and on SQLite (a store problem).
    Stores,
}

#[derive(Clone, Debug)]
pub struct Failure {
    pub culprit: Culprit,
    pub detail: String,
}

enum CaseResult {
    /// spargebra rejected the generated text (a scoping rule).
    Invalid,
    /// The oracle's answer is too large to compare cheaply.
    TooLarge,
    Checked(Vec<Failure>),
}

/// Totals of a run.
#[derive(Debug, Default)]
pub struct Totals {
    pub cases: usize,
    pub invalid: usize,
    pub too_large: usize,
    pub oracle_errors: usize,
    /// Cases whose oracle answer is empty (no rows, false, no triples).
    pub empty: usize,
    /// Native evaluations compared with the oracle.
    pub comparisons: usize,
    pub failures: usize,
    pub reports: Vec<String>,
    /// Physical operators seen in the plans of sampled cases (coverage census).
    pub operators: std::collections::BTreeMap<&'static str, usize>,
    verbose: bool,
}

fn check(stores: &Stores, q: &Query, configs: &[Config], max_rows: usize, totals: Option<&mut Totals>) -> CaseResult {
    let text = q.to_string();
    let verbose = totals.as_ref().is_some_and(|t| t.verbose);
    let parsed = match sq_plan::parse_query(&text, None) {
        Ok(p) => p,
        Err(e) => {
            if verbose {
                eprintln!("--- invalid ({e}):\n{text}");
            }
            return CaseResult::Invalid;
        }
    };
    let order = q.order();
    let union_default = q.union_default;
    let oopts = oracle_options(union_default);
    let truth_mem = stores.with(StoreKind::Mem, |s| oracle(&*s, &parsed, &oopts));
    if verbose {
        let what = match &truth_mem {
            Outcome::Error(e) => format!("error {e}"),
            o => format!("{} results", o.size()),
        };
        eprintln!("--- case ({what}):\n{text}");
    }
    if truth_mem.size() > max_rows {
        return CaseResult::TooLarge;
    }
    // The oracle on every SQLite store, checked against the oracle on MemStore.
    let mut failures = Vec::new();
    let truth_sqlite = stores.with(StoreKind::Sqlite, |s| oracle(&*s, &parsed, &oopts));
    let truth_file = stores.with(StoreKind::File, |s| oracle(&*s, &parsed, &oopts));
    for t in [&truth_sqlite, &truth_file] {
        if let Err(d) = same(&truth_mem, t, order, Free::All) {
            failures.push(Failure { culprit: Culprit::Stores, detail: describe(d, &truth_mem, t, order) });
            break;
        }
    }
    let mut compared = 0;
    for cfg in configs {
        let truth = match cfg.store {
            StoreKind::Mem => &truth_mem,
            StoreKind::Sqlite => &truth_sqlite,
            StoreKind::File => &truth_file,
        };
        let got = stores.with(cfg.store, |s| native(s, &parsed, &options(cfg, union_default, q.probes)));
        compared += 1;
        if let Err(d) = same(truth, &got, order, Free::Minted) {
            failures.push(Failure { culprit: Culprit::Native(*cfg), detail: describe(d, truth, &got, order) });
        }
    }
    if let Some(t) = totals {
        t.comparisons += compared;
        if t.cases % 5 == 0 {
            census(stores, &parsed, configs, union_default, q.probes, &mut t.operators);
        }
        match &truth_mem {
            Outcome::Error(_) => t.oracle_errors += 1,
            Outcome::Boolean(false) => t.empty += 1,
            Outcome::Solutions { rows, .. } | Outcome::Graph(rows) if rows.is_empty() => t.empty += 1,
            _ => {}
        }
    }
    CaseResult::Checked(failures)
}

/// Counts the operators of the plans of `q` under every configuration.
fn census(
    stores: &Stores,
    q: &spargebra::Query,
    configs: &[Config],
    union_default: bool,
    probes: bool,
    ops: &mut std::collections::BTreeMap<&'static str, usize>,
) {
    use sq_core::plan::{JoinKind, Op, Plan};
    fn walk(p: &Plan, ops: &mut std::collections::BTreeMap<&'static str, usize>) {
        let name = match &p.op {
            Op::Unit => "Unit",
            Op::Empty => "Empty",
            Op::Scan(s) if s.range.is_some() => "Scan+IntRange",
            Op::Scan(_) => "Scan",
            Op::Values { .. } => "Values",
            Op::BindJoin { kind, .. } => match kind {
                JoinKind::Inner => "BindJoin",
                JoinKind::Left { .. } => "BindJoin(Left)",
                JoinKind::Semi => "BindJoin(Semi)",
                JoinKind::Anti => "BindJoin(Anti)",
            },
            Op::HashJoin { kind, .. } => match kind {
                JoinKind::Inner => "HashJoin",
                JoinKind::Left { .. } => "HashJoin(Left)",
                JoinKind::Semi => "HashJoin(Semi)",
                JoinKind::Anti => "HashJoin(Anti)",
            },
            Op::MergeJoin { .. } => "MergeJoin",
            Op::StarMerge { .. } => "StarMerge",
            Op::NestedLoop { kind, .. } => match kind {
                JoinKind::Inner => "NestedLoop",
                JoinKind::Left { .. } => "NestedLoop(Left)",
                JoinKind::Semi => "NestedLoop(Semi)",
                JoinKind::Anti => "NestedLoop(Anti)",
            },
            Op::Minus { .. } => "Minus",
            Op::Union(_) => "Union",
            Op::Filter { .. } => "Filter",
            Op::Extend { .. } => "Extend",
            Op::Path { .. } => "Path",
            Op::GraphLoop { .. } => "GraphLoop",
            Op::GraphNames { .. } => "GraphNames",
            Op::Group { streaming: true, .. } => "Group(streaming)",
            Op::Group { .. } => "Group",
            Op::Count { .. } => "Count",
            Op::Sort { .. } => "Sort",
            Op::SortBySlot { .. } => "SortBySlot",
            Op::TopK { .. } => "TopK",
            Op::Distinct { .. } => "Distinct",
            Op::Reduced { .. } => "Reduced",
            Op::Slice { .. } => "Slice",
            Op::Project { .. } => "Project",
            Op::SqlJoin { .. } => "SqlJoin",
        };
        *ops.entry(name).or_default() += 1;
        for c in p.children() {
            walk(c, ops);
        }
    }
    for cfg in configs {
        let plan = stores.with(cfg.store, |s| {
            sq_plan::plan_query(q, &*s, &options(cfg, union_default, probes)).ok()
        });
        if let Some(plan) = plan {
            walk(&plan.root, ops);
            if !plan.exists.is_empty() {
                *ops.entry("(EXISTS plan)").or_default() += plan.exists.len();
                plan.exists.iter().for_each(|e| walk(e, ops));
            }
        }
    }
}

fn describe(diff: String, expected: &Outcome, got: &Outcome, order: Order) -> String {
    format!("{diff}\n--- expected (oracle):\n{}--- got:\n{}", expected.describe(order, 40), got.describe(order, 40))
}

/// The configuration(s) needed to re-check a culprit.
fn configs_for(c: &Culprit) -> Vec<Config> {
    match c {
        Culprit::Native(cfg) => vec![*cfg],
        Culprit::Stores => Vec::new(),
    }
}

/// Re-runs one culprit; Some(detail) if it still fails.
fn still_fails(stores: &Stores, q: &Query, culprit: &Culprit, max_rows: usize) -> Option<String> {
    match check(stores, q, &configs_for(culprit), max_rows, None) {
        CaseResult::Checked(fs) => fs.into_iter().find(|f| f.culprit == *culprit).map(|f| f.detail),
        CaseResult::Invalid | CaseResult::TooLarge => None,
    }
}

/// Candidate evaluations allowed while shrinking one failure.
const SHRINK_BUDGET: usize = 1500;

/// Greedy shrinking: simplify the query while it still fails, then the data, and repeat.
fn shrink(mut data: Dataset, mut q: Query, culprit: &Culprit, mut detail: String, max_rows: usize) -> (Dataset, Query, String) {
    let mut budget = SHRINK_BUDGET;
    let mut stores = Stores::load(&data);
    loop {
        let mut progress = false;
        for cand in q.simplifications() {
            if budget == 0 {
                return (data, q, detail);
            }
            budget -= 1;
            if let Some(d) = still_fails(&stores, &cand, culprit, max_rows) {
                q = cand;
                detail = d;
                progress = true;
                break;
            }
        }
        if progress {
            continue;
        }
        for cand in data.simplifications() {
            if budget == 0 {
                return (data, q, detail);
            }
            budget -= 1;
            let s = Stores::load(&cand);
            if let Some(d) = still_fails(&s, &q, culprit, max_rows) {
                data = cand;
                stores = s;
                detail = d;
                progress = true;
                break;
            }
        }
        if !progress {
            return (data, q, detail);
        }
    }
}

fn report(case: usize, seed: u64, data: &Dataset, q: &Query, culprit: &Culprit, detail: &str) -> String {
    let mut out = String::new();
    let what = match culprit {
        Culprit::Native(c) => format!("native {:?} policy={:?} reorder={}", c.store, c.policy, c.reorder),
        Culprit::Stores => "oracle on MemStore vs oracle on a SQLite store".to_owned(),
    };
    let _ = writeln!(
        out,
        "=== MISMATCH: {what} (case {case} of seed {seed}; union_default_graph={}; probe counts={}; analyzed={})",
        q.union_default, q.probes, data.analyze
    );
    let _ = writeln!(out, "--- data (TriG):\n{}", data.trig());
    let _ = writeln!(out, "--- query:\n{q}");
    let _ = writeln!(out, "--- difference: {detail}");
    out
}

/// Runs the fuzzer; failures are shrunk and reported in `Totals::reports`.
pub fn run(settings: &Settings) -> Totals {
    let configs = Config::all(&settings.policies);
    let mut totals = Totals { verbose: settings.verbose, ..Totals::default() };
    let mut r = Rng(settings.seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let started = Instant::now();
    let mut case = 0;
    while case < settings.cases {
        let data = Dataset::generate(&mut r);
        let stores = Stores::load(&data);
        for _ in 0..settings.queries_per_dataset {
            if case >= settings.cases {
                break;
            }
            case += 1;
            let mut qr = Rng(r.next_u64() | 1);
            let q = Gen::new(&mut qr, &data).bnode_anywhere(settings.bnode_anywhere).query();
            totals.cases += 1;
            match check(&stores, &q, &configs, settings.max_rows, Some(&mut totals)) {
                CaseResult::Invalid => totals.invalid += 1,
                CaseResult::TooLarge => totals.too_large += 1,
                CaseResult::Checked(failures) => {
                    totals.failures += failures.len();
                    // One report per case: the first failing culprit.
                    if let Some(f) = failures.into_iter().next()
                        && totals.reports.len() < settings.max_reports
                    {
                        let (d, q, detail) = shrink(data.clone(), q.clone(), &f.culprit, f.detail, settings.max_rows);
                        let rep = report(case, settings.seed, &d, &q, &f.culprit, &detail);
                        eprintln!("{rep}");
                        totals.reports.push(rep);
                    }
                }
            }
        }
    }
    eprintln!(
        "differential fuzzer: seed {} - {} cases ({} invalid, {} too large, {} oracle errors, {} empty), {} native comparisons over {} configurations, {} failures, {:.1} s",
        settings.seed,
        totals.cases,
        totals.invalid,
        totals.too_large,
        totals.oracle_errors,
        totals.empty,
        totals.comparisons,
        configs.len(),
        totals.failures,
        started.elapsed().as_secs_f64()
    );
    let census: Vec<String> = totals.operators.iter().map(|(k, v)| format!("{k} {v}")).collect();
    eprintln!("operators in sampled plans: {}", census.join(", "));
    totals
}

/// Re-runs a saved report (the `--- data (TriG):` and `--- query:` sections of a mismatch
/// report) under every configuration; prints each outcome and the plans that disagree.
pub fn replay(report: &str, union_default: bool, probes: bool, analyze: bool) -> String {
    let section = |start: &str| -> String {
        let from = report.find(start).map(|i| i + start.len()).unwrap_or(0);
        let rest = &report[from..];
        let end = rest.find("\n--- ").unwrap_or(rest.len());
        rest[..end].trim().to_owned()
    };
    let trig = section("--- data (TriG):");
    let text = section("--- query:");
    let data = Dataset::from_trig(&trig, analyze);
    let stores = Stores::load(&data);
    let parsed = match sq_plan::parse_query(&text, None) {
        Ok(p) => p,
        Err(e) => return format!("query does not parse: {e}"),
    };
    let order = if text.contains("REDUCED") { Order::Set } else { Order::Bag };
    let oopts = oracle_options(union_default);
    let mut out = String::new();
    for kind in StoreKind::ALL {
        let truth = stores.with(kind, |s| oracle(&*s, &parsed, &oopts));
        let _ = writeln!(out, "=== {kind:?}: oracle\n{}", truth.describe(order, 20));
        for cfg in Config::all(&[JoinPolicy::Auto, JoinPolicy::PreferHash, JoinPolicy::PreferBind, JoinPolicy::PreferMerge, JoinPolicy::NoSql])
            .into_iter()
            .filter(|c| c.store == kind)
        {
            let opts = options(&cfg, union_default, probes);
            let got = stores.with(kind, |s| native(s, &parsed, &opts));
            let show_plan = std::env::var("SQ_REPRO_PLANS").is_ok_and(|v| v == "1");
            match same(&truth, &got, order, Free::Minted) {
                Ok(()) if !show_plan => {
                    let _ = writeln!(out, "--- {:?} reorder={}: ok", cfg.policy, cfg.reorder);
                }
                Ok(()) => {
                    let plan = stores.with(kind, |s| {
                        sq_engine::explain(s, &text, sq_plan::ExplainMode::Plan, &opts).unwrap_or_else(|e| e.to_string())
                    });
                    let _ = writeln!(out, "--- {:?} reorder={}: ok\n{plan}", cfg.policy, cfg.reorder);
                }
                Err(d) => {
                    let plan = stores.with(kind, |s| {
                        sq_engine::explain(s, &text, sq_plan::ExplainMode::Plan, &opts).unwrap_or_else(|e| e.to_string())
                    });
                    let _ = writeln!(out, "--- {:?} reorder={}: MISMATCH {d}\n{}{plan}", cfg.policy, cfg.reorder, got.describe(order, 20));
                }
            }
        }
    }
    out
}
