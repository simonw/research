//! Differential fuzzing: seeded random graphs and random queries over the baseline subset,
//! each compared with spareval. The generator avoids the constructs where spareval 0.2.7 is
//! known to deviate from the spec (see tools/sq-w3c/reference_divergences.txt and the
//! `SPEC_ANSWERS` of tests/equivalence.rs): MINUS inside EXISTS, GRAPH ?g over anything but
//! basic graph patterns, zero-length paths from constants absent from the data, NaN, boolean
//! ordering, MIN / MAX / SAMPLE / GROUP_CONCAT. Found by this fuzzer and also avoided:
//! spareval deduplicates a top-level alternative path `?x (:p|:q) ?y` (the spec's
//! Union is a bag), orders IRIs and blank nodes with `<` / `<=` (they have no operator mapping:
//! a type error), lets a FILTER see variables hidden inside a nested sub-select (and does not
//! substitute EXISTS bindings into a FILTER next to a sub-select in the EXISTS body), evaluates a
//! GRAPH nested in `GRAPH <absent>` (an absent graph is not in the dataset: no solutions), and
//! requires the endpoint of a zero-length path that EXISTS substitutes to be a node of the graph
//! (a substituted value is a constant, which always matches itself; docs/SPEC.md §5.5).
mod common;

use std::fmt::Write as _;

use common::{DataFile, Engine, check};

/// xorshift64*.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

const SUBJECTS: &[&str] = &[":s0", ":s1", ":s2", ":s3", ":s4", ":s5"];
const PREDICATES: &[&str] = &[":p", ":q", ":r"];
const LITERALS: &[&str] = &["0", "1", "2", "3", "1.5", "-2", "\"a\"", "\"b\"", "\"a long literal\"", "\"x\"@en"];
const VARS: &[&str] = &["?a", "?b", "?c", "?d"];
/// Variables of sub-select bodies, per nesting level: only ?a is projected, so the others
/// never meet an enclosing query's variables.
const INNER_VARS: &[&[&str]] = &[&["?a", "?e", "?f"], &["?a", "?h", "?i"], &["?a", "?j", "?k"]];

fn data(rng: &mut Rng) -> String {
    let mut out = String::from("@prefix : <http://ex.org/> .\n");
    // Every subject occurs in every graph (path constants are never absent from a graph).
    for s in SUBJECTS {
        let _ = writeln!(out, "{s} :r :s0 .");
        let _ = writeln!(out, ":g1 {{ {s} :r :s1 }} :g2 {{ {s} :r :s2 }}");
    }
    let object = |rng: &mut Rng| {
        if rng.chance(55) { rng.pick(SUBJECTS).to_owned() } else { rng.pick(LITERALS).to_owned() }
    };
    for _ in 0..28 {
        let (s, p, o) = (rng.pick(SUBJECTS), rng.pick(PREDICATES), object(rng));
        let _ = writeln!(out, "{s} {p} {o} .");
    }
    let _ = writeln!(out, "_:b0 :p :s1 . _:b0 :q 2 . :s2 :p _:b0 . _:b1 :q _:b0 .");
    for g in [":g1", ":g2"] {
        let _ = write!(out, "{g} {{");
        for _ in 0..6 {
            let (s, p, o) = (rng.pick(SUBJECTS), rng.pick(PREDICATES), object(rng));
            let _ = write!(out, " {s} {p} {o} .");
        }
        let _ = writeln!(out, " }}");
    }
    out
}

/// Random query text generator.
struct Gen {
    rng: Rng,
    /// Fresh BIND targets.
    next_var: usize,
    /// The variables patterns use (`VARS`, or `INNER_VARS` inside a sub-select).
    vars: &'static [&'static str],
    /// Sub-select nesting depth.
    sub_depth: usize,
    /// Inside GRAPH (no nested GRAPH).
    in_graph: bool,
}

impl Gen {
    fn term(&mut self, position: usize) -> String {
        match position {
            0 if self.rng.chance(25) => self.rng.pick(SUBJECTS).to_owned(),
            1 if self.rng.chance(85) => self.rng.pick(PREDICATES).to_owned(),
            2 if self.rng.chance(25) => {
                if self.rng.chance(50) {
                    self.rng.pick(SUBJECTS).to_owned()
                } else {
                    self.rng.pick(LITERALS).to_owned()
                }
            }
            _ => self.rng.pick(self.vars).to_owned(),
        }
    }

    fn var(&mut self) -> &'static str {
        self.rng.pick(self.vars)
    }

    fn triple(&mut self) -> String {
        format!("{} {} {} .", self.term(0), self.term(1), self.term(2))
    }

    fn bgp(&mut self) -> String {
        let n = 1 + self.rng.below(2);
        (0..n).map(|_| self.triple()).collect::<Vec<_>>().join(" ")
    }

    fn path(&mut self, in_exists: bool) -> String {
        let p = self.rng.pick(&[":p", ":q", "^:p", "(:p|:q)", ":p/:q"]);
        let op = match (in_exists, p) {
            (true, "(:p|:q)") => "+",
            (true, _) => self.rng.pick(&["+", ""]),
            (false, "(:p|:q)") => self.rng.pick(&["+", "*", "?"]),
            (false, _) => self.rng.pick(&["+", "*", "?", ""]),
        };
        let path = if op.is_empty() { p.to_owned() } else { format!("{p}{op}") };
        let end = |g: &mut Gen| if g.rng.chance(30) { g.rng.pick(SUBJECTS).to_owned() } else { g.var().to_owned() };
        let (s, o) = (end(self), end(self));
        format!("{s} {path} {o} .")
    }

    fn operand(&mut self) -> String {
        match self.rng.below(6) {
            0 => self.rng.pick(&["0", "1", "2", "1.5", "\"a\"", ":s1"]).to_owned(),
            1 => format!("({} + 1)", self.var()),
            2 => format!("COALESCE({}, 0)", self.var()),
            _ => self.var().to_owned(),
        }
    }

    fn expr(&mut self, depth: usize) -> String {
        let v = self.var();
        match self.rng.below(if depth == 0 { 9 } else { 13 }) {
            0 => format!("BOUND({v})"),
            1 => format!("{} = {}", self.operand(), self.operand()),
            2 => format!("{} != {}", self.operand(), self.operand()),
            3 => {
                let c = self.rng.pick(&["0", "1", "2", "1.5", "\"a\""]);
                format!("{} {} {c}", self.operand(), self.rng.pick(&["<", ">"]))
            }
            4 => format!("{}({v})", self.rng.pick(&["isIRI", "isLiteral", "isBlank", "isNumeric"])),
            5 => format!("{v} IN ({}, {})", self.operand(), self.operand()),
            6 => format!("REGEX(STR({v}), \"{}\")", self.rng.pick(&["s1", "^a", "1$"])),
            7 => format!("sameTerm({v}, {})", self.var()),
            8 if self.rng.chance(50) => {
                format!("{v} {} {}", self.rng.pick(&["<=", ">="]), self.rng.pick(&["0", "1", "2", "1.5", "\"a\""]))
            }
            8 => format!("{v}"),
            9 => format!("({}) && ({})", self.expr(depth - 1), self.expr(depth - 1)),
            10 => format!("({}) || ({})", self.expr(depth - 1), self.expr(depth - 1)),
            11 => format!("!({})", self.expr(depth - 1)),
            _ => format!("IF({}, true, {})", self.expr(depth - 1), self.expr(depth - 1)),
        }
    }

    /// A group graph pattern body. `in_exists`: no MINUS (substitution vs join semantics).
    fn pattern(&mut self, depth: usize, in_exists: bool) -> String {
        if depth == 0 {
            return if self.rng.chance(15) { self.path(in_exists) } else { self.bgp() };
        }
        let d = depth - 1;
        match self.rng.below(12) {
            0 | 1 => format!("{} {}", self.pattern(d, in_exists), self.pattern(d, in_exists)),
            2 | 3 => {
                let filter = if self.rng.chance(40) { format!(" FILTER({})", self.expr(1)) } else { String::new() };
                format!("{} OPTIONAL {{ {}{filter} }}", self.pattern(d, in_exists), self.pattern(d, in_exists))
            }
            4 => format!("{{ {} }} UNION {{ {} }}", self.pattern(d, in_exists), self.pattern(d, in_exists)),
            5 if !in_exists => format!("{} MINUS {{ {} }}", self.pattern(d, in_exists), self.pattern(d, in_exists)),
            5 | 6 => format!("{} FILTER({})", self.pattern(d, in_exists), self.expr(2)),
            7 => {
                let not = if self.rng.chance(50) { "NOT " } else { "" };
                format!("{} FILTER {not}EXISTS {{ {} }}", self.pattern(d, in_exists), self.pattern(d, true))
            }
            8 => {
                self.next_var += 1;
                let n = self.next_var;
                let e = match self.rng.below(3) {
                    0 => format!("{} + 1", self.var()),
                    1 => format!("STR({})", self.var()),
                    _ => format!("COALESCE({}, {})", self.var(), self.var()),
                };
                format!("{} BIND({e} AS ?x{n})", self.pattern(d, in_exists))
            }
            9 => {
                let v = self.var();
                let rows: Vec<&str> = (0..1 + self.rng.below(3))
                    .map(|_| self.rng.pick(&[":s0", ":s1", ":s2", "1", "\"a\"", "UNDEF"]))
                    .collect();
                format!("VALUES {v} {{ {} }} {}", rows.join(" "), self.pattern(d, in_exists))
            }
            10 if !self.in_graph => {
                if self.rng.chance(50) {
                    let g = self.rng.pick(&[":g1", ":g2", ":g3"]);
                    self.in_graph = true;
                    let body = self.pattern(d, in_exists);
                    self.in_graph = false;
                    format!("GRAPH {g} {{ {body} }}")
                } else {
                    format!("GRAPH ?g {{ {} }}", self.bgp())
                }
            }
            10 => self.pattern(d, in_exists),
            _ if in_exists => self.pattern(d, in_exists),
            _ => {
                let distinct = if self.rng.chance(40) { "DISTINCT " } else { "" };
                let level = INNER_VARS[self.sub_depth.min(INNER_VARS.len() - 1)];
                let outer = std::mem::replace(&mut self.vars, level);
                self.sub_depth += 1;
                let body = self.pattern(d, in_exists);
                self.sub_depth -= 1;
                self.vars = outer;
                format!("{{ SELECT {distinct}?a WHERE {{ {body} }} }}")
            }
        }
    }

    fn query(&mut self) -> String {
        let depth = 1 + self.rng.below(3);
        let body = self.pattern(depth, false);
        let prefix = "PREFIX : <http://ex.org/> ";
        match self.rng.below(10) {
            0 => format!("{prefix}ASK {{ {body} }}"),
            1 => {
                let v = self.rng.pick(VARS);
                let w = self.rng.pick(VARS);
                let agg = self.rng.pick(&["COUNT(*)", "COUNT(?b)", "COUNT(DISTINCT ?c)", "SUM(?d)", "AVG(?b)"]);
                format!("{prefix}SELECT {v} ({agg} AS ?n) WHERE {{ {body} {w} ?anyp ?anyo }} GROUP BY {v}")
            }
            2 => format!("{IMPLICIT_GROUP}{body} }}"),
            3 | 4 => format!("{prefix}SELECT DISTINCT * WHERE {{ {body} }}"),
            _ => format!("{prefix}SELECT * WHERE {{ {body} }}"),
        }
    }
}

const IMPLICIT_GROUP: &str = "PREFIX : <http://ex.org/> SELECT (COUNT(*) AS ?n) WHERE { ";

/// spareval's ASK over a group body (`… }` as left by stripping `IMPLICIT_GROUP`).
fn reference_ask(engine: &Engine, body: &str) -> bool {
    let ask = format!("PREFIX : <http://ex.org/> ASK {{ {body}");
    let query = sq_plan::parse_query(&ask, None).expect("the SELECT parsed");
    matches!(engine.reference(&query), Ok(common::Outcome::Boolean(true)))
}

/// The baseline's rows (None when unsupported).
fn baseline_rows(engine: &Engine, q: &str) -> Option<Vec<Vec<Option<oxrdf::Term>>>> {
    let query = sq_plan::parse_query(q, None).expect("parsed before");
    match engine.baseline(&query).expect("baseline runs") {
        common::Baseline::Ran { outcome: common::Outcome::Solutions { rows, .. }, .. } => Some(rows),
        _ => None,
    }
}

fn run(seed: u64, queries: usize) -> (usize, Vec<String>) {
    let mut rng = Rng(seed);
    let engine = Engine::new(&[DataFile::trig(&data(&mut rng))]).unwrap();
    let mut generator = Gen { rng, next_var: 0, vars: VARS, sub_depth: 0, in_graph: false };
    let mut supported = 0;
    let mut failures = Vec::new();
    for i in 0..queries {
        let q = generator.query();
        // spargebra rejects some random texts (e.g. BIND of an in-scope variable).
        if sq_plan::parse_query(&q, None).is_err() {
            continue;
        }
        // spareval drops the single row of an implicit group when a FILTER folds to false;
        // the spec's answer is one row with COUNT 0.
        if let Some(body) = q.strip_prefix(IMPLICIT_GROUP)
            && !reference_ask(&engine, body)
        {
            match baseline_rows(&engine, &q) {
                Some(rows) if rows == [vec![Some(oxrdf::Literal::from(0).into())]] => supported += 1,
                Some(rows) => failures.push(format!("seed {seed} query {i}: {q}\n  expected one row with 0: {rows:?}")),
                None => {}
            }
            continue;
        }
        match check(&engine, &q, None) {
            Ok(Some(_)) => supported += 1,
            Ok(None) => {}
            Err(e) => failures.push(format!("seed {seed} query {i}: {q}\n  {e}")),
        }
    }
    (supported, failures)
}

#[test]
fn random_queries_match_spareval() {
    let mut total = 0;
    let mut failures = Vec::new();
    // `SQ_SQLGEN_FUZZ_SEEDS=n` widens the sweep (exploratory runs); the default is 4 seeds.
    let seeds: u64 = std::env::var("SQ_SQLGEN_FUZZ_SEEDS").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
    for seed in (1..=seeds).map(|i| 0x5eed_0000 + i * 0x1001) {
        let (n, f) = run(seed, 600);
        total += n;
        failures.extend(f);
    }
    eprintln!("fuzz: {total} supported random queries compared with spareval");
    assert!(total as u64 > 375 * seeds, "the generator should mostly produce supported queries");
    let first: Vec<&str> = failures.iter().take(10).map(String::as_str).collect();
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), first.join("\n"));
}
