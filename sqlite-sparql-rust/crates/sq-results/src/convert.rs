//! Zero-copy views of decoded [`Term`]s as oxrdf references, so serializers can borrow the
//! decoded terms of a batch without allocating per value.
use oxrdf::{BlankNodeRef, LiteralRef, NamedNodeRef, NamedOrBlankNodeRef, TermRef};
use sq_core::codec::XSD_STRING_IRI;
use sq_core::term::{Literal, Term};
use sq_core::{Error, Result};

/// Borrows `t` as an oxrdf term. Labels and IRIs are trusted: they come from the store's
/// dictionary, the codec or the execution's `TempDict`, all of which hold valid terms.
pub(crate) fn term_ref(t: &Term) -> TermRef<'_> {
    match t {
        Term::Iri(iri) => NamedNodeRef::new_unchecked(iri).into(),
        Term::BNode(label) => BlankNodeRef::new_unchecked(label).into(),
        Term::Literal(l) => literal_ref(l).into(),
    }
}

pub(crate) fn literal_ref(l: &Literal) -> LiteralRef<'_> {
    match &l.lang {
        Some(lang) => LiteralRef::new_language_tagged_literal_unchecked(&l.lex, lang),
        None if &*l.datatype == XSD_STRING_IRI => LiteralRef::new_simple_literal(&l.lex),
        None => LiteralRef::new_typed_literal(&l.lex, NamedNodeRef::new_unchecked(&l.datatype)),
    }
}

/// A triple subject: IRI or blank node.
pub(crate) fn subject_ref(t: &Term) -> Result<NamedOrBlankNodeRef<'_>> {
    match t {
        Term::Iri(iri) => Ok(NamedNodeRef::new_unchecked(iri).into()),
        Term::BNode(label) => Ok(BlankNodeRef::new_unchecked(label).into()),
        Term::Literal(_) => Err(Error::internal("graph result contains a triple with a literal subject")),
    }
}

/// A triple predicate: IRI only.
pub(crate) fn predicate_ref(t: &Term) -> Result<NamedNodeRef<'_>> {
    match t {
        Term::Iri(iri) => Ok(NamedNodeRef::new_unchecked(iri)),
        _ => Err(Error::internal("graph result contains a triple whose predicate is not an IRI")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn views_match_owned_conversion() {
        let terms = [
            Term::Iri("http://ex/a".into()),
            Term::BNode("b1f".into()),
            Term::Literal(Literal::simple("plain")),
            Term::Literal(Literal::lang("chat", "FR")),
            Term::Literal(Literal::typed("1", "http://www.w3.org/2001/XMLSchema#integer")),
        ];
        for t in &terms {
            assert_eq!(term_ref(t).into_owned(), t.to_oxrdf(), "{t:?}");
        }
    }

    #[test]
    fn triple_positions_are_checked() {
        let lit = Term::Literal(Literal::simple("x"));
        let bnode = Term::BNode("b1".into());
        assert!(subject_ref(&lit).is_err());
        assert!(subject_ref(&bnode).is_ok());
        assert!(predicate_ref(&bnode).is_err());
        assert!(predicate_ref(&Term::Iri("http://ex/p".into())).is_ok());
    }
}
