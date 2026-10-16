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

/// Checks that `t` is written correctly in N-Triples syntax (also Turtle and TSV, which share
/// its term syntax): IRIs (also datatypes) hold no character that `IRIREF` forbids
/// (`<>"{}|^`\`, controls, space), language tags match `[a-zA-Z]+(-[a-zA-Z0-9]+)*` and blank
/// node labels are valid. Lexical forms need no check: the serializers escape them.
///
/// Every input path validates terms, but the dictionary rows of a database file are not
/// re-validated when they are decoded: a hostile or corrupted file could otherwise plant an
/// IRI such as `http://ex/b> } ; DROP ALL ; INSERT DATA { <x` and have it printed verbatim (a
/// SPARQL injection for applications that splice `sparql_term()` output into an update).
/// Escaping such a character as `\u003E` would not help, since the IRI stays invalid, so the
/// term is refused.
pub fn check_ntriples_term(t: &Term) -> Result<()> {
    match t {
        Term::Iri(iri) => check_iri(iri),
        Term::BNode(label) => match BlankNodeRef::new(label) {
            Ok(_) => Ok(()),
            Err(_) => Err(corrupt("blank node label", label)),
        },
        Term::Literal(l) => {
            if let Some(lang) = &l.lang {
                if !valid_lang(lang) {
                    return Err(corrupt("language tag", lang));
                }
            }
            check_iri(&l.datatype)
        }
    }
}

fn check_iri(iri: &str) -> Result<()> {
    let bad = |c: char| c <= ' ' || matches!(c, '<' | '>' | '"' | '{' | '}' | '|' | '^' | '`' | '\\');
    if iri.chars().any(bad) {
        return Err(corrupt("IRI", iri));
    }
    Ok(())
}

/// `[a-zA-Z]+(-[a-zA-Z0-9]+)*`, the N-Triples / Turtle / SPARQL `LANGTAG`.
fn valid_lang(lang: &str) -> bool {
    let mut parts = lang.split('-');
    let first = parts.next().unwrap_or("");
    !first.is_empty()
        && first.bytes().all(|b| b.is_ascii_alphabetic())
        && parts.all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_alphanumeric()))
}

fn corrupt(what: &str, text: &str) -> Error {
    let shown: String = text.chars().take(80).collect();
    Error::Invalid(format!(
        "the store holds a {what} that is not valid RDF ({shown:?}); the database file is corrupt or was \
         edited outside sparqlite, so the term is not written in N-Triples, Turtle or TSV syntax"
    ))
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

    /// Terms a hostile database file can plant are refused; valid ones pass.
    #[test]
    fn ntriples_term_check_refuses_what_ntriples_cannot_write() {
        let ok = [
            Term::Iri("http://ex/a?b=c#d".into()),
            Term::Iri("http://ex/é".into()),
            Term::BNode("b1f".into()),
            Term::Literal(Literal::simple("any \"text\" > <x> \n")),
            Term::Literal(Literal::lang("chat", "fr-BE")),
            Term::Literal(Literal::lang("x", "en-US-x-1990")),
            Term::Literal(Literal::typed("1", "http://www.w3.org/2001/XMLSchema#integer")),
        ];
        for t in &ok {
            assert!(check_ntriples_term(t).is_ok(), "{t:?}");
        }
        let bad = [
            Term::Iri("http://ex/b> } ; DROP ALL ; INSERT DATA { <x".into()),
            Term::Iri("http://ex/a b".into()),
            Term::Iri("http://ex/a\nb".into()),
            Term::Iri("http://ex/{x}".into()),
            Term::Iri("http://ex/a\\b".into()),
            Term::BNode("b1 } ; DROP ALL".into()),
            Term::Literal(Literal::lang("lit", "en\"><script>")),
            Term::Literal(Literal::lang("lit", "")),
            Term::Literal(Literal::lang("lit", "en-")),
            Term::Literal(Literal::lang("lit", "1en")),
            Term::Literal(Literal::typed("1", "http://ex/dt> . <x")),
        ];
        for t in &bad {
            let e = check_ntriples_term(t).unwrap_err().to_string();
            assert!(e.contains("not valid RDF"), "{t:?}: {e}");
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
