//! Result comparison (§12.2). Every comparison runs twice: pass 1 is exact; pass 2 treats
//! numeric literals of one datatype with equal values as equal and is reported separately as
//! `pass-lenient`, because expected results often use non-canonical numerals.
mod dataset;
mod numeric;
mod solutions;
mod text;
mod ties;

use oxrdf::{Dataset, Variable};
use spargebra::algebra::{Expression, GraphPattern, OrderExpression};
use spargebra::Query;

pub use dataset::compare_datasets;
pub use numeric::Numeric;
pub use solutions::compare_solutions;
pub use text::{compare_result_text, ColumnOrder};
pub(crate) use text::{csv_records, csv_value};

use crate::results::{csv_lossy, triples_to_dataset, Expected, QueryOutcome, Solutions};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    /// Equal only under numeric leniency; carries the exact-comparison difference.
    PassLenient(String),
    Fail(String),
}

/// Ordering constraint of a SELECT result, from the query's top-level algebra.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowOrder {
    Unordered,
    /// ORDER BY on plain projected variables: runs of tied keys (equal under SPARQL's
    /// ordering: `1` and `1.0`, any two blank nodes) are compared in order, rows within a run
    /// as a multiset.
    Runs(Vec<Variable>),
    /// ORDER BY on expressions or hidden variables: the exact sequence is compared.
    Sequence,
}

impl RowOrder {
    /// The constraint for `query`: ordered iff, under the query's own solution modifiers,
    /// the algebra is an OrderBy (the ORDER BY of a sub-select does not order the outer query).
    pub fn of(query: &Query) -> RowOrder {
        let Some((projection, pattern)) = select_modifiers(query) else { return RowOrder::Unordered };
        let GraphPattern::OrderBy { expression, .. } = pattern else { return RowOrder::Unordered };
        let keys: Option<Vec<Variable>> = expression
            .iter()
            .map(|e| match e {
                OrderExpression::Asc(Expression::Variable(v)) | OrderExpression::Desc(Expression::Variable(v)) => {
                    Some(v.clone())
                }
                _ => None,
            })
            .collect();
        match keys {
            Some(keys) if keys.iter().all(|k| projection.is_none_or(|p| p.contains(k))) => RowOrder::Runs(keys),
            _ => RowOrder::Sequence,
        }
    }

    pub fn describe(&self) -> String {
        match self {
            RowOrder::Unordered => "unordered".to_owned(),
            RowOrder::Runs(keys) => {
                let keys: Vec<String> = keys.iter().map(|k| k.to_string()).collect();
                format!("ORDER BY {}", keys.join(" "))
            }
            RowOrder::Sequence => "ORDER BY on expressions: exact sequence".to_owned(),
        }
    }
}

/// Strips a SELECT query's own solution modifiers (Slice, then Distinct / Reduced, then one
/// Project): returns the projected variables (if projected) and the pattern underneath.
pub fn select_modifiers(query: &Query) -> Option<(Option<&[Variable]>, &GraphPattern)> {
    let Query::Select { pattern, .. } = query else { return None };
    let mut pattern = pattern;
    if let GraphPattern::Slice { inner, .. } = pattern {
        pattern = inner;
    }
    if let GraphPattern::Distinct { inner } | GraphPattern::Reduced { inner } = pattern {
        pattern = inner;
    }
    Some(match pattern {
        GraphPattern::Project { inner, variables } => (Some(variables.as_slice()), inner.as_ref()),
        other => (None, other),
    })
}

/// Runs `compare` exactly, then leniently.
fn two_passes(compare: impl Fn(Numeric) -> Result<(), String>) -> Verdict {
    match compare(Numeric::Exact) {
        Ok(()) => Verdict::Pass,
        Err(exact) => match compare(Numeric::Lenient) {
            Ok(()) => Verdict::PassLenient(exact),
            Err(_) => Verdict::Fail(exact),
        },
    }
}

/// Compares a query outcome with the expected result.
pub fn compare_query(expected: &Expected, actual: &QueryOutcome, order: &RowOrder, lax: bool) -> Verdict {
    match (expected, actual) {
        (Expected::Boolean(e), QueryOutcome::Boolean(a)) => {
            if e == a {
                Verdict::Pass
            } else {
                Verdict::Fail(format!("expected {e}, got {a}"))
            }
        }
        (Expected::Solutions(e), QueryOutcome::Solutions(a)) => {
            two_passes(|numeric| compare_solutions(e, a, order, lax, numeric))
        }
        (Expected::CsvSolutions(e), QueryOutcome::Solutions(a)) => {
            let a = Solutions {
                variables: a.variables.clone(),
                rows: a.rows.iter().map(|r| r.iter().map(|t| t.as_ref().and_then(csv_lossy)).collect()).collect(),
                ordered: a.ordered,
            };
            two_passes(|numeric| compare_solutions(e, &a, order, lax, numeric))
        }
        (Expected::Graph(e), QueryOutcome::Graph(a)) => {
            let a = triples_to_dataset(a);
            two_passes(|numeric| compare_datasets(e, &a, numeric))
        }
        (e, a) => Verdict::Fail(format!("expected {}, got {}", e.kind(), a.kind())),
    }
}

/// Compares the store after an update with the expected dataset.
pub fn compare_update(expected: &Dataset, actual: &Dataset) -> Verdict {
    two_passes(|numeric| compare_datasets(expected, actual, numeric))
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxrdf::{Literal, NamedNode, Term};

    fn order(q: &str) -> RowOrder {
        RowOrder::of(&spargebra::SparqlParser::new().parse_query(q).unwrap())
    }

    #[test]
    fn row_order_from_algebra() {
        let v = |n: &str| Variable::new_unchecked(n);
        assert_eq!(order("SELECT * { ?s ?p ?o }"), RowOrder::Unordered);
        assert_eq!(order("SELECT * { ?s ?p ?o } ORDER BY ?s DESC(?o)"), RowOrder::Runs(vec![v("s"), v("o")]));
        assert_eq!(order("SELECT DISTINCT ?s { ?s ?p ?o } ORDER BY ?s LIMIT 2"), RowOrder::Runs(vec![v("s")]));
        assert_eq!(order("SELECT ?s { ?s ?p ?o } ORDER BY ?o"), RowOrder::Sequence);
        assert_eq!(order("SELECT ?s { ?s ?p ?o } ORDER BY STR(?s)"), RowOrder::Sequence);
        assert_eq!(order("SELECT (COUNT(*) AS ?c) { ?s ?p ?o } GROUP BY ?s ORDER BY ?c"), RowOrder::Runs(vec![v("c")]));
        assert_eq!(order("ASK { ?s ?p ?o }"), RowOrder::Unordered);
        assert_eq!(order("SELECT * { { SELECT * { ?s ?p ?o } ORDER BY ?s } }"), RowOrder::Unordered);
    }

    #[test]
    fn kinds_must_agree() {
        let v = compare_query(&Expected::Boolean(true), &QueryOutcome::Boolean(false), &RowOrder::Unordered, false);
        assert_eq!(v, Verdict::Fail("expected true, got false".into()));
        let v = compare_query(&Expected::Boolean(true), &QueryOutcome::Graph(vec![]), &RowOrder::Unordered, false);
        assert!(matches!(v, Verdict::Fail(m) if m.contains("got a graph")));
    }

    #[test]
    fn csv_expectations_compare_lossy_forms() {
        let csv = b"s,o\r\nhttp://ex/a,1\r\n_:x,\r\n";
        let expected = crate::results::parse_expected(csv, "http://t/r.csv").unwrap();
        let vars = vec![Variable::new_unchecked("o"), Variable::new_unchecked("s")];
        let actual = |o: Term| {
            QueryOutcome::Solutions(crate::results::Solutions {
                variables: vars.clone(),
                rows: vec![
                    vec![Some(o), Some(NamedNode::new_unchecked("http://ex/a").into())],
                    vec![None, Some(oxrdf::BlankNode::new_unchecked("b9").into())],
                ],
                ordered: true,
            })
        };
        let order = RowOrder::Unordered;
        assert_eq!(compare_query(&expected, &actual(Literal::from(1).into()), &order, false), Verdict::Pass);
        let two = actual(Literal::from(2).into());
        assert!(matches!(compare_query(&expected, &two, &order, false), Verdict::Fail(_)));
    }

    #[test]
    fn lenient_pass_is_reported_separately() {
        let dt = NamedNode::new_unchecked("http://www.w3.org/2001/XMLSchema#decimal");
        let row = |lex: &str| vec![Some(Term::from(Literal::new_typed_literal(lex, dt.clone())))];
        let vars = vec![Variable::new_unchecked("x")];
        let e = Expected::Solutions(crate::results::Solutions { variables: vars.clone(), rows: vec![row("2.50")], ordered: true });
        let a = QueryOutcome::Solutions(crate::results::Solutions { variables: vars, rows: vec![row("2.5")], ordered: true });
        let verdict = compare_query(&e, &a, &RowOrder::Unordered, false);
        assert!(matches!(&verdict, Verdict::PassLenient(m) if m.contains("2.50")), "{verdict:?}");
    }
}
