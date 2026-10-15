//! W3C SPARQL 1.0 / 1.1 query-evaluation tests inside the baseline's subset: every test whose
//! query the translator accepts must give spareval's answer, except the tests listed in
//! tools/sq-w3c/reference_divergences.txt (spareval bugs), which must give the W3C expected
//! results instead.
//!
//! The suite is read from `$SQ_W3C_DIR` (default: the session scratchpad copy); the test is
//! skipped with a note when it is absent. Run with `--nocapture` for the report.
mod common;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use common::{Baseline, DataFile, Engine, Expect, Outcome, compare};
use oxrdf::{Graph, NamedOrBlankNodeRef, TermRef, vocab::rdf};
use oxrdfio::RdfFormat;
use sparesults::{QueryResultsFormat, QueryResultsParser, SliceQueryResultsParserOutput};

const DEFAULT_SUITE: &str = concat!(
    "/tmp/claude-0/-home-user-research/82148577-c93b-5a93-8757-e7fe894ecb5e/scratchpad/",
    "w3c/rdf-tests/sparql"
);
const SUITE_URL: &str = "https://w3c.github.io/rdf-tests/sparql/";
const MF: &str = "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#";
const QT: &str = "http://www.w3.org/2001/sw/DataAccess/tests/test-query#";
const RS: &str = "http://www.w3.org/2001/sw/DataAccess/tests/result-set#";

/// In-subset tests whose baseline answer is knowingly wrong, with the reason.
const KNOWN_MISMATCHES: &[(&str, &str)] = &[(
    concat!(
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/optional-filter/manifest",
        "#dawg-optional-filter-005-not-simplified"
    ),
    "spargebra 0.4.7 parses OPTIONAL { { P FILTER(F) } } as LeftJoin(_, P, F); the baseline translates \
     spargebra's algebra (sq-plan and the native engine inherit the same reading)",
)];

struct TestCase {
    id: String,
    query: String,
    data: Vec<String>,
    graph_data: Vec<String>,
    result: Option<String>,
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
            result: iris(&g, test.into(), &format!("{MF}result")).pop(),
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

/// The W3C expected results (SPARQL XML / JSON / TSV, or an `rs:ResultSet` graph).
fn expected(root: &Path, url: &str) -> Result<Outcome, String> {
    let path = local_path(root, url).ok_or("result outside the suite")?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let format = match url.rsplit('.').next() {
        Some("srx") => QueryResultsFormat::Xml,
        Some("srj") => QueryResultsFormat::Json,
        Some("tsv") => QueryResultsFormat::Tsv,
        Some("ttl" | "rdf") => return result_set(&bytes, url),
        _ => return Err(format!("unsupported result format: {url}")),
    };
    match QueryResultsParser::from_format(format).for_slice(&bytes).map_err(|e| e.to_string())? {
        SliceQueryResultsParserOutput::Boolean(b) => Ok(Outcome::Boolean(b)),
        SliceQueryResultsParserOutput::Solutions(sols) => {
            let vars: Vec<String> = sols.variables().iter().map(|v| v.as_str().to_owned()).collect();
            let mut rows = Vec::new();
            for s in sols {
                let s = s.map_err(|e| e.to_string())?;
                rows.push(vars.iter().map(|v| s.get(v.as_str()).cloned()).collect());
            }
            Ok(Outcome::Solutions { vars, rows })
        }
    }
}

/// An `rs:ResultSet` graph (rows ordered by `rs:index` when present).
fn result_set(bytes: &[u8], url: &str) -> Result<Outcome, String> {
    let format = if url.ends_with(".rdf") { RdfFormat::RdfXml } else { RdfFormat::Turtle };
    let mut g = Graph::new();
    for t in oxrdfio::RdfParser::from_format(format).with_base_iri(url).map_err(|e| e.to_string())?.for_slice(bytes) {
        let q = t.map_err(|e| e.to_string())?;
        g.insert(oxrdf::TripleRef::new(&q.subject, &q.predicate, &q.object));
    }
    let rs = |local: &str| nn(&format!("{RS}{local}"));
    let set = g
        .subjects_for_predicate_object(rdf::TYPE, &rs("ResultSet"))
        .next()
        .ok_or("no rs:ResultSet")?;
    if let Some(TermRef::Literal(b)) = g.object_for_subject_predicate(set, &rs("boolean")) {
        return Ok(Outcome::Boolean(b.value() == "true"));
    }
    let literal = |t: TermRef<'_>| match t {
        TermRef::Literal(l) => Some(l.value().to_owned()),
        _ => None,
    };
    let vars: Vec<String> =
        g.objects_for_subject_predicate(set, &rs("resultVariable")).filter_map(literal).collect();
    let mut rows = Vec::new();
    for sol in g.objects_for_subject_predicate(set, &rs("solution")).filter_map(subject) {
        let index = g
            .object_for_subject_predicate(sol, &rs("index"))
            .and_then(literal)
            .and_then(|i| i.parse::<i64>().ok());
        let mut row = vec![None; vars.len()];
        for b in g.objects_for_subject_predicate(sol, &rs("binding")).filter_map(subject) {
            let var = g.object_for_subject_predicate(b, &rs("variable")).and_then(literal);
            let value = g.object_for_subject_predicate(b, &rs("value"));
            if let (Some(var), Some(value)) = (var, value)
                && let Some(i) = vars.iter().position(|v| *v == var)
            {
                row[i] = Some(value.into_owned());
            }
        }
        rows.push((index, row));
    }
    rows.sort_by_key(|(i, _)| *i);
    Ok(Outcome::Solutions { vars, rows: rows.into_iter().map(|(_, r)| r).collect() })
}

enum Verdict {
    Match,
    Mismatch(String),
    Unsupported(String),
    Skipped(String),
}

fn run_test(root: &Path, t: &TestCase, reference_is_wrong: bool) -> Verdict {
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
        use spargebra::Query::{Ask, Select};
        if t.data.is_empty()
            && let Select { dataset: Some(ds), .. } | Ask { dataset: Some(ds), .. } = &query
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
        Ok(Baseline::Unsupported(msg)) => { if msg.contains("nesting") { eprintln!("DEBUG {}", t.id); } return Verdict::Unsupported(msg) }
        Ok(Baseline::Ran { sql, outcome }) => (sql, outcome),
        Err(e) => return Verdict::Mismatch(format!("baseline failed: {e}")),
    };
    let reference = if reference_is_wrong {
        match t.result.as_deref().map(|r| expected(root, r)) {
            Some(Ok(r)) => r,
            Some(Err(e)) => return Verdict::Skipped(format!("expected results: {e}")),
            None => return Verdict::Skipped("no expected results".into()),
        }
    } else {
        match engine.reference(&query) {
            Ok(r) => r,
            Err(e) => return Verdict::Skipped(format!("reference: {e}")),
        }
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
    let (mut matched, mut matched_expected, mut unsupported, mut skipped, mut known_count) = (0, 0, 0, 0, 0);
    let mut reasons: HashMap<String, usize> = HashMap::new();
    let mut failures = Vec::new();
    for t in &tests {
        let reference_is_wrong = divergent.contains(&t.id);
        match run_test(&root, t, reference_is_wrong) {
            Verdict::Match if reference_is_wrong => matched_expected += 1,
            Verdict::Match => matched += 1,
            Verdict::Unsupported(msg) => {
                unsupported += 1;
                *reasons.entry(msg).or_default() += 1;
            }
            Verdict::Skipped(why) => {
                skipped += 1;
                *reasons.entry(format!("skipped: {why}")).or_default() += 1;
            }
            Verdict::Mismatch(why) if known.contains_key(t.id.as_str()) => {
                known_count += 1;
                eprintln!("known mismatch {} ({}):\n  {why}", t.id, known[t.id.as_str()]);
            }
            Verdict::Mismatch(why) => failures.push(format!("{}\n  {why}", t.id)),
        }
    }
    let in_subset = matched + matched_expected + known_count + failures.len();
    eprintln!(
        "W3C query-evaluation tests: {} total, {in_subset} inside the baseline subset: {} match \
         ({matched} vs spareval, {matched_expected} vs the W3C expected results where spareval is \
         wrong), {known_count} known mismatches, {} unexpected mismatches; {unsupported} unsupported, \
         {skipped} skipped",
        tests.len(),
        matched + matched_expected,
        failures.len()
    );
    let mut reasons: Vec<_> = reasons.into_iter().collect();
    reasons.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (why, n) in reasons {
        eprintln!("  {n:4}  {why}");
    }
    assert!(failures.is_empty(), "{} mismatches:\n{}", failures.len(), failures.join("\n"));
}
