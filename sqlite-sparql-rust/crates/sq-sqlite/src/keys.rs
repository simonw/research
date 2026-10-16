//! Dictionary identity of non-inline terms: `(tag, lex, extra)` with extra = datatype IRI
//! (TYPED), lower-case language (LANG), "" otherwise — the same identity as
//! `sq_core::codec::TermKey`, but borrowable so hash maps can be probed without allocating.
use std::borrow::{Borrow, Cow};
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

use rustc_hash::FxHashMap;
use sq_core::codec::{self, WELL_KNOWN};
use sq_core::{Tag, TermId};

pub(crate) trait KeyLike {
    fn tag(&self) -> Tag;
    fn lex(&self) -> &str;
    fn extra(&self) -> &str;
}

impl Hash for dyn KeyLike + '_ {
    fn hash<H: Hasher>(&self, h: &mut H) {
        (self.tag() as u8).hash(h);
        self.lex().hash(h);
        self.extra().hash(h);
    }
}

impl PartialEq for dyn KeyLike + '_ {
    fn eq(&self, other: &Self) -> bool {
        self.tag() == other.tag() && self.lex() == other.lex() && self.extra() == other.extra()
    }
}

impl Eq for dyn KeyLike + '_ {}

/// Owned key (map keys, LookupCache entries).
#[derive(Clone, Debug)]
pub(crate) struct OwnedKey {
    pub tag: Tag,
    pub lex: Box<str>,
    pub extra: Box<str>,
}

impl KeyLike for OwnedKey {
    fn tag(&self) -> Tag {
        self.tag
    }
    fn lex(&self) -> &str {
        &self.lex
    }
    fn extra(&self) -> &str {
        &self.extra
    }
}

impl Hash for OwnedKey {
    fn hash<H: Hasher>(&self, h: &mut H) {
        (self as &dyn KeyLike).hash(h)
    }
}

impl PartialEq for OwnedKey {
    fn eq(&self, other: &Self) -> bool {
        (self as &dyn KeyLike) == (other as &dyn KeyLike)
    }
}

impl Eq for OwnedKey {}

impl<'a> Borrow<dyn KeyLike + 'a> for OwnedKey {
    fn borrow(&self) -> &(dyn KeyLike + 'a) {
        self
    }
}

/// Borrowed key of a term (lower-cases a language tag only if needed).
#[derive(Debug)]
pub(crate) struct KeyView<'a> {
    pub tag: Tag,
    pub lex: &'a str,
    pub extra: Cow<'a, str>,
}

impl KeyLike for KeyView<'_> {
    fn tag(&self) -> Tag {
        self.tag
    }
    fn lex(&self) -> &str {
        self.lex
    }
    fn extra(&self) -> &str {
        &self.extra
    }
}

impl<'a> KeyView<'a> {
    /// Key of an IRI or literal. The caller has already handled inline-encodable literals
    /// and blank nodes (which return None).
    pub fn of(t: oxrdf::TermRef<'a>) -> Option<KeyView<'a>> {
        match t {
            oxrdf::TermRef::NamedNode(n) => Some(KeyView::iri(n.as_str())),
            oxrdf::TermRef::Literal(l) => Some(match l.language() {
                Some(lang) => KeyView {
                    tag: Tag::Lang,
                    lex: l.value(),
                    extra: if lang.bytes().any(|b| b.is_ascii_uppercase()) {
                        Cow::Owned(lang.to_ascii_lowercase())
                    } else {
                        Cow::Borrowed(lang)
                    },
                },
                None if l.datatype().as_str() == codec::XSD_STRING_IRI => {
                    KeyView { tag: Tag::Str, lex: l.value(), extra: Cow::Borrowed("") }
                }
                None => KeyView { tag: Tag::Typed, lex: l.value(), extra: Cow::Borrowed(l.datatype().as_str()) },
            }),
            _ => None,
        }
    }

    pub fn iri(iri: &'a str) -> KeyView<'a> {
        KeyView { tag: Tag::Iri, lex: iri, extra: Cow::Borrowed("") }
    }

    /// `rdf_term.h` (frozen `codec::term_hash`).
    pub fn hash64(&self) -> i64 {
        codec::term_hash(self.tag, self.lex, &self.extra)
    }

    pub fn to_owned_key(&self) -> OwnedKey {
        OwnedKey { tag: self.tag, lex: self.lex.into(), extra: (*self.extra).into() }
    }

    pub fn as_dyn(&self) -> &(dyn KeyLike + 'a) {
        self
    }
}

/// Id of a `codec::WELL_KNOWN` IRI (rows 1..=63 are fixed by `sparql_init`): no I/O.
pub(crate) fn well_known_iri(iri: &str) -> Option<TermId> {
    static MAP: OnceLock<FxHashMap<&'static str, TermId>> = OnceLock::new();
    MAP.get_or_init(|| WELL_KNOWN.iter().map(|(row, iri)| (*iri, TermId::from_dict(*row, Tag::Iri))).collect())
        .get(iri)
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sq_core::codec::TermKey;

    #[test]
    fn key_view_matches_codec_term_key() {
        let dt = oxrdf::NamedNode::new("http://www.w3.org/2001/XMLSchema#double").unwrap();
        let terms: Vec<oxrdf::Term> = vec![
            oxrdf::NamedNode::new("http://ex/a").unwrap().into(),
            oxrdf::Literal::new_simple_literal("a long simple literal").into(),
            oxrdf::Literal::new_language_tagged_literal("chat", "FR-be").unwrap().into(),
            oxrdf::Literal::new_typed_literal("1.5e0", dt).into(),
        ];
        for t in &terms {
            let v = KeyView::of(t.as_ref()).unwrap();
            let k = TermKey::of(t.as_ref()).unwrap();
            assert_eq!((v.tag, v.lex, &*v.extra), (k.tag, &*k.lex, &*k.extra));
            assert_eq!(v.hash64(), k.hash64());
        }
        // Borrowed probes find owned keys.
        let mut m: FxHashMap<OwnedKey, u32> = FxHashMap::default();
        let v = KeyView::of(terms[2].as_ref()).unwrap();
        m.insert(v.to_owned_key(), 7);
        assert_eq!(m.get(v.as_dyn()), Some(&7));
        let other = KeyView { tag: Tag::Str, lex: "chat", extra: Cow::Borrowed("fr-be") };
        assert_eq!(m.get(other.as_dyn()), None);
    }

    #[test]
    fn well_known_ids() {
        assert_eq!(well_known_iri(codec::XSD_INTEGER_IRI), Some(codec::XSD_INTEGER));
        assert_eq!(well_known_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"), Some(codec::RDF_TYPE));
        assert_eq!(well_known_iri("http://ex/a"), None);
    }
}
