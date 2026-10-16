//! Contracts between the expression evaluator (sq-expr) and its hosts (sq-exec, sq-oracle).
use std::sync::Arc;

use rustc_hash::FxHashMap;

use crate::error::Result;
use crate::id::{Tag, TermId};
use crate::ir::ExistsId;
use crate::term::Term;

/// Execution-local terms: computed values and query constants absent from the store.
/// Never persisted (writers reject TEMP ids). Equal terms get equal TEMP ids.
#[derive(Clone, Debug, Default)]
pub struct TempDict {
    terms: Vec<Arc<Term>>,
    index: FxHashMap<Arc<Term>, TermId>,
}

impl TempDict {
    pub fn new() -> Self {
        Self::default()
    }
    /// Caller guarantees: `t` is not inline-encodable and (if canonical ids are required) not
    /// in the store. Blank nodes passed here are treated as named (label-keyed) TEMP bnodes.
    pub fn intern(&mut self, t: Term) -> TermId {
        if let Some(id) = self.index.get(&t) {
            return *id;
        }
        let tag = match &t {
            Term::Iri(_) => Tag::Iri,
            Term::BNode(_) => Tag::BNode,
            Term::Literal(l) if l.lang.is_some() => Tag::Lang,
            Term::Literal(l) if l.is_simple() => Tag::Str,
            Term::Literal(_) => Tag::Typed,
        };
        let id = TermId::temp(self.terms.len() as u32, tag);
        let t = Arc::new(t);
        self.terms.push(t.clone());
        self.index.insert(t, id);
        id
    }
    /// A brand-new blank node (BNODE(), CONSTRUCT template bnodes): label "t<index>".
    pub fn fresh_bnode(&mut self) -> TermId {
        let idx = self.terms.len() as u32;
        let t = Arc::new(Term::BNode(format!("t{idx}").into()));
        let id = TermId::temp(idx, Tag::BNode);
        self.terms.push(t.clone());
        self.index.insert(t, id);
        id
    }
    pub fn get(&self, id: TermId) -> Option<&Arc<Term>> {
        self.terms.get(id.temp_index()? as usize)
    }
    pub fn len(&self) -> usize {
        self.terms.len()
    }
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }
}

/// Implemented by sq-exec (store dictionary + DecodeCache + TempDict) and by sq-oracle.
pub trait TermResolver {
    /// Decode any bound id: dictionary, bnode, inline or TEMP. UNBOUND/ERROR = Error::Internal.
    fn term(&mut self, id: TermId) -> Result<Arc<Term>>;
    /// Id of a computed term. `canonical = true`: inline id, else dictionary id if the term is
    /// stored, else TEMP id (required whenever the value can meet stored ids: joins, DISTINCT,
    /// GROUP keys, aggregate DISTINCT, MINUS, VALUES, sameTerm, hash keys, EXISTS seeds,
    /// CONSTRUCT dedupe). `canonical = false`: inline id, else TEMP id (output-only values).
    fn intern(&mut self, t: &Term, canonical: bool) -> Result<TermId>;
    /// Fresh TEMP blank node.
    fn fresh_bnode(&mut self) -> TermId;
}

/// EXISTS / NOT EXISTS callback: evaluate `exists[id]` with every bound slot of `row`
/// substituted (SeedMode::Substitute). Hosts memoize per tuple of correlated slots.
pub trait ExistsEval {
    fn exists(&mut self, id: ExistsId, row: &[TermId]) -> Result<bool>;
}

/// A stream of SELECT solutions in projection order (native executor or baseline SQL).
/// Result writers (sq-results) and the SQL surface (sq-api) consume only this trait.
pub trait SolutionSource {
    fn variables(&self) -> &[String];
    /// Append up to `max_rows` rows of `variables().len()` ids each (UNBOUND = 0).
    /// Returns rows appended; 0 = exhausted.
    fn next_rows(&mut self, out: &mut Vec<TermId>, max_rows: usize) -> Result<usize>;
    /// Decode ids produced by this source (dictionary, bnode, inline, TEMP); UNBOUND -> None.
    /// Implementations batch, sort and cache dictionary misses.
    fn decode(&mut self, ids: &[TermId], out: &mut Vec<Option<Arc<Term>>>) -> Result<()>;
}

/// CONSTRUCT / DESCRIBE output: a SET of triples (already deduplicated), first-seen order.
pub trait TripleSource {
    fn next_triples(&mut self, out: &mut Vec<[TermId; 3]>, max: usize) -> Result<usize>;
    fn decode(&mut self, ids: &[TermId], out: &mut Vec<Option<Arc<Term>>>) -> Result<()>;
}
