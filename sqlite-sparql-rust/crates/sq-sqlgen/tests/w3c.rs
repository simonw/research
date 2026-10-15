//! W3C SPARQL 1.0 / 1.1 query-evaluation tests inside the baseline's subset: every test whose
//! query the translator accepts must give spareval's answer (tests listed in
//! tools/sq-w3c/reference_divergences.txt are reference bugs and are only counted).
//!
//! The suite is read from `$SQ_W3C_DIR` (default: the session scratchpad copy); the test is
//! skipped with a note when it is absent. Run with `--nocapture` for the report.
mod common;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use common::{Baseline, DataFile, Engine, Expect, compare};
use oxrdf::{Graph, NamedOrBlankNodeRef, TermRef, vocab::rdf};
use oxrdfio::RdfFormat;

const DEFAULT_SUITE: &str = "/tmp/claude-0/-home-user-research/82148577-c93b-5a93-8757-e7fe894ecb5e/scratchpad/w3c/rdf-tests/sparql";
const SUITE_URL: &str = "https://w3c.github.io/rdf-tests/sparql/";
const MF: &str = "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#";
const QT: &str = "http://www.w3.org/2001/sw/DataAccess/tests/test-query#";

/// In-subset tests whose baseline answer differs from spareval for a documented reason
/// (none so far).
const KNOWN_MISMATCHES: &[(&str, &str)] = &[];

struct TestCase {
    id: String,
    query: String,
    data: Vec<String>,
    graph_data: Vec<String>,
    lax: bool,
}

fn suite_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("SQ_W3C_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(DEFAULT_SUITE));
    dir.join("sparql11").is_dir().then_some(dir)
}

fn local_path(root: &Path, url: &str) -> Option<PathBuf> {
    url.strip_prefix(SUITE_URL).map(|rel| root.join(rel))
}

fn nn(iri: &str) -> oxrdf::NamedNode {
    oxrdf::NamedNode::new_unchecked(iri)
}

fn subject(t: TermRef<'_>) -> Option<NamedOrBlankNodeRef<'_>> {
    match t {
        TermRef::NamedNode(n) => Some(n.into()),
        TermRef::BlankNode(b) => Some(b.into()),
        _ => None,
    }
}

fn rdf_list<'a>(g: &'a Graph, mut head: TermRef<'a>) -> Vec<TermRef<'a>> {
    let mut out = Vec::new();
    while let Some(s) = subject(head) {
        if head == rdf::NIL.into() {
            break;
        }
        match g.object_for_subject_predicate(s, rdf::FIRST) {
            Some(first) => out.push(first),
            None => break,
        }
        match g.object_for_subject_predicate(s, rdf::REST) {
            Some(rest) => head = rest,
            None => break,
        }
    }
    out
}

fn iris(g: &Graph, s: NamedOrBlankNodeRef<'_>, p: &str) -> Vec<String> {
    g.objects_for_subject_predicate(s, &nn(p))
        .filter_map(|o| match o {
            TermRef::NamedNode(n) => Some(n.as_str().to_owned()),
            _ => None,
        })
        .collect()
}

fn load_manifest(root: &Path, url: &str, seen: &mut HashSet<String>, out: &mut Vec<TestCase>) {
    if !seen.insert(url.to_owned()) {
        return;
    }
    let Some(path) = local_path(root, url) else { return };
    let Ok(bytes) = std::fs::read(&path) else { return };
    let mut g = Graph::new();
    for t in oxttl::TurtleParser::new().with_base_iri(url).expect("valid base").for_slice(&bytes) {
        g.insert(&t.expect("valid manifest"));
    }
    let me = nn(url);
    for inc in g.objects_for_subject_predicate(&me, &nn(&format!("{MF}include"))) {
        for m in rdf_list(&g, inc) {
            if let TermRef::NamedNode(n) = m {
                load_manifest(root, n.as_str(), seen, out);
            }
        }
    }
    let Some(entries) = g.object_for_subject_predicate(&me, &nn(&format!("{MF}entries"))) else { return };
    for entry in rdf_list(&g, entries) {
        let TermRef::NamedNode(test) = entry else { continue };
        let is_query_eval = g
            .objects_for_subject_predicate(test, rdf::TYPE)
            .any(|t| t == nn(&format!("{MF}QueryEvaluationTest")).as_ref().into());
        if !is_query_eval {
            continue;
        }
        let Some(action) = g.object_for_subject_predicate(test, &nn(&format!("{MF}action"))).and_then(subject) else {
            continue;
        };
        let Some(query) = iris(&g, action, &format!("{QT}query")).pop() else { continue };
        let lax = g
            .object_for_subject_predicate(test, &nn(&format!("{MF}resultCardinality")))
            .is_some_and(|c| c == nn(&format!("{MF}LaxCardinality")).as_ref().into());
        out.push(TestCase {
            id: test.as_str().to_owned(),
            query,
            data: iris(&g, action, &format!("{QT}data")),
            graph_data: iris(&g, action, &format!("{QT}graphData")),
            lax,
        });
    }
}

fn format_of(url: &str) -> RdfFormat {
    match url.rsplit('.').next() {
        Some("rdf") => RdfFormat::RdfXml,
        Some("nt") => RdfFormat::NTriples,
        Some("nq") => RdfFormat::NQuads,
        Some("trig") => RdfFormat::TriG,
        _ => RdfFormat::Turtle,
    }
}

fn data_file(root: &Path, url: &str, graph: Option<&str>) -> Result<DataFile, String> {
    let path = local_path(root, url).ok_or_else(|| format!("not a suite file: {url}"))?;
    let data = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(DataFile { data, format: format_of(url), base: Some(url.to_owned()), graph: graph.map(str::to_owned) })
}

enum Verdict {
    Match,
    Mismatch(String),
    Unsupported(String),
    Skipped(String),
}

fn run_test(root: &Path, t: &TestCase) -> Verdict {
    let Some(qpath) = local_path(root, &t.query) else { return Verdict::Skipped("query outside the suite".into()) };
    let Ok(text) = std::fs::read_to_string(&qpath) else { return Verdict::Skipped("unreadable query".into()) };
    let query = match sq_plan::parse_query(&text, Some(&t.query)) {
        Ok(q) => q,
        Err(e) => return Verdict::Skipped(format!("parse: {e}")),
    };
    let mut files = Vec::new();
    let result: Result<(), String> = (|| {
        for d in &t.data {
            files.push(data_file(root, d, None)?);
        }
        for g in &t.graph_data {
            files.push(data_file(root, g, Some(g))?);
        }
        // FROM / FROM NAMED without qt:data: the referenced files are the named graphs.
        if t.data.is_empty()
            && let spargebra::Query::Select { dataset: Some(ds), .. } | spargebra::Query::Ask { dataset: Some(ds), .. } =
                &query
        {
            let mut names: Vec<&str> = ds.default.iter().map(|n| n.as_str()).collect();
            names.extend(ds.named.iter().flatten().map(|n| n.as_str()));
            for n in names {
                if !t.graph_data.iter().any(|g| g == n) && local_path(root, n).is_some_and(|p| p.is_file()) {
                    files.push(data_file(root, n, Some(n))?);
                }
            }
        }
        Ok(())
    })();
    if let Err(e) = result {
        return Verdict::Skipped(e);
    }
    let engine = match Engine::new(&files) {
        Ok(e) => e,
        Err(e) => return Verdict::Skipped(format!("load: {e}")),
    };
    let ours = match engine.baseline(&query) {
        Ok(Baseline::Unsupported(msg)) => return Verdict::Unsupported(msg),
        Ok(Baseline::Ran { sql, outcome }) => (sql, outcome),
        Err(e) => return Verdict::Mismatch(format!("baseline failed: {e}")),
    };
    let reference = match engine.reference(&query) {
        Ok(r) => r,
        Err(e) => return Verdict::Skipped(format!("reference: {e}")),
    };
    let mut expect = Expect::of(&query);
    expect.lax |= t.lax;
    match compare(&ours.1, &reference, &expect) {
        Ok(()) => Verdict::Match,
        Err(e) => Verdict::Mismatch(format!("{e}\nSQL: {}", ours.0.sql)),
    }
}

fn reference_divergences() -> HashSet<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/sq-w3c/reference_divergences.txt");
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .filter(|w| !w.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

#[test]
fn w3c_query_evaluation_tests_in_the_subset_match() {
    let Some(root) = suite_dir() else {
        eprintln!("note: W3C suite not found (set SQ_W3C_DIR); skipping");
        return;
    };
    let mut tests = Vec::new();
    let mut seen = HashSet::new();
    for m in ["sparql11/manifest-all.ttl", "sparql10/manifest-evaluation.ttl"] {
        load_manifest(&root, &format!("{SUITE_URL}{m}"), &mut seen, &mut tests);
    }
    let divergent = reference_divergences();
    let known: HashMap<&str, &str> = KNOWN_MISMATCHES.iter().copied().collect();
    let (mut matched, mut unsupported, mut skipped, mut diverging) = (0, 0, 0, 0);
    let mut reasons: HashMap<String, usize> = HashMap::new();
    let mut failures = Vec::new();
    for t in &tests {
        match run_test(&root, t) {
            Verdict::Match => matched += 1,
            Verdict::Unsupported(msg) => {
                unsupported += 1;
                *reasons.entry(msg).or_default() += 1;
            }
            Verdict::Skipped(why) => {
                skipped += 1;
                *reasons.entry(format!("skipped: {why}")).or_default() += 1;
            }
            Verdict::Mismatch(_) if divergent.contains(&t.id) => diverging += 1,
            Verdict::Mismatch(_) if known.contains_key(t.id.as_str()) => diverging += 1,
            Verdict::Mismatch(why) => failures.push(format!("{}\n  {why}", t.id)),
        }
    }
    let in_subset = matched + diverging + failures.len();
    eprintln!(
        "W3C query-evaluation tests: {} total, {in_subset} inside the baseline subset: {matched} match, \
         {diverging} reference divergences, {} mismatches; {unsupported} unsupported, {skipped} skipped",
        tests.len(),
        failures.len()
    );
    let mut reasons: Vec<_> = reasons.into_iter().collect();
    reasons.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (why, n) in reasons {
        eprintln!("  {n:4}  {why}");
    }
    assert!(failures.is_empty(), "{} mismatches:\n{}", failures.len(), failures.join("\n"));
}
