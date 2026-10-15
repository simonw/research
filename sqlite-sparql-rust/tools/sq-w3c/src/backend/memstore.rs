//! MemStore helpers shared by the `mem` and `oracle` backends.
use std::collections::HashMap;

use oxrdf::{Dataset, GraphName, NamedOrBlankNode, Quad, Term};
use sq_core::store::QuadStore;
use sq_core::TermId;
use sq_memstore::MemStore;

/// The whole store as an oxrdf dataset (graph id 0 = default graph).
pub fn dump(store: &MemStore) -> Result<Dataset, String> {
    let quads = store.quads();
    let mut ids: Vec<TermId> = quads.iter().flatten().copied().filter(|id| id.is_bound()).collect();
    ids.sort_unstable();
    ids.dedup();
    let mut decoded = Vec::with_capacity(ids.len());
    store.dict().decode(&ids, &mut decoded).map_err(|e| format!("cannot decode the store: {e}"))?;
    let terms: HashMap<TermId, Term> = ids.iter().copied().zip(decoded.iter().map(|t| t.to_oxrdf())).collect();

    let mut dataset = Dataset::new();
    for [g, s, p, o] in quads {
        let term = |id: TermId| terms.get(&id).cloned().ok_or_else(|| format!("undecoded id {id:?}"));
        let subject = match term(s)? {
            Term::NamedNode(n) => NamedOrBlankNode::from(n),
            Term::BlankNode(b) => b.into(),
            Term::Literal(l) => return Err(format!("store holds a quad with literal subject {l}")),
        };
        let Term::NamedNode(predicate) = term(p)? else {
            return Err(format!("store holds a quad with non-IRI predicate {p:?}"));
        };
        let graph = if g == TermId::DEFAULT_GRAPH {
            GraphName::DefaultGraph
        } else {
            match term(g)? {
                Term::NamedNode(n) => n.into(),
                Term::BlankNode(b) => b.into(),
                Term::Literal(l) => return Err(format!("store holds a quad in literal graph {l}")),
            }
        };
        dataset.insert(&Quad::new(subject, predicate, term(o)?, graph));
    }
    Ok(dataset)
}
