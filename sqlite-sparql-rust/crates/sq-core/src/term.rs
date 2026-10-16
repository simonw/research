//! Owned, cheaply clonable decoded terms (the dictionary / TempDict / output currency).
use std::sync::Arc;

use crate::codec::{XSD_STRING_IRI, RDF_LANG_STRING_IRI};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Term {
    Iri(Arc<str>),
    /// Label without "_:". Store blank nodes are "b<hex counter>", TEMP ones "t<index>".
    BNode(Arc<str>),
    Literal(Literal),
}

/// `datatype` is the full IRI. Simple literals have datatype xsd:string and `lang == None`;
/// language-tagged literals have datatype rdf:langString and a lower-case `lang`.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Literal {
    pub lex: Arc<str>,
    pub datatype: Arc<str>,
    pub lang: Option<Arc<str>>,
}

impl Literal {
    pub fn simple(lex: &str) -> Self {
        Literal { lex: lex.into(), datatype: XSD_STRING_IRI.into(), lang: None }
    }
    pub fn lang(lex: &str, lang: &str) -> Self {
        Literal { lex: lex.into(), datatype: RDF_LANG_STRING_IRI.into(), lang: Some(lang.to_ascii_lowercase().into()) }
    }
    pub fn typed(lex: &str, datatype: &str) -> Self {
        Literal { lex: lex.into(), datatype: datatype.into(), lang: None }
    }
    pub fn is_simple(&self) -> bool {
        self.lang.is_none() && &*self.datatype == XSD_STRING_IRI
    }
}

impl Term {
    pub fn from_oxrdf(t: oxrdf::TermRef<'_>) -> Term {
        #[allow(unreachable_patterns)]
        match t {
            oxrdf::TermRef::NamedNode(n) => Term::Iri(n.as_str().into()),
            oxrdf::TermRef::BlankNode(b) => Term::BNode(b.as_str().into()),
            oxrdf::TermRef::Literal(l) => Term::Literal(Literal {
                lex: l.value().into(),
                datatype: l.datatype().as_str().into(),
                lang: l.language().map(Into::into),
            }),
            _ => panic!("RDF 1.2 triple terms are not supported (oxrdf rdf-12 must stay disabled)"),
        }
    }

    pub fn to_oxrdf(&self) -> oxrdf::Term {
        match self {
            Term::Iri(i) => oxrdf::NamedNode::new_unchecked(i.to_string()).into(),
            Term::BNode(b) => oxrdf::BlankNode::new_unchecked(b.to_string()).into(),
            Term::Literal(l) => match &l.lang {
                Some(lang) => oxrdf::Literal::new_language_tagged_literal_unchecked(l.lex.to_string(), lang.to_string()).into(),
                None if &*l.datatype == XSD_STRING_IRI => oxrdf::Literal::new_simple_literal(l.lex.to_string()).into(),
                None => oxrdf::Literal::new_typed_literal(
                    l.lex.to_string(),
                    oxrdf::NamedNode::new_unchecked(l.datatype.to_string()),
                )
                .into(),
            },
        }
    }

    /// N-Triples serialization (the `t0..t15` columns, `sparql_term()`).
    pub fn to_ntriples(&self) -> String {
        self.to_oxrdf().to_string()
    }

    pub fn kind(&self) -> crate::Kind {
        match self {
            Term::Iri(_) => crate::Kind::Iri,
            Term::BNode(_) => crate::Kind::BNode,
            Term::Literal(_) => crate::Kind::Literal,
        }
    }
}
