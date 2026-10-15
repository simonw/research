//! The oracle's `TermResolver`: the store dictionary for stored ids, the query's `TempDict`
//! (extended with computed terms and fresh blank nodes) for TEMP ids.
//!
//! Every id the oracle creates is canonical (§3): inline when encodable, the dictionary id
//! when the term is stored, the store id for a store blank-node label, a TEMP id otherwise.
//! That keeps sameTerm ⇔ id equality for joins, DISTINCT, GROUP BY and CONSTRUCT dedupe.
use std::sync::Arc;

use rustc_hash::FxHashMap;
use sq_core::codec;
use sq_core::eval_api::{TempDict, TermResolver};
use sq_core::store::Dictionary;
use sq_core::{Error, Result, Tag, Term, TermId};

pub(crate) struct OracleResolver<'s> {
    dict: &'s dyn Dictionary,
    /// Starts as a copy of the lowered query's TempDict, so TEMP constants decode.
    temps: TempDict,
    /// Decoded dictionary ids.
    decoded: FxHashMap<TermId, Arc<Term>>,
}

impl<'s> OracleResolver<'s> {
    pub(crate) fn new(dict: &'s dyn Dictionary, temps: TempDict) -> Self {
        OracleResolver { dict, temps, decoded: FxHashMap::default() }
    }
}

impl TermResolver for OracleResolver<'_> {
    fn term(&mut self, id: TermId) -> Result<Arc<Term>> {
        if id.is_inline() {
            return codec::decode_inline(id)
                .map(Arc::new)
                .ok_or_else(|| Error::internal(format!("sq-oracle: invalid inline id {}", id.0)));
        }
        match id.tag() {
            Tag::Temp => {
                self.temps.get(id).cloned().ok_or_else(|| Error::internal(format!("sq-oracle: unknown TEMP id {}", id.0)))
            }
            Tag::BNode => Ok(Arc::new(Term::BNode(codec::bnode_label(id).into()))),
            Tag::Iri | Tag::Str | Tag::Lang | Tag::Typed if id.is_dict() => {
                if let Some(t) = self.decoded.get(&id) {
                    return Ok(t.clone());
                }
                let t = self.dict.decode_one(id)?;
                self.decoded.insert(id, t.clone());
                Ok(t)
            }
            _ => Err(Error::internal(format!("sq-oracle: cannot decode term id {}", id.0))),
        }
    }

    fn intern(&mut self, t: &Term, canonical: bool) -> Result<TermId> {
        if let Term::Literal(l) = t
            && let Some(id) = codec::encode_inline_literal(&l.lex, &l.datatype, l.lang.as_deref())
        {
            return Ok(id);
        }
        // A store blank node keeps its identity whatever `canonical` says.
        if let Term::BNode(label) = t
            && let Some(id) = codec::parse_bnode_label(label)
        {
            return Ok(id);
        }
        if canonical && let Some(id) = self.dict.lookup(t.to_oxrdf().as_ref())? {
            return Ok(id);
        }
        Ok(self.temps.intern(t.clone()))
    }

    fn fresh_bnode(&mut self) -> TermId {
        self.temps.fresh_bnode()
    }
}

#[cfg(test)]
mod tests {
    use sq_core::store::QuadStoreWriter;
    use sq_core::Literal;
    use sq_memstore::MemStore;

    use super::*;

    #[test]
    fn ids_are_canonical_and_round_trip() {
        let store = MemStore::new();
        let stored = store.intern(oxrdf::NamedNodeRef::new_unchecked("http://ex/stored").into()).unwrap();
        let mut temps = TempDict::new();
        let absent = temps.intern(Term::Iri("http://ex/absent".into()));
        let mut r = OracleResolver::new(&store, temps);

        // Dictionary probe for canonical ids; TEMP otherwise; TEMP constants keep their id.
        assert_eq!(r.intern(&Term::Iri("http://ex/stored".into()), true).unwrap(), stored);
        assert!(r.intern(&Term::Iri("http://ex/stored".into()), false).unwrap().is_temp());
        assert_eq!(r.intern(&Term::Iri("http://ex/absent".into()), true).unwrap(), absent);
        assert_eq!(&*r.term(absent).unwrap(), &Term::Iri("http://ex/absent".into()));
        assert_eq!(&*r.term(stored).unwrap(), &Term::Iri("http://ex/stored".into()));

        // Inline literals never touch the dictionary.
        let one = r.intern(&Term::Literal(Literal::typed("1", codec::XSD_INTEGER_IRI)), true).unwrap();
        assert_eq!(one, TermId::from_int(1).unwrap());
        assert_eq!(&*r.term(one).unwrap(), &Term::Literal(Literal::typed("1", codec::XSD_INTEGER_IRI)));

        // Store blank nodes keep their counter id; fresh ones are TEMP and decode to their label.
        let b = store.fresh_bnode().unwrap();
        let label = r.term(b).unwrap();
        assert_eq!(r.intern(&label, false).unwrap(), b);
        let f = r.fresh_bnode();
        let ft = r.term(f).unwrap();
        assert!(matches!(&*ft, Term::BNode(_)));
        assert_eq!(r.intern(&ft, true).unwrap(), f);

        assert!(r.term(TermId::UNBOUND).is_err());
        assert!(r.term(TermId::ERROR).is_err());
    }
}
