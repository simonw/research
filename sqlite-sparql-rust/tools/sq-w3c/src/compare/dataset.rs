//! Graph / dataset comparison up to blank-node isomorphism (CONSTRUCT, DESCRIBE, updates),
//! via oxrdf canonicalization of both sides.
use std::collections::BTreeSet;

use oxrdf::dataset::CanonicalizationAlgorithm;
use oxrdf::Dataset;

use super::numeric::Numeric;

const MAX_DIFF_QUADS: usize = 12;

pub fn compare_datasets(expected: &Dataset, actual: &Dataset, numeric: Numeric) -> Result<(), String> {
    let mut e = numeric.apply_dataset(expected);
    let mut a = numeric.apply_dataset(actual);
    e.canonicalize(CanonicalizationAlgorithm::Unstable);
    a.canonicalize(CanonicalizationAlgorithm::Unstable);
    if e == a {
        return Ok(());
    }
    let render = |d: &Dataset| d.iter().map(|q| q.to_string()).collect::<BTreeSet<_>>();
    let (e_text, a_text) = (render(&e), render(&a));
    let mut out = format!("expected {} quads, got {}", e.len(), a.len());
    for (label, only) in [("missing", e_text.difference(&a_text)), ("unexpected", a_text.difference(&e_text))] {
        let only: Vec<&String> = only.collect();
        if only.is_empty() {
            continue;
        }
        out.push_str(&format!("; {} {label} (canonical blank-node labels):", only.len()));
        for q in only.iter().take(MAX_DIFF_QUADS) {
            out.push_str("\n    ");
            out.push_str(q);
        }
        if only.len() > MAX_DIFF_QUADS {
            out.push_str(&format!("\n    … {} more", only.len() - MAX_DIFF_QUADS));
        }
    }
    Err(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::suite::parse_rdf_into;
    use oxrdfio::RdfFormat;

    fn trig(text: &str) -> Dataset {
        let mut d = Dataset::new();
        parse_rdf_into(&mut d, text.as_bytes(), RdfFormat::TriG, "http://ex/", None).unwrap();
        d
    }

    #[test]
    fn isomorphic_datasets_are_equal() {
        let e = trig("_:a <p> _:b . _:b <p> _:a . <g> { _:c <q> 1 }");
        let a = trig("_:x <p> _:y . _:y <p> _:x . <g> { _:z <q> 1 }");
        compare_datasets(&e, &a, Numeric::Exact).unwrap();
    }

    #[test]
    fn graph_names_and_structure_matter() {
        let e = trig("<g> { <s> <p> <o> }");
        let a = trig("<h> { <s> <p> <o> }");
        let err = compare_datasets(&e, &a, Numeric::Exact).unwrap_err();
        assert!(err.contains("1 missing") && err.contains("1 unexpected"), "{err}");
        let e = trig("_:a <p> _:a .");
        let a = trig("_:a <p> _:b .");
        assert!(compare_datasets(&e, &a, Numeric::Exact).is_err());
    }

    #[test]
    fn lenient_numerics() {
        let e = trig("<s> <p> \"01\"^^<http://www.w3.org/2001/XMLSchema#integer> .");
        let a = trig("<s> <p> 1 .");
        assert!(compare_datasets(&e, &a, Numeric::Exact).is_err());
        compare_datasets(&e, &a, Numeric::Lenient).unwrap();
    }
}
