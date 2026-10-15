//! TEMPORARY planning-time probe (deleted before handing in).
mod common;

use common::*;
use sq_core::plan::{JoinPolicy, QueryOptions};
use sq_core::store::QuadStore;

fn with(policy: JoinPolicy) -> QueryOptions {
    QueryOptions { join_policy: policy, ..Default::default() }
}

fn time_plan(q: &str, st: &dyn QuadStore, opts: &QueryOptions) -> (f64, String) {
    let parsed = sq_plan::parse_query(&format!("{PREFIXES}{q}"), None).expect("parses");
    let mut best = f64::INFINITY;
    let mut text = String::new();
    for _ in 0..7 {
        let lowered = sq_plan::lower_query(&parsed, st, opts).expect("lowers");
        let t = std::time::Instant::now();
        let p = sq_plan::optimize(lowered, st, opts).expect("plans");
        best = best.min(t.elapsed().as_secs_f64() * 1e3);
        text = sq_plan::explain(&p, None);
    }
    (best, text)
}

fn queries() -> Vec<(&'static str, String)> {
    let clique = "SELECT * { ?a :knows ?b . ?a :knows ?c . ?a :knows ?d . ?a :knows ?e . ?b :knows ?c . ?b :knows ?d . \
             ?b :knows ?e . ?c :knows ?d . ?c :knows ?e . ?d :knows ?e }"
        .to_owned();
    let clique4x6 = "SELECT * { ?a :knows ?b . ?a :knows ?c . ?a :knows ?d . ?b :knows ?c . ?b :knows ?d . ?c :knows ?d }"
        .to_owned();
    let grid = "SELECT * { ?a :knows ?b . ?b :knows ?c . ?c :knows ?d . ?d :knows ?e . ?e :knows ?a . \
                ?a :age ?x . ?b :age ?x . ?c :city ?y . ?d :city ?y . ?e :name ?z }"
        .to_owned();
    let star10 = "SELECT * { ?x a :C3 ; :age ?a ; :city ?c ; :name ?n ; :knows ?k1 . ?k1 :age ?a2 ; :city ?c2 ; :name ?n2 ; \
                  :knows ?k2 . ?k2 :age ?a3 }"
        .to_owned();
    let star9 = "SELECT * { ?x a :C3 ; :age ?a ; :city ?c ; :name ?n ; :knows ?k1 , ?k2, ?k3, ?k4, ?k5 }".to_owned();
    let chain12 = "SELECT * { ?a :knows ?b . ?b :knows ?c . ?c :knows ?d . ?d :knows ?e . ?e :knows ?f . ?f :knows ?g . \
                   ?g :knows ?h . ?h :knows ?i . ?i :knows ?j . ?j :knows ?k . ?k :knows ?l . ?l :knows ?m }"
        .to_owned();
    vec![
        ("clique5x10", clique),
        ("clique4x6", clique4x6),
        ("grid10", grid),
        ("star10", star10),
        ("star9", star9),
        ("chain12", chain12),
    ]
}

#[test]
fn zz_dp_timing() {
    let mem = social(500);
    let db = sqlite_social(500);
    let _read = db.store.read_scope(0).expect("read scope");
    let show = std::env::var("SHOW").is_ok();
    for (name, q) in queries() {
        for (sname, st) in [("mem", &mem as &dyn QuadStore), ("sql", &db.store as &dyn QuadStore)] {
            let mut line = format!("{name:>11} {sname}:");
            for (pname, opts) in [
                ("auto", with(JoinPolicy::Auto)),
                ("merge", with(JoinPolicy::PreferMerge)),
                ("hash", with(JoinPolicy::PreferHash)),
                ("nosql", with(JoinPolicy::NoSql)),
            ] {
                let (t, text) = time_plan(&q, st, &opts);
                line.push_str(&format!(" {pname}={t:6.2}"));
                if show && pname == "auto" {
                    eprintln!("{text}");
                }
            }
            eprintln!("{line}");
        }
    }
}
