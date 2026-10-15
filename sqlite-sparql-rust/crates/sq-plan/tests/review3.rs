//! Scratch (review round 3).
mod common;

use common::*;
use sq_core::plan::QueryOptions;

fn show(q: &str) {
    let st = social(50);
    let parsed = sq_plan::parse_query(&format!("{PREFIXES}{q}"), None).expect("parse");
    let lowered = sq_plan::lower_query(&parsed, &st, &QueryOptions::default()).expect("lower");
    println!("=== {q}\n{}", sq_plan::explain_logical(&lowered));
    match sq_plan::optimize(lowered, &st, &QueryOptions::default()) {
        Ok(p) => println!("{}", sq_plan::explain(&p, None)),
        Err(e) => println!("ERR {e}"),
    }
}

#[test]
fn scratch_show() {
    for q in [
        "SELECT * { ?x :name ?n OPTIONAL { ?x :age ?a } FILTER(!bound(?a)) }",
        "SELECT * { ?x :name ?n FILTER(?n = \"Person 1\") }",
        "SELECT * { GRAPH ?g { ?s :p ?o } FILTER(?g = :g1) }",
        "SELECT ?x (COUNT(*) AS ?c) { ?x :knows ?y } GROUP BY ?x HAVING(?x = :p1)",
        "SELECT * { ?x :name ?n { SELECT ?x { ?x :age ?a } LIMIT 3 } FILTER(?x = :p1) }",
        "ASK { ?x :name ?n }",
        "CONSTRUCT { ?x :q [ :r ?n ] } WHERE { ?x :name ?n }",
        "DESCRIBE :p1 ?x WHERE { ?x :name ?n }",
    ] {
        show(q);
    }
}

fn time_plan(q: &str) -> (std::time::Duration, std::time::Duration) {
    let st = social(50);
    let t0 = std::time::Instant::now();
    let parsed = sq_plan::parse_query(&format!("{PREFIXES}{q}"), None).expect("parse");
    let t1 = std::time::Instant::now();
    let lowered = sq_plan::lower_query(&parsed, &st, &QueryOptions::default()).expect("lower");
    let p = sq_plan::optimize(lowered, &st, &QueryOptions::default()).expect("plan");
    let t2 = std::time::Instant::now();
    std::hint::black_box(p);
    (t1 - t0, t2 - t1)
}

#[test]
fn scratch_perf() {
    for n in [50, 100, 200, 250] {
        let mut q = String::from("SELECT * { ?x :name ?n0 ");
        for i in 0..n {
            q.push_str(&format!("OPTIONAL {{ ?x :age ?a{i} }} "));
        }
        q.push('}');
        let (p, t) = time_plan(&q);
        println!("optional chain {n}: parse {p:?} plan {t:?}");
    }
    for n in [50, 100, 200] {
        let mut q = String::from("SELECT * { ?x :name ?n0 ");
        for i in 0..n {
            q.push_str(&format!("MINUS {{ ?x :age ?a{i} }} "));
        }
        q.push('}');
        let (p, t) = time_plan(&q);
        println!("minus chain {n}: parse {p:?} plan {t:?}");
    }
    for n in [50, 100, 200] {
        let mut q = String::from("SELECT * { ?x :name ?n0 ");
        for i in 0..n {
            q.push_str(&format!("FILTER EXISTS {{ ?x :age ?a{i} }} "));
        }
        q.push('}');
        let (p, t) = time_plan(&q);
        println!("exists {n}: parse {p:?} plan {t:?}");
    }
}

#[test]
fn scratch_perf_nested() {
    for d in [5, 10, 20, 40] {
        // nested groups each with an OPTIONAL and a FILTER (so the group stays opaque)
        let mut q = String::from("SELECT * { ");
        for i in 0..d {
            q.push_str(&format!("?v{i} :knows ?v{} . OPTIONAL {{ ?v{i} :age ?a{i} }} {{ ", i + 1));
        }
        for i in (0..d).rev() {
            q.push_str(&format!("FILTER(?v{i} != ?v{}) }} ", i + 1));
        }
        q.push('}');
        let (p, t) = time_plan(&q);
        println!("nested groups {d}: parse {p:?} plan {t:?}");
    }
    for d in [5, 10, 20, 40] {
        let mut q = String::from("SELECT * { ");
        for i in 0..d {
            q.push_str(&format!("?v{i} :knows ?v{} . {{ ?v{} :city ?c{i} }} UNION {{ ", i + 1, i + 1));
        }
        q.push_str("?z :name ?n ");
        for _ in 0..d {
            q.push_str("} ");
        }
        q.push('}');
        let (p, t) = time_plan(&q);
        println!("nested unions {d}: parse {p:?} plan {t:?}");
    }
    for d in [5, 10, 20, 40] {
        let mut q = String::from("SELECT * { ");
        for i in 0..d {
            q.push_str(&format!("?v{i} :knows ?v{} . OPTIONAL {{ ", i + 1));
        }
        q.push_str("?z :name ?n ");
        for _ in 0..d {
            q.push_str("} ");
        }
        q.push('}');
        let (p, t) = time_plan(&q);
        println!("nested optionals {d}: parse {p:?} plan {t:?}");
    }
    for d in [5, 10, 20, 40] {
        let mut q = String::from("SELECT * { ");
        for i in 0..d {
            q.push_str(&format!("?v{i} :knows ?v{} . FILTER EXISTS {{ ?v{} :age ?a{i} . ", i + 1, i + 1));
        }
        q.push_str("?z :name ?n ");
        for _ in 0..d {
            q.push_str("} ");
        }
        q.push('}');
        let (p, t) = time_plan(&q);
        println!("nested exists {d}: parse {p:?} plan {t:?}");
    }
}

#[test]
fn scratch_exotic() {
    use sq_core::plan::JoinPolicy;
    let st = social(60);
    load_ttl(&st, "@prefix : <http://ex/> .\n:p1 :knows :p2 . :p2 :city :city1 . :g1 :age 3 .", Some("http://ex/g1"));
    let qs = [
        "SELECT * { ?s ?p ?o } VALUES () { () () }",
        "SELECT * { ?s ?p ?o VALUES () { } }",
        "SELECT * { }",
        "SELECT (1 AS ?x) {}",
        "SELECT * { ?s ?p ?o FILTER(?unknown) }",
        "SELECT * { ?s (:knows|^:knows)* ?o }",
        "SELECT * { ?s !(:knows|^:age) ?o }",
        "SELECT * { :p1 :knows* :p1 }",
        "SELECT * { ?s :knows* ?s }",
        "SELECT * { GRAPH ?g { ?g :age ?o } }",
        "SELECT * { GRAPH ?g { ?s :knows ?o } GRAPH ?g { ?o :city ?c } }",
        "SELECT * { ?g :age ?a GRAPH ?g { ?s :knows ?o } }",
        "SELECT * { ?g :age ?a GRAPH ?g { } }",
        "SELECT * { ?s :knows ?o FILTER(?s = ?o) }",
        "SELECT * { ?s :knows ?o FILTER(sameTerm(?s, ?o)) }",
        "SELECT * { ?s :knows ?s }",
        "SELECT * { ?s ?s ?s }",
        "SELECT * { GRAPH ?g { ?g ?g ?g } }",
        "SELECT * { ?s :knows ?o OPTIONAL { ?o :knows ?s } FILTER(!bound(?s)) }",
        "SELECT * { OPTIONAL { ?s :knows ?o } }",
        "SELECT * { OPTIONAL { ?s :knows ?o } OPTIONAL { ?o :knows ?z } }",
        "SELECT * { { } UNION { ?s :knows ?o } }",
        "SELECT * { ?s :knows ?o MINUS { } }",
        "SELECT * { MINUS { ?s :knows ?o } }",
        "SELECT * { ?s :knows ?o FILTER NOT EXISTS { } }",
        "SELECT * { ?s :knows ?o FILTER EXISTS { ?s :knows* ?z } }",
        "SELECT * { ?s :knows ?o FILTER EXISTS { GRAPH ?s { } } }",
        "SELECT * { GRAPH ?g { ?s :knows ?o FILTER EXISTS { ?o :knows ?s } } }",
        "SELECT * { GRAPH :g1 { ?s :knows ?o FILTER EXISTS { ?o :city ?c } } }",
        "SELECT * { GRAPH :nope { } }",
        "SELECT (COUNT(*) AS ?c) { GRAPH ?g { ?s ?p ?o } }",
        "SELECT ?g (COUNT(*) AS ?c) { GRAPH ?g { ?s ?p ?o } } GROUP BY ?g",
        "SELECT * { GRAPH ?g { SELECT (COUNT(*) AS ?c) { ?s ?p ?o } } }",
        "SELECT * { ?x :knows ?y { SELECT ?y (COUNT(*) AS ?c) { ?y :knows ?z } GROUP BY ?y } }",
        "SELECT * { ?x :knows ?y { SELECT * { ?y :knows ?z } ORDER BY ?z LIMIT 1 OFFSET 2 } }",
        "SELECT DISTINCT ?x { ?x :knows ?y } ORDER BY ?x LIMIT 3",
        "SELECT REDUCED ?x { ?x :knows ?y } ORDER BY ?x LIMIT 3 OFFSET 1",
        "SELECT ?x { ?x :knows ?y } ORDER BY ?y LIMIT 0",
        "SELECT ?x { ?x :knows ?y } LIMIT 0",
        "SELECT (COUNT(DISTINCT *) AS ?c) { ?x :knows [] }",
        "SELECT (SUM(?a) AS ?s) (COUNT(*) AS ?c) { ?x :age ?a }",
        "SELECT ?x (GROUP_CONCAT(?n; separator='|') AS ?g) { ?x :name ?n } GROUP BY ?x HAVING (COUNT(*) > 1)",
        "SELECT * { ?x :age ?a BIND(?a + 1 AS ?b) FILTER(?b > 3) ?y :age ?b }",
        "SELECT * { BIND(:p1 AS ?x) ?x :knows ?y }",
        "SELECT * { ?x :knows ?y BIND(EXISTS { ?y :knows ?x } AS ?e) }",
        "SELECT * { ?x :knows ?y } ORDER BY (EXISTS { ?y :knows ?x }) LIMIT 2",
        "SELECT * { SERVICE SILENT <http://x> { ?s ?p ?o } }",
        "SELECT * { ?x :knows ?y SERVICE SILENT <http://x> { ?y ?p ?o } }",
        "SELECT * { VALUES (?x ?y) { (:p1 UNDEF) (UNDEF :p2) } ?x :knows ?y }",
        "SELECT * { ?x :knows ?y VALUES (?x ?y) { (:p1 UNDEF) (UNDEF :p2) } }",
        "SELECT * { ?x :knows ?y OPTIONAL { VALUES ?y { :p1 } } }",
        "SELECT * { ?x :knows ?y MINUS { VALUES ?y { :p1 } } }",
        "SELECT * { ?a :knows ?b . ?b :knows ?c . ?c :knows ?d . ?d :knows ?e . ?e :knows ?f . ?f :knows ?g . ?g :knows ?h . ?h :knows ?i . ?i :knows ?j . ?j :knows ?k . ?k :knows ?l . ?l :knows ?m . ?m :knows ?n . ?n :knows ?a }",
        "ASK { ?x :knows ?y FILTER(?x = ?y) }",
        "DESCRIBE ?x { ?x :knows :p1 }",
        "DESCRIBE <http://ex/p1>",
        "CONSTRUCT WHERE { ?x :knows ?y }",
        "SELECT * FROM <http://ex/g1> FROM NAMED <http://ex/g1> { ?s ?p ?o GRAPH ?g { ?a ?b ?c } }",
        "SELECT * FROM NAMED <http://ex/absent> { GRAPH ?g { } }",
        "SELECT * FROM NAMED <http://ex/absent> { GRAPH <http://ex/absent> { } }",
        "SELECT * FROM <http://ex/absent> { ?s ?p ?o }",
        "SELECT * { ?s :knows ?o FILTER(?o IN (:p1, :p2)) }",
        "SELECT * { ?s :knows ?o FILTER(?o = :p1 || ?o = :p2) }",
        "SELECT * { ?s :knows ?o FILTER(?o = :p1 && ?s = :p2) }",
        "SELECT * { ?s :knows ?o FILTER(?o = :p1) FILTER(?o = :p2) }",
        "SELECT * { ?s :name ?o FILTER(?o = 'Person 1') }",
        "SELECT * { ?s :name ?o FILTER('Person 1' = ?o) }",
        "SELECT * { ?s :name ?o FILTER(?o = 'Person 1'@en) }",
    ];
    let mut fails = Vec::new();
    for q in qs {
        for opts in [
            QueryOptions::default(),
            QueryOptions { join_policy: JoinPolicy::PreferBind, ..Default::default() },
            QueryOptions { join_policy: JoinPolicy::PreferHash, ..Default::default() },
            QueryOptions { reorder: false, ..Default::default() },
            QueryOptions { union_default_graph: true, ..Default::default() },
        ] {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let parsed = sq_plan::parse_query(&format!("{PREFIXES}{q}"), None).map_err(|e| e.to_string())?;
                let lowered = sq_plan::lower_query(&parsed, &st, &opts).map_err(|e| e.to_string())?;
                let p = sq_plan::optimize(lowered, &st, &opts).map_err(|e| e.to_string())?;
                sq_plan::validate(&p).map_err(|e| e.to_string())?;
                Ok::<_, String>(sq_plan::explain(&p, None))
            }));
            match r {
                Ok(Ok(_)) => {}
                Ok(Err(e)) => fails.push(format!("{q} [{:?} reorder={}]: {e}", opts.join_policy, opts.reorder)),
                Err(_) => fails.push(format!("{q}: PANIC")),
            }
        }
    }
    for f in &fails {
        println!("FAIL {f}");
    }
}

#[test]
fn scratch_show2() {
    for q in [
        "SELECT * { ?s :knows ?o FILTER(?s = ?o) }",
        "SELECT * { ?x :knows ?y OPTIONAL { VALUES ?y { :p1 } } }",
        "SELECT * { VALUES (?x ?y) { (:p1 UNDEF) (UNDEF :p2) } ?x :knows ?y }",
        "SELECT * { ?g :age ?a GRAPH ?g { ?s :knows ?o } }",
        "SELECT * { ?s :knows ?o FILTER EXISTS { GRAPH ?s { } } }",
        "SELECT * { GRAPH ?g { ?s :knows ?o FILTER EXISTS { ?o :knows ?s } } }",
        "SELECT * { ?s :knows ?o FILTER(?o = :p1) FILTER(?o = :p2) }",
        "SELECT ?x { ?x :knows ?y } ORDER BY ?y LIMIT 0",
        "SELECT * { ?x :knows ?y { SELECT * { ?y :knows ?z } ORDER BY ?z LIMIT 1 OFFSET 2 } }",
    ] {
        show(q);
    }
}

#[test]
fn scratch_inf() {
    let st = social(3000);
    for k in [20, 40, 60, 80, 100, 120] {
        let q: String = (0..k).map(|i| format!("?a{i} :knows ?b{i} . ")).collect();
        let q = format!("SELECT * {{ {q} }}");
        let parsed = sq_plan::parse_query(&format!("{PREFIXES}{q}"), None).unwrap();
        let lowered = sq_plan::lower_query(&parsed, &st, &QueryOptions::default()).unwrap();
        match sq_plan::optimize(lowered, &st, &QueryOptions::default()) {
            Ok(p) => println!("k={k}: est={} cost={}", p.root.est_rows, p.root.est_cost_ns),
            Err(e) => println!("k={k}: ERR {e}"),
        }
    }
}

#[test]
fn scratch_min() {
    let st = social(50);
    let Ok(q) = std::env::var("REVIEW_Q") else { return };
    let parsed = sq_plan::parse_query(&format!("{PREFIXES}{q}"), None).expect("parse");
    let lowered = sq_plan::lower_query(&parsed, &st, &QueryOptions::default()).expect("lower");
    println!("{}", sq_plan::explain_logical(&lowered));
    match sq_plan::optimize(lowered, &st, &QueryOptions::default()) {
        Ok(p) => {
            println!("{}", sq_plan::explain(&p, None));
            println!("validate: {:?}", sq_plan::validate(&p));
        }
        Err(e) => println!("ERR {e}"),
    }
}

#[test]
fn scratch_empty_graph() {
    use sq_core::plan::JoinPolicy;
    let st = social(50);
    for q in [
        "SELECT * { ?x :name ?y GRAPH :zz { ?x :age ?z } OPTIONAL { ?x :city ?w } }",
        "SELECT * { ?x :name ?y GRAPH :zz { ?x :age ?z } MINUS { ?x :city ?w } }",
        "SELECT * { ?a :knows ?b { { ?b :city ?c } UNION { ?b :age ?c GRAPH :zz { } } } }",
        "SELECT * { ?a :knows ?b { ?b :city ?c GRAPH :zz { ?c :p ?d } FILTER(?a != ?c) } }",
        "SELECT * { ?a :knows ?b OPTIONAL { ?b :city ?c GRAPH :zz { ?c :p ?d } } }",
        "SELECT * FROM NAMED :g1 { ?x :name ?y GRAPH :g2 { ?x :age ?z } OPTIONAL { ?x :city ?w } }",
    ] {
        for (name, opts) in [
            ("auto", QueryOptions::default()),
            ("prefer-hash", QueryOptions { join_policy: JoinPolicy::PreferHash, ..Default::default() }),
            ("prefer-bind", QueryOptions { join_policy: JoinPolicy::PreferBind, ..Default::default() }),
        ] {
            let parsed = sq_plan::parse_query(&format!("{PREFIXES}{q}"), None).expect("parse");
            let lowered = sq_plan::lower_query(&parsed, &st, &opts).expect("lower");
            match sq_plan::optimize(lowered, &st, &opts) {
                Ok(_) => println!("OK   [{name}] {q}"),
                Err(e) => println!("FAIL [{name}] {q}: {e}"),
            }
        }
    }
}
