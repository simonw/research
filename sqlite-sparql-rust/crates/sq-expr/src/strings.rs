//! String builtin helpers (§6.6): argument compatibility, fn:substring, ENCODE_FOR_URI,
//! LANGMATCHES and the hash functions. All positions are Unicode code points.
use std::fmt::Write as _;
use std::sync::Arc;

use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha384, Sha512};

use crate::numeric::f64_round;
use crate::value::Value;

/// A string literal argument: simple / xsd:string (`lang == None`) or language-tagged.
#[derive(Clone, Debug)]
pub struct StrArg {
    pub s: Arc<str>,
    pub lang: Option<Arc<str>>,
}

impl StrArg {
    /// Accepts simple literals, xsd:string and language-tagged literals.
    pub fn from_value(v: Value) -> Option<StrArg> {
        match v {
            Value::Str(s) => Some(StrArg { s, lang: None }),
            Value::LangStr { lex, lang } => Some(StrArg { s: lex, lang: Some(lang) }),
            _ => None,
        }
    }

    /// A result string carrying this argument's language tag.
    pub fn with_same_tag(&self, s: &str) -> Value {
        match &self.lang {
            Some(lang) => Value::LangStr { lex: s.into(), lang: lang.clone() },
            None => Value::Str(s.into()),
        }
    }
}

/// Argument compatibility of the two-argument string functions: (simple|xsd:string,
/// simple|xsd:string), (lang L, lang L) and (lang, simple). Everything else is an error.
pub fn compatible(a: &StrArg, b: &StrArg) -> bool {
    match (&a.lang, &b.lang) {
        (_, None) => true,
        (Some(x), Some(y)) => x.eq_ignore_ascii_case(y),
        (None, Some(_)) => false,
    }
}

/// XPath fn:substring: the code points at 1-based positions p with
/// round(start) ≤ p < round(start) + round(len) (NaN and infinities follow IEEE rules).
pub fn substring(s: &str, start: f64, len: Option<f64>) -> &str {
    let first_pos = f64_round(start);
    let end_pos = match len {
        Some(l) => first_pos + f64_round(l),
        None => f64::INFINITY,
    };
    let mut from = None;
    let mut to = s.len();
    for (p, (i, _)) in (1u64..).zip(s.char_indices()) {
        let p = p as f64;
        match from {
            None if p >= first_pos && p < end_pos => from = Some(i),
            Some(_) if p >= end_pos => {
                to = i;
                break;
            }
            None if p >= end_pos => break,
            _ => {}
        }
    }
    match from {
        Some(f) => &s[f..to],
        None => "",
    }
}

/// ENCODE_FOR_URI (RFC 3986): percent-encodes every UTF-8 byte except the unreserved set.
pub fn encode_for_uri(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// RFC 4647 basic filtering: `*` matches any non-empty tag; otherwise the range must equal
/// the tag or be a prefix of it followed by `-` (case-insensitive).
pub fn lang_matches(tag: &str, range: &str) -> bool {
    if range == "*" {
        return !tag.is_empty();
    }
    let (t, r) = (tag.as_bytes(), range.as_bytes());
    t.len() >= r.len() && t[..r.len()].eq_ignore_ascii_case(r) && (t.len() == r.len() || t[r.len()] == b'-')
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum HashAlg {
    Md5,
    Sha1,
    Sha256,
    Sha384,
    Sha512,
}

fn hex<D: Digest>(input: &str) -> String {
    let digest = D::digest(input.as_bytes());
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest.iter() {
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// Lower-case hex digest of the UTF-8 bytes.
pub fn hash_hex(alg: HashAlg, input: &str) -> String {
    match alg {
        HashAlg::Md5 => hex::<Md5>(input),
        HashAlg::Sha1 => hex::<Sha1>(input),
        HashAlg::Sha256 => hex::<Sha256>(input),
        HashAlg::Sha384 => hex::<Sha384>(input),
        HashAlg::Sha512 => hex::<Sha512>(input),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substring_follows_xpath_rounding() {
        // Examples from XPath F&O fn:substring.
        assert_eq!(substring("motor car", 6.0, None), " car");
        assert_eq!(substring("metadata", 4.0, Some(3.0)), "ada");
        assert_eq!(substring("12345", 1.5, Some(2.6)), "234");
        assert_eq!(substring("12345", 0.0, Some(3.0)), "12");
        assert_eq!(substring("12345", 5.0, Some(-3.0)), "");
        assert_eq!(substring("12345", -3.0, Some(5.0)), "1");
        assert_eq!(substring("12345", f64::NAN, Some(3.0)), "");
        assert_eq!(substring("12345", 1.0, Some(f64::NAN)), "");
        assert_eq!(substring("12345", -42.0, Some(f64::INFINITY)), "12345");
        assert_eq!(substring("12345", f64::NEG_INFINITY, Some(f64::INFINITY)), "");
        assert_eq!(substring("食べ物", 2.0, Some(1.0)), "べ");
        assert_eq!(substring("abc", 4.0, None), "");
    }

    #[test]
    fn encode_for_uri_examples() {
        assert_eq!(encode_for_uri("Los Angeles"), "Los%20Angeles");
        assert_eq!(encode_for_uri("~bébé"), "~b%C3%A9b%C3%A9");
        assert_eq!(encode_for_uri("100% organic"), "100%25%20organic");
    }

    #[test]
    fn langmatches_basic_filtering() {
        assert!(lang_matches("en", "en"));
        assert!(lang_matches("en-US", "en"));
        assert!(lang_matches("EN-us", "en-US"));
        assert!(!lang_matches("eng", "en"));
        assert!(!lang_matches("en", "en-US"));
        assert!(lang_matches("fr", "*"));
        assert!(!lang_matches("", "*"));
        assert!(lang_matches("", ""));
        assert!(!lang_matches("en", ""));
        assert!(!lang_matches("é-x", "e"));
    }

    #[test]
    fn hashes() {
        assert_eq!(hash_hex(HashAlg::Md5, "abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(hash_hex(HashAlg::Sha1, "abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            hash_hex(HashAlg::Sha256, "abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(hash_hex(HashAlg::Sha384, "").len(), 96);
        assert_eq!(hash_hex(HashAlg::Sha512, "").len(), 128);
    }

    #[test]
    fn compatibility_rules() {
        let s = |x: &str| StrArg { s: x.into(), lang: None };
        let l = |x: &str, g: &str| StrArg { s: x.into(), lang: Some(g.into()) };
        assert!(compatible(&s("a"), &s("b")));
        assert!(compatible(&l("a", "en"), &s("b")));
        assert!(compatible(&l("a", "en"), &l("b", "en")));
        assert!(!compatible(&l("a", "en"), &l("b", "fr")));
        assert!(!compatible(&s("a"), &l("b", "en")));
    }
}
