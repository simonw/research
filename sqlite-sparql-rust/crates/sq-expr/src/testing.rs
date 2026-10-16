//! Test doubles (feature `testing`): an in-memory `TermResolver` with the store's id
//! discipline (inline ids, well-known dictionary rows, counter blank nodes, TEMP ids for
//! everything else), an `ExistsEval` stub and an environment that builds `EvalCtx`s.
use std::str::FromStr;
use std::sync::Arc;

use rand::SeedableRng;
use rand::rngs::StdRng;
use rustc_hash::FxHashMap;
use sq_core::codec::{self, FIRST_USER_ROW, WELL_KNOWN};
use sq_core::eval_api::{ExistsEval, TempDict, TermResolver};
use sq_core::ir::ExistsId;
use sq_core::{Error, Literal, Result, Tag, Term, TermId};

use crate::{EvalCtx, ValueCache};

/// A dictionary-backed resolver: `add` stores terms (inline ids, dictionary rows starting at
/// `FIRST_USER_ROW`, counter blank nodes); anything else interned gets a TEMP id.
pub struct TestResolver {
    rows: FxHashMap<i64, Arc<Term>>,
    index: FxHashMap<Term, TermId>,
    next_row: i64,
    next_bnode: i64,
    pub temps: TempDict,
}

impl Default for TestResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl TestResolver {
    pub fn new() -> Self {
        let mut r = TestResolver {
            rows: FxHashMap::default(),
            index: FxHashMap::default(),
            next_row: FIRST_USER_ROW,
            next_bnode: 1,
            temps: TempDict::new(),
        };
        for (row, iri) in WELL_KNOWN {
            let t = Term::Iri((*iri).into());
            let id = TermId::from_dict(*row, Tag::Iri);
            r.rows.insert(*row, Arc::new(t.clone()));
            r.index.insert(t, id);
        }
        r
    }

    /// Stores a term (as a loader would) and returns its canonical id.
    pub fn add(&mut self, t: Term) -> TermId {
        if let Term::Literal(l) = &t
            && let Some(id) = codec::encode_inline_literal(&l.lex, &l.datatype, l.lang.as_deref())
        {
            return id;
        }
        if let Some(id) = self.index.get(&t) {
            return *id;
        }
        let id = match &t {
            Term::BNode(_) => {
                let id = TermId::bnode(self.next_bnode);
                self.next_bnode += 1;
                id
            }
            _ => {
                let tag = codec::dict_tag(t.to_oxrdf().as_ref()).expect("IRI or literal");
                let id = TermId::from_dict(self.next_row, tag);
                self.rows.insert(self.next_row, Arc::new(t.clone()));
                self.next_row += 1;
                id
            }
        };
        self.index.insert(t, id);
        id
    }

    pub fn iri(&mut self, iri: &str) -> TermId {
        self.add(Term::Iri(iri.into()))
    }

    pub fn simple(&mut self, lex: &str) -> TermId {
        self.add(Term::Literal(Literal::simple(lex)))
    }

    pub fn lang(&mut self, lex: &str, lang: &str) -> TermId {
        self.add(Term::Literal(Literal::lang(lex, lang)))
    }

    /// Typed literal; `dt` may be an XSD local name (`"integer"`) or a full IRI.
    pub fn typed(&mut self, lex: &str, dt: &str) -> TermId {
        let dt = if dt.contains(':') { dt.to_string() } else { format!("{}{dt}", codec::XSD) };
        self.add(Term::Literal(Literal::typed(lex, &dt)))
    }

    pub fn bnode(&mut self) -> TermId {
        let id = TermId::bnode(self.next_bnode);
        self.next_bnode += 1;
        id
    }

    /// A TEMP id for a term that is not stored (as lowering does for absent constants).
    pub fn temp(&mut self, t: Term) -> TermId {
        self.temps.intern(t)
    }

    /// Decodes an id for assertions.
    pub fn show(&mut self, id: Option<TermId>) -> String {
        match id {
            None => "ERROR".into(),
            Some(id) => self.term(id).map(|t| t.to_ntriples()).unwrap_or_else(|e| format!("<{e}>")),
        }
    }
}

impl TermResolver for TestResolver {
    fn term(&mut self, id: TermId) -> Result<Arc<Term>> {
        if id.is_inline() {
            return codec::decode_inline(id).map(Arc::new).ok_or_else(|| Error::internal("bad inline id"));
        }
        match id.tag() {
            Tag::BNode => Ok(Arc::new(Term::BNode(codec::bnode_label(id).into()))),
            Tag::Temp => self.temps.get(id).cloned().ok_or_else(|| Error::internal("unknown TEMP id")),
            Tag::Iri | Tag::Str | Tag::Lang | Tag::Typed => {
                self.rows.get(&id.dict_row()).cloned().ok_or_else(|| Error::internal("unknown dictionary id"))
            }
            _ => Err(Error::internal(format!("cannot decode id {}", id.0))),
        }
    }

    fn intern(&mut self, t: &Term, canonical: bool) -> Result<TermId> {
        if let Term::Literal(l) = t
            && let Some(id) = codec::encode_inline_literal(&l.lex, &l.datatype, l.lang.as_deref())
        {
            return Ok(id);
        }
        if let Term::BNode(label) = t
            && let Some(id) = codec::parse_bnode_label(label)
        {
            return Ok(id);
        }
        if canonical && let Some(id) = self.index.get(t) {
            return Ok(*id);
        }
        Ok(self.temps.intern(t.clone()))
    }

    fn fresh_bnode(&mut self) -> TermId {
        self.temps.fresh_bnode()
    }
}

/// EXISTS stub: answers from a closure over (id, row).
pub struct FnExists<F: FnMut(ExistsId, &[TermId]) -> bool>(pub F);

impl<F: FnMut(ExistsId, &[TermId]) -> bool> ExistsEval for FnExists<F> {
    fn exists(&mut self, id: ExistsId, row: &[TermId]) -> Result<bool> {
        Ok((self.0)(id, row))
    }
}

/// EXISTS stub that reports a fatal error (expressions must not contain EXISTS).
pub struct NoExists;

impl ExistsEval for NoExists {
    fn exists(&mut self, _id: ExistsId, _row: &[TermId]) -> Result<bool> {
        Err(Error::internal("unexpected EXISTS"))
    }
}

/// Everything needed to build an `EvalCtx`.
pub struct TestEnv {
    pub resolver: TestResolver,
    pub values: ValueCache,
    pub rng: StdRng,
    pub now: oxsdatatypes::DateTime,
    pub base: Option<String>,
}

impl Default for TestEnv {
    fn default() -> Self {
        Self::new()
    }
}

impl TestEnv {
    pub fn new() -> Self {
        TestEnv {
            resolver: TestResolver::new(),
            values: ValueCache::new(),
            rng: StdRng::seed_from_u64(7),
            now: oxsdatatypes::DateTime::from_str("2026-10-15T12:00:00Z").expect("valid"),
            base: None,
        }
    }

    /// Runs `f` with an `EvalCtx` whose EXISTS answers come from `exists`.
    pub fn with_exists<R>(
        &mut self,
        exists: &mut dyn ExistsEval,
        f: impl FnOnce(&mut EvalCtx<'_>) -> R,
    ) -> R {
        let mut cx = EvalCtx {
            resolver: &mut self.resolver,
            exists,
            values: &mut self.values,
            now: self.now,
            base_iri: self.base.as_deref(),
            rng: &mut self.rng,
        };
        f(&mut cx)
    }

    /// Runs `f` with an `EvalCtx` (EXISTS is a fatal error).
    pub fn with<R>(&mut self, f: impl FnOnce(&mut EvalCtx<'_>) -> R) -> R {
        let mut no = NoExists;
        self.with_exists(&mut no, f)
    }
}
