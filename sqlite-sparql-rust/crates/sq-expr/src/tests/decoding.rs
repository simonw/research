//! Decoding discipline: the `ValueCache` admits a value on the second decode of its id, the
//! value class of a non-TYPED id comes from its tag (no datatype comparison), string
//! arguments of non-string ids are type errors decided without decoding, and
//! `CompiledExpr::string_slots` names exactly the slots a host may decode ahead of a batch.
use std::sync::Arc;

use sq_core::eval_api::TermResolver;
use sq_core::{Literal, Result, Term, TermId};

use super::harness::Harness;
use crate::testing::{NoExists, TestResolver};
use crate::{EvalCtx, Value, ValueCache, value_of};

/// A resolver that counts `term` calls (decodes reaching the host).
struct Counting<'a> {
    inner: &'a mut TestResolver,
    terms: usize,
}

impl TermResolver for Counting<'_> {
    fn term(&mut self, id: TermId) -> Result<Arc<Term>> {
        self.terms += 1;
        self.inner.term(id)
    }
    fn intern(&mut self, t: &Term, canonical: bool) -> Result<TermId> {
        self.inner.intern(t, canonical)
    }
    fn fresh_bnode(&mut self) -> TermId {
        self.inner.fresh_bnode()
    }
}

/// Runs `f` on an `EvalCtx` over `h`'s resolver and the given cache; returns `f`'s result and
/// the number of decodes that reached the resolver.
fn counted<R>(h: &mut Harness, values: &mut ValueCache, f: impl FnOnce(&mut EvalCtx<'_>) -> R) -> (R, usize) {
    let mut res = Counting { inner: &mut h.env.resolver, terms: 0 };
    let mut no = NoExists;
    let mut cx = EvalCtx {
        resolver: &mut res,
        exists: &mut no,
        values,
        now: h.env.now,
        base_iri: None,
        rng: &mut h.env.rng,
    };
    let r = f(&mut cx);
    (r, res.terms)
}

#[test]
fn values_are_cached_from_the_second_decode() {
    let mut h = Harness::new();
    let a = h.term(r#""a dictionary string""#);
    let b = h.term(r#""another dictionary string""#);
    let mut values = ValueCache::new();
    let ((), n) = counted(&mut h, &mut values, |cx| {
        for id in [a, b] {
            value_of(id, cx).unwrap();
        }
    });
    assert_eq!((n, values.len()), (2, 0), "a single decode is not cached");
    let ((), n) = counted(&mut h, &mut values, |cx| {
        value_of(a, cx).unwrap();
    });
    assert_eq!((n, values.len()), (1, 1), "the second decode is cached");
    let ((), n) = counted(&mut h, &mut values, |cx| {
        for _ in 0..3 {
            assert!(matches!(value_of(a, cx).unwrap(), Value::Str(s) if &*s == "a dictionary string"));
        }
    });
    assert_eq!((n, values.len()), (0, 1), "later decodes hit the cache");
    // `clear` empties the values; the admission filter remembers the id, so the next decode
    // caches it again.
    values.clear();
    let ((), n) = counted(&mut h, &mut values, |cx| {
        value_of(a, cx).unwrap();
        value_of(a, cx).unwrap();
    });
    assert_eq!((n, values.len()), (1, 1));
}

#[test]
fn single_use_ids_never_fill_the_cache() {
    let mut h = Harness::new();
    let ids: Vec<TermId> = (0..20_000).map(|i| h.term(&format!("\"distinct name {i}\""))).collect();
    let mut values = ValueCache::new();
    let ((), n) = counted(&mut h, &mut values, |cx| {
        for &id in &ids {
            value_of(id, cx).unwrap();
        }
    });
    assert_eq!(n, ids.len());
    assert!(values.is_empty());
    // A bounded cache still works for repeated ids (cleared when full).
    let mut small = ValueCache::with_limit(16);
    let ((), _) = counted(&mut h, &mut small, |cx| {
        for _ in 0..3 {
            for &id in &ids[..100] {
                value_of(id, cx).unwrap();
            }
        }
    });
    assert!(!small.is_empty() && small.len() <= 16);
}

/// The value class comes from the id's tag for IRIs, blank nodes, strings and language
/// strings (dictionary, TEMP and inline ids), and agrees with classifying the decoded term.
#[test]
fn values_by_tag_match_values_by_datatype() {
    let mut h = Harness::new();
    let terms = [
        Term::Iri("http://example.org/a".into()),
        Term::Literal(Literal::simple("a long simple literal")),
        Term::Literal(Literal::simple("short")),
        Term::Literal(Literal::typed("an explicit xsd:string", "http://www.w3.org/2001/XMLSchema#string")),
        Term::Literal(Literal::lang("chat", "fr")),
        Term::Literal(Literal::typed("1.5e0", "http://www.w3.org/2001/XMLSchema#double")),
        Term::Literal(Literal::typed("300", "http://www.w3.org/2001/XMLSchema#byte")),
        Term::Literal(Literal::typed("x", "http://example.org/dt")),
        Term::Literal(Literal::typed("0012", "http://www.w3.org/2001/XMLSchema#integer")),
    ];
    let mut ids: Vec<TermId> = terms.iter().map(|t| h.env.resolver.add(t.clone())).collect();
    for t in &terms[..] {
        if let Term::Literal(l) = t
            && sq_core::codec::encode_inline_literal(&l.lex, &l.datatype, l.lang.as_deref()).is_some()
        {
            continue;
        }
        ids.push(h.env.resolver.temp(t.clone()));
    }
    ids.push(h.env.resolver.bnode());
    ids.push(h.env.resolver.temps.fresh_bnode());
    let mut values = ValueCache::new();
    for id in ids {
        let term = h.env.resolver.term(id).unwrap();
        let expected = format!("{:?}", Value::from_term(&term));
        // Twice: the first decode classifies the term, the second hits the cache.
        for _ in 0..2 {
            let (got, _) = counted(&mut h, &mut values, |cx| value_of(id, cx).unwrap());
            assert_eq!(format!("{got:?}"), expected, "{term:?}");
        }
    }
}

#[test]
fn string_arguments_of_other_ids_are_errors_without_decoding() {
    let mut h = Harness::new();
    let others = [
        h.term("<http://example.org/x>"),
        h.term(r#""1.5e0"^^xsd:double"#),
        h.term(r#""x"^^<http://example.org/dt>"#),
        h.term(r#""0012"^^xsd:integer"#),
        h.term("_:b"),
        h.term("12"),
        h.term("true"),
        h.env.resolver.temp(Term::Iri("http://example.org/absent".into())),
    ];
    for e in [r#"REGEX(?x, "a")"#, r#"CONTAINS(?x, "a")"#, "STRLEN(?x)", "UCASE(?x)", "LANGMATCHES(?x, \"en\")", "MD5(?x)"] {
        let c = h.compile(e);
        for &id in &others {
            let row = h.row(&[("x", id)]);
            let mut values = ValueCache::new();
            let (r, n) = counted(&mut h, &mut values, |cx| c.eval_val(&row, cx).unwrap());
            assert!(r.is_none(), "{e} on {id:?}");
            assert_eq!(n, 0, "{e} on {id:?} decoded the id");
        }
    }
    // String ids are decoded and evaluated.
    let s = h.term(r#""a dictionary string""#);
    let l = h.term(r#""a language string"@en"#);
    assert_eq!(h.ebv(r#"REGEX(?x, "string$")"#, &[("x", s)]), Some(true));
    assert_eq!(h.ebv(r#"REGEX(?x, "string$")"#, &[("x", l)]), Some(true));
    assert_eq!(h.show(r#"MD5(?x)"#, &[("x", l)]), "ERROR", "MD5 needs a simple literal");
}

#[test]
fn string_slots_are_the_strings_decoded_first() {
    let mut h = Harness::new();
    for (e, want) in [
        (r#"REGEX(?n, "x", "i")"#, vec!["n"]),
        (r#"REGEX(LCASE(?n), "x")"#, vec!["n"]),
        (r#"REGEX(?n, ?p)"#, vec!["n"]),
        (r#"REPLACE(?n, "a", "b")"#, vec!["n"]),
        (r#"CONTAINS(?a, ?b)"#, vec!["a"]),
        (r#"STRLEN(?n) > 3"#, vec!["n"]),
        (r#"UCASE(?n) = "A""#, vec!["n"]),
        (r#""A" = UCASE(?n)"#, vec!["n"]),
        (r#"!REGEX(?n, "x")"#, vec!["n"]),
        (r#"REGEX(?n, "x") && ?age > 3"#, vec!["n"]),
        (r#"REGEX(?n, "x") || ?age > 3"#, vec!["n"]),
        (r#"COALESCE(UCASE(?n), "x")"#, vec!["n"]),
        (r#"IF(CONTAINS(?n, "a"), 1, 2)"#, vec!["n"]),
        (r#"CONCAT(?a, ?b)"#, vec!["a"]),
        (r#"SUBSTR(?n, 2)"#, vec!["n"]),
        (r#"STRSTARTS(?n, "a") && STRENDS(?m, "b")"#, vec!["n"]),
        // Nothing decoded first, or not necessarily a string decode.
        (r#"?age > 3 && REGEX(?n, "x")"#, vec![]),
        (r#"BOUND(?n) && REGEX(?n, "x")"#, vec![]),
        (r#"?n = "abc""#, vec![]),
        (r#"isIRI(?n)"#, vec![]),
        (r#"STR(?n)"#, vec![]),
        (r#"LANG(?n) = "en""#, vec![]),
        (r#"LANGMATCHES(LANG(?n), "en")"#, vec![]),
        (r#"?n < "b""#, vec![]),
        ("?n", vec![]),
    ] {
        let c = h.compile(e);
        let want: Vec<sq_core::Slot> = want.iter().map(|v| h.low.slot(v)).collect();
        assert_eq!(c.string_slots(), want, "{e}");
    }
}
