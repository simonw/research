//! Parse + lower + optimize + validate every `.rq` of the W3C SPARQL 1.0 / 1.1 suites over
//! (a) an empty MemStore and (b) a MemStore loaded with the test's data (with and without
//! statistics, under every join policy), checking the static-property invariants on every
//! logical and physical node. Files spargebra rejects are skipped (syntax tests).
//!
//! The suite location is `$SQ_W3C_DIR` (default: the scratchpad checkout); the test is skipped
//! with a note when it is absent.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use oxrdf::{GraphName, NamedNode, NamedOrBlankNode, Term};
use rustc_hash::FxHashMap;
use sq_core::ir::{Logical, LoweredQuery};
use sq_core::plan::{JoinPolicy, Plan, QueryOptions, QueryPlan};
use sq_core::store::{QuadStore, QuadStoreWriter};
use sq_core::{Error, TermId};
use sq_memstore::MemStore;
use sq_plan::{explain, explain_logical, lower_query, optimize, parse_query, plan_props, props, validate};

const DEFAULT_SUITE: &str =
    "/tmp/claude-0/-home-user-research/82148577-c93b-5a93-8757-e7fe894ecb5e/scratchpad/w3c/rdf-tests/sparql";
const BASE: &str = "https://w3c.github.io/rdf-tests/sparql/";
const QT: &str = "http://www.w3.org/2001/sw/DataAccess/tests/test-query#";

fn suite_dir() -> Option<PathBuf> {
    let dir = std::env::var("SQ_W3C_DIR").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(DEFAULT_SUITE));
    if dir.join("sparql11").is_dir() {
        Some(dir)
    } else {
        eprintln!("W3C suite not found at {} (set SQ_W3C_DIR); skipping", dir.display());
        None
    }
}

fn iri_of(root: &Path, file: &Path) -> String {
    format!("{BASE}{}", file.strip_prefix(root).expect("file under the suite").display())
}

fn path_of(root: &Path, iri: &str) -> Option<PathBuf> {
    iri.strip_prefix(BASE).map(|rel| root.join(rel))
}

#[derive(Default, Clone)]
struct Case {
    data: Vec<PathBuf>,
    graph_data: Vec<PathBuf>,
}

/// query file -> its data files, from every `manifest.ttl`.
fn manifest_cases(root: &Path) -> FxHashMap<PathBuf, Case> {
    let mut out: FxHashMap<PathBuf, Case> = FxHashMap::default();
    for m in files(root, "ttl").into_iter().filter(|p| p.file_name().is_some_and(|n| n == "manifest.ttl")) {
        let Ok(parser) = oxttl::TurtleParser::new().with_base_iri(iri_of(root, &m)) else { continue };
        let Ok(bytes) = fs::read(&m) else { continue };
        let triples: Vec<oxrdf::Triple> = parser.for_slice(&bytes).filter_map(Result::ok).collect();
        let mut by_action: BTreeMap<String, (Option<String>, Case)> = BTreeMap::new();
        for t in &triples {
            let Some(local) = t.predicate.as_str().strip_prefix(QT) else { continue };
            let Term::NamedNode(obj) = &t.object else { continue };
            let key = match &t.subject {
                NamedOrBlankNode::BlankNode(b) => b.as_str().to_owned(),
                NamedOrBlankNode::NamedNode(n) => n.as_str().to_owned(),
                #[allow(unreachable_patterns)]
                _ => continue,
            };
            let entry = by_action.entry(key).or_default();
            let Some(path) = path_of(root, obj.as_str()) else { continue };
            match local {
                "query" => entry.0 = Some(obj.as_str().to_owned()),
                "data" => entry.1.data.push(path),
                "graphData" => entry.1.graph_data.push(path),
                _ => {}
            }
        }
        for (query, case) in by_action.into_values() {
            if let Some(q) = query.and_then(|q| path_of(root, &q)) {
                out.insert(q, case);
            }
        }
    }
    out
}

fn files(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == ext) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Load an RDF file into `store` (default graph, or the named graph = file IRI), with fresh
/// blank nodes per file.
fn load(store: &MemStore, root: &Path, file: &Path, named: bool) {
    let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
    let Some(format) = oxrdfio::RdfFormat::from_extension(ext) else { return };
    let iri = iri_of(root, file);
    let Ok(bytes) = fs::read(file) else { return };
    let Ok(parser) = oxrdfio::RdfParser::from_format(format).with_base_iri(iri.clone()) else { return };
    let g = if named { store.intern(NamedNode::new_unchecked(iri).as_ref().into()).unwrap() } else { TermId::DEFAULT_GRAPH };
    let mut bnodes: FxHashMap<String, TermId> = FxHashMap::default();
    let mut id = |t: oxrdf::TermRef<'_>| -> TermId {
        match t {
            oxrdf::TermRef::BlankNode(b) => {
                *bnodes.entry(b.as_str().to_owned()).or_insert_with(|| store.fresh_bnode().unwrap())
            }
            other => store.intern(other).unwrap(),
        }
    };
    let mut quads = Vec::new();
    for q in parser.for_slice(&bytes) {
        let Ok(q) = q else { return }; // unparsable data (negative syntax tests): leave the store empty
        let graph = match q.graph_name {
            GraphName::DefaultGraph => g,
            GraphName::NamedNode(n) => id(n.as_ref().into()),
            GraphName::BlankNode(b) => id(b.as_ref().into()),
        };
        let s = id(q.subject.as_ref().into());
        let p = id(q.predicate.as_ref().into());
        let o = id(q.object.as_ref());
        quads.push([graph, s, p, o]);
    }
    store.insert(&mut quads).unwrap();
}

fn walk_logical<'a>(l: &'a Logical, f: &mut impl FnMut(&'a Logical)) {
    f(l);
    match l {
        Logical::Unit
        | Logical::Empty
        | Logical::Bgp(_)
        | Logical::Path { .. }
        | Logical::Values { .. }
        | Logical::Service { .. } => {}
        Logical::Join(a, b) | Logical::Union(a, b) => {
            walk_logical(a, f);
            walk_logical(b, f);
        }
        Logical::LeftJoin { left, right, .. } | Logical::Minus { left, right } => {
            walk_logical(left, f);
            walk_logical(right, f);
        }
        Logical::Filter { input, .. }
        | Logical::Extend { input, .. }
        | Logical::Graph { input, .. }
        | Logical::Group { input, .. }
        | Logical::OrderBy { input, .. }
        | Logical::Project { input, .. }
        | Logical::Distinct(input)
        | Logical::Reduced(input)
        | Logical::Slice { input, .. } => walk_logical(input, f),
    }
}

fn walk_plan<'a>(p: &'a Plan, f: &mut impl FnMut(&'a Plan)) {
    f(p);
    for c in p.children() {
        walk_plan(c, f);
    }
}

/// safe_inject ⊆ vars, certain ⊆ maybe ⊆ vars on every node.
fn check_props(name: &str, q: &LoweredQuery) {
    let mut check = |l: &Logical| {
        let p = props(l, &q.exists);
        assert!(p.safe_inject.is_subset(&p.vars), "{name}: safe_inject ⊄ vars at {l:?}");
        assert!(p.certain.is_subset(&p.maybe), "{name}: certain ⊄ maybe at {l:?}");
        assert!(p.maybe.is_subset(&p.vars), "{name}: maybe ⊄ vars at {l:?}");
    };
    walk_logical(&q.root, &mut check);
    for e in &q.exists {
        walk_logical(e, &mut check);
    }
}

fn check_plan_props(name: &str, plan: &QueryPlan) {
    let mut check = |p: &Plan| {
        let pp = plan_props(p, &plan.exists);
        assert!(pp.safe_inject.is_subset(&pp.vars), "{name}: plan safe_inject ⊄ vars at #{}", p.id);
        assert!(pp.certain.is_subset(&pp.maybe), "{name}: plan certain ⊄ maybe at #{}", p.id);
    };
    walk_plan(&plan.root, &mut check);
    for e in &plan.exists {
        walk_plan(e, &mut check);
    }
}

#[derive(Default, Debug)]
struct Tally {
    queries: usize,
    rejected_by_parser: usize,
    with_data: usize,
    plans: usize,
    unsupported: usize,
}

/// Lower, check, optimize, validate and explain one query; Err(Unsupported) is tallied.
fn plan_once(name: &str, q: &spargebra::Query, store: &dyn QuadStore, opts: &QueryOptions, tally: &mut Tally) {
    let lowered = lower_query(q, store, opts).unwrap_or_else(|e| panic!("{name}: lowering failed: {e}"));
    check_props(name, &lowered);
    assert!(!explain_logical(&lowered).is_empty());
    match optimize(lowered, store, opts) {
        Ok(plan) => {
            validate(&plan).unwrap_or_else(|e| panic!("{name}: {e}\n{}", explain(&plan, None)));
            check_plan_props(name, &plan);
            assert!(!explain(&plan, None).is_empty());
            tally.plans += 1;
        }
        Err(Error::Unsupported(msg)) => {
            assert!(msg.contains("SERVICE"), "{name}: unexpected Unsupported: {msg}");
            tally.unsupported += 1;
        }
        Err(e) => panic!("{name}: optimize failed: {e}"),
    }
}

fn option_sets() -> Vec<QueryOptions> {
    let base = QueryOptions::default();
    vec![
        base.clone(),
        QueryOptions { join_policy: JoinPolicy::PreferHash, ..base.clone() },
        QueryOptions { join_policy: JoinPolicy::PreferBind, ..base.clone() },
        QueryOptions { reorder: false, union_default_graph: true, ..base },
    ]
}

#[test]
fn every_w3c_query_plans_and_validates() {
    let Some(root) = suite_dir() else { return };
    let cases = manifest_cases(&root);
    let mut tally = Tally::default();
    for suite in ["sparql10", "sparql11"] {
        for file in files(&root.join(suite), "rq") {
            let name = file.strip_prefix(&root).unwrap().display().to_string();
            let Ok(text) = fs::read_to_string(&file) else { continue };
            tally.queries += 1;
            let q = match parse_query(&text, Some(&iri_of(&root, &file))) {
                Ok(q) => q,
                Err(Error::Syntax(_)) => {
                    tally.rejected_by_parser += 1;
                    continue;
                }
                Err(e) => panic!("{name}: unexpected parse error {e}"),
            };
            // (a) empty store
            let empty = MemStore::new();
            plan_once(&name, &q, &empty, &QueryOptions::default(), &mut tally);
            // (b) the test's data, without and with statistics, under every policy
            let Some(case) = cases.get(&file) else { continue };
            let store = MemStore::new();
            for d in &case.data {
                load(&store, &root, d, false);
            }
            for g in &case.graph_data {
                load(&store, &root, g, true);
            }
            if !store.quads().is_empty() {
                tally.with_data += 1;
            }
            for opts in option_sets() {
                plan_once(&name, &q, &store, &opts, &mut tally);
            }
            store.analyze();
            for opts in option_sets() {
                plan_once(&name, &q, &store, &opts, &mut tally);
            }
        }
    }
    eprintln!("{tally:?}");
    assert!(tally.queries > 900, "suite incomplete: {tally:?}");
    assert!(tally.with_data > 400, "manifests / data not picked up: {tally:?}");
    assert!(tally.plans > 3000, "too few plans: {tally:?}");
}
