//! `QuadStoreWriter` for SQLite. Atomicity is the caller's (a `WriteScope`); behaviour matches
//! `sq_memstore::MemStore` exactly (CLEAR keeps graphs registered, `drop_graph` only touches
//! the registry, inserts register every named graph they mention).
use sq_core::codec::{self, FIRST_USER_ROW};
use sq_core::store::GraphTarget;
use sq_core::{Error, Result, Tag, TermId};

use crate::analyze::GROWTH_SINCE_ANALYZE;
use crate::keys::KeyView;
use crate::raw::RawStmt;
use crate::shared::StorageShared;
use crate::template::{Fixed, QUAD_INSERT_BATCH};

impl StorageShared {
    /// `QuadStoreWriter::intern`.
    pub(crate) fn intern_term(&self, t: oxrdf::TermRef<'_>) -> Result<TermId> {
        if let Some(id) = codec::encode_inline(t) {
            return Ok(id);
        }
        if let oxrdf::TermRef::BlankNode(_) = t {
            return Err(Error::Invalid("blank nodes cannot be interned; use fresh_bnode()".into()));
        }
        let view = KeyView::of(t).ok_or_else(|| Error::Invalid(format!("cannot intern {t}")))?;
        self.require_ready()?;
        self.intern_key(&view)
    }

    pub(crate) fn intern_key(&self, view: &KeyView<'_>) -> Result<TermId> {
        if let Some(id) = self.lookup_key(view)? {
            return Ok(id);
        }
        // A TYPED literal's datatype IRI is interned first and referenced by the dt column.
        let dt = if view.tag == Tag::Typed { Some(self.intern_key(&KeyView::iri(&view.extra))?) } else { None };
        let row = self.next_dict_row()?;
        self.insert_term_row(row, view, dt, view.hash64())?;
        Ok(TermId::from_dict(row, view.tag))
    }

    /// `max(max(id) + 1, FIRST_USER_ROW)` (valid under the write lock).
    pub(crate) fn next_dict_row(&self) -> Result<i64> {
        let mut st = self.pool.lease_fixed(Fixed::TermMaxRow)?;
        st.step()?;
        Ok(st.col_opt_i64(0).map_or(FIRST_USER_ROW, |m| (m + 1).max(FIRST_USER_ROW)))
    }

    pub(crate) fn insert_term_row(&self, row: i64, view: &KeyView<'_>, dt: Option<TermId>, h: i64) -> Result<()> {
        let mut st = self.pool.lease_fixed(Fixed::TermInsert)?;
        bind_term_row(&mut st, 1, row, view, dt, h)?;
        st.run()?;
        self.count(|c| c.terms_inserted += 1);
        Ok(())
    }

    pub(crate) fn meta_get(&self, key: &str) -> Result<Option<i64>> {
        let mut st = self.pool.lease_fixed(Fixed::MetaGet)?;
        st.bind_text(1, key)?;
        Ok(if st.step()? { st.col_opt_i64(0) } else { None })
    }

    pub(crate) fn meta_set(&self, key: &str, value: i64) -> Result<()> {
        let mut st = self.pool.lease_fixed(Fixed::MetaSet)?;
        st.bind_text(1, key)?;
        st.bind_i64(2, value)?;
        st.run()
    }

    /// Adds `delta` to a counter row, creating it (with `delta`) if absent.
    pub(crate) fn meta_add(&self, key: &str, delta: i64) -> Result<()> {
        let mut st = self.pool.lease_fixed(Fixed::MetaAdd)?;
        st.bind_text(1, key)?;
        st.bind_i64(2, delta)?;
        st.run()
    }

    /// Adds `delta` (quads inserted minus removed) to the re-analysis counter
    /// (`GROWTH_SINCE_ANALYZE`); one upsert per write call, none for a no-op.
    pub(crate) fn note_growth(&self, delta: i64) -> Result<()> {
        if delta == 0 {
            return Ok(());
        }
        self.meta_add(GROWTH_SINCE_ANALYZE, delta)
    }

    pub(crate) fn meta_bump(&self, key: &str) -> Result<()> {
        let mut st = self.pool.lease_fixed(Fixed::MetaBump)?;
        st.bind_text(1, key)?;
        st.run()
    }

    /// `QuadStoreWriter::fresh_bnode`: allocates from `rdf_meta.next_bnode`.
    pub(crate) fn fresh_bnode_id(&self) -> Result<TermId> {
        self.require_ready()?;
        let n = self.meta_get("next_bnode")?.unwrap_or(1).max(1);
        self.meta_set("next_bnode", n + 1)?;
        Ok(TermId::bnode(n))
    }

    /// Runs a pooled single-quad statement over sorted, deduplicated quads; returns the
    /// number of changed rows (`sqlite3_changes`). Inserts go `QUAD_INSERT_BATCH` rows per
    /// statement.
    pub(crate) fn apply_quads(&self, which: Fixed, quads: &[[TermId; 4]]) -> Result<u64> {
        let db = self.db();
        let mut changed = 0u64;
        let mut rest = quads;
        if which == Fixed::QuadInsert && quads.len() >= QUAD_INSERT_BATCH {
            let mut many = self.pool.lease_fixed(Fixed::QuadInsertMany)?;
            let mut chunks = quads.chunks_exact(QUAD_INSERT_BATCH);
            for chunk in &mut chunks {
                many.reset();
                for (i, v) in chunk.iter().flatten().enumerate() {
                    many.bind_i64(i as i32 + 1, v.0)?;
                }
                many.run()?;
                changed += db.changes() as u64;
            }
            rest = chunks.remainder();
        }
        let mut st = self.pool.lease_fixed(which)?;
        for q in rest {
            st.reset();
            for (i, v) in q.iter().enumerate() {
                st.bind_i64(i as i32 + 1, v.0)?;
            }
            st.run()?;
            changed += db.changes() as u64;
        }
        Ok(changed)
    }

    pub(crate) fn register_graphs(&self, quads: &[[TermId; 4]]) -> Result<()> {
        let mut st = self.pool.lease_fixed(Fixed::GraphRegister)?;
        let mut last = TermId::DEFAULT_GRAPH;
        for q in quads {
            let g = q[0];
            if g != TermId::DEFAULT_GRAPH && g != last {
                st.reset();
                st.bind_i64(1, g.0)?;
                st.run()?;
                last = g;
            }
        }
        Ok(())
    }

    /// `QuadStoreWriter::insert`.
    pub(crate) fn insert_quads(&self, quads: &mut Vec<[TermId; 4]>) -> Result<u64> {
        quads.sort_unstable();
        quads.dedup();
        if let Some(q) = quads.iter().find(|q| !storable_quad(q)) {
            return Err(Error::internal(format!("insert of non-storable quad {q:?}")));
        }
        if quads.is_empty() {
            return Ok(0);
        }
        self.require_ready()?;
        let n = self.apply_quads(Fixed::QuadInsert, quads)?;
        self.register_graphs(quads)?;
        self.note_growth(changed_rows(n)?)?;
        Ok(n)
    }

    /// `QuadStoreWriter::remove`.
    pub(crate) fn remove_quads(&self, quads: &mut Vec<[TermId; 4]>) -> Result<u64> {
        quads.sort_unstable();
        quads.dedup();
        if quads.is_empty() || !self.tables_present()? {
            return Ok(0);
        }
        let n = self.apply_quads(Fixed::QuadDelete, quads)?;
        self.note_growth(-changed_rows(n)?)?;
        Ok(n)
    }

    /// `QuadStoreWriter::clear` (the registry is left alone, as in MemStore).
    pub(crate) fn clear_quads(&self, target: GraphTarget) -> Result<u64> {
        if !self.tables_present()? {
            return Ok(0);
        }
        let (which, g) = match target {
            GraphTarget::Default => (Fixed::ClearDefault, None),
            GraphTarget::Named(g) => (Fixed::ClearGraph, Some(g)),
            GraphTarget::AllNamed => (Fixed::ClearNamed, None),
            GraphTarget::All => (Fixed::ClearAll, None),
        };
        let mut st = self.pool.lease_fixed(which)?;
        if let Some(g) = g {
            st.bind_i64(1, g.0)?;
        }
        st.run()?;
        let n = self.db().changes();
        self.note_growth(-n)?;
        Ok(n as u64)
    }

    fn graph_flag(&self, which: Fixed, g: TermId) -> Result<bool> {
        let mut st = self.pool.lease_fixed(which)?;
        st.bind_i64(1, g.0)?;
        st.step()
    }

    /// `QuadStoreWriter::create_graph`: Ok(false) if the graph already existed (registered or
    /// holding quads).
    pub(crate) fn create_graph_id(&self, g: TermId) -> Result<bool> {
        if !g.is_storable() || g == TermId::DEFAULT_GRAPH {
            return Err(Error::Invalid(format!("not a storable graph name: {g:?}")));
        }
        self.require_ready()?;
        let existed = self.graph_flag(Fixed::GraphRegistered, g)? || self.graph_flag(Fixed::GraphHasQuads, g)?;
        let mut st = self.pool.lease_fixed(Fixed::GraphRegister)?;
        st.bind_i64(1, g.0)?;
        st.run()?;
        Ok(!existed)
    }

    /// `QuadStoreWriter::drop_graph`: removes the registry entry; Ok(false) if absent.
    pub(crate) fn drop_graph_id(&self, g: TermId) -> Result<bool> {
        if !self.tables_present()? {
            return Ok(false);
        }
        let mut st = self.pool.lease_fixed(Fixed::GraphUnregister)?;
        st.bind_i64(1, g.0)?;
        st.run()?;
        Ok(self.db().changes() > 0)
    }

    /// Registry ∪ distinct `g <> 0` of `rdf_quad` (loose index scan of the primary key), sorted.
    pub(crate) fn named_graph_ids(&self) -> Result<Vec<TermId>> {
        if !self.tables_present()? {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        {
            let mut st = self.pool.lease_fixed(Fixed::Registry)?;
            while st.step()? {
                out.push(TermId(st.col_i64(0)));
            }
        }
        let mut st = self.pool.lease_fixed(Fixed::NextGraph)?;
        let mut from = i64::MIN;
        loop {
            st.reset();
            st.bind_i64(1, from)?;
            if !st.step()? {
                break;
            }
            let g = st.col_i64(0);
            if g != 0 {
                out.push(TermId(g));
            }
            match g.checked_add(1) {
                Some(next) => from = next,
                None => break,
            }
        }
        out.sort_unstable();
        out.dedup();
        Ok(out)
    }
}

/// Binds one `rdf_term` row `(id, tag, lex, dt, lang, h)` at parameters `first..first+6`.
pub(crate) fn bind_term_row(
    st: &mut RawStmt,
    first: i32,
    row: i64,
    view: &KeyView<'_>,
    dt: Option<TermId>,
    h: i64,
) -> Result<()> {
    st.bind_i64(first, row)?;
    st.bind_i64(first + 1, view.tag as i64)?;
    st.bind_text(first + 2, view.lex)?;
    st.bind_opt_i64(first + 3, dt.map(TermId::raw))?;
    st.bind_opt_text(first + 4, (view.tag == Tag::Lang).then_some(&*view.extra))?;
    st.bind_i64(first + 5, h)
}

/// A row count as the signed delta of the re-analysis counter.
pub(crate) fn changed_rows(n: u64) -> Result<i64> {
    i64::try_from(n).map_err(|_| Error::internal("row count overflows i64"))
}

/// `g` is the default graph or storable; s, p, o are storable.
pub(crate) fn storable_quad(q: &[TermId; 4]) -> bool {
    (q[0] == TermId::DEFAULT_GRAPH || q[0].is_storable()) && q[1..].iter().all(|t| t.is_storable())
}
