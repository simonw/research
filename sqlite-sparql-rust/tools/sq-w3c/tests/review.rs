//! Adversarial review tests for the W3C harness (comparison soundness).
//!
//! Each `review_*` test pins down a behaviour where the harness either rejects a result that
//! SPARQL 1.1 allows (false failure) or accepts one it forbids (false pass).
use oxrdf::{BlankNode, Dataset, Literal, NamedNode, NamedNodeRef, Term, Variable};
use oxrdfio::RdfFormat;
use sq_w3c::backend::{Backend, BackendError, BackendResult, QueryRequest, UpdateRequest};
use sq_w3c::compare::{compare_solutions, Numeric, RowOrder};
use sq_w3c::manifest::load_tests;
use sq_w3c::results::{QueryOutcome, Solutions, TextFormat};
use sq_w3c::runner::{run_test, Status};
use sq_w3c::suite::Suite;

type Row = Vec<Option<Term>>;

fn vars(names: &[&str]) -> Vec<Variable> {
    names.iter().map(|n| Variable::new_unchecked(*n)).collect()
}

fn sols(names: &[&str], rows: Vec<Row>) -> Solutions {
    Solutions { variables: vars(names), rows, ordered: true }
}

fn bnode(label: &str) -> Option<Term> {
    Some(BlankNode::new_unchecked(label).into())
}

fn string(s: &str) -> Option<Term> {
    Some(Literal::new_simple_literal(s).into())
}

fn typed(lex: &str, local: &str) -> Option<Term> {
    Some(Literal::new_typed_literal(lex, NamedNode::new_unchecked(format!("http://www.w3.org/2001/XMLSchema#{local}"))).into())
}

/// `SELECT ?x ?name { ?x :name ?name } ORDER BY ?x` where every ?x is a blank node.
///
/// SPARQL 1.1 §15.1 orders blank nodes before IRIs and literals but defines NO order among
/// blank nodes, so the two solutions below may come out in either order. The harness splits
/// runs on RDF-term inequality of the key, so two different blank nodes form two runs and the
/// engine's (equally valid) order is rejected.
#[test]
fn review_order_by_runs_treat_distinct_blank_node_keys_as_ties() {
    let expected = sols(&["x", "name"], vec![vec![bnode("a"), string("Eve")], vec![bnode("b"), string("Alice")]]);
    let actual = sols(&["x", "name"], vec![vec![bnode("p"), string("Alice")], vec![bnode("q"), string("Eve")]]);
    let order = RowOrder::Runs(vars(&["x"]));
    let verdict = compare_solutions(&expected, &actual, &order, false, Numeric::Exact);
    assert!(verdict.is_ok(), "a valid ORDER BY output was rejected: {verdict:?}");
}

/// `ORDER BY ?k` where `"1"^^xsd:integer` and `"1.0"^^xsd:decimal` are both keys: neither is
/// `<` the other (op:numeric-less-than), so they tie and their relative order is undefined.
/// The harness puts them in separate runs (different RDF terms) and rejects the swapped order.
#[test]
fn review_order_by_runs_treat_numerically_equal_keys_as_ties() {
    let expected = sols(
        &["k", "v"],
        vec![vec![typed("1", "integer"), string("a")], vec![typed("1.0", "decimal"), string("b")]],
    );
    let actual = sols(
        &["k", "v"],
        vec![vec![typed("1.0", "decimal"), string("b")], vec![typed("1", "integer"), string("a")]],
    );
    let order = RowOrder::Runs(vars(&["k"]));
    let verdict = compare_solutions(&expected, &actual, &order, false, Numeric::Exact);
    assert!(verdict.is_ok(), "a valid ORDER BY output was rejected: {verdict:?}");
}

/// A backend whose CSV output swaps the two projected columns (header and values).
struct SwappedCsv;

impl Backend for SwappedCsv {
    fn reset(&mut self) -> BackendResult<()> {
        Ok(())
    }
    fn load(&mut self, _: &[u8], _: RdfFormat, _: &str, _: Option<NamedNodeRef<'_>>) -> BackendResult<()> {
        Ok(())
    }
    fn query(&mut self, _: &QueryRequest<'_>) -> BackendResult<QueryOutcome> {
        Err(BackendError::Failed("only CSV text is produced".into()))
    }
    fn query_text(&mut self, _: &QueryRequest<'_>, format: TextFormat) -> BackendResult<String> {
        assert_eq!(format, TextFormat::Csv);
        Ok("b,a\r\n2,1\r\n".to_owned())
    }
    fn update(&mut self, _: &UpdateRequest<'_>) -> BackendResult<()> {
        Ok(())
    }
    fn dump(&mut self) -> BackendResult<Dataset> {
        Ok(Dataset::new())
    }
}

/// mf:CSVResultFormatTest with an EXPLICIT projection `SELECT ?a ?b`: the CSV header order is
/// fixed by the query (SPARQL 1.1 CSV/TSV results §2), so `b,a` is wrong output. The harness
/// re-aligns the actual columns on the expected header for every CSV test (meant for
/// `SELECT *`, whose order spargebra sorts) and therefore passes it.
#[test]
fn review_csv_format_test_rejects_wrong_explicit_column_order() {
    let dir = std::env::temp_dir().join(format!(
        "sq-w3c-review-csv-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    let files = [
        (
            "t/manifest.ttl",
            r#"@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
@prefix mf: <http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#> .
@prefix qt: <http://www.w3.org/2001/sw/DataAccess/tests/test-query#> .
@prefix : <http://example.org/t#> .
<> a mf:Manifest ; mf:entries (:csv) .
:csv a mf:CSVResultFormatTest ; mf:name "csv" ; mf:action [ qt:query <q.rq> ] ; mf:result <r.csv> .
"#,
        ),
        ("t/q.rq", "SELECT ?a ?b WHERE { BIND(1 AS ?a) BIND(2 AS ?b) }"),
        ("t/r.csv", "a,b\r\n1,2\r\n"),
    ];
    for (rel, text) in files {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    let suite = Suite::new(&dir).unwrap();
    let tests = load_tests(&suite, &[suite.url("t/manifest.ttl")]).unwrap();
    let outcome = run_test(&suite, &mut SwappedCsv, &tests[0]);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(outcome.status, Status::Fail, "columns in the wrong projection order were accepted: {outcome:?}");
}
