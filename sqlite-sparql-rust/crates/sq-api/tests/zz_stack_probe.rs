//! TEMPORARY stack probe (deleted before hand-off).
mod common;
use common::*;

fn build(kind: &str, d: usize) -> (String, &'static str) {
    let q = match kind {
        "paren" => format!("SELECT * WHERE {{ ?s ?p ?o FILTER({}?o{}) }}", "(".repeat(d), ")".repeat(d)),
        "group" => format!("SELECT * WHERE {}?s ?p ?o{}", "{ ".repeat(d), " }".repeat(d)),
        "plus" => format!("SELECT * WHERE {{ ?s ?p ?o FILTER(?o {} > 0) }}", "+ 1 ".repeat(d)),
        "or" => format!("SELECT * WHERE {{ ?s ?p ?o FILTER(?o = 0 {}) }}", "|| ?o = 1 ".repeat(d)),
        "and" => format!("SELECT * WHERE {{ ?s ?p ?o FILTER(?o != 0 {}) }}", "&& ?o != 1 ".repeat(d)),
        "union" => format!("SELECT * WHERE {{ {{ ?s ?p ?o }} {} }}", "UNION { ?s ?p ?o } ".repeat(d)),
        "optional" => format!("SELECT * WHERE {{ ?s ?p ?o {} }}", "OPTIONAL { ?s ?p ?o } ".repeat(d)),
        "filters" => format!("SELECT * WHERE {{ ?s ?p ?o {} }}", "FILTER(?o != 1) ".repeat(d)),
        "binds" => format!("SELECT * WHERE {{ ?s ?p ?o {} }}", (0..d).map(|i| format!("BIND(1 AS ?b{i}) ")).collect::<String>()),
        "joins" => format!("SELECT * WHERE {{ {} }}", "{ ?s ?p ?o } ".repeat(d)),
        "pathseq" => format!("SELECT * WHERE {{ ?s <http://ex/knows>{} ?o }}", "/<http://ex/knows>".repeat(d)),
        "pathalt" => format!("SELECT * WHERE {{ ?s <http://ex/knows>{} ?o }}", "|<http://ex/knows>".repeat(d)),
        "pathtriples" => format!("SELECT * WHERE {{ {} }}", "?s <http://ex/knows>? ?o . ".repeat(d)),
        "fn" => format!("SELECT * WHERE {{ ?s ?p ?o FILTER({}?o{} != 0) }}", "STR(".repeat(d), ")".repeat(d)),
        "exists" => format!("SELECT * WHERE {{ ?s ?p ?o {}{} }}", "FILTER EXISTS { ?s ?p ?o ".repeat(d), "}".repeat(d)),
        "subq" => format!("SELECT * WHERE {}?s ?p ?o{}", "{ SELECT * WHERE { ".repeat(d), " } }".repeat(d)),
        "coll" => return (format!("INSERT DATA {{ <http://ex/a> <http://ex/p> {}1{} }}", "( ".repeat(d), " )".repeat(d)), "update"),
        "bnode" => return (format!("INSERT DATA {{ <http://ex/a> <http://ex/p> {}1{} }}", "[ <http://ex/q> ".repeat(d), " ]".repeat(d)), "update"),
        "bnodeq" => format!("SELECT * WHERE {{ ?a <http://ex/p> {}1{} }}", "[ <http://ex/q> ".repeat(d), " ]".repeat(d)),
        "collq" => format!("SELECT * WHERE {{ ?a <http://ex/p> {}1{} }}", "( ".repeat(d), " )".repeat(d)),
        other => panic!("{other}"),
    };
    (q, "query")
}

#[test]
fn zz_probe_child() {
    let Ok(kind) = std::env::var("PROBE_KIND") else { return };
    let d: usize = std::env::var("PROBE_DEPTH").unwrap().parse().unwrap();
    let stack: usize = std::env::var("PROBE_STACK").unwrap().parse().unwrap();
    let engine = std::env::var("PROBE_ENGINE").unwrap_or("native".into());
    let t = std::thread::Builder::new().stack_size(stack).spawn(move || {
        let c = loaded();
        c.query_row("SELECT sparql_config('engine', ?1)", [&engine], |_| Ok(())).unwrap();
        let (q, what) = build(&kind, d);
        let r = if what == "update" {
            c.query_row("SELECT sparql_update(?1)", [&q], |r| r.get::<_, i64>(0))
        } else {
            c.query_row("SELECT count(*) FROM sparql_rows(?1)", [&q], |r| r.get::<_, i64>(0))
        };
        eprintln!("RESULT {r:?}");
    }).unwrap();
    t.join().unwrap();
}

#[test]
fn zz_probe() {
    let Ok(kinds) = std::env::var("PROBE_KINDS") else { return };
    let stack: usize = std::env::var("PROBE_STACK_PARENT").unwrap_or("8388608".into()).parse().unwrap();
    let engine = std::env::var("PROBE_ENGINE").unwrap_or("native".into());
    for kind in kinds.split(',') {
        let ok = |d: usize| {
            let out = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "zz_probe_child", "--nocapture", "--test-threads=1"])
                .env("PROBE_KIND", kind).env("PROBE_DEPTH", d.to_string()).env("PROBE_STACK", stack.to_string())
                .env("PROBE_ENGINE", &engine).env_remove("PROBE_KINDS")
                .output().unwrap();
            out.status.success()
        };
        let (mut lo, mut hi) = (1usize, 1usize);
        while hi < 200_000 && ok(hi) { lo = hi; hi *= 2; }
        if hi >= 200_000 { eprintln!("PROBE {kind}: >= {lo}"); continue; }
        while hi - lo > (lo / 20).max(1) { let m = (lo + hi) / 2; if ok(m) { lo = m } else { hi = m } }
        eprintln!("PROBE {kind} engine={engine} stack={stack}: max ok depth ~{lo} (fails at {hi})");
    }
}
