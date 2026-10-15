//! Dictionary read paths (§4.3): `lookup` (inline, store bnode labels and well-known IRIs ->
//! no I/O; LookupCache then a hash probe of `rdf_term_h` with exact compare) and `decode`
//! (inline / bnode / well-known rows without I/O; DecodeCache; sorted, deduplicated misses
//! fetched by pooled point lookups or, for large batches, one merge scan). Only rows admitted
//! by the §4.4 rule enter the connection caches; negative lookups are never cached.
use std::sync::Arc;

use sq_core::codec::{self, bnode_label, decode_inline};
use sq_core::term::{Literal, Term};
use sq_core::{Error, Result, Tag, TermId};

use crate::keys::{well_known_iri, KeyLike, KeyView};
use crate::raw::RawStmt;
use crate::shared::StorageShared;
use crate::template::Fixed;

/// A batch switches from point lookups to a merge scan when it needs more than 1/20 of the
/// committed dictionary rows.
const MERGE_SCAN_DIVISOR: usize = 20;
/// The merge scan re-seeks instead of stepping over gaps larger than this many rows.
const MERGE_SCAN_MAX_GAP: i64 = 16;
/// Approximate fixed overhead of a cached term (Arc, enum, map slot), in bytes.
const ENTRY_OVERHEAD: usize = 96;

/// One fetched `rdf_term` row.
struct Row {
    id: TermId,
    lex: Arc<str>,
    dt: i64,
    lang: Option<Arc<str>>,
}

impl StorageShared {
    /// `Dictionary::lookup`.
    pub(crate) fn lookup_term(&self, t: oxrdf::TermRef<'_>) -> Result<Option<TermId>> {
        if let Some(id) = codec::encode_inline(t) {
            return Ok(Some(id));
        }
        if let oxrdf::TermRef::BlankNode(b) = t {
            return Ok(codec::parse_bnode_label(b.as_str()));
        }
        let Some(view) = KeyView::of(t) else { return Ok(None) };
        if !self.tables_present()? {
            // Before sparql_init only the well-known IRIs exist: their ids are fixed, and
            // codec::inline_datatype hands them out without any dictionary access.
            return Ok(if view.tag == Tag::Iri { well_known_iri(view.lex) } else { None });
        }
        self.lookup_key(&view)
    }

    /// Id of an existing dictionary term (IRI or non-inline literal).
    pub(crate) fn lookup_key(&self, view: &KeyView<'_>) -> Result<Option<TermId>> {
        if view.tag == Tag::Iri
            && let Some(id) = well_known_iri(view.lex)
        {
            return Ok(Some(id));
        }
        let h = view.hash64();
        {
            let mut cache = self.lookup_cache.try_borrow_mut().map_err(|_| Error::Reentrant)?;
            if let Some((key, id)) = cache.get(h)
                && (key as &dyn KeyLike) == view.as_dyn()
            {
                return Ok(Some(*id));
            }
        }
        let dt = if view.tag == Tag::Typed {
            match self.lookup_key(&KeyView::iri(&view.extra))? {
                Some(id) => id,
                // The datatype IRI is interned before any literal of that type.
                None => return Ok(None),
            }
        } else {
            TermId::UNBOUND
        };
        let found = self.probe(h, view, dt)?;
        if let Some(id) = found
            && self.admissible(id.dict_row())
        {
            self.lookup_cache.try_borrow_mut().map_err(|_| Error::Reentrant)?.insert(h, (view.to_owned_key(), id), 1);
        }
        Ok(found)
    }

    /// Hash probe with exact compare (collisions are legal: `rdf_term_h` is not UNIQUE).
    fn probe(&self, h: i64, view: &KeyView<'_>, dt: TermId) -> Result<Option<TermId>> {
        self.count(|c| c.probes += 1);
        let mut st = self.pool.lease_fixed(Fixed::TermByHash)?;
        st.bind_i64(1, h)?;
        while st.step()? {
            if row_matches(&st, 1, view, dt) {
                return Ok(Some(TermId::from_dict(st.col_i64(0), view.tag)));
            }
        }
        Ok(None)
    }

    /// `Dictionary::decode`. On error `out` is left as it was.
    pub(crate) fn decode_ids(&self, ids: &[TermId], out: &mut Vec<Arc<Term>>) -> Result<()> {
        self.count(|c| c.decode_calls += 1);
        let base = out.len();
        let r = self.decode_into(ids, out, base);
        if r.is_err() {
            out.truncate(base);
        }
        r
    }

    fn decode_into(&self, ids: &[TermId], out: &mut Vec<Arc<Term>>, base: usize) -> Result<()> {
        out.reserve(ids.len());
        let mut misses: Vec<(usize, TermId)> = Vec::new();
        {
            let mut cache = self.decode_cache.try_borrow_mut().map_err(|_| Error::Reentrant)?;
            for (i, &id) in ids.iter().enumerate() {
                let t = if id.is_inline() {
                    Arc::new(decode_inline(id).ok_or_else(|| Error::internal(format!("invalid inline id {id:?}")))?)
                } else if id.tag() == Tag::BNode && id.is_storable() {
                    Arc::new(Term::BNode(bnode_label(id).into()))
                } else if let Some(t) = self.well_known_term(id) {
                    t.clone()
                } else if id.is_dict() {
                    match cache.get(id.0) {
                        Some(t) => t.clone(),
                        None => {
                            misses.push((base + i, id));
                            self.placeholder.clone()
                        }
                    }
                } else {
                    return Err(Error::internal(format!("decode of non-storable id {id:?}")));
                };
                out.push(t);
            }
        }
        if misses.is_empty() {
            return Ok(());
        }
        let mut want: Vec<TermId> = misses.iter().map(|m| m.1).collect();
        want.sort_unstable();
        want.dedup();
        let terms = self.fetch_terms(&want)?;
        for (pos, id) in misses {
            let k = want.binary_search(&id).expect("every miss was fetched");
            out[pos] = terms[k].clone();
        }
        Ok(())
    }

    /// Fetches sorted, distinct dictionary ids; returns their terms in the same order and
    /// admits committed rows to the DecodeCache.
    fn fetch_terms(&self, ids: &[TermId]) -> Result<Vec<Arc<Term>>> {
        let committed = self.committed_max_row();
        let rows = if committed > 0 && ids.len() * MERGE_SCAN_DIVISOR > committed as usize {
            self.count(|c| c.decode_merge_scans += 1);
            self.fetch_merge(ids)?
        } else {
            self.fetch_points(ids)?
        };
        self.count(|c| c.decode_db_rows += rows.len() as u64);
        // Datatype IRIs of TYPED rows: from this batch, the cache, or one more fetch.
        let mut cached_dt: Vec<(TermId, Arc<Term>)> = Vec::new();
        let mut missing_dt: Vec<TermId> = Vec::new();
        {
            let mut cache = self.decode_cache.try_borrow_mut().map_err(|_| Error::Reentrant)?;
            for r in rows.iter().filter(|r| r.id.tag() == Tag::Typed) {
                let dt = TermId(r.dt);
                if ids.binary_search(&dt).is_ok() || cached_dt.iter().any(|(d, _)| *d == dt) {
                    continue;
                }
                if let Some(t) = self.well_known_term(dt) {
                    cached_dt.push((dt, t.clone()));
                    continue;
                }
                match cache.get(dt.0) {
                    Some(t) => cached_dt.push((dt, t.clone())),
                    None => missing_dt.push(dt),
                }
            }
        }
        missing_dt.sort_unstable();
        missing_dt.dedup();
        let extra_dt = if missing_dt.is_empty() { Vec::new() } else { self.fetch_terms(&missing_dt)? };

        let mut terms: Vec<Arc<Term>> = Vec::with_capacity(rows.len());
        // IRIs first, so TYPED rows of the same batch can share their datatype string.
        let mut built: Vec<Option<Arc<Term>>> = vec![None; rows.len()];
        for (i, r) in rows.iter().enumerate() {
            if r.id.tag() != Tag::Typed {
                built[i] = Some(Arc::new(self.build_term(r, None)?));
            }
        }
        for (i, r) in rows.iter().enumerate() {
            if built[i].is_some() {
                continue;
            }
            let dt = TermId(r.dt);
            let dt_term = if let Ok(k) = ids.binary_search(&dt) {
                built[k].clone()
            } else if let Ok(k) = missing_dt.binary_search(&dt) {
                Some(extra_dt[k].clone())
            } else {
                cached_dt.iter().find(|(d, _)| *d == dt).map(|(_, t)| t.clone())
            };
            let Some(dt_term) = dt_term else {
                return Err(Error::internal(format!("datatype {dt:?} of {:?} is not a stored IRI", r.id)));
            };
            let Term::Iri(dt_iri) = &*dt_term else {
                return Err(Error::internal(format!("datatype {dt:?} of {:?} is not an IRI", r.id)));
            };
            built[i] = Some(Arc::new(self.build_term(r, Some(dt_iri.clone()))?));
        }
        let mut cache = self.decode_cache.try_borrow_mut().map_err(|_| Error::Reentrant)?;
        for (r, t) in rows.iter().zip(built) {
            let t = t.expect("every row was built");
            if self.admissible(r.id.dict_row()) {
                let cost = ENTRY_OVERHEAD + r.lex.len() + r.lang.as_ref().map_or(0, |l| l.len());
                cache.insert(r.id.0, t.clone(), cost);
            }
            terms.push(t);
        }
        Ok(terms)
    }

    fn build_term(&self, r: &Row, datatype: Option<Arc<str>>) -> Result<Term> {
        Ok(match r.id.tag() {
            Tag::Iri => Term::Iri(r.lex.clone()),
            Tag::Str => Term::Literal(Literal { lex: r.lex.clone(), datatype: self.iris.xsd_string.clone(), lang: None }),
            Tag::Lang => Term::Literal(Literal {
                lex: r.lex.clone(),
                datatype: self.iris.lang_string.clone(),
                lang: Some(r.lang.clone().ok_or_else(|| Error::internal(format!("LANG row {:?} has no lang", r.id)))?),
            }),
            Tag::Typed => Term::Literal(Literal {
                lex: r.lex.clone(),
                datatype: datatype.ok_or_else(|| Error::internal("TYPED row without datatype"))?,
                lang: None,
            }),
            other => return Err(Error::internal(format!("dictionary id {:?} has tag {other:?}", r.id))),
        })
    }

    /// Pooled `SELECT … WHERE id=?1` per id (sorted ids keep the B-tree path warm).
    fn fetch_points(&self, ids: &[TermId]) -> Result<Vec<Row>> {
        let mut st = self.pool.lease_fixed(Fixed::TermById)?;
        let mut rows = Vec::with_capacity(ids.len());
        for &id in ids {
            st.reset();
            st.bind_i64(1, id.dict_row())?;
            if !st.step()? {
                return Err(unknown(id));
            }
            rows.push(read_row(&st, id, 0)?);
        }
        Ok(rows)
    }

    /// One forward scan `WHERE id >= ?1 ORDER BY id`, re-seeking over large gaps.
    fn fetch_merge(&self, ids: &[TermId]) -> Result<Vec<Row>> {
        let mut st = self.pool.lease_fixed(Fixed::TermScanFrom)?;
        let mut rows = Vec::with_capacity(ids.len());
        let mut cur: Option<i64> = None;
        for &id in ids {
            let want = id.dict_row();
            match cur {
                Some(c) if c <= want && want - c <= MERGE_SCAN_MAX_GAP => {
                    while let Some(c) = cur {
                        if c >= want {
                            break;
                        }
                        cur = if st.step()? { Some(st.col_i64(0)) } else { None };
                    }
                }
                _ => {
                    st.reset();
                    st.bind_i64(1, want)?;
                    cur = if st.step()? { Some(st.col_i64(0)) } else { None };
                }
            }
            if cur != Some(want) {
                return Err(unknown(id));
            }
            rows.push(read_row(&st, id, 1)?);
        }
        Ok(rows)
    }
}

fn unknown(id: TermId) -> Error {
    Error::internal(format!("unknown dictionary id {id:?}"))
}

/// Reads `tag, lex, dt, lang` starting at column `c0` and checks the tag against the id.
fn read_row(st: &RawStmt, id: TermId, c0: i32) -> Result<Row> {
    if st.col_i64(c0) != id.tag() as i64 {
        return Err(Error::internal(format!("dictionary row {} has tag {}, id {id:?}", id.dict_row(), st.col_i64(c0))));
    }
    Ok(Row {
        id,
        lex: Arc::from(st.col_str(c0 + 1)?),
        dt: st.col_opt_i64(c0 + 2).unwrap_or(0),
        lang: st.col_opt_str(c0 + 3)?.map(Arc::from),
    })
}

/// Exact compare of a `(id, tag, lex, dt, lang)` row (tag at column `c0`) with a key.
pub(crate) fn row_matches(st: &RawStmt, c0: i32, view: &KeyView<'_>, dt: TermId) -> bool {
    st.col_i64(c0) == view.tag as i64
        && st.col_bytes(c0 + 1) == view.lex.as_bytes()
        && match view.tag {
            Tag::Typed => st.col_i64(c0 + 2) == dt.raw(),
            Tag::Lang => st.col_bytes(c0 + 3) == view.extra.as_bytes(),
            _ => true,
        }
}
