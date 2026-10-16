//! Adversarial review tests for WP5 (sq-plan). Every `review_*` test pins a defect found in
//! review (all fixed; they now guard against regressions).
mod common;

use common::*;
use sq_core::batch::SlotSet;
use sq_core::plan::{JoinKind, JoinPolicy, Op, Plan, QueryOptions, QueryPlan};
use sq_memstore::MemStore;

/// lower + optimize + validate, returning the first error as text (optimize itself validates
/// in debug builds, which is how the engine runs it in every test).
fn plan_result(q: &str, st: &MemStore, opts: &QueryOptions) -> Result<QueryPlan, String> {
    let parsed = sq_plan::parse_query(&format!("{PREFIXES}{q}"), None).map_err(|e| e.to_string())?;
    let lowered = sq_plan::lower_query(&parsed, st, opts).map_err(|e| e.to_string())?;
    let p = sq_plan::optimize(lowered, st, opts).map_err(|e| e.to_string())?;
    sq_plan::validate(&p).map_err(|e| format!("{e}\n{}", sq_plan::explain(&p, None)))?;
    Ok(p)
}

fn policies() -> Vec<(&'static str, QueryOptions)> {
    vec![
        ("auto", QueryOptions::default()),
        ("prefer-bind", QueryOptions { join_policy: JoinPolicy::PreferBind, ..Default::default() }),
        ("prefer-hash", QueryOptions { join_policy: JoinPolicy::PreferHash, ..Default::default() }),
        ("no-reorder", QueryOptions { reorder: false, ..Default::default() }),
    ]
}

// ------------------------------------------------------------------------------------------
// Finding 1: inside a group opened with a Join-mode seed (BindJoin right side), an inner
// HashJoin / NestedLoop step whose right unit does not mention the seeded slot fails
// validate(): check_keys() counts the outer seed as "maybe bound on both sides" but the
// planner's `check` only covers left.maybe ∩ unit.maybe. With default options a plain
// OPTIONAL containing a cross product cannot be planned in debug builds.
// ------------------------------------------------------------------------------------------
#[test]
fn review_seeded_group_cross_product_validates() {
    let st = social(200);
    for q in [
        "SELECT * { ?x :name \"Person 5\" OPTIONAL { ?x :knows ?y . ?z :city :city1 } }",
        "SELECT * { ?x :age ?x . OPTIONAL { ?x :name ?n . { ?v :city ?c } } }",
    ] {
        if let Err(e) = plan_result(q, &st, &QueryOptions::default()) {
            panic!("default options: {q}\n{e}");
        }
    }
}

// ------------------------------------------------------------------------------------------
// Finding 2: group filter conjuncts are applied as soon as their evars are in
// `shape.certain`, which includes slots that are only bound by the outer Join-mode seed. The
// Filter then sits below the unit that binds the slot, its physical props make the seeded slot
// unsafe, and validate() rejects the enclosing BindJoin ("seed not in safe_inject(right)").
// ------------------------------------------------------------------------------------------
#[test]
fn review_group_filter_on_seeded_slot_validates() {
    let st = social(200);
    for q in [
        "SELECT * { ?v :name \"Person 5\" . { ?a :name \"Person 1\" . ?a :knows ?v FILTER(?a != ?v) } }",
        "SELECT * { ?v :name \"Person 5\" OPTIONAL { ?a :name \"Person 1\" . ?a :knows ?v FILTER(?a != ?v) } }",
    ] {
        if let Err(e) = plan_result(q, &st, &QueryOptions::default()) {
            panic!("default options: {q}\n{e}");
        }
    }
}

// ------------------------------------------------------------------------------------------
// Findings 1+2 at scale: deterministic random queries (BGPs, OPTIONAL, UNION, MINUS, nested
// groups, LIMIT sub-selects, [NOT] EXISTS, GRAPH, VALUES, BIND, aggregates, paths, filters)
// must plan and validate under every join policy. In review ~7% (auto) / ~11% (prefer-bind)
// of these queries fail with Error::Internal("invalid plan ...").
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
}

fn random_triple(r: &mut Rng) -> String {
    let vars = ["?a", "?b", "?c", "?v", "?x"];
    let preds = [":knows", ":age", ":city", ":name", "a", ":knows+", ":knows*", "^:knows", ":knows/:city", "?p"];
    let p = r.pick(&preds);
    let s = r.pick(&vars);
    let o = match p {
        "a" => r.pick(&[":C1", ":C2", "?t"]),
        ":name" => r.pick(&["\"Person 1\"", "?n"]),
        ":city" => r.pick(&[":city1", "?c"]),
        _ => r.pick(&vars),
    };
    format!("{s} {p} {o} . ")
}

fn random_group(r: &mut Rng, depth: u32) -> String {
    let mut out = String::new();
    for _ in 0..1 + r.next() % 3 {
        out.push_str(&random_triple(r));
    }
    if depth > 0 {
        let d = depth - 1;
        let part = match r.next() % 14 {
            0 => format!("OPTIONAL {{ {} }} ", random_group(r, d)),
            1 => format!("{{ {} }} UNION {{ {} }} ", random_group(r, d), random_group(r, d)),
            2 => format!("{{ {} }} ", random_group(r, d)),
            3 => format!("MINUS {{ {} }} ", random_group(r, d)),
            4 => format!("{{ SELECT * {{ {} }} LIMIT 3 }} ", random_group(r, d)),
            5 => format!("FILTER EXISTS {{ {} }} ", random_group(r, d)),
            6 => format!("FILTER NOT EXISTS {{ {} }} ", random_group(r, d)),
            7 => format!("GRAPH ?g {{ {} }} ", random_group(r, d)),
            8 => format!("GRAPH :g1 {{ {} }} ", random_group(r, d)),
            9 => "VALUES ?a { :p1 :p2 UNDEF } ".to_owned(),
            10 => format!("BIND(?{} AS ?w) ", r.pick(&["a", "b", "x"])),
            11 => format!("{{ SELECT ?a (COUNT(*) AS ?cnt) {{ {} }} GROUP BY ?a }} ", random_group(r, d)),
            12 => format!("{{ SELECT ?a ?b {{ {} }} ORDER BY ?b LIMIT 2 }} ", random_group(r, d)),
            _ => format!("FILTER(?{} = :p1) ", r.pick(&["a", "b", "x", "v"])),
        };
        out.push_str(&part);
    }
    if r.next() % 3 == 0 {
        let a = r.pick(&["?a", "?b", "?v", "?x"]);
        let b = r.pick(&["?a", "?b", "?v", "?x", "1"]);
        out.push_str(&format!("FILTER({a} != {b}) "));
    }
    out
}

#[test]
fn review_random_queries_plan_and_validate() {
    let st = social(120);
    load_ttl(&st, "@prefix : <http://ex/> .\n:p1 :knows :p2 . :p2 :city :city1 .", Some("http://ex/g1"));
    load_ttl(&st, "@prefix : <http://ex/> .\n:p3 :knows :p1 .", Some("http://ex/g2"));
    let mut r = Rng(0x1234_5678_9abc_def1);
    let policies = policies();
    let (mut planned, mut failures) = (0usize, Vec::new());
    for _ in 0..400 {
        let q = format!("SELECT * {{ {}}}", random_group(&mut r, 2));
        if sq_plan::parse_query(&format!("{PREFIXES}{q}"), None).is_err() {
            continue;
        }
        for (name, opts) in &policies {
            planned += 1;
            if let Err(e) = plan_result(&q, &st, opts) {
                failures.push(format!("[{name}] {q}\n    {}", e.lines().next().unwrap_or("")));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {planned} (query, policy) pairs fail to plan/validate; first ones:\n{}",
        failures.len(),
        failures.iter().take(6).cloned().collect::<Vec<_>>().join("\n")
    );
}

// ------------------------------------------------------------------------------------------
// Finding 3: the planner (and validate) treat every slot of a Join-mode seed as bound in the
// output rows of ANY operator opened with it. The documented seed contract says Slice, TopK,
// Count, GraphLoop and GraphNames are never seeded, and Op::Project "resets all others to
// UNBOUND". A BindJoin / HashJoin inside a seeded group therefore reads the seeded slot from
// left rows that never carry it (probe with UNBOUND -> no match -> wrong results) unless sq-exec
// re-merges seeds after those operators, which nothing documents.
// ------------------------------------------------------------------------------------------

/// Seeded slots of `s` that survive into the output rows of `p`, reading the documented
/// contract literally.
fn carried(p: &Plan, s: SlotSet) -> SlotSet {
    let inner_join = |left: &Plan, right: &Plan, rs: SlotSet, kind: &JoinKind| {
        let l = carried(left, s);
        if *kind == JoinKind::Inner { l.union(carried(right, rs)) } else { l }
    };
    match &p.op {
        Op::Unit | Op::Empty | Op::Scan(_) | Op::Values { .. } | Op::Path { .. } => s,
        Op::Slice { .. } | Op::TopK { .. } | Op::Count { .. } | Op::GraphLoop { .. } | Op::GraphNames { .. } | Op::SqlJoin { .. } => {
            SlotSet::EMPTY
        }
        Op::Filter { input, .. }
        | Op::Extend { input, .. }
        | Op::Distinct { input, .. }
        | Op::Reduced { input, .. }
        | Op::Sort { input, .. }
        | Op::SortBySlot { input, .. } => carried(input, s),
        Op::Project { input, slots } => carried(input, s).intersect(SlotSet::from_slots(slots.iter().copied())),
        Op::Group { input, keys, .. } => carried(input, s).intersect(SlotSet::from_slots(keys.iter().copied())),
        Op::Union(bs) => bs.iter().fold(s, |acc, b| acc.intersect(carried(b, s))),
        Op::BindJoin { left, right, seed, kind, .. } => {
            inner_join(left, right, SlotSet::from_slots(seed.iter().copied()), kind)
        }
        Op::HashJoin { build, probe, kind, build_is_left, .. } => {
            let (l, r) = if *build_is_left { (build, probe) } else { (probe, build) };
            inner_join(l, r, s, kind)
        }
        Op::NestedLoop { left, right, kind, .. } => inner_join(left, right, s, kind),
        Op::MergeJoin { left, right, .. } => carried(left, s).union(carried(right, s)),
        Op::StarMerge { inputs, .. } => inputs.iter().fold(SlotSet::EMPTY, |acc, i| acc.union(carried(i, s))),
        Op::Minus { left, .. } => carried(left, s),
    }
}

/// Join decisions that read a slot from rows which only "have" it through a seed that never
/// reaches them.
fn seed_reads(p: &Plan, s: SlotSet, exists: &[Plan], out: &mut Vec<String>) {
    let bound = |x: &Plan, xs: SlotSet| sq_plan::plan_props(x, exists).certain.union(carried(x, xs));
    match &p.op {
        Op::BindJoin { left, right, seed, .. } => {
            let seed_set = SlotSet::from_slots(seed.iter().copied());
            if !seed_set.is_subset(&bound(left, s)) {
                out.push(format!("#{}: BindJoin seed {seed:?} not bound in its left rows", p.id));
            }
            seed_reads(left, s, exists, out);
            seed_reads(right, seed_set, exists, out);
        }
        Op::HashJoin { build, probe, keys, kind, build_is_left, .. } => {
            let (l, r) = if *build_is_left { (build, probe) } else { (probe, build) };
            let rs = if *kind == JoinKind::Inner { s } else { SlotSet::EMPTY };
            let both = bound(l, s).intersect(bound(r, rs));
            if !SlotSet::from_slots(keys.iter().copied()).is_subset(&both) {
                out.push(format!("#{}: HashJoin keys {keys:?} not bound on both sides", p.id));
            }
            seed_reads(l, s, exists, out);
            seed_reads(r, rs, exists, out);
        }
        Op::NestedLoop { left, right, kind, .. } => {
            seed_reads(left, s, exists, out);
            seed_reads(right, if *kind == JoinKind::Inner { s } else { SlotSet::EMPTY }, exists, out);
        }
        Op::Minus { left, right, .. } => {
            seed_reads(left, s, exists, out);
            seed_reads(right, SlotSet::EMPTY, exists, out);
        }
        Op::Slice { input, .. } | Op::TopK { input, .. } | Op::Count { input, .. } | Op::GraphLoop { input, .. } => {
            seed_reads(input, SlotSet::EMPTY, exists, out)
        }
        _ => p.children().into_iter().for_each(|c| seed_reads(c, s, exists, out)),
    }
}

#[test]
fn review_join_reads_seed_from_never_seeded_left_input() {
    let st = social(200);
    let mut problems = Vec::new();
    for q in [
        // left = Slice (never seeded)
        "SELECT * { ?v :name \"Person 5\" OPTIONAL { { SELECT ?a { ?a :name \"Person 1\" } LIMIT 1 } ?a :knows ?v } }",
        // left = GraphNames (never seeded)
        "SELECT * { ?v :name \"Person 5\" OPTIONAL { GRAPH ?a { } ?a :knows ?v } }",
        // left = Project [?a] (resets ?v)
        "SELECT * { ?v :name \"Person 5\" OPTIONAL { { SELECT ?a { ?a :name \"Person 1\" } } ?a :knows ?v } }",
    ] {
        let p = plan_result(q, &st, &QueryOptions::default()).unwrap_or_else(|e| panic!("{q}\n{e}"));
        let mut out = Vec::new();
        seed_reads(&p.root, SlotSet::EMPTY, &p.exists, &mut out);
        if !out.is_empty() {
            problems.push(format!("{q}\n  {}\n{}", out.join("\n  "), sq_plan::explain(&p, None)));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

// ------------------------------------------------------------------------------------------
// Finding 4: EXISTS decorrelation (spec §7.3.4 conditions) is not equivalent to the correlated
// evaluation when Q contains a MINUS whose right side mentions a slot of S. The correlated
// EXISTS runs in SeedMode::Substitute, where Minus excludes substituted slots from its domain
// test (sq_core::plan::SeedMode docs), so for ?x = :a with `:a :knows :b . :a :age 3`
// `EXISTS { ?x :knows ?y MINUS { ?x :age ?a } }` is TRUE; the Semi join evaluates Q
// independently (MINUS shares ?x and removes :a) and yields FALSE. Results then depend on the
// join policy / cost model. (pyoxigraph and rdflib both answer like the Semi join, so the lead
// has to pick one semantics; until then the planner must not decorrelate such patterns.)
// ------------------------------------------------------------------------------------------
#[test]
fn review_exists_with_minus_on_correlated_slot_is_not_decorrelated() {
    let st = social(40);
    let q = "SELECT * { ?x a :C1 FILTER EXISTS { ?x :knows ?y MINUS { ?x :age ?a } } }";
    let mut decorrelated = Vec::new();
    for (name, opts) in policies() {
        let p = plan_result(q, &st, &opts).unwrap_or_else(|e| panic!("{name}: {e}"));
        let semi = count_ops(&p, |o| {
            matches!(
                o,
                Op::HashJoin { kind: JoinKind::Semi | JoinKind::Anti, .. }
                    | Op::BindJoin { kind: JoinKind::Semi | JoinKind::Anti, .. }
                    | Op::NestedLoop { kind: JoinKind::Semi | JoinKind::Anti, .. }
            )
        });
        if semi > 0 {
            decorrelated.push(format!("[{name}]\n{}", sq_plan::explain(&p, None)));
        }
    }
    assert!(decorrelated.is_empty(), "decorrelated although Q's MINUS shares ?x:\n{}", decorrelated.join("\n"));
}
