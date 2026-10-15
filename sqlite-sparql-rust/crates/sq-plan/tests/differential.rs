//! End-to-end differential check of the optimizer: random BGP / OPTIONAL / FILTER / COUNT /
//! GROUP BY / ORDER BY + LIMIT / DISTINCT queries over random small graphs are planned under every
//! `JoinPolicy` (and with `reorder` off), executed by sq-exec on a MemStore and on an in-memory
//! SQLite store, with and without statistics, and compared with sq-oracle as multisets.
//!
//! The data mix IRIs, inline integers, decimals, doubles, non-canonical integers and strings
//! as objects over a default graph and two named graphs; queries add GRAPH ?g patterns, FROM
//! clauses and the union default graph (merged multi-graph scans), so integer range pushdown (IntRange scans over rdf_gpos + rdf_gpox), merge joins
//! with enforcers, SQL chains (SqlJoin, Auto on SQLite only), star merges, factorized counts and
//! sorted probes all run against the reference semantics. Every SqlJoin statement is also
//! checked with `EXPLAIN QUERY PLAN`: one `SEARCH` per alias, in alias order, through the
//! alias's index and exactly its bound prefix.
mod common;

use std::rc::Rc;

use common::*;
use sq_core::plan::{JoinPolicy, Op, QueryOptions, QueryPlan};
use sq_core::store::{Col, IndexOrder, QuadStore};
use sq_core::testing::Rng;

const SUBJECTS: u64 = 7;

fn pick<'a>(r: &mut Rng, xs: &[&'a str]) -> &'a str {
    xs[r.below(xs.len() as u64) as usize]
}

/// A random object: IRIs, integers (some negative), decimals, doubles, non-canonical
/// integers, strings.
fn object(r: &mut Rng) -> String {
    match r.below(10) {
        0..=3 => format!(":s{}", r.below(SUBJECTS)),
        4..=6 => format!("{}", r.below(16) as i64 - 4),
        7 => format!("{}.5", r.below(12) as i64 - 3),
        8 => match r.below(3) {
            0 => format!("\"{}.0e0\"^^xsd:double", r.below(12)),
            1 => format!("\"0{}\"^^xsd:integer", r.below(12)),
            _ => format!("\"{}\"^^xsd:int", r.below(12)),
        },
        _ => format!("\"{}\"", pick(r, &["a", "bb", "a longer string"])),
    }
}

const TTL_PREFIXES: &str = "@prefix : <http://ex/> .\n@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .\n";

/// Named graphs of the random data (besides the default graph).
const GRAPHS: [&str; 2] = ["http://ex/g1", "http://ex/g2"];

/// Turtle documents for the default graph and for each of [`GRAPHS`].
fn graph(r: &mut Rng) -> [String; 3] {
    let mut docs = [TTL_PREFIXES.to_owned(), TTL_PREFIXES.to_owned(), TTL_PREFIXES.to_owned()];
    let n = 12 + r.below(50);
    for _ in 0..n {
        let s = r.below(SUBJECTS);
        let triple = if r.below(6) == 0 {
            format!(":s{s} a :C{} .\n", r.below(2))
        } else {
            format!(":s{s} :p{} {} .\n", r.below(4), object(r))
        };
        // Most triples in the default graph; some in a named graph, some in both.
        match r.below(8) {
            0 => docs[1].push_str(&triple),
            1 => docs[2].push_str(&triple),
            2 => {
                docs[0].push_str(&triple);
                docs[1].push_str(&triple);
            }
            _ => docs[0].push_str(&triple),
        }
    }
    docs
}

const VARS: [&str; 5] = ["?a", "?b", "?c", "?d", "?e"];

/// A triple pattern over the variables seen so far (`used`), connected to them most of the
/// time.
fn pattern(r: &mut Rng, used: &mut Vec<&'static str>) -> String {
    let var = |r: &mut Rng, used: &mut Vec<&'static str>| {
        let v = if !used.is_empty() && r.below(3) != 0 {
            used[r.below(used.len() as u64) as usize]
        } else {
            VARS[r.below(VARS.len() as u64) as usize]
        };
        if !used.contains(&v) {
            used.push(v);
        }
        v.to_owned()
    };
    let s = if r.below(6) == 0 { format!(":s{}", r.below(SUBJECTS)) } else { var(r, used) };
    if r.below(7) == 0 {
        return format!("{s} a :C{}", r.below(2));
    }
    let p = if r.below(8) == 0 { var(r, used) } else { format!(":p{}", r.below(4)) };
    let o = match r.below(8) {
        0 => format!(":s{}", r.below(SUBJECTS)),
        1 => format!("{}", r.below(10)),
        _ => var(r, used),
    };
    format!("{s} {p} {o}")
}

fn number(r: &mut Rng) -> String {
    let v = r.below(14) as i64 - 3;
    match r.below(4) {
        0 => format!("{v}"),
        1 => format!("{v}.5"),
        2 => format!("{v}.0e0"),
        _ => format!("\"{v}\"^^<http://www.w3.org/2001/XMLSchema#integer>"),
    }
}

fn filter(r: &mut Rng, used: &[&'static str]) -> String {
    let v = used[r.below(used.len() as u64) as usize];
    let op = pick(r, &[">", ">=", "<", "<="]);
    match r.below(7) {
        0..=2 => format!("FILTER({v} {op} {})", number(r)),
        3 => format!("FILTER({v} > {} && {v} <= {})", number(r), number(r)),
        4 => format!("FILTER({v} = :s{})", r.below(SUBJECTS)),
        5 => format!("FILTER({v} != :s{})", r.below(SUBJECTS)),
        _ => format!("FILTER({} {op} {v})", number(r)),
    }
}

/// A random query whose solutions are a well-defined multiset (ORDER BY sorts on every
/// projected variable before a LIMIT).
fn query(r: &mut Rng) -> String {
    let mut used: Vec<&'static str> = Vec::new();
    let mut body: Vec<String> = (0..1 + r.below(4)).map(|_| pattern(r, &mut used)).collect();
    if used.is_empty() {
        body.push("?a :p0 ?b".to_owned());
        used.extend(["?a", "?b"]);
    }
    if r.below(5) == 0 {
        // A pattern of the named graphs (GRAPH ?g pushed into the scan, or a loop).
        let p = pattern(r, &mut used);
        body.push(format!("GRAPH ?g {{ {p} }}"));
        used.push("?g");
    }
    if r.below(2) == 0 {
        // A numeric range on the object of a constant predicate: an IntRange scan candidate.
        let s = if r.below(2) == 0 { "?r" } else { used[r.below(used.len() as u64) as usize] };
        if !used.contains(&s) {
            used.push(s);
        }
        let o = if used.contains(&"?e") { "?d" } else { "?e" };
        if !used.contains(&o) {
            used.push(o);
        }
        let op = pick(r, &[">", ">=", "<", "<="]);
        body.push(format!("{s} :p{} {o} FILTER({o} {op} {})", r.below(4), number(r)));
    }
    if r.below(3) == 0 {
        body.push(filter(r, &used));
    }
    if r.below(4) == 0 {
        let mut inner_used = used.clone();
        let mut inner: Vec<String> = (0..1 + r.below(2)).map(|_| pattern(r, &mut inner_used)).collect();
        if r.below(3) == 0 {
            inner.push(filter(r, &inner_used));
        }
        body.push(format!("OPTIONAL {{ {} }}", inner.join(" . ")));
        used = inner_used;
    }
    let body = body.join(" . ");
    // Dataset clauses: one FROM graph, or the merge of two.
    let from = match r.below(10) {
        0 => " FROM :g1",
        1 => " FROM :g1 FROM :g2",
        _ => "",
    };
    let body = format!("{from} {{ {body} }}");
    match r.below(7) {
        0 => format!("SELECT (COUNT(*) AS ?n){body}"),
        6 => format!("SELECT {v} (COUNT(*) AS ?n){body} GROUP BY {v}", v = used[0]),
        1 => {
            let vars = used.join(" ");
            format!("SELECT {vars}{body} ORDER BY {vars} LIMIT {}", 1 + r.below(5))
        }
        2 => format!("SELECT DISTINCT {}{body}", used[0]),
        _ => format!("SELECT *{body}"),
    }
}

fn option_sets() -> Vec<(&'static str, QueryOptions)> {
    let with = |p| QueryOptions { join_policy: p, ..Default::default() };
    vec![
        ("auto", with(JoinPolicy::Auto)),
        ("prefer-merge", with(JoinPolicy::PreferMerge)),
        ("prefer-hash", with(JoinPolicy::PreferHash)),
        ("prefer-bind", with(JoinPolicy::PreferBind)),
        ("no-sql", with(JoinPolicy::NoSql)),
        ("no-reorder", QueryOptions { reorder: false, ..Default::default() }),
        ("union-default", QueryOptions { union_default_graph: true, ..Default::default() }),
        ("union-merge", QueryOptions { union_default_graph: true, ..with(JoinPolicy::PreferMerge) }),
    ]
}

/// Every SqlJoin statement of `p` is planned by SQLite exactly as rendered: one SEARCH per
/// alias, in alias order, through the alias's index, constrained on exactly its bound prefix.
fn check_sql_joins(p: &QueryPlan, db: &Sqlite) -> Result<usize, String> {
    let mut n = 0;
    for node in nodes(p) {
        let Op::SqlJoin { sql, .. } = &node.op else { continue };
        n += 1;
        let eqp = db.store.explain_query_plan(sql).map_err(|e| e.to_string())?;
        let lines: Vec<&str> = eqp.lines().collect();
        let aliases = sql_aliases(sql);
        if lines.len() != aliases.len() {
            return Err(format!("{sql}\n{eqp}\nexpected {} SEARCH lines", aliases.len()));
        }
        for (i, (line, (order, prefix))) in lines.iter().zip(&aliases).enumerate() {
            let index = match order {
                IndexOrder::Gspo => "PRIMARY KEY".to_owned(),
                o => format!("COVERING INDEX {}", o.sql_from().rsplit(' ').next().unwrap_or_default()),
            };
            let cons: Vec<String> =
                order.cols()[..*prefix].iter().map(|c| format!("{}=?", ["g", "s", "p", "o"][*c as usize])).collect();
            let expected = format!("SEARCH t{i} USING {index} ({})", cons.join(" AND "));
            if line.trim() != expected {
                return Err(format!("{sql}\n{eqp}\nline {i}: expected `{expected}`"));
            }
        }
    }
    Ok(n)
}

/// (index order, number of leading index columns bound by a constant or an earlier alias) per
/// alias of a rendered SqlJoin statement.
fn sql_aliases(sql: &str) -> Vec<(IndexOrder, usize)> {
    let (_, rest) = sql.split_once(" FROM ").expect("FROM");
    let (from, where_) = rest.split_once(" WHERE ").unwrap_or((rest, ""));
    let terms: Vec<(&str, &str)> = where_.split(" AND ").filter_map(|t| t.split_once('=')).collect();
    from.split(" CROSS JOIN ")
        .enumerate()
        .map(|(i, item)| {
            let index = item.rsplit(' ').next().unwrap_or_default();
            let order = IndexOrder::ALL
                .into_iter()
                .find(|o| o.sql_from().ends_with(&format!(" {index}")))
                .expect("known index");
            let col_name = |c: &Col| ["g", "s", "p", "o"][*c as usize];
            // A parameter, or a column of an earlier alias.
            let earlier = |rhs: &str| {
                let alias = rhs.strip_prefix('t').and_then(|r| r.split('.').next());
                alias.is_some_and(|a| a.parse().is_ok_and(|a: usize| a < i))
            };
            let bound = |c: &Col| {
                let lhs_wanted = format!("t{i}.{}", col_name(c));
                terms.iter().any(|(lhs, rhs)| *lhs == lhs_wanted && (rhs.starts_with('?') || earlier(rhs)))
            };
            let prefix = order.cols().iter().take_while(|c| bound(c)).count();
            (order, prefix)
        })
        .collect()
}

#[derive(Default, Debug)]
struct Tally {
    cases: usize,
    runs: usize,
    sql_joins: usize,
    merge_joins: usize,
    star_merges: usize,
    enforcers: usize,
    int_ranges: usize,
    fast_counts: usize,
    streaming_groups: usize,
}

fn tally_plan(p: &QueryPlan, t: &mut Tally) {
    for n in nodes(p) {
        match &n.op {
            Op::MergeJoin { .. } => t.merge_joins += 1,
            Op::StarMerge { .. } => t.star_merges += 1,
            Op::SortBySlot { .. } => t.enforcers += 1,
            Op::Scan(s) if s.range.is_some() => t.int_ranges += 1,
            Op::Count { input, .. } if matches!(input.op, Op::MergeJoin { .. } | Op::StarMerge { .. }) => {
                t.fast_counts += 1
            }
            Op::Group { streaming: true, .. } => t.streaming_groups += 1,
            _ => {}
        }
    }
}

/// Every comparison operator against integer, decimal, double, float and non-canonical integer
/// constants around the stored values: the IntRange bounds (and the numeric exceptions read
/// from rdf_gpox) must keep exactly the rows the FILTER keeps.
#[test]
fn int_range_scans_keep_exactly_the_filtered_rows() {
    let mut ttl = String::from("@prefix : <http://ex/> .\n@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .\n");
    for (i, o) in [
        "-5", "-4", "-3", "0", "1", "2", "3", "7", "9", "10", "11", "12", "-3.5", "2.5", "9.0", "\"9.5e0\"^^xsd:double",
        "\"010\"^^xsd:integer", "\"3\"^^xsd:short", "\"NaN\"^^xsd:double", "\"x\"^^xsd:integer", "\"10\"", ":z",
        "\"2.5\"^^xsd:float",
    ]
    .iter()
    .enumerate()
    {
        ttl.push_str(&format!(":s{i} :p {o} .\n:s{i} :n {o} .\n"));
    }
    // Far-away integers make every range below selective, so the IntRange scan is chosen: :p
    // (queried with > and >=) gets very small ones, :n (queried with < and <=) very large ones.
    for i in 0..150 {
        ttl.push_str(&format!(":f{i} :p {} .\n:f{i} :n {} .\n", -1000 - i, 1000 + i));
    }
    let mem = store(ttl.strip_prefix("@prefix : <http://ex/> .\n").unwrap());
    mem.analyze();
    let db = Sqlite::new();
    db.load(&ttl, None);
    db.store.analyze().expect("analyze");
    let mem_rc: Rc<dyn QuadStore> = Rc::new(mem.clone());
    let sql_rc: Rc<dyn QuadStore> = Rc::new(db.store.clone());
    let constants = [
        "-4", "0", "3", "9", "10", "11", "-3.5", "2.5", "9.5", "-0.5", "3.0e0", "9.5e0", "-3.5e0", "1e1",
        "\"9.5\"^^<http://www.w3.org/2001/XMLSchema#float>", "\"010\"^^<http://www.w3.org/2001/XMLSchema#integer>",
        "\"7\"^^<http://www.w3.org/2001/XMLSchema#byte>",
    ];
    let mut failures = Vec::new();
    let mut ranges = 0;
    for (op, flipped, p) in [(">", "<", ":p"), (">=", "<=", ":p"), ("<", ">", ":n"), ("<=", ">=", ":n")] {
        for c in constants {
            for q in [
                format!("SELECT ?s ?o {{ ?s {p} ?o FILTER(?o {op} {c}) }}"),
                format!("SELECT ?s ?o {{ ?s {p} ?o FILTER({c} {flipped} ?o) }}"),
                format!("SELECT ?s ?o {{ ?s {p} ?o FILTER(?o {op} {c} && ?o != 3) }}"),
                format!("SELECT (COUNT(*) AS ?n) {{ ?s {p} ?o FILTER(?o {op} {c}) }}"),
            ] {
                let expected = oracle(&lower_on(&q, &mem, &QueryOptions::default()), &mem).expect("oracle");
                for (store_name, st) in [("mem", &mem_rc), ("sqlite", &sql_rc)] {
                    let _read = (store_name == "sqlite").then(|| {
                        let scope = db.store.read_scope(0).expect("read scope");
                        db.store.shared().validate_epoch().expect("epoch");
                        scope
                    });
                    let plan = plan_on(&q, &**st, &QueryOptions::default());
                    ranges += nodes(&plan).iter().filter(|n| matches!(&n.op, Op::Scan(s) if s.range.is_some())).count();
                    let text = sq_plan::explain(&plan, None);
                    match execute(Rc::clone(st), plan, &QueryOptions::default()) {
                        Ok(got) if got == expected => {}
                        other => failures.push(format!(
                            "[{store_name}] {q}\n{text}expected {expected:?}\n     got {other:?}"
                        )),
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures[..failures.len().min(4)].join("\n\n"));
    // The filler makes every range selective: (nearly) every query reads an IntRange scan.
    assert!(ranges > 400, "only {ranges} IntRange scans");
}

#[test]
fn plans_match_the_oracle_on_memory_and_sqlite_stores() {
    let mut r = Rng(0x5EED_0F_0B71);
    let mut failures: Vec<String> = Vec::new();
    let mut t = Tally::default();
    for case in 0..250 {
        let docs = graph(&mut r);
        let mem = sq_memstore::MemStore::new();
        let db = Sqlite::new();
        for (doc, g) in docs.iter().zip([None, Some(GRAPHS[0]), Some(GRAPHS[1])]) {
            load_ttl(&mem, doc, g);
            db.load(doc, g);
        }
        let stats = case % 2 == 1;
        if stats {
            mem.analyze();
            db.store.analyze().expect("analyze");
        }
        let mem_rc: Rc<dyn QuadStore> = Rc::new(mem.clone());
        let sql_rc: Rc<dyn QuadStore> = Rc::new(db.store.clone());
        for _ in 0..4 {
            let q = query(&mut r);
            t.cases += 1;
            // The reference per default-graph mode (the union flag changes the dataset).
            let mut expected_by_union = [None, None];
            for (name, opts) in option_sets() {
                let slot = &mut expected_by_union[usize::from(opts.union_default_graph)];
                if slot.is_none() {
                    let lowered = lower_on(&q, &mem, &opts);
                    *slot = Some(oracle(&lowered, &mem));
                }
                let expected = match slot.as_ref().expect("computed above") {
                    Ok(rows) => rows.clone(),
                    Err(e) => {
                        failures.push(format!("oracle failed on {q}: {e}"));
                        continue;
                    }
                };
                for (store_name, st) in [("mem", &mem_rc), ("sqlite", &sql_rc)] {
                    t.runs += 1;
                    let _read = (store_name == "sqlite").then(|| {
                        let scope = db.store.read_scope(0).expect("read scope");
                        db.store.shared().validate_epoch().expect("epoch");
                        scope
                    });
                    let plan = plan_on(&q, &**st, &opts);
                    tally_plan(&plan, &mut t);
                    if store_name == "sqlite" {
                        match check_sql_joins(&plan, &db) {
                            Ok(n) => t.sql_joins += n,
                            Err(e) => failures.push(format!("[{name}/{store_name}] {q}\n{e}")),
                        }
                    }
                    let text = sq_plan::explain(&plan, None);
                    match execute(Rc::clone(st), plan, &opts) {
                        Ok(got) if got == expected => {}
                        Ok(got) => failures.push(format!(
                            "[{name}/{store_name} stats={stats}] {q}\n{text}\
                             expected {} rows {:?}\n     got {} rows {:?}",
                            expected.len(),
                            &expected[..expected.len().min(6)],
                            got.len(),
                            &got[..got.len().min(6)]
                        )),
                        Err(e) => failures.push(format!("[{name}/{store_name} stats={stats}] {q}\n{text}error: {e}")),
                    }
                }
            }
        }
    }
    eprintln!("{t:?}");
    let first = failures[..failures.len().min(5)].join("\n\n");
    assert!(failures.is_empty(), "{} failures, first ones:\n{first}", failures.len());
    // The random workload must actually exercise the optimizer's operators.
    assert!(t.sql_joins > 0 && t.merge_joins > 0 && t.enforcers > 0 && t.int_ranges > 0 && t.fast_counts > 0, "{t:?}");
}
