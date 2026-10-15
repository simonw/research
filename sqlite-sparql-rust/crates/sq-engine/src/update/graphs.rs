//! Graph management operations: `LOAD`, `CLEAR`, `CREATE` and `DROP` (SPARQL 1.1 Update
//! §3.1.4–§3.2.2). A named graph "exists" iff it is one of the store's `named_graphs()` (the
//! registry, plus every graph that holds quads).
use std::path::PathBuf;

use oxrdf::NamedNode;
use oxrdfio::RdfFormat;
use spargebra::algebra::GraphTarget as UpdateTarget;
use spargebra::term::GraphName;
use sq_core::store::GraphTarget;
use sq_core::{Error, Result, TermId};

use super::{silenced, Updater};
use crate::load;

impl Updater<'_> {
    /// The id of `g` if it is an existing named graph.
    fn existing_graph(&self, g: &NamedNode) -> Result<Option<TermId>> {
        let Some(id) = self.store.dict().lookup(g.as_ref().into())? else {
            return Ok(None);
        };
        Ok(self.store.named_graphs()?.contains(&id).then_some(id))
    }

    fn missing(g: &NamedNode) -> Error {
        Error::Invalid(format!("graph {g} does not exist"))
    }

    /// `CLEAR`: removes the triples; the graphs themselves stay (CLEAR of a named graph that
    /// does not exist is an error unless SILENT).
    pub(super) fn clear(&mut self, silent: bool, target: &UpdateTarget) -> Result<()> {
        let target = match target {
            UpdateTarget::NamedNode(g) => match self.existing_graph(g)? {
                Some(id) => GraphTarget::Named(id),
                None => return silenced(silent, Err(Self::missing(g))),
            },
            UpdateTarget::DefaultGraph => GraphTarget::Default,
            UpdateTarget::NamedGraphs => GraphTarget::AllNamed,
            UpdateTarget::AllGraphs => GraphTarget::All,
        };
        self.report.deleted += self.store.clear(target)?;
        Ok(())
    }

    /// `CREATE`: registers an empty named graph (an error if it exists, unless SILENT).
    pub(super) fn create(&mut self, silent: bool, g: &NamedNode) -> Result<()> {
        if self.existing_graph(g)?.is_some() {
            return silenced(silent, Err(Error::Invalid(format!("graph {g} already exists"))));
        }
        let id = self.store.intern(g.as_ref().into())?;
        self.store.create_graph(id)?;
        Ok(())
    }

    /// `DROP`: clears the graph(s) and removes them from the registry. `DROP DEFAULT` is
    /// `CLEAR DEFAULT` (the default graph always exists); dropping a missing named graph is an
    /// error unless SILENT.
    pub(super) fn drop_graphs(&mut self, silent: bool, target: &UpdateTarget) -> Result<()> {
        let named = match target {
            UpdateTarget::NamedNode(g) => match self.existing_graph(g)? {
                Some(id) => {
                    self.report.deleted += self.store.clear(GraphTarget::Named(id))?;
                    vec![id]
                }
                None => return silenced(silent, Err(Self::missing(g))),
            },
            UpdateTarget::DefaultGraph => {
                self.report.deleted += self.store.clear(GraphTarget::Default)?;
                Vec::new()
            }
            UpdateTarget::NamedGraphs => {
                let graphs = self.store.named_graphs()?;
                self.report.deleted += self.store.clear(GraphTarget::AllNamed)?;
                graphs
            }
            UpdateTarget::AllGraphs => {
                let graphs = self.store.named_graphs()?;
                self.report.deleted += self.store.clear(GraphTarget::All)?;
                graphs
            }
        };
        for g in named {
            self.store.drop_graph(g)?;
        }
        Ok(())
    }

    /// `LOAD`: only `file://` IRIs, and only when local files are allowed. The document is
    /// read and parsed completely before anything is written, so `SILENT` can drop a bad
    /// document without leaving part of it behind.
    pub(super) fn load(&mut self, silent: bool, source: &NamedNode, destination: &GraphName) -> Result<()> {
        let graph = match destination {
            GraphName::NamedNode(g) => Some(g.as_str()),
            GraphName::DefaultGraph => None,
        };
        let quads = match self.fetch(source).and_then(|(data, format)| {
            load::parse_document(&data, format, Some(source.as_str()), graph)
        }) {
            Ok(quads) => quads,
            Err(e) => return silenced(silent, Err(e)),
        };
        self.report.inserted += load::insert_quads(&*self.store, &quads)?.quads_new;
        if let GraphName::NamedNode(g) = destination {
            // LOAD INTO GRAPH creates the graph even for an empty document.
            let id = self.store.intern(g.as_ref().into())?;
            self.store.create_graph(id)?;
        }
        Ok(())
    }

    /// Reads a `file://` document and picks its format from the file extension.
    fn fetch(&self, source: &NamedNode) -> Result<(Vec<u8>, RdfFormat)> {
        let path = file_path(source.as_str()).ok_or_else(|| {
            Error::Unsupported(format!("LOAD {source}: only file:// IRIs can be loaded"))
        })?;
        if !self.allow_files {
            return Err(Error::Unsupported(format!(
                "LOAD {source}: reading local files is disabled (sparql_config('allow_files', 1))"
            )));
        }
        let format = path
            .extension()
            .and_then(|e| e.to_str())
            .and_then(RdfFormat::from_extension)
            .ok_or_else(|| Error::Invalid(format!("LOAD {source}: unknown RDF format (file extension)")))?;
        let data = std::fs::read(&path).map_err(|e| Error::Invalid(format!("LOAD {source}: {e}")))?;
        Ok((data, format))
    }
}

/// The local path of a `file:` IRI (`file:///p`, `file://localhost/p` or `file:/p`), with
/// percent-escapes decoded; None for other schemes and remote hosts.
fn file_path(iri: &str) -> Option<PathBuf> {
    let rest = iri.strip_prefix("file:")?;
    let path = match rest.strip_prefix("//") {
        Some(authority_and_path) => {
            let slash = authority_and_path.find('/')?;
            let (host, path) = authority_and_path.split_at(slash);
            if !(host.is_empty() || host.eq_ignore_ascii_case("localhost")) {
                return None;
            }
            path
        }
        None => rest,
    };
    if !path.starts_with('/') {
        return None;
    }
    // Drop a query or fragment, then decode %XX escapes.
    let path = path.split(['?', '#']).next().unwrap_or(path);
    let mut bytes = Vec::with_capacity(path.len());
    let raw = path.as_bytes();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' && i + 2 < raw.len() {
            let hex = std::str::from_utf8(&raw[i + 1..i + 3]).ok()?;
            bytes.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            bytes.push(raw[i]);
            i += 1;
        }
    }
    Some(PathBuf::from(String::from_utf8(bytes).ok()?))
}

#[cfg(test)]
mod tests {
    use super::file_path;
    use std::path::PathBuf;

    #[test]
    fn file_iris_map_to_local_paths() {
        assert_eq!(file_path("file:///tmp/a.ttl"), Some(PathBuf::from("/tmp/a.ttl")));
        assert_eq!(file_path("file://localhost/tmp/a.ttl"), Some(PathBuf::from("/tmp/a.ttl")));
        assert_eq!(file_path("file:/tmp/a%20b.ttl"), Some(PathBuf::from("/tmp/a b.ttl")));
        assert_eq!(file_path("file:///tmp/a.ttl#frag"), Some(PathBuf::from("/tmp/a.ttl")));
        assert_eq!(file_path("file://example.org/tmp/a.ttl"), None);
        assert_eq!(file_path("http://example.org/a.ttl"), None);
        assert_eq!(file_path("file:relative.ttl"), None);
        assert_eq!(file_path("file:///tmp/bad%zz"), None);
        assert_eq!(file_path("file:///tmp/trailing%2"), Some(PathBuf::from("/tmp/trailing%2")));
    }
}
