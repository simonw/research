//! The RDF dataset D of a query (§13, docs/SPEC.md §5.6) and triple matching against one of
//! its graphs.
//!
//! A graph of the dataset is a list of stored graphs (`g` values of `rdf_quad`, 0 being the
//! store's default graph). A list of several stored graphs is their RDF merge: a set of
//! triples, so a triple stored in two of them matches once. Graph names that are not stored
//! (TEMP ids) are empty graphs.
use sq_core::ir::{DatasetSpec, DefaultGraph, GraphSel, NamedGraphs};
use sq_core::store::{Col, IndexOrder, QuadStore, ScanSpec};
use sq_core::{Result, TermId};

/// Which graph of the dataset patterns currently read (§18.6 D(G)).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum ActiveGraph {
    /// The dataset's default graph.
    Default,
    /// A named graph of the dataset (inside `GRAPH`).
    Named(TermId),
}

/// One graph of the dataset: the stored graphs it merges.
#[derive(Copy, Clone, Debug)]
pub(crate) enum Graph<'a> {
    Merge(&'a [TermId]),
    One(TermId),
}

impl Graph<'_> {
    fn stored(&self) -> &[TermId] {
        match self {
            Graph::Merge(gs) => gs,
            Graph::One(g) => std::slice::from_ref(g),
        }
    }
}

/// D resolved against the store.
pub(crate) struct Dataset {
    /// Stored graphs merged into the default graph.
    default: Vec<TermId>,
    /// Names of the named graphs, sorted (they may be absent from the store).
    named: Vec<TermId>,
}

impl Dataset {
    pub(crate) fn resolve(spec: &DatasetSpec, store: &dyn QuadStore) -> Result<Self> {
        let named = match &spec.named {
            NamedGraphs::All => store.named_graphs()?,
            NamedGraphs::Only(list) => sorted(list.to_vec()),
        };
        let default = match &spec.default {
            DefaultGraph::StoreDefault => vec![TermId::DEFAULT_GRAPH],
            DefaultGraph::UnionAll => {
                let mut all = store.named_graphs()?;
                all.insert(0, TermId::DEFAULT_GRAPH);
                sorted(all)
            }
            DefaultGraph::Graphs(list) => sorted(list.to_vec()),
        };
        Ok(Dataset { default, named })
    }

    pub(crate) fn named(&self) -> &[TermId] {
        &self.named
    }

    pub(crate) fn is_named(&self, g: TermId) -> bool {
        self.named.binary_search(&g).is_ok()
    }

    /// The graph a pattern with graph selector `sel` reads while `active` is the active graph.
    /// `GraphSel::Slot` ranges over the named graphs and is expanded by the caller into one
    /// `GraphSel::Const` per graph; here it reads nothing.
    pub(crate) fn graph(&self, sel: GraphSel, active: ActiveGraph) -> Graph<'_> {
        match (sel, active) {
            (GraphSel::Const(g), _) | (GraphSel::Active, ActiveGraph::Named(g)) => Graph::One(g),
            (GraphSel::Default | GraphSel::Active, _) => Graph::Merge(&self.default),
            (GraphSel::Slot(_), _) => {
                debug_assert!(false, "GraphSel::Slot must be expanded by the caller");
                Graph::Merge(&[])
            }
        }
    }
}

fn sorted(mut v: Vec<TermId>) -> Vec<TermId> {
    v.sort_unstable();
    v.dedup();
    v
}

/// Can `id` occur in `rdf_quad`? TEMP ids (terms absent from the store) never match.
fn storable(id: TermId) -> bool {
    id.is_storable()
}

/// Reads triples of a dataset graph through `QuadStore` cursors.
#[derive(Copy, Clone)]
pub(crate) struct GraphReader<'a> {
    pub(crate) store: &'a dyn QuadStore,
}

impl GraphReader<'_> {
    /// Appends the distinct triples `[s, p, o]` of `graph` that match the fixed positions.
    pub(crate) fn triples(
        &self,
        graph: Graph<'_>,
        s: Option<TermId>,
        p: Option<TermId>,
        o: Option<TermId>,
        out: &mut Vec<[TermId; 3]>,
    ) -> Result<()> {
        self.store.check_interrupt()?;
        if [s, p, o].into_iter().flatten().any(|id| !storable(id)) {
            return Ok(());
        }
        let start = out.len();
        let stored = graph.stored();
        for &g in stored {
            if g != TermId::DEFAULT_GRAPH && !storable(g) {
                continue;
            }
            self.scan_graph(g, s, p, o, out)?;
        }
        if stored.len() > 1 {
            // RDF merge: a graph is a set of triples.
            let mut merged = out.split_off(start);
            merged.sort_unstable();
            merged.dedup();
            out.extend(merged);
        }
        Ok(())
    }

    /// nodes(G): every term used as a subject or object of a triple of `graph`, sorted.
    pub(crate) fn nodes(&self, graph: Graph<'_>) -> Result<Vec<TermId>> {
        let mut triples = Vec::new();
        self.triples(graph, None, None, None, &mut triples)?;
        let mut nodes: Vec<TermId> = triples.iter().flat_map(|[s, _, o]| [*s, *o]).collect();
        nodes.sort_unstable();
        nodes.dedup();
        Ok(nodes)
    }

    /// One stored graph: a range scan on any index whose prefix is exactly the fixed columns.
    fn scan_graph(
        &self,
        g: TermId,
        s: Option<TermId>,
        p: Option<TermId>,
        o: Option<TermId>,
        out: &mut Vec<[TermId; 3]>,
    ) -> Result<()> {
        let fixed = [Some(g), s, p, o];
        let bound = fixed.map(|v| v.is_some());
        let (order, k) = *IndexOrder::candidates(bound)
            .first()
            .ok_or_else(|| sq_core::Error::internal("sq-oracle: no index order for a graph-first scan"))?;
        let cols = order.cols();
        let k = k as usize;
        let prefix: Vec<TermId> =
            cols[..k].iter().map(|c| fixed[*c as usize].expect("prefix column is fixed")).collect();
        let mut cursor = self.store.cursor(&ScanSpec::new(order, &prefix))?;
        let width = cursor.width();
        let mut buf = Vec::new();
        loop {
            buf.clear();
            let n = cursor.fill(&mut buf, 1024)?;
            if n == 0 {
                break;
            }
            // Rows are `width` free columns each; width 0 (every column fixed) still counts rows.
            for row in (0..n).map(|i| &buf[i * width..(i + 1) * width]) {
                let mut quad = fixed.map(|v| v.unwrap_or(TermId::UNBOUND));
                for (col, v) in cols[k..].iter().zip(row) {
                    quad[*col as usize] = *v;
                }
                debug_assert!(quad[Col::G as usize] == g);
                out.push([quad[Col::S as usize], quad[Col::P as usize], quad[Col::O as usize]]);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sq_core::store::QuadStoreWriter;
    use sq_memstore::MemStore;

    use super::*;

    fn iri(store: &MemStore, s: &str) -> TermId {
        store.intern(oxrdf::NamedNodeRef::new_unchecked(s).into()).unwrap()
    }

    #[test]
    fn merged_default_graph_is_a_set() {
        let store = MemStore::new();
        let (g1, g2, a, p, b) = (
            iri(&store, "http://g/1"),
            iri(&store, "http://g/2"),
            iri(&store, "http://ex/a"),
            iri(&store, "http://ex/p"),
            iri(&store, "http://ex/b"),
        );
        store.insert(&mut vec![[g1, a, p, b], [g2, a, p, b], [g2, b, p, a], [TermId::DEFAULT_GRAPH, a, p, a]]).unwrap();
        let reader = GraphReader { store: &store };
        let spec = DatasetSpec {
            default: DefaultGraph::Graphs(Arc::from(vec![g2, g1])),
            named: NamedGraphs::Only(Arc::from(vec![])),
        };
        let ds = Dataset::resolve(&spec, &store).unwrap();
        let mut out = Vec::new();
        reader.triples(ds.graph(GraphSel::Default, ActiveGraph::Default), None, Some(p), None, &mut out).unwrap();
        assert_eq!(out.len(), 2, "{out:?}");
        assert!(ds.named().is_empty());

        let union = DatasetSpec { default: DefaultGraph::UnionAll, named: NamedGraphs::All };
        let all = Dataset::resolve(&union, &store).unwrap();
        out.clear();
        reader.triples(all.graph(GraphSel::Default, ActiveGraph::Default), Some(a), None, None, &mut out).unwrap();
        assert_eq!(out.len(), 2, "{out:?}");
        assert_eq!(all.named(), &[g1, g2]);
        assert_eq!(reader.nodes(all.graph(GraphSel::Default, ActiveGraph::Default)).unwrap().len(), 2);

        // Fixed TEMP terms and TEMP graphs never match.
        let mut temps = sq_core::eval_api::TempDict::new();
        let absent = temps.intern(sq_core::Term::Iri("http://ex/absent".into()));
        out.clear();
        reader.triples(Graph::One(absent), None, None, None, &mut out).unwrap();
        reader.triples(Graph::One(g1), Some(absent), None, None, &mut out).unwrap();
        assert!(out.is_empty());
        reader.triples(Graph::One(g1), Some(a), Some(p), Some(b), &mut out).unwrap();
        assert_eq!(out, vec![[a, p, b]]);
    }
}
