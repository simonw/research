//! Term functions: `sparql_term`, `sparql_term_value`, `sparql_term_id` and the
//! `sparql_term_kind` / `_lex` / `_lang` / `_datatype` accessors.
//!
//! An id that does not denote a term of this store — 0 (unbound), an execution-local TEMP
//! id, the ERROR sentinel, a reserved tag or an invalid inline payload — gives NULL. Inline
//! and blank-node ids decode without I/O; dictionary ids are read inside a read entry, and a
//! dictionary id without a row is an error.
use std::sync::Arc;

use rusqlite::functions::Context;
use rusqlite::types::{Value, ValueRef};
use sq_core::codec::{self, RDF_LANG_STRING_IRI, XSD_STRING_IRI};
use sq_core::store::Dictionary;
use sq_core::{Error, Result, Tag, Term, TermId};

use super::text_arg;
use crate::state::ConnState;
use crate::value::{native, parse_term, write_ntriples};

/// The id argument (NULL → None; non-integers are an error).
fn id_arg(ctx: &Context<'_>) -> Result<Option<TermId>> {
    match ctx.get_raw(0) {
        ValueRef::Null => Ok(None),
        ValueRef::Integer(v) => Ok(Some(TermId(v))),
        other => Err(Error::Invalid(format!("the term id must be an INTEGER, got {}", other.data_type()))),
    }
}

/// Decodes a term id of this store (None for ids that denote no stored term).
pub(crate) fn decode(st: &ConnState, id: TermId) -> Result<Option<Arc<Term>>> {
    match id.tag() {
        Tag::Unbound | Tag::Temp | Tag::Error | Tag::Reserved12 | Tag::Reserved13 => Ok(None),
        _ if id.is_inline() => Ok(codec::decode_inline(id).map(Arc::new)),
        _ if id.raw() < 0 => Ok(None),
        Tag::BNode => Ok(Some(Arc::new(Term::BNode(codec::bnode_label(id).into())))),
        _ => st.read_call(|| match st.store().decode_one(id) {
            Ok(t) => Ok(Some(t)),
            Err(Error::Internal(_)) => Err(Error::Invalid(format!("{} is not a term id of this store", id.raw()))),
            Err(e) => Err(e),
        }),
    }
}

fn with_term<T>(st: &ConnState, ctx: &Context<'_>, f: impl FnOnce(&Term) -> Option<T>) -> Result<Option<T>> {
    let Some(id) = id_arg(ctx)? else { return Ok(None) };
    Ok(decode(st, id)?.and_then(|t| f(&t)))
}

/// `sparql_term(id)`: N-Triples text.
pub(super) fn term(st: &ConnState, ctx: &Context<'_>) -> Result<Option<String>> {
    with_term(st, ctx, |t| {
        let mut s = String::new();
        write_ntriples(t, &mut s);
        Some(s)
    })
}

/// `sparql_term_value(id)`: native SQL value (§8.4).
pub(super) fn term_value(st: &ConnState, ctx: &Context<'_>) -> Result<Value> {
    Ok(with_term(st, ctx, |t| Some(native(Some(t)).into_value()))?.unwrap_or(Value::Null))
}

/// `sparql_term_id(nt)`: the id of an existing term (lookup only, never allocates).
pub(super) fn term_id(st: &ConnState, ctx: &Context<'_>) -> Result<Option<i64>> {
    let term = parse_term(text_arg(ctx, 0, "term")?)?;
    if let Some(id) = codec::encode_inline(term.as_ref()) {
        return Ok(Some(id.raw()));
    }
    st.read_call(|| Ok(st.store().lookup(term.as_ref())?.map(TermId::raw)))
}

/// `sparql_term_kind(id)`: `iri`, `bnode` or `literal` (no I/O).
pub(super) fn kind(_st: &ConnState, ctx: &Context<'_>) -> Result<Option<&'static str>> {
    let Some(id) = id_arg(ctx)? else { return Ok(None) };
    if !id.is_storable() || (id.raw() < 0 && !id.is_inline()) {
        return Ok(None);
    }
    Ok(Some(match id.kind() {
        sq_core::Kind::Iri => "iri",
        sq_core::Kind::BNode => "bnode",
        sq_core::Kind::Literal => "literal",
        sq_core::Kind::Unbound => return Ok(None),
    }))
}

/// `sparql_term_lex(id)`: IRI, blank-node label or lexical form.
pub(super) fn lex(st: &ConnState, ctx: &Context<'_>) -> Result<Option<String>> {
    with_term(st, ctx, |t| {
        Some(match t {
            Term::Iri(s) | Term::BNode(s) => s.to_string(),
            Term::Literal(l) => l.lex.to_string(),
        })
    })
}

/// `sparql_term_lang(id)`: language tag of a language-tagged literal.
pub(super) fn lang(st: &ConnState, ctx: &Context<'_>) -> Result<Option<String>> {
    with_term(st, ctx, |t| match t {
        Term::Literal(l) => l.lang.as_deref().map(str::to_string),
        _ => None,
    })
}

/// `sparql_term_datatype(id)`: datatype IRI of a literal (xsd:string for simple literals,
/// rdf:langString for language-tagged ones).
pub(super) fn datatype(st: &ConnState, ctx: &Context<'_>) -> Result<Option<String>> {
    with_term(st, ctx, |t| match t {
        Term::Literal(l) if l.lang.is_some() => Some(RDF_LANG_STRING_IRI.to_string()),
        Term::Literal(l) if l.datatype.is_empty() => Some(XSD_STRING_IRI.to_string()),
        Term::Literal(l) => Some(l.datatype.to_string()),
        _ => None,
    })
}
