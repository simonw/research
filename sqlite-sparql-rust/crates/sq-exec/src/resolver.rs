//! The execution's `TermResolver` (§4.3).
//!
//! Decoding an id:
//! 1. UNBOUND / ERROR: an internal error (callers map them to "no term" first);
//! 2. TEMP: the execution's `TempDict` (a clone of the plan's, grown by computed terms);
//! 3. inline and blank-node ids: `codec::decode_inline` / `bnode_label`, no I/O;
//! 4. dictionary ids: the batch prefetched by an operator ([`Terms::prefetch`]), then
//!    `store.dict()`, whose connection-level DecodeCache (sq-sqlite) already caches decoded
//!    terms across queries. There is no per-execution decoded-term map on top of it: for
//!    single-use ids (a FILTER over distinct names) such a map only cost an insert and its
//!    rehashes per id, and sq-expr's `ValueCache` caches the values of repeated ids.
//!    Result decoding ([`Terms::decode_batch`]) hands a whole batch to the store in one call,
//!    which sorts and deduplicates its misses.
//!
//! Interning a computed term (`intern(t, canonical)`): inline-encodable literals get their
//! inline id and store blank-node labels their counter id; otherwise, when a canonical id is
//! required, the store dictionary is probed (results cached per execution, including misses)
//! and a miss falls back to the `TempDict`; non-canonical interns go straight to the `TempDict`.
//!
//! The `TempDict` only grows during an execution (the canonical-lookup map is bounded), so
//! every TEMP term it adds is charged against the memory budget.
use std::sync::Arc;

use rustc_hash::FxHashMap;
use sq_core::codec;
use sq_core::eval_api::{TempDict, TermResolver};
use sq_core::store::QuadStore;
use sq_core::{Batch, Error, Result, Slot, Tag, Term, TermId};

use crate::ctx::{Charge, Shared};

/// Bound on the per-execution canonical-lookup cache (cleared when full).
const LOOKUP_LIMIT: usize = 1 << 16;
/// Bytes of a TEMP term besides its strings: the `Arc<Term>`, its `TempDict` slot and index
/// entry.
const TEMP_TERM_BYTES: usize = size_of::<Term>() + 64;
/// Bytes per string of a term (the `Arc<str>` header).
const STR_BYTES: usize = 16;

/// Memory held by a TEMP term.
fn temp_bytes(t: &Term) -> usize {
    let strings = match t {
        Term::Iri(s) | Term::BNode(s) => STR_BYTES + s.len(),
        Term::Literal(l) => {
            2 * STR_BYTES + l.lex.len() + l.datatype.len() + l.lang.as_ref().map_or(0, |g| STR_BYTES + g.len())
        }
    };
    TEMP_TERM_BYTES + strings
}

/// Execution-local term state behind the resolver.
pub(crate) struct Terms {
    temps: TempDict,
    /// Strings decoded ahead of an operator's row-by-row evaluation ([`Terms::prefetch`]).
    pre: Prefetched,
    /// Canonical ids of interned terms (dictionary ids, or TEMP ids for terms the store lacks).
    canonical: FxHashMap<Term, TermId>,
    /// The TEMP terms added by this execution (held until it ends).
    charge: Charge,
    /// Dictionary ids served by a prefetched batch (no store request of their own).
    pub hits: u64,
    /// Dictionary ids requested from the store, one by one or in a batch call (counted per
    /// id). Inline, blank-node and TEMP ids are decoded locally and not counted.
    pub misses: u64,
}

impl Terms {
    pub fn new(temps: TempDict) -> Self {
        Terms {
            temps,
            pre: Prefetched::default(),
            canonical: FxHashMap::default(),
            charge: Charge::default(),
            hits: 0,
            misses: 0,
        }
    }

    /// The TEMP id of `t`, charging the term when it is new.
    fn temp(&mut self, sh: &Shared, t: &Term) -> Result<TermId> {
        let n = self.temps.len();
        let id = self.temps.intern(t.clone());
        if self.temps.len() > n {
            sh.charge(&mut self.charge, temp_bytes(t))?;
        }
        Ok(id)
    }

    /// A fresh TEMP blank node (BNODE(), CONSTRUCT template blank nodes). Its memory is
    /// recorded without a budget check (the resolver's `fresh_bnode` cannot fail): an exceeded
    /// budget is reported by the next charge or interrupt poll.
    pub fn fresh_bnode(&mut self, sh: &Shared) -> TermId {
        let id = self.temps.fresh_bnode();
        if let Some(t) = self.temps.get(id) {
            sh.record(&mut self.charge, temp_bytes(t));
        }
        id
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

    pub fn term(&mut self, store: &dyn QuadStore, id: TermId) -> Result<Arc<Term>> {
        if !id.is_dict() {
            return self.decode_local(id)?.ok_or_else(|| Error::internal(format!("cannot decode term id {}", id.0)));
        }
        if let Some(t) = self.pre.get(id) {
            self.hits += 1;
            return Ok(t);
        }
        self.misses += 1;
        store.dict().decode_one(id)
    }

    /// Decodes, in one store call, the dictionary ids of tag STR or LANG found in `slots` of
    /// the rows `from..` of `rows` (an operator's input, before its row-by-row evaluation),
    /// and keeps them, aligned with the rows, until the next prefetch. The operator then
    /// announces each row it evaluates with [`Terms::at_row`]. `slots` comes from
    /// `CompiledExpr::string_slots`, so these are ids the evaluation decodes anyway; the store
    /// sorts and deduplicates the misses, which a cold DecodeCache reads in index order instead
    /// of one point read per row.
    pub fn prefetch(&mut self, store: &dyn QuadStore, rows: &Batch, from: usize, slots: &[Slot]) -> Result<()> {
        let p = &mut self.pre;
        p.clear();
        let mut want = std::mem::take(&mut p.want);
        for i in from..rows.len() {
            let row = rows.row(i);
            for &s in slots {
                let id = row.get(s as usize).copied().unwrap_or(TermId::UNBOUND);
                if id.is_dict() && matches!(id.tag(), Tag::Str | Tag::Lang) {
                    p.ids.push(id);
                    want.push(id);
                } else {
                    p.ids.push(TermId::UNBOUND);
                }
            }
        }
        let r = if want.is_empty() {
            Ok(())
        } else {
            let mut got = std::mem::take(&mut p.got);
            let r = store.dict().decode(&want, &mut got);
            if r.is_ok() && got.len() == want.len() {
                self.misses += want.len() as u64;
                let mut got = got.drain(..);
                p.terms.extend(p.ids.iter().map(|id| if id.is_bound() { got.next() } else { None }));
                p.width = slots.len();
            }
            got.clear();
            p.got = got;
            r
        };
        want.clear();
        p.want = want;
        if p.width == 0 {
            p.clear();
        }
        r
    }

    /// The operator is about to evaluate row `from + i` of the batch it prefetched.
    #[inline]
    pub fn at_row(&mut self, i: usize) {
        self.pre.row = i;
    }

    /// Decodes `ids` (UNBOUND and ERROR give None), appending one entry per id. The
    /// dictionary ids of the whole slice go to the store in one call (it sorts and
    /// deduplicates its misses).
    pub fn decode_batch(
        &mut self,
        store: &dyn QuadStore,
        ids: &[TermId],
        out: &mut Vec<Option<Arc<Term>>>,
    ) -> Result<()> {
        let dict: Vec<TermId> = ids.iter().copied().filter(|id| id.is_dict()).collect();
        let mut terms = Vec::with_capacity(dict.len());
        if !dict.is_empty() {
            store.dict().decode(&dict, &mut terms)?;
            if terms.len() != dict.len() {
                return Err(Error::internal("dictionary decode returned a wrong number of terms"));
            }
            self.misses += dict.len() as u64;
        }
        let mut terms = terms.into_iter();
        out.reserve(ids.len());
        for &id in ids {
            if !id.is_bound() || id == TermId::ERROR {
                out.push(None);
            } else if id.is_dict() {
                out.push(Some(terms.next().ok_or_else(|| Error::internal("dictionary decode ran short"))?));
            } else {
                out.push(self.decode_local(id)?);
            }
        }
        Ok(())
    }

    pub fn intern(&mut self, sh: &Shared, t: &Term, canonical: bool) -> Result<TermId> {
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
                return self.temp(sh, t);
            }
            Term::Iri(_) => {}
        }
        if !canonical {
            return self.temp(sh, t);
        }
        if let Some(id) = self.canonical.get(t) {
            return Ok(*id);
        }
        let id = match sh.store.dict().lookup(t.to_oxrdf().as_ref())? {
            Some(id) => id,
            None => self.temp(sh, t)?,
        };
        if self.canonical.len() >= LOOKUP_LIMIT {
            self.canonical.clear();
        }
        self.canonical.insert(t.clone(), id);
        Ok(id)
    }
}

/// The terms of [`Terms::prefetch`]: for row `r` (relative to the first prefetched row) and
/// its `j`-th slot, `ids[r * width + j]` is the id (UNBOUND when it was not decoded) and
/// `terms[r * width + j]` its term. Lookups only consult the current row (`row`, set by the
/// operator), so they cost one comparison per slot; a hit is correct whatever the row,
/// because a term is a function of its id.
#[derive(Default)]
struct Prefetched {
    width: usize,
    row: usize,
    ids: Vec<TermId>,
    terms: Vec<Option<Arc<Term>>>,
    /// Scratch: the ids handed to the store and the terms it returned.
    want: Vec<TermId>,
    got: Vec<Arc<Term>>,
}

impl Prefetched {
    fn clear(&mut self) {
        self.width = 0;
        self.row = 0;
        self.ids.clear();
        self.terms.clear();
    }

    #[inline]
    fn get(&self, id: TermId) -> Option<Arc<Term>> {
        if self.width == 0 {
            return None;
        }
        let at = self.row * self.width;
        let ids = self.ids.get(at..at + self.width)?;
        let j = ids.iter().position(|x| *x == id)?;
        self.terms[at + j].clone()
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
        self.sh.terms.borrow_mut().intern(self.sh, t, canonical)
    }

    fn fresh_bnode(&mut self) -> TermId {
        self.sh.terms.borrow_mut().fresh_bnode(self.sh)
    }
}
