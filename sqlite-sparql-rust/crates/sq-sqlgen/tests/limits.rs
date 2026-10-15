//! SQLite's compile-time limits: every query the translator accepts must prepare on the system
//! libsqlite3 (3.45.1) and give spareval's answer, however deep or wide it is; queries past
//! the limits are `Error::Unsupported`. Each family below grows one construct until the
//! translator rejects it (or a size cap), and must be accepted while shallow.
mod common;

use common::{Baseline, DataFile, Engine, Expect, compare};

const DATA: &str = r#"
@prefix : <http://ex.org/> .
:a :knows :b ; :rank 1 ; :p :b .
:b :knows :c ; :rank 2 ; :p :c ; :q :a .
:c :knows :a ; :rank 3 ; :p :d .
:d :rank 4 ; :q :c .
:g1 { :a :knows :c . :c :p :a . }
"#;

const PREFIX: &str = "PREFIX : <http://ex.org/> ";

/// A family of queries, `make(n)` growing with n.
struct Family {
    name: &'static str,
    make: fn(usize) -> String,
    /// Sizes to try, in increasing order.
    sizes: &'static [usize],
    /// Sizes up to this one must be accepted.
    accepted_up_to: usize,
}

fn nest(n: usize, open: &str, close: &str, inner: &str) -> String {
    format!("{}{inner}{}", open.repeat(n), close.repeat(n))
}

const DEPTHS: &[usize] = &[1, 2, 3, 4, 5, 6, 8, 10, 12, 16, 24, 32];

const FAMILIES: &[Family] = &[
    Family {
        name: "nested FILTER EXISTS",
        make: |n| format!("SELECT ?s WHERE {{ {} }}", nest(n, "?s :knows ?o FILTER EXISTS { ", "} ", "?s :rank ?r ")),
        sizes: DEPTHS,
        accepted_up_to: 4,
    },
    Family {
        name: "nested FILTER NOT EXISTS",
        make: |n| {
            let mut p = "?o :rank 2".to_owned();
            for i in 0..n {
                p = format!("?o :knows ?x{i} FILTER NOT EXISTS {{ {p} }}");
            }
            format!("SELECT ?o WHERE {{ {p} }}")
        },
        sizes: DEPTHS,
        accepted_up_to: 3,
    },
    Family {
        name: "nested OPTIONAL with filters",
        make: |n| {
            let mut p = "?x :rank ?r FILTER(?r > 1)".to_owned();
            for i in 0..n {
                p = format!("?x :knows ?y{i} OPTIONAL {{ ?y{i} :knows ?z{i} . {p} }}");
            }
            format!("SELECT * WHERE {{ {p} }}")
        },
        sizes: DEPTHS,
        accepted_up_to: 4,
    },
    Family {
        name: "nested sub-selects",
        make: |n| {
            let mut p = "?s :rank ?r".to_owned();
            for _ in 0..n {
                p = format!("{{ SELECT DISTINCT ?s ?r WHERE {{ {p} . ?s :p ?o }} ORDER BY ?r LIMIT 10 }}");
            }
            format!("SELECT ?s ?r WHERE {{ {p} }}")
        },
        sizes: DEPTHS,
        accepted_up_to: 3,
    },
    Family {
        name: "nested UNION",
        make: |n| {
            let mut p = "?s :rank ?r".to_owned();
            for i in 0..n {
                p = format!("{{ {p} }} UNION {{ ?s :p ?u{i} }}");
            }
            format!("SELECT * WHERE {{ ?s :knows ?k OPTIONAL {{ {p} }} }}")
        },
        sizes: DEPTHS,
        accepted_up_to: 6,
    },
    Family {
        name: "nested MINUS",
        make: |n| {
            let mut p = "?s :rank 3".to_owned();
            for _ in 0..n {
                p = format!("?s :knows ?o MINUS {{ {p} }}");
            }
            format!("SELECT * WHERE {{ {p} }}")
        },
        sizes: DEPTHS,
        accepted_up_to: 4,
    },
    Family {
        name: "nested GROUP BY",
        make: |n| {
            let mut p = "?s :rank ?r".to_owned();
            for _ in 0..n {
                p = format!("{{ SELECT ?s (MAX(?r) AS ?r) WHERE {{ {p} }} GROUP BY ?s }}");
            }
            format!("SELECT * WHERE {{ {p} }}")
        },
        sizes: DEPTHS,
        accepted_up_to: 4,
    },
    Family {
        name: "nested IF / COALESCE",
        make: |n| {
            let mut e = "?r".to_owned();
            for _ in 0..n {
                e = format!("IF(?r > 1, COALESCE({e}, 0), ?r + 1)");
            }
            format!("SELECT ?s ?v WHERE {{ ?s :rank ?r BIND({e} AS ?v) }}")
        },
        sizes: DEPTHS,
        accepted_up_to: 3,
    },
    Family {
        name: "left-deep arithmetic",
        make: |n| format!("SELECT ?s ?v WHERE {{ ?s :rank ?r BIND({} AS ?v) }}", format!("?r{}", " + 1".repeat(n))),
        sizes: DEPTHS,
        accepted_up_to: 4,
    },
    Family {
        name: "NOT chains",
        make: |n| format!("SELECT ?s WHERE {{ ?s :rank ?r FILTER({}?r > 1{}) }}", "!(".repeat(n), ")".repeat(n)),
        // spargebra's parser takes time exponential in the depth of `!(…)`.
        sizes: &[1, 2, 4, 6, 8, 10, 12, 14],
        accepted_up_to: 6,
    },
    Family {
        name: "nested closures",
        make: |n| format!("SELECT ?x WHERE {{ :a {} ?x }}", nest(n, "(", ")*", ":p")),
        sizes: DEPTHS,
        accepted_up_to: 2,
    },
    Family {
        name: "wide BGP",
        make: |n| {
            let pats: Vec<String> = (0..n).map(|i| format!("?s :knows ?o{i} .")).collect();
            format!("SELECT (COUNT(*) AS ?c) WHERE {{ {} }}", pats.join(" "))
        },
        sizes: &[10, 64, 65, 130],
        accepted_up_to: 130,
    },
    Family {
        name: "wide join of groups",
        make: |n| {
            let groups: Vec<String> = (0..n).map(|i| format!("{{ ?s :knows ?a{i} . ?a{i} :rank ?r{i} }}")).collect();
            format!("SELECT (COUNT(*) AS ?c) WHERE {{ {} }}", groups.join(" "))
        },
        sizes: &[10, 33, 64],
        accepted_up_to: 64,
    },
    Family {
        name: "wide UNION",
        make: |n| {
            let branches: Vec<String> = (0..n).map(|i| format!("{{ ?s :rank {} }}", i % 5)).collect();
            format!("SELECT ?s WHERE {{ {} }}", branches.join(" UNION "))
        },
        // sq-plan's lowering recursion limits the width.
        sizes: &[2, 500, 501, 700],
        accepted_up_to: 700,
    },
    Family {
        name: "long OR chain",
        make: |n| {
            let terms: Vec<String> = (0..n).map(|i| format!("?r = {i}")).collect();
            format!("SELECT ?s WHERE {{ ?s :rank ?r FILTER({}) }}", terms.join(" || "))
        },
        sizes: &[2, 100, 1100],
        accepted_up_to: 1100,
    },
    Family {
        name: "long IN list",
        make: |n| {
            let item = |i: usize| if i % 2 == 0 { i.to_string() } else { format!("{i}.5") };
            let items: Vec<String> = (0..n).map(item).collect();
            format!("SELECT ?s WHERE {{ ?s :rank ?r FILTER(?r IN ({})) }}", items.join(", "))
        },
        sizes: &[2, 1100],
        accepted_up_to: 1100,
    },
    Family {
        name: "long path sequence",
        // Negated property sets stay path steps (a plain predicate would become a BGP); over
        // this data they only follow the :knows cycle.
        make: |n| format!("SELECT ?x ?y WHERE {{ ?x {} ?y }}", vec!["!(:rank|:p|:q)"; n].join("/")),
        // Each step is one table: past 63 a prefix becomes one opaque relation.
        sizes: &[2, 30, 66],
        accepted_up_to: 66,
    },
    Family {
        name: "long path alternative",
        make: |n| {
            let alts: Vec<&str> = (0..n).map(|i| if i % 2 == 0 { ":p" } else { "^:q" }).collect();
            format!("SELECT ?y WHERE {{ :a ({})+ ?y }}", alts.join("|"))
        },
        sizes: &[2, 600],
        accepted_up_to: 600,
    },
    Family {
        name: "BIND chain, each read twice",
        make: |n| {
            let mut p = "?s :rank ?v0".to_owned();
            for i in 1..=n {
                p.push_str(&format!(" BIND(COALESCE(?v{0}, ?v{0}) AS ?v{i})", i - 1));
            }
            format!("SELECT ?s ?v{n} WHERE {{ {p} FILTER(?v{n} > 1) }} ORDER BY ?v{n}")
        },
        sizes: DEPTHS,
        accepted_up_to: 10,
    },
    Family {
        name: "nested IN, computed operand",
        make: |n| {
            let mut e = "?r".to_owned();
            for _ in 0..n {
                e = format!("(COALESCE({e}, 0) IN (?r, 1, 2.5, \"x\"))");
            }
            format!("SELECT ?s WHERE {{ ?s :rank ?r FILTER({e}) }}")
        },
        sizes: DEPTHS,
        accepted_up_to: 2,
    },
    Family {
        name: "wide CONCAT",
        make: |n| format!("SELECT ?v WHERE {{ :a :rank ?r BIND(CONCAT({}) AS ?v) }}", vec!["STR(?r)"; n].join(", ")),
        sizes: &[2, 126, 127, 200],
        accepted_up_to: 126,
    },
];

#[test]
fn accepted_queries_prepare_and_match_spareval() {
    // Parsing, lowering and spareval recurse over the query's depth.
    let worker = std::thread::Builder::new().stack_size(256 << 20).spawn(run_families).unwrap();
    worker.join().unwrap();
}

fn run_families() {
    let engine = Engine::new(&[DataFile::trig(DATA)]).unwrap();
    let mut failures = Vec::new();
    for f in FAMILIES {
        let mut deepest_accepted = 0;
        for &n in f.sizes {
            let text = format!("{PREFIX}{}", (f.make)(n));
            let query = sq_plan::parse_query(&text, None).unwrap_or_else(|e| panic!("{}({n}): {e}", f.name));
            match engine.baseline(&query) {
                Ok(Baseline::Unsupported(msg)) => {
                    if n <= f.accepted_up_to {
                        failures.push(format!("{}({n}) rejected: {msg}", f.name));
                    }
                }
                Ok(Baseline::Ran { outcome, .. }) => {
                    deepest_accepted = n;
                    let reference = engine.reference(&query).unwrap();
                    if let Err(e) = compare(&outcome, &reference, &Expect::of(&query)) {
                        failures.push(format!("{}({n}) differs from spareval: {}", f.name, first_line(&e)));
                    }
                }
                Err(e) => failures.push(format!("{}({n}) accepted but failed: {}", f.name, first_line(&e))),
            }
        }
        eprintln!("{:28} deepest accepted size: {deepest_accepted}", f.name);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("")
}
