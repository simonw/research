//! Output formats accepted by `sparql_query(query, format)` and the engine facade.
use std::fmt;
use std::str::FromStr;

use oxrdfio::RdfFormat;
use sparesults::QueryResultsFormat;
use sq_core::{Error, Result};

/// A serialization format for query results.
///
/// SELECT / ASK results use the SPARQL 1.1 result formats ([`ResultFormat::Json`],
/// [`ResultFormat::Xml`], [`ResultFormat::Csv`], [`ResultFormat::Tsv`]); CONSTRUCT / DESCRIBE
/// results use the RDF formats ([`ResultFormat::NTriples`], [`ResultFormat::NQuads`],
/// [`ResultFormat::Turtle`]).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ResultFormat {
    Json,
    Xml,
    Csv,
    Tsv,
    NTriples,
    NQuads,
    Turtle,
}

impl ResultFormat {
    /// Every format, in documentation order.
    pub const ALL: [ResultFormat; 7] = [
        ResultFormat::Json,
        ResultFormat::Xml,
        ResultFormat::Csv,
        ResultFormat::Tsv,
        ResultFormat::NTriples,
        ResultFormat::NQuads,
        ResultFormat::Turtle,
    ];

    /// Parses a format name, ignoring ASCII case: `json`, `xml`, `csv`, `tsv`,
    /// `ntriples` / `nt`, `nquads` / `nq`, `turtle` / `ttl`.
    pub fn parse(s: &str) -> Result<Self> {
        const NAMES: [(&str, ResultFormat); 10] = [
            ("json", ResultFormat::Json),
            ("xml", ResultFormat::Xml),
            ("csv", ResultFormat::Csv),
            ("tsv", ResultFormat::Tsv),
            ("ntriples", ResultFormat::NTriples),
            ("nt", ResultFormat::NTriples),
            ("nquads", ResultFormat::NQuads),
            ("nq", ResultFormat::NQuads),
            ("turtle", ResultFormat::Turtle),
            ("ttl", ResultFormat::Turtle),
        ];
        let name = s.trim();
        NAMES
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, f)| *f)
            .ok_or_else(|| {
                Error::Invalid(format!(
                    "unknown result format '{s}' (expected json, xml, csv, tsv, ntriples/nt, nquads/nq or turtle/ttl)"
                ))
            })
    }

    /// Canonical (long) name, as accepted by [`ResultFormat::parse`].
    pub const fn name(self) -> &'static str {
        match self {
            ResultFormat::Json => "json",
            ResultFormat::Xml => "xml",
            ResultFormat::Csv => "csv",
            ResultFormat::Tsv => "tsv",
            ResultFormat::NTriples => "ntriples",
            ResultFormat::NQuads => "nquads",
            ResultFormat::Turtle => "turtle",
        }
    }

    /// IANA media type of the serialization.
    pub const fn media_type(self) -> &'static str {
        match self {
            ResultFormat::Json => "application/sparql-results+json",
            ResultFormat::Xml => "application/sparql-results+xml",
            ResultFormat::Csv => "text/csv; charset=utf-8",
            ResultFormat::Tsv => "text/tab-separated-values; charset=utf-8",
            ResultFormat::NTriples => "application/n-triples",
            ResultFormat::NQuads => "application/n-quads",
            ResultFormat::Turtle => "text/turtle",
        }
    }

    /// True for the SPARQL result formats (SELECT / ASK).
    pub const fn is_solutions_format(self) -> bool {
        self.solutions_format().is_some()
    }

    /// True for the RDF formats (CONSTRUCT / DESCRIBE).
    pub const fn is_graph_format(self) -> bool {
        self.graph_format().is_some()
    }

    /// The sparesults format for SELECT / ASK output, if this is one.
    pub const fn solutions_format(self) -> Option<QueryResultsFormat> {
        match self {
            ResultFormat::Json => Some(QueryResultsFormat::Json),
            ResultFormat::Xml => Some(QueryResultsFormat::Xml),
            ResultFormat::Csv => Some(QueryResultsFormat::Csv),
            ResultFormat::Tsv => Some(QueryResultsFormat::Tsv),
            _ => None,
        }
    }

    /// The oxrdfio format for CONSTRUCT / DESCRIBE output, if this is one.
    pub const fn graph_format(self) -> Option<RdfFormat> {
        match self {
            ResultFormat::NTriples => Some(RdfFormat::NTriples),
            ResultFormat::NQuads => Some(RdfFormat::NQuads),
            ResultFormat::Turtle => Some(RdfFormat::Turtle),
            _ => None,
        }
    }
}

impl FromStr for ResultFormat {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::parse(s)
    }
}

impl fmt::Display for ResultFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_alias_case_insensitively() {
        let cases = [
            ("json", ResultFormat::Json),
            ("JSON", ResultFormat::Json),
            ("Xml", ResultFormat::Xml),
            ("csv", ResultFormat::Csv),
            ("TSV", ResultFormat::Tsv),
            ("ntriples", ResultFormat::NTriples),
            ("NT", ResultFormat::NTriples),
            ("nquads", ResultFormat::NQuads),
            ("nQ", ResultFormat::NQuads),
            ("Turtle", ResultFormat::Turtle),
            ("ttl", ResultFormat::Turtle),
            (" json ", ResultFormat::Json),
        ];
        for (name, expected) in cases {
            assert_eq!(ResultFormat::parse(name).unwrap(), expected, "{name}");
            assert_eq!(name.parse::<ResultFormat>().unwrap(), expected, "{name}");
        }
    }

    #[test]
    fn rejects_unknown_names() {
        for name in ["", "rdfxml", "jsonld", "n-triples", "srj"] {
            let err = ResultFormat::parse(name).unwrap_err();
            assert!(matches!(err, Error::Invalid(_)), "{name}: {err}");
        }
    }

    #[test]
    fn names_round_trip_and_kinds_partition() {
        for f in ResultFormat::ALL {
            assert_eq!(ResultFormat::parse(f.name()).unwrap(), f);
            assert_eq!(f.to_string(), f.name());
            assert_ne!(f.is_solutions_format(), f.is_graph_format(), "{f}");
            assert!(!f.media_type().is_empty());
        }
    }
}
