//! W3C test manifests: parsed with oxttl, following `mf:include` and `mf:entries` lists.
use std::collections::HashSet;

use oxrdf::{Graph, NamedNodeRef, NamedOrBlankNodeRef, TermRef};
use oxttl::TurtleParser;

use crate::suite::Suite;
use crate::vocab::{dawgt, mf, qt, rdf, rdfs, sd, ut};

/// Entry points of a full run, relative to the suite directory.
pub const DEFAULT_MANIFESTS: [&str; 2] = ["sparql11/manifest-all.ttl", "sparql10/manifest-evaluation.ttl"];

/// Manifest directories whose tests need features outside a local query engine (§12.2).
const SKIPPED_DIRECTORIES: [(&str, &str); 6] = [
    ("entailment", "entailment"),
    ("service", "service"),
    ("protocol", "protocol"),
    ("graph-store-protocol", "graph-store-protocol"),
    ("http-rdf-update", "http-rdf-update"),
    ("service-description", "service-description"),
];

/// Approval states whose tests are not run.
const SKIPPED_APPROVALS: [&str; 4] = ["Rejected", "Withdrawn", "Obsoleted", "NotApproved"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestCase {
    /// Test IRI (the key of `known_failures.txt`).
    pub id: String,
    pub name: String,
    pub comment: Option<String>,
    /// Suite-relative path of the manifest that lists the test.
    pub manifest: String,
    /// Local name of the `dawgt:approval` value, if any.
    pub approval: Option<String>,
    pub kind: TestKind,
    /// Why the test is not run (approval, category or test type).
    pub skip: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TestKind {
    Query(QueryTest),
    /// `mf:CSVResultFormatTest`: the CSV/TSV text itself is compared.
    ResultFormat(QueryTest),
    Update(UpdateTest),
    /// Any other test type (syntax, protocol, …): recorded as skipped.
    Other(String),
}

/// A file loaded as a named graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphFile {
    /// Public IRI of the file.
    pub file: String,
    /// Graph name: the file IRI, or the `rdfs:label` of an update test's graph.
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryTest {
    pub query: String,
    /// Files merged into the default graph.
    pub data: Vec<String>,
    pub graph_data: Vec<GraphFile>,
    pub result: Option<String>,
    /// `mf:resultCardinality mf:LaxCardinality`: compare as sets.
    pub lax_cardinality: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct DatasetFiles {
    pub data: Vec<String>,
    pub graph_data: Vec<GraphFile>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateTest {
    pub request: String,
    pub before: DatasetFiles,
    pub after: DatasetFiles,
}

/// Loads every test reachable from the given manifests (public IRIs), in manifest order.
/// Each manifest is visited once even if several manifests include it.
pub fn load_tests(suite: &Suite, manifests: &[String]) -> Result<Vec<TestCase>, String> {
    let mut loader = Loader { suite, visited: HashSet::new(), tests: Vec::new() };
    for manifest in manifests {
        loader.visit(manifest)?;
    }
    Ok(loader.tests)
}

struct Loader<'a> {
    suite: &'a Suite,
    visited: HashSet<String>,
    tests: Vec<TestCase>,
}

impl Loader<'_> {
    fn visit(&mut self, url: &str) -> Result<(), String> {
        if !self.visited.insert(url.to_owned()) {
            return Ok(());
        }
        let bytes = self.suite.read(url)?;
        let mut graph = Graph::new();
        let parser = TurtleParser::new().with_base_iri(url).map_err(|e| format!("bad manifest IRI {url}: {e}"))?;
        for triple in parser.for_slice(&bytes) {
            graph.insert(&triple.map_err(|e| format!("cannot parse manifest {url}: {e}"))?);
        }
        let relative = self.suite.relative(url).unwrap_or(url).to_owned();

        let this = NamedNodeRef::new(url).map_err(|e| format!("bad manifest IRI {url}: {e}"))?;
        let mut roots: Vec<NamedOrBlankNodeRef<'_>> = vec![this.into()];
        for s in graph.subjects_for_predicate_object(rdf::TYPE, mf::MANIFEST) {
            if !roots.contains(&s) {
                roots.push(s);
            }
        }

        let mut includes = Vec::new();
        for root in &roots {
            for list in graph.objects_for_subject_predicate(*root, mf::ENTRIES) {
                for entry in rdf_list(&graph, list)? {
                    let entry = match entry {
                        TermRef::NamedNode(n) => NamedOrBlankNodeRef::from(n),
                        TermRef::BlankNode(b) => b.into(),
                        TermRef::Literal(l) => return Err(format!("{url}: literal {l} in mf:entries")),
                    };
                    self.tests.push(read_test(&graph, entry, &relative)?);
                }
            }
            for list in graph.objects_for_subject_predicate(*root, mf::INCLUDE) {
                for item in rdf_list(&graph, list)? {
                    match item {
                        TermRef::NamedNode(n) => includes.push(n.as_str().to_owned()),
                        other => return Err(format!("{url}: mf:include item {other} is not an IRI")),
                    }
                }
            }
        }
        for include in includes {
            self.visit(&include)?;
        }
        Ok(())
    }
}

/// Items of an RDF collection.
fn rdf_list<'a>(graph: &'a Graph, head: TermRef<'a>) -> Result<Vec<TermRef<'a>>, String> {
    let mut items = Vec::new();
    let mut node = head;
    let mut seen = HashSet::new();
    loop {
        let subject = match node {
            TermRef::NamedNode(n) if n == rdf::NIL => return Ok(items),
            TermRef::NamedNode(n) => NamedOrBlankNodeRef::from(n),
            TermRef::BlankNode(b) => b.into(),
            TermRef::Literal(l) => return Err(format!("literal {l} in an RDF list")),
        };
        if !seen.insert(subject) {
            return Err("cyclic RDF list".to_owned());
        }
        if let Some(first) = graph.object_for_subject_predicate(subject, rdf::FIRST) {
            items.push(first);
        }
        node = graph.object_for_subject_predicate(subject, rdf::REST).ok_or("RDF list without rdf:rest")?;
    }
}

fn subject_of(t: TermRef<'_>) -> Option<NamedOrBlankNodeRef<'_>> {
    match t {
        TermRef::NamedNode(n) => Some(n.into()),
        TermRef::BlankNode(b) => Some(b.into()),
        TermRef::Literal(_) => None,
    }
}

fn iri_objects(graph: &Graph, s: NamedOrBlankNodeRef<'_>, p: NamedNodeRef<'_>) -> Vec<String> {
    graph
        .objects_for_subject_predicate(s, p)
        .filter_map(|o| match o {
            TermRef::NamedNode(n) => Some(n.as_str().to_owned()),
            _ => None,
        })
        .collect()
}

fn literal_object(graph: &Graph, s: NamedOrBlankNodeRef<'_>, p: NamedNodeRef<'_>) -> Option<String> {
    graph.objects_for_subject_predicate(s, p).find_map(|o| match o {
        TermRef::Literal(l) => Some(l.value().to_owned()),
        _ => None,
    })
}

fn local_name(iri: &str) -> &str {
    iri.rsplit(['#', '/']).next().unwrap_or(iri)
}

/// `qt:graphData` / `ut:graphData` values: a file IRI, or a node `[ ut:graph <file> ; rdfs:label "name" ]`.
fn graph_files(graph: &Graph, s: NamedOrBlankNodeRef<'_>, p: NamedNodeRef<'_>) -> Result<Vec<GraphFile>, String> {
    let mut files = Vec::new();
    for o in graph.objects_for_subject_predicate(s, p) {
        match o {
            TermRef::NamedNode(file) => {
                files.push(GraphFile { file: file.as_str().to_owned(), name: file.as_str().to_owned() })
            }
            TermRef::BlankNode(b) => {
                let file = iri_objects(graph, b.into(), ut::GRAPH)
                    .into_iter()
                    .next()
                    .ok_or_else(|| format!("graph data node {b} without ut:graph"))?;
                let name = literal_object(graph, b.into(), rdfs::LABEL).unwrap_or_else(|| file.clone());
                files.push(GraphFile { file, name });
            }
            TermRef::Literal(l) => return Err(format!("literal {l} as graph data")),
        }
    }
    Ok(files)
}

fn dataset_files(
    graph: &Graph,
    node: NamedOrBlankNodeRef<'_>,
    data: NamedNodeRef<'_>,
    graph_data: NamedNodeRef<'_>,
) -> Result<DatasetFiles, String> {
    Ok(DatasetFiles { data: iri_objects(graph, node, data), graph_data: graph_files(graph, node, graph_data)? })
}

fn read_test(graph: &Graph, node: NamedOrBlankNodeRef<'_>, manifest: &str) -> Result<TestCase, String> {
    let id = match node {
        NamedOrBlankNodeRef::NamedNode(n) => n.as_str().to_owned(),
        NamedOrBlankNodeRef::BlankNode(b) => format!("{manifest}#_:{}", b.as_str()),
    };
    let name = literal_object(graph, node, mf::NAME).unwrap_or_else(|| local_name(&id).to_owned());
    let comment = literal_object(graph, node, rdfs::COMMENT);
    let approval = iri_objects(graph, node, dawgt::APPROVAL).first().map(|a| local_name(a).to_owned());
    let types = iri_objects(graph, node, rdf::TYPE);
    let action = graph.object_for_subject_predicate(node, mf::ACTION).and_then(subject_of);
    let result = graph.object_for_subject_predicate(node, mf::RESULT);
    let has = |t: NamedNodeRef<'_>| types.iter().any(|x| x == t.as_str());

    let query_test = || -> Result<QueryTest, String> {
        let action = action.ok_or_else(|| format!("{id}: no mf:action"))?;
        let query = iri_objects(graph, action, qt::QUERY)
            .into_iter()
            .next()
            .ok_or_else(|| format!("{id}: no qt:query"))?;
        Ok(QueryTest {
            query,
            data: iri_objects(graph, action, qt::DATA),
            graph_data: graph_files(graph, action, qt::GRAPH_DATA)?,
            result: match result {
                Some(TermRef::NamedNode(n)) => Some(n.as_str().to_owned()),
                _ => None,
            },
            lax_cardinality: graph
                .objects_for_subject_predicate(node, mf::RESULT_CARDINALITY)
                .any(|o| o == mf::LAX_CARDINALITY.into()),
        })
    };

    let untyped_query =
        types.is_empty() && action.is_some_and(|a| graph.object_for_subject_predicate(a, qt::QUERY).is_some());
    let kind = if has(mf::QUERY_EVALUATION_TEST) || untyped_query {
        TestKind::Query(query_test()?)
    } else if has(mf::CSV_RESULT_FORMAT_TEST) {
        TestKind::ResultFormat(query_test()?)
    } else if has(mf::UPDATE_EVALUATION_TEST) {
        let action = action.ok_or_else(|| format!("{id}: no mf:action"))?;
        let request = iri_objects(graph, action, ut::REQUEST)
            .into_iter()
            .next()
            .ok_or_else(|| format!("{id}: no ut:request"))?;
        let after = match result.and_then(subject_of) {
            Some(r) => dataset_files(graph, r, ut::DATA, ut::GRAPH_DATA)?,
            None => return Err(format!("{id}: update test without mf:result")),
        };
        TestKind::Update(UpdateTest { request, before: dataset_files(graph, action, ut::DATA, ut::GRAPH_DATA)?, after })
    } else {
        TestKind::Other(types.first().map_or("untyped", |t| local_name(t)).to_owned())
    };

    let entailment = [Some(node), action]
        .into_iter()
        .flatten()
        .any(|n| graph.object_for_subject_predicate(n, sd::ENTAILMENT_REGIME).is_some());
    let service_data = action.is_some_and(|a| graph.object_for_subject_predicate(a, qt::SERVICE_DATA).is_some());
    let directory = manifest.split('/').rev().nth(1).unwrap_or("");
    let skip = if let Some(a) = approval.as_deref().filter(|a| SKIPPED_APPROVALS.contains(a)) {
        Some(format!("approval: {a}"))
    } else if let Some((_, category)) = SKIPPED_DIRECTORIES.iter().find(|(dir, _)| *dir == directory) {
        Some(format!("category: {category}"))
    } else if entailment {
        Some("category: entailment".to_owned())
    } else if service_data {
        Some("category: service".to_owned())
    } else if let TestKind::Other(t) = &kind {
        Some(format!("test type: {t}"))
    } else {
        None
    };

    Ok(TestCase { id, name, comment, manifest: manifest.to_owned(), approval, kind, skip })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &std::path::Path, rel: &str, text: &str) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    const PREFIXES: &str = r#"
@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix mf: <http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#> .
@prefix qt: <http://www.w3.org/2001/sw/DataAccess/tests/test-query#> .
@prefix ut: <http://www.w3.org/2009/sparql/tests/test-update#> .
@prefix dawgt: <http://www.w3.org/2001/sw/DataAccess/tests/test-dawg#> .
@prefix : <http://example.org/tests#> .
"#;

    fn fixture() -> (tempdir::Dir, Suite) {
        let dir = tempdir::Dir::new("manifest");
        write(
            dir.path(),
            "top/manifest.ttl",
            &format!("{PREFIXES} <> a mf:Manifest ; mf:include (<../q/manifest.ttl> <../u/manifest.ttl> <../q/manifest.ttl>) ."),
        );
        write(
            dir.path(),
            "q/manifest.ttl",
            &format!(
                r#"{PREFIXES}
<> a mf:Manifest ; mf:entries (:q1 :q2 :csv :syn :rej) .
:q1 a mf:QueryEvaluationTest ; mf:name "q1" ; dawgt:approval dawgt:Approved ;
    mf:action [ qt:query <q1.rq> ; qt:data <d1.ttl>, <d2.ttl> ; qt:graphData <g1.ttl> ] ;
    mf:result <q1.srx> .
:q2 a mf:QueryEvaluationTest ; mf:name "q2" ; mf:resultCardinality mf:LaxCardinality ;
    mf:action [ qt:query <q2.rq> ] ; mf:result <q2.ttl> .
:csv a mf:CSVResultFormatTest ; mf:name "csv" ; mf:action [ qt:query <c.rq> ] ; mf:result <c.csv> .
:syn a mf:PositiveSyntaxTest ; mf:name "syntax" ; mf:action <s.rq> .
:rej a mf:QueryEvaluationTest ; mf:name "rejected" ; dawgt:approval dawgt:Withdrawn ;
    mf:action [ qt:query <r.rq> ] ; mf:result <r.srx> .
"#
            ),
        );
        write(
            dir.path(),
            "u/manifest.ttl",
            &format!(
                r#"{PREFIXES}
<> a mf:Manifest ; mf:entries (:u1) .
:u1 a mf:UpdateEvaluationTest ; mf:name "u1" ;
    mf:action [ ut:request <u1.ru> ; ut:data <pre.ttl> ;
                ut:graphData [ ut:graph <g.ttl> ; rdfs:label "http://example.org/g1" ] ] ;
    mf:result [ ut:graphData [ ut:graph <post.ttl> ; rdfs:label "http://example.org/g1" ] ] .
"#
            ),
        );
        write(dir.path(), "entailment/manifest.ttl", &format!(
            "{PREFIXES} <> a mf:Manifest ; mf:entries (:e1) . :e1 a mf:QueryEvaluationTest ; mf:action [ qt:query <e.rq> ] ; mf:result <e.srx> ."
        ));
        let suite = Suite::new(dir.path()).unwrap();
        (dir, suite)
    }

    /// Minimal self-cleaning temporary directory (no extra dependency).
    mod tempdir {
        pub struct Dir(std::path::PathBuf);
        impl Dir {
            pub fn new(tag: &str) -> Self {
                let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
                let path = std::env::temp_dir().join(format!("sq-w3c-{tag}-{}-{n}", std::process::id()));
                std::fs::create_dir_all(&path).unwrap();
                Dir(path)
            }
            pub fn path(&self) -> &std::path::Path {
                &self.0
            }
        }
        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    #[test]
    fn follows_includes_once_and_reads_every_kind() {
        let (_dir, suite) = fixture();
        let tests = load_tests(&suite, &[suite.url("top/manifest.ttl"), suite.url("entailment/manifest.ttl")]).unwrap();
        let ids: Vec<&str> = tests.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "http://example.org/tests#q1",
                "http://example.org/tests#q2",
                "http://example.org/tests#csv",
                "http://example.org/tests#syn",
                "http://example.org/tests#rej",
                "http://example.org/tests#u1",
                "http://example.org/tests#e1",
            ]
        );
        let base = "https://w3c.github.io/rdf-tests/sparql/";
        let TestKind::Query(q1) = &tests[0].kind else { panic!("{:?}", tests[0].kind) };
        assert_eq!(q1.query, format!("{base}q/q1.rq"));
        let mut data = q1.data.clone();
        data.sort();
        assert_eq!(data, [format!("{base}q/d1.ttl"), format!("{base}q/d2.ttl")]);
        assert_eq!(q1.graph_data, [GraphFile { file: format!("{base}q/g1.ttl"), name: format!("{base}q/g1.ttl") }]);
        assert_eq!(q1.result.as_deref(), Some(format!("{base}q/q1.srx").as_str()));
        assert!(!q1.lax_cardinality);
        assert_eq!(tests[0].manifest, "q/manifest.ttl");
        assert_eq!(tests[0].approval.as_deref(), Some("Approved"));
        assert_eq!(tests[0].skip, None);

        let TestKind::Query(q2) = &tests[1].kind else { panic!() };
        assert!(q2.lax_cardinality);
        assert!(matches!(tests[2].kind, TestKind::ResultFormat(_)));
        assert_eq!(tests[3].skip.as_deref(), Some("test type: PositiveSyntaxTest"));
        assert_eq!(tests[4].skip.as_deref(), Some("approval: Withdrawn"));

        let TestKind::Update(u1) = &tests[5].kind else { panic!() };
        assert_eq!(u1.request, format!("{base}u/u1.ru"));
        assert_eq!(u1.before.data, [format!("{base}u/pre.ttl")]);
        assert_eq!(u1.before.graph_data, [GraphFile { file: format!("{base}u/g.ttl"), name: "http://example.org/g1".into() }]);
        assert!(u1.after.data.is_empty());
        assert_eq!(u1.after.graph_data[0].file, format!("{base}u/post.ttl"));

        assert_eq!(tests[6].skip.as_deref(), Some("category: entailment"));
    }

    #[test]
    fn missing_manifest_is_an_error() {
        let (_dir, suite) = fixture();
        assert!(load_tests(&suite, &[suite.url("nope/manifest.ttl")]).is_err());
    }
}
