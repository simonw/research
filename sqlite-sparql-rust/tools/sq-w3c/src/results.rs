//! Query outcomes (what a backend returned) and expected results (what a test expects), and
//! the parsers that build them from SPARQL result documents and RDF graphs.
use std::collections::BTreeMap;

use oxrdf::{
    BlankNode, Dataset, Graph, GraphName, Literal, NamedOrBlankNodeRef, Quad, Term, TermRef, Triple, Variable,
};
use oxrdfio::RdfFormat;
use sparesults::{QueryResultsFormat, QueryResultsParser, SliceQueryResultsParserOutput};

use crate::compare::{csv_records, csv_value};
use crate::suite::{parse_rdf_into, rdf_format};
use crate::vocab::{rdf, rs};

/// A table of solutions. `rows[i][j]` is the value of `variables[j]` (None = unbound).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Solutions {
    pub variables: Vec<Variable>,
    pub rows: Vec<Vec<Option<Term>>>,
    /// The row sequence is meaningful (false for RDF result sets without `rs:index`).
    pub ordered: bool,
}

/// What a backend returned for a query.
#[derive(Clone, Debug, PartialEq)]
pub enum QueryOutcome {
    Solutions(Solutions),
    Boolean(bool),
    /// CONSTRUCT / DESCRIBE.
    Graph(Vec<Triple>),
}

impl QueryOutcome {
    pub fn kind(&self) -> &'static str {
        match self {
            QueryOutcome::Solutions(_) => "solutions",
            QueryOutcome::Boolean(_) => "a boolean",
            QueryOutcome::Graph(_) => "a graph",
        }
    }
}

/// The textual SPARQL result formats whose documents `mf:CSVResultFormatTest` compares as text.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TextFormat {
    Csv,
    Tsv,
}

impl TextFormat {
    /// From a result file name.
    pub fn of_file(url: &str) -> Option<Self> {
        match url.rsplit_once('.')?.1.to_ascii_lowercase().as_str() {
            "csv" => Some(TextFormat::Csv),
            "tsv" => Some(TextFormat::Tsv),
            _ => None,
        }
    }

    /// The format name of `sparql_query(query, format)` / `ResultFormat::parse`.
    pub const fn name(self) -> &'static str {
        match self {
            TextFormat::Csv => "csv",
            TextFormat::Tsv => "tsv",
        }
    }

    pub const fn sparesults(self) -> QueryResultsFormat {
        match self {
            TextFormat::Csv => QueryResultsFormat::Csv,
            TextFormat::Tsv => QueryResultsFormat::Tsv,
        }
    }
}

/// What a query test expects.
#[derive(Clone, Debug, PartialEq)]
pub enum Expected {
    Solutions(Solutions),
    /// From a CSV document, which only keeps strings: values are [`csv_lossy`] forms.
    CsvSolutions(Solutions),
    Boolean(bool),
    Graph(Dataset),
}

impl Expected {
    pub fn kind(&self) -> &'static str {
        match self {
            Expected::Solutions(_) | Expected::CsvSolutions(_) => "solutions",
            Expected::Boolean(_) => "a boolean",
            Expected::Graph(_) => "a graph",
        }
    }
}

/// Parses SPARQL result text produced by a backend (JSON, XML or TSV).
pub fn parse_results(data: &[u8], format: QueryResultsFormat) -> Result<QueryOutcome, String> {
    let parsed = QueryResultsParser::from_format(format)
        .for_slice(data)
        .map_err(|e| format!("invalid {} results: {e}", format.name()))?;
    match parsed {
        SliceQueryResultsParserOutput::Boolean(b) => Ok(QueryOutcome::Boolean(b)),
        SliceQueryResultsParserOutput::Solutions(solutions) => {
            let variables = solutions.variables().to_vec();
            let mut rows = Vec::new();
            for solution in solutions {
                let solution = solution.map_err(|e| format!("invalid {} results: {e}", format.name()))?;
                rows.push(solution.values().to_vec());
            }
            Ok(QueryOutcome::Solutions(Solutions { variables, rows, ordered: true }))
        }
    }
}

/// Parses an RDF graph produced by a backend (CONSTRUCT / DESCRIBE output).
pub fn parse_graph(data: &[u8], format: RdfFormat) -> Result<QueryOutcome, String> {
    let mut triples = Vec::new();
    for quad in oxrdfio::RdfParser::from_format(format).for_slice(data) {
        let quad = quad.map_err(|e| format!("invalid {} graph: {e}", format.name()))?;
        if quad.graph_name != GraphName::DefaultGraph {
            return Err(format!("graph result contains a quad in graph {}", quad.graph_name));
        }
        triples.push(Triple::from(quad));
    }
    Ok(QueryOutcome::Graph(triples))
}

/// Loads the expected result of a query test from its file (by extension): `.srx`, `.srj`,
/// `.tsv`, `.csv` (lossy), or an RDF document holding an `rs:ResultSet` or a CONSTRUCT graph.
pub fn parse_expected(data: &[u8], url: &str) -> Result<Expected, String> {
    let ext = url.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    let results_format = match ext.as_str() {
        "srx" => Some(QueryResultsFormat::Xml),
        "srj" => Some(QueryResultsFormat::Json),
        "tsv" => Some(QueryResultsFormat::Tsv),
        "csv" => return parse_csv(data),
        _ => None,
    };
    if let Some(format) = results_format {
        return Ok(match parse_results(data, format)? {
            QueryOutcome::Solutions(s) => Expected::Solutions(s),
            QueryOutcome::Boolean(b) => Expected::Boolean(b),
            QueryOutcome::Graph(_) => unreachable!("result documents never hold graphs"),
        });
    }
    let format = rdf_format(url).ok_or_else(|| format!("unknown result file type: {url}"))?;
    let mut dataset = Dataset::new();
    parse_rdf_into(&mut dataset, data, format, url, None)?;
    let graph = Graph::from_iter(dataset.iter().map(|q| Quad::from(q)).map(Triple::from));
    match result_set(&graph)? {
        Some(expected) => Ok(expected),
        None => Ok(Expected::Graph(dataset)),
    }
}

/// The CSV form of a term, as a term: IRIs and literals become simple literals of their
/// string (an empty string is indistinguishable from unbound), blank nodes stay blank nodes.
pub fn csv_lossy(term: &Term) -> Option<Term> {
    let text = match term {
        Term::BlankNode(_) => return Some(term.clone()),
        Term::NamedNode(n) => n.as_str(),
        Term::Literal(l) => l.value(),
    };
    (!text.is_empty()).then(|| Literal::new_simple_literal(text).into())
}

/// Expected results in CSV: a header of variable names, then one record per solution; a
/// lone `true` / `false` is an ASK result.
fn parse_csv(data: &[u8]) -> Result<Expected, String> {
    let text = std::str::from_utf8(data).map_err(|e| format!("CSV results are not UTF-8: {e}"))?;
    let records = csv_records(text);
    let Some((header, records)) = records.split_first() else {
        return Err("empty CSV results".into());
    };
    if records.is_empty() && header.len() == 1 && matches!(header[0].as_str(), "true" | "false") {
        return Ok(Expected::Boolean(header[0] == "true"));
    }
    let variables = header
        .iter()
        .map(|name| Variable::new(csv_value(name)).map_err(|e| format!("invalid CSV variable {name}: {e}")))
        .collect::<Result<Vec<_>, _>>()?;
    let rows = records
        .iter()
        .map(|record| {
            let row: Vec<Option<Term>> = record
                .iter()
                .map(|field| {
                    let value = csv_value(field);
                    match value.strip_prefix("_:").map(BlankNode::new) {
                        Some(Ok(b)) => Some(b.into()),
                        _ => csv_lossy(&Literal::new_simple_literal(value).into()),
                    }
                })
                .collect();
            if row.len() == variables.len() {
                Ok(row)
            } else {
                Err(format!("CSV record with {} fields for {} variables", row.len(), variables.len()))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Expected::CsvSolutions(Solutions { variables, rows, ordered: true }))
}

/// Reads an `rs:ResultSet` (DAWG result-set vocabulary) out of an RDF graph, if there is one.
fn result_set(graph: &Graph) -> Result<Option<Expected>, String> {
    let Some(set) = graph
        .subjects_for_predicate_object(rdf::TYPE, rs::RESULT_SET)
        .next()
        .or_else(|| graph.triples_for_predicate(rs::BOOLEAN).next().map(|t| t.subject))
        .or_else(|| graph.triples_for_predicate(rs::SOLUTION).next().map(|t| t.subject))
    else {
        return Ok(None);
    };
    if let Some(value) = graph.object_for_subject_predicate(set, rs::BOOLEAN) {
        return match value {
            TermRef::Literal(l) if matches!(l.value(), "true" | "1") => Ok(Some(Expected::Boolean(true))),
            TermRef::Literal(l) if matches!(l.value(), "false" | "0") => Ok(Some(Expected::Boolean(false))),
            other => Err(format!("invalid rs:boolean {other}")),
        };
    }

    let mut variables: Vec<Variable> = Vec::new();
    for v in graph.objects_for_subject_predicate(set, rs::RESULT_VARIABLE) {
        let TermRef::Literal(l) = v else { return Err(format!("invalid rs:resultVariable {v}")) };
        variable_index(&mut variables, l.value())?;
    }

    let mut solutions: Vec<(Option<i64>, Vec<(usize, Term)>)> = Vec::new();
    for solution in graph.objects_for_subject_predicate(set, rs::SOLUTION) {
        let solution = as_subject(solution)?;
        let index = match graph.object_for_subject_predicate(solution, rs::INDEX) {
            Some(TermRef::Literal(l)) => Some(l.value().parse::<i64>().map_err(|e| format!("invalid rs:index: {e}"))?),
            Some(other) => return Err(format!("invalid rs:index {other}")),
            None => None,
        };
        let mut bindings = Vec::new();
        for binding in graph.objects_for_subject_predicate(solution, rs::BINDING) {
            let binding = as_subject(binding)?;
            let Some(TermRef::Literal(name)) = graph.object_for_subject_predicate(binding, rs::VARIABLE) else {
                return Err("rs:binding without a literal rs:variable".into());
            };
            let value = graph
                .object_for_subject_predicate(binding, rs::VALUE)
                .ok_or_else(|| format!("rs:binding of {} without rs:value", name.value()))?;
            bindings.push((variable_index(&mut variables, name.value())?, value.into_owned()));
        }
        solutions.push((index, bindings));
    }

    let ordered = !solutions.is_empty() && solutions.iter().all(|(i, _)| i.is_some());
    if ordered {
        solutions.sort_by_key(|(i, _)| *i);
    }
    let rows = solutions
        .into_iter()
        .map(|(_, bindings)| {
            let mut row = vec![None; variables.len()];
            for (i, value) in bindings {
                row[i] = Some(value);
            }
            row
        })
        .collect();
    Ok(Some(Expected::Solutions(Solutions { variables, rows, ordered })))
}

/// Position of `name` in `variables`, appending it if absent.
fn variable_index(variables: &mut Vec<Variable>, name: &str) -> Result<usize, String> {
    if let Some(i) = variables.iter().position(|v| v.as_str() == name) {
        return Ok(i);
    }
    variables.push(Variable::new(name).map_err(|e| format!("invalid result variable {name}: {e}"))?);
    Ok(variables.len() - 1)
}

fn as_subject(t: TermRef<'_>) -> Result<NamedOrBlankNodeRef<'_>, String> {
    match t {
        TermRef::NamedNode(n) => Ok(n.into()),
        TermRef::BlankNode(b) => Ok(b.into()),
        TermRef::Literal(l) => Err(format!("literal {l} where a node was expected")),
    }
}

/// Converts a list of triples into a dataset (default graph).
pub fn triples_to_dataset(triples: &[Triple]) -> Dataset {
    triples.iter().map(|t| t.as_ref().in_graph(GraphName::DefaultGraph.as_ref())).collect()
}

/// A compact rendering of a row (`?s=<a> ?o="x"`), unbound values omitted.
pub fn format_row(variables: &[Variable], row: &[Option<Term>]) -> String {
    let parts: BTreeMap<&str, String> = variables
        .iter()
        .zip(row)
        .filter_map(|(v, t)| Some((v.as_str(), t.as_ref()?.to_string())))
        .collect();
    if parts.is_empty() {
        return "{}".to_owned();
    }
    parts.iter().map(|(v, t)| format!("?{v}={t}")).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxrdf::NamedNode;

    #[test]
    fn parses_srx_and_srj() {
        let srx = br#"<?xml version="1.0"?><sparql xmlns="http://www.w3.org/2005/sparql-results#"><head><variable name="x"/><variable name="y"/></head><results><result><binding name="x"><uri>http://ex/a</uri></binding></result></results></sparql>"#;
        let Expected::Solutions(s) = parse_expected(srx, "http://t/r.srx").unwrap() else { panic!() };
        assert_eq!(s.variables, [Variable::new_unchecked("x"), Variable::new_unchecked("y")]);
        assert_eq!(s.rows, [vec![Some(NamedNode::new_unchecked("http://ex/a").into()), None]]);
        assert!(s.ordered);
        let srj = br#"{"head":{},"boolean":true}"#;
        assert_eq!(parse_expected(srj, "http://t/r.srj").unwrap(), Expected::Boolean(true));
        let Expected::CsvSolutions(s) =
            parse_expected(b"a,b\r\nhttp://ex/x,\"1,5\"\r\n_:n,\r\n", "http://t/r.csv").unwrap()
        else {
            panic!()
        };
        assert_eq!(s.variables, [Variable::new_unchecked("a"), Variable::new_unchecked("b")]);
        assert_eq!(s.rows[0], [Some(Literal::new_simple_literal("http://ex/x").into()), Some(Literal::from("1,5").into())]);
        assert!(matches!(s.rows[1][..], [Some(Term::BlankNode(_)), None]));
        assert_eq!(parse_expected(b"true", "http://t/r.csv").unwrap(), Expected::Boolean(true));
        assert!(parse_expected(b"a,b\r\n1\r\n", "http://t/r.csv").is_err());
    }

    #[test]
    fn parses_rdf_result_sets() {
        let ttl = br#"
@prefix rs: <http://www.w3.org/2001/sw/DataAccess/tests/result-set#> .
[] a rs:ResultSet ; rs:resultVariable "a", "b" ;
   rs:solution [ rs:index 2 ; rs:binding [ rs:variable "a" ; rs:value <x> ] ] ;
   rs:solution [ rs:index 1 ; rs:binding [ rs:variable "b" ; rs:value 7 ] , [ rs:variable "a" ; rs:value _:n ] ] .
"#;
        let Expected::Solutions(s) = parse_expected(ttl, "http://t/dir/r.ttl").unwrap() else { panic!() };
        assert!(s.ordered);
        let a = s.variables.iter().position(|v| v.as_str() == "a").unwrap();
        let b = s.variables.iter().position(|v| v.as_str() == "b").unwrap();
        assert_eq!(s.rows.len(), 2);
        assert!(matches!(s.rows[0][a], Some(Term::BlankNode(_))));
        assert_eq!(s.rows[0][b], Some(Literal::from(7).into()));
        assert_eq!(s.rows[1][a], Some(NamedNode::new_unchecked("http://t/dir/x").into()));
        assert_eq!(s.rows[1][b], None);

        let unordered = br#"@prefix rs: <http://www.w3.org/2001/sw/DataAccess/tests/result-set#> .
[] a rs:ResultSet ; rs:resultVariable "a" ; rs:solution [ rs:binding [ rs:variable "a" ; rs:value 1 ] ] ."#;
        let Expected::Solutions(s) = parse_expected(unordered, "http://t/r.ttl").unwrap() else { panic!() };
        assert!(!s.ordered);

        let boolean = br#"@prefix rs: <http://www.w3.org/2001/sw/DataAccess/tests/result-set#> .
[] a rs:ResultSet ; rs:boolean false ."#;
        assert_eq!(parse_expected(boolean, "http://t/r.ttl").unwrap(), Expected::Boolean(false));
    }

    #[test]
    fn plain_rdf_is_a_graph_result() {
        let Expected::Graph(g) = parse_expected(b"<s> <p> <o> .", "http://t/c.ttl").unwrap() else { panic!() };
        assert_eq!(g.len(), 1);
    }

    #[test]
    fn parses_backend_outputs() {
        let json = br#"{"head":{"vars":["s"]},"results":{"bindings":[{"s":{"type":"bnode","value":"b1"}},{}]}}"#;
        let QueryOutcome::Solutions(s) = parse_results(json, QueryResultsFormat::Json).unwrap() else { panic!() };
        assert_eq!(s.rows.len(), 2);
        assert_eq!(s.rows[1], [None]);
        let QueryOutcome::Graph(t) = parse_graph(b"<http://a> <http://b> \"c\" .\n", RdfFormat::NTriples).unwrap() else {
            panic!()
        };
        assert_eq!(t.len(), 1);
        assert!(parse_results(b"{not json", QueryResultsFormat::Json).is_err());
    }

    #[test]
    fn rows_render_sorted_by_variable() {
        let vars = [Variable::new_unchecked("b"), Variable::new_unchecked("a")];
        let row = [Some(Literal::from(1).into()), Some(NamedNode::new_unchecked("http://x").into())];
        assert_eq!(format_row(&vars, &row), "?a=<http://x> ?b=\"1\"^^<http://www.w3.org/2001/XMLSchema#integer>");
        assert_eq!(format_row(&vars, &[None, None]), "{}");
    }
}
