//! The local copy of the W3C `rdf-tests/sparql` directory and its public naming.
//!
//! Every local file `<suite>/<path>` is named `https://w3c.github.io/rdf-tests/sparql/<path>`.
//! That IRI is the base IRI used to parse the file (manifests, queries, data, expected results)
//! and the graph name of data loaded with `qt:graphData`, so relative IRIs in queries
//! (`FROM <data-g1.ttl>`) and in expected results agree with the loaded data.
use std::path::{Component, Path, PathBuf};

use oxrdf::{Dataset, GraphName, NamedNodeRef};
use oxrdfio::{RdfFormat, RdfParser};

/// Public base IRI of the suite directory.
pub const PUBLIC_BASE: &str = "https://w3c.github.io/rdf-tests/sparql/";

#[derive(Clone, Debug)]
pub struct Suite {
    root: PathBuf,
}

impl Suite {
    /// `root` is the `sparql` directory of a checkout of w3c/rdf-tests.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, String> {
        let root = root.into();
        if !root.is_dir() {
            return Err(format!("suite directory {} does not exist", root.display()));
        }
        Ok(Suite { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Public IRI of a suite-relative path such as `sparql11/manifest-all.ttl`.
    pub fn url(&self, relative: &str) -> String {
        format!("{PUBLIC_BASE}{}", relative.trim_start_matches('/'))
    }

    /// Suite-relative path of a public IRI, if the IRI names a file of the suite.
    pub fn relative<'a>(&self, url: &'a str) -> Option<&'a str> {
        let rel = url.strip_prefix(PUBLIC_BASE)?;
        let rel = rel.split(['#', '?']).next().unwrap_or(rel);
        let safe = Path::new(rel).components().all(|c| matches!(c, Component::Normal(_)));
        (safe && !rel.is_empty()).then_some(rel)
    }

    /// Local path of a public IRI.
    pub fn path(&self, url: &str) -> Option<PathBuf> {
        self.relative(url).map(|rel| self.root.join(rel))
    }

    /// Reads the file named by a public IRI.
    pub fn read(&self, url: &str) -> Result<Vec<u8>, String> {
        let path = self.path(url).ok_or_else(|| format!("{url} is not a file of the test suite"))?;
        std::fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))
    }

    /// Reads a UTF-8 text file named by a public IRI.
    pub fn read_text(&self, url: &str) -> Result<String, String> {
        String::from_utf8(self.read(url)?).map_err(|e| format!("{url} is not UTF-8: {e}"))
    }
}

/// RDF syntax of a data or result file, from its extension.
pub fn rdf_format(url: &str) -> Option<RdfFormat> {
    let ext = url.rsplit_once('.')?.1;
    match ext.to_ascii_lowercase().as_str() {
        "ttl" => Some(RdfFormat::Turtle),
        "nt" => Some(RdfFormat::NTriples),
        "nq" => Some(RdfFormat::NQuads),
        "trig" => Some(RdfFormat::TriG),
        "rdf" | "owl" | "xml" => Some(RdfFormat::RdfXml),
        "n3" => Some(RdfFormat::N3),
        _ => None,
    }
}

/// Parses an RDF document into `dataset`. Triples go to `graph` (None = default graph);
/// quads of TriG / N-Quads keep their graph names. Blank nodes get fresh labels, so separate
/// documents (or two loads of one document) never share blank nodes.
pub fn parse_rdf_into(
    dataset: &mut Dataset,
    data: &[u8],
    format: RdfFormat,
    base: &str,
    graph: Option<NamedNodeRef<'_>>,
) -> Result<(), String> {
    let mut parser = RdfParser::from_format(format)
        .with_base_iri(base)
        .map_err(|e| format!("invalid base IRI {base}: {e}"))?
        .rename_blank_nodes();
    if let Some(graph) = graph {
        parser = parser.with_default_graph(GraphName::NamedNode(graph.into_owned()));
    }
    for quad in parser.for_slice(data) {
        let quad = quad.map_err(|e| format!("cannot parse {base}: {e}"))?;
        dataset.insert(&quad);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_paths_and_urls() {
        let suite = Suite::new(std::env::temp_dir()).unwrap();
        let url = suite.url("sparql11/add/manifest.ttl");
        assert_eq!(url, "https://w3c.github.io/rdf-tests/sparql/sparql11/add/manifest.ttl");
        assert_eq!(suite.relative(&url), Some("sparql11/add/manifest.ttl"));
        assert_eq!(suite.relative(&format!("{url}#add01")), Some("sparql11/add/manifest.ttl"));
        assert_eq!(suite.path(&url), Some(std::env::temp_dir().join("sparql11/add/manifest.ttl")));
        assert_eq!(suite.relative("http://example.org/data.ttl"), None);
        assert_eq!(suite.relative("https://w3c.github.io/rdf-tests/sparql/../secret"), None);
        assert_eq!(suite.relative(PUBLIC_BASE), None);
    }

    #[test]
    fn formats_by_extension() {
        assert_eq!(rdf_format("a/b.ttl"), Some(RdfFormat::Turtle));
        assert_eq!(rdf_format("a/b.NT"), Some(RdfFormat::NTriples));
        assert_eq!(rdf_format("a/b.trig"), Some(RdfFormat::TriG));
        assert_eq!(rdf_format("a/b.rdf"), Some(RdfFormat::RdfXml));
        assert_eq!(rdf_format("a/b.srx"), None);
        assert_eq!(rdf_format("noext"), None);
    }

    #[test]
    fn parses_into_graphs_with_fresh_blank_nodes() {
        let mut ds = Dataset::new();
        let g = NamedNodeRef::new_unchecked("http://ex/g");
        let data = b"@prefix : <http://ex/> . _:a :p <rel> .";
        parse_rdf_into(&mut ds, data, RdfFormat::Turtle, "http://base/dir/f.ttl", None).unwrap();
        parse_rdf_into(&mut ds, data, RdfFormat::Turtle, "http://base/dir/f.ttl", Some(g)).unwrap();
        assert_eq!(ds.len(), 2);
        let objects: Vec<String> = ds.iter().map(|q| q.object.to_string()).collect();
        assert!(objects.iter().all(|o| o == "<http://base/dir/rel>"), "{objects:?}");
        let subjects: std::collections::HashSet<String> = ds.iter().map(|q| q.subject.to_string()).collect();
        assert_eq!(subjects.len(), 2, "each load gets its own blank nodes");
        assert_eq!(ds.iter().filter(|q| q.graph_name == g.into()).count(), 1);
    }
}
