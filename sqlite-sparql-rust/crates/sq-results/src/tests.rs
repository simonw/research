//! Golden tests of the writers against hand-written fake `SolutionSource` / `TripleSource`s.
use std::sync::Arc;

use sq_core::eval_api::{SolutionSource, TripleSource};
use sq_core::term::{Literal, Term};
use sq_core::{Error, Result, Tag, TermId, BATCH_ROWS};

use crate::{solution_json, write_boolean, write_solutions, write_triples, ResultFormat};

const XSD_INTEGER: &str = "http://www.w3.org/2001/XMLSchema#integer";

/// A tiny id <-> term table shared by the fakes: id = dictionary row (1-based) of the term.
#[derive(Default)]
struct FakeDict {
    terms: Vec<Arc<Term>>,
    decode_calls: usize,
}

impl FakeDict {
    fn id(&mut self, t: &Term) -> TermId {
        let row = match self.terms.iter().position(|x| **x == *t) {
            Some(i) => i,
            None => {
                self.terms.push(Arc::new(t.clone()));
                self.terms.len() - 1
            }
        };
        TermId::from_dict(row as i64 + 1, Tag::Iri)
    }

    fn decode(&mut self, ids: &[TermId], out: &mut Vec<Option<Arc<Term>>>) -> Result<()> {
        self.decode_calls += 1;
        for id in ids {
            if *id == TermId::UNBOUND {
                out.push(None);
            } else if id.is_dict() {
                let term = self.terms.get(id.dict_row() as usize - 1).ok_or_else(|| Error::internal("unknown id"))?;
                out.push(Some(term.clone()));
            } else {
                return Err(Error::internal(format!("fake cannot decode {id:?}")));
            }
        }
        Ok(())
    }
}

struct FakeSolutions {
    vars: Vec<String>,
    rows: Vec<Vec<TermId>>,
    pos: usize,
    dict: FakeDict,
    next_rows_calls: usize,
    max_rows_requested: usize,
}

impl FakeSolutions {
    fn new(vars: &[&str], rows: &[Vec<Option<Term>>]) -> Self {
        let mut dict = FakeDict::default();
        let rows = rows
            .iter()
            .map(|r| r.iter().map(|t| t.as_ref().map_or(TermId::UNBOUND, |t| dict.id(t))).collect())
            .collect();
        FakeSolutions {
            vars: vars.iter().map(|v| v.to_string()).collect(),
            rows,
            pos: 0,
            dict,
            next_rows_calls: 0,
            max_rows_requested: 0,
        }
    }
}

impl SolutionSource for FakeSolutions {
    fn variables(&self) -> &[String] {
        &self.vars
    }

    fn next_rows(&mut self, out: &mut Vec<TermId>, max_rows: usize) -> Result<usize> {
        self.next_rows_calls += 1;
        self.max_rows_requested = self.max_rows_requested.max(max_rows);
        let end = (self.pos + max_rows).min(self.rows.len());
        for row in &self.rows[self.pos..end] {
            out.extend_from_slice(row);
        }
        let n = end - self.pos;
        self.pos = end;
        Ok(n)
    }

    fn decode(&mut self, ids: &[TermId], out: &mut Vec<Option<Arc<Term>>>) -> Result<()> {
        self.dict.decode(ids, out)
    }
}

struct FakeTriples {
    triples: Vec<[TermId; 3]>,
    pos: usize,
    dict: FakeDict,
}

impl FakeTriples {
    fn new(triples: &[[Term; 3]]) -> Self {
        let mut dict = FakeDict::default();
        let triples = triples.iter().map(|t| [dict.id(&t[0]), dict.id(&t[1]), dict.id(&t[2])]).collect();
        FakeTriples { triples, pos: 0, dict }
    }
}

impl TripleSource for FakeTriples {
    fn next_triples(&mut self, out: &mut Vec<[TermId; 3]>, max: usize) -> Result<usize> {
        let end = (self.pos + max).min(self.triples.len());
        out.extend_from_slice(&self.triples[self.pos..end]);
        let n = end - self.pos;
        self.pos = end;
        Ok(n)
    }

    fn decode(&mut self, ids: &[TermId], out: &mut Vec<Option<Arc<Term>>>) -> Result<()> {
        self.dict.decode(ids, out)
    }
}

fn iri(s: &str) -> Term {
    Term::Iri(s.into())
}

fn bnode(s: &str) -> Term {
    Term::BNode(s.into())
}

fn lit(s: &str) -> Term {
    Term::Literal(Literal::simple(s))
}

/// The golden solution sequence: every term kind, an unbound value and characters that need
/// escaping in every format.
fn golden_rows() -> Vec<Vec<Option<Term>>> {
    vec![
        vec![Some(iri("http://ex/s1")), Some(lit("plain"))],
        vec![Some(bnode("b1")), Some(Term::Literal(Literal::lang("chat", "fr")))],
        vec![Some(iri("http://ex/s2")), Some(Term::Literal(Literal::typed("42", XSD_INTEGER)))],
        vec![Some(iri("http://ex/s3")), None],
        vec![Some(iri("http://ex/s4")), Some(lit("say \"hi\",\tnow\nok<&>"))],
    ]
}

fn render_solutions(src: &mut dyn SolutionSource, fmt: ResultFormat) -> String {
    let mut buf = Vec::new();
    write_solutions(src, fmt, &mut buf).unwrap();
    String::from_utf8(buf).unwrap()
}

fn render_golden(fmt: ResultFormat) -> String {
    render_solutions(&mut FakeSolutions::new(&["s", "o"], &golden_rows()), fmt)
}

#[test]
fn golden_json() {
    assert_eq!(
        render_golden(ResultFormat::Json),
        concat!(
            r#"{"head":{"vars":["s","o"]},"results":{"bindings":["#,
            r#"{"s":{"type":"uri","value":"http://ex/s1"},"o":{"type":"literal","value":"plain"}},"#,
            r#"{"s":{"type":"bnode","value":"b1"},"o":{"type":"literal","value":"chat","xml:lang":"fr"}},"#,
            r#"{"s":{"type":"uri","value":"http://ex/s2"},"o":{"type":"literal","value":"42","datatype":"http://www.w3.org/2001/XMLSchema#integer"}},"#,
            r#"{"s":{"type":"uri","value":"http://ex/s3"}},"#,
            r#"{"s":{"type":"uri","value":"http://ex/s4"},"o":{"type":"literal","value":"say \"hi\",\tnow\nok<&>"}}"#,
            r#"]}}"#
        )
    );
}

#[test]
fn golden_xml() {
    assert_eq!(
        render_golden(ResultFormat::Xml),
        concat!(
            r#"<?xml version="1.0"?><sparql xmlns="http://www.w3.org/2005/sparql-results#">"#,
            r#"<head><variable name="s"/><variable name="o"/></head><results>"#,
            r#"<result><binding name="s"><uri>http://ex/s1</uri></binding><binding name="o"><literal>plain</literal></binding></result>"#,
            r#"<result><binding name="s"><bnode>b1</bnode></binding><binding name="o"><literal xml:lang="fr">chat</literal></binding></result>"#,
            r#"<result><binding name="s"><uri>http://ex/s2</uri></binding><binding name="o"><literal datatype="http://www.w3.org/2001/XMLSchema#integer">42</literal></binding></result>"#,
            r#"<result><binding name="s"><uri>http://ex/s3</uri></binding></result>"#,
            "<result><binding name=\"s\"><uri>http://ex/s4</uri></binding><binding name=\"o\"><literal>say &quot;hi&quot;,\tnow\nok&lt;&amp;&gt;</literal></binding></result>",
            r#"</results></sparql>"#
        )
    );
}

#[test]
fn golden_csv() {
    assert_eq!(
        render_golden(ResultFormat::Csv),
        concat!(
            "s,o\r\n",
            "http://ex/s1,plain\r\n",
            "_:b1,chat\r\n",
            "http://ex/s2,42\r\n",
            "http://ex/s3,\r\n",
            "http://ex/s4,\"say \"\"hi\"\",\tnow\nok<&>\"\r\n",
        )
    );
}

#[test]
fn golden_tsv() {
    assert_eq!(
        render_golden(ResultFormat::Tsv),
        concat!(
            "?s\t?o\n",
            "<http://ex/s1>\t\"plain\"\n",
            "_:b1\t\"chat\"@fr\n",
            "<http://ex/s2>\t42\n",
            "<http://ex/s3>\t\n",
            "<http://ex/s4>\t\"say \\\"hi\\\",\\tnow\\nok<&>\"\n",
        )
    );
}

#[test]
fn solution_json_matches_the_json_writer() {
    let rows = golden_rows();
    let full: serde_json::Value = serde_json::from_str(&render_golden(ResultFormat::Json)).unwrap();
    let bindings = full["results"]["bindings"].as_array().unwrap();
    let vars = ["s".to_string(), "o".to_string()];
    assert_eq!(bindings.len(), rows.len());
    for (row, expected) in rows.iter().zip(bindings) {
        let row: Vec<Option<Arc<Term>>> = row.iter().map(|t| t.clone().map(Arc::new)).collect();
        let actual: serde_json::Value = serde_json::from_str(&solution_json(&vars, &row)).unwrap();
        assert_eq!(&actual, expected);
    }
}

#[test]
fn pulls_and_decodes_in_batches() {
    let total = 2 * BATCH_ROWS + 17;
    let rows: Vec<Vec<Option<Term>>> = (0..total).map(|i| vec![Some(iri(&format!("http://ex/{}", i % 7)))]).collect();
    let mut src = FakeSolutions::new(&["x"], &rows);
    let out = render_solutions(&mut src, ResultFormat::Tsv);
    assert_eq!(out.lines().count(), total + 1);
    assert_eq!(out.lines().nth(total), Some("<http://ex/6>"), "{}", (total - 1) % 7);
    assert_eq!(src.max_rows_requested, BATCH_ROWS);
    assert_eq!(src.next_rows_calls, 4, "3 batches + the exhausting call");
    assert_eq!(src.dict.decode_calls, 3, "one decode per batch");
}

#[test]
fn zero_width_solutions() {
    let mut src = FakeSolutions::new(&[], &[vec![], vec![]]);
    assert_eq!(render_solutions(&mut src, ResultFormat::Json), r#"{"head":{"vars":[]},"results":{"bindings":[{},{}]}}"#);
    let mut empty = FakeSolutions::new(&["a"], &[]);
    assert_eq!(render_solutions(&mut empty, ResultFormat::Csv), "a\r\n");
}

#[test]
fn error_group_key_is_written_as_unbound() {
    let mut src = FakeSolutions::new(&["k", "n"], &[vec![None, Some(lit("x"))]]);
    src.rows[0][0] = TermId::ERROR;
    assert_eq!(render_solutions(&mut src, ResultFormat::Tsv), "?k\t?n\n\t\"x\"\n");
}

#[test]
fn source_errors_propagate() {
    struct Failing;
    impl SolutionSource for Failing {
        fn variables(&self) -> &[String] {
            &[]
        }
        fn next_rows(&mut self, _: &mut Vec<TermId>, _: usize) -> Result<usize> {
            Err(Error::Interrupted)
        }
        fn decode(&mut self, _: &[TermId], _: &mut Vec<Option<Arc<Term>>>) -> Result<()> {
            Ok(())
        }
    }
    let err = write_solutions(&mut Failing, ResultFormat::Json, &mut Vec::new()).unwrap_err();
    assert!(matches!(err, Error::Interrupted), "{err}");
}

#[test]
fn contract_violations_are_internal_errors() {
    struct ShortDecode(bool);
    impl SolutionSource for ShortDecode {
        fn variables(&self) -> &[String] {
            static VARS: std::sync::LazyLock<Vec<String>> = std::sync::LazyLock::new(|| vec!["v".to_string()]);
            &VARS
        }
        fn next_rows(&mut self, out: &mut Vec<TermId>, _: usize) -> Result<usize> {
            if std::mem::replace(&mut self.0, false) {
                out.push(TermId::from_bool(true));
                Ok(1)
            } else {
                Ok(0)
            }
        }
        fn decode(&mut self, _: &[TermId], _: &mut Vec<Option<Arc<Term>>>) -> Result<()> {
            Ok(())
        }
    }
    let err = write_solutions(&mut ShortDecode(true), ResultFormat::Json, &mut Vec::new()).unwrap_err();
    assert!(matches!(err, Error::Internal(_)), "{err}");
}

#[test]
fn wrong_format_kind_is_invalid() {
    for fmt in [ResultFormat::NTriples, ResultFormat::NQuads, ResultFormat::Turtle] {
        let mut src = FakeSolutions::new(&["s"], &[]);
        assert!(matches!(write_solutions(&mut src, fmt, &mut Vec::new()), Err(Error::Invalid(_))));
        assert!(matches!(write_boolean(true, fmt, &mut Vec::new()), Err(Error::Invalid(_))));
    }
    for fmt in [ResultFormat::Json, ResultFormat::Xml, ResultFormat::Csv, ResultFormat::Tsv] {
        let mut src = FakeTriples::new(&[]);
        assert!(matches!(write_triples(&mut src, fmt, &mut Vec::new()), Err(Error::Invalid(_))));
    }
}

#[test]
fn golden_booleans() {
    let render = |b: bool, fmt: ResultFormat| {
        let mut buf = Vec::new();
        write_boolean(b, fmt, &mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    };
    assert_eq!(render(true, ResultFormat::Json), r#"{"head":{},"boolean":true}"#);
    assert_eq!(render(false, ResultFormat::Json), r#"{"head":{},"boolean":false}"#);
    assert_eq!(
        render(true, ResultFormat::Xml),
        r#"<?xml version="1.0"?><sparql xmlns="http://www.w3.org/2005/sparql-results#"><head></head><boolean>true</boolean></sparql>"#
    );
    assert_eq!(render(false, ResultFormat::Csv), "false");
    assert_eq!(render(true, ResultFormat::Tsv), "true");
}

fn golden_triples() -> Vec<[Term; 3]> {
    vec![
        [iri("http://ex/s"), iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"), iri("http://ex/C")],
        [iri("http://ex/s"), iri("http://ex/p"), Term::Literal(Literal::typed("1", XSD_INTEGER))],
        [bnode("b2"), iri("http://ex/q"), Term::Literal(Literal::lang("x\"y", "en"))],
    ]
}

fn render_triples(fmt: ResultFormat) -> String {
    let mut buf = Vec::new();
    write_triples(&mut FakeTriples::new(&golden_triples()), fmt, &mut buf).unwrap();
    String::from_utf8(buf).unwrap()
}

#[test]
fn golden_ntriples_and_nquads() {
    let expected = concat!(
        "<http://ex/s> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://ex/C> .\n",
        "<http://ex/s> <http://ex/p> \"1\"^^<http://www.w3.org/2001/XMLSchema#integer> .\n",
        "_:b2 <http://ex/q> \"x\\\"y\"@en .\n",
    );
    assert_eq!(render_triples(ResultFormat::NTriples), expected);
    assert_eq!(render_triples(ResultFormat::NQuads), expected, "default-graph quads are triples");
}

#[test]
fn golden_turtle() {
    assert_eq!(
        render_triples(ResultFormat::Turtle),
        concat!(
            "@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .\n",
            "@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .\n",
            "@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .\n",
            "<http://ex/s> a <http://ex/C> ;\n",
            "\t<http://ex/p> 1 .\n",
            "_:b2 <http://ex/q> \"x\\\"y\"@en .\n",
        )
    );
}

#[test]
fn turtle_output_round_trips() {
    let text = render_triples(ResultFormat::Turtle);
    let parsed: Vec<oxrdf::Triple> =
        oxttl::TurtleParser::new().for_slice(text.as_bytes()).collect::<std::result::Result<_, _>>().unwrap();
    let expected: Vec<oxrdf::Triple> = golden_triples()
        .iter()
        .map(|[s, p, o]| {
            let s = match s.to_oxrdf() {
                oxrdf::Term::NamedNode(n) => oxrdf::NamedOrBlankNode::from(n),
                oxrdf::Term::BlankNode(b) => b.into(),
                oxrdf::Term::Literal(_) => unreachable!(),
            };
            let oxrdf::Term::NamedNode(p) = p.to_oxrdf() else { unreachable!() };
            oxrdf::Triple::new(s, p, o.to_oxrdf())
        })
        .collect();
    assert_eq!(parsed, expected);
}

#[test]
fn invalid_triples_are_internal_errors() {
    for bad in [
        [lit("s"), iri("http://ex/p"), iri("http://ex/o")],
        [iri("http://ex/s"), bnode("p"), iri("http://ex/o")],
    ] {
        let mut src = FakeTriples::new(&[bad]);
        let err = write_triples(&mut src, ResultFormat::NTriples, &mut Vec::new()).unwrap_err();
        assert!(matches!(err, Error::Internal(_)), "{err}");
    }
    let mut unbound = FakeTriples::new(&[[iri("http://ex/s"), iri("http://ex/p"), iri("http://ex/o")]]);
    unbound.triples[0][2] = TermId::UNBOUND;
    assert!(matches!(write_triples(&mut unbound, ResultFormat::NTriples, &mut Vec::new()), Err(Error::Internal(_))));
}

#[test]
fn many_triples_stream_in_batches() {
    let triples: Vec<[Term; 3]> =
        (0..BATCH_ROWS + 5).map(|i| [iri(&format!("http://ex/s{i}")), iri("http://ex/p"), lit("o")]).collect();
    let mut src = FakeTriples::new(&triples);
    let mut buf = Vec::new();
    write_triples(&mut src, ResultFormat::NTriples, &mut buf).unwrap();
    assert_eq!(buf.iter().filter(|b| **b == b'\n').count(), BATCH_ROWS + 5);
    assert_eq!(src.dict.decode_calls, 2);
}
