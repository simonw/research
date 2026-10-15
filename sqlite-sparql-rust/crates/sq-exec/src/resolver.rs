//! The execution's `TermResolver` (§4.3).
//!
//! Decoding an id:
//! 1. UNBOUND / ERROR: an internal error (callers map them to "no term" first);
//! 2. TEMP: the execution's `TempDict` (a clone of the plan's, grown by computed terms);
//! 3. inline and blank-node ids: `codec::decode_inline` / `bnode_label`, no I/O;
//! 4. dictionary ids: the per-execution map, then `store.dict().decode` — batched, sorted and
//!    deduplicated for result decoding ([`Terms::decode_batch`]); the store's dictionary
//!    applies the connection-level DecodeCache.
//!
//! Interning a computed term (`intern(t, canonical)`): inline-encodable literals get their
//! inline id and store blank-node labels their counter id; otherwise, when a canonical id is
//! required, the store dictionary is probed (results cached per execution, including misses)
//! and a miss falls back to the `TempDict`; non-canonical interns go straight to the `TempDict`.
use std::sync::Arc;

use rustc_hash::FxHashMap;
use sq_core::codec;
use sq_core::eval_api::{TempDict, TermResolver};
use sq_core::store::QuadStore;
use sq_core::{Error, Result, Tag, Term, TermId};

use crate::ctx::Shared;

/// Bound on the per-execution decoded-term map (cleared when full).
const DECODED_LIMIT: usize = 1 << 17;
/// Bound on the per-execution canonical-lookup cache (cleared when full).
const LOOKUP_LIMIT: usize = 1 << 16;

/// Execution-local term state behind the resolver.
pub(crate) struct Terms {
    pub temps: TempDict,
    decoded: FxHashMap<TermId, Arc<Term>>,
    /// Canonical ids of interned terms (dictionary ids, or TEMP ids for terms the store lacks).
    canonical: FxHashMap<Term, TermId>,
    pub hits: u64,
    pub misses: u64,
}

impl Terms {
    pub fn new(temps: TempDict) -> Self {
        Terms { temps, decoded: FxHashMap::default(), canonical: FxHashMap::default(), hits: 0, misses: 0 }
    }

    /// Decodes a bound id without dictionary I/O, if possible (inline, blank node, TEMP).
    fn decode_local(&self, id: TermId) -> Result<Option<Arc<Term>>> {
        if id.is_inline() {
            let t = codec::decode_inline(id).ok_or_else(|| Error::internal(format!("bad inline id {}", id.0)))?;
            return Ok(Some(Arc::new(t)));
        }
        match id.tag() {
            Tag::BNode => Ok(Some(Arc::new(Term::BNode(codec::bnode_label(id).into())))),
            Tag::Temp => self
                .temps
                .get(id)
                .cloned()
                .map(Some)
                .ok_or_else(|| Error::internal(format!("unknown TEMP id {}", id.0))),
            Tag::Iri | Tag::Str | Tag::Lang | Tag::Typed if id.is_dict() => Ok(None),
            _ => Err(Error::internal(format!("cannot decode term id {}", id.0))),
        }
    }

    fn remember(&mut self, id: TermId, t: Arc<Term>) {
        if self.decoded.len() >= DECODED_LIMIT {
            self.decoded.clear();
        }
        self.decoded.insert(id, t);
    }

    pub fn term(&mut self, store: &dyn QuadStore, id: TermId) -> Result<Arc<Term>> {
        if let Some(t) = self.decode_local(id)? {
            return Ok(t);
        }
        if let Some(t) = self.decoded.get(&id) {
            self.hits += 1;
            return Ok(t.clone());
        }
        self.misses += 1;
        let t = store.dict().decode_one(id)?;
        self.remember(id, t.clone());
        Ok(t)
    }

    /// Decodes `ids` (UNBOUND and ERROR give None), appending one entry per id. Dictionary
    /// misses of the whole slice are decoded in one sorted, deduplicated call.
    pub fn decode_batch(
        &mut self,
        store: &dyn QuadStore,
        ids: &[TermId],
        out: &mut Vec<Option<Arc<Term>>>,
    ) -> Result<()> {
        let mut misses: Vec<TermId> =
            ids.iter().copied().filter(|id| id.is_dict() && !self.decoded.contains_key(id)).collect();
        if !misses.is_empty() {
            misses.sort_unstable();
            misses.dedup();
            let mut terms = Vec::with_capacity(misses.len());
            store.dict().decode(&misses, &mut terms)?;
            if terms.len() != misses.len() {
                return Err(Error::internal("dictionary decode returned a wrong number of terms"));
            }
            self.misses += misses.len() as u64;
            if self.decoded.len() + misses.len() > DECODED_LIMIT {
                self.decoded.clear();
            }
            for (id, t) in misses.into_iter().zip(terms) {
                self.decoded.insert(id, t);
            }
        }
        out.reserve(ids.len());
        for &id in ids {
            if !id.is_bound() || id == TermId::ERROR {
                out.push(None);
            } else if id.is_dict() {
                match self.decoded.get(&id) {
                    Some(t) => {
                        self.hits += 1;
                        out.push(Some(t.clone()));
                    }
                    // Evicted by the bound above within this very call: decode it alone.
                    None => out.push(Some(self.term(store, id)?)),
                }
            } else {
                out.push(self.decode_local(id)?);
            }
        }
        Ok(())
    }

    pub fn intern(&mut self, store: &dyn QuadStore, t: &Term, canonical: bool) -> Result<TermId> {
        match t {
            Term::Literal(l) => {
                if let Some(id) = codec::encode_inline_literal(&l.lex, &l.datatype, l.lang.as_deref()) {
                    return Ok(id);
                }
            }
            Term::BNode(label) => {
                if let Some(id) = codec::parse_bnode_label(label) {
                    return Ok(id);
                }
                // TEMP blank nodes ("t<index>") are found by label in the TempDict.
                return Ok(self.temps.intern(t.clone()));
            }
            Term::Iri(_) => {}
        }
        if !canonical {
            return Ok(self.temps.intern(t.clone()));
        }
        if let Some(id) = self.canonical.get(t) {
            return Ok(*id);
        }
        let id = match store.dict().lookup(t.to_oxrdf().as_ref())? {
            Some(id) => id,
            None => self.temps.intern(t.clone()),
        };
        if self.canonical.len() >= LOOKUP_LIMIT {
            self.canonical.clear();
        }
        self.canonical.insert(t.clone(), id);
        Ok(id)
    }
}

/// `TermResolver` handle over the execution's shared term state. Each call borrows the state
/// only for its own duration, so EXISTS evaluations nested inside an expression evaluation can
/// use their own handle.
pub(crate) struct Resolver<'a> {
    pub sh: &'a Shared,
}

impl TermResolver for Resolver<'_> {
    fn term(&mut self, id: TermId) -> Result<Arc<Term>> {
        self.sh.terms.borrow_mut().term(&*self.sh.store, id)
    }

    fn intern(&mut self, t: &Term, canonical: bool) -> Result<TermId> {
        self.sh.terms.borrow_mut().intern(&*self.sh.store, t, canonical)
    }

    fn fresh_bnode(&mut self) -> TermId {
        self.sh.terms.borrow_mut().temps.fresh_bnode()
    }
}
